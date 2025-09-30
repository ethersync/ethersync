// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use ethersync::{
    cli::ask,
    config::{self, AppConfig},
    daemon::Daemon,
    history, logging, sandbox,
};
use std::path::{Path, PathBuf};
use tokio::signal;
use tracing::{debug, info, warn};

mod jsonrpc_forwarder;

// TODO: Define these constants in the ethersync crate, and use them here.
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
    },
    /// Join a shared directory via a join code, or connect to the most recent one.
    Join {
        /// Specify to connect to a new peer. Otherwise, try to connect to the most recent peer.
        join_code: Option<String>,
    },
    /// Remember the current state of the directory, allowing you to compare it later.
    Seenit,
    /// Render the "seenit" state or latest state to a directory.
    Snapshot {
        /// Directory to render the snapshot to.
        target_directory: PathBuf,
        /// Whether to snapshot the "seenit" state. If not provided, snapshot the latest
        /// state.
        #[arg(long)]
        seenit: bool,
    },
    /// Print a summary of changes since the last "seenit" command run.
    Whatsnew {
        /// Which external command to use to compare the two revisions.
        #[arg(long)]
        tool: String,
    },
    /// Open a JSON-RPC connection to the Ethersync daemon on stdin/stdout. Used by text editor plugins.
    Client,
}

fn has_ethersync_directory(dir: &Path) -> bool {
    let ethersync_dir = dir.join(config::CONFIG_DIR);
    // Using the sandbox method here is technically unnecessary,
    // but we want to really run all path operations through the sandbox module.
    sandbox::exists(dir, &ethersync_dir).expect("Failed to check") && ethersync_dir.is_dir()
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

    let directory = get_directory(cli.directory).context("Failed to find .ethersync/ directory")?;

    let config_file = directory.join(config::CONFIG_DIR).join(config::CONFIG_FILE);

    let socket_path = directory
        .join(config::CONFIG_DIR)
        .join(config::DEFAULT_SOCKET_NAME);

    match cli.command {
        Commands::Share { .. } | Commands::Join { .. } => {
            let persist = !config::has_git_remote(&directory);
            if !persist {
                info!(
                    "Detected a Git remote: Assuming a pair-programming use-case and starting a new history."
                );
            }

            config::ensure_ethersync_is_ignored(&directory)?;

            let mut init_doc = false;
            let mut app_config;

            match cli.command {
                Commands::Share {
                    init,
                    no_join_code,
                    show_secret_address,
                    ..
                } => {
                    init_doc = init;
                    let app_config_cli = AppConfig {
                        peer: None,
                        emit_join_code: !no_join_code,
                        emit_secret_address: show_secret_address,
                    };
                    app_config = app_config_cli.merge(AppConfig::from_config_file(&config_file));

                    // Because of the "share" subcommand, explicitly don't connect anywhere.
                    app_config.peer = None;
                }
                Commands::Join { join_code, .. } => {
                    let app_config_cli = AppConfig {
                        peer: join_code.map(config::Peer::JoinCode),
                        emit_join_code: false,
                        emit_secret_address: false,
                    };

                    app_config = app_config_cli.merge(AppConfig::from_config_file(&config_file));

                    app_config = app_config
                        .resolve_peer(&directory, &config_file)
                        .await
                        .context("Failed to resolve peer")?;
                }
                _ => {
                    panic!("This can't happen, as we earlier matched on Share|Join.")
                }
            }

            debug!("Starting Ethersync on {}.", directory.display());
            let _daemon = Daemon::new(app_config, &socket_path, &directory, init_doc, persist)
                .await
                .context("Failed to launch the daemon")?;
            wait_for_shutdown().await;
        }
        Commands::Seenit => {
            history::seenit(&directory)?;
        }
        Commands::Snapshot {
            target_directory,
            seenit,
        } => {
            history::snapshot(&directory, &target_directory, seenit)?;
        }
        Commands::Whatsnew {tool} => {
            history::whatsnew(&directory, tool)?;
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
    if !has_ethersync_directory(&directory) {
        let ethersync_dir = directory.join(config::CONFIG_DIR);

        warn!(
            "{:?} hasn't been used as an Ethersync directory before.",
            &directory
        );

        if ask(&format!(
            "Create an {}/ directory to allow live collaboration?",
            config::CONFIG_DIR
        ))? {
            sandbox::create_dir(&directory, &ethersync_dir)?;
            info!("Created! Resuming launch.");
        } else {
            bail!("Aborting launch. Ethersync needs an .ethersync/ directory to function");
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
