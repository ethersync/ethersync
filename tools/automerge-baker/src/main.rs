// SPDX-FileCopyrightText: NONE
//
// SPDX-License-Identifier: CC0-1.0

//! A helper tool to pre-bake the initial state of the Automerge document.
//! See https://automerge.org/docs/cookbook/modeling-data/#setting-up-an-initial-document-structure

use automerge::{transaction::Transactable, AutoCommit, ObjType};

fn main() {
    let mut doc = AutoCommit::new();
    doc.put_object(automerge::ROOT, "files", ObjType::Map)
        .expect("Failed to initialize files Map object");
    doc.put_object(automerge::ROOT, "states", ObjType::Map)
        .expect("Failed to initialize states Map object");
    println!("{:?}", doc.save());
}
