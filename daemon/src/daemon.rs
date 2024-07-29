use crate::connect;
use crate::document::Document;
use crate::editor::EditorHandle;
use crate::ot::OTServer;
use crate::sandbox;
use crate::types::{
    CursorState, EditorProtocolMessageFromEditor, EditorProtocolMessageToEditor, EditorTextDelta,
    FileTextDelta, PatchEffect, Range, RevisionedEditorTextDelta, TextDelta,
};
use anyhow::Result;
use automerge::{
    sync::{Message as AutomergeSyncMessage, State as SyncState},
    Patch,
};
use ignore::WalkBuilder;
use notify::{RecursiveMode, Result as NotifyResult, Watcher};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use tokio::{
    sync::{broadcast, mpsc, oneshot},
    time::Duration,
};
use tracing::{debug, info, warn};

pub const TEST_FILE_PATH: &str = "text";

// These messages are sent to the task that owns the document.
pub enum DocMessage {
    GetContent {
        response_tx: oneshot::Sender<Result<String>>,
    },
    FromEditor(EditorProtocolMessageFromEditor),
    RemoveFile {
        file_path: String,
    },
    Persist,
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
    CloseEditorConnection,
}

impl fmt::Debug for DocMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            DocMessage::GetContent { .. } => "get content",
            DocMessage::FromEditor(_) => "open/close/edit/... message from editor",
            DocMessage::RemoveFile { .. } => "delete file",
            DocMessage::Persist => "persist",
            DocMessage::RandomEdit => "random edit",
            DocMessage::ReceiveSyncMessage { .. } => "<automerge internal sync rcv>",
            DocMessage::GenerateSyncMessage { .. } => "<automerge internal sync gen>",
            DocMessage::NewEditorConnection(_) => "editor connected",
            DocMessage::CloseEditorConnection => "editor disconnected",
        };
        write!(f, "{repr}")
    }
}

type DocMessageSender = mpsc::Sender<DocMessage>;
type DocChangedSender = broadcast::Sender<()>;
type DocChangedReceiver = broadcast::Receiver<()>;

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
        init: bool,
        is_host: bool,
    ) -> Self {
        // If there is a persisted version in base_dir/.ethersync/doc, load it.
        // TODO: Pull out ".ethersync" string into a constant.
        let persistence_file = base_dir.join(".ethersync/doc");
        let persistence_file_exists = sandbox::exists(&base_dir, &persistence_file)
            .expect("Could not check for the existence of the persistence file");

        let crdt_doc = if persistence_file_exists && !init {
            info!("Loading persisted CRDT document from {persistence_file:?}");
            let bytes = sandbox::read_file(&base_dir, &persistence_file)
                .unwrap_or_else(|_| panic!("Could not read file '{persistence_file:?}'"));
            Document::load(&bytes)
        } else {
            info!("Initializing a new CRDT document");
            Document::default()
        };
        info!("Done.");

        let mut s = Self {
            doc_message_rx,
            doc_changed_ping_tx,
            editor_clients: HashMap::default(),
            base_dir,
            ot_servers: HashMap::default(),
            crdt_doc,
        };

        if persistence_file_exists {
            s.read_current_content_from_dir(init);
        } else if is_host {
            s.read_current_content_from_dir(true);
        }

        s
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
            DocMessage::RemoveFile { file_path } => {
                // TODO: This is a workaround. Handle this case better by returning a Result in
                // file_path_for_uri! The deletion happens when the fuzzer removes the temp dir.
                if file_path.as_str() != self.base_dir.as_os_str() {
                    let file_path = self.file_path_for_uri(&file_path);
                    self.crdt_doc.remove_text(&file_path);
                    let _ = self.doc_changed_ping_tx.send(());
                }
            }
            DocMessage::Persist => {
                debug!("Persisting!");
                let bytes = self.crdt_doc.save();
                let persistence_file = self.base_dir.join(".ethersync/doc");
                sandbox::write_file(&self.base_dir, &persistence_file, &bytes)
                    .unwrap_or_else(|_| panic!("Failed to persist to {persistence_file:?}"));
            }
            DocMessage::ReceiveSyncMessage {
                message,
                state: mut peer_state,
                response_tx,
            } => {
                let patches = self.apply_sync_message_to_doc(message, &mut peer_state);

                let patch_effects = PatchEffect::from_crdt_patches(patches);

                let mut file_deltas = vec![];
                let mut cursor_states = vec![];

                for patch_effect in patch_effects {
                    match patch_effect {
                        PatchEffect::FileChange(file_text_delta) => {
                            file_deltas.push(file_text_delta);
                        }
                        PatchEffect::CursorChange(cursor_state) => {
                            cursor_states.push(cursor_state);
                        }
                        PatchEffect::FileRemoval(file_path) => {
                            sandbox::remove_file(
                                &self.base_dir,
                                Path::new(&self.absolute_path_for_file_path(&file_path)),
                            )
                            .unwrap_or_else(|err| {
                                warn!("Failed to remove file {file_path}: {err}")
                            });
                        }
                        PatchEffect::NoEffect => {}
                    }
                }

                self.maybe_write_files_changed_in_file_deltas(&file_deltas);
                self.maybe_process_crdt_file_deltas_in_ot(file_deltas).await;
                self.process_cursor_states(cursor_states).await;

                if response_tx.send(peer_state).is_err() {
                    warn!("Failed to send peer state in response to ReceiveSyncMessage.");
                }
            }
            DocMessage::GenerateSyncMessage {
                state: mut peer_state,
                response_tx,
            } => {
                let message = self.crdt_doc.generate_sync_message(&mut peer_state);

                if response_tx.send((peer_state, message)).is_err() {
                    warn!("Failed to send peer state and sync message in response to GenerateSyncMessage.");
                }
            }
            DocMessage::NewEditorConnection(editor_handle) => {
                // TODO: if we use more than one ID, we should now easily have multiple editors.
                // Modulo managing the OT server for each of them per file...
                self.editor_clients.insert(EditorId(0), editor_handle);
            }
            DocMessage::CloseEditorConnection => {
                self.editor_clients.remove(&EditorId(0));

                let userid = self.crdt_doc.actor_id();
                self.maybe_delete_cursor_position(userid);
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

    async fn handle_message_from_editor(&mut self, message: EditorProtocolMessageFromEditor) {
        match message {
            EditorProtocolMessageFromEditor::Open { uri } => {
                let file_path = self.file_path_for_uri(&uri);
                debug!("Got an 'open' message for {file_path}");
                self.open_file_path(file_path);
            }
            EditorProtocolMessageFromEditor::Close { uri } => {
                let file_path = self.file_path_for_uri(&uri);
                debug!("Got a 'close' message for {file_path}");
                self.ot_servers.remove(&file_path);
            }
            EditorProtocolMessageFromEditor::Edit {
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
            EditorProtocolMessageFromEditor::Cursor { uri, ranges } => {
                let file_path = self.file_path_for_uri(&uri);
                let userid = self.crdt_doc.actor_id();
                self.store_cursor_position(userid, file_path, ranges);
            }
        }
    }

    fn open_file_path(&mut self, file_path: String) {
        let text = match self.current_file_content(&file_path) {
            Ok(text) => text,
            Err(_) => {
                // The file doesn't exist yet - create it in the Automerge document.
                let text = "".to_string();
                self.crdt_doc.initialize_text(&text, &file_path);
                text
            }
        };
        let ot_server = OTServer::new(text);
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
            .expect("Should have initialized text before performing edit");
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

    async fn process_cursor_states(&mut self, cursor_states: Vec<CursorState>) {
        for CursorState {
            userid,
            name,
            file_path,
            ranges,
        } in cursor_states
        {
            let message = EditorProtocolMessageToEditor::Cursor {
                userid,
                name,
                uri: format!("file://{}", self.absolute_path_for_file_path(&file_path)),
                ranges,
            };
            self.send_to_editor_clients(message).await;
        }
    }

    async fn send_to_editors(&mut self, rev_delta: RevisionedEditorTextDelta, file_path: &str) {
        let message = EditorProtocolMessageToEditor::Edit {
            uri: format!("file://{}", self.absolute_path_for_file_path(file_path)),
            delta: rev_delta,
        };
        self.send_to_editor_clients(message).await;
    }

    async fn send_to_editor_clients(&mut self, message: EditorProtocolMessageToEditor) {
        let mut to_remove = Vec::new();
        for (id, handle) in &mut self.editor_clients.iter_mut() {
            if handle.send(message.clone()).await.is_err() {
                // Remove this client.
                to_remove.push(*id);
            }
        }
        for id in to_remove {
            // The destructor of EditorHandle will shut down the actors when
            // we remove it from the HashMap.
            info!("Removing EditorHandle from client list.");
            self.editor_clients.remove(&id);
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
            if let Ok(text) = self.current_file_content(file_path) {
                let abs_path = self.absolute_path_for_file_path(file_path);
                debug!("Writing to {abs_path}.");

                // Create the parent directorie(s), if neccessary.
                let parent_dir = Path::new(&abs_path).parent().unwrap();
                sandbox::create_dir_all(&self.base_dir, parent_dir).unwrap_or_else(|_| {
                    panic!("Could not create parent directory {}", parent_dir.display())
                });

                sandbox::write_file(&self.base_dir, Path::new(&abs_path), &text.into_bytes())
                    .unwrap_or_else(|_| panic!("Could not write to file {abs_path}"));
            } else {
                warn!("Failed to get content of file '{file_path}' when writing to disk. Key should have existed?");
            }
        }
    }

    /// Reading in the file is a preparatory step, before kicking off the actor.
    fn read_current_content_from_dir(&mut self, init: bool) {
        let ignored_things = [".git", ".ethersync"];
        // TODO: How to deal with binary files?
        WalkBuilder::new(self.base_dir.clone())
            .standard_filters(true)
            .hidden(false)
            // Interestingly, the standard filters don't seem to ignore .git.
            .filter_entry(move |dir_entry| {
                let name = dir_entry
                    .path()
                    .file_name()
                    .expect("Failed to get file name from path.")
                    .to_str()
                    .expect("Failed to convert OsStr to str");
                !ignored_things.contains(&name)
            })
            .build()
            .filter_map(Result::ok)
            .filter(|dir_entry| {
                dir_entry
                    .file_type()
                    .expect("Couldn't get file type of dir entry.")
                    .is_file()
            })
            .for_each(|dir_entry| {
                let file_path = dir_entry.path();
                match sandbox::read_file(&self.base_dir, file_path) {
                    Ok(bytes) => {
                        let relative_file_path = self.file_path_for_uri(
                            file_path
                                .to_str()
                                .expect("Could not convert PathBuf to str"),
                        );
                        let text = String::from_utf8(bytes).expect("Could not read file as UTF-8");
                        if init {
                            self.crdt_doc.initialize_text(&text, &relative_file_path);
                        } else {
                            self.crdt_doc.update_text(&text, &relative_file_path);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read file {}: {e}", file_path.display());
                    }
                }
            });
        for file_path in self.crdt_doc.files() {
            let absolute_file_path = self.absolute_path_for_file_path(&file_path);
            if !sandbox::exists(&self.base_dir, Path::new(&absolute_file_path)).expect("") {
                warn!(
                    "File '{file_path}' exists in the CRDT, but not on disk. Deleting from CRDT."
                );
                self.crdt_doc.remove_text(&file_path);
            }
        }
        let _ = self.doc_changed_ping_tx.send(());
    }

    fn current_file_content(&self, file_path: &str) -> Result<String> {
        self.crdt_doc.current_file_content(file_path)
    }

    fn apply_delta_to_doc(&mut self, delta: &EditorTextDelta, file_path: &str) {
        self.crdt_doc.apply_delta_to_doc(delta, file_path);
        let _ = self.doc_changed_ping_tx.send(());
        self.maybe_write_file(file_path);
    }

    fn store_cursor_position(&mut self, userid: String, file_path: String, ranges: Vec<Range>) {
        self.crdt_doc
            .store_cursor_position(userid, file_path, ranges);
        let _ = self.doc_changed_ping_tx.send(());
    }

    fn maybe_delete_cursor_position(&mut self, userid: String) {
        self.crdt_doc.maybe_delete_cursor_position(userid);
        let _ = self.doc_changed_ping_tx.send(());
    }

    async fn run(&mut self) {
        while let Some(message) = self.doc_message_rx.recv().await {
            self.handle_message(message).await;
        }
        debug!("Channel towards document handle has been closed (probably shutting down)");
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
    pub fn new(base_dir: &Path, init: bool, is_host: bool) -> Self {
        // The document task will receive messages on this channel.
        let (doc_message_tx, doc_message_rx) = mpsc::channel(1);

        // The document task will send a ping on this channel whenever it changes.
        // The sync tasks will subscribe to it, and react to it by syncing with the peers.
        let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(1);

        let mut actor = DocumentActor::new(
            doc_message_rx,
            doc_changed_ping_tx.clone(),
            base_dir.into(),
            init,
            is_host,
        );

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
        init: bool,
    ) -> Self {
        // If the peer address is empty, we're the host.
        let is_host = peer.is_none();

        let document_handle = DocumentActorHandle::new(base_dir, init, is_host);

        // Initialize file watcher.
        let watcher_document_handle = document_handle.clone();
        let watcher_base_dir = base_dir.to_path_buf();
        tokio::spawn(async move {
            spawn_file_watcher(watcher_base_dir, watcher_document_handle).await;
        });

        // Initialize persister.
        let persister_document_handle = document_handle.clone();
        tokio::spawn(async move {
            let mut doc_changed_ping_rx = persister_document_handle.subscribe_document_changes();

            persister_document_handle
                .send_message(DocMessage::Persist)
                .await;

            // TODO: debounce / slow down?
            while doc_changed_ping_rx.recv().await.is_ok() {
                persister_document_handle
                    .send_message(DocMessage::Persist)
                    .await;
            }
        });

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

async fn spawn_file_watcher(base_dir: PathBuf, document_handle: DocumentActorHandle) {
    // Beware of the file watching spaghetti triangle monster.
    let mut watcher = notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
        futures::executor::block_on(async {
            match res {
                Ok(event) => {
                    // TODO: On Linux, even a directory deletion seems to yield a Remove(File)?
                    if let notify::event::EventKind::Remove(notify::event::RemoveKind::File) =
                        event.kind
                    {
                        for path in event.paths {
                            document_handle
                                .send_message(DocMessage::RemoveFile {
                                    file_path: path
                                        .to_str()
                                        .expect("Failed to convert path to string")
                                        .into(),
                                })
                                .await;
                        }
                    }
                }
                Err(e) => panic!("watch error: {:?}", e),
            }
        })
    })
    .expect("Failed to initialize file watcher");

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher
        .watch(&base_dir, RecursiveMode::Recursive)
        .expect("Failed to watch directory");

    // TODO: can this be event based?
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::factories::*;

    mod document_actor {
        use super::*;
        use temp_dir::TempDir;
        use tracing_test::traced_test;

        impl DocumentActor {
            fn setup_for_testing(directory: PathBuf) -> Self {
                // The document task will receive messages on this channel.
                let (_doc_message_tx, doc_message_rx) = mpsc::channel(1);

                // The document task will send a ping on this channel whenever it changes.
                // The sync tasks will subscribe to it, and react to it by syncing with the peers.
                let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(1);

                DocumentActor::new(
                    doc_message_rx,
                    doc_changed_ping_tx.clone(),
                    directory,
                    true,
                    true,
                )
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
            let subdir = dir.child("sub");
            sandbox::create_dir(dir.path(), &subdir).unwrap();
            let file3 = dir.child("sub/file3");
            sandbox::write_file(dir.path(), &file1, b"content1").unwrap();
            sandbox::write_file(dir.path(), &file2, b"content2").unwrap();
            sandbox::write_file(dir.path(), &file3, b"content3").unwrap();
            dir
        }

        #[test]
        fn read_contents_from_dir() {
            let dir = setup_filesystem_for_testing();
            let mut actor = DocumentActor::setup_for_testing(dir.path().to_path_buf());

            actor.read_current_content_from_dir(true);

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

            actor.read_current_content_from_dir(true);

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
            assert_eq!(
                sandbox::read_file(dir.path(), &dir.child("file1")).unwrap(),
                b"foobarcontent1",
            );
            assert_eq!(
                sandbox::read_file(dir.path(), &dir.child("file2")).unwrap(),
                b"content2",
            );
            assert_eq!(
                sandbox::read_file(dir.path(), &dir.child("sub/file3")).unwrap(),
                b"content3",
            );
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
            actor.read_current_content_from_dir(true);

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
