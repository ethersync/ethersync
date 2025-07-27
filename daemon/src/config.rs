// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Data structures and helper methods around influencing the configuration of the application.
use crate::sandbox;
use crate::wormhole::get_secret_address_from_wormhole;
use anyhow::{bail, Context, Result};
use ini::Ini;
use std::path::Path;
use tracing::info;

pub const DEFAULT_SOCKET_NAME: &str = "socket";
pub const CONFIG_DIR: &str = ".ethersync";
pub const CONFIG_FILE: &str = "config";

const EMIT_JOIN_CODE_DEFAULT: bool = true;
const EMIT_SECRET_ADDRESS_DEFAULT: bool = false;

#[derive(Clone)]
pub enum Peer {
    SecretAddress(String),
    JoinCode(String),
}

#[derive(Clone)]
pub struct AppConfig {
    pub peer: Option<Peer>,
    pub emit_join_code: bool,
    pub emit_secret_address: bool,
}

impl AppConfig {
    pub fn from_config_file(config_file: &Path) -> Option<Self> {
        if config_file.exists() {
            let conf = Ini::load_from_file(config_file)
                .expect("Could not access config file, even though it exists");
            let general_section = conf.general_section();
            Some(Self {
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
            })
        } else {
            None
        }
    }

    /// If we have a join code, try to use that and overwrite the config file.
    /// If we don't have a join code, try to use the configured peer.
    /// Otherwise, fail.
    pub async fn resolve_peer(self, directory: &Path, config_file: &Path) -> Result<Self> {
        let peer = match self.peer {
            Some(Peer::JoinCode(join_code)) => {
                let secret_address = get_secret_address_from_wormhole(&join_code).await.context(
                    "Failed to retreive secret address, was this join code already used?",
                )?;
                info!(
                    "Derived peer from join code. Storing in config (overwriting previous config)."
                );
                store_peer_in_config(directory, config_file, &secret_address)?;
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
        Ok(AppConfig {
            peer: Some(peer),
            emit_join_code: self.emit_join_code,
            emit_secret_address: self.emit_secret_address,
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
    pub fn merge(self, other: Option<Self>) -> Self {
        match other {
            None => self,
            Some(other) => Self {
                peer: self.peer.or(other.peer),
                emit_join_code: self.emit_join_code && other.emit_join_code,
                emit_secret_address: self.emit_secret_address || other.emit_secret_address,
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

pub fn has_git_remote(path: &Path) -> bool {
    if let Ok(repo) = find_git_repo(path) {
        if let Ok(remotes) = repo.remotes() {
            return !remotes.is_empty();
        }
    }
    false
}

pub fn ethersync_directory_should_be_ignored_but_isnt(path: &Path) -> bool {
    if let Ok(repo) = find_git_repo(path) {
        let ethersync_dir = path.join(CONFIG_DIR);
        return !repo
            .is_path_ignored(ethersync_dir)
            .expect("Should have been able to determine ignore state for {path}");
    }
    false
}

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

pub fn add_ethersync_to_global_gitignore() -> Result<()> {
    let home_dir = std::env::home_dir().expect("Could not find home directory");
    let git_config_dir = home_dir.join(".config/git");
    let ignore_file_path = git_config_dir.join("ignore");

    sandbox::create_dir_all(&git_config_dir, &git_config_dir)?;
    let bytes_in =
        sandbox::read_file(&git_config_dir, &ignore_file_path).unwrap_or("".to_string().into());
    let mut content = std::str::from_utf8(&bytes_in)?.to_string();
    if !content.is_empty() && !content.ends_with("\n") {
        content.push('\n');
    }
    content.push_str(".ethersync/\n");
    let bytes_out = content.as_bytes();
    sandbox::write_file(&git_config_dir, &ignore_file_path, bytes_out)?;

    Ok(())
}
