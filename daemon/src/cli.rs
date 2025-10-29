// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

// TODO: Define these constants in the teamtype crate, and use them here.
#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    /// The shared directory. Defaults to current directory.
    #[arg(long, global = true)]
    pub directory: Option<PathBuf>,
}

#[derive(Args)]
pub struct SyncVcsFlag {
    /// EXPERIMENTAL: Also synchronize version-control directories like .git/ or .jj/, which are normally
    /// ignored. For Git, this will synchronize all branches, commits, etc. as well as your .git/config.
    /// This means that new commits will immediately appear at all peers, you can change branches together, etc.
    #[arg(long)]
    pub sync_vcs: bool,
}

#[derive(Subcommand)]
pub enum Commands {
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
        #[command(flatten)]
        sync_vcs: SyncVcsFlag,
    },
    /// Join a shared directory via a join code, or connect to the most recent one.
    Join {
        /// Specify to connect to a new peer. Otherwise, try to connect to the most recent peer.
        join_code: Option<String>,
        #[command(flatten)]
        sync_vcs: SyncVcsFlag,
    },
    /// Open a JSON-RPC connection to the Teamtype daemon on stdin/stdout. Used by text editor plugins.
    Client,
}

#[test]
fn verify() {
    use clap::CommandFactory as _;
    Cli::command().debug_assert();
}
