use clap::{Parser, Subcommand};
use ethersync::daemon::Daemon;
use ethersync::logging;
use std::io;
use std::path::PathBuf;
use tokio::signal;

mod client;

const DEFAULT_SOCKET_PATH: &str = "/tmp/ethersync";

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, global = true)]
    socket_path: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch Ethersync's background process that connects with clients and other nodes.
    Daemon {
        /// IP + port of a peer to connect to. Example: 192.168.1.42:1234
        peer: Option<String>,
        #[arg(short, long)]
        file: PathBuf,
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

    logging::initialize();

    let cli = Cli::parse();

    let socket_path = cli.socket_path.unwrap_or(DEFAULT_SOCKET_PATH.into());

    match cli.command {
        Commands::Daemon { peer, file } => {
            Daemon::new(peer, &socket_path, &file);
            match signal::ctrl_c().await {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("Unable to listen for shutdown signal: {err}");
                    // still shut down.
                }
            }
        }
        Commands::Client => {
            client::connection(&socket_path);
        }
    }
    Ok(())
}
