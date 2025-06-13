// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Data structures and helper methods around influencing the configuration of the application.
use crate::sandbox;
use anyhow::{Context, Result};
use ini::Ini;
use std::path::Path;
use tracing::info;

#[derive(Clone)]
pub struct AppConfig {
    pub peer: Option<String>,
    pub emit_join_code: Option<bool>,
    pub emit_secret_address: Option<bool>,
}

impl AppConfig {
    pub fn from_config_file(config_file: &Path) -> Option<Self> {
        if config_file.exists() {
            let conf = Ini::load_from_file(config_file)
                .expect("Could not access config file, even though it exists");
            let general_section = conf.general_section();
            Some(Self {
                peer: general_section.get("peer").map(|p| p.to_string()),
                emit_join_code: general_section.get("emit_join_code").map(|p| {
                    p.parse()
                        .expect("Failed to parse config parameter `emit_join_code` as bool")
                }),
                emit_secret_address: general_section.get("emit_secret_address").map(|p| {
                    p.parse()
                        .expect("Failed to parse config parameter `emit_secret_address` as bool")
                }),
            })
        } else {
            None
        }
    }

    #[must_use]
    pub const fn is_host(&self) -> bool {
        self.peer.is_none()
    }

    pub fn emit_join_code(&self) -> bool {
        // defaults to true if not configured/provided
        self.emit_join_code.unwrap_or(true)
    }

    pub fn emit_secret_address(&self) -> bool {
        // defaults to false if not configured/provided
        self.emit_secret_address.unwrap_or(false)
    }
}

pub fn store_peer_in_config(directory: &Path, config_file: &Path, peer: &str) -> Result<()> {
    info!("Storing peer's address in .ethersync/config.");

    let content = format!("peer={peer}\n");
    sandbox::write_file(directory, config_file, content.as_bytes())
        .context("Failed to write to config file")
}
