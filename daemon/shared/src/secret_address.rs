// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::{Display, Formatter};
use std::str::FromStr;
use anyhow::{bail,Result};
use iroh::{NodeAddr, SecretKey};

const ADDRESS_DELIMITER: char = '#';

pub struct SecretAddress {
    pub node_addr: NodeAddr,
    pub passphrase: SecretKey,
}

impl Clone for SecretAddress {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id,
            passphrase: self.passphrase.clone(),
        }
    }
}

impl Display for SecretAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}{}", self.node_addr.node_id, ADDRESS_DELIMITER, self.passphrase)
    }
}

impl FromStr for SecretAddress {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(ADDRESS_DELIMITER).collect();
        if parts.len() != 2 {
            bail!("Peer string must have format <node_id>#<passphrase>");
        }

        let node_addr = iroh::PublicKey::from_str(parts[0])?.into();
        let passphrase = SecretKey::from_str(parts[1])?;

        Ok(Self {
            node_addr,
            passphrase,
        })
    }
}