use rand::Rng;
use std::path::PathBuf;
use tokio::time::{sleep, timeout};
use types::TextDelta;

// TODO: Move types to lib directory?
#[path = "../types.rs"]
mod types;

// Construct a random insertion or deletion for the content.
fn random_delta(content: &str) -> TextDelta {
    let content_length = content.chars().count();
    if rand::thread_rng().gen_range(0.0..0.1) < 0.5 {
        // Insertion.
        let start = rand::thread_rng().gen_range(0..content_length);
        let length = rand::thread_rng().gen_range(0..10);
        let text = (0..length)
            .map(|_| rand::random::<char>())
            .collect::<String>();
        let mut delta = TextDelta::default();
        delta.retain(start);
        delta.insert(&text);
        delta
    } else {
        // Deletion.
        let start = rand::thread_rng().gen_range(0..content_length);
        let length = rand::thread_rng().gen_range(0..content_length - start);
        let mut delta = TextDelta::default();
        delta.retain(start);
        delta.delete(length);
        delta
    }
}

trait Actor {
    fn content(&self) -> String;
    fn apply_delta(&mut self, delta: TextDelta);
    //fn wait_for_sync(&self);
    fn set_online(&mut self, online: bool);
}

struct Daemon {}

struct Neovim {}

struct Buffer {}

impl Daemon {
    fn new() -> Self {
        todo!()
    }

    fn launch(&mut self, _address: Option<String>) {
        todo!()
    }

    fn tcp_address(&self) -> String {
        todo!()
    }
}

impl Neovim {
    fn new() -> Self {
        todo!()
    }

    fn open(&mut self, file: PathBuf) -> Buffer {
        todo!()
    }
}

impl Actor for Daemon {
    fn content(&self) -> String {
        todo!()
    }

    fn apply_delta(&mut self, delta: TextDelta) {
        todo!()
    }

    fn set_online(&mut self, online: bool) {
        todo!()
    }
}

impl Actor for Buffer {
    fn content(&self) -> String {
        todo!()
    }

    fn apply_delta(&mut self, delta: TextDelta) {
        todo!()
    }

    fn set_online(&mut self, online: bool) {
        todo!()
    }
}

async fn perform_random_edits(actor: &mut impl Actor) {
    loop {
        let content = actor.content();
        let delta = random_delta(&content);
        actor.apply_delta(delta);
        let random_millis = rand::thread_rng().gen_range(0..1000);
        sleep(std::time::Duration::from_millis(random_millis)).await;

        if rand::thread_rng().gen_range(0.0..0.1) < 0.1 {
            if rand::thread_rng().gen_range(0.0..0.1) < 0.5 {
                actor.set_online(true);
            } else {
                actor.set_online(false);
            }
        }
    }
}

fn create_ethersync_dir(dir: PathBuf) {
    todo!();
}

#[tokio::main]
async fn main() {
    // Set up the project directory.
    let dir = temp_dir::TempDir::new().unwrap();
    let file = dir.child("file");
    create_ethersync_dir(dir.path().to_path_buf());

    // Set up the actors.
    let mut daemon = Daemon::new();
    daemon.launch(None);

    let mut nvim = Neovim::new();
    let mut buffer = nvim.open(file);

    let mut peer = Daemon::new();
    peer.launch(Some(daemon.tcp_address()));

    // Perform random edits in parallel for a number of seconds.
    timeout(std::time::Duration::from_secs(10), async {
        perform_random_edits(&mut daemon).await;
        perform_random_edits(&mut peer).await;
        perform_random_edits(&mut buffer).await;
    })
    .await
    .unwrap();

    // Set all actors to be online.
    daemon.set_online(true);
    peer.set_online(true);
    buffer.set_online(true);

    // Wait for a moment to allow them to sync.
    sleep(std::time::Duration::from_secs(1)).await;
    // TODO: Maybe broadcast "ready" message? Wait for roundtrip?

    // Check that all actors have the same content.
    let buffer_content = buffer.content();
    let daemon_content = daemon.content();
    let peer_content = peer.content();
    assert_eq!(buffer_content, daemon_content);
    assert_eq!(buffer_content, peer_content);
}

#[test]
fn test_random_delta() {
    (0..100).for_each(|_| {
        let content = "Hello, world!";
        let delta = random_delta(content);
        // Verify that delta can be applied to content.
        delta.apply(content);
    });
}
