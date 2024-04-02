use clap::{Parser, Subcommand};
use std::io;

mod client;
mod daemon;
mod ot;
mod types;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch Ethersync's background process that connects with clients and other nodes.
    Daemon {
        /// IP + port of a peer to connect to. Example: 192.168.1.42:1234
        peer: Option<String>,
    },
    /// Open a JSON-RPC connection to the Ethersync daemon on stdin/stdout.
    Client,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Daemon { peer } => {
            if let Some(peer) = peer {
                daemon::launch(Some(peer.to_string())).await;
            } else {
                daemon::launch(None).await;
            }
        }
        Commands::Client => {
            client::connection()?;
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
