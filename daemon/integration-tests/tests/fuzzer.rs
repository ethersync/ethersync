// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use ethersync_integration_tests::actors::{Actor, Neovim};

use ethersync::config::AppConfig;
use ethersync::daemon::{Daemon, TEST_FILE_PATH};
use ethersync::editor::get_socket_path;
use ethersync::logging;
use ethersync::sandbox;

use anyhow::Result;
use futures::future::join_all;
use pretty_assertions::assert_eq;
use rand::Rng;
use tokio::time::{sleep, timeout, Duration};
use tracing::{error, info};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

async fn perform_random_edits(actor: &mut (impl Actor + ?Sized)) {
    for _ in 1..500 {
        actor.apply_random_delta().await;

        let random_millis = rand::thread_rng().gen_range(0..5);
        sleep(Duration::from_millis(random_millis)).await;
    }
}

fn initialize_project() -> (temp_dir::TempDir, PathBuf) {
    let dir = temp_dir::TempDir::new().expect("Failed to create temp directory");
    let mut ethersync_dir = dir.path().to_path_buf();
    ethersync_dir.push(".ethersync");
    sandbox::create_dir(dir.path(), &ethersync_dir).expect("Failed to create .ethersync directory");

    let file = dir.child(TEST_FILE_PATH);
    sandbox::write_file(dir.path(), &file, b"").expect("Failed to create file in temp directory");

    (dir, file)
}

#[tokio::main]
async fn main() -> Result<()> {
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    logging::initialize()?;

    // Set up files in project directories.
    let (dir, file) = initialize_project();
    let (dir2, file2) = initialize_project();

    // Set up the actors.
    let socket_name = Path::new("ethersync-fuzzer-peer-1");
    let socket_path = get_socket_path(socket_name);
    let daemon = Daemon::new(
        AppConfig {
            peer: None,
            emit_join_code: Some(false),
            emit_secret_address: Some(false),
        },
        &socket_path,
        dir.path(),
        true,
    )
    .await?;

    // Wait until iroh's DNS discovery (hopefully) works.
    sleep(Duration::from_millis(1000)).await;

    std::env::set_var("ETHERSYNC_SOCKET", socket_name);
    let nvim = Neovim::new(file).await;

    let socket_name_2 = Path::new("ethersync-fuzzer-peer-2");
    let socket_path_2 = get_socket_path(socket_name_2);
    let peer = Daemon::new(
        AppConfig {
            peer: Some(daemon.address.clone()),
            emit_join_code: Some(false),
            emit_secret_address: Some(false),
        },
        &socket_path_2,
        dir2.path(),
        false,
    )
    .await?;

    // Wait until file2 appears.
    while !file2.exists() {
        dbg!("{&file2} doesnt");
        sleep(Duration::from_millis(100)).await;
    }

    std::env::set_var("ETHERSYNC_SOCKET", socket_name_2);
    let nvim2 = Neovim::new(file2).await;

    // Give the second Neovim time to process the "open" call.
    sleep(Duration::from_millis(1000)).await;

    let mut actors: HashMap<String, Box<dyn Actor>> = HashMap::new();
    actors.insert("daemon".to_string(), Box::new(daemon));
    actors.insert("nvim".to_string(), Box::new(nvim));
    actors.insert("peer".to_string(), Box::new(peer));
    actors.insert("nvim2".to_string(), Box::new(nvim2));

    info!("Performing edits");

    let handles = actors
        .iter_mut()
        .map(|(_, actor)| perform_random_edits(actor.as_mut()));
    join_all(handles).await;

    let mut contents: HashMap<String, String> = HashMap::new();

    info!("Waiting for all contents to be equal");

    timeout(Duration::from_secs(5 * 60), async {
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
            sleep(Duration::from_millis(1000)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        error!("Timeout while waiting for all contents to be equal");
    });

    // Get all contents.
    for (name, actor) in &mut actors {
        let content = actor.content().await;
        contents.insert(name.clone(), content.clone());
    }

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

    // Quit immediately, so that we don't run into cleanup issues, which would make our CI fail...
    // TODO: Handle shutdown more gracefully.
    std::process::exit(0);
}
