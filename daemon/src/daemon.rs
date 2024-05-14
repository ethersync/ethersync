#![allow(dead_code)]
use crate::ot::OTServer;
use crate::types::{EditorProtocolMessage, EditorTextDelta, RevisionedEditorTextDelta, TextDelta};
use anyhow::Result;
use automerge::{
    patches::TextRepresentation,
    sync::{Message, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, Patch, PatchLog, ReadDoc,
};
use local_ip_address::local_ip;
use rand::Rng;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf},
    net::UnixListener,
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, oneshot},
    time::{sleep, Duration},
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
    Debug, // TODO: Find a better way to drop debug messages from the editor.
    RandomEdit,
    RevDelta(RevisionedEditorTextDelta),
    #[allow(dead_code)]
    Delta(TextDelta),
    ReceiveSyncMessage {
        message: Message,
        state: SyncState,
        response_tx: oneshot::Sender<SyncState>,
    },
    GenerateSyncMessage {
        state: SyncState,
        response_tx: oneshot::Sender<(SyncState, Option<Message>)>,
    },
}

impl fmt::Debug for DocMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            DocMessage::GetContent { .. } => "get content",
            DocMessage::Open => "open",
            DocMessage::Close => "close",
            DocMessage::Debug => "debug",
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
            EditorProtocolMessage::Debug(_) => DocMessage::Debug,
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

type EditorMessageSender = mpsc::Sender<RevisionedEditorTextDelta>;
type EditorMessageReceiver = mpsc::Receiver<RevisionedEditorTextDelta>;

type SyncerMessageSender = mpsc::Sender<SyncerMessage>;

/// Encapsulates the Automerge `AutoCommit` and provides a generic interface,
/// s.t. we don't need to worry about automerge internals elsewhere.
#[derive(Debug, Default)]
pub struct Document {
    doc: AutoCommit,
}

impl Document {
    fn receive_sync_message_log_patches(
        &mut self,
        message: Message,
        peer_state: &mut SyncState,
    ) -> Vec<Patch> {
        let mut patch_log = PatchLog::active(TextRepresentation::String);
        self.doc
            .sync()
            .receive_sync_message_log_patches(peer_state, message, &mut patch_log)
            .expect("Failed to apply sync message to Automerge document");
        self.doc.make_patches(&mut patch_log)
    }

    fn receive_sync_message(&mut self, message: Message, peer_state: &mut SyncState) {
        self.doc
            .sync()
            .receive_sync_message(peer_state, message)
            .expect("Failed to apply sync message to Automerge document");
    }

    fn generate_sync_message(&mut self, peer_state: &mut SyncState) -> Option<Message> {
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

pub struct DaemonActor {
    doc_message_rx: mpsc::Receiver<DocMessage>,
    doc_changed_ping_tx: DocChangedSender,
    socket_message_tx: EditorMessageSender,
    /// if we have an ot_server, it means that an editor is connected
    ot_server: Option<OTServer>,
    crdt_doc: Document,
    file_path: PathBuf,
}

impl DaemonActor {
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
            DocMessage::Debug => {
                // Ignore.
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
        message: Message,
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
                debug!(current_ot_doc = ot_server.apply_to_initial_content());
            } else {
                std::fs::write(&self.file_path, &text).expect("Could not write to file");
            }
        }
    }

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
            if let DocMessage::Debug = &message {
                // No need to do anything.
                continue;
            } else {
                self.handle_message(message).await;
                self.write_current_content_to_file();
            }
        }
        panic!("Channel towards document task has been closed");
    }
}

pub struct Daemon {
    doc_message_tx: DocMessageSender,
}

impl Daemon {
    // Launch the daemon. Optionally, connect to given peer.
    pub fn new(
        port: Option<u16>,
        peer: Option<String>,
        socket_path: &Path,
        file_path: &Path,
    ) -> Self {
        // The document task will receive messages on this channel.
        // The TCP and socket connections will send messages to it when they receive something.
        let (doc_message_tx, doc_message_rx) = mpsc::channel(1);

        // The document task will send a ping on this channel whenever it changes.
        // The sync tasks will subscribe to it, and react to it by syncing with the peers.
        let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(1);

        // The document task will send messages intended for the socket connection on this channel.
        let (socket_message_tx, socket_message_rx) = mpsc::channel::<RevisionedEditorTextDelta>(1);

        let mut daemon_actor = DaemonActor::new(
            doc_message_rx,
            doc_changed_ping_tx.clone(),
            socket_message_tx.clone(),
            file_path.into(),
        );

        // If we are the host, read file content.
        if peer.is_none() {
            daemon_actor.read_current_content_from_file();
        }

        // Make edits to the document occasionally.
        // To activate, build with --features simulate_edits_on_crdt
        if cfg!(feature = "simulate_edits_on_crdt") {
            let tx = doc_message_tx.clone();
            tokio::spawn(async move {
                sleep(Duration::from_secs(2)).await;
                loop {
                    tx.send(DocMessage::RandomEdit)
                        .await
                        .expect("Failed to send random edit");
                    sleep(Duration::from_secs(2)).await;
                }
            });
        }

        // Send random edits to editors occasionally.
        // To activate, build with --features simulate_edits_from_editor
        // TODO: this feature is currently be broken, so it's even commented out.
        // (mostly because it doesn't send a proper revision? also not the correct type.)
        /*
        if cfg!(feature="simulate_edits_from_editor") {
            let tx = socket_message_tx.clone();
            tokio::spawn(async move {
                let editor_revision = 0;
                loop {
                    let random_string: String = rand::thread_rng()
                        .sample_iter(&Alphanumeric)
                        .take(1)
                        .map(char::from)
                        .collect();
                    let random_position = 0; //rand::thread_rng().gen_range(0..(editor_revision + 1));
                    let message = EditorMessage::Insert {
                        editor_revision,
                        position: random_position,
                        text: random_string,
                    };
                    debug!(new_message = ?message);
                    tx.send(message).expect("Failed to send random insert");

                    sleep(Duration::from_secs(2)).await;
                }
            });
        }
        */

        // Dial peer, or listen for incoming connections.
        let doc_message_tx_clone = doc_message_tx.clone();
        let doc_changed_ping_tx_clone = doc_changed_ping_tx.clone();
        let doc_message_tx_clone_2 = doc_message_tx_clone.clone();

        if let Some(peer) = peer {
            tokio::spawn(async {
                dial_tcp(doc_message_tx_clone, doc_changed_ping_tx_clone, peer)
                    .await
                    .expect("Failed to dial peer");
            });
        } else {
            let port = port.unwrap_or(4242);
            tokio::spawn(async move {
                listen_tcp(doc_message_tx_clone, doc_changed_ping_tx_clone, port)
                    .await
                    .expect("Failed to listen on TCP port");
            });
        }

        let socket_path_clone = socket_path.to_path_buf();
        let file_path_clone = file_path.to_path_buf();
        tokio::spawn(async move {
            listen_socket(
                doc_message_tx_clone_2,
                socket_message_rx,
                &socket_path_clone,
                file_path_clone,
            )
            .await
            .expect("Failed to listen on UNIX socket");
        });

        tokio::spawn(async move { daemon_actor.run().await });
        Self { doc_message_tx }
    }

    pub async fn content(&self) -> Result<String> {
        let (send, recv) = oneshot::channel();
        let message = DocMessage::GetContent { response_tx: send };
        // Ignore send errors, because recv.await will fail anyway.
        let _ = self.doc_message_tx.send(message).await;
        recv.await.expect("DaemonActor task has been killed")
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

async fn listen_tcp(
    tx: DocMessageSender,
    doc_changed_ping_tx: DocChangedSender,
    port: u16,
) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    if let Ok(ip) = local_ip() {
        info!("Listening on local TCP: {}:{}", ip, port);
    }

    if let Some(ip) = public_ip::addr().await {
        info!("Listening on public TCP: {}:{}", ip, port);
    }

    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            error!("Error accepting connection.");
            continue;
        };

        let tx = tx.clone();
        let doc_changed_ping_tx = doc_changed_ping_tx.clone();
        tokio::spawn(async move {
            info!("Peer dialed us.");
            match start_sync(tx, doc_changed_ping_tx, stream).await {
                Ok(()) => {
                    info!("Peer disconnected.");
                }
                Err(e) => {
                    error!("listen_tcp Error: {:#?}", e);
                }
            }
        });
    }
}

async fn dial_tcp(
    tx: DocMessageSender,
    doc_changed_ping_tx: DocChangedSender,
    addr: String,
) -> Result<()> {
    let stream = TcpStream::connect(addr).await?;

    start_sync(tx, doc_changed_ping_tx, stream).await?;

    Ok(())
}

async fn start_sync(
    tx: DocMessageSender,
    doc_changed_ping_tx: DocChangedSender,
    stream: TcpStream,
) -> Result<()> {
    let mut peer_state = SyncState::new();

    let (syncer_message_tx, mut syncer_message_rx) = mpsc::channel(16);
    let (tcp_read, mut tcp_write) = tokio::io::split(stream);

    // TCP reader.
    let receiver = SyncReceiver::new(tcp_read, syncer_message_tx.clone());
    let shutdown_token = CancellationToken::new();
    let shutdown_token_clone = shutdown_token.clone();
    tokio::spawn(async move {
        sync_receive(receiver).await;
        shutdown_token_clone.cancel()
    });

    // Generate sync message when doc changes.
    let syncer_message_tx_clone = syncer_message_tx.clone();
    let shutdown_token_clone = shutdown_token.clone();
    tokio::spawn(async move {
        let mut doc_changed_ping_rx = doc_changed_ping_tx.subscribe();
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

    loop {
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                debug!("Shutting down main start_sync loop");
                break;
            }
            syncer_message_maybe = syncer_message_rx.recv() => match syncer_message_maybe {
                None => {
                    panic!("Channel towards sync task has been closed");
                }
                Some(message) => match message {
                    SyncerMessage::ReceiveSyncMessage { message } => {
                        let (reponse_tx, response_rx) = oneshot::channel();
                        let message = Message::decode(&message)?;
                        tx.send(DocMessage::ReceiveSyncMessage {
                            message,
                            state: peer_state,
                            response_tx: reponse_tx,
                        })
                        .await?;
                        peer_state = response_rx.await?;
                    }
                    SyncerMessage::GenerateSyncMessage {} => {
                        let (reponse_tx, response_rx) = oneshot::channel();
                        tx.send(DocMessage::GenerateSyncMessage {
                            state: peer_state,
                            response_tx: reponse_tx,
                        })
                        .await?;
                        let (ps, message) = response_rx.await?;
                        peer_state = ps;
                        if let Some(message) = message {
                            let message = message.encode();
                            let message_len = message.len() as i32;
                            tcp_write
                                .write_all(&message_len.to_be_bytes())
                                .await
                                .expect("GenerateSyncMessage: write message len failed");
                            tcp_write
                                .write_all(&message)
                                .await
                                .expect("GenerateSyncMessage: write message failed");
                        }
                    }
                },
            }
        }
    }
    Ok(())
}

async fn listen_socket(
    tx: DocMessageSender,
    mut editor_message_rx: EditorMessageReceiver,
    socket_path: &Path,
    file_path: PathBuf,
) -> Result<()> {
    if Path::new(&socket_path).exists() {
        fs::remove_file(socket_path)?;
    }
    let listener = UnixListener::bind(socket_path)?;
    info!("Listening on UNIX socket: {}", socket_path.display());

    let file_path_clone = file_path.clone();

    loop {
        // TODO: Accept multiple connections.
        // TODO: How do we know/what do we do when a client disconnects?
        match listener.accept().await {
            Ok((stream, _addr)) => {
                info!("Client connection established.");

                let (mut stream_read, mut stream_write) = tokio::io::split(stream);

                // We're parsing a line from the reader (if we have one)
                // which means we got a Delta from the ethersync client.
                //
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let buf_reader = BufReader::new(&mut stream_read);
                    let mut lines = buf_reader.lines();

                    loop {
                        match lines.next_line().await {
                            Ok(Some(line)) => {
                                debug!("Got a line from the client: {:#?}", line);
                                let jsonrpc = EditorProtocolMessage::from_jsonrpc(&line)
                                    .expect("Failed to parse JSON-RPC message");
                                tx_clone
                                    .send(jsonrpc.into())
                                    .await
                                    .expect("Failed to send message to document");
                            }
                            Ok(None) => {
                                break;
                            }
                            Err(e) => {
                                error!("Error reading line: {:#?}", e);
                            }
                        }
                    }
                });

                // We're sending an editor message to the client.
                loop {
                    match editor_message_rx.recv().await {
                        Some(rev_delta) => {
                            debug!("Received editor message to send to it.");
                            let message = EditorProtocolMessage::Edit {
                                uri: format!("file://{}", file_path_clone.display()),
                                delta: rev_delta,
                            };
                            let payload = message
                                .to_jsonrpc()
                                .expect("Failed to serialize JSON-RPC message");
                            debug!("Sending message to editor: {:#?}", payload);
                            stream_write
                                .write_all(format!("{payload}\n").as_bytes())
                                .await
                                .expect("Failed to write to TCP stream");
                        }
                        None => {
                            panic!("TODO: why?");
                        }
                    }
                }
            }
            Err(e) => {
                error!("listen_socket Error: {:#?}", e);
            }
        }
    }
}

struct SyncReceiver {
    sender: SyncerMessageSender,
    reader: ReadHalf<TcpStream>,
}

impl SyncReceiver {
    fn new(reader: ReadHalf<TcpStream>, sender: SyncerMessageSender) -> Self {
        Self { sender, reader }
    }

    async fn forward_sync_message(&self, message: Vec<u8>) {
        self.sender
            .send(SyncerMessage::ReceiveSyncMessage { message })
            .await
            .expect("Channel for sending Sync Task has been closed");
    }

    async fn read_message(&mut self) -> Result<Vec<u8>> {
        let mut message_len_buf = [0; 4];
        self.reader.read_exact(&mut message_len_buf).await?;
        let message_len = i32::from_be_bytes(message_len_buf);
        let mut message_buf = vec![0; message_len as usize];
        self.reader.read_exact(&mut message_buf).await?;
        Ok(message_buf)
    }
}

async fn sync_receive(mut sync_receiver: SyncReceiver) {
    while let Ok(message) = sync_receiver.read_message().await {
        sync_receiver.forward_sync_message(message).await;
    }
    info!("Sync Receive loop stopped (peer disconnected)");
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
