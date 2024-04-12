#![allow(dead_code)]
use crate::ot::OTServer;
use crate::types::{
    EditorTextDelta, EditorTextOp, Range, RevisionedEditorTextDelta, RevisionedTextDelta,
    TextDelta, TextOp,
};
use anyhow::Result;
use automerge::{
    patches::TextRepresentation,
    sync::{Message, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, Patch, PatchLog, ReadDoc,
};
use rand::{distributions::Alphanumeric, Rng};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf},
    net::UnixListener,
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, oneshot},
    time::{sleep, Duration},
};
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

// These messages are sent to tasks that own peer sync states.
enum SyncerMessage {
    ReceiveSyncMessage { message: Vec<u8> },
    GenerateSyncMessage,
}

type DocMessageSender = mpsc::Sender<DocMessage>;
type DocChangedSender = broadcast::Sender<()>;
type EditorMessageSender = broadcast::Sender<RevisionedTextDelta>;
type SyncerMessageSender = mpsc::Sender<SyncerMessage>;

/// Encapsulates the Automerge AutoCommit and provides a generic interface,
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
        for op in &delta.0 {
            let (position, length) = op.range.as_relative();
            self.doc
                .splice_text(text_obj.clone(), position, length as isize, &op.replacement)
                .expect("Failed to splice Automerge text object");
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
            .splice_text(text_obj, 0, 0, &text)
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
    fn handle_message(&mut self, message: DocMessage) {
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
                self.apply_delta_to_doc(&delta.clone().into());
                self.process_crdt_delta_in_ot(delta);
            }
            DocMessage::Delta(delta) => {
                let editor_delta: EditorTextDelta = delta.clone().into();
                self.apply_delta_to_doc(&editor_delta);
                self.process_crdt_delta_in_ot(delta);
            }
            DocMessage::RevDelta(rev_delta) => {
                let (editor_delta_for_crdt, rev_deltas_for_editor) =
                    self.apply_delta_to_ot(rev_delta);

                self.apply_delta_to_doc(&editor_delta_for_crdt);
                self.send_deltas_to_editor(rev_deltas_for_editor);
            }
            DocMessage::ReceiveSyncMessage {
                message,
                state: mut peer_state,
                response_tx,
            } => {
                if let Some(patches) = self.apply_sync_message_to_doc(message, &mut peer_state) {
                    debug!(?patches);
                    for patch in patches {
                        match patch.action.try_into() {
                            Ok(delta) => {
                                self.process_crdt_delta_in_ot(delta);
                            }
                            Err(e) => {
                                warn!("Failed to convert patch to delta: {:#?}", e);
                            }
                        }
                    }
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
        let result;
        if self.ot_server.is_some() {
            let patches = self
                .crdt_doc
                .receive_sync_message_log_patches(message, peer_state);
            result = Some(patches)
        } else {
            self.crdt_doc.receive_sync_message(message, peer_state);
            result = None
        }
        let _ = self.doc_changed_ping_tx.send(());
        result
    }

    fn send_deltas_to_editor(&self, rev_deltas: Vec<RevisionedTextDelta>) {
        for rev_delta in rev_deltas {
            self.socket_message_tx
                .send(rev_delta)
                .expect("Failed to send message to socket channel.");
        }
    }

    fn apply_delta_to_ot(
        &mut self,
        rev_delta: RevisionedEditorTextDelta,
    ) -> (EditorTextDelta, Vec<RevisionedTextDelta>) {
        let ot_server = self
            .ot_server
            .as_mut()
            .expect("No editor connected, where does this delta come from?");

        let (delta_for_crdt, rev_deltas_for_editor) =
            ot_server.apply_editor_operation(rev_delta.into());

        (delta_for_crdt.into(), rev_deltas_for_editor)
    }

    fn random_delta(&self) -> TextDelta {
        let text = self
            .current_content()
            .expect("Should have initialized text before performing random edit");
        let random_string: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(1)
            .map(char::from)
            .collect();
        let text_length = text.chars().count();
        let random_position = rand::thread_rng().gen_range(0..=text_length);

        let mut delta = TextDelta::default();
        delta.retain(random_position);
        delta.insert(&random_string);
        delta
    }

    fn process_crdt_delta_in_ot(&mut self, delta: TextDelta) {
        if let Some(ot_server) = &mut self.ot_server {
            let rev_text_delta_for_editor = ot_server.apply_crdt_change(delta);
            self.socket_message_tx
                .send(rev_text_delta_for_editor)
                .expect("Failed to send message to socket channel.");
        }
    }

    fn write_current_content_to_file(&mut self) {
        let content = self.current_content();
        if let Ok(text) = content {
            debug!(current_text = text);
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
            self.handle_message(message);
            self.write_current_content_to_file();
        }
        debug!("Channel towards document task has been closed");
    }
}

pub struct Daemon {
    doc_message_tx: DocMessageSender,
}

impl Daemon {
    // Launch the daemon. Optionally, connect to given peer.
    pub fn new(peer: Option<String>, socket_path: &Path, file_path: &Path) -> Self {
        // The document task will receive messages on this channel.
        // The TCP and socket connections will send messages to it when they receive something.
        let (doc_message_tx, doc_message_rx) = mpsc::channel(1);

        // The document task will send a ping on this channel whenever it changes.
        // The sync tasks will subscribe to it, and react to it by syncing with the peers.
        let (doc_changed_ping_tx, _doc_changed_ping_rx) = broadcast::channel::<()>(16);

        // The document task will send messages intended for the socket connection on this channel.
        let (socket_message_tx, _socket_message_rx) = broadcast::channel::<RevisionedTextDelta>(16);

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
            tokio::spawn(async {
                listen_tcp(doc_message_tx_clone, doc_changed_ping_tx_clone)
                    .await
                    .expect("Failed to listen on TCP port");
            });
        }

        let socket_message_tx_clone = socket_message_tx.clone();
        let socket_path_clone = socket_path.to_path_buf();
        tokio::spawn(async move {
            listen_socket(
                doc_message_tx_clone_2,
                socket_message_tx_clone,
                &socket_path_clone,
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

    #[allow(dead_code)]
    #[must_use]
    pub fn tcp_address(&self) -> String {
        // TODO: Get the actual address.
        "0.0.0.0:4242".to_string()
    }
}

async fn listen_tcp(tx: DocMessageSender, doc_changed_ping_tx: DocChangedSender) -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:4242").await?;
    info!("Listening on TCP port: {}", listener.local_addr()?);

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
                    debug!("Sync OK?!");
                }
                Err(e) => {
                    error!("Error: {:#?}", e);
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
    tokio::spawn(sync_receive(receiver));

    // Generate sync message when doc changes.
    let syncer_message_tx_clone = syncer_message_tx.clone();
    tokio::spawn(async move {
        let mut doc_changed_ping_rx = doc_changed_ping_tx.subscribe();
        loop {
            syncer_message_tx_clone
                .send(SyncerMessage::GenerateSyncMessage {})
                .await
                .expect("Failed to send GenerateSyncMessage to document task");
            doc_changed_ping_rx
                .recv()
                .await
                .expect("Doc changed channel has been closed.");
        }
    });

    loop {
        match syncer_message_rx.recv().await {
            None => {
                panic!("Channel towards sync task has been closed.");
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
                        tcp_write.write_all(&message_len.to_be_bytes()).await?;
                        tcp_write.write_all(&message).await?;
                    }
                }
            },
        }
    }
}

async fn listen_socket(
    tx: DocMessageSender,
    editor_message_tx: EditorMessageSender,
    socket_path: &Path,
) -> Result<()> {
    if Path::new(&socket_path).exists() {
        fs::remove_file(socket_path)?;
    }
    let listener = UnixListener::bind(socket_path)?;
    info!("Listening on UNIX socket: {}", socket_path.display());

    loop {
        // TODO: Accept multiple connections.
        match listener.accept().await {
            Ok((stream, _addr)) => {
                info!("Client connection established.");

                let mut editor_message_rx = editor_message_tx.subscribe();

                let (mut tcp_read, mut tcp_write) = tokio::io::split(stream);
                let buf_reader = BufReader::new(&mut tcp_read);
                //for line in buf_reader.lines() {
                let mut lines = buf_reader.lines();

                loop {
                    // either we're parsing a line from the reader (if we have one)
                    // which means we got a Delta from the ethersync client
                    //
                    // or we're sending an editor message to the client
                    tokio::select! {
                        line_maybe = lines.next_line() => {
                            match line_maybe {
                                Ok(Some(line)) => {
                                    match jsonrpc_to_docmessage(&line) {
                                        Ok(message) => {
                                            tx.send(message).await?;
                                        }
                                        Err(e) => {
                                            error!("Failed to parse message from editor: {:#?}", e);
                                        }
                                    }
                                }
                                Ok(None) => {
                                    break;
                                }
                                Err(e) => {
                                    error!("Error reading line: {:#?}", e);
                                }
                            }
                        }
                        Ok(rev_delta) = editor_message_rx.recv() => {
                            debug!("Received editor message.");

                            let mut json_params = vec![];
                            for op in rev_delta.delta {
                                match op {
                                    TextOp::Retain(n) => {
                                        json_params.push(json!(n));
                                    }
                                    TextOp::Insert(s) => {
                                        json_params.push(json!(s));
                                    }
                                    TextOp::Delete(n) => {
                                        json_params.push(json!(-(n as i64)));
                                    }
                                }
                            }
                            let payload = json!({
                                "method": "operation",
                                "params": [rev_delta.revision, json_params]
                            });
                            tcp_write.write_all(format!("{payload}\n").as_bytes()).await?;
                        }
                    }
                }
                info!("Client connection closed.");
            }
            Err(e) => {
                error!("Error: {:#?}", e);
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
    warn!("Sync Receive loop stopped");
}

fn jsonrpc_to_docmessage(s: &str) -> Result<DocMessage> {
    let json = serde_json::from_str(s)?;
    match json {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(method)) = map.get("method") {
                match method.as_str() {
                    "open" => Ok(DocMessage::Open),
                    "close" => Ok(DocMessage::Close),
                    "insert" => {
                        if let Some(serde_json::Value::Array(array)) = map.get("params") {
                            if let Some(serde_json::Value::Number(revision)) = array.get(1) {
                                if let Some(serde_json::Value::Number(position)) = array.get(2) {
                                    if let Some(serde_json::Value::String(text)) = array.get(3) {
                                        let revision =
                                            revision.as_u64().expect("Failed to parse revision");
                                        let position =
                                            position.as_u64().expect("Failed to parse position");
                                        let text = text.as_str().to_string();
                                        let op = EditorTextOp {
                                            range: Range {
                                                anchor: position as usize,
                                                head: position as usize,
                                            },
                                            replacement: text,
                                        };
                                        let delta = EditorTextDelta(vec![op]);
                                        Ok(DocMessage::RevDelta(RevisionedEditorTextDelta {
                                            revision: revision as usize,
                                            delta,
                                        }))
                                    } else {
                                        Err(anyhow::anyhow!(
                                            "Could not find text param in position #3"
                                        ))
                                    }
                                } else {
                                    Err(anyhow::anyhow!(
                                        "Could not find position param in position #2"
                                    ))
                                }
                            } else {
                                Err(anyhow::anyhow!(
                                    "Could not find revision param in position #1"
                                ))
                            }
                        } else {
                            Err(anyhow::anyhow!("Could not find params for insert method"))
                        }
                    }
                    "delete" => {
                        if let Some(serde_json::Value::Array(array)) = map.get("params") {
                            if let Some(serde_json::Value::Number(revision)) = array.get(1) {
                                if let Some(serde_json::Value::Number(position)) = array.get(2) {
                                    if let Some(serde_json::Value::Number(length)) = array.get(3) {
                                        let revision =
                                            revision.as_u64().expect("Failed to parse revision");
                                        let position =
                                            position.as_u64().expect("Failed to parse position");

                                        let length =
                                            length.as_u64().expect("Failed to parse length");

                                        let op = EditorTextOp {
                                            range: Range {
                                                anchor: position as usize,
                                                head: position as usize + length as usize,
                                            },
                                            replacement: String::new(),
                                        };
                                        let delta = EditorTextDelta(vec![op]);
                                        Ok(DocMessage::RevDelta(RevisionedEditorTextDelta {
                                            revision: revision as usize,
                                            delta,
                                        }))
                                    } else {
                                        Err(anyhow::anyhow!(
                                            "Could not find length param in position #3"
                                        ))
                                    }
                                } else {
                                    Err(anyhow::anyhow!(
                                        "Could not find position param in position #2"
                                    ))
                                }
                            } else {
                                Err(anyhow::anyhow!("Could not find params for delete method"))
                            }
                        } else {
                            Err(anyhow::anyhow!(
                                "Could not find revision param in position #1"
                            ))
                        }
                    }
                    _ => Err(anyhow::anyhow!("Unknown JSON method: {}", method)),
                }
            } else {
                Err(anyhow::anyhow!("Could not find method in JSON message"))
            }
        }
        _ => Err(anyhow::anyhow!(
            "JSON message is not an object: {:#?}",
            json
        )),
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

        #[test]
        fn can_apply_delta_basic_insertion() {
            let mut document = Document::default();
            let text = String::new();
            document.initialize_text(&text);

            let delta = insert(0, "foobar");

            document.apply_delta_to_doc(&delta.into());
            assert_eq!(document.current_content().unwrap(), "foobar");
        }

        #[test]
        fn can_apply_delta_basic_deletion() {
            let mut document = Document::default();
            let text = "foobar".to_string();
            document.initialize_text(&text);

            let delta = delete(3, 3);

            document.apply_delta_to_doc(&delta.into());
            assert_eq!(document.current_content().unwrap(), "foo");
        }

        #[test]
        fn can_apply_delta_multiple_ops() {
            let mut document = Document::default();
            let text = "To be or not to be, that is the question".to_string();
            document.initialize_text(&text);

            let mut delta = insert(3, "m");
            delta.delete(1); // "b"
            delta.retain(5); // "e or "
            delta.delete(4); // "not "
            delta.retain(3); // "to "
            delta.delete(2); // "be"
            delta.insert("you");

            document.apply_delta_to_doc(&delta.into());
            assert_eq!(
                document.current_content().unwrap(),
                "To me or to you, that is the question"
            );
        }
    }

    #[test]
    fn json_to_docmessage() {
        let json = serde_json::json!({
            "method": "insert",
            "params": ["", 0, 1, "a"]
        });

        let message = jsonrpc_to_docmessage(&json.to_string()).unwrap();
        if let DocMessage::RevDelta(delta) = message {
            assert_eq!(
                delta,
                RevisionedEditorTextDelta {
                    revision: 0,
                    delta: EditorTextDelta(vec![EditorTextOp {
                        range: Range { anchor: 1, head: 1 },
                        replacement: "a".to_string(),
                    }])
                }
            );
        } else {
            panic!("Expected DocMessage::Delta, got something else.");
        }

        let json = serde_json::json!({
            "method": "delete",
            "params": ["", 2, 1, 3]
        });

        let message = jsonrpc_to_docmessage(&json.to_string()).unwrap();
        if let DocMessage::RevDelta(delta) = message {
            assert_eq!(
                delta,
                RevisionedEditorTextDelta {
                    revision: 2,
                    delta: EditorTextDelta(vec![EditorTextOp {
                        range: Range { anchor: 1, head: 4 },
                        replacement: "".to_string(),
                    }])
                }
            );
        } else {
            panic!("Expected DocMessage::Delta, got something else.");
        }
    }
}
