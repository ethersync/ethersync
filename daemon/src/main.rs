use anyhow::{bail, Context, Result};
use clap::{parser::ValueSource, CommandFactory, FromArgMatches, Parser, Subcommand};
use ethersync::peer::PeerConnectionInfo;
use ethersync::{daemon::Daemon, logging, sandbox};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use tokio::signal;
use tracing::{error, info};

mod jsonrpc_forwarder;

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

fn is_valid_socket_name(socket_name: &Path) -> Result<()> {
    if socket_name.components().count() != 1 {
        bail!("The socket name must be a single path component");
    }
    if let std::path::Component::Normal(_) = socket_name
        .components()
        .next()
        .expect("The component count of socket_name was previously checked to be non-empty")
    {
        // All good :)
    } else {
        bail!("The socket name must be a plain filename");
    }
    Ok(())
}

fn get_fallback_socket_dir() -> String {
    let socket_dir = format!(
        "/tmp/ethersync-{}",
        std::env::var("USER").expect("$USER should be set")
    );
    if !fs::exists(&socket_dir).expect("Should be able to test for existence of directory in /tmp")
    {
        fs::create_dir(&socket_dir).expect("Should be able to create a directory in /tmp");
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&socket_dir, permissions)
            .expect("Should be able to set permissions for a directory we just created");
    }
    socket_dir
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

    let socket_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| get_fallback_socket_dir());
    let socket_dir = Path::new(&socket_dir);
    if let Err(description) = is_valid_socket_name(&cli.socket_name) {
        panic!("{}", description);
    }
    let socket_path = socket_dir.join(cli.socket_name);

    match cli.command {
        Commands::Daemon {
            directory,
            peer,
            port,
            secret,
            init,
        } => {
            if matches.value_source("socket_name").unwrap() == ValueSource::EnvVariable {
                info!(
                    "Using socket path {} from env var {}",
                    socket_path.display(),
                    ETHERSYNC_SOCKET_ENV_VAR
                );
            }

            let directory = directory
                .unwrap_or_else(|| {
                    std::env::current_dir().expect("Could not access current directory")
                })
                .canonicalize()
                .expect("Could not access given directory");
            if !has_ethersync_directory(&directory) {
                error!(
                    "No {}/ found in {} (create that directory to Ethersync-enable the project)",
                    ETHERSYNC_CONFIG_DIR,
                    directory.display()
                );
                return Ok(());
            }

            let mut peer_connection_info = PeerConnectionInfo {
                peer,
                port,
                passphrase: secret,
            };

            let config_file = directory
                .join(ETHERSYNC_CONFIG_DIR)
                .join(ETHERSYNC_CONFIG_FILE);

            if let Some(config_from_file) = PeerConnectionInfo::from_config_file(&config_file) {
                peer_connection_info = peer_connection_info.takes_precedence_over(config_from_file);
            }

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
