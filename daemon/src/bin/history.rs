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
    for _ in 0..10 {
        let change = doc.get_change_by_hash(&current_head).unwrap().clone();
        let parents = change.deps();
        let patches = doc.diff(parents, &[current_head]);
        println!("* {}", summarize_diff(patches));
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
            dbg!(&effects[0]);
            match &effects[0] {
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
        _ => {
            // count types
            format!("{} effects", effects.len())
        }
    }
}
