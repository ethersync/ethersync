// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::Result;
use magic_wormhole::{transfer, AppID, Code, MailboxConnection, Wormhole};
use std::{str::FromStr, time::Duration};
use tokio::time::sleep;
use tracing::{info, warn};

pub async fn put_secret_address_into_wormhole(address: &str) {
    let config = transfer::APP_CONFIG.id(AppID::new("ethersync"));
    let payload: Vec<u8> = address.into();

    tokio::spawn(async move {
        loop {
            let mailbox_connection = MailboxConnection::create(config.clone(), 2).await.unwrap();
            let code = mailbox_connection.code().clone();

            info!(
                "\n\tOne other person can use this to connect to you:\n\n\tethersync join {}\n",
                &code
            );

            if let Ok(mut wormhole) = Wormhole::connect(mailbox_connection).await {
                let _ = wormhole.send(payload.clone()).await;
            } else {
                warn!("Failed to share secret address. Did your peer mistype the join code?");
            }

            // Print a new join code in the next iteration of the foor loop, to allow more people
            // to join.
            sleep(Duration::from_millis(500)).await;
        }
    });
}

pub async fn get_secret_address_from_wormhole(code: &str) -> Result<String> {
    let config = transfer::APP_CONFIG.id(AppID::new("ethersync"));

    let mut wormhole =
        Wormhole::connect(MailboxConnection::connect(config, Code::from_str(code)?, false).await?)
            .await?;
    let bytes = wormhole.receive().await?;
    Ok(String::from_utf8(bytes)?)
}
