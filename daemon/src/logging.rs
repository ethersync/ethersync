// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::Result;
use time::macros::format_description;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt::time::UtcTime, EnvFilter, FmtSubscriber};

pub fn initialize() -> Result<()> {
    let simplified_logging = std::env::var("RUST_LOG").is_err();

    if simplified_logging {
        let subscriber = FmtSubscriber::builder()
            .with_env_filter(EnvFilter::new("ethersync=info"))
            .without_time()
            .with_level(false)
            .with_target(false)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("Setting default log subscriber failed");
    } else {
        let timer = UtcTime::new(format_description!("[hour]:[minute]:[second]Z"));
        let filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env()?;
        let subscriber = FmtSubscriber::builder()
            .with_env_filter(filter)
            .with_thread_ids(true)
            .with_timer(timer)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("Setting default log subscriber failed");
    }

    Ok(())
}
