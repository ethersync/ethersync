// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use ethersync::sandbox;
use serde_json::Value as JSONValue;
use std::path::Path;
use tokio::{
    io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::UnixListener,
    sync::mpsc,
    time::Duration,
};

pub struct MockSocket {
    writer_tx: tokio::sync::mpsc::Sender<String>,
    reader_rx: tokio::sync::mpsc::Receiver<String>,
}

impl MockSocket {
    pub fn new(socket_path: &Path) -> Self {
        let socket_dir = socket_path
            .parent()
            .expect("The constructed socket paths should be in a directory");
        if sandbox::exists(socket_dir, socket_path).expect("Could not check for socket existence") {
            sandbox::remove_file(socket_dir, socket_path).expect("Could not remove socket");
        }

        let listener = UnixListener::bind(socket_path).expect("Could not bind to socket");
        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(1);
        let (reader_tx, reader_rx) = mpsc::channel::<String>(1);

        tokio::spawn(async move {
            let (socket, _) = listener
                .accept()
                .await
                .expect("Could not accept connection");

            let (reader, writer) = split(socket);
            let mut writer = BufWriter::new(writer);
            let mut reader = BufReader::new(reader);

            tokio::spawn(async move {
                while let Some(message) = writer_rx.recv().await {
                    writer
                        .write_all(message.as_bytes())
                        .await
                        .expect("Could not write to socket");
                    writer.flush().await.expect("Could not flush socket");
                }
            });

            tokio::spawn(async move {
                let mut buffer = String::new();
                while reader.read_line(&mut buffer).await.is_ok() {
                    reader_tx
                        .send(buffer.clone())
                        .await
                        .expect("Could not send message to reader channel");
                    buffer.clear();
                }
            });
        });

        Self {
            writer_tx,
            reader_rx,
        }
    }

    pub async fn send(&mut self, message: &str) {
        self.writer_tx
            .send(message.to_string())
            .await
            .expect("Could not send message");
    }

    pub async fn recv(&mut self) -> JSONValue {
        let line = self
            .reader_rx
            .recv()
            .await
            .expect("Could not receive message");
        serde_json::from_str(&line).expect("Could not parse JSON")
    }

    pub async fn acknowledge_open(&mut self) -> JSONValue {
        let json = self.recv().await;
        if json.get("method").unwrap() == "open" {
            let id = json.get("id").unwrap();
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": "success"
            });
            self.send(&response.to_string()).await;
            self.send("\n").await;
            // Wait a bit so that Neovim can boot up its change tracking.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        json
    }
}
