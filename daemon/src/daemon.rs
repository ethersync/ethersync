// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::config;
use crate::document::Document;
use crate::editor::{Editor, EditorId, EditorWriter};
use crate::editor_connection::EditorConnection;
use crate::path::{AbsolutePath, RelativePath};
use crate::peer;
use crate::sandbox;
use crate::types::{
    ComponentMessage, CursorId, CursorState, EditorProtocolMessageError,
    EditorProtocolMessageFromEditor, EditorProtocolMessageToEditor, EditorProtocolObject,
    EphemeralMessage, FileTextDelta, JSONRPCFromEditor, JSONRPCResponse, PatchEffect, TextDelta,
};
use crate::watcher::Watcher;
use crate::watcher::WatcherEvent;
use crate::wormhole::put_secret_address_into_wormhole;
use anyhow::Result;
use automerge::ChangeHash;
use automerge::{
    sync::{Message as AutomergeSyncMessage, State as SyncState},
    Patch,
};
use futures::SinkExt;
use ignore::{Walk, WalkBuilder};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::{
    sync::{broadcast, mpsc, oneshot},
    time::Duration,
};
use tracing::{debug, error, info, warn};

pub const TEST_FILE_PATH: &str = "text";

// These messages are sent to the task that owns the document.
pub enum DocMessage {
    GetContent {
        response_tx: oneshot::Sender<Result<String>>,
    },
    FromEditor(EditorId, String),
    FromWatcher(WatcherEvent),
    RescanFiles,
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
    NewEditorConnection(EditorId, EditorWriter),
    CloseEditorConnection(EditorId),
    ReceiveEphemeral(EphemeralMessage),
}

impl fmt::Debug for DocMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            DocMessage::GetContent { .. } => "GetContent".to_string(),
            DocMessage::FromEditor(id, s) => format!("FromEditor({id}, {s})"),
            DocMessage::FromWatcher(e) => format!("FromWatcher({e:?}"),
            DocMessage::RescanFiles => "RescanFiles".to_string(),
            DocMessage::Persist => "Persist".to_string(),
            DocMessage::RandomEdit => "RandomEdit".to_string(),
            DocMessage::ReceiveSyncMessage { .. } => "ReceiveSyncMessage".to_string(),
            DocMessage::GenerateSyncMessage { .. } => "GenerateSyncMessage".to_string(),
            DocMessage::NewEditorConnection(id, _) => format!("NewEditorConnection({id})"),
            DocMessage::CloseEditorConnection(id) => format!("CloseEditorConnection({id})"),
            DocMessage::ReceiveEphemeral(m) => format!("ReceiveEphemeral({m:?})"),
        };
        write!(f, "{repr}")
    }
}

type DocMessageSender = mpsc::Sender<DocMessage>;
type DocChangedSender = broadcast::Sender<()>;
type DocChangedReceiver = broadcast::Receiver<()>;
type EphemeralMessageSender = broadcast::Sender<EphemeralMessage>;
type EphemeralMessageReceiver = broadcast::Receiver<EphemeralMessage>;

/// This Actor is responsible for applying changes to the document asynchronously.
///
/// Any `DocMessage` that is emitted via `DocumentActorHandle` should have an effect eventually.
pub struct DocumentActor {
    doc_message_rx: mpsc::Receiver<DocMessage>,
    doc_changed_ping_tx: DocChangedSender,
    ephemeral_message_tx: EphemeralMessageSender,
    editor_connections: HashMap<EditorId, (EditorConnection, EditorWriter)>,
    ephemeral_states: HashMap<CursorId, EphemeralMessage>,
    /// The Document is the main I/O managed resource of this actor.
    crdt_doc: Document,
    base_dir: PathBuf,
    save_fully: bool,
}

impl DocumentActor {
    #[must_use]
    fn new(
        doc_message_rx: mpsc::Receiver<DocMessage>,
        doc_changed_ping_tx: DocChangedSender,
        ephemeral_message_tx: EphemeralMessageSender,
        base_dir: PathBuf,
        init: bool,
        is_host: bool,
        persist: bool,
    ) -> Self {
        // If there is a persisted version in base_dir/.ethersync/doc, load it.
        // TODO: Pull out ".ethersync" string into a constant.
        let persistence_file = base_dir.join(".ethersync/doc");
        let persistence_file_exists = sandbox::exists(&base_dir, &persistence_file)
            .expect("Could not check for the existence of the persistence file");

        let load_crdt_doc = persistence_file_exists && !init && persist;
        let crdt_doc = if load_crdt_doc {
            debug!(
                "Loading persisted CRDT document from '{}'.",
                persistence_file.display()
            );
            let bytes = sandbox::read_file(&base_dir, &persistence_file)
                .unwrap_or_else(|_| panic!("Could not read file '{}'", persistence_file.display()));
            Document::load(&bytes)
        } else {
            Document::default()
        };
        debug!("Loading CRDT document completed.");

        let mut s = Self {
            doc_message_rx,
            doc_changed_ping_tx,
            ephemeral_message_tx,
            editor_connections: HashMap::default(),
            ephemeral_states: HashMap::default(),
            base_dir,
            crdt_doc,
            save_fully: true,
        };

        if persistence_file_exists && persist {
            s.read_current_content_from_dir(init);
        } else if is_host {
            s.read_current_content_from_dir(true);
        }

        s
    }

    /// If any editor owns the file, it means that the daemon doesn't have ownership.
    fn owns(&mut self, file_path: &RelativePath) -> bool {
        !self
            .editor_connections
            .values()
            .any(|connection| connection.0.owns(file_path))
    }

    async fn handle_message(&mut self, message: DocMessage) {
        debug!("Handling doc message: {message:?}");
        match message {
            DocMessage::GetContent { response_tx } => {
                response_tx
                    .send(self.current_file_content(&RelativePath::new(TEST_FILE_PATH)))
                    .expect("Failed to send content to response channel");
            }
            DocMessage::RandomEdit => {
                let delta = self.random_delta();
                let message = ComponentMessage::Edit {
                    file_path: RelativePath::new(TEST_FILE_PATH),
                    delta,
                };
                self.process_component_message(None, &message).await;
            }
            DocMessage::FromEditor(editor_id, message) => {
                self.handle_message_from_editor(editor_id, message).await;
            }
            DocMessage::FromWatcher(watcher_event) => {
                self.handle_watcher_event(watcher_event).await;
            }
            DocMessage::RescanFiles => {
                self.read_current_content_from_dir(false);
            }
            DocMessage::Persist => {
                let persistence_file = self.base_dir.join(".ethersync/doc");
                if self.save_fully {
                    debug!("Persisting CRDT document fully.");
                    let bytes = self.crdt_doc.save();
                    sandbox::write_file(&self.base_dir, &persistence_file, &bytes).unwrap_or_else(
                        |_| panic!("Failed to persist to '{}'", persistence_file.display()),
                    );
                    self.save_fully = false
                } else {
                    debug!("Persisting CRDT document incrementally.");
                    let bytes = self.crdt_doc.save_incremental();
                    sandbox::append_file(&self.base_dir, &persistence_file, &bytes).unwrap_or_else(
                        |_| panic!("Failed to persist to '{}'", persistence_file.display()),
                    );
                }
            }
            DocMessage::ReceiveSyncMessage {
                message,
                state: mut peer_state,
                response_tx,
            } => {
                let heads_before_sync_message = self.get_heads();

                let patches = self.apply_sync_message_to_doc(message, &mut peer_state);

                let patch_effects = PatchEffect::from_crdt_patches(patches);

                let mut file_deltas = vec![];

                for patch_effect in patch_effects {
                    match patch_effect {
                        PatchEffect::FileChange(file_text_delta) => {
                            file_deltas.push(file_text_delta);
                        }
                        PatchEffect::FileRemoval(file_path) => {
                            if self.owns(&file_path) {
                                info!("Removing file {file_path}.");

                                sandbox::remove_file(
                                    &self.base_dir,
                                    &self.absolute_path_for_file_path(&file_path),
                                )
                                .unwrap_or_else(|err| {
                                    warn!("Failed to remove file {file_path}: {err}");
                                });
                            } else {
                                // At least one editor has the file open. We want to allow it to
                                // keep editing it. Conceptually, we want to treat the file as
                                // still there, but send deltas to the editor to make it empty.

                                // Delete all previous file_deltas touching that file. After the
                                // deletion, they are now irrelevant to the editor. And it's easier
                                // for us to find out the file's content directly before the sync
                                // message was applied.
                                file_deltas.retain(|d| d.file_path != file_path);

                                let content_before_sync_message = self
                                    .file_content_at(&file_path, &heads_before_sync_message)
                                    .expect("Could not get file content at heads");

                                // Create a delta that deletes all the previous content.
                                let mut text_delta = TextDelta::default();
                                text_delta.delete(content_before_sync_message.chars().count());
                                let delta = FileTextDelta {
                                    file_path: file_path.clone(),
                                    delta: text_delta,
                                };
                                file_deltas.push(delta);

                                // If the file doesn't exist anymore after the sync message was
                                // applied (which is now!), we'd like it to be there again. So
                                // re-create an empty version.
                                if !self.crdt_doc.file_exists(&file_path) {
                                    info!("Peer deleted {file_path}, but you have it open in an editor. Bringing back an empty version.");
                                    self.crdt_doc.update_text("", &file_path);
                                } else {
                                    // If the file is still there, the upcoming patches of the sync
                                    // message will re-add it for us. In that case, we don't want
                                    // to touch it in the doc, because we will send the
                                    // modifications to the editors, and these contents should be
                                    // consistent. So we don't need to do anything.
                                }
                            }
                        }
                        PatchEffect::FileBytes(file_path, bytes) => {
                            let absolute_path = &self.absolute_path_for_file_path(&file_path);

                            // If the file didn't exist before, log it.
                            if !sandbox::exists(&self.base_dir, absolute_path)
                                .expect("Failed to check for file existence before writing to it")
                            {
                                info!("Creating binary file {file_path}.");
                            }

                            sandbox::write_file(&self.base_dir, absolute_path, &bytes)
                                .unwrap_or_else(|err| {
                                    warn!("Failed to write to file {file_path}: {err}");
                                });
                        }
                        PatchEffect::NoEffect => {}
                    }
                }

                self.write_files_changed_in_file_deltas(&file_deltas);

                for file_text_delta in &file_deltas {
                    let message = ComponentMessage::Edit {
                        file_path: file_text_delta.file_path.clone(),
                        delta: file_text_delta.delta.clone(),
                    };
                    self.broadcast_to_editors(None, &message).await;
                }

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
            DocMessage::NewEditorConnection(id, editor_writer) => {
                let editor_connection_id = self.cursor_id(id);
                self.editor_connections.insert(
                    id,
                    (
                        EditorConnection::new(editor_connection_id, self.base_dir.clone()),
                        editor_writer,
                    ),
                );

                // Send all known cursor states to editor.
                for (cursor_id, ephemeral_message) in self.ephemeral_states.clone() {
                    let message = ComponentMessage::Cursor {
                        cursor_id: cursor_id.clone(),
                        cursor_state: ephemeral_message.cursor_state.clone(),
                    };
                    self.send_to_editor(id, &message).await;
                }
            }
            DocMessage::CloseEditorConnection(editor_id) => {
                self.editor_connections.remove(&editor_id);

                let cursor_id = self.cursor_id(editor_id);
                debug!("Deleting cursor {cursor_id}.");
                self.maybe_delete_cursor_position(&cursor_id).await;
            }
            DocMessage::ReceiveEphemeral(ephemeral_message) => {
                self.react_to_ephemeral_message(ephemeral_message.clone())
                    .await;
            }
        }
    }

    fn absolute_path_for_file_path(&self, file_path: &RelativePath) -> AbsolutePath {
        AbsolutePath::from_parts(&self.base_dir, file_path).expect("base_dir should be absolute")
    }

    // Returns the messages to send back to the editor which made the request.
    async fn react_to_message_from_editor(
        &mut self,
        editor_id: EditorId,
        message: &EditorProtocolMessageFromEditor,
    ) -> Result<Vec<EditorProtocolMessageToEditor>, EditorProtocolMessageError> {
        // First, convert the editor message into a component message (+ transformed edits from the
        // OT server).
        let (inside_message, mut messages_to_editor) = self
            .editor_connections
            .get_mut(&editor_id)
            .expect("Could not get editor connection")
            .0
            .message_from_editor(message)?;

        // Then, forward them to the "core", and get back component messages that should be
        // returned to the editor (because, for example, it opened a file with a not up-to-date
        // content.)
        let component_messages_to_editor = self
            .process_component_message(Some(editor_id), &inside_message)
            .await;

        // And finally, send these component messages back to the editor connection (pass them
        // through the OT server), to retrieve raw messages for the editor.
        let mut more_messages_to_editor =
            self.process_in_editor(editor_id, component_messages_to_editor);

        messages_to_editor.append(&mut more_messages_to_editor);

        Ok(messages_to_editor)
    }

    fn cursor_id(&self, editor_id: EditorId) -> String {
        self.crdt_doc.actor_id() + "-" + editor_id.to_string().as_str()
    }

    async fn handle_message_from_editor(&mut self, editor_id: EditorId, message: String) {
        match JSONRPCFromEditor::from_jsonrpc(&message) {
            Ok(parsed_message) => match parsed_message {
                JSONRPCFromEditor::Request { id, payload } => {
                    let result = self.react_to_message_from_editor(editor_id, &payload).await;
                    match result {
                        Err(error) => {
                            error!("Error for JSON-RPC request: {:?}", error);
                            self.send_to_editor_client(
                                &editor_id,
                                EditorProtocolObject::Response(JSONRPCResponse::RequestError {
                                    id: Some(id),
                                    error,
                                }),
                            )
                            .await;
                        }
                        Ok(messages) => {
                            self.send_to_editor_client(
                                &editor_id,
                                EditorProtocolObject::Response(JSONRPCResponse::RequestSuccess {
                                    id,
                                    result: "success".into(),
                                }),
                            )
                            .await;
                            for message in messages {
                                self.send_to_editor_client(
                                    &editor_id,
                                    EditorProtocolObject::Request(message),
                                )
                                .await;
                            }
                        }
                    };
                }
                JSONRPCFromEditor::Notification { payload } => {
                    let _ = self.react_to_message_from_editor(editor_id, &payload).await;
                }
            },
            Err(e) => {
                let response = JSONRPCResponse::RequestError {
                    id: None,
                    error: EditorProtocolMessageError {
                        code: -32700,
                        message: format!("Invalid request: {e}"),
                        data: None,
                    },
                };
                error!("Error for JSON-RPC request: {:?}", response);
                self.send_to_editor_client(&editor_id, EditorProtocolObject::Response(response))
                    .await;
            }
        }
    }

    async fn handle_watcher_event(&mut self, watcher_event: WatcherEvent) {
        match watcher_event {
            WatcherEvent::Created { file_path } => {
                let relative_file_path = RelativePath::try_from_path(&self.base_dir, &file_path)
                    .expect("Watcher event should have a path within the base directory");
                if self.owns(&relative_file_path) {
                    if !self.crdt_doc.file_exists(&relative_file_path) {
                        let content = match sandbox::read_file(
                            &self.base_dir,
                            Path::new(&file_path),
                        ) {
                            Ok(content) => content,
                            Err(e) => {
                                warn!(
                                    "The file watcher noticed a file creation for {relative_file_path}, \
                                    but we couldn't read it: {e} (probably it disappeared again already?)"
                                );
                                return;
                            }
                        };
                        if let Ok(content) = String::from_utf8(content.clone()) {
                            self.crdt_doc.initialize_text(&content, &relative_file_path);
                        } else {
                            self.crdt_doc.set_bytes(&content, &relative_file_path);
                        }
                        let _ = self.doc_changed_ping_tx.send(());
                    } else {
                        debug!("Received watcher creation event, but file already exists in CRDT.");
                    }
                }
            }
            WatcherEvent::Removed { file_path } => {
                let relative_file_path = RelativePath::try_from_path(&self.base_dir, &file_path)
                    .expect("Watcher event should have a path within the base directory");
                if self.owns(&relative_file_path) {
                    self.remove_file(&relative_file_path);
                    let _ = self.doc_changed_ping_tx.send(());
                }
            }
            WatcherEvent::Changed { file_path } => {
                let relative_file_path = RelativePath::try_from_path(&self.base_dir, &file_path)
                    .expect("Watcher event should have a path within the base directory");
                if self.owns(&relative_file_path) {
                    let new_content = match sandbox::read_file(
                        &self.base_dir,
                        Path::new(&file_path),
                    ) {
                        Ok(content) => content,
                        Err(e) => {
                            warn!(
                                "The file watcher noticed a file change for {relative_file_path}, \
                                but we couldn't read it: {e} (probably it was deleted after the change?)"
                            );
                            return;
                        }
                    };
                    if let Ok(new_content) = String::from_utf8(new_content.clone()) {
                        self.crdt_doc.update_text(&new_content, &relative_file_path);
                        // TODO: Once we get back to processing file changes while editors have it
                        // open, send the delta returned by update_text to editors.
                    } else {
                        self.crdt_doc.set_bytes(&new_content, &relative_file_path);
                    }
                    let _ = self.doc_changed_ping_tx.send(());
                }
            }
        }
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

    fn get_heads(&mut self) -> Vec<ChangeHash> {
        self.crdt_doc.get_heads()
    }

    fn random_delta(&self) -> TextDelta {
        let text = self
            .current_file_content(&RelativePath::new(TEST_FILE_PATH))
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

    async fn send_to_editor_client(&mut self, editor_id: &EditorId, message: EditorProtocolObject) {
        let connection = self
            .editor_connections
            .get_mut(editor_id)
            .expect("Could not get editor handle");

        connection.1.send(message).await.unwrap_or_else(|err| {
            error!("Failed to send message to editor: {err} Removing editor.");
            self.editor_connections.remove(editor_id);
        });
    }

    fn write_files_changed_in_file_deltas(&mut self, file_deltas: &Vec<FileTextDelta>) {
        // Collect file paths into a set, so we don't write files multiple times on complex
        // patches.
        let mut file_paths = HashSet::new();
        for FileTextDelta { file_path, .. } in file_deltas {
            file_paths.insert(file_path);
        }

        for file_path in file_paths {
            self.write_file(file_path);
        }
    }

    fn write_file(&mut self, file_path: &RelativePath) {
        if let Ok(text) = self.current_file_content(file_path) {
            let abs_path = self.absolute_path_for_file_path(file_path);
            debug!("Writing to {abs_path}.");

            // If the file didn't exist before, log it.
            if !sandbox::exists(&self.base_dir, &abs_path)
                .expect("Failed to check for file existence before writing to it")
            {
                info!("Creating file {file_path}.");
            }

            sandbox::write_file(&self.base_dir, &abs_path, &text.into_bytes())
                .unwrap_or_else(|_| panic!("Could not write to file {abs_path}"));
        } else {
            warn!("Failed to get content of file '{file_path}' when writing to disk. Key should have existed?");
        }
    }

    // TODO: Join this code with the WalkBuilder in the sandbox module!
    fn build_walk(&mut self) -> Walk {
        let ignored_things = [".git", ".ethersync"];
        // TODO: How to deal with binary files?
        WalkBuilder::new(self.base_dir.clone())
            .standard_filters(true)
            .hidden(false)
            .require_git(false)
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
    }

    fn read_current_content_from_dir(&mut self, init: bool) {
        debug!("Reading current contents from disk (init: {init}).");
        self.build_walk()
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
                        let relative_file_path =
                            RelativePath::try_from_path(&self.base_dir, file_path)
                                .expect("Walked file path should be within base directory");
                        if self.owns(&relative_file_path) {
                            if let Ok(text) = String::from_utf8(bytes.clone()) {
                                if init {
                                    self.crdt_doc.initialize_text(&text, &relative_file_path);
                                } else {
                                    self.crdt_doc.update_text(&text, &relative_file_path);
                                }
                            } else {
                                self.crdt_doc.set_bytes(&bytes, &relative_file_path);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read file '{}': {e}", file_path.display());
                    }
                }
            });

        for relative_file_path in self.crdt_doc.files() {
            let absolute_file_path = self.absolute_path_for_file_path(&relative_file_path);
            if !sandbox::exists(&self.base_dir, &absolute_file_path)
                .expect(
                    "Should have been able to check for file existence while reading current directory content"
                )
                && self.owns(&relative_file_path)
            {
                warn!(
                        "File {relative_file_path} exists in the CRDT, but not on disk. Deleting from CRDT."
                    );
                self.remove_file(&relative_file_path);
            }
        }
        let _ = self.doc_changed_ping_tx.send(());
    }

    fn current_file_content(&self, file_path: &RelativePath) -> Result<String> {
        self.crdt_doc.current_file_content(file_path)
    }

    fn file_content_at(&self, file_path: &RelativePath, heads: &[ChangeHash]) -> Result<String> {
        self.crdt_doc.file_content_at(file_path, heads)
    }

    fn remove_file(&mut self, file_path: &RelativePath) {
        if self.owns(file_path) {
            self.crdt_doc.remove_file(file_path);
        } else {
            // TODO: Once we remove the concept of ownership entirely, make sure to send proper
            // ComponentMessagse to the editors that remove their entire content.
        }
    }

    /// Called when a component message is sent "into the core".
    /// Returns the component messages to send back to the editor that sent the component message.
    /// `from_editor` must be `None` if the component message originates from the "CRDT component".
    async fn process_component_message(
        &mut self,
        from_editor: Option<EditorId>,
        message: &ComponentMessage,
    ) -> Vec<ComponentMessage> {
        let mut to_editor = vec![];

        match message {
            ComponentMessage::Open { file_path, content } => {
                if let Ok(crdt_content) = self.current_file_content(file_path) {
                    // We want to compare the content sent along with the "open" with the content
                    // that's known to the CRDT.
                    let chunks = dissimilar::diff(content, &crdt_content);
                    if let [] | [dissimilar::Chunk::Equal(_)] = chunks.as_slice() {
                        // The contents match, nothing to do.
                    } else {
                        // The editor's content and the CRDT content differ. Update the editor to
                        // match.
                        let text_delta: TextDelta = chunks.into();
                        let update_message = ComponentMessage::Edit {
                            file_path: file_path.clone(),
                            delta: text_delta,
                        };

                        to_editor.push(update_message);
                    }
                } else {
                    // The file doesn't exist yet - create it in the Automerge document.
                    self.crdt_doc.initialize_text(content, file_path);
                    let _ = self.doc_changed_ping_tx.send(());
                    self.write_file(file_path);
                };
            }
            ComponentMessage::Close { file_path } => {
                self.write_file(file_path);
            }
            ComponentMessage::Edit { file_path, delta } => {
                self.crdt_doc.apply_delta_to_doc(delta, file_path);
                let _ = self.doc_changed_ping_tx.send(());
                self.write_file(file_path);
            }
            ComponentMessage::Cursor {
                cursor_id,
                cursor_state,
            } => {
                let next_sequence_number =
                    if let Some(old_cursor_state) = self.ephemeral_states.get_mut(cursor_id) {
                        old_cursor_state.sequence_number + 1
                    } else {
                        0
                    };

                let new_cursor_state = EphemeralMessage {
                    cursor_id: cursor_id.to_string(),
                    sequence_number: next_sequence_number,
                    cursor_state: cursor_state.clone(),
                };

                self.ephemeral_states
                    .insert(cursor_id.clone(), new_cursor_state.clone());

                let _ = self.ephemeral_message_tx.send(new_cursor_state);
            }
        }

        self.broadcast_to_editors(from_editor, message).await;

        to_editor
    }

    // Send component message to all editors, excluding `exlude_id`.
    async fn broadcast_to_editors(
        &mut self,
        exclude_id: Option<EditorId>,
        message: &ComponentMessage,
    ) {
        let editor_ids: Vec<EditorId> = self.editor_connections.keys().cloned().collect();
        for editor_id in editor_ids {
            if Some(editor_id) == exclude_id {
                continue;
            }

            self.send_to_editor(editor_id, message).await;
        }
    }

    // Returns the protocol messages that should be sent to the editor.
    fn process_in_editor(
        &mut self,
        editor_id: EditorId,
        messages: Vec<ComponentMessage>,
    ) -> Vec<EditorProtocolMessageToEditor> {
        let mut all_responses = vec![];
        let connection = &mut self
            .editor_connections
            .get_mut(&editor_id)
            .expect("Could not get editor connection")
            .0;

        for message in messages {
            let mut responses = connection.message_from_inside(&message);
            all_responses.append(&mut responses);
        }

        all_responses
    }

    async fn send_to_editor(&mut self, editor_id: EditorId, message: &ComponentMessage) {
        let messages_to_editor = self.process_in_editor(editor_id, vec![message.clone()]);

        for message_to_editor in messages_to_editor {
            self.send_to_editor_client(
                &editor_id,
                EditorProtocolObject::Request(message_to_editor),
            )
            .await;
        }
    }

    async fn react_to_ephemeral_message(&mut self, new_ephemeral_message: EphemeralMessage) {
        let cursor_id = new_ephemeral_message.cursor_id.clone();
        if let Some(existing_state) = self.ephemeral_states.get_mut(&cursor_id) {
            if new_ephemeral_message.sequence_number <= existing_state.sequence_number {
                // We've already seen a newer ephemeral message for this cursor_id, thus ignoring
                // this older one.
                return;
            }
        }
        self.ephemeral_states
            .insert(cursor_id.clone(), new_ephemeral_message.clone());

        // Broadcast to peers.
        let _ = self
            .ephemeral_message_tx
            .send(new_ephemeral_message.clone());

        // Broadcast to editors.
        self.broadcast_to_editors(
            None,
            &ComponentMessage::Cursor {
                cursor_id: new_ephemeral_message.cursor_id,
                cursor_state: new_ephemeral_message.cursor_state,
            },
        )
        .await;
    }

    async fn maybe_delete_cursor_position(&mut self, cursor_id: &CursorId) {
        let message = ComponentMessage::Cursor {
            cursor_id: cursor_id.clone(),
            cursor_state: CursorState {
                name: None,
                // NOTE: The "cursor" message doesn't have a specific
                // deletion mechanism. We get around this by setting it to an empty file path,
                // which means it will disappear from any previous file path.
                file_path: RelativePath::new(""),
                ranges: vec![],
            },
        };

        self.process_component_message(None, &message).await;

        self.ephemeral_states.remove(cursor_id);
    }

    async fn run(&mut self) {
        while let Some(message) = self.doc_message_rx.recv().await {
            self.handle_message(message).await;
        }
        debug!("Channel towards document handle has been closed (probably shutting down).");
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
    ephemeral_message_tx: EphemeralMessageSender,
    next_id: Arc<AtomicUsize>,
}

impl DocumentActorHandle {
    pub fn new(base_dir: &Path, init: bool, is_host: bool, persist: bool) -> Self {
        // The document task will receive messages on this channel.
        let (doc_message_tx, doc_message_rx) = mpsc::channel(1);

        // The document task will send a ping on this channel whenever it changes.
        // The sync tasks will subscribe to it, and react to it by syncing with the peers.
        let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(1);

        // The document actor will send ephemeral messages for other peers to this channel.
        let (ephemeral_message_tx, _ephemeral_message_rx) =
            broadcast::channel::<EphemeralMessage>(100);

        let mut actor = DocumentActor::new(
            doc_message_rx,
            doc_changed_ping_tx.clone(),
            ephemeral_message_tx.clone(),
            base_dir.into(),
            init,
            is_host,
            persist,
        );

        tokio::spawn(async move { actor.run().await });

        Self {
            doc_message_tx,
            doc_changed_ping_tx,
            ephemeral_message_tx,
            next_id: Arc::default(),
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

    pub fn subscribe_ephemeral_messages(&self) -> EphemeralMessageReceiver {
        self.ephemeral_message_tx.subscribe()
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

    pub fn next_editor_id(&self) -> EditorId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

pub struct Daemon {
    pub document_handle: DocumentActorHandle,
    pub address: String,
    socket_path: PathBuf,
    base_dir: PathBuf,
    #[allow(dead_code)]
    // We need to store the connection manager in order to keep the connection alive.
    connection_manager: peer::ConnectionManager,
}

impl Daemon {
    // Launch the daemon. Optionally, connect to given peer.
    pub async fn new(
        app_config: config::AppConfig,
        editor: Box<dyn Editor>,
        base_dir: &Path,
        init: bool,
        persist: bool,
    ) -> Result<Self> {
        let is_host = app_config.is_host();

        let document_handle = DocumentActorHandle::new(base_dir, init, is_host, persist);

        // Start socket listener.
        editor.spawn_socket_listener(document_handle.clone()).expect("Failed to start socket listener");
        // Start file watcher.
        let base_dir = base_dir.to_path_buf();
        spawn_file_watcher(&base_dir, document_handle.clone()).await;

        if persist {
            // Start persister.
            spawn_persister(document_handle.clone()).await;
        }

        // Start connection manager.
        let connection_manager = peer::ConnectionManager::new(document_handle.clone(), &base_dir)
            .await
            .expect("Failed to start connection manager");
        let address = connection_manager.secret_address();

        if app_config.emit_secret_address {
            info!(
            "\n\n\tOthers can connect by putting the following secret address in their .ethersync/config:\n\n\t{}\n",
            address
            );
        }
        if app_config.emit_join_code {
            put_secret_address_into_wormhole(&address).await;
        }
        if let Some(config::Peer::SecretAddress(secret_address)) = app_config.peer {
            connection_manager
                .connect(secret_address)
                .await
                .expect("Failed to connect to specified peer");
        }
        let socket_path = editor.get_socket_path().clone();
        Ok(Self {
            document_handle,
            address,
            socket_path,
            base_dir,
            connection_manager,
        })
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        debug!("Daemon dropped, removing socket");
        sandbox::remove_file(Path::new(&self.base_dir), &self.socket_path)
            .expect("Could not remove socket");
    }
}

// Spawn a file watcher and feed its events to the document_handle.
// In addition, a short timeout after the last event, do a full re-scan, so that we don't miss any
// file changes - the watcher isn't necessarily exhaustive.
async fn spawn_file_watcher(base_dir: &Path, document_handle: DocumentActorHandle) {
    let mut watcher = Watcher::new(base_dir);

    tokio::spawn(async move {
        let debounce_duration = Duration::from_millis(100);

        let debounce_timer = tokio::time::sleep(debounce_duration);
        // Sleep does not implement the Unpin trait, so in order to use it with select!, we have to
        // pin it first (according to the documentation https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html).
        tokio::pin!(debounce_timer);

        let mut rescan_required = false;

        loop {
            tokio::select! {
                maybe_event = watcher.next() => {
                    if let Some(watcher_event) = maybe_event {
                        document_handle
                            .send_message(DocMessage::FromWatcher(watcher_event))
                            .await;

                        debounce_timer.as_mut().reset(tokio::time::Instant::now() + debounce_duration);
                        rescan_required = true;
                    } else {
                        // Watcher terminated. Seems we're shutting down.
                        return;
                    }
                }

                _ = &mut debounce_timer, if rescan_required => {
                    document_handle
                        .send_message(DocMessage::RescanFiles)
                        .await;
                    rescan_required = false;
                }
            }
        }
    });
}

async fn spawn_persister(document_handle: DocumentActorHandle) {
    tokio::spawn(async move {
        let mut doc_changed_ping_rx = document_handle.subscribe_document_changes();

        document_handle.send_message(DocMessage::Persist).await;

        loop {
            match doc_changed_ping_rx.recv().await {
                Ok(()) => {
                    // The document has changed.
                }
                Err(broadcast::error::RecvError::Closed) => {
                    panic!("Doc changed channel has been closed");
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // This is fine, the messages in this channel are just pings.
                    // It's fine if we miss some.
                    debug!("Doc changed ping channel lagged (this is probably fine).");
                }
            }

            document_handle.send_message(DocMessage::Persist).await;

            // Alternatively to sleeping, we could use a "back channel" in the Persist
            // message, so that the daemon tells us when it's done persisting.
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    //use crate::types::factories::*;

    mod document_actor {
        use super::*;
        use temp_dir::TempDir;
        //use tracing_test::traced_test;

        impl DocumentActor {
            // TODO: Refactor, to reuse stuff from DocumentActorHandle constructor.
            fn setup_for_testing(directory: &TempDir) -> Self {
                // The document task will receive messages on this channel.
                let (_doc_message_tx, doc_message_rx) = mpsc::channel(1);

                // The document task will send a ping on this channel whenever it changes.
                // The sync tasks will subscribe to it, and react to it by syncing with the peers.
                let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(1);

                // The document actor will send ephemeral messages for other peers to this channel.
                let (ephemeral_message_tx, _ephemeral_message_rx) =
                    broadcast::channel::<EphemeralMessage>(100);

                DocumentActor::new(
                    doc_message_rx,
                    doc_changed_ping_tx.clone(),
                    ephemeral_message_tx.clone(),
                    directory.path().to_path_buf(),
                    true,
                    true,
                    false,
                )
            }
            fn assert_file_content(&self, file_path: &RelativePath, content: &str) {
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
            let mut actor = DocumentActor::setup_for_testing(&dir);

            actor.read_current_content_from_dir(true);

            actor.assert_file_content(&RelativePath::new("file1"), "content1");
            actor.assert_file_content(&RelativePath::new("file2"), "content2");
            actor.assert_file_content(&RelativePath::new("sub/file3"), "content3");
        }
    }
}
