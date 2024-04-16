use crate::daemon::Daemon;
use async_trait::async_trait;
use nvim_rs::{compat::tokio::Compat, create::tokio::new_child_cmd, rpc::handler::Dummy};
use rand::Rng;
use serde_json::Value as JSONValue;
use std::fs;
use std::path::{Path, PathBuf};
use temp_dir::TempDir;
use tokio::{
    io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::UnixListener,
    process::ChildStdin,
    sync::mpsc,
};

// TODO: Consider renaming this, to avoid confusion with tokio "actors".
#[async_trait]
pub trait Actor: Send {
    async fn apply_random_delta(&mut self);
    async fn content(&self) -> String;
    //fn wait_for_sync(&self);
    //async fn set_online(&mut self, online: bool);
}

pub struct Neovim {
    nvim: nvim_rs::Neovim<Compat<ChildStdin>>,
    buffer: nvim_rs::Buffer<Compat<ChildStdin>>,
}

impl Neovim {
    pub async fn new(file_path: PathBuf) -> Self {
        let handler = Dummy::new();
        let mut cmd = tokio::process::Command::new("nvim");
        cmd.arg("--headless").arg("--embed");
        let (nvim, _, _) = new_child_cmd(&mut cmd, handler).await.unwrap();

        nvim.command(&format!("edit! {}", file_path.display()))
            .await
            .expect("Opening file in nvim failed");
        let buffer = nvim.get_current_buf().await.unwrap();

        Self { nvim, buffer }
    }

    pub async fn input(&mut self, input: &str) {
        self.nvim
            .input(input)
            .await
            .expect("Failed to send input to Neovim");
    }

    // TODO: The "Etherbonk" approach is not a very good way of picking different sockets...
    pub async fn etherbonk(&mut self) {
        self.nvim
            .command("Etherbonk")
            .await
            .expect("Running Etherbonk failed");
    }

    #[allow(dead_code)]
    async fn new_ethersync_enabled(initial_content: &str) -> Self {
        let dir = TempDir::new().unwrap();
        let ethersync_dir = dir.child(".ethersync");
        std::fs::create_dir(ethersync_dir).unwrap();
        let file_path = dir.child("test");
        std::fs::write(&file_path, initial_content).unwrap();

        Self::new(file_path).await
    }
}

#[async_trait]
impl Actor for Daemon {
    async fn apply_random_delta(&mut self) {
        self.apply_random_delta().await;
    }

    async fn content(&self) -> String {
        self.content().await.unwrap()
    }
}

#[async_trait]
impl Actor for Neovim {
    async fn apply_random_delta(&mut self) {
        let mut vim_normal_command = String::new();

        let directions = ["h", "j", "k", "l"];
        (0..10).for_each(|_| {
            vim_normal_command
                .push_str(directions[rand::thread_rng().gen_range(0..(directions.len()))]);
        });

        // TODO: There seems to be a bug when enabling multiline insertions and/or multi-line
        // deletions. Something to do with empty lines?
        if rand::thread_rng().gen_bool(0.5) {
            let deletion_components = vec!["x"]; //, "dd", "vllld"];
            vim_normal_command.push_str(&random_string(
                rand_usize_inclusive(1, 2),
                &deletion_components,
            ));
        } else {
            vim_normal_command.push('i');
            let vim_components = vec!["x", "🥕", "_", "💚"]; //, "\n"];
            vim_normal_command
                .push_str(&random_string(rand_usize_inclusive(1, 10), &vim_components));
        }

        // We run the commands using :silent!, so that they don't stop on errors (e.g. when trying
        // to navigate outside of the buffer).
        self.nvim
            .command(&format!(r#"silent! execute "normal {vim_normal_command}""#))
            .await
            .expect("Executing normal command failed");
    }
    async fn content(&self) -> String {
        self.buffer
            .get_lines(0, -1, false)
            .await
            .unwrap()
            .join("\n")
    }

    /*
    async fn apply_delta(&mut self, _delta: TextDelta) {
        // TODO: Actually apply the delta.
        self.buffer
            .set_text(0, 0, 0, 0, vec!["!".into()])
            .await
            .unwrap();
    }*/
}

fn random_string(length: usize, components: &[&str]) -> String {
    (0..length)
        .map(|_| components[rand_usize_inclusive(0, components.len() - 1)])
        .collect::<String>()
}

fn rand_usize_inclusive(start: usize, end: usize) -> usize {
    if start == end {
        start
    } else {
        rand::thread_rng().gen_range(start..=end)
    }
}

#[allow(dead_code)]
struct MockSocket {
    writer_tx: tokio::sync::mpsc::Sender<String>,
    reader_rx: tokio::sync::mpsc::Receiver<String>,
}

#[allow(dead_code)]
impl MockSocket {
    async fn new(socket_path: &str) -> Self {
        if Path::new(socket_path).exists() {
            fs::remove_file(socket_path).expect("Could not remove existing socket file");
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

    async fn send(&mut self, message: &str) {
        self.writer_tx
            .send(message.to_string())
            .await
            .expect("Could not send message");
    }

    async fn recv(&mut self) -> JSONValue {
        loop {
            let line = self
                .reader_rx
                .recv()
                .await
                .expect("Could not receive message");
            let json: JSONValue = serde_json::from_str(&line).expect("Could not parse JSON");
            if json["method"] == "debug" {
                continue;
            } else {
                return json;
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::daemon::jsonrpc_to_docmessage;
    use crate::daemon::DocMessage;
    use crate::types::{factories::*, TextDelta};
    use tokio::{
        runtime::Runtime,
        time::{timeout, Duration},
    };

    #[test]
    #[ignore] // TODO: enable as soon as we have figured out how to install plugin on gh actions
    fn plugin_loaded() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let handler = Dummy::new();
            let mut cmd = tokio::process::Command::new("nvim");
            cmd.arg("--headless").arg("--embed");
            let (nvim, _, _) = new_child_cmd(&mut cmd, handler).await.unwrap();
            nvim.command("Ethersync")
                .await
                .expect("Failed to run Ethersync");
        });
    }

    #[test]
    #[ignore]
    fn vim_processes_delta() {
        let runtime = Runtime::new().expect("Could not create Tokio runtime");
        runtime.block_on(async {
            let mut socket = MockSocket::new("/tmp/ethersync").await;
            let nvim = Neovim::new_ethersync_enabled("").await;
            socket
                .send(r#"{"jsonrpc":"2.0","method":"operation","params":[0,["bananas"]]}"#)
                .await;
            socket.send("\n").await;
            tokio::time::sleep(Duration::from_millis(0)).await; // TODO: This is a bit funny, but it
                                                                // seems necessary.
            assert_eq!(nvim.content().await, "bananas");
        });
    }

    fn assert_vim_input_yields_text_deltas(
        initial_content: &str,
        input: &str,
        expected: Vec<TextDelta>,
    ) {
        let runtime = Runtime::new().expect("Could not create Tokio runtime");
        runtime.block_on(async {
            timeout(Duration::from_millis(500), async {
                let mut socket = MockSocket::new("/tmp/ethersync").await;
                let mut nvim = Neovim::new_ethersync_enabled(initial_content).await;
                nvim.input(input).await;

                let msg = socket.recv().await;
                assert_eq!(msg["method"], "open");

                for expected_delta in expected {
                    let msg = socket.recv().await;
                    let docmessage = jsonrpc_to_docmessage(&msg.to_string())
                        .expect("Could not convert JSON to DocMessage");
                    if let DocMessage::RevDelta(rev_delta) = docmessage {
                        let text_delta: TextDelta = rev_delta.delta.into();
                        assert_eq!(text_delta, expected_delta);
                    } else {
                        panic!("Expected RevDelta message, got something else");
                    }
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Nvim test for input '{input}' timed out. We probably received too few messages?"
                )
            });
        });
    }

    #[test]
    #[ignore]
    fn vim_sends_correct_delta() {
        assert_vim_input_yields_text_deltas("", "ia", vec![insert(0, "a")]);
        assert_vim_input_yields_text_deltas("a\n", "x", vec![delete(0, 1)]);
        assert_vim_input_yields_text_deltas("abc\n", "lx", vec![delete(1, 1)]);
        assert_vim_input_yields_text_deltas("abc\n", "vlld", vec![delete(0, 3)]);
        assert_vim_input_yields_text_deltas("a\n", "rb", vec![delete(0, 1), insert(0, "b")]);
        assert_vim_input_yields_text_deltas("a\n", "Ab", vec![insert(1, "b")]);
        assert_vim_input_yields_text_deltas("a\n", "Ib", vec![insert(0, "b")]);

        assert_vim_input_yields_text_deltas("a\n", "O", vec![insert(0, "\n")]);
        assert_vim_input_yields_text_deltas("a\nb\n", "dd", vec![delete(0, 2)]);
        assert_vim_input_yields_text_deltas("a\nb\n", "jdd", vec![delete(2, 2)]);
        assert_vim_input_yields_text_deltas("a\n", "dd", vec![delete(0, 2)]);

        assert_vim_input_yields_text_deltas(
            "a\n",
            ":s/a/b<CR>",
            vec![delete(0, 1), insert(0, "b")],
        );

        // TODO: Tests that are a bit weird:

        // A direct insert(1, "\n") would be nicer.
        assert_vim_input_yields_text_deltas("a\n", "o", vec![delete(1, 1), insert(1, "\n\n")]);

        // A direct insert(1, "\na") would be nicer.
        assert_vim_input_yields_text_deltas("a\n", "yyp", vec![delete(1, 1), insert(1, "\na\n")]);

        // vec![delete(1, 1), insert(1, " ")] would be nicer.
        assert_vim_input_yields_text_deltas("a\nb\n", "J", vec![insert(1, " b"), delete(4, 2)]);
    }
}
