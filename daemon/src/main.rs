// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Context, Result};
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use teamtype::{
    cli::ask,
    config::{self, AppConfig},
    daemon::Daemon,
    history, logging, sandbox,
};
use std::path::{Path, PathBuf};
use tokio::signal;
use tracing::{debug, info, warn};

mod jsonrpc_forwarder;

// TODO: Define these constants in the teamtype crate, and use them here.
#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// The shared directory. Defaults to current directory.
    #[arg(long, global = true)]
    directory: Option<PathBuf>,
}

#[derive(Args)]
struct SyncVcsFlag {
    /// EXPERIMENTAL: Also synchronize version-control directories like .git/ or .jj/, which are normally
    /// ignored. For Git, this will synchronize all branches, commits, etc. as well as your .git/config.
    /// This means that new commits will immediately appear at all peers, you can change branches together, etc.
    #[arg(long)]
    sync_vcs: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Share a directory with a new peer.
    Share {
        /// Re-initialize the history of the shared directory. You will loose previous history.
        #[arg(long)]
        init: bool,
        /// Do not generate a join code. To prevent unintended sharing or simply if you want to
        /// keep Magic Wormhole out of the loop.
        #[arg(long)]
        no_join_code: bool,
        /// Print the secret address. Useful for sharing with multiple people.
        #[arg(long)]
        show_secret_address: bool,
        #[command(flatten)]
        sync_vcs: SyncVcsFlag,
    },
    /// Join a shared directory via a join code, or connect to the most recent one.
    Join {
        /// Specify to connect to a new peer. Otherwise, try to connect to the most recent peer.
        join_code: Option<String>,
        #[command(flatten)]
        sync_vcs: SyncVcsFlag,
    },
    /// Remember the current state of the directory, allowing you to compare it later.
    Bookmark,
    /// Show the differences between the bookmark and the current state with a tool of your choice.
    Diff {
        /// Which external command to use to compare the two revisions. A good option is `meld`.
        #[arg(long)]
        tool: String,
    },
    /// Open a JSON-RPC connection to the Teamtype daemon on stdin/stdout. Used by text editor plugins.
    Client,
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
                _ => {
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
        Commands::Bookmark => {
            history::bookmark(&directory).context("Bookmark command failed")?;
            info!("Successfully created a bookmark. Use `teamtype diff` to later see what changed since bookmarking.");
        }
        Commands::Diff { tool } => {
            history::diff(&directory, &tool).context("Diff command failed")?;
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
    if !has_teamtype_directory(&directory) {
        let teamtype_dir = directory.join(config::CONFIG_DIR);

        warn!(
            "{:?} hasn't been used as an Teamtype directory before.",
            &directory
        );

        if ask(&format!(
            "Do you want to enable live collaboration here? (This will create an {}/ directory.)",
            config::CONFIG_DIR
        ))? {
            sandbox::create_dir(&directory, &teamtype_dir)?;
            info!("Created! Resuming launch.");
        } else {
            bail!("Aborting launch. Teamtype needs an .teamtype/ directory to function");
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
