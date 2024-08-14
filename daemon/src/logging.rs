// SPDX-FileCopyrightText: 2024 blinry
// SPDX-FileCopyrightText: 2024 zormit
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use time;

use tracing_subscriber::{fmt, FmtSubscriber};

pub fn initialize(debug: bool) {
    let timer = time::format_description::parse("[hour]:[minute]:[second]")
        .expect("Could not create time format description");
    let time_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let timer = fmt::time::OffsetTime::new(time_offset, timer);

    let level = if debug {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    let subscriber = FmtSubscriber::builder()
        // .pretty()
        .with_max_level(level)
        // .with_thread_names(true)
        .with_thread_ids(true)
        .with_timer(timer)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default log subscriber failed");
}
