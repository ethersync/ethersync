// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{sandbox,config};
use automerge::AutoCommit;
use std::path::Path;
use anyhow::Result;

pub fn seenit(directory: &Path) -> Result<()> {
    let doc_path = directory
        .join(config::CONFIG_DIR)
        .join(config::DOC_FILE);

    let bookmark_path = directory
        .join(config::CONFIG_DIR)
        .join(config::BOOKMARK_FILE);

    let bytes = sandbox::read_file(&directory, &doc_path)?;
    let mut doc = AutoCommit::load(&bytes)?;

    let heads = doc.get_heads();
    let json = serde_json::to_string(&heads)?;
    let bytes = json.as_bytes();
    sandbox::write_file(&directory, &bookmark_path, &bytes)?;

    Ok(())
}
