// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Data structures and helper methods around influencing the configuration of the application.
use crate::sandbox;
use crate::wormhole::get_secret_address_from_wormhole;
use anyhow::{bail, Context, Result};
use ini::Ini;
use std::path::{Path, PathBuf};
use tracing::info;

pub const DOC_FILE: &str = "doc";
pub const DEFAULT_SOCKET_NAME: &str = "socket";
pub const CONFIG_DIR: &str = ".ethersync";
pub const CONFIG_FILE: &str = "config";
pub const BOOKMARK_FILE: &str = "bookmark";

const EMIT_JOIN_CODE_DEFAULT: bool = true;
const EMIT_SECRET_ADDRESS_DEFAULT: bool = false;

#[derive(Clone)]
pub enum Peer {
    SecretAddress(String),
    JoinCode(String),
}

#[derive(Clone)]
#[must_use]
pub struct AppConfig {
    pub base_dir: PathBuf,
    pub peer: Option<Peer>,
    pub emit_join_code: bool,
    pub emit_secret_address: bool,
    // Whether to sync version control directories like .git, .jj, ...
    pub sync_vcs: bool,
}

impl AppConfig {
    #[must_use]
    pub fn from_config_file(config_file: &Path) -> Option<Self> {
        if config_file.exists() {
            let conf = Ini::load_from_file(config_file)
                .expect("Could not access config file, even though it exists");
            let general_section = conf.general_section();
            Some(Self {
                // TODO: extract all the other fields to its own struct, s.t. we don't have to work
                // around the fact that base_dir won't ever be in the config file.
                base_dir: Path::new("/does-not-exist").to_path_buf(),
                peer: general_section
                    .get("peer")
                    .map(|p| Peer::SecretAddress(p.to_string())),
                emit_join_code: general_section.get("emit_join_code").map_or(
                    EMIT_JOIN_CODE_DEFAULT,
                    |p| {
                        p.parse()
                            .expect("Failed to parse config parameter `emit_join_code` as bool")
                    },
                ),
                emit_secret_address: general_section.get("emit_secret_address").map_or(
                    EMIT_SECRET_ADDRESS_DEFAULT,
                    |p| {
                        p.parse().expect(
                            "Failed to parse config parameter `emit_secret_address` as bool",
                        )
                    },
                ),
                sync_vcs: false,
            })
        } else {
            None
        }
    }

    fn config_file(&self) -> PathBuf {
        self.base_dir.join(CONFIG_DIR).join(CONFIG_FILE)
    }

    /// If we have a join code, try to use that and overwrite the config file.
    /// If we don't have a join code, try to use the configured peer.
    /// Otherwise, fail.
    pub async fn resolve_peer(self) -> Result<Self> {
        let peer = match self.peer {
            Some(Peer::JoinCode(ref join_code)) => {
                let secret_address = get_secret_address_from_wormhole(&join_code).await.context(
                    "Failed to retreive secret address, was this join code already used?",
                )?;
                info!(
                    "Derived peer from join code. Storing in config (overwriting previous config)."
                );
                store_peer_in_config(&self.base_dir, &self.config_file(), &secret_address)?;
                Peer::SecretAddress(secret_address)
            }
            Some(Peer::SecretAddress(secret_address)) => {
                info!("Using peer from config file.");
                Peer::SecretAddress(secret_address)
            }
            None => {
                bail!("Missing join code, and no peer=<secret address> in .ethersync/config");
            }
        };
        Ok(Self {
            base_dir: self.base_dir,
            peer: Some(peer),
            emit_join_code: self.emit_join_code,
            emit_secret_address: self.emit_secret_address,
            sync_vcs: self.sync_vcs,
        })
    }

    #[must_use]
    pub const fn is_host(&self) -> bool {
        self.peer.is_none()
    }

    /// Merges two configurations by taking the "superset" of them.
    ///
    /// It depends on the attribute how we're merging it:
    /// - For strings, the existing (calling) attribute has precedence.
    /// - For booleans, if a value deviates from the default, it "wins".
    /// - The base_dir will be taken from the caller.
    pub fn merge(self, other: Option<Self>) -> Self {
        match other {
            None => self,
            Some(other) => Self {
                base_dir: self.base_dir,
                peer: self.peer.or(other.peer),
                emit_join_code: self.emit_join_code && other.emit_join_code,
                emit_secret_address: self.emit_secret_address || other.emit_secret_address,
                sync_vcs: self.sync_vcs || other.sync_vcs,
            },
        }
    }
}

pub fn store_peer_in_config(directory: &Path, config_file: &Path, peer: &str) -> Result<()> {
    info!("Storing peer's address in .ethersync/config.");

    let content = format!("peer={peer}\n");
    sandbox::write_file(directory, config_file, content.as_bytes())
        .context("Failed to write to config file")
}

#[must_use]
pub fn has_git_remote(path: &Path) -> bool {
    if let Ok(repo) = find_git_repo(path) {
        if let Ok(remotes) = repo.remotes() {
            return !remotes.is_empty();
        }
    }
    false
}

#[must_use]
fn ethersync_directory_should_be_ignored_but_isnt(path: &Path) -> bool {
    if let Ok(repo) = find_git_repo(path) {
        let ethersync_dir = path.join(CONFIG_DIR);
        return !repo
            .is_path_ignored(ethersync_dir)
            .expect("Should have been able to determine ignore state of path");
    }
    false
}

#[must_use]
pub fn get_username(base_dir: &Path) -> Option<String> {
    local_git_username(base_dir)
        .or_else(|_| global_git_username())
        .ok()
}

fn local_git_username(base_dir: &Path) -> Result<String> {
    Ok(find_git_repo(base_dir)?
        .config()?
        .snapshot()?
        .get_str("user.name")?
        .to_string())
}

fn global_git_username() -> Result<String> {
    Ok(git2::Config::open_default()?
        .snapshot()?
        .get_str("user.name")?
        .to_string())
}

fn find_git_repo(path: &Path) -> Result<git2::Repository, git2::Error> {
    git2::Repository::discover(path)
}

fn add_ethersync_to_local_gitignore(directory: &Path) -> Result<()> {
    let mut ignore_file_path = directory.join(CONFIG_DIR);
    ignore_file_path.push(".gitignore");

    // It's very unlikely that .ethersync/.gitignore will already contain something, but let's
    // still append.
    let bytes_in = sandbox::read_file(directory, &ignore_file_path).unwrap_or_default();
    // TODO: use String::from_utf8
    let mut content = std::str::from_utf8(&bytes_in)?.to_string();

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str("/*\n");
    let bytes_out = content.as_bytes();
    sandbox::write_file(directory, &ignore_file_path, bytes_out)?;

    Ok(())
}

pub fn ensure_ethersync_is_ignored(directory: &Path) -> Result<()> {
    if ethersync_directory_should_be_ignored_but_isnt(directory) {
        add_ethersync_to_local_gitignore(directory)?;
    }
    Ok(())
}
