use anyhow::{Context, Result};
use clap::{parser::ValueSource, CommandFactory, FromArgMatches, Parser, Subcommand};
use ethersync::peer::PeerConnectionInfo;
use ethersync::{daemon::Daemon, editor, logging, sandbox};
use std::path::{Path, PathBuf};
use tokio::signal;
use tracing::{error, info};

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

    let matches = Cli::command().get_matches();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(e) => e.exit(),
    };

    logging::initialize(cli.debug);

    let editor: Box<dyn editor::Editor>;
    #[cfg(windows)]
    {
        editor = Box::new(editor::windows::EditorWindows {});
    }
    #[cfg(unix)]
    {
        editor = Box::new(editor::unix::EditorUnix { socket_name: cli.socket_name });
    }

    let socket_path = editor.get_socket_path();

    match cli.command {
        Commands::Daemon {
            directory,
            peer,
            port,
            init,
        } => {
            if matches.value_source("socket_name").unwrap() == ValueSource::EnvVariable {
                info!(
                    "Using socket path {} from env var {}",
                    socket_path.display(),
                    ETHERSYNC_SOCKET_ENV_VAR
                );
            }

            let directory = normalize_directory(directory.unwrap_or_else(|| {
                std::env::current_dir().expect("Could not access current directory")
            }));

            if !has_ethersync_directory(&directory) {
                error!(
                    "No {}/ found in {} (create that directory to Ethersync-enable the project)",
                    ETHERSYNC_CONFIG_DIR,
                    directory.display()
                );
                return Ok(());
            }

            // There's the option to put a user provided passphrase here, which is disabled for
            // now until we code a more secure way for user to provide it.
            let mut peer_connection_info = PeerConnectionInfo {
                peer,
                port,
                passphrase: None,
            };

            let config_file = directory
                .join(ETHERSYNC_CONFIG_DIR)
                .join(ETHERSYNC_CONFIG_FILE);

            if let Some(config_from_file) = PeerConnectionInfo::from_config_file(&config_file) {
                peer_connection_info = peer_connection_info.takes_precedence_over(config_from_file);
            }

            info!("Starting Ethersync on {}", directory.display());
            Daemon::new(peer_connection_info, editor, &directory, init);
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

fn normalize_directory(directory: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let directory_str = directory.to_string_lossy();
        if directory_str.len() > 2 && directory_str.chars().nth(1) == Some(':') {
            let (drive_letter, rest) = directory_str.split_at(1);
            let lower_drive = drive_letter.to_lowercase();
            let path_with_forward_slashes = rest.replace('\\', "/");
            return PathBuf::from(format!("{}{}", lower_drive, path_with_forward_slashes));
        }
        directory
    }
    #[cfg(unix)]
    {
        return directory
            .canonicalize()
            .expect("Could not access given directory");
    }
}
