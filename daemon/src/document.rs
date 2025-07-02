// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    path::RelativePath,
    types::{EditorTextDelta, TextDelta},
};
use anyhow::Result;
use automerge::{
    patches::TextRepresentation,
    sync::{Message as AutomergeSyncMessage, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, Patch, PatchLog, ReadDoc, TextEncoding, ROOT,
};
use dissimilar::Chunk;
use tracing::{debug, info};

/// Encapsulates the Automerge `AutoCommit` and provides a generic interface,
/// s.t. we don't need to worry about automerge internals elsewhere.
///
/// The interface allows us to apply changes to the CRDT in two ways:
/// - synchronization with other CRDTs through sync messages
/// - applying a delta (coming from an editor) directly
///
/// Furthermore there's a way to retrieve and initialize the content.
#[derive(Debug)]
pub struct Document {
    doc: AutoCommit,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            doc: AutoCommit::default(),
        }
    }
}

impl Document {
    pub fn load(bytes: &[u8]) -> Self {
        let doc =
            AutoCommit::load(bytes).expect("Failed to load Automerge document from given bytes");
        Self { doc }
    }

    pub fn save(&mut self) -> Vec<u8> {
        self.doc.save()
    }

    pub fn save_incremental(&mut self) -> Vec<u8> {
        self.doc.save_incremental()
    }

    pub fn actor_id(&self) -> String {
        self.doc.get_actor().to_hex_string()
    }

    pub fn receive_sync_message_log_patches(
        &mut self,
        message: AutomergeSyncMessage,
        peer_state: &mut SyncState,
    ) -> Vec<Patch> {
        let mut patch_log = PatchLog::active(TextRepresentation::String(TextEncoding::default()));
        self.doc
            .sync()
            .receive_sync_message_log_patches(peer_state, message, &mut patch_log)
            .expect("Failed to apply sync message to Automerge document");
        self.doc.make_patches(&mut patch_log)
    }

    pub fn generate_sync_message(
        &mut self,
        peer_state: &mut SyncState,
    ) -> Option<AutomergeSyncMessage> {
        self.doc.sync().generate_sync_message(peer_state)
    }

    pub fn apply_delta_to_doc(&mut self, delta: &TextDelta, file_path: &RelativePath) {
        let text_obj = self
            .text_obj(file_path)
            .expect("Couldn't get automerge text object, so not able to modify it");
        let mut offset = 0isize;
        let text = self
            .current_file_content(file_path)
            .expect("Should have initialized text before applying delta to it");
        let ed_delta = EditorTextDelta::from_delta(delta.clone(), &text);

        for op in &ed_delta.0 {
            let (start, length) = op.range.as_relative(&text);
            self.doc
                .splice_text(
                    text_obj.clone(),
                    ((start as isize) + offset) as usize,
                    length as isize,
                    &op.replacement,
                )
                .expect("Failed to splice Automerge text object");
            offset -= length as isize;
            offset += op.replacement.chars().count() as isize;
        }
    }

    pub fn current_file_content(&self, file_path: &RelativePath) -> Result<String> {
        self.text_obj(file_path).map(|to| {
            self.doc
                .text(to)
                .expect("Failed to get string from Automerge text object")
        })
    }

    pub fn initialize_text(&mut self, text: &str, file_path: &RelativePath) {
        info!("Initializing {file_path} in CRDT.");

        let text_obj = self
            .doc
            .put_object(&ROOT, file_path, ObjType::Text)
            .expect("Failed to initialize text object in Automerge document");
        self.doc
            .splice_text(text_obj, 0, 0, text)
            .expect("Failed to splice text into Automerge text object");
    }

    // This function is used to integrate text that was changed while the daemon was offline.
    // We need to calculate the patches compared to the current CRDT content, and apply them.
    pub fn update_text(&mut self, desired_text: &str, file_path: &RelativePath) {
        if self.text_obj(file_path).is_ok() {
            let current_text = self
                .current_file_content(file_path)
                .unwrap_or_else(|_| panic!("Failed to get {file_path} text object"));

            let chunks = dissimilar::diff(&current_text, desired_text);
            if let [] | [Chunk::Equal(_)] = chunks.as_slice() {
                return;
            }

            let text_delta: TextDelta = chunks.into();
            info!("Updating {file_path} in CRDT with delta: {text_delta:?}");
            self.apply_delta_to_doc(&text_delta, file_path);
        } else {
            // The file doesn't exist in the CRDT yet, so we need to initialize it.
            self.initialize_text(desired_text, file_path);
        }
    }

    pub fn remove_text(&mut self, file_path: &RelativePath) {
        if self.text_obj(file_path).is_err() {
            debug!("Failed to get {file_path} Text object, so I can't remove it from the CRDT.");
            return;
        };

        info!("Removing {file_path} from CRDT.");
        // TODO: Also remove it from ot server, if applicable
        self.doc
            .delete(&ROOT, file_path)
            .expect("Failed to delete text object");
    }

    fn text_obj(&self, file_path: &RelativePath) -> Result<automerge::ObjId> {
        let text_obj = self
            .doc
            .get(&ROOT, file_path)
            .unwrap_or_else(|_| panic!("Failed to get {file_path} key from Automerge document"));
        if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
            Ok(text_obj)
        } else {
            Err(anyhow::anyhow!(
                "Automerge document doesn't have a {file_path} Text object, so I can't provide it"
            ))
        }
    }

    pub fn files(&self) -> Vec<RelativePath> {
        self.doc
            .keys(&ROOT)
            .map(|k| RelativePath::new(&k))
            .collect()
    }

    pub fn file_exists(&self, file_path: &RelativePath) -> bool {
        self.text_obj(file_path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::factories::*;

    impl Document {
        fn assert_file_content(&self, file_path: &RelativePath, content: &str) {
            // unfortunately anyhow::Error doesn't implement PartialEq, so we'll rather unwrap.
            assert_eq!(self.current_file_content(file_path).unwrap(), content);
        }
    }

    #[test]
    fn can_initialize_content() {
        let mut document = Document::default();
        let text = "To be or not to be, that is the question";
        let file = RelativePath::new("text");

        document.initialize_text(text, &file);

        document.assert_file_content(&file, text);
    }

    #[test]
    fn can_initialize_content_multifile() {
        let mut document = Document::default();

        let text = "To be or not to be, that is the question";
        let text2 = "2b||!2b, that is the question";

        let file1 = RelativePath::new("text");
        let file2 = RelativePath::new("text2");

        document.initialize_text(text, &file1);
        document.initialize_text(text2, &file2);

        document.assert_file_content(&file1, text);
        document.assert_file_content(&file2, text2);
    }

    #[test]
    fn retrieve_content_file_nonexistent_errs() {
        let document = Document::default();
        document
            .current_file_content(&RelativePath::new("text"))
            .expect_err("File shouldn't exist");
    }

    fn apply_delta_to_doc_works(initial: &str, delta: &TextDelta, expected: &str) {
        let mut document = Document::default();
        let file = RelativePath::new("text");

        document.initialize_text(initial, &file);
        document.apply_delta_to_doc(delta, &file);

        document.assert_file_content(&file, expected);
    }

    #[test]
    fn can_apply_delta_basic_insertion() {
        let delta = insert(0, "foobar");
        apply_delta_to_doc_works("", &delta, "foobar");
    }

    #[test]
    fn can_apply_delta_basic_deletion() {
        let delta = delete(3, 3);
        apply_delta_to_doc_works("foobar", &delta, "foo");
    }

    #[test]
    fn can_apply_delta_basic_replacement() {
        let delta = replace(1, 2, "uu");
        apply_delta_to_doc_works("foobar", &delta, "fuubar");
    }

    #[test]
    fn can_apply_delta_multiple_ops() {
        let initial_text = "To be or not to be, that is the question";

        let mut delta = insert(3, "m");
        delta.delete(1); // "b"
        delta.retain(5); // "e or "
        delta.delete(4); // "not "
        delta.retain(3); // "to "
        delta.delete(2); // "be"
        delta.insert("you");

        apply_delta_to_doc_works(
            initial_text,
            &delta,
            "To me or to you, that is the question",
        );
    }

    #[test]
    fn apply_delta_only_changes_specified_file() {
        let mut document = Document::default();

        let file1 = RelativePath::new("text");
        let file2 = RelativePath::new("text2");

        document.initialize_text("", &file1);
        document.initialize_text("", &file2);

        let delta = insert(0, "foobar");
        document.apply_delta_to_doc(&delta, &file1);

        document.assert_file_content(&file1, "foobar");
        document.assert_file_content(&file2, "");
    }

    /// This set of tests has some documentation character to show to ourselves,
    /// what happens under the hood when sync'ing.
    mod automerge_interna {
        use super::*;

        #[test]
        fn test_generate_sync_message() {
            let mut document = Document::default();
            let mut state = SyncState::new();
            assert!(document.generate_sync_message(&mut state).is_some());
            // Stops for now and waits for a response
            assert!(document.generate_sync_message(&mut state).is_none());

            document.initialize_text("", &RelativePath::new("text"));
            // We have progressed our state, so update all peers about that.
            assert!(document.generate_sync_message(&mut state).is_some());
            // Again, stop, until peers tell us if they want more information.
            assert!(document.generate_sync_message(&mut state).is_none());
        }
    }
}
