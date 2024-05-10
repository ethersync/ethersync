#![allow(dead_code)]
use ethersync::actors::{Actor, Neovim};
use ethersync::daemon::Daemon;
use ethersync::logging;
use futures::future::join_all;
use pretty_assertions::assert_eq;
use rand::Rng;
use std::collections::HashMap;
use std::path::Path;
use tokio::time::{sleep, timeout, Duration};

async fn perform_random_edits(actor: &mut (impl Actor + ?Sized)) {
    for _ in 1..10 {
        actor.apply_random_delta().await;

        // Note: Don't lower the lower bound too much. nvim-rs seems to require that inputs are not
        // being sent too quickly?
        let random_millis = rand::thread_rng().gen_range(0..1);
        sleep(Duration::from_millis(random_millis)).await;
    }
}

fn create_ethersync_dir(dir: &Path) {
    let mut ethersync_dir = dir.to_path_buf();
    ethersync_dir.push(".ethersync");
    std::fs::create_dir(ethersync_dir).expect("Failed to create .ethersync directory");
}

#[tokio::main]
async fn main() {
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    logging::initialize(true);

    // Set up the project directory.
    let dir = temp_dir::TempDir::new().expect("Failed to create temp directory");
    let file = dir.child("file");
    //let file2 = dir.child("file2");
    create_ethersync_dir(dir.path());

    // Set up the actors.
    let daemon = Daemon::new(None, None, Path::new("/tmp/ethersync"), file.as_path());

    let nvim = Neovim::new(file).await;

    /*let peer = Daemon::new(
        None,
        Some("127.0.0.1:4242".to_string()),
        Path::new("/tmp/etherbonk"),
        file2.as_path(),
    );

    std::env::set_var("ETHERSYNC_SOCKET", "/tmp/etherbonk");
    let nvim2 = Neovim::new(file2).await;
    */

    let mut actors: HashMap<String, Box<dyn Actor>> = HashMap::new();
    actors.insert("daemon".to_string(), Box::new(daemon));
    actors.insert("nvim".to_string(), Box::new(nvim));
    //actors.insert("peer".to_string(), Box::new(peer));
    //actors.insert("nvim2".to_string(), Box::new(nvim2));

    sleep(std::time::Duration::from_millis(100)).await;

    // Perform random edits in parallel.
    let handles = actors
        .iter_mut()
        .map(|(_, actor)| perform_random_edits(actor.as_mut()));
    join_all(handles).await;

    let mut contents: HashMap<String, String> = HashMap::new();

    let _ = timeout(Duration::from_secs(1), async {
        loop {
            // Get all contents.
            for (name, actor) in &mut actors {
                let content = actor.content().await;
                contents.insert(name.clone(), content.clone());
            }

            // If all contents are equal already, we have succeeded!
            let first = contents.values().next().expect("No contents found");
            let mut all_equal = true;
            for content in contents.values() {
                if first != content {
                    all_equal = false;
                }
            }
            if all_equal {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    // Print all contents.
    for (name, content) in &contents {
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
