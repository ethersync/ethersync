// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::{Display, Formatter};
use std::str::FromStr;
use anyhow::{bail,Result};
use iroh::{NodeId, SecretKey};

const ADDRESS_DELIMITER: char = '#';

pub struct SecretAddress {
    pub node_id: NodeId,
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
        write!(f, "{}{}{}", self.node_id, ADDRESS_DELIMITER, self.passphrase)
    }
}

impl SecretAddress {
    pub fn from_str_parts(node_id_part: &str, passphrase_part: &str) -> Result<Self> {
        let node_id = NodeId::from_str(node_id_part)?.into();
        let passphrase = SecretKey::from_str(passphrase_part)?;

        Ok(Self {
            node_id,
            passphrase,
        })
    }
}

impl FromStr for SecretAddress {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(ADDRESS_DELIMITER).collect();
        if parts.len() != 2 {
            bail!("Peer string must have format <node_id>{ADDRESS_DELIMITER}<passphrase>");
        }

        Self::from_str_parts(parts[0], parts[1])
    }
}