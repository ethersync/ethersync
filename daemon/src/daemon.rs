use crate::connect;
use crate::editor::EditorHandle;
use crate::ot::OTServer;
use crate::types::{
    EditorProtocolMessage, EditorTextDelta, FileTextDelta, RevisionedEditorTextDelta, TextDelta,
};
use anyhow::Result;
use automerge::{
    patches::TextRepresentation,
    sync::{Message as AutomergeSyncMessage, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, Patch, PatchLog, ReadDoc,
};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

pub const TEST_FILE_PATH: &str = "text";

// These messages are sent to the task that owns the document.
pub enum DocMessage {
    GetContent {
        response_tx: oneshot::Sender<Result<String>>,
    },
    FromEditor(EditorProtocolMessage),
    RandomEdit,
    ReceiveSyncMessage {
        message: AutomergeSyncMessage,
        state: SyncState,
        response_tx: oneshot::Sender<SyncState>,
    },
    GenerateSyncMessage {
        state: SyncState,
        response_tx: oneshot::Sender<(SyncState, Option<AutomergeSyncMessage>)>,
    },
    NewEditorConnection(EditorHandle),
}

impl fmt::Debug for DocMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            DocMessage::GetContent { .. } => "get content",
            DocMessage::FromEditor(_) => "open/close/edit/... message from editor",
            DocMessage::RandomEdit => "random edit",
            DocMessage::ReceiveSyncMessage { .. } => "<automerge internal sync rcv>",
            DocMessage::GenerateSyncMessage { .. } => "<automerge internal sync gen>",
            DocMessage::NewEditorConnection(_) => "editor connected",
        };
        write!(f, "{repr}")
    }
}

type DocMessageSender = mpsc::Sender<DocMessage>;
type DocChangedSender = broadcast::Sender<()>;
type DocChangedReceiver = broadcast::Receiver<()>;

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
    fn receive_sync_message_log_patches(
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

    fn generate_sync_message(
        &mut self,
        peer_state: &mut SyncState,
    ) -> Option<AutomergeSyncMessage> {
        self.doc.sync().generate_sync_message(peer_state)
    }

    fn apply_delta_to_doc(&mut self, delta: &EditorTextDelta, file_path: &str) {
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

    fn current_file_content(&self, file_path: &str) -> Result<String> {
        self.text_obj(file_path).map(|to| {
            self.doc
                .text(to)
                .expect("Failed to get string from Automerge text object")
        })
    }

    fn initialize_text(&mut self, text: &str, file_path: &str) {
        info!("Initializing {file_path} in CRDT");
        if self.text_obj(file_path).is_ok() {
            // While automerge accepts putting an object multiple times, in our current
            // architecture this should not happen: Only the host should initialize every file
            // once, while peers just take in whatever already exists.
            //
            // This might change in a future more peer to peer world.
            panic!("It seems {file_path} was already initialized.");
        }
        let text_obj = self
            .doc
            .put_object(automerge::ROOT, file_path, ObjType::Text)
            .expect("Failed to initialize text object in Automerge document");
        self.doc
            .splice_text(text_obj, 0, 0, text)
            .expect("Failed to splice text into Automerge text object");
    }

    fn text_obj(&self, file_path: &str) -> Result<automerge::ObjId> {
        let text_obj = self
            .doc
            .get(automerge::ROOT, file_path)
            .unwrap_or_else(|_| panic!("Failed to get {file_path} key from Automerge document"));
        if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
            Ok(text_obj)
        } else {
            Err(anyhow::anyhow!(
                "Automerge document doesn't have a {file_path} Text object, so I can't provide it"
            ))
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct EditorId(usize);

/// This Actor is responsible for applying changes to the document asynchronously.
///
/// Any `DocMessage` that is emitted via `DocumentActorHandle` should have an effect eventually.
pub struct DocumentActor {
    doc_message_rx: mpsc::Receiver<DocMessage>,
    doc_changed_ping_tx: DocChangedSender,
    editor_clients: HashMap<EditorId, EditorHandle>,
    /// If we have an ot_server for a given file, it means that editor has ownership.
    ot_servers: HashMap<String, OTServer>,
    /// The Document is the main I/O managed resource of this actor.
    crdt_doc: Document,
    base_dir: PathBuf,
}

impl DocumentActor {
    #[must_use]
    fn new(
        doc_message_rx: mpsc::Receiver<DocMessage>,
        doc_changed_ping_tx: DocChangedSender,
        base_dir: PathBuf,
    ) -> Self {
        Self {
            doc_message_rx,
            doc_changed_ping_tx,
            editor_clients: HashMap::default(),
            base_dir,
            ot_servers: HashMap::default(),
            crdt_doc: Document::default(),
        }
    }

    async fn handle_message(&mut self, message: DocMessage) {
        debug!("Handling doc message: {message:?}");
        match message {
            DocMessage::GetContent { response_tx } => {
                response_tx
                    .send(self.current_file_content(TEST_FILE_PATH))
                    .expect("Failed to send content to response channel");
            }
            DocMessage::RandomEdit => {
                let delta = self.random_delta();
                let text = self
                    .current_file_content(TEST_FILE_PATH)
                    .expect("Should have initialized text before performing random edit");
                let ed_delta = EditorTextDelta::from_delta(delta.clone(), &text);
                self.apply_delta_to_doc(&ed_delta, TEST_FILE_PATH);
                self.maybe_process_crdt_file_deltas_in_ot(vec![FileTextDelta::new(
                    TEST_FILE_PATH.to_string(),
                    delta,
                )])
                .await;
            }
            DocMessage::FromEditor(message) => self.handle_message_from_editor(message).await,
            DocMessage::ReceiveSyncMessage {
                message,
                state: mut peer_state,
                response_tx,
            } => {
                let patches = self.apply_sync_message_to_doc(message, &mut peer_state);
                let file_deltas = FileTextDelta::from_crdt_patches(patches);

                self.maybe_write_files_changed_in_file_deltas(&file_deltas);
                self.maybe_process_crdt_file_deltas_in_ot(file_deltas).await;

                response_tx
                    .send(peer_state)
                    .expect("Failed to send peer state in response to ReceiveSyncMessage");
            }
            DocMessage::GenerateSyncMessage {
                state: mut peer_state,
                response_tx,
            } => {
                let message = self.crdt_doc.generate_sync_message(&mut peer_state);
                response_tx.send((peer_state, message)).expect(
                    "Failed to send peer state and sync message in response to GenerateSyncMessage",
                );
            }
            DocMessage::NewEditorConnection(editor_handle) => {
                // TODO: if we use more than one ID, we should now easily have multiple editors.
                // Modulo managing the OT server for each of them per file...
                self.editor_clients.insert(EditorId(0), editor_handle);
            }
        }
    }

    fn file_path_for_uri(&self, uri: &str) -> String {
        // If uri starts with "file://", we remove it.
        let absolute_path = uri.strip_prefix("file://").unwrap_or(uri);

        // Check that it's an absolute path.
        assert!(absolute_path.starts_with('/'), "Path is not absolute");

        // TODO: Instead of panicking, we should handle this in a way so we don't crash.
        // Once the editor protocol is based on requests, we can send back an error?
        absolute_path
            .strip_prefix(&self.base_dir.display().to_string())
            .unwrap_or_else(|| panic!("Path {absolute_path} is not within base dir"))
            .strip_prefix('/')
            .expect("Could not remove a '/' while computing file path")
            .to_string()
    }

    fn absolute_path_for_file_path(&self, file_path: &str) -> String {
        format!("{}/{}", self.base_dir.display(), file_path)
    }

    async fn handle_message_from_editor(&mut self, message: EditorProtocolMessage) {
        match message {
            EditorProtocolMessage::Open { uri } => {
                let file_path = self.file_path_for_uri(&uri);
                debug!("Got an 'open' message for {file_path}");
                self.open_file_path(file_path);
            }
            EditorProtocolMessage::Close { uri } => {
                let file_path = self.file_path_for_uri(&uri);
                debug!("Got a 'close' message for {file_path}");
                self.ot_servers.remove(&file_path);
            }
            EditorProtocolMessage::Edit {
                delta: rev_delta,
                uri,
            } => {
                debug!("Handling RevDelta from editor: {:#?}", rev_delta);
                let file_path = self.file_path_for_uri(&uri);
                let (editor_delta_for_crdt, rev_deltas_for_editor) =
                    self.apply_delta_to_ot(rev_delta, &file_path);

                self.apply_delta_to_doc(&editor_delta_for_crdt, &file_path);
                self.send_deltas_to_editor(rev_deltas_for_editor, &file_path)
                    .await;
            }
        }
    }

    fn open_file_path(&mut self, file_path: String) {
        let ot_server = OTServer::new(self.current_file_content(&file_path).unwrap_or_else(|_| {
            panic!("Could not open file {file_path}, because it doesn't exist in the CRDT")
        }));
        self.ot_servers.insert(file_path, ot_server);
    }

    fn apply_sync_message_to_doc(
        &mut self,
        message: AutomergeSyncMessage,
        peer_state: &mut SyncState,
    ) -> Vec<Patch> {
        let patches = self
            .crdt_doc
            .receive_sync_message_log_patches(message, peer_state);
        let _ = self.doc_changed_ping_tx.send(());
        patches
    }

    async fn send_deltas_to_editor(
        &mut self,
        rev_deltas: Vec<RevisionedEditorTextDelta>,
        file_path: &str,
    ) {
        for rev_delta in rev_deltas {
            debug!("Sending RevDelta to socket: {:#?}", rev_delta);

            self.send_to_editors(rev_delta, file_path).await;
        }
    }

    fn get_ot_server(&mut self, file_path: &str) -> &mut OTServer {
        // TODO: Once we are able to send responses to the client,
        // fail in a nicer way, if Edit for unknown OTServer (client messed up).
        let error_message = format!("Could not get OTServer for {file_path}.");
        self.ot_servers.get_mut(file_path).expect(&error_message)
    }

    fn apply_delta_to_ot(
        &mut self,
        rev_editor_delta: RevisionedEditorTextDelta,
        file_path: &str,
    ) -> (EditorTextDelta, Vec<RevisionedEditorTextDelta>) {
        let text = self
            .current_file_content(file_path)
            .expect("Should have initialized text before performing random edit");
        let ot_server = self.get_ot_server(file_path);
        let (delta_for_crdt, rev_deltas_for_editor) =
            ot_server.apply_editor_operation(rev_editor_delta);

        let editor_delta_for_crdt = EditorTextDelta::from_delta(delta_for_crdt, &text);
        (editor_delta_for_crdt, rev_deltas_for_editor)
    }

    fn random_delta(&self) -> TextDelta {
        let text = self
            .current_file_content(TEST_FILE_PATH)
            .expect("Should have initialized text before performing random edit");
        let options = ["d", "ü", "🥕", "💚", "\n"];
        let random_text: String = (1..5)
            .map(|_| {
                let random_option = rand::thread_rng().gen_range(0..options.len());
                options[random_option]
            })
            .collect();
        let text_length = text.chars().count();
        let random_position = rand::thread_rng().gen_range(0..=text_length);

        let mut delta = TextDelta::default();
        delta.retain(random_position);
        delta.insert(&random_text);

        // TODO: Delete the end/beginning of the content on purpose sometimes!
        // Goal is to make "more critical" edits more likely. Like an "inverted" gauss curve :D
        let mut deletion_length = 0;
        if (text_length - random_position) > 0 {
            deletion_length = rand::thread_rng().gen_range(0..(text_length - random_position));
            deletion_length = deletion_length.min(3);
        }
        delta.delete(deletion_length);

        delta
    }

    async fn maybe_process_crdt_file_deltas_in_ot(&mut self, file_deltas: Vec<FileTextDelta>) {
        for FileTextDelta { file_path, delta } in file_deltas {
            // Only process the CRDT delta, if editor has the file open.
            if let Some(ot_server) = self.ot_servers.get_mut(&file_path) {
                debug!("Applying incoming CRDT patch for {file_path}");
                let rev_text_delta_for_editor = ot_server.apply_crdt_change(delta);
                self.send_to_editors(rev_text_delta_for_editor, &file_path)
                    .await;
            }
        }
    }

    async fn send_to_editors(&mut self, rev_delta: RevisionedEditorTextDelta, file_path: &str) {
        let message = EditorProtocolMessage::Edit {
            uri: format!("file://{}", self.absolute_path_for_file_path(file_path)),
            delta: rev_delta,
        };

        for handle in &mut self.editor_clients.values_mut() {
            handle.send(message.clone()).await;
        }
    }

    fn maybe_write_files_changed_in_file_deltas(&mut self, file_deltas: &Vec<FileTextDelta>) {
        // Collect file paths into a set, so we don't write files multiple times on complex
        // patches.
        let mut file_paths = HashSet::new();
        for FileTextDelta { file_path, .. } in file_deltas {
            file_paths.insert(file_path);
        }

        for file_path in file_paths {
            self.maybe_write_file(file_path);
        }
    }

    fn maybe_write_file(&mut self, file_path: &str) {
        // Only write to the file if editor *doesn't* have the file open.
        if !self.ot_servers.contains_key(file_path) {
            let text = self
                .current_file_content(file_path)
                .expect("Failed to get file content when writing to disk. Key should have existed");
            let abs_path = self.absolute_path_for_file_path(file_path);
            debug!("Writing to {abs_path}.");

            // Create the parent directorie(s), if neccessary.
            let parent_dir = Path::new(&abs_path).parent().unwrap();
            std::fs::create_dir_all(parent_dir).unwrap_or_else(|_| {
                panic!("Could not create parent directory {}", parent_dir.display())
            });

            std::fs::write(&abs_path, text)
                .unwrap_or_else(|_| panic!("Could not write to file {abs_path}"));
        }
    }

    /// Reading in the file is a preparatory step, before kicking off the actor.
    fn read_current_content_from_dir(&mut self) {
        // TODO: Filter out files ignored by .gitignore and such.
        WalkDir::new(self.base_dir.clone())
            .into_iter()
            .filter_map(Result::ok)
            .filter(|metadata| metadata.file_type().is_file())
            .for_each(|file_path| {
                let file_path = file_path.path();
                match std::fs::read_to_string(file_path) {
                    Ok(text) => {
                        let relative_file_path = self.file_path_for_uri(
                            file_path
                                .to_str()
                                .expect("Could not convert PathBuf to str"),
                        );
                        self.crdt_doc.initialize_text(&text, &relative_file_path);
                    }
                    Err(e) => {
                        warn!("Failed to read file {}: {e}", file_path.display());
                    }
                }
            });
    }

    fn current_file_content(&self, file_path: &str) -> Result<String> {
        self.crdt_doc.current_file_content(file_path)
    }

    fn apply_delta_to_doc(&mut self, delta: &EditorTextDelta, file_path: &str) {
        self.crdt_doc.apply_delta_to_doc(delta, file_path);
        let _ = self.doc_changed_ping_tx.send(());
        self.maybe_write_file(file_path);
    }

    async fn run(&mut self) {
        while let Some(message) = self.doc_message_rx.recv().await {
            self.handle_message(message).await;
        }
        panic!("Channel towards document task has been closed");
    }
}

/// This handle knows how to talk to the `DocumentActor` and provides an interface for doing so.
///
/// The main iterfaces for doing so is through through sending `DocMessage`s with `send_message`.
/// An alternative pathway is to subscribe to documents changes through `subscribe_document_changes`.
///
/// The rest of the methods are used for instrumentation (e.g. by the fuzzer).
#[derive(Clone)]
pub struct DocumentActorHandle {
    doc_message_tx: DocMessageSender,
    doc_changed_ping_tx: DocChangedSender,
}

impl DocumentActorHandle {
    pub fn new(base_dir: &Path, host: bool) -> Self {
        // The document task will receive messages on this channel.
        let (doc_message_tx, doc_message_rx) = mpsc::channel(1);

        // The document task will send a ping on this channel whenever it changes.
        // The sync tasks will subscribe to it, and react to it by syncing with the peers.
        let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(1);

        let mut actor =
            DocumentActor::new(doc_message_rx, doc_changed_ping_tx.clone(), base_dir.into());

        // Initialize the text from the file_path, if this is the document owned by the host.
        if host {
            actor.read_current_content_from_dir();
        }

        tokio::spawn(async move { actor.run().await });

        Self {
            doc_message_tx,
            doc_changed_ping_tx,
        }
    }

    /// The TCP and socket connections will send messages through this when they receive something.
    pub async fn send_message(&self, message: DocMessage) {
        self.doc_message_tx
            .send(message)
            .await
            .expect("DocumentActor task has been killed");
    }

    pub fn subscribe_document_changes(&self) -> DocChangedReceiver {
        self.doc_changed_ping_tx.subscribe()
    }

    pub async fn content(&self) -> Result<String> {
        let (send, recv) = oneshot::channel();
        let message = DocMessage::GetContent { response_tx: send };
        // Ignore send errors, because recv.await will fail anyway.
        let _ = self.doc_message_tx.send(message).await;
        recv.await.expect("DocumentActor task has been killed")
    }

    pub async fn apply_random_delta(&mut self) {
        let message = DocMessage::RandomEdit;
        self.doc_message_tx
            .send(message)
            .await
            .expect("Failed to send random edit to document task");
    }
}

pub struct Daemon {
    pub document_handle: DocumentActorHandle,
}

impl Daemon {
    // Launch the daemon. Optionally, connect to given peer.
    pub fn new(
        port: Option<u16>,
        peer: Option<String>,
        socket_path: &Path,
        base_dir: &Path,
    ) -> Self {
        // If the peer address is empty, we're the host.
        let is_host = peer.is_none();

        let document_handle = DocumentActorHandle::new(base_dir, is_host);

        let connection_document_handle = document_handle.clone();
        let peer_info = connect::PeerConnectionInfo::new(port, peer);
        tokio::spawn(async move {
            connect::make_peer_connection(peer_info, connection_document_handle).await;
        });

        let editor_socket_path = socket_path.to_path_buf();
        let editor_document_handle = document_handle.clone();
        tokio::spawn(async move {
            connect::make_editor_connection(editor_socket_path, editor_document_handle).await;
        });

        Self { document_handle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::factories::*;

    mod document {
        use super::*;

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
            assert_eq!(
                format!(
                    "{}",
                    document
                        .current_file_content("text")
                        .unwrap_err()
                        .root_cause()
                ),
                "Automerge document doesn't have a text Text object, so I can't provide it"
            );
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
            use automerge::PatchAction;
            use pretty_assertions::assert_eq;

            #[test]
            fn test_generate_sync_message() {
                let mut document = Document::default();
                let mut state = SyncState::new();
                assert!(document.generate_sync_message(&mut state).is_some());
                // Stops for now and waits for a response
                assert!(document.generate_sync_message(&mut state).is_none());

                document.initialize_text("", "text");
                // We have progressed out state, so update all peers about that.
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

            #[test]
            fn test_receive_sync_message_log_patches_intialize() {
                let mut document = Document::default();
                document.initialize_text("", "text");

                let mut patches = patches_when_syncing_with_peer(document);

                assert_eq!(patches.len(), 1);
                let patch = patches.pop().unwrap();
                assert_matches!(patch, Patch { path, action: PatchAction::PutMap { key, .. }, .. } => {
                    assert_eq!(path.len(), 0);
                    assert_eq!(key, "text");
                });
            }

            #[test]
            fn test_receive_sync_message_log_patches_has_delta() {
                let mut document = Document::default();
                document.initialize_text("", "text");
                let ed_delta = ed_delta_single((0, 0), (0, 0), "foobar");
                document.apply_delta_to_doc(&ed_delta, "text");

                let mut patches = patches_when_syncing_with_peer(document);

                assert_eq!(patches.len(), 2);
                // The last element corresponds to the delta
                let patch = patches.pop().unwrap();
                assert_matches!(patch, Patch { path, action: PatchAction::SpliceText { index, value, .. }, .. } => {
                    assert_eq!(path.len(), 1);
                    assert_eq!(index, 0);
                    assert_eq!(value.make_string(), "foobar");
                });
            }
        }
    }

    mod document_actor {
        use super::*;
        use assert_fs::prelude::*;
        use assert_fs::TempDir;
        use tracing_test::traced_test;

        impl DocumentActor {
            fn setup_for_testing(directory: PathBuf) -> Self {
                // The document task will receive messages on this channel.
                let (_doc_message_tx, doc_message_rx) = mpsc::channel(1);

                // The document task will send a ping on this channel whenever it changes.
                // The sync tasks will subscribe to it, and react to it by syncing with the peers.
                let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(1);

                DocumentActor::new(doc_message_rx, doc_changed_ping_tx.clone(), directory)
            }
            fn assert_file_content(&self, file_path: &str, content: &str) {
                // unfortunately anyhow::Error doesn't implement PartialEq, so we'll rather unwrap.
                assert_eq!(self.current_file_content(file_path).unwrap(), content);
            }
        }

        fn setup_filesystem_for_testing() -> TempDir {
            let dir = TempDir::new().expect("Failed to create temp directory");
            let file1 = dir.child("file1");
            let file2 = dir.child("file2");
            let file3 = dir.child("sub/file3");
            file1
                .write_str("content1")
                .expect("Failed to write test file");
            file2
                .write_str("content2")
                .expect("Failed to write test file");
            file3
                .write_str("content3")
                .expect("Failed to write test file");
            dir
        }

        #[test]
        fn read_contents_from_dir() {
            let dir = setup_filesystem_for_testing();
            let mut actor = DocumentActor::setup_for_testing(dir.path().to_path_buf());

            actor.read_current_content_from_dir();

            actor.assert_file_content("file1", "content1");
            actor.assert_file_content("file2", "content2");
            actor.assert_file_content("sub/file3", "content3");
        }

        #[test]
        #[traced_test]
        fn test_maybe_write_files_changed_in_file_deltas() {
            let dir = setup_filesystem_for_testing();
            debug!("{}", dir.path().display());
            let mut actor = DocumentActor::setup_for_testing(dir.path().to_path_buf());

            actor.read_current_content_from_dir();

            // One change to rule them all.
            let ed_delta = ed_delta_single((0, 0), (0, 0), "foobar");

            // "manually" apply the deltas, as we want to test
            // "maybe_write_files_changed_in_file_deltas" independently.
            actor.crdt_doc.apply_delta_to_doc(&ed_delta, "file1");
            actor.crdt_doc.apply_delta_to_doc(&ed_delta, "file2");
            actor.crdt_doc.apply_delta_to_doc(&ed_delta, "sub/file3");

            let delta = TextDelta::from_ed_delta(ed_delta, "content1");
            let file_deltas = vec![
                FileTextDelta::new("file1".to_string(), delta.clone()),
                FileTextDelta::new("file2".to_string(), delta.clone()),
                FileTextDelta::new("sub/file3".to_string(), delta),
            ];

            // The editor has file2 and sub/file3 open.
            actor.open_file_path("file2".into());
            actor.open_file_path("sub/file3".into());
            actor.maybe_write_files_changed_in_file_deltas(&file_deltas);

            // Thus, we only expect file1 to be changed on disk.
            dir.child("file1").assert("foobarcontent1");
            dir.child("file2").assert("content2");
            dir.child("sub/file3").assert("content3");
        }

        #[test]
        #[should_panic]
        fn test_file_path_for_uri_fails_not_absolute() {
            let dir = setup_filesystem_for_testing();
            let actor = DocumentActor::setup_for_testing(dir.path().to_path_buf());

            actor.file_path_for_uri("this/is/absolutely/not/absolute");
        }

        #[test]
        #[should_panic]
        fn test_file_path_for_uri_fails_not_within_base_dir() {
            let dir = setup_filesystem_for_testing();
            let actor = DocumentActor::setup_for_testing(dir.path().to_path_buf());

            actor.file_path_for_uri("/this/is/not/the/base_dir/file");
        }

        #[test]
        #[should_panic]
        fn test_file_path_for_uri_fails_only_base_dir() {
            let dir = setup_filesystem_for_testing();
            let actor = DocumentActor::setup_for_testing(dir.path().to_path_buf());

            actor.file_path_for_uri(&format!("{}", dir.path().display()));
        }

        #[test]
        fn test_file_path_for_uri_works() {
            let dir = setup_filesystem_for_testing();
            let actor = DocumentActor::setup_for_testing(dir.path().to_path_buf());

            let file_paths = vec!["afile", "adir/with/some/file", "just/adir/"];
            let prefix_options = vec!["file://", ""];
            for prefix in prefix_options {
                for &expected in &file_paths {
                    let uri = format!("{}{}/{}", prefix, dir.path().display(), expected);

                    assert_eq!(actor.file_path_for_uri(&uri), expected);
                }
            }
        }

        #[test]
        fn test_simulate_editor_edits() {
            let dir = setup_filesystem_for_testing();
            let mut actor = DocumentActor::setup_for_testing(dir.path().to_path_buf());
            actor.read_current_content_from_dir();

            let file_path = "file1".to_string();

            actor.open_file_path(file_path.clone());

            let delta = rev_ed_delta_single(0, (0, 0), (0, 0), "foobar");
            let (editor_delta_for_crdt, rev_ed_text_deltas) =
                actor.apply_delta_to_ot(delta, "file1");
            actor.apply_delta_to_doc(&editor_delta_for_crdt, &file_path);

            // Confirm nothing transformed needs to go to editor.
            assert_eq!(rev_ed_text_deltas, vec![]);

            // Confirm edit was applied.
            actor.assert_file_content(&file_path, "foobarcontent1");
        }
    }
}
