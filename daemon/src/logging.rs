use time;

use tracing_subscriber::{fmt, FmtSubscriber};

pub fn initialize() {
    let timer = time::format_description::parse("[hour]:[minute]:[second]")
        .expect("Could not create time format description");
    let time_offset =
        time::UtcOffset::current_local_offset().unwrap_or_else(|_| time::UtcOffset::UTC);
    let timer = fmt::time::OffsetTime::new(time_offset, timer);

    let subscriber = FmtSubscriber::builder()
        // .pretty()
        .with_max_level(tracing::Level::INFO)
        // .with_thread_names(true)
        .with_thread_ids(true)
        .with_timer(timer)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default log subscriber failed");
}
