// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use ethersync::{cli::ask, config::{self, AppConfig}, daemon::Daemon, editor, logging, sandbox};
use std::path::{Path, PathBuf};
use tokio::signal;
use tracing::{debug, info, warn};
use crate::jsonrpc_forwarder::JsonRPCForwarder;

mod jsonrpc_forwarder;

// TODO: Define these constants in the ethersync crate, and use them here.
#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// The shared directory. Defaults to current directory.
    #[arg(long, global = true)]
    directory: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Share a directory with a new peer.
    Share {
        /// Re-initialize the history of the shared directory. You will loose previous history.
        #[arg(long)]
        init: bool,
        /// Do not generate a join code. To prevent unintended sharing or simply if you want to
        /// keep Magic Wormhole out of the loop.
        #[arg(long)]
        no_join_code: bool,
        /// Print the secret address. Useful for sharing with multiple people.
        #[arg(long)]
        show_secret_address: bool,
    },
    /// Join a shared directory via a join code, or connect to the most recent one.
    Join {
        /// Specify to connect to a new peer. Otherwise, try to connect to the most recent peer.
        join_code: Option<String>,
    },
    /// Open a JSON-RPC connection to the Ethersync daemon on stdin/stdout. Used by text editor plugins.
    Client,
}

fn has_ethersync_directory(dir: &Path) -> bool {
    let ethersync_dir = dir.join(config::CONFIG_DIR);
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

    logging::initialize().context("Failed to initialize logging")?;

    let directory = get_directory(cli.directory).context("Failed to find .ethersync/ directory")?;

    let config_file = directory.join(config::CONFIG_DIR).join(config::CONFIG_FILE);

    let socket_path = directory
        .join(config::CONFIG_DIR)
        .join(config::DEFAULT_SOCKET_NAME);
    let editor: Box<dyn editor::Editor>;
    #[cfg(windows)]
    {
        editor = Box::new(editor::windows::EditorWindows { pipe_name: socket_path.clone() });
    }
    #[cfg(unix)]
    {
        editor = Box::new(editor::unix::EditorUnix { socket_path: socket_path.clone() });
    }

    match cli.command {
        Commands::Share { .. } | Commands::Join { .. } => {
            let persist = !config::has_git_remote(&directory);
            if !persist {
                info!(
                    "Detected a Git remote: Assuming a pair-programming use-case and starting a new history."
                );
            }

            if config::ethersync_directory_should_be_ignored_but_isnt(&directory) {
                if ask("Ethersync uses the directory .ethersync/ to store sensitive secrets. Add it to your global ~/.config/git/ignore?")? {
                    config::add_ethersync_to_global_gitignore()?;
                    info!("Added! Resuming launch.");
                } else {
                    bail!("Aborting launch. We *really* don't want you to accidentally commit your secrets. Add .ethersync/ to a (global or local) .gitignore and try again.");
                }
            }

            let mut init_doc = false;
            let mut app_config;

            match cli.command {
                Commands::Share {
                    init,
                    no_join_code,
                    show_secret_address,
                    ..
                } => {
                    init_doc = init;
                    let app_config_cli = AppConfig {
                        peer: None,
                        emit_join_code: !no_join_code,
                        emit_secret_address: show_secret_address,
                    };
                    app_config = app_config_cli.merge(AppConfig::from_config_file(&config_file));

                    // Because of the "share" subcommand, explicitly don't connect anywhere.
                    app_config.peer = None;
                }
                Commands::Join { join_code, .. } => {
                    let app_config_cli = AppConfig {
                        peer: join_code.map(config::Peer::JoinCode),
                        emit_join_code: false,
                        emit_secret_address: false,
                    };

                    app_config = app_config_cli.merge(AppConfig::from_config_file(&config_file));

                    app_config = app_config
                        .resolve_peer(&directory, &config_file)
                        .await
                        .context("Failed to resolve peer")?;
                }
                Commands::Client => {
                    panic!("This can't happen, as we earlier matched on Share|Join.")
                }
            }

            debug!("Starting Ethersync on {}.", directory.display());
            let _daemon = Daemon::new(app_config, editor, &directory, init_doc, persist)
                .await
                .context("Failed to launch the daemon")?;
            wait_for_shutdown().await;
        }
        Commands::Client => {
            let forwarder;
            #[cfg(windows)]
            {
                forwarder = jsonrpc_forwarder::windows::WindowsJsonRPCForwarder { };
            }
            #[cfg(unix)]
            {
                forwarder = jsonrpc_forwarder::unix::UnixJsonRPCForwarder { };
            }

            forwarder.connection(&socket_path).await.expect("JSON-RPC forwarder failed");
        }
    }
    Ok(())
}

fn get_directory(directory: Option<PathBuf>) -> Result<PathBuf> {
    let directory = normalize_directory(directory
        .unwrap_or_else(|| std::env::current_dir().expect("Could not access current directory")));
    if !has_ethersync_directory(&directory) {
        let ethersync_dir = directory.join(config::CONFIG_DIR);

        warn!(
            "{:?} hasn't been used as an Ethersync directory before.",
            &directory
        );

        if ask(&format!(
            "Create an {}/ directory to allow live collaboration?",
            config::CONFIG_DIR
        ))? {
            sandbox::create_dir(&directory, &ethersync_dir)?;
            info!("Created! Resuming launch.");
        } else {
            bail!("Aborting launch. Ethersync needs an .ethersync/ directory to function");
        }
    }
    Ok(directory)
}

#[cfg(unix)]
pub async fn wait_for_shutdown() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm =
        signal(SignalKind::terminate()).expect("failed to create SIGTERM stream");

    tokio::select! {
        _ = signal::ctrl_c() => {
            debug!("Got SIGINT (Ctrl+C), shutting down");
        }
        _ = sigterm.recv() => {
            debug!("Got SIGTERM, shutting down");
        }
    }
}

#[cfg(windows)]
pub async fn wait_for_shutdown() {
    use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_logoff, ctrl_shutdown};

    let mut brk  = ctrl_break().expect("failed to create CTRL_BREAK handler");
    let mut clos = ctrl_close().expect("failed to create CTRL_CLOSE handler");
    let mut logf = ctrl_logoff().expect("failed to create CTRL_LOGOFF handler");
    let mut shdn = ctrl_shutdown().expect("failed to create CTRL_SHUTDOWN handler");

    tokio::select! {
        _ = signal::ctrl_c() => {
            debug!("Got CTRL+C, shutting down");
        }
        _ = brk.recv() => {
            debug!("Got CTRL_BREAK, shutting down");
        }
        _ = clos.recv() => {
            debug!("Got CTRL_CLOSE, shutting down");
        }
        _ = logf.recv() => {
            debug!("Got CTRL_LOGOFF, shutting down");
        }
        _ = shdn.recv() => {
            debug!("Got CTRL_SHUTDOWN, shutting down");
        }
    }
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
