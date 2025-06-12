// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Context, Result};
use clap::{parser::ValueSource, CommandFactory, FromArgMatches, Parser, Subcommand};
use ethersync::config::{store_peer_in_config, AppConfig};
use ethersync::wormhole::get_ticket_from_wormhole;
use ethersync::{daemon::Daemon, editor, logging, sandbox};
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
        /// Re-initialize the history of the shared project.
        #[arg(long)]
        init: bool,
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
        Commands::Share { directory, init } => {
            let directory = get_directory(directory)?;
            print_starting_info(arg_matches, &socket_path, &directory);
            let _daemon =
                Daemon::new(AppConfig { peer: None }, &socket_path, &directory, init).await?;
            wait_for_ctrl_c().await;
        }
        Commands::Join {
            join_code,
            directory,
        } => {
            let directory = get_directory(directory)?;
            let config_file = directory
                .join(ETHERSYNC_CONFIG_DIR)
                .join(ETHERSYNC_CONFIG_FILE);

            let app_config = match join_code {
                Some(join_code) => {
                    let peer = get_ticket_from_wormhole(&join_code).await?;
                    store_peer_in_config(&directory, &config_file, &peer)?;
                    AppConfig { peer: Some(peer) }
                }
                None => match AppConfig::from_config_file(&config_file) {
                    None | Some(AppConfig { peer: None }) => {
                        bail!("Missing join code, and no peer=<node ticket> in .ethersync/config");
                    }
                    Some(app_config) => {
                        info!("Using peer from config file.");
                        app_config
                    }
                },
            };

            print_starting_info(arg_matches, &socket_path, &directory);
            let _daemon = Daemon::new(app_config, &socket_path, &directory, false).await?;
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
