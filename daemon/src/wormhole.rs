// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::Result;
use magic_wormhole::{transfer, AppID, Code, MailboxConnection, Wormhole};
use std::str::FromStr;
use tracing::info;

pub async fn put_secret_address_into_wormhole(address: &str) {
    let config = transfer::APP_CONFIG.id(AppID::new("ethersync"));

    let mailbox_connection = MailboxConnection::create(config.clone(), 2).await.unwrap();
    let code = mailbox_connection.code().clone();

    info!(
        "\n\tTo connect to you, another person can run:\n\n\tethersync join {}\n",
        &code
    );

    let payload = address.into();
    tokio::spawn(async move {
        let mut wormhole = Wormhole::connect(mailbox_connection)
            .await
            .expect("Failed to initiate wormhole connection");
        let _ = wormhole.send(payload).await;
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
