#![allow(dead_code)]
use ethersync::actors::{Actor, Neovim};
use ethersync::daemon::Daemon;
use futures::future::join_all;
use pretty_assertions::assert_eq;
use rand::Rng;
use std::collections::HashMap;
use std::path::Path;
use tokio::time::{sleep, timeout};
use tracing::info;
use tracing_subscriber::FmtSubscriber;

async fn perform_random_edits(actor: &mut (impl Actor + ?Sized)) {
    loop {
        actor.apply_random_delta().await;
        let random_millis = rand::thread_rng().gen_range(0..100);
        sleep(std::time::Duration::from_millis(random_millis)).await;
    }
}

fn create_ethersync_dir(dir: &Path) {
    let mut ethersync_dir = dir.to_path_buf();
    ethersync_dir.push(".ethersync");
    std::fs::create_dir(ethersync_dir).expect("Failed to create .ethersync directory");
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default log subscriber failed");

    // Set up the project directory.
    let dir = temp_dir::TempDir::new().expect("Failed to create temp directory");
    let file = dir.child("file");
    let file2 = dir.child("file2");
    create_ethersync_dir(dir.path());

    // Set up the actors.
    let daemon = Daemon::new(None, Path::new("/tmp/ethersync"), file.as_path());

    let nvim = Neovim::new(file).await;

    let peer = Daemon::new(
        Some(daemon.tcp_address()),
        Path::new("/tmp/etherbonk"),
        file2.as_path(),
    );

    let mut nvim2 = Neovim::new(file2).await;
    nvim2.etherbonk().await;

    let mut actors: HashMap<String, Box<dyn Actor>> = HashMap::new();
    actors.insert("daemon".to_string(), Box::new(daemon));
    actors.insert("nvim".to_string(), Box::new(nvim));
    actors.insert("peer".to_string(), Box::new(peer));
    actors.insert("nvim2".to_string(), Box::new(nvim2));

    sleep(std::time::Duration::from_millis(100)).await;

    // Perform random edits in parallel.
    timeout(std::time::Duration::from_secs(1), async {
        let handles = actors
            .iter_mut()
            .map(|(_, actor)| perform_random_edits(actor.as_mut()));
        join_all(handles).await;
    })
    .await
    .expect_err("Random edits died unexpectedly");

    info!("Sleep a bit, so that the actors can sync");
    sleep(std::time::Duration::from_millis(1000)).await;
    // TODO: Maybe broadcast "ready" message? Wait for roundtrip?

    let mut contents: HashMap<String, String> = HashMap::new();
    for (name, actor) in &mut actors {
        let content = actor.content().await;
        contents.insert(name.clone(), content.clone());
        println!(
            r#"
{name} content:
---------------------------------
{content}
---------------------------------
"#
        );
    }

    // Check that all contents are identical.
    let first = contents.values().next().expect("No contents found");
    let first_name = contents.keys().next().expect("No content keys found");
    for (name, content) in &contents {
        assert_eq!(
            first, content,
            "Content of {} differs from {}",
            first_name, name
        );
    }

    println!("SUCCESS! 🥳");
}
