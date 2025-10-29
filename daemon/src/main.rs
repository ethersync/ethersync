// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use self::cli::{Cli, Commands, SyncVcsFlag};
use anyhow::{bail, Context, Result};
use clap::{CommandFactory as _, FromArgMatches as _};
use std::path::{Path, PathBuf};
use teamtype::{
    cli_ask::ask,
    config::{self, AppConfig},
    daemon::Daemon,
    logging, sandbox,
};
use tokio::signal;
use tracing::{debug, info, warn};

mod cli;
mod jsonrpc_forwarder;

fn has_ethersync_directory(dir: &Path) -> bool {
    let ethersync_dir = dir.join(config::LEGACY_CONFIG_DIR);
    // Using the sandbox method here is technically unnecessary,
    // but we want to really run all path operations through the sandbox module.
    sandbox::exists(dir, &ethersync_dir).expect("Failed to check") && ethersync_dir.is_dir()
}

fn has_teamtype_directory(dir: &Path) -> bool {
    let teamtype_dir = dir.join(config::CONFIG_DIR);
    // Using the sandbox method here is technically unnecessary,
    // but we want to really run all path operations through the sandbox module.
    sandbox::exists(dir, &teamtype_dir).expect("Failed to check") && teamtype_dir.is_dir()
}

#[tokio::main]
async fn main() -> Result<()> {
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    let arg_matches = Cli::command().get_matches();
    let cli = match Cli::from_arg_matches(&arg_matches) {
        Ok(cli) => cli,
        Err(e) => e.exit(),
    };

    logging::initialize().context("Failed to initialize logging")?;

    let directory = get_directory(cli.directory).context("Failed to find .teamtype/ directory")?;

    let config_file = directory.join(config::CONFIG_DIR).join(config::CONFIG_FILE);

    let socket_path = directory
        .join(config::CONFIG_DIR)
        .join(config::DEFAULT_SOCKET_NAME);

    match cli.command {
        Commands::Share { .. } | Commands::Join { .. } => {
            let persist = !config::has_git_remote(&directory);
            if !persist {
                // TODO: drop .teamtype/doc here? Would that be rude?
                info!(
                    "Detected a Git remote: Assuming a pair-programming use-case and starting a new history."
                );
            }

            config::ensure_teamtype_is_ignored(&directory)?;

            let mut init_doc = false;
            let mut app_config;

            match cli.command {
                Commands::Share {
                    init,
                    no_join_code,
                    show_secret_address,
                    sync_vcs: SyncVcsFlag { sync_vcs },
                    ..
                } => {
                    init_doc = init;
                    let app_config_cli = AppConfig {
                        base_dir: directory,
                        peer: None,
                        emit_join_code: !no_join_code,
                        emit_secret_address: show_secret_address,
                        sync_vcs,
                    };
                    app_config = app_config_cli.merge(AppConfig::from_config_file(&config_file));

                    // Because of the "share" subcommand, explicitly don't connect anywhere.
                    app_config.peer = None;
                }
                Commands::Join {
                    join_code,
                    sync_vcs: SyncVcsFlag { sync_vcs },
                    ..
                } => {
                    let app_config_cli = AppConfig {
                        base_dir: directory,
                        peer: join_code.map(config::Peer::JoinCode),
                        emit_join_code: false,
                        emit_secret_address: false,
                        sync_vcs,
                    };

                    app_config = app_config_cli.merge(AppConfig::from_config_file(&config_file));

                    app_config = app_config
                        .resolve_peer()
                        .await
                        .context("Failed to resolve peer")?;
                }
                Commands::Client => {
                    panic!("This can't happen, as we earlier matched on Share|Join.")
                }
            }

            if app_config.sync_vcs
                && config::has_local_user_config(&app_config.base_dir).is_ok_and(|v| v)
            {
                warn!("You have a local user configuration in your .git/config. In --sync-vcs mode, this file will also be synchronized between peers. If your version \"wins\", all peers will have the same Git identity. As a workaround, you could use `git commit --author`.");
            }

            debug!("Starting Teamtype on {}.", app_config.base_dir.display());

            // TODO: Derive socket_path inside the constructor.
            let _daemon = Daemon::new(app_config, &socket_path, init_doc, persist)
                .await
                .context("Failed to launch the daemon")?;
            wait_for_shutdown().await;
        }
        Commands::Client => {
            jsonrpc_forwarder::connection(&socket_path)
                .await
                .context("JSON-RPC forwarder failed")?;
        }
    }
    Ok(())
}

fn get_directory(directory: Option<PathBuf>) -> Result<PathBuf> {
    let directory = directory
        .unwrap_or_else(|| std::env::current_dir().expect("Could not access current directory"))
        .canonicalize()
        .expect("Could not access given directory");
    if has_ethersync_directory(&directory) {
        let old_directory = directory.join(config::LEGACY_CONFIG_DIR);

        warn!("You have an '{}/' directory, back from when the project was called \"Ethersync\" until October 2025.", &old_directory.display());

        if ask(&format!(
            "Do you want to rename {}/ to {}/?",
            &config::LEGACY_CONFIG_DIR,
            &config::CONFIG_DIR,
        ))? {
            let new_directory = directory.join(config::CONFIG_DIR);
            sandbox::rename_file(&directory, &old_directory, &new_directory)?;
        } else {
            bail!(
                "Aborting launch. Rename or remove the {} directory yourself to continue.",
                &config::LEGACY_CONFIG_DIR
            );
        }
    }
    if !has_teamtype_directory(&directory) {
        let teamtype_dir = directory.join(config::CONFIG_DIR);

        warn!(
            "'{}' hasn't been used as a Teamtype directory before.",
            &directory.display()
        );

        if ask(&format!(
            "Do you want to enable live collaboration here? (This will create an {}/ directory.)",
            config::CONFIG_DIR
        ))? {
            sandbox::create_dir(&directory, &teamtype_dir)?;
            info!("Created! Resuming launch.");
        } else {
            bail!("Aborting launch. Teamtype needs a .teamtype/ directory to function");
        }
    }
    Ok(directory)
}

async fn wait_for_shutdown() {
    let mut signal_terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
        .expect("Should have been able to create terminate signal stream");
    tokio::select! {
        _ = signal::ctrl_c() => {
            debug!("Got SIGINT (Ctrl+C), shutting down");
        }
        _ = signal_terminate.recv() => {
            debug!("Got SIGTERM, shutting down");
        }
    }
}
