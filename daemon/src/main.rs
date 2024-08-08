use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ethersync::peer::PeerConnectionInfo;
use ethersync::{daemon::Daemon, logging, sandbox};
use std::path::{Path, PathBuf};
use tokio::signal;
use tracing::{error, info};

mod jsonrpc_forwarder;

const DEFAULT_SOCKET_PATH: &str = "/tmp/ethersync";
const ETHERSYNC_CONFIG_DIR: &str = ".ethersync";
const ETHERSYNC_CONFIG_FILE: &str = "config";

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Path to the Unix domain socket to use for communication between daemon and editors.
    #[arg(short, long, global = true, default_value = DEFAULT_SOCKET_PATH)]
    socket_path: PathBuf,
    /// Enable verbose debug output.
    #[arg(short, long, global = true, action)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch Ethersync's background process that connects with clients and other nodes.
    Daemon {
        /// The directory to sync. Defaults to current directory.
        directory: Option<PathBuf>,
        /// Multiaddr of a peer to connect to.
        #[arg(long)]
        peer: Option<String>,
        /// Port to listen on as a hosting peer [default: assigned by OS].
        #[arg(long)]
        port: Option<u16>,
        /// Shared secret passphrase to use for mutual authorization.
        #[arg(long)]
        secret: Option<String>,
        /// Initialize the current contents of the directory as a new Ethersync directory.
        #[arg(long)]
        init: bool,
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

    let cli = Cli::parse();

    logging::initialize(cli.debug);

    let socket_path = cli.socket_path;

    match cli.command {
        Commands::Daemon {
            directory,
            mut peer,
            mut port,
            mut secret,
            init,
        } => {
            let directory = directory
                .unwrap_or_else(|| {
                    std::env::current_dir().expect("Could not access current directory")
                })
                .canonicalize()
                .expect("Could not access given directory");
            if !has_ethersync_directory(&directory) {
                error!(
                    "No {} found in {} (create it to Ethersync-enable the directory)",
                    ETHERSYNC_CONFIG_DIR,
                    directory.display()
                );
                return Ok(());
            }

            let config_from_options = PeerConnectionInfo {
                peer: peer.take(),
                port: port.take(),
                passphrase: secret.take(),
            };

            let config_file = directory
                .join(ETHERSYNC_CONFIG_DIR)
                .join(ETHERSYNC_CONFIG_FILE);
            let config_from_file = PeerConnectionInfo::from_config_file(&config_file)?;

            let peer_connection_info = config_from_options.takes_precedence_over(config_from_file);

            info!("Starting Ethersync on {}", directory.display());
            Daemon::new(peer_connection_info, &socket_path, &directory, init);
            match signal::ctrl_c().await {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("Unable to listen for shutdown signal: {err}");
                    // still shut down.
                }
            }
        }
        Commands::Client => {
            jsonrpc_forwarder::connection(&socket_path)
                .await
                .context("JSON-RPC forwarder failed")?;
        }
    }
    Ok(())
}
