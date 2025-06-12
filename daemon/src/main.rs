// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Context, Result};
use clap::{parser::ValueSource, CommandFactory, FromArgMatches, Parser, Subcommand};
use ethersync::peer::PeerConnectionInfo;
use ethersync::wormhole::get_ticket_from_wormhole;
use ethersync::{daemon::Daemon, editor, logging, sandbox};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::signal;
use tracing::info;

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
    /// Enable verbose debug output.
    #[arg(short, long, global = true)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Share the a directory with a peer.
    Share {
        /// The directory to sync. Defaults to current directory.
        directory: Option<PathBuf>,
        /// Initialize the current contents of the directory as a new Ethersync directory.
        #[arg(long)]
        init: bool,
    },
    /// Join a shared project.
    Join {
        /// The directory to sync. Defaults to current directory.
        directory: Option<PathBuf>,
    },
    /// Open a JSON-RPC connection to the Ethersync daemon on stdin/stdout.
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

    logging::initialize(cli.debug);

    let socket_path = editor::get_socket_path(&cli.socket_name);

    match cli.command {
        Commands::Share { directory, init } => {
            let directory = get_directory(directory)?;
            print_starting_info(arg_matches, &socket_path, &directory);
            let _daemon = Daemon::new(
                PeerConnectionInfo { peer: None },
                &socket_path,
                &directory,
                init,
            )
            .await?;
            wait_for_ctrl_c().await;
        }
        Commands::Join { directory } => {
            let directory = get_directory(directory)?;
            let config_file = directory
                .join(ETHERSYNC_CONFIG_DIR)
                .join(ETHERSYNC_CONFIG_FILE);

            let peer_connection_info = match PeerConnectionInfo::from_config_file(&config_file) {
                None | Some(PeerConnectionInfo { peer: None }) => {
                    // If no peer is configured, or no config exists, ask for it.
                    let peer = read_ticket().await?;
                    PeerConnectionInfo { peer: Some(peer) }
                }
                Some(peer_connection_info) => {
                    info!("Using peer from config file");
                    peer_connection_info
                }
            };

            print_starting_info(arg_matches, &socket_path, &directory);
            let _daemon =
                Daemon::new(peer_connection_info, &socket_path, &directory, false).await?;
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

async fn read_ticket() -> Result<String> {
    let mut line = String::new();
    print!("Enter peer's magic connection code: ");
    std::io::stdout().flush()?;
    std::io::stdin().read_line(&mut line)?;
    let code = line.trim();
    let ticket = get_ticket_from_wormhole(&code).await?;
    Ok(ticket)
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
            "Using socket path {} from env var {}",
            socket_path.display(),
            ETHERSYNC_SOCKET_ENV_VAR
        );
    }

    info!("Starting Ethersync on {}", directory.display());
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
