// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use ethersync::{
    config::{self, AppConfig},
    daemon::Daemon,
    logging, sandbox,
};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tokio::signal;
use tracing::{debug, info, warn};

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
        /// Re-initialize the history of the shared project. You will loose previous history.
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
    /// Join a shared project via join code.
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
            let _daemon = Daemon::new(app_config, &socket_path, &directory, init_doc, persist)
                .await
                .context("Failed to launch the daemon")?;
            wait_for_ctrl_c().await;
        }
        Commands::Client => {
            jsonrpc_forwarder::connection(&socket_path)
                .await
                .context("JSON-RPC forwarder failed")?;
        }
    }
    Ok(())
}

fn get_directory(directory: Option<PathBuf>) -> Result<PathBuf> {
    let directory = directory
        .unwrap_or_else(|| std::env::current_dir().expect("Could not access current directory"))
        .canonicalize()
        .expect("Could not access given directory");
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

async fn wait_for_ctrl_c() {
    match signal::ctrl_c().await {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {err}");
            // still shut down.
        }
    }
}

fn ask(question: &str) -> Result<bool> {
    print!("{question} (y/N): ");
    std::io::stdout().flush()?;
    let mut lines = std::io::stdin().lines();
    if let Some(Ok(line)) = lines.next() {
        match line.to_lowercase().as_str() {
            "y" | "yes" => Ok(true),
            _ => Ok(false),
        }
    } else {
        bail!("Failed to read answer");
    }
}
