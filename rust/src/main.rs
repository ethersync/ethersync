use clap::{Parser, Subcommand};
use ethersync::daemon::Daemon;
use std::io;
use std::path::PathBuf;
use time;
use tokio::signal;
use tracing_subscriber::{fmt, FmtSubscriber};

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
    let timer = time::format_description::parse("[hour]:[minute]:[second]")
        .expect("Could not create time format description");
    let time_offset =
        time::UtcOffset::current_local_offset().unwrap_or_else(|_| time::UtcOffset::UTC);
    let timer = fmt::time::OffsetTime::new(time_offset, timer);

    let subscriber = FmtSubscriber::builder()
        // .pretty()
        .with_max_level(tracing::Level::DEBUG)
        // .with_thread_names(true)
        .with_thread_ids(true)
        .with_timer(timer)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default log subscriber failed");

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
