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
    AutoCommit, ObjType, PatchLog, ReadDoc,
};
use rand::{distributions::Alphanumeric, Rng};
use serde_json::json;
use std::fs;
use std::path::Path;
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

pub struct Daemon {
    doc_message_tx: DocMessageSender,
    doc_message_rx: mpsc::Receiver<DocMessage>,
}

impl Daemon {
    pub fn new() -> Self {
        // The document task will receive messages on this channel.
        // The TCP and socket connections will send messages to it when they receive something.
        let (doc_message_tx, doc_message_rx) = mpsc::channel(1);

        Self {
            doc_message_tx,
            doc_message_rx,
        }
    }

    #[allow(dead_code)]
    pub async fn message(&self, message: DocMessage) {
        self.doc_message_tx
            .send(message)
            .await
            .expect("Failed to send message to document task");
    }

    #[allow(dead_code)]
    pub fn tcp_address(&self) -> String {
        // TODO: Get the actual address.
        "0.0.0.0:4242".to_string()
    }

    // Launch the daemon. Optionally, connect to given peer.
    pub async fn launch(&mut self, peer: Option<String>, socket_path: &Path, file_path: &Path) {
        let mut doc = AutoCommit::new();
        let mut editor_is_connected = false;
        let mut ot_server: OTServer = Default::default();

        // The document task will send a ping on this channel whenever it changes.
        // The sync tasks will subscribe to it, and react to it by syncing with the peers.
        let (doc_changed_tx, _doc_changed_rx) = broadcast::channel::<()>(16);

        // The document task will send messages intended for the socket connection on this channel.
        let (socket_message_tx, _socket_message_rx) = broadcast::channel::<RevisionedTextDelta>(16);

        // If we are the host, read file content.
        if peer.is_none() {
            // Create the file if it doesn't exist.
            if !file_path.exists() {
                std::fs::write(file_path, "").expect("Could not create file");
            }

            if let Ok(text) = std::fs::read_to_string(file_path) {
                let text_obj = doc
                    .put_object(automerge::ROOT, "text", ObjType::Text)
                    .expect("Failed to initialize text object in Automerge document");
                doc.splice_text(text_obj, 0, 0, &text)
                    .expect("Failed to splice text into Automerge text object");
            } else {
                // TODO: Look at *why* we couldn't read the file.
                panic!("Could not read file {}", file_path.display());
            }
        }

        // Make edits to the document occasionally.
        // To activate, build with --features simulate_edits_on_crdt
        if cfg!(feature = "simulate_edits_on_crdt") {
            let tx = self.doc_message_tx.clone();
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
        let doc_message_tx_clone = self.doc_message_tx.clone();
        let doc_changed_tx_clone = doc_changed_tx.clone();
        let doc_message_tx_clone_2 = doc_message_tx_clone.clone();

        if let Some(peer) = peer {
            tokio::spawn(async {
                dial_tcp(doc_message_tx_clone, doc_changed_tx_clone, peer)
                    .await
                    .expect("Failed to dial peer");
            });
        } else {
            tokio::spawn(async {
                listen_tcp(doc_message_tx_clone, doc_changed_tx_clone)
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

        loop {
            let message = self
                .doc_message_rx
                .recv()
                .await
                .expect("Channel towards document task has been closed");
            match message {
                DocMessage::Open => {
                    editor_is_connected = true;
                    ot_server =
                        OTServer::new(current_content(&doc).expect(
                            "Should have initialized text before initializing the document",
                        ));
                }
                DocMessage::Close => {
                    editor_is_connected = false;
                }
                DocMessage::RandomEdit => {
                    let text = current_content(&doc)
                        .expect("Should have initialized text before performing random edit");
                    let random_string: String = rand::thread_rng()
                        .sample_iter(&Alphanumeric)
                        .take(1)
                        .map(char::from)
                        .collect();
                    let text_length = text.chars().count();
                    let random_position = rand::thread_rng().gen_range(0..(text_length + 1));

                    if editor_is_connected {
                        let mut delta = TextDelta::default();
                        delta.retain(random_position);
                        delta.insert(&random_string);

                        let rev_delta = ot_server.apply_crdt_change(delta);
                        socket_message_tx
                            .send(rev_delta)
                            .expect("Failed to send message to socket channel.");
                    }

                    let text_obj = doc
                        .get(automerge::ROOT, "text")
                        .expect("Failed to get text object from Automerge document");
                    if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
                        doc.insert(text_obj, random_position, random_string)
                            .expect("Failed to insert into Automerge text object");
                    }

                    let _ = doc_changed_tx.send(());
                }
                DocMessage::Delta(delta) => {
                    let editor_delta: EditorTextDelta = delta.into();
                    apply_delta(&mut doc, &editor_delta);
                }
                DocMessage::RevDelta(rev_delta) => {
                    if !editor_is_connected {
                        panic!("No editor connected, where does this delta come from?");
                    }

                    let (delta_for_crdt, rev_deltas_for_editor) =
                        ot_server.apply_editor_operation(rev_delta.into());

                    let editor_delta_for_crdt: EditorTextDelta = delta_for_crdt.into();

                    apply_delta(&mut doc, &editor_delta_for_crdt);

                    for rev_delta in rev_deltas_for_editor {
                        socket_message_tx
                            .send(rev_delta)
                            .expect("Failed to send message to socket channel.");
                    }

                    let _ = doc_changed_tx.send(());
                }
                DocMessage::ReceiveSyncMessage {
                    message,
                    state: mut peer_state,
                    response_tx,
                } => {
                    let mut patch_log = PatchLog::active(TextRepresentation::String);
                    doc.sync()
                        .receive_sync_message_log_patches(&mut peer_state, message, &mut patch_log)
                        .expect("Failed to apply sync message to Automerge document");
                    if editor_is_connected {
                        let patches = doc.make_patches(&mut patch_log);
                        debug!(?patches);
                        for patch in patches {
                            match patch.action.try_into() {
                                Ok(delta) => {
                                    let rev_delta = ot_server.apply_crdt_change(delta);
                                    socket_message_tx
                                        .send(rev_delta)
                                        .expect("Failed to send message to socket channel.");
                                }
                                Err(e) => {
                                    warn!("Failed to convert patch to delta: {:#?}", e);
                                }
                            }
                        }
                    }
                    let _ = doc_changed_tx.send(());
                    response_tx
                        .send(peer_state)
                        .expect("Failed to send peer state in response to ReceiveSyncMessage");
                }
                DocMessage::GenerateSyncMessage {
                    state: mut peer_state,
                    response_tx,
                } => {
                    let message = doc.sync().generate_sync_message(&mut peer_state);
                    response_tx.send((peer_state, message)).expect(
                        "Failed to send peer state and sync message in response to GenerateSyncMessage",
                    );
                }
            }

            let content = current_content(&doc);
            if let Ok(text) = content {
                debug!(current_text = text);
                if editor_is_connected {
                    debug!(current_ot_doc = ot_server.apply_to_initial_content());
                } else {
                    std::fs::write(&file_path, &text).expect("Could not write to file");
                }
            }
        }
    }
}

async fn listen_tcp(tx: DocMessageSender, doc_changed_tx: DocChangedSender) -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:4242").await?;
    info!("Listening on TCP port: {}", listener.local_addr()?);

    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            error!("Error accepting connection.");
            continue;
        };

        let tx = tx.clone();
        let doc_changed_tx = doc_changed_tx.clone();
        tokio::spawn(async move {
            info!("Peer dialed us.");
            match start_sync(tx, doc_changed_tx, stream).await {
                Ok(_) => {
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
    doc_changed_tx: DocChangedSender,
    addr: String,
) -> Result<()> {
    let stream = TcpStream::connect(addr).await?;

    start_sync(tx, doc_changed_tx, stream).await?;

    Ok(())
}

async fn start_sync(
    tx: DocMessageSender,
    doc_changed_tx: DocChangedSender,
    stream: TcpStream,
) -> Result<()> {
    let mut peer_state = SyncState::new();

    let (reader_message_tx, mut reader_message_rx) = mpsc::channel(1);
    let (tcp_read, mut tcp_write) = tokio::io::split(stream);

    // TCP reader.
    let message_tx_clone = reader_message_tx.clone();
    tokio::spawn(async move {
        match sync_receive(tcp_read, message_tx_clone).await {
            Ok(_) => {
                debug!("Sync receive OK.");
            }
            Err(e) => {
                error!("Error sync_receive: {:#?}", e);
            }
        }
    });

    // Generate sync message when doc changes.
    let reader_message_tx_clone = reader_message_tx.clone();
    tokio::spawn(async move {
        let mut doc_changed_rx = doc_changed_tx.subscribe();
        loop {
            reader_message_tx_clone
                .send(SyncerMessage::GenerateSyncMessage {})
                .await
                .expect("Failed to send GenerateSyncMessage to document task");
            doc_changed_rx
                .recv()
                .await
                .expect("Doc changed channel has been closed.");
        }
    });

    loop {
        match reader_message_rx.recv().await {
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
        fs::remove_file(&socket_path)?;
    }
    let listener = UnixListener::bind(&socket_path)?;
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
                            tcp_write.write_all(format!("{}\n", payload).as_bytes()).await?;
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

async fn sync_receive(mut reader: ReadHalf<TcpStream>, tx: SyncerMessageSender) -> Result<()> {
    loop {
        let mut message_len_buf = [0; 4];
        reader.read_exact(&mut message_len_buf).await?;
        let message_len = i32::from_be_bytes(message_len_buf);
        let mut message_buf = vec![0; message_len as usize];
        reader.read_exact(&mut message_buf).await?;

        tx.send(SyncerMessage::ReceiveSyncMessage {
            message: message_buf,
        })
        .await?;
    }
}

fn current_content(doc: &AutoCommit) -> Result<String> {
    let text_obj = doc
        .get(automerge::ROOT, "text")
        .expect("Failed to get text key from Automerge document");
    if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
        Ok(doc
            .text(&text_obj)
            .expect("Failed to get string from Automerge text object"))
    } else {
        Err(anyhow::anyhow!(
            "Could not get text object from Automerge object. Is it a text value?"
        ))
    }
}

fn apply_delta(doc: &mut AutoCommit, delta: &EditorTextDelta) {
    let text_obj = doc
        .get(automerge::ROOT, "text")
        .expect("Failed to get text key from Automerge document");
    if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
        for op in &delta.0 {
            let (position, length) = op.range.as_relative();
            doc.splice_text(text_obj.clone(), position, length as isize, &op.replacement)
                .expect("Failed to splice Automerge text object");
        }
    } else {
        panic!("Automerge document doesn't have a text object, so I can't modify it");
    }
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
                                            replacement: "".to_string(),
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
