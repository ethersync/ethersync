#![allow(dead_code)]
use actors::{Actor, Neovim};
use ethersync::daemon::Daemon;
use pretty_assertions::assert_eq;
use rand::Rng;
use std::path::Path;
use std::path::PathBuf;
use tokio::time::{sleep, timeout};
use tracing_subscriber::FmtSubscriber;

// TODO: Can we do this in a better way?
#[path = "../actors.rs"]
mod actors;

async fn perform_random_edits(actor: &mut impl Actor) {
    loop {
        actor.apply_random_delta().await;
        let random_millis = rand::thread_rng().gen_range(0..100);
        sleep(std::time::Duration::from_millis(random_millis)).await;

        /*if rand::thread_rng().gen_range(0.0..1.0) < 0.1 {
            if rand::thread_rng().gen_range(0.0..1.0) < 0.5 {
                actor.set_online(true);
            } else {
                actor.set_online(false);
            }
        }*/
    }
}

fn create_ethersync_dir(dir: PathBuf) {
    let mut ethersync_dir = dir.clone();
    ethersync_dir.push(".ethersync");
    std::fs::create_dir(ethersync_dir).unwrap();
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default log subscriber failed");

    // Set up the project directory.
    let dir = temp_dir::TempDir::new().unwrap();
    let file = dir.child("file");
    let file2 = dir.child("file2");
    create_ethersync_dir(dir.path().to_path_buf());

    println!("Setting up actors");

    // Set up the actors.
    let mut daemon = Daemon::new(None, Path::new("/tmp/ethersync"), file.as_path());

    sleep(std::time::Duration::from_secs(1)).await;

    let mut nvim = Neovim::new(file).await;

    println!("Launching peer");

    let mut peer = Daemon::new(
        Some(daemon.tcp_address()),
        Path::new("/tmp/etherbonk"),
        file2.as_path(),
    );

    println!("Performing random edits");

    sleep(std::time::Duration::from_secs(1)).await;

    // Perform random edits in parallel for a number of seconds.
    timeout(std::time::Duration::from_secs(2), async {
        tokio::join!(
            perform_random_edits(&mut daemon),
            perform_random_edits(&mut peer),
            perform_random_edits(&mut nvim),
        )
    })
    .await
    .expect_err("Random edits died unexpectedly");

    // Set all actors to be online.
    /*
    daemon.set_online(true);
    peer.set_online(true);
    nvim.set_online(true);
    */

    println!("Sleep a bit");

    // Wait for a moment to allow them to sync.
    sleep(std::time::Duration::from_secs(1)).await;
    // TODO: Maybe broadcast "ready" message? Wait for roundtrip?

    println!("Checking content");

    // Check that all actors have the same content.
    let nvim_content = nvim.content().await;
    let daemon_content = <Daemon as Actor>::content(&daemon).await;
    let peer_content = <Daemon as Actor>::content(&peer).await;

    println!("Neovim: {:?}", nvim_content);
    println!("Daemon: {:?}", daemon_content);
    println!("Peer:   {:?}", peer_content);

    if nvim_content != daemon_content {
        println!("Neovim and daemon content differ");
    }
    if nvim_content != peer_content {
        println!("Neovim and peer content differ");
    }

    assert_eq!(nvim_content, daemon_content);
    assert_eq!(nvim_content, peer_content);
}
