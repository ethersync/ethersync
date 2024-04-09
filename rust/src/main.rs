use clap::{Parser, Subcommand};
use std::io;
use tracing_subscriber::FmtSubscriber;

mod client;
mod daemon;
mod ot;
mod types;

const DEFAULT_SOCKET_PATH: &str = "/tmp/ethersync";

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, global = true)]
    socket_path: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch Ethersync's background process that connects with clients and other nodes.
    Daemon {
        /// IP + port of a peer to connect to. Example: 192.168.1.42:1234
        peer: Option<String>,
        #[arg(short, long)]
        file: String,
    },
    /// Open a JSON-RPC connection to the Ethersync daemon on stdin/stdout.
    Client,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default log subscriber failed");

    let cli = Cli::parse();

    let socket_path = cli.socket_path.unwrap_or(DEFAULT_SOCKET_PATH.to_string());

    match cli.command {
        Commands::Daemon { peer, file } => {
            daemon::launch(peer, socket_path, file).await;
        }
        Commands::Client => {
            client::connection(&socket_path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
