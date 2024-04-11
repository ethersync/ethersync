use actors::{random_delta, Actor, Neovim};
use ethersync::daemon::Daemon;
use rand::Rng;
use std::path::Path;
use std::path::PathBuf;
use tokio::time::{sleep, timeout};

// TODO: Move types to lib directory.
#[path = "../actors.rs"]
mod actors;
#[path = "../daemon.rs"]
mod daemon;
#[path = "../ot.rs"]
mod ot;
#[path = "../types.rs"]
mod types;

async fn perform_random_edits(actor: &mut impl Actor) {
    loop {
        let content = actor.content().await;
        let delta = random_delta(&content);
        actor.apply_delta(delta).await;
        let random_millis = rand::thread_rng().gen_range(0..1000);
        sleep(std::time::Duration::from_millis(random_millis)).await;

        /*if rand::thread_rng().gen_range(0.0..0.1) < 0.1 {
            if rand::thread_rng().gen_range(0.0..0.1) < 0.5 {
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
    // Set up the project directory.
    let dir = temp_dir::TempDir::new().unwrap();
    let file = dir.child("file");
    let file2 = dir.child("file2");
    create_ethersync_dir(dir.path().to_path_buf());

    println!("Setting up actors");

    // Set up the actors.
    let mut daemon = Daemon::new(None, Path::new("/tmp/ethersync"), file.as_path());

    let mut nvim = Neovim::new().await;
    let mut buffer = nvim.open(file).await;

    println!("Launching peer");

    let mut peer = Daemon::new(
        Some(daemon.tcp_address()),
        Path::new("/tmp/etherbonk"),
        file2.as_path(),
    );

    println!("Performing random edits");

    // Perform random edits in parallel for a number of seconds.
    timeout(std::time::Duration::from_secs(1), async {
        perform_random_edits(&mut daemon).await;
        perform_random_edits(&mut peer).await;
        perform_random_edits(&mut buffer).await;
    })
    .await
    .unwrap();

    // Set all actors to be online.
    /*
    daemon.set_online(true);
    peer.set_online(true);
    buffer.set_online(true);
    */

    println!("Sleep a bit");

    // Wait for a moment to allow them to sync.
    sleep(std::time::Duration::from_secs(1)).await;
    // TODO: Maybe broadcast "ready" message? Wait for roundtrip?

    println!("Checking content");

    // Check that all actors have the same content.
    let buffer_content = buffer.content().await;
    let daemon_content = <Daemon as Actor>::content(&daemon).await;
    let peer_content = <Daemon as Actor>::content(&peer).await;
    assert_eq!(buffer_content, daemon_content);
    assert_eq!(buffer_content, peer_content);
}
