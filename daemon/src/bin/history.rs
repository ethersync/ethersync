// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

#![allow(unused, dead_code)]

use automerge::{
    iter::Keys,
    patches::TextRepresentation,
    sync::{Message, State as SyncState, SyncDoc},
    transaction::Transactable,
    ActorId, AutoCommit, Change, ChangeHash, ObjType, Patch, PatchLog, ReadDoc, Value,
};
use ethersync::types::PatchEffect;
use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    ethersync_file_history(&args[1]);
}

fn ethersync_file_history(doc_path: &str) {
    let bytes = fs::read(doc_path).unwrap();
    let mut doc = AutoCommit::load(&bytes).unwrap();

    //dbg!(&doc.keys(automerge::ROOT).collect::<Vec<_>>());

    let file_map = doc.get(automerge::ROOT, "files").unwrap().unwrap().1;

    //dbg!(&doc.keys(&file_map).collect::<Vec<_>>());
    let heads = doc.get_heads();
    let mut current_head = heads[0];
    let mut snapshot_head = current_head;
    let mut previous_actor_id: Option<ActorId> = None;
    loop {
        let change = doc.get_change_by_hash(&current_head).unwrap().clone();
        let actor_id = change.actor_id().clone();
        let parents = change.deps();
        if parents.len() > 1 {
            dbg!(current_head);
        }
        // TODO: Can we prevent this clone somehow?
        if let Some(pa) = previous_actor_id.clone() {
            if pa != actor_id {
                let patches = doc.diff(parents, &[snapshot_head]);
                println!("* {}", summarize_diff(patches));
                snapshot_head = parents[0];
                previous_actor_id = Some(actor_id);
                return;
            }
        } else {
            previous_actor_id = Some(actor_id.clone());
        }
        current_head = parents[0];
    }

    /*
    dbg!(&heads);
    let changes = doc.get_changes(&[]);
    dbg!(changes.len());
    let mut hashes = vec![];
    let mut i = 0;
    for change in &changes {
        let h = change.hash();
        if i > 10 && i < 100 {
            dbg!(&change.decode());
        }
        i += 1;

        if i >= 100 {
            return;
        }
        // dbg!(&change.hash());
        hashes.push(h);
    }
    let mut number_of_documents: usize = 0;
    let mut prev_keys: HashSet<String> = HashSet::new();

    for h in hashes {
        let keys = doc.keys_at(&file_map, &[h]);
        let c = keys.count();
        if c != number_of_documents {
            number_of_documents = c;
            let keys_new = doc.keys_at(&file_map, &[h]).collect::<HashSet<_>>();
            let added: Vec<_> = keys_new.difference(&prev_keys).collect();
            if !added.is_empty() {
                println!("Newly added documents in {h}:");
                dbg!(added);
            }
            let removed: Vec<_> = prev_keys.difference(&keys_new).collect();
            if !removed.is_empty() {
                println!("Removed documents in {h}:");
                dbg!(removed);
            }
            prev_keys = keys_new;
        }
    }
    */
}

/*
- Anzahl der Ops pro Typ Op
*/
fn summarize_diff(patches: Vec<Patch>) -> String {
    let effects = PatchEffect::from_crdt_patches(patches);

    match effects.len() {
        0 => "zero effects ???".into(),
        1 => {
            // diplay nicely
            //dbg!(&effects[0]);
            summarize_effect(&effects[0])
        }
        _ => {
            // count types
            let mut s = format!("{} effects", effects.len());
            for effect in effects {
                s += format!("   {}\n\n\n\n", summarize_effect(&effect)).as_str();
            }
            s
        }
    }
}

fn summarize_effect(patch_effect: &PatchEffect) -> String {
    match patch_effect {
        PatchEffect::FileChange(file_text_delta) => {
            format!(
                "file change in {}: {}",
                file_text_delta.file_path, file_text_delta.delta
            )
        }
        PatchEffect::FileRemoval(relative_path) => {
            format!("file removed: {relative_path}")
        }
        _ => "binary or no effect".into(),
    }
}
