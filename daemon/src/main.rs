use clap::{Parser, Subcommand};
use ethersync::daemon::Daemon;
use ethersync::logging;
use std::io;
use std::path::PathBuf;
use tokio::signal;

mod jsonrpc_forwarder;

const DEFAULT_SOCKET_PATH: &str = "/tmp/ethersync";

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Path to the Unix domain socket to use for communication between daemon and editors.
    #[arg(short, long, global = true, default_value = DEFAULT_SOCKET_PATH)]
    socket_path: Option<PathBuf>,
    /// Enable verbose debug output.
    #[arg(short, long, global = true, action)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch Ethersync's background process that connects with clients and other nodes.
    Daemon {
        // TODO: Move the default port definition to a constant.
        /// Port to listen on as a hosting peer.
        #[arg(short, long, default_value = "4242")]
        port: Option<u16>,
        /// The directory to sync.
        directory: Option<PathBuf>,
        /// IP + port of a peer to connect to. Example: 192.168.1.42:1234
        #[arg(long)]
        peer: Option<String>,
    },
    /// Open a JSON-RPC connection to the Ethersync daemon on stdin/stdout.
    Client,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    let cli = Cli::parse();

    logging::initialize(cli.debug);

    let socket_path = cli.socket_path.unwrap_or(DEFAULT_SOCKET_PATH.into());

    match cli.command {
        Commands::Daemon {
            port,
            directory,
            peer,
        } => {
            // TODO: directory should exist
            let directory = directory
                .unwrap_or(std::env::current_dir().expect("Could not access current directory"));
            Daemon::new(port, peer, &socket_path, &directory);
            match signal::ctrl_c().await {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("Unable to listen for shutdown signal: {err}");
                    // still shut down.
                }
            }
        }
        Commands::Client => {
            jsonrpc_forwarder::connection(&socket_path);
        }
    }
    Ok(())
}
