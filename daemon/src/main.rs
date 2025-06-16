// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Context, Result};
use clap::{parser::ValueSource, CommandFactory, FromArgMatches, Parser, Subcommand};
use ethersync::{
    config::{self, AppConfig},
    daemon::Daemon,
    editor, logging, sandbox,
};
use std::path::{Path, PathBuf};
use tokio::signal;
use tracing::{debug, info};

mod jsonrpc_forwarder;

// TODO: Define these constants in the ethersync crate, and use them here.
const DEFAULT_SOCKET_NAME: &str = "ethersync";
const ETHERSYNC_CONFIG_DIR: &str = ".ethersync";
const ETHERSYNC_CONFIG_FILE: &str = "config";
const ETHERSYNC_SOCKET_ENV_VAR: &str = "ETHERSYNC_SOCKET";

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Path to the Unix domain socket to use for communication between daemon and editors.
    #[arg(
      short, long, global = true,
      default_value = DEFAULT_SOCKET_NAME,
      env = ETHERSYNC_SOCKET_ENV_VAR,
    )]
    socket_name: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Share a directory with a new peer.
    Share {
        /// Re-initialize the history of the shared project. You will loose previous history.
        #[arg(long)]
        init: bool,
        /// Do not generate a join code. To prevent unintended sharing or simply if you want to
        /// keep Magic Wormhole out of the loop.
        #[arg(long)]
        no_join_code: bool,
        /// Do print the secret address. Useful for bulk sharing.
        #[arg(long)]
        show_secret_address: bool,
        /// The directory to share. Defaults to current directory.
        #[arg(long)]
        directory: Option<PathBuf>,
    },
    /// Join a shared project via join code.
    Join {
        /// Specify to connect to a new peer. Otherwise, try to connect to the most recent peer.
        join_code: Option<String>,
        /// The directory to sync. Defaults to current directory.
        #[arg(long)]
        directory: Option<PathBuf>,
    },
    /// Open a JSON-RPC connection to the Ethersync daemon on stdin/stdout. Used by text editor plugins.
    Client,
}

fn has_ethersync_directory(dir: &Path) -> bool {
    let ethersync_dir = dir.join(ETHERSYNC_CONFIG_DIR);
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

    logging::initialize()?;

    let socket_path = editor::get_socket_path(&cli.socket_name);

    match cli.command {
        Commands::Share { ref directory, .. } | Commands::Join { ref directory, .. } => {
            let directory = get_directory(directory.clone())?;
            let config_file = directory
                .join(ETHERSYNC_CONFIG_DIR)
                .join(ETHERSYNC_CONFIG_FILE);
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

                    app_config = app_config.resolve_peer(&directory, &config_file).await?;
                }
                Commands::Client => {
                    panic!("This can't happen, as we earlier matched on Share|Join.")
                }
            }

            print_starting_info(arg_matches, &socket_path, &directory);
            let _daemon = Daemon::new(app_config, &socket_path, &directory, init_doc).await?;
            wait_for_ctrl_c().await;
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
        bail!(
            "No {}/ found in {} (create that directory to Ethersync-enable the project)",
            ETHERSYNC_CONFIG_DIR,
            directory.display()
        );
    }
    Ok(directory)
}

fn print_starting_info(arg_matches: clap::ArgMatches, socket_path: &Path, directory: &Path) {
    if arg_matches.value_source("socket_name").unwrap() == ValueSource::EnvVariable {
        info!(
            "Using socket path {} from environment variable {}.",
            socket_path.display(),
            ETHERSYNC_SOCKET_ENV_VAR
        );
    }

    debug!("Starting Ethersync on {}.", directory.display());
}

async fn wait_for_ctrl_c() {
    match signal::ctrl_c().await {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {err}");
            // still shut down.
        }
    }
}
