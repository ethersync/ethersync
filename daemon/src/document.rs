use crate::types::EditorTextDelta;
use crate::types::{CursorState, Range};
use anyhow::Result;
use automerge::{
    patches::TextRepresentation,
    sync::{Message as AutomergeSyncMessage, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, Patch, PatchLog, ReadDoc,
};
use tracing::{debug, info};

/// Encapsulates the Automerge `AutoCommit` and provides a generic interface,
/// s.t. we don't need to worry about automerge internals elsewhere.
///
/// The interface allows us to apply changes to the CRDT in two ways:
/// - synchronization with other CRDTs through sync messages
/// - applying a delta (coming from an editor) directly
///
/// Furthermore there's a way to retrieve and initialize the content.
#[derive(Debug, Default)]
pub struct Document {
    doc: AutoCommit,
}

impl Document {
    pub fn actor_id(&self) -> String {
        self.doc.get_actor().to_hex_string()
    }

    pub fn receive_sync_message_log_patches(
        &mut self,
        message: AutomergeSyncMessage,
        peer_state: &mut SyncState,
    ) -> Vec<Patch> {
        let mut patch_log = PatchLog::active(TextRepresentation::String);
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

    pub fn apply_delta_to_doc(&mut self, delta: &EditorTextDelta, file_path: &str) {
        let text_obj = self
            .text_obj(file_path)
            .expect("Couldn't get automerge text object, so not able to modify it");
        let mut offset = 0i32;
        let text = self
            .current_file_content(file_path)
            .expect("Should have initialized text before applying delta to it");
        for op in &delta.0 {
            let (start, length) = op.range.as_relative(&text);
            self.doc
                .splice_text(
                    text_obj.clone(),
                    (start as i32 + offset) as usize,
                    length as isize,
                    &op.replacement,
                )
                .expect("Failed to splice Automerge text object");
            offset -= length as i32;
            offset += op.replacement.chars().count() as i32;
        }
    }

    pub fn current_file_content(&self, file_path: &str) -> Result<String> {
        self.text_obj(file_path).map(|to| {
            self.doc
                .text(to)
                .expect("Failed to get string from Automerge text object")
        })
    }

    fn initialize_top_level_maps(&mut self) {
        self.doc
            .put_object(automerge::ROOT, "files", ObjType::Map)
            .expect("Failed to initialize files Map object");
        self.doc
            .put_object(automerge::ROOT, "states", ObjType::Map)
            .expect("Failed to initialize states Map object");
    }

    pub fn initialize_text(&mut self, text: &str, file_path: &str) {
        info!("Initializing {file_path} in CRDT");
        if self.text_obj(file_path).is_ok() {
            // While automerge accepts putting an object multiple times, in our current
            // architecture this should not happen: Only the host should initialize every file
            // once, while peers just take in whatever already exists.
            //
            // This might change in a future more peer to peer world.
            panic!("It seems {file_path} was already initialized.");
        }

        // TODO: I don't love the assumption that the first document to initialize a text
        // object should initialize the maps...
        if self.top_level_map_obj("files").is_err() {
            self.initialize_top_level_maps();
        }

        // Now it should definitely work?
        let file_map = self
            .top_level_map_obj("files")
            .expect("Failed to get files Map object");

        let text_obj = self
            .doc
            .put_object(file_map, file_path, ObjType::Text)
            .expect("Failed to initialize text object in Automerge document");
        self.doc
            .splice_text(text_obj, 0, 0, text)
            .expect("Failed to splice text into Automerge text object");
    }

    fn top_level_map_obj(&self, name: &str) -> Result<automerge::ObjId> {
        let file_map = self.doc.get(automerge::ROOT, name);
        if let Ok(Some((automerge::Value::Object(ObjType::Map), file_map))) = file_map {
            Ok(file_map)
        } else {
            Err(anyhow::anyhow!(
                "Automerge document doesn't have a {name} Map object"
            ))
        }
    }

    fn text_obj(&self, file_path: &str) -> Result<automerge::ObjId> {
        let file_map = self.top_level_map_obj("files")?;
        let text_obj = self
            .doc
            .get(file_map, file_path)
            .unwrap_or_else(|_| panic!("Failed to get {file_path} key from Automerge document"));
        if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
            Ok(text_obj)
        } else {
            Err(anyhow::anyhow!(
                "Automerge document doesn't have a {file_path} Text object, so I can't provide it"
            ))
        }
    }

    pub fn store_cursor_position(&mut self, userid: String, file_path: String, ranges: Vec<Range>) {
        let state_map = self
            .top_level_map_obj("states")
            .expect("Failed to get states Map object");
        let user_obj = self
            .doc
            .put_object(state_map, &userid, ObjType::Text)
            .expect("Failed to initialize user state Map object in Automerge document");
        let cursor_state = CursorState {
            userid: userid.clone(),
            file_path,
            ranges,
        };
        let data = serde_json::to_string(&cursor_state).expect("Could not serialize cursor state");
        self.doc
            .splice_text(user_obj, 0, 0, &data)
            .expect("Failed to splice text into Automerge text object");
        debug!("Stored user state for '{userid}': {data}");
    }

    pub fn maybe_delete_cursor_position(&mut self, userid: String) {
        // We try to set an empty cursor position, but in case we don't have any file in the
        // project its not a big deal if it stays.
        if let Some(file_path) = self.get_valid_file_path() {
            self.store_cursor_position(userid, file_path, vec![])
        }
    }

    fn get_valid_file_path(&self) -> Option<String> {
        let file_map = self.top_level_map_obj("files");
        if let Ok(file_map) = file_map {
            self.doc.keys(file_map).next()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::factories::*;

    impl Document {
        fn assert_file_content(&self, file_path: &str, content: &str) {
            // unfortunately anyhow::Error doesn't implement PartialEq, so we'll rather unwrap.
            assert_eq!(self.current_file_content(file_path).unwrap(), content);
        }
    }

    #[test]
    fn can_initialize_content() {
        let mut document = Document::default();
        let text = "To be or not to be, that is the question";

        document.initialize_text(text, "text");

        document.assert_file_content("text", text);
    }

    #[test]
    #[should_panic]
    fn cannot_initialize_content_same_file_twice() {
        let mut document = Document::default();
        let text = "To be or not to be, that is the question";

        document.initialize_text(text, "text");
        // This call should fail.
        document.initialize_text(text, "text");
    }

    #[test]
    fn can_initialize_content_multifile() {
        let mut document = Document::default();
        let text = "To be or not to be, that is the question";
        let text2 = "2b||!2b, that is the question";

        document.initialize_text(text, "text");
        document.initialize_text(text2, "text2");

        document.assert_file_content("text", text);
        document.assert_file_content("text2", text2);
    }

    #[test]
    fn retrieve_content_file_nonexistent_errs() {
        let document = Document::default();
        document
            .current_file_content("text")
            .expect_err("File shouldn't exist");
    }

    fn apply_delta_to_doc_works(initial: &str, ed_delta: &EditorTextDelta, expected: &str) {
        let mut document = Document::default();
        document.initialize_text(initial, "text");
        document.apply_delta_to_doc(ed_delta, "text");

        document.assert_file_content("text", expected);
    }

    #[test]
    fn can_apply_delta_basic_insertion() {
        let ed_delta = ed_delta_single((0, 0), (0, 0), "foobar");
        apply_delta_to_doc_works("", &ed_delta, "foobar");
    }

    #[test]
    fn can_apply_delta_basic_deletion() {
        let ed_delta = ed_delta_single((0, 3), (0, 6), "");
        apply_delta_to_doc_works("foobar", &ed_delta, "foo");
    }

    #[test]
    fn can_apply_delta_basic_replacement() {
        let ed_delta = ed_delta_single((0, 1), (0, 3), "uu");
        apply_delta_to_doc_works("foobar", &ed_delta, "fuubar");
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
            &EditorTextDelta::from_delta(delta, initial_text),
            "To me or to you, that is the question",
        );
    }

    #[test]
    fn can_apply_delta_multiple_ops_bug() {
        let content = "xeins\nzwei\ndrei\n";

        let ed_delta = EditorTextDelta(vec![
            replace_ed((1, 0), (1, 0), "xzwei\nx"),
            replace_ed((1, 0), (2, 0), ""),
        ]);

        apply_delta_to_doc_works(content, &ed_delta, "xeins\nxzwei\nxdrei\n");
    }

    #[test]
    fn apply_delta_only_changes_specified_file() {
        let mut document = Document::default();
        document.initialize_text("", "text");
        document.initialize_text("", "text2");

        let ed_delta = ed_delta_single((0, 0), (0, 0), "foobar");
        document.apply_delta_to_doc(&ed_delta, "text");

        document.assert_file_content("text", "foobar");
        document.assert_file_content("text2", "");
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

            document.initialize_text("", "text");
            // We have progressed our state, so update all peers about that.
            assert!(document.generate_sync_message(&mut state).is_some());
            // Again, stop, until peers tell us if they want more information.
            assert!(document.generate_sync_message(&mut state).is_none());
        }

        fn patches_when_syncing_with_peer(mut host: Document) -> Vec<Patch> {
            let mut peer = Document::default();
            let mut peer_state = SyncState::new();
            let mut host_state = SyncState::new();

            let mut patches = vec![];
            while let Some(message) = host.generate_sync_message(&mut peer_state) {
                // This is assuming the interesting patch to test is the *last* one.
                patches = peer.receive_sync_message_log_patches(message, &mut host_state);
                if let Some(response) = peer.generate_sync_message(&mut host_state) {
                    host.receive_sync_message_log_patches(response, &mut peer_state);
                }
            }
            patches
        }
    }
}
