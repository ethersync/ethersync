// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{sandbox,config, document::Document, path::AbsolutePath};
use automerge::ChangeHash;
use std::path::Path;
use anyhow::Result;

fn load_doc(directory: &Path) -> Result<Document> {
    let doc_path = directory
        .join(config::CONFIG_DIR)
        .join(config::DOC_FILE);

    let bytes = sandbox::read_file(&directory, &doc_path)?;
    Ok(Document::load(&bytes))
}

pub fn seenit(directory: &Path) -> Result<()> {
    let mut doc = load_doc(directory)?;

    let bookmark_path = directory
        .join(config::CONFIG_DIR)
        .join(config::BOOKMARK_FILE);

    let heads = doc.get_heads();
    let json = serde_json::to_string(&heads)?;
    let bytes = json.as_bytes();
    sandbox::write_file(&directory, &bookmark_path, &bytes)?;

    Ok(())
}

pub fn snapshot(directory: &Path, target_directory: &Path, seenit: bool) -> Result<()> {
    let mut doc = load_doc(directory)?;

    let bookmark_path = directory
        .join(config::CONFIG_DIR)
        .join(config::BOOKMARK_FILE);

    let heads: Vec<ChangeHash> = if seenit {
        let bytes=  sandbox::read_file(&directory, &bookmark_path)?;
        let json = String::from_utf8(bytes)?;
        serde_json::from_str(&json)?
    } else {
        doc.get_heads()
    };

    for relative_file_path in &doc.files_at(&heads) {
        let content = doc.file_content_at(relative_file_path, &heads)?;
        let bytes = content.as_bytes();
        let absolute_path = AbsolutePath::from_parts(target_directory, relative_file_path)?;
        sandbox::write_file(target_directory, &absolute_path, bytes)?;
    }

    Ok(())
}