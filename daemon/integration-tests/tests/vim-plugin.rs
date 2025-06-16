// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use ethersync_integration_tests::actors::*;
use ethersync_integration_tests::socket::*;

use ethersync::types::{
    factories::*, EditorProtocolMessageFromEditor, EditorProtocolMessageToEditor,
    EditorProtocolObject, EditorTextDelta, EditorTextOp, JSONRPCFromEditor,
};

use pretty_assertions::assert_eq;
use serial_test::serial;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn plugin_loaded() {
    let handler = Dummy::new();
    let mut cmd = tokio::process::Command::new("nvim");
    cmd.arg("--headless").arg("--embed");
    let (nvim, _, _) = new_child_cmd(&mut cmd, handler).await.unwrap();
    nvim.command("EthersyncInfo")
        .await
        .expect("Failed to run EthersyncInfo");
}

#[tokio::test]
async fn ethersync_executable_from_vim() {
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
}

async fn assert_vim_deltas_yield_content(
    initial_content: &str,
    deltas: Vec<EditorTextOp>,
    expected_content: &str,
) {
    let (nvim, file_path, socket_path, _temp_dir) =
        Neovim::new_ethersync_enabled(initial_content).await;
    let mut socket = MockSocket::new(&socket_path);
    socket.acknowledge_open().await;

    for op in &deltas {
        let editor_message = EditorProtocolMessageToEditor::Edit {
            uri: format!("file://{}", file_path.display()),
            revision: 0,
            delta: EditorTextDelta(vec![op.clone()]),
        };
        let payload = EditorProtocolObject::Request(editor_message)
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
}

#[tokio::test]
#[serial]
async fn vim_processes_deltas_correctly() {
    assert_vim_deltas_yield_content("", vec![replace_ed((0, 0), (0, 0), "a")], "a").await;
    assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "")], "x").await;
    assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "y")], "xy").await;
    assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "\n")], "x\n").await;
    assert_vim_deltas_yield_content("x\n", vec![replace_ed((0, 1), (1, 0), "\n\n")], "x\n\n").await;
    assert_vim_deltas_yield_content("x\n123\nz", vec![replace_ed((1, 1), (2, 1), "y")], "x\n1y")
        .await;
    assert_vim_deltas_yield_content("x", vec![replace_ed((0, 1), (0, 1), "\n")], "x\n").await;

    assert_vim_deltas_yield_content(
        "bananas",
        vec![
            replace_ed((0, 2), (0, 3), ""),
            replace_ed((0, 3), (0, 4), ""),
        ],
        "baaas",
    )
    .await;

    assert_vim_deltas_yield_content("ba\nna\nnas", vec![replace_ed((0, 1), (2, 1), "")], "bas")
        .await;

    assert_vim_deltas_yield_content(
        "hi\n",
        vec![replace_ed((1, 0), (1, 0), "there\n")],
        "hi\nthere\n",
    )
    .await;

    assert_vim_deltas_yield_content(
        "hi\n",
        vec![replace_ed((1, 0), (1, 0), "there\n\n")],
        "hi\nthere\n\n",
    )
    .await;

    assert_vim_deltas_yield_content("hi\n", vec![replace_ed((1, 0), (1, 0), "\n")], "hi\n\n").await;
}

async fn assert_vim_input_yields_replacements(
    initial_content: &str,
    input: &str,
    mut expected_replacements: Vec<EditorTextOp>,
) {
    timeout(Duration::from_millis(5000), async {
                let (mut nvim, _file_path, socket_path, _temp_dir) = Neovim::new_ethersync_enabled(initial_content).await;
                let mut socket = MockSocket::new(&socket_path);
                socket.acknowledge_open().await;

                {
                    let input = input.to_string();
                    tokio::spawn(async move {
                        nvim.input(&input).await;
                    });
                }

                // Note: This doesn't check whether there are more replacements pending than the
                // expected ones.
                while !expected_replacements.is_empty() {
                    let msg = socket.recv().await;
                    let message: JSONRPCFromEditor = serde_json::from_str(&msg.to_string())
                        .expect("Could not parse EditorProtocolMessage");
                    let JSONRPCFromEditor::Request{
                        payload: EditorProtocolMessageFromEditor::Edit{ delta, ..},
                        ..
                    } = message else {continue;};
                    let expected_replacement = expected_replacements.remove(0);
                    let operations = delta.0;
                    assert_eq!(vec![expected_replacement], operations, "Different replacements when applying input '{}' to content '{:?}'", input, initial_content);
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Nvim test for input '{input}' on '{initial_content:?}' timed out. Maybe increase timeout to make sure vim started fast enough. We probably received too few messages?"
                )
            });
}

#[tokio::test]
#[serial]
async fn vim_sends_correct_delta() {
    // Edits on a single line.
    assert_vim_input_yields_replacements("", "ia", vec![replace_ed((0, 0), (0, 0), "a")]).await;
    assert_vim_input_yields_replacements("a\n", "x", vec![replace_ed((0, 0), (0, 1), "")]).await;
    assert_vim_input_yields_replacements("abc\n", "lx", vec![replace_ed((0, 1), (0, 2), "")]).await;
    assert_vim_input_yields_replacements("abc\n", "vd", vec![replace_ed((0, 0), (0, 1), "")]).await;
    assert_vim_input_yields_replacements("abc\n", "vlld", vec![replace_ed((0, 0), (0, 3), "")])
        .await;
    assert_vim_input_yields_replacements("a\n", "rb", vec![replace_ed((0, 0), (0, 1), "b")]).await;
    // To add to end of line, the existence of a newline should not matter.
    assert_vim_input_yields_replacements("a", "Ab", vec![replace_ed((0, 1), (0, 1), "b")]).await;
    assert_vim_input_yields_replacements("a\n", "Ab", vec![replace_ed((0, 1), (0, 1), "b")]).await;
    assert_vim_input_yields_replacements("a\n", "Ib", vec![replace_ed((0, 0), (0, 0), "b")]).await;

    // Edits involving multiple lines.
    assert_vim_input_yields_replacements("a\n", "O", vec![replace_ed((0, 0), (0, 0), "\n")]).await;
    // Indentation matters.
    assert_vim_input_yields_replacements(
        "    a\n",
        "O",
        vec![replace_ed((0, 0), (0, 0), "    \n")],
    )
    .await;
    assert_vim_input_yields_replacements("a\nb\n", "dd", vec![replace_ed((0, 0), (1, 0), "")])
        .await;
    assert_vim_input_yields_replacements("a\nb\n", "jdd", vec![replace_ed((0, 1), (1, 1), "")])
        .await;
    // Also works without \n at the end.
    assert_vim_input_yields_replacements("a\nb", "jdd", vec![replace_ed((0, 1), (1, 1), "")]).await;
    // 'eol' will still be on, so let's keep the newline.
    assert_vim_input_yields_replacements("a\n", "dd", vec![replace_ed((0, 0), (0, 1), "")]).await;
    // Our design goal: produce something, that works without any implict newlines.
    assert_vim_input_yields_replacements("a", "dd", vec![replace_ed((0, 0), (0, 1), "")]).await;
    // Test what happens when we start with empty buffer:
    // The eol option can be "true" unexpectedly.
    assert_vim_input_yields_replacements(
        "",
        "ia<Esc>dd",
        vec![
            replace_ed((0, 0), (0, 0), "a"),
            replace_ed((0, 0), (0, 1), ""),
        ],
    )
    .await;

    assert_vim_input_yields_replacements("", "i<CR>", vec![replace_ed((0, 0), (0, 0), "\n")]).await;
    assert_vim_input_yields_replacements(
        "",
        "i<CR>i",
        vec![
            replace_ed((0, 0), (0, 0), "\n"),
            replace_ed((1, 0), (1, 0), "i"),
        ],
    )
    .await;
    assert_vim_input_yields_replacements(
        "",
        "ia<CR>a",
        vec![
            replace_ed((0, 0), (0, 0), "a"),
            replace_ed((0, 1), (0, 1), "\n"),
            replace_ed((1, 0), (1, 0), "a"),
        ],
    )
    .await;

    assert_vim_input_yields_replacements(
        "a\n",
        ":s/a/b<CR>",
        vec![replace_ed((0, 0), (0, 1), "b")],
    )
    .await;

    assert_vim_input_yields_replacements(
        "",
        "i<CR><BS>",
        vec![
            replace_ed((0, 0), (0, 0), "\n"),
            // no-op: Copy nothing to previous line.
            replace_ed((0, 0), (0, 0), ""),
            replace_ed((0, 0), (1, 0), ""),
        ],
    )
    .await;

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
    )
    .await;

    assert_vim_input_yields_replacements(
        "hello\nworld\n",
        "llvjd",
        vec![
            replace_ed((0, 2), (0, 5), ""), // d: llo
            replace_ed((1, 0), (1, 3), ""), // d: wor
            replace_ed((0, 2), (0, 2), "ld"),
            replace_ed((0, 4), (1, 2), ""), // d: \nld
        ],
    )
    .await;

    assert_vim_input_yields_replacements(
        "",
        "ox",
        vec![
            replace_ed((0, 0), (0, 0), "\n"),
            replace_ed((1, 0), (1, 0), "x"),
        ],
    )
    .await;

    assert_vim_input_yields_replacements(
        "a\n",
        "ddo",
        vec![
            replace_ed((0, 0), (0, 1), ""), // 'eol' is still on, so we keep the newline.
            replace_ed((0, 0), (0, 0), "\n"),
        ],
    )
    .await;

    assert_vim_input_yields_replacements("a\n", "o", vec![replace_ed((0, 1), (0, 1), "\n")]).await;

    // Unicode tests
    assert_vim_input_yields_replacements("ä\nü\n", "dd", vec![replace_ed((0, 0), (1, 0), "")])
        .await;
    assert_vim_input_yields_replacements("ä💚🥕", "vlld", vec![replace_ed((0, 0), (0, 3), "")])
        .await;
    assert_vim_input_yields_replacements("ä", "dd", vec![replace_ed((0, 0), (0, 1), "")]).await;

    assert_vim_input_yields_replacements("a\n", "yyp", vec![replace_ed((0, 1), (0, 1), "\na")])
        .await;
    assert_vim_input_yields_replacements("🥕\n", "yyp", vec![replace_ed((0, 1), (0, 1), "\n🥕")])
        .await;
    assert_vim_input_yields_replacements("a", "yyp", vec![replace_ed((0, 1), (0, 1), "\na")]).await;

    assert_vim_input_yields_replacements(
        "a\n🥕\n",
        "jyyp",
        vec![replace_ed((1, 1), (1, 1), "\n🥕")],
    )
    .await;

    assert_vim_input_yields_replacements("a", "o", vec![replace_ed((0, 1), (0, 1), "\n")]).await;

    assert_vim_input_yields_replacements("eins\ntwo", "jo", vec![replace_ed((1, 3), (1, 3), "\n")])
        .await;

    assert_vim_input_yields_replacements(
        "eins\ntwo\n",
        "jo",
        vec![replace_ed((1, 3), (1, 3), "\n")],
    )
    .await;

    assert_vim_input_yields_replacements(
        "eins\ntwo\nthree",
        "jo",
        vec![replace_ed((1, 3), (1, 3), "\n")],
    )
    .await;

    // Tests where Vim behaves a bit weirdly.

    // A direct replace_ed((0, 1), (1, 0), " ") would be nicer.
    assert_vim_input_yields_replacements(
        "a\nb\n",
        "J",
        vec![
            replace_ed((0, 1), (0, 1), " b"),
            replace_ed((0, 3), (1, 1), ""),
        ],
    )
    .await;

    assert_vim_input_yields_replacements(
        "a\nb",
        "J",
        vec![
            replace_ed((0, 1), (0, 1), " b"),
            replace_ed((0, 3), (1, 1), ""),
        ],
    )
    .await;

    // Visual on multiple lines
    assert_vim_input_yields_replacements(
        "abc\nde\nf\n",
        "jVjd",
        vec![replace_ed((0, 3), (2, 1), "")],
    )
    .await;
}
