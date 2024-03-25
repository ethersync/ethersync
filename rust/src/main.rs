use clap::{Parser, Subcommand};
use std::io;
use std::thread;

mod client;
mod daemon;

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

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Daemon { peer } => {
            let mut daemon = daemon::Daemon::new();

            // TODO: How can we listen on socket & port at the same time?
            //thread::spawn(move || {
            //    daemon.listen_socket().unwrap();
            //});

            if let Some(peer) = peer {
                daemon.dial_tcp(peer)?;
            } else {
                daemon.listen_tcp()?;
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
