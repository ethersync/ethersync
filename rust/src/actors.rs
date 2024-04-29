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

        let string_components = vec![
            "e".to_string(),
            "💚".to_string(),
            "🥕".to_string(),
            "\n".to_string(),
        ];
        let s = random_string(rand_usize_inclusive(1, 4), string_components);

        let components = vec![
            "h".to_string(),
            "j".to_string(),
            "k".to_string(),
            "l".to_string(),
            "gg".to_string(),
            "G".to_string(),
            "$".to_string(),
            "^".to_string(),
            "x".to_string(),
            "vllld".to_string(),
            "rü".to_string(),
            "dd".to_string(),
            "J".to_string(),
            format!("i{}", s),
            format!("o{}", s),
            format!("O{}", s),
            format!("A{}", s),
            format!("I{}", s),
        ];

        vim_normal_command.push_str(&random_string(rand_usize_inclusive(1, 10), components));

        self.nvim
            .command(&format!(r#"silent! execute "normal {vim_normal_command}""#))
            //.input(&vim_normal_command)
            .await
            .expect("Failed to send input to Neovim");
    }
    async fn content(&self) -> String {
        let mut content = self
            .buffer
            .get_lines(0, -1, false)
            .await
            .unwrap()
            .join("\n");
        if self
            .nvim
            .command_output("set eol?")
            .await
            .expect("Failed to get value of eol")
            .trim()
            == "endofline"
        {
            content.push('\n');
        }
        content
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

fn random_string(length: usize, components: Vec<String>) -> String {
    (0..length)
        .map(|_| components[rand_usize_inclusive(0, components.len() - 1)].clone())
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
    use crate::types::{
        factories::*, EditorProtocolMessage, EditorTextDelta, EditorTextOp,
        RevisionedEditorTextDelta,
    };
    use pretty_assertions::assert_eq;
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

    fn assert_vim_deltas_yield_content(
        initial_content: &str,
        deltas: Vec<EditorTextOp>,
        expected_content: &str,
    ) {
        let runtime = Runtime::new().expect("Could not create Tokio runtime");
        runtime.block_on(async {
            let mut socket = MockSocket::new("/tmp/ethersync").await;
            let nvim = Neovim::new_ethersync_enabled(initial_content).await;

            for op in deltas {
                let rev_editor_delta = RevisionedEditorTextDelta {
                    revision: 0,
                    delta: EditorTextDelta(vec![op]),
                };
                let editor_message = EditorProtocolMessage::Edit {
                    uri: "<tbd>".to_string(),
                    delta: rev_editor_delta,
                };
                let payload = editor_message
                    .to_jsonrpc()
                    .expect("Could not serialize EditorTextDelta");
                socket.send(&format!("{payload}\n")).await;
            }

            tokio::time::sleep(Duration::from_millis(10)).await; // TODO: This is a bit funny, but it seems necessary?

            assert_eq!(nvim.content().await, expected_content);
        });
    }

    #[test]
    #[ignore]
    fn vim_processes_deltas_correctly() {
        assert_vim_deltas_yield_content("", vec![replace_ed((0, 0), (0, 0), "a")], "a");

        assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "\n\n")], "x\n\n");

        // TODO: Is it important that this works?
        // assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "")], "x");

        assert_vim_deltas_yield_content(
            "bananas",
            vec![
                replace_ed((0, 2), (0, 3), ""),
                replace_ed((0, 3), (0, 4), ""),
            ],
            "baaas",
        );

        assert_vim_deltas_yield_content("ba\nna\nnas", vec![replace_ed((0, 1), (2, 1), "")], "bas");
    }

    fn assert_vim_input_yields_replacements(
        initial_content: &str,
        input: &str,
        expected_replacements: Vec<EditorTextOp>,
    ) {
        let runtime = Runtime::new().expect("Could not create Tokio runtime");
        runtime.block_on(async {
            timeout(Duration::from_millis(5000), async {
                let mut socket = MockSocket::new("/tmp/ethersync").await;
                let mut nvim = Neovim::new_ethersync_enabled(initial_content).await;
                nvim.input(input).await;

                let msg = socket.recv().await;
                assert_eq!(msg["method"], "open");

                // TODO: This doesn't check whether there are more replacements pending than the
                // expected ones.
                for expected_replacement in expected_replacements {
                    let msg = socket.recv().await;
                    let message: EditorProtocolMessage = serde_json::from_str(&msg.to_string())
                        .expect("Could not parse EditorProtocolMessage");
                    if let EditorProtocolMessage::Edit{ delta, ..} = message {
                        let operations = delta.delta.0;
                        assert_eq!(vec![expected_replacement], operations, "Different replacements when applying input '{}' to content '{:?}'", input, initial_content);
                    } else {
                        panic!("Expected edit message, got {:?}", message);
                    }
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Nvim test for input '{input}' on '{initial_content:?}' timed out. Maybe increase timeout to make sure vim started fast enough. We probably received too few messages?"
                )
            });
        });
    }

    #[ignore]
    #[test]
    fn vim_sends_correct_delta() {
        // Edits on a single line.
        assert_vim_input_yields_replacements("", "ia", vec![replace_ed((0, 0), (0, 0), "a")]);
        assert_vim_input_yields_replacements("a\n", "x", vec![replace_ed((0, 0), (0, 1), "")]);
        assert_vim_input_yields_replacements("abc\n", "lx", vec![replace_ed((0, 1), (0, 2), "")]);
        assert_vim_input_yields_replacements("abc\n", "vd", vec![replace_ed((0, 0), (0, 1), "")]);
        assert_vim_input_yields_replacements("abc\n", "vlld", vec![replace_ed((0, 0), (0, 3), "")]);
        assert_vim_input_yields_replacements("a\n", "rb", vec![replace_ed((0, 0), (0, 1), "b")]);
        // To add to end of line, the existence of a newline should not matter.
        assert_vim_input_yields_replacements("a", "Ab", vec![replace_ed((0, 1), (0, 1), "b")]);
        assert_vim_input_yields_replacements("a\n", "Ab", vec![replace_ed((0, 1), (0, 1), "b")]);
        assert_vim_input_yields_replacements("a\n", "Ib", vec![replace_ed((0, 0), (0, 0), "b")]);

        // Edits involving multiple lines.
        assert_vim_input_yields_replacements("a\n", "O", vec![replace_ed((0, 0), (0, 0), "\n")]);
        // Indentation matters.
        assert_vim_input_yields_replacements(
            "    a\n",
            "O",
            vec![replace_ed((0, 0), (0, 0), "    \n")],
        );
        assert_vim_input_yields_replacements("a\nb\n", "dd", vec![replace_ed((0, 0), (1, 0), "")]);
        assert_vim_input_yields_replacements("a\nb\n", "jdd", vec![replace_ed((0, 1), (1, 1), "")]);
        // Also works without \n at the end.
        assert_vim_input_yields_replacements("a\nb", "jdd", vec![replace_ed((0, 1), (1, 1), "")]);
        // This seems to be the default behavior in vim: The newline goes away.
        assert_vim_input_yields_replacements("a\n", "dd", vec![replace_ed((0, 0), (1, 0), "")]);
        // Our design goal: produce something, that works without any implict newlines.
        assert_vim_input_yields_replacements("a", "dd", vec![replace_ed((0, 0), (0, 1), "")]);
        // Test what happens when we start with empty buffer:
        // The eol option can be "true" unexpectedly.
        assert_vim_input_yields_replacements(
            "",
            "ia<Esc>dd",
            vec![
                replace_ed((0, 0), (0, 0), "a"),
                replace_ed((0, 0), (0, 1), ""),
            ],
        );

        assert_vim_input_yields_replacements("", "i<CR>", vec![replace_ed((0, 0), (0, 0), "\n")]);
        assert_vim_input_yields_replacements(
            "",
            "i<CR>i",
            vec![
                replace_ed((0, 0), (0, 0), "\n"),
                replace_ed((1, 0), (1, 0), "i"),
            ],
        );
        assert_vim_input_yields_replacements(
            "",
            "ia<CR>a",
            vec![
                replace_ed((0, 0), (0, 0), "a"),
                replace_ed((0, 1), (0, 1), "\n"),
                replace_ed((1, 0), (1, 0), "a"),
            ],
        );

        assert_vim_input_yields_replacements(
            "a\n",
            ":s/a/b<CR>",
            vec![replace_ed((0, 0), (0, 1), "b")],
        );

        assert_vim_input_yields_replacements(
            "",
            "i<CR><BS>",
            vec![
                replace_ed((0, 0), (0, 0), "\n"),
                // no-op: Copy nothing to previous line.
                replace_ed((0, 0), (0, 0), ""),
                replace_ed((0, 0), (1, 0), ""),
            ],
        );

        assert_vim_input_yields_replacements(
            "a\n",
            "ddix<CR><BS>",
            vec![
                replace_ed((0, 0), (1, 0), ""),
                replace_ed((0, 0), (0, 0), "x"),
                replace_ed((0, 1), (0, 1), "\n"),
                // no-op: Copy nothing to previous line.
                replace_ed((0, 1), (0, 1), ""),
                replace_ed((0, 1), (1, 0), ""),
            ],
        );

        assert_vim_input_yields_replacements(
            "hello\nworld\n",
            "llvjd",
            vec![
                replace_ed((0, 2), (0, 5), ""), // d: llo
                replace_ed((1, 0), (1, 3), ""), // d: wor
                replace_ed((0, 2), (0, 2), "ld"),
                replace_ed((0, 4), (1, 2), ""), // d: \nld
            ],
        );

        assert_vim_input_yields_replacements(
            "",
            "ox",
            vec![
                replace_ed((0, 0), (0, 0), "\n"),
                replace_ed((1, 0), (1, 0), "x"),
            ],
        );

        // Unicode tests
        assert_vim_input_yields_replacements("ä\nü\n", "dd", vec![replace_ed((0, 0), (1, 0), "")]);
        assert_vim_input_yields_replacements("ä💚🥕", "vlld", vec![replace_ed((0, 0), (0, 3), "")]);
        assert_vim_input_yields_replacements("ä", "dd", vec![replace_ed((0, 0), (0, 1), "")]);

        // Tests where Vim behaves a bit weirdly.

        // A direct replace_ed((0, 1), (0, 1), "\n") would be nicer.
        assert_vim_input_yields_replacements("a", "o", vec![replace_ed((0, 1), (0, 1), "\n")]);

        // A direct replace_ed((0, 1), (0, 1), "\n") would be nicer.
        assert_vim_input_yields_replacements("a\n", "o", vec![replace_ed((0, 1), (1, 0), "\n\n")]);

        assert_vim_input_yields_replacements(
            "eins\ntwo\n",
            "jo",
            vec![replace_ed((1, 3), (2, 0), "\n\n")],
        );

        assert_vim_input_yields_replacements("a", "yyp", vec![replace_ed((0, 1), (0, 1), "\na")]);

        // A direct replace_ed((1, 0), (1, 0), "a\n") would be nicer.
        assert_vim_input_yields_replacements(
            "a\n",
            "yyp",
            // Could change depending on what's easier to handle implementation-wise.
            vec![replace_ed((0, 1), (1, 0), "\na\n")],
        );

        // A direct replace_ed((0, 1), (1, 0), " ") would be nicer.
        assert_vim_input_yields_replacements(
            "a\nb\n",
            "J",
            vec![
                replace_ed((0, 1), (0, 1), " b"),
                replace_ed((0, 3), (1, 1), ""),
            ],
        );

        assert_vim_input_yields_replacements(
            "a\nb",
            "J",
            vec![
                replace_ed((0, 1), (0, 1), " b"),
                replace_ed((0, 3), (1, 1), ""),
            ],
        );
    }
}
