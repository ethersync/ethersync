use crate::daemon::Daemon;
use crate::security;
use async_trait::async_trait;
use nvim_rs::{compat::tokio::Compat, create::tokio::new_child_cmd, rpc::handler::Dummy};
use rand::Rng;
use serde_json::Value as JSONValue;
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
    /// # Panics
    ///
    /// Will panic if Neovim cannot be started or if the file cannot be opened.
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

    /// # Panics
    ///
    /// Will panic if input cannot be sent to Neovim.
    pub async fn input(&mut self, input: &str) {
        self.nvim
            .input(input)
            .await
            .expect("Failed to send input to Neovim");
    }

    #[allow(dead_code)]
    async fn new_ethersync_enabled(initial_content: &str) -> (Self, PathBuf) {
        let dir = TempDir::new().unwrap();
        let ethersync_dir = dir.child(".ethersync");
        security::create_dir(dir.path(), &ethersync_dir).unwrap();
        let file_path = dir.child("test");
        security::write_file(dir.path(), &file_path, initial_content.as_bytes())
            .expect("Failed to write initial file content");

        (Self::new(file_path.clone()).await, file_path)
    }
}

#[async_trait]
impl Actor for Daemon {
    async fn apply_random_delta(&mut self) {
        self.document_handle.apply_random_delta().await;
    }

    async fn content(&self) -> String {
        self.document_handle.content().await.unwrap()
    }
}

#[async_trait]
impl Actor for Neovim {
    async fn apply_random_delta(&mut self) {
        let mut vim_normal_command = String::new();

        let string_components = vec![
            "e".to_string(),
            "ä".to_string(),
            "💚".to_string(),
            "🥕".to_string(),
            "\n".to_string(),
        ];
        let s = random_string(rand_usize_inclusive(1, 5), &string_components);

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
            "Vjjjd".to_string(),
            "rü".to_string(),
            "dd".to_string(),
            "J".to_string(),
            format!("i{}", s),
            format!("jjI{}", s),
            format!("o{}", s),
            format!("O{}", s),
            format!("A{}", s),
            format!("I{}", s),
        ];

        vim_normal_command.push_str(&random_string(rand_usize_inclusive(1, 2), &components));

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

fn random_string(length: usize, components: &[String]) -> String {
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
    fn new(socket_path: &str, ignore_reads: bool) -> Self {
        if Path::new(socket_path).exists() {
            security::remove_file(Path::new("/tmp"), Path::new(socket_path))
                .expect("Could not remove socket");
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

            if !ignore_reads {
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
            }
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
        let line = self
            .reader_rx
            .recv()
            .await
            .expect("Could not receive message");
        serde_json::from_str(&line).expect("Could not parse JSON")
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::types::{
        factories::*, EditorProtocolMessageFromEditor, EditorProtocolMessageToEditor,
        EditorTextDelta, EditorTextOp, RevisionedEditorTextDelta,
    };
    use pretty_assertions::assert_eq;
    use serial_test::serial;
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
            nvim.command("EthersyncInfo")
                .await
                .expect("Failed to run EthersyncInfo");
        });
    }

    #[test]
    #[ignore]
    fn ethersync_executable_from_vim() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let handler = Dummy::new();
            let mut cmd = tokio::process::Command::new("nvim");
            cmd.arg("--headless").arg("--embed");
            let (nvim, _, _) = new_child_cmd(&mut cmd, handler).await.unwrap();
            assert_eq!(
                nvim.command_output("echomsg executable('ethersync')")
                    .await
                    .expect("Failed to run executable() in Vim"),
                "1",
                "Failed to run ethersync executable from Vim"
            );
        });
    }

    fn assert_vim_deltas_yield_content(
        initial_content: &str,
        deltas: Vec<EditorTextOp>,
        expected_content: &str,
    ) {
        let runtime = Runtime::new().expect("Could not create Tokio runtime");
        runtime.block_on(async {
            let mut socket = MockSocket::new("/tmp/ethersync", true);
            let (nvim, file_path) = Neovim::new_ethersync_enabled(initial_content).await;

            for op in &deltas {
                let rev_editor_delta = RevisionedEditorTextDelta {
                    revision: 0,
                    delta: EditorTextDelta(vec![op.clone()]),
                };
                let editor_message = EditorProtocolMessageToEditor::Edit {
                    uri: format!("file://{}", file_path.display()),
                    delta: rev_editor_delta,
                };
                let payload = editor_message
                    .to_jsonrpc()
                    .expect("Could not serialize EditorTextDelta");
                socket.send(&format!("{payload}\n")).await;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;

            let actual_content = nvim.content().await;
            assert_eq!(
                expected_content,
                actual_content,
                "Different content when we start with content '{:?}' and apply deltas '{:?}'. Expected '{:?}', actual '{:?}'.",
                initial_content,
                deltas,
                expected_content,
                actual_content
            );
        });
    }

    #[test]
    #[ignore]
    #[serial]
    fn vim_processes_deltas_correctly() {
        assert_vim_deltas_yield_content("", vec![replace_ed((0, 0), (0, 0), "a")], "a");
        assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "")], "x");
        assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "y")], "xy");
        assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "\n")], "x\n");
        assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "\n\n")], "x\n\n");
        assert_vim_deltas_yield_content(
            "x\n123\nz",
            vec![replace_ed((1, 1), (2, 1), "y")],
            "x\n1y",
        );
        assert_vim_deltas_yield_content("x", vec![replace_ed((0, 1), (0, 1), "\n")], "x\n");

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
        mut expected_replacements: Vec<EditorTextOp>,
    ) {
        let runtime = Runtime::new().expect("Could not create Tokio runtime");
        runtime.block_on(async {
            timeout(Duration::from_millis(5000), async {
                let mut socket = MockSocket::new("/tmp/ethersync", false);
                let (mut nvim, _file_path) = Neovim::new_ethersync_enabled(initial_content).await;

                let msg = socket.recv().await;
                assert_eq!(msg["method"], "open");

                let input_clone = input.to_string();
                tokio::spawn(async move {
                    nvim.input(&input_clone).await;
                });

                // Note: This doesn't check whether there are more replacements pending than the
                // expected ones.
                while !expected_replacements.is_empty() {
                    let msg = socket.recv().await;
                    let message: EditorProtocolMessageFromEditor = serde_json::from_str(&msg.to_string())
                        .expect("Could not parse EditorProtocolMessage");
                    if let EditorProtocolMessageFromEditor::Edit{ delta, ..} = message {
                        let expected_replacement = expected_replacements.remove(0);
                        let operations = delta.delta.0;
                        assert_eq!(vec![expected_replacement], operations, "Different replacements when applying input '{}' to content '{:?}'", input, initial_content);
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
    #[serial]
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
        // 'eol' will still be on, so let's keep the newline.
        assert_vim_input_yields_replacements("a\n", "dd", vec![replace_ed((0, 0), (0, 1), "")]);
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
                replace_ed((0, 0), (0, 1), ""),
                replace_ed((0, 0), (0, 0), "x"),  // d: "x\n"
                replace_ed((0, 1), (0, 1), "\n"), // d: "x\n\n"
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

        assert_vim_input_yields_replacements(
            "a\n",
            "ddo",
            vec![
                replace_ed((0, 0), (0, 1), ""), // 'eol' is still on, so we keep the newline.
                replace_ed((0, 0), (0, 0), "\n"),
            ],
        );

        assert_vim_input_yields_replacements("a\n", "o", vec![replace_ed((0, 1), (0, 1), "\n")]);

        // Unicode tests
        assert_vim_input_yields_replacements("ä\nü\n", "dd", vec![replace_ed((0, 0), (1, 0), "")]);
        assert_vim_input_yields_replacements("ä💚🥕", "vlld", vec![replace_ed((0, 0), (0, 3), "")]);
        assert_vim_input_yields_replacements("ä", "dd", vec![replace_ed((0, 0), (0, 1), "")]);

        assert_vim_input_yields_replacements("a\n", "yyp", vec![replace_ed((0, 1), (0, 1), "\na")]);
        assert_vim_input_yields_replacements(
            "🥕\n",
            "yyp",
            vec![replace_ed((0, 1), (0, 1), "\n🥕")],
        );
        assert_vim_input_yields_replacements("a", "yyp", vec![replace_ed((0, 1), (0, 1), "\na")]);

        assert_vim_input_yields_replacements(
            "a\n🥕\n",
            "jyyp",
            vec![replace_ed((1, 1), (1, 1), "\n🥕")],
        );

        assert_vim_input_yields_replacements("a", "o", vec![replace_ed((0, 1), (0, 1), "\n")]);

        assert_vim_input_yields_replacements(
            "eins\ntwo",
            "jo",
            vec![replace_ed((1, 3), (1, 3), "\n")],
        );

        assert_vim_input_yields_replacements(
            "eins\ntwo\n",
            "jo",
            vec![replace_ed((1, 3), (1, 3), "\n")],
        );

        assert_vim_input_yields_replacements(
            "eins\ntwo\nthree",
            "jo",
            vec![replace_ed((1, 3), (1, 3), "\n")],
        );

        // Tests where Vim behaves a bit weirdly.

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

        // Visual on multiple lines
        assert_vim_input_yields_replacements(
            "abc\nde\nf\n",
            "jVjd",
            vec![replace_ed((0, 3), (2, 1), "")],
        );
    }
}
