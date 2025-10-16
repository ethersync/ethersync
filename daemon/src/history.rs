// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{config, document::Document, path::AbsolutePath, sandbox};
use anyhow::{bail, Context, Result};
use automerge::ChangeHash;
use std::{
    io::{self, Write},
    path::Path,
    process::Command,
};
use temp_dir::TempDir;
use tracing::info;

fn load_doc(directory: &Path) -> Result<Document> {
    let doc_path = directory.join(config::CONFIG_DIR).join(config::DOC_FILE);

    let bytes = sandbox::read_file(directory, &doc_path).context(
        "Loading the CRDT document failed. \
        Are you in a pair-programming use case? Or was the daemon never started?",
    )?;

    Ok(Document::load(&bytes))
}

pub fn bookmark(directory: &Path) -> Result<()> {
    let mut doc = load_doc(directory)?;

    let bookmark_path = directory
        .join(config::CONFIG_DIR)
        .join(config::BOOKMARK_FILE);

    let heads = doc.get_heads();
    let json = serde_json::to_string(&heads)?;
    let bytes = json.as_bytes();
    sandbox::write_file(directory, &bookmark_path, bytes)?;

    Ok(())
}

fn read_bookmark(directory: &Path) -> Result<Vec<ChangeHash>> {
    let bookmark_path = directory
        .join(config::CONFIG_DIR)
        .join(config::BOOKMARK_FILE);

    let bytes = sandbox::read_file(directory, &bookmark_path)?;
    let json = String::from_utf8(bytes)?;
    Ok(serde_json::from_str(&json)?)
}

fn write_doc_contents_to_dir(
    doc: &Document,
    target_directory: &Path,
    heads: &[ChangeHash],
) -> Result<()> {
    for relative_file_path in &doc.files_at(heads) {
        let absolute_path = AbsolutePath::from_parts(target_directory, relative_file_path)?;

        if let Ok(content) = doc.file_content_at(relative_file_path, heads) {
            let bytes = content.as_bytes();
            sandbox::write_file(target_directory, &absolute_path, bytes)?;
        } else {
            let bytes = doc.get_bytes_at(relative_file_path, heads)?;
            sandbox::write_file(target_directory, &absolute_path, &bytes)?;
        }
    }
    Ok(())
}

pub fn diff(directory: &Path, invocation: &str) -> Result<()> {
    let mut doc = load_doc(directory)?;

    let current_heads = doc.get_heads();
    let bookmark_heads = read_bookmark(directory)?;

    let temp_dir = TempDir::new()?;
    let left_dir = temp_dir.child("left");
    let right_dir = temp_dir.child("right");

    write_doc_contents_to_dir(&doc, &left_dir, &bookmark_heads)?;
    write_doc_contents_to_dir(&doc, &right_dir, &current_heads)?;

    let mut invocation_split = invocation.split_whitespace();
    let Some(tool) = invocation_split.next() else {
        bail!("Tried to invoke diff with empty tool argument");
    };

    let mut args = invocation_split.collect::<Vec<_>>();

    let left_dir = left_dir.as_path().to_str().expect("");
    let right_dir = right_dir.as_path().to_str().expect("");
    args.extend_from_slice(&[left_dir, right_dir]);

    info!(
        "Running provided diff tool as follows:\n\t{} {}\n",
        &tool,
        &args.join(" ")
    );

    let output = Command::new(tool).args(args).output()?;
    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;

    Ok(())
}
