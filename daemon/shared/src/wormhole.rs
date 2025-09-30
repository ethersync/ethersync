// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::Result;
use magic_wormhole::{transfer, AppID, Code, MailboxConnection, Wormhole};
use std::{str::FromStr};
use tracing::{error, warn};
use crate::secret_address::SecretAddress;
use crate::platform::{spawn, Sender};

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
use futures_util::sink::SinkExt;

pub fn put_secret_address_into_wormhole(address: &SecretAddress, join_code_tx: Sender<Code>) {
    let config = transfer::APP_CONFIG.id(AppID::new("ethersync"));
    let payload: Vec<u8> = address.to_string().into();

    spawn(async move {
        let Ok(mailbox_connection) = MailboxConnection::create(config.clone(), 2).await else {
            error!(
            "Failed to share join code via magic wormhole. Restart Ethersync to try again."
        );
            return;
        };

        let code = mailbox_connection.code().clone();
        if join_code_tx.to_owned().send(code).await.is_err() {
            return;
        }

        if let Ok(mut wormhole) = Wormhole::connect(mailbox_connection).await {
            let _ = wormhole.send(payload.clone()).await;
        } else {
            warn!("Failed to share secret address. Did your peer mistype the join code?");
        }
    });
}

pub async fn get_secret_address_from_wormhole(code: &str) -> Result<SecretAddress> {
    let config = transfer::APP_CONFIG.id(AppID::new("ethersync"));

    let mut wormhole =
        Wormhole::connect(MailboxConnection::connect(config, Code::from_str(code)?, false).await?)
            .await?;
    let bytes = wormhole.receive().await?;
    SecretAddress::from_str(&String::from_utf8(bytes)?)
}
