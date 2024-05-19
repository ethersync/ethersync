use crate::connect;
use crate::ot::OTServer;
use crate::types::{EditorProtocolMessage, EditorTextDelta, RevisionedEditorTextDelta, TextDelta};
use anyhow::Result;
use automerge::{
    patches::TextRepresentation,
    sync::{Message as AutomergeSyncMessage, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, Patch, PatchLog, ReadDoc,
};
use rand::Rng;
use std::fmt;
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf},
    net::TcpStream,
    net::{UnixListener, UnixStream},
    sync::{broadcast, mpsc, oneshot},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

// These messages are sent to the task that owns the document.
pub enum DocMessage {
    GetContent {
        response_tx: oneshot::Sender<Result<String>>,
    },
    Open,
    Close,
    RandomEdit,
    RevDelta(RevisionedEditorTextDelta),
    Delta(TextDelta),
    ReceiveSyncMessage {
        message: AutomergeSyncMessage,
        state: SyncState,
        response_tx: oneshot::Sender<SyncState>,
    },
    GenerateSyncMessage {
        state: SyncState,
        response_tx: oneshot::Sender<(SyncState, Option<AutomergeSyncMessage>)>,
    },
}

impl fmt::Debug for DocMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            DocMessage::GetContent { .. } => "get content",
            DocMessage::Open => "open",
            DocMessage::Close => "close",
            DocMessage::RandomEdit => "random edit",
            DocMessage::RevDelta(_) => "delta from editor",
            DocMessage::Delta(_) => "delta from peer",
            DocMessage::ReceiveSyncMessage { .. } => "<automerge internal sync rcv>",
            DocMessage::GenerateSyncMessage { .. } => "<automerge internal sync gen>",
        };
        write!(f, "{repr}")
    }
}

impl From<EditorProtocolMessage> for DocMessage {
    fn from(rpc_message: EditorProtocolMessage) -> Self {
        match rpc_message {
            EditorProtocolMessage::Open { uri: _ } => DocMessage::Open,
            EditorProtocolMessage::Close { uri: _ } => DocMessage::Close,
            EditorProtocolMessage::Edit { uri: _, delta } => DocMessage::RevDelta(delta),
        }
    }
}

// These messages are sent to tasks that own peer sync states.
enum SyncerMessage {
    ReceiveSyncMessage { message: Vec<u8> },
    GenerateSyncMessage,
}

type DocMessageSender = mpsc::Sender<DocMessage>;
type DocChangedSender = broadcast::Sender<()>;
type DocChangedReceiver = broadcast::Receiver<()>;

type EditorMessageSender = mpsc::Sender<RevisionedEditorTextDelta>;
type EditorMessageReceiver = mpsc::Receiver<RevisionedEditorTextDelta>;

type SyncerMessageSender = mpsc::Sender<SyncerMessage>;
type SyncerMessageReceiver = mpsc::Receiver<SyncerMessage>;

/// Encapsulates the Automerge `AutoCommit` and provides a generic interface,
/// s.t. we don't need to worry about automerge internals elsewhere.
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

    fn receive_sync_message(&mut self, message: AutomergeSyncMessage, peer_state: &mut SyncState) {
        self.doc
            .sync()
            .receive_sync_message(peer_state, message)
            .expect("Failed to apply sync message to Automerge document");
    }

    fn generate_sync_message(
        &mut self,
        peer_state: &mut SyncState,
    ) -> Option<AutomergeSyncMessage> {
        self.doc.sync().generate_sync_message(peer_state)
    }

    fn text_obj(&self) -> Result<automerge::ObjId> {
        let text_obj = self
            .doc
            .get(automerge::ROOT, "text")
            .expect("Failed to get text key from Automerge document");
        if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
            Ok(text_obj)
        } else {
            Err(anyhow::anyhow!(
                "Automerge document doesn't have a text object, so I can't provide it"
            ))
        }
    }

    fn apply_delta_to_doc(&mut self, delta: &EditorTextDelta) {
        let text_obj = self
            .text_obj()
            .expect("Couldn't get automerge text object, so not able to modify it");
        let mut offset = 0i32;
        let text = self
            .current_content()
            .expect("Should have initialized text before performing random edit");
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

    fn current_content(&self) -> Result<String> {
        self.text_obj().map(|to| {
            self.doc
                .text(to)
                .expect("Failed to get string from Automerge text object")
        })
    }

    fn initialize_text(&mut self, text: &str) {
        let text_obj = self
            .doc
            .put_object(automerge::ROOT, "text", ObjType::Text)
            .expect("Failed to initialize text object in Automerge document");
        self.doc
            .splice_text(text_obj, 0, 0, text)
            .expect("Failed to splice text into Automerge text object");
    }
}

/// This Actor is responsible for applying changes to the document asynchronously.
///
/// Any DocMessage that is emitted via DocumentActorHandle should have an effect eventually.
pub struct DocumentActor {
    doc_message_rx: mpsc::Receiver<DocMessage>,
    doc_changed_ping_tx: DocChangedSender,
    socket_message_tx: EditorMessageSender,
    /// If we have an ot_server, it means that an editor is connected.
    ot_server: Option<OTServer>,
    /// The Document is the main I/O managed resource of this actor.
    crdt_doc: Document,
    file_path: PathBuf,
}

impl DocumentActor {
    #[must_use]
    fn new(
        doc_message_rx: mpsc::Receiver<DocMessage>,
        doc_changed_ping_tx: DocChangedSender,
        socket_message_tx: EditorMessageSender,
        file_path: PathBuf,
    ) -> Self {
        Self {
            doc_message_rx,
            doc_changed_ping_tx,
            socket_message_tx,
            file_path,
            ot_server: None,
            crdt_doc: Document::default(),
        }
    }
    async fn handle_message(&mut self, message: DocMessage) {
        // TODO: Show the type in the debug message, or implement Debug for DocMessage.
        debug!("Handling doc message: {message:?}");
        match message {
            DocMessage::GetContent { response_tx } => {
                response_tx
                    .send(self.current_content())
                    .expect("Failed to send content to response channel");
            }
            DocMessage::Open => {
                self.ot_server = Some(OTServer::new(
                    self.current_content()
                        .expect("Should have initialized text before initializing the document"),
                ));
            }
            DocMessage::Close => {
                self.ot_server = None;
            }
            DocMessage::RandomEdit => {
                let delta = self.random_delta();
                let text = self
                    .current_content()
                    .expect("Should have initialized text before performing random edit");
                let ed_delta = EditorTextDelta::from_delta(delta.clone(), &text);
                self.apply_delta_to_doc(&ed_delta);
                self.process_crdt_delta_in_ot(delta).await;
            }
            DocMessage::Delta(delta) => {
                let text = self
                    .current_content()
                    .expect("Should have initialized text before performing random edit");
                let editor_delta = EditorTextDelta::from_delta(delta.clone(), &text);
                self.apply_delta_to_doc(&editor_delta);
                self.process_crdt_delta_in_ot(delta).await;
            }
            DocMessage::RevDelta(rev_delta) => {
                debug!("Handling RevDelta from editor: {:#?}", rev_delta);
                let (editor_delta_for_crdt, rev_deltas_for_editor) =
                    self.apply_delta_to_ot(rev_delta);

                self.apply_delta_to_doc(&editor_delta_for_crdt);
                self.send_deltas_to_editor(rev_deltas_for_editor).await;
            }
            DocMessage::ReceiveSyncMessage {
                message,
                state: mut peer_state,
                response_tx,
            } => {
                if let Some(patches) = self.apply_sync_message_to_doc(message, &mut peer_state) {
                    self.process_crdt_patches_in_ot(patches).await;
                }
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
        }
    }

    fn apply_sync_message_to_doc(
        &mut self,
        message: AutomergeSyncMessage,
        peer_state: &mut SyncState,
    ) -> Option<Vec<Patch>> {
        let result = if self.ot_server.is_some() {
            let patches = self
                .crdt_doc
                .receive_sync_message_log_patches(message, peer_state);
            Some(patches)
        } else {
            self.crdt_doc.receive_sync_message(message, peer_state);
            None
        };
        let _ = self.doc_changed_ping_tx.send(());
        result
    }

    async fn send_deltas_to_editor(&self, rev_deltas: Vec<RevisionedEditorTextDelta>) {
        for rev_delta in rev_deltas {
            debug!("Sending RevDelta to socket: {:#?}", rev_delta);

            self.socket_message_tx
                .send(rev_delta)
                .await
                .expect("Failed to send message to socket channel.");
        }
    }

    fn apply_delta_to_ot(
        &mut self,
        rev_editor_delta: RevisionedEditorTextDelta,
    ) -> (EditorTextDelta, Vec<RevisionedEditorTextDelta>) {
        let text = self
            .current_content()
            .expect("Should have initialized text before performing random edit");
        let ot_server = self
            .ot_server
            .as_mut()
            .expect("No editor connected, where does this delta come from?");
        let (delta_for_crdt, rev_deltas_for_editor) =
            ot_server.apply_editor_operation(rev_editor_delta);

        let editor_delta_for_crdt = EditorTextDelta::from_delta(delta_for_crdt, &text);
        (editor_delta_for_crdt, rev_deltas_for_editor)
    }

    fn random_delta(&self) -> TextDelta {
        let text = self
            .current_content()
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
        let mut deletion_length = 0;
        if (text_length - random_position) > 0 {
            deletion_length = rand::thread_rng().gen_range(0..(text_length - random_position));
            deletion_length = deletion_length.min(3);
        }
        delta.delete(deletion_length);

        delta
    }

    async fn process_crdt_patches_in_ot(&mut self, patches: Vec<Patch>) {
        debug!(?patches);
        for patch in patches {
            match patch.action.try_into() {
                Ok(delta) => {
                    self.process_crdt_delta_in_ot(delta).await;
                }
                Err(e) => {
                    warn!("Failed to convert patch to delta: {:#?}", e);
                }
            }
        }
    }

    async fn process_crdt_delta_in_ot(&mut self, delta: TextDelta) {
        if let Some(ot_server) = &mut self.ot_server {
            let rev_text_delta_for_editor = ot_server.apply_crdt_change(delta);
            self.socket_message_tx
                .send(rev_text_delta_for_editor)
                .await
                .expect("Failed to send message to socket channel.");
        }
    }

    fn write_current_content_to_file(&mut self) {
        let content = self.current_content();
        if let Ok(text) = content {
            debug!(current_text__ = text);
            if let Some(ot_server) = &mut self.ot_server {
                debug!(current_ot_doc = ot_server.current_content());
            } else {
                std::fs::write(&self.file_path, &text).expect("Could not write to file");
            }
        }
    }

    /// Reading in the file is a preparatory step, before kicking off the actor.
    fn read_current_content_from_file(&mut self) {
        // Create the file if it doesn't exist.
        if !self.file_path.exists() {
            std::fs::write(&self.file_path, "").expect("Could not create file");
        }

        if let Ok(text) = std::fs::read_to_string(&self.file_path) {
            self.crdt_doc.initialize_text(&text);
        } else {
            // TODO: Look at *why* we couldn't read the file.
            panic!("Could not read file {}", self.file_path.display());
        }
    }

    fn current_content(&self) -> Result<String> {
        self.crdt_doc.current_content()
    }

    fn apply_delta_to_doc(&mut self, delta: &EditorTextDelta) {
        self.crdt_doc.apply_delta_to_doc(delta);
        let _ = self.doc_changed_ping_tx.send(());
    }

    async fn run(&mut self) {
        while let Some(message) = self.doc_message_rx.recv().await {
            self.handle_message(message).await;
            self.write_current_content_to_file();
        }
        panic!("Channel towards document task has been closed");
    }
}

/// This handle knows how to talk to the DocumentActor and provides an interface for doing so.
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
    pub fn new(
        socket_message_tx: mpsc::Sender<RevisionedEditorTextDelta>,
        file_path: &Path,
        host: bool,
    ) -> Self {
        // The document task will receive messages on this channel.
        let (doc_message_tx, doc_message_rx) = mpsc::channel(1);

        // The document task will send a ping on this channel whenever it changes.
        // The sync tasks will subscribe to it, and react to it by syncing with the peers.
        let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(1);

        let mut actor = DocumentActor::new(
            doc_message_rx,
            doc_changed_ping_tx.clone(),
            socket_message_tx.clone(),
            file_path.into(),
        );

        // Initialize the text from the file_path, if this is the document owned by the host.
        if host {
            actor.read_current_content_from_file();
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
            .expect("DocumentActor task has been killed")
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

    pub async fn apply_delta(&mut self, delta: TextDelta) {
        let message = DocMessage::Delta(delta);
        self.doc_message_tx
            .send(message)
            .await
            .expect("Failed to send delta to document task");
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
        file_path: &Path,
    ) -> Self {
        // The document task will send messages intended for the socket connection on this channel.
        let (socket_message_tx, socket_message_rx) = mpsc::channel::<RevisionedEditorTextDelta>(1);

        // If the peer address is empty, we're the host.
        let is_host = peer.is_none();

        let document_handle =
            DocumentActorHandle::new(socket_message_tx.clone(), file_path, is_host);

        let connection_document_handle = document_handle.clone();
        tokio::spawn(async move {
            connect::make_connection(port, peer, connection_document_handle).await;
        });

        let socket_path_clone = socket_path.to_path_buf();
        let file_path_clone = file_path.to_path_buf();
        let client_document_handle = document_handle.clone();
        tokio::spawn(async move {
            listen_socket(
                client_document_handle,
                socket_message_rx,
                &socket_path_clone,
                file_path_clone,
            )
            .await
            .expect("Failed to listen on UNIX socket");
        });

        Self { document_handle }
    }
}

/// Reads from a TCP stream and forwards it to the Syncer
struct TCPReadActor {
    sync_handle: SyncActorHandle,
    reader: ReadHalf<TcpStream>,
    shutdown_token: CancellationToken,
}

impl TCPReadActor {
    fn new(
        reader: ReadHalf<TcpStream>,
        sync_handle: SyncActorHandle,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            sync_handle,
            reader,
            shutdown_token,
        }
    }

    async fn forward_sync_message(&self, message: Vec<u8>) {
        self.sync_handle
            .send(SyncerMessage::ReceiveSyncMessage { message })
            .await
    }

    async fn read_message(&mut self) -> Result<Vec<u8>> {
        let mut message_len_buf = [0; 4];
        self.reader.read_exact(&mut message_len_buf).await?;
        let message_len = i32::from_be_bytes(message_len_buf);
        let mut message_buf = vec![0; message_len as usize];
        self.reader.read_exact(&mut message_buf).await?;
        Ok(message_buf)
    }

    async fn run(&mut self) {
        while let Ok(message) = self.read_message().await {
            self.forward_sync_message(message).await;
        }
        info!("Sync Receive loop stopped (peer disconnected)");
        self.shutdown_token.cancel()
    }
}

struct TCPWriteActor {
    writer: WriteHalf<TcpStream>,
    automerge_message_receiver: mpsc::Receiver<AutomergeSyncMessage>,
}

impl TCPWriteActor {
    fn new(
        writer: WriteHalf<TcpStream>,
        automerge_message_receiver: mpsc::Receiver<AutomergeSyncMessage>,
    ) -> Self {
        Self {
            writer,
            automerge_message_receiver,
        }
    }

    async fn run(&mut self) {
        while let Some(message) = self.automerge_message_receiver.recv().await {
            // TODO: move encode to Syncer for symmetry?
            let message = message.encode();
            let message_len = message.len() as i32;
            self.writer
                .write_all(&message_len.to_be_bytes())
                .await
                .expect("GenerateSyncMessage: write message len failed");
            self.writer
                .write_all(&message)
                .await
                .expect("GenerateSyncMessage: write message failed");
        }
        // At this point, our channel has been closed, which is the signal for us to stop.
        debug!("TCPWriteActor stopped (channel closed)");
    }
}

struct SyncActor {
    syncer_receiver: SyncerMessageReceiver,
    document_handle: DocumentActorHandle,
    tcp_handle: TCPActorHandle,
    peer_state: SyncState,
}

impl SyncActor {
    fn new(
        syncer_receiver: SyncerMessageReceiver,
        document_handle: DocumentActorHandle,
        tcp_handle: TCPActorHandle,
    ) -> Self {
        Self {
            syncer_receiver,
            document_handle,
            tcp_handle,
            peer_state: SyncState::new(),
        }
    }

    async fn handle_message(&mut self, message: SyncerMessage) {
        match message {
            SyncerMessage::ReceiveSyncMessage { message } => {
                let (reponse_tx, response_rx) = oneshot::channel();
                let message = AutomergeSyncMessage::decode(&message)
                    .expect("Failed to decode automerge message");
                self.document_handle
                    .send_message(DocMessage::ReceiveSyncMessage {
                        message,
                        state: mem::take(&mut self.peer_state),
                        response_tx: reponse_tx,
                    })
                    .await;
                self.peer_state = response_rx
                    .await
                    .expect("Couldn't read response from Document channel");
            }
            SyncerMessage::GenerateSyncMessage {} => {
                let (reponse_tx, response_rx) = oneshot::channel();
                self.document_handle
                    .send_message(DocMessage::GenerateSyncMessage {
                        state: mem::take(&mut self.peer_state),
                        response_tx: reponse_tx,
                    })
                    .await;
                let (ps, message) = response_rx
                    .await
                    .expect("Could not read response from Document channel");
                self.peer_state = ps;
                if let Some(message) = message {
                    self.tcp_handle.send(message).await;
                }
            }
        }
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                _ = self.tcp_handle.shutdown_token.cancelled() => {
                    debug!("Shutting down main start_sync loop");
                    break;
                }
                Some(message) = self.syncer_receiver.recv() => {
                    self.handle_message(message).await;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct SyncActorHandle {
    syncer_message_tx: SyncerMessageSender,
}

impl SyncActorHandle {
    pub fn new(document_handle: DocumentActorHandle, tcp_handle: TCPActorHandle) -> Self {
        let (syncer_message_tx, syncer_message_rx) = mpsc::channel(16);

        // Sync actor.
        let syncer = SyncActor::new(
            syncer_message_rx,
            document_handle.clone(),
            tcp_handle.clone(),
        );
        tokio::spawn(syncer.run());

        // Generate sync message when doc changes.
        let shutdown_token_clone = tcp_handle.shutdown_token.clone();
        let mut doc_changed_ping_rx = document_handle.subscribe_document_changes();
        let syncer_message_tx_clone = syncer_message_tx.clone();

        // TODO: can we explain here, why this forwarding is necessary?
        tokio::spawn(async move {
            loop {
                syncer_message_tx_clone
                    .send(SyncerMessage::GenerateSyncMessage {})
                    .await
                    .expect("Failed to send GenerateSyncMessage to document task");
                tokio::select! {
                    _ = shutdown_token_clone.cancelled() => {
                        debug!("Stopping GenerateSyncMessage ping forwarding.");
                        break;
                    }
                    doc_ping = doc_changed_ping_rx.recv() => match doc_ping {
                        Ok(()) => {
                            debug!("Doc changed.");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            panic!("Doc changed channel has been closed");
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // This is fine, the messages in this channel are just pings.
                            // It's okay if we miss some.
                        }
                    }
                }
            }
        });

        Self { syncer_message_tx }
    }

    async fn send(&self, message: SyncerMessage) {
        self.syncer_message_tx
            .send(message)
            .await
            .expect("Channel closed (TODO)")
    }
}

#[derive(Clone)]
pub struct TCPActorHandle {
    automerge_message_tx: mpsc::Sender<AutomergeSyncMessage>,
    shutdown_token: CancellationToken,
}

/// The TCP statemachine works as follows:
/// - if we're the host,
/// - if we're a peer, we
///
/// How do other parts of the code communicate with TCP? Through this handle.
/// What can be communicated?
impl TCPActorHandle {
    async fn send(&mut self, message: AutomergeSyncMessage) {
        self.automerge_message_tx
            .send(message)
            .await
            .expect("Channel to TCPActor(s) closed.");
    }

    pub fn start_sync(stream: TcpStream, sync_handle: oneshot::Receiver<SyncActorHandle>) -> Self {
        let shutdown_token = CancellationToken::new();

        let read_shutdown_token = shutdown_token.clone();
        let (tcp_read, tcp_write) = tokio::io::split(stream);
        let (automerge_message_tx, automerge_message_rx) = mpsc::channel(16);
        tokio::spawn(async move {
            let sync_handle = match sync_handle.await {
                Ok(my_handle) => my_handle,
                Err(_) => return,
            };
            let mut receiver = TCPReadActor::new(tcp_read, sync_handle, read_shutdown_token);
            tokio::spawn(async move {
                receiver.run().await;
            });
            let mut writer = TCPWriteActor::new(tcp_write, automerge_message_rx);
            tokio::spawn(async move {
                writer.run().await;
            });
        });
        Self {
            automerge_message_tx,
            shutdown_token,
        }
    }
}

struct SocketReadActor {
    reader: ReadHalf<UnixStream>,
    shutdown_token: CancellationToken,
    document_handle: DocumentActorHandle,
}

impl SocketReadActor {
    fn new(
        reader: ReadHalf<UnixStream>,
        shutdown_token: CancellationToken,
        document_handle: DocumentActorHandle,
    ) -> Self {
        Self {
            reader,
            shutdown_token,
            document_handle,
        }
    }

    async fn run(&mut self) {
        let buf_reader = BufReader::new(&mut self.reader);
        let mut lines = buf_reader.lines();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    debug!("Got a line from the client: {:#?}", line);
                    let jsonrpc = EditorProtocolMessage::from_jsonrpc(&line)
                        .expect("Failed to parse JSON-RPC message");
                    self.document_handle.send_message(jsonrpc.into()).await;
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    error!("Error reading line: {:#?}", e);
                }
            }
        }
        self.shutdown_token.cancel();
        info!("Client disconnect.");
    }
}

struct SocketWriteActor<'a> {
    writer: WriteHalf<UnixStream>,
    shutdown_token: CancellationToken,
    editor_message_receiver: &'a mut EditorMessageReceiver,
    file_path: PathBuf,
}

impl<'a> SocketWriteActor<'a> {
    fn new(
        writer: WriteHalf<UnixStream>,
        editor_message_receiver: &'a mut EditorMessageReceiver,
        shutdown_token: CancellationToken,
        file_path: PathBuf,
    ) -> Self {
        Self {
            writer,
            editor_message_receiver,
            shutdown_token,
            file_path,
        }
    }

    async fn write_to_socket(&mut self, rev_delta: RevisionedEditorTextDelta) {
        debug!("Received editor message to send to it.");
        let message = EditorProtocolMessage::Edit {
            uri: format!("file://{}", self.file_path.display()),
            delta: rev_delta,
        };
        let payload = message
            .to_jsonrpc()
            .expect("Failed to serialize JSON-RPC message");
        debug!("Sending message to editor: {:#?}", payload);
        self.writer
            .write_all(format!("{payload}\n").as_bytes())
            .await
            .expect("Failed to write to socket");
    }

    async fn run(&mut self) {
        // We're sending an editor message to the client.
        loop {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => {
                    debug!("Shutting down JSON-RPC sender (due to socket disconnet)");
                    break;
                }
                editor_message_maybe = self.editor_message_receiver.recv() => match editor_message_maybe {
                    None => {
                        panic!("Editor message channel has been closed. How did this happen?");
                    }
                    Some(editor_message) => {
                        self.write_to_socket(editor_message).await;
                    }
                }
            }
        }
    }
}

async fn listen_socket(
    document_handle: DocumentActorHandle,
    mut editor_message_rx: EditorMessageReceiver,
    socket_path: &Path,
    file_path: PathBuf,
) -> Result<()> {
    if Path::new(&socket_path).exists() {
        fs::remove_file(socket_path)?;
    }
    let listener = UnixListener::bind(socket_path)?;
    info!("Listening on UNIX socket: {}", socket_path.display());

    loop {
        let shutdown_token = CancellationToken::new();

        // TODO: Accept multiple connections.
        match listener.accept().await {
            Ok((stream, _addr)) => {
                info!("Client connection established.");

                let (stream_read, stream_write) = tokio::io::split(stream);

                // We're parsing a line from the reader (if we have one)
                // which means we got a Delta from the ethersync client.

                let mut reader = SocketReadActor::new(
                    stream_read,
                    shutdown_token.clone(),
                    document_handle.clone(),
                );
                tokio::spawn(async move { reader.run().await });

                let mut writer = SocketWriteActor::new(
                    stream_write,
                    &mut editor_message_rx,
                    shutdown_token,
                    file_path.clone(),
                );
                writer.run().await;
            }
            Err(e) => {
                error!("listen_socket Error: {:#?}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod document {
        use super::*;
        use crate::types::factories::*;

        #[test]
        fn can_initialize_content() {
            let mut document = Document::default();
            let text = "To be or not to be, that is the question".to_string();

            document.initialize_text(&text);

            // unfortunately anyhow::Error doesn't implement PartialEq, so we'll rather unwrap.
            assert_eq!(document.current_content().unwrap(), text);
        }

        fn apply_delta_to_doc_works(initial: &str, ed_delta: &EditorTextDelta, expected: &str) {
            let mut document = Document::default();
            document.initialize_text(initial);
            document.apply_delta_to_doc(ed_delta);

            // unfortunately anyhow::Error doesn't implement PartialEq, so we'll rather unwrap.
            assert_eq!(document.current_content().unwrap(), expected);
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
    }
}
