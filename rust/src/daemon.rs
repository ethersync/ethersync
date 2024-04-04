#![allow(dead_code, unused_imports)]
use crate::ot::{OTServer, RevisionedTextDelta};
use crate::types::RevisionedEditorTextDelta;
use anyhow::Result;
use automerge::{
    patches::TextRepresentation,
    sync::{Message, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, Patch, PatchAction, PatchLog, ReadDoc,
};
use rand::{distributions::Alphanumeric, Rng};
use std::fs;
use std::path::Path;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf},
    net::UnixListener,
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, oneshot},
    time::{sleep, Duration},
};
use tracing::{debug, error, info, trace, warn};

const SOCKET_PATH: &str = "/tmp/ethersync";

// These messages are sent to the task that owns the document.
enum DocMessage {
    Init,
    RandomEdit,
    Delta(RevisionedEditorTextDelta),
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

#[derive(Clone, Debug)]
enum EditorMessage {
    Insert {
        editor_revision: usize,
        position: usize,
        text: String,
    },
    Delete {
        editor_revision: usize,
        position: usize,
        length: usize,
    },
}

type DocMessageSender = mpsc::Sender<DocMessage>;
type DocChangedSender = broadcast::Sender<()>;
type EditorMessageSender = broadcast::Sender<EditorMessage>;
type SyncerMessageSender = mpsc::Sender<SyncerMessage>;

// Launch the daemon. Optionally, connect to given peer.
pub async fn launch(peer: Option<String>) {
    let mut doc = AutoCommit::new();
    let mut ot_server: OTServer = Default::default();

    // The document task will send a ping on this channel whenever it changes.
    // The sync tasks will subscribe to it, and react to it by syncing with the peers.
    let (doc_changed_tx, _doc_changed_rx) = broadcast::channel::<()>(16);

    // The document task will receive messages on this channel.
    // The TCP and socket connections will send messages to it when they receive something.
    let (doc_message_tx, mut doc_message_rx) = mpsc::channel(1);

    // The document task will send messages intended for the socket connection on this channel.
    let (socket_message_tx, _socket_message_rx) = broadcast::channel::<EditorMessage>(16);

    // Make edits to the document occasionally.
    if true {
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
    if true {
        let tx = socket_message_tx.clone();
        tokio::spawn(async move {
            let mut editor_revision = 0;
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
                //editor_revision += 1;
            }
        });
    }

    // Dial peer, or listen for incoming connections.
    let doc_message_tx_clone = doc_message_tx.clone();
    let doc_changed_tx_clone = doc_changed_tx.clone();
    if let Some(peer) = peer {
        tokio::spawn(async {
            dial_tcp(doc_message_tx_clone, doc_changed_tx_clone, peer)
                .await
                .expect("Failed to dial peer");
        });
    } else {
        let doc_message_tx_clone_2 = doc_message_tx_clone.clone();
        let socket_message_tx_clone = socket_message_tx.clone();
        tokio::spawn(async {
            listen_socket(doc_message_tx_clone_2, socket_message_tx_clone)
                .await
                .expect("Failed to listen on UNIX socket");
        });
        tokio::spawn(async {
            listen_tcp(doc_message_tx_clone, doc_changed_tx_clone)
                .await
                .expect("Failed to listen on TCP port");
        });
    }

    loop {
        let message = doc_message_rx
            .recv()
            .await
            .expect("Channel towards document task has been closed");
        match message {
            DocMessage::Init => {
                let _text = doc
                    .put_object(automerge::ROOT, "text", ObjType::Text)
                    .expect("Failed to initialize text object in Automerge document");
                // In the beginning, no-one might be interested in these messages, so the
                // send might fail, I think?
                let _ = doc_changed_tx.send(());
            }
            DocMessage::RandomEdit => {
                let text_obj = doc
                    .get(automerge::ROOT, "text")
                    .expect("Failed to get text object from Automerge document");
                if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
                    let text_length = doc
                        .text(&text_obj)
                        .expect("Failed to get string from Automerge text object")
                        .len();
                    let random_string: String = rand::thread_rng()
                        .sample_iter(&Alphanumeric)
                        .take(1)
                        .map(char::from)
                        .collect();
                    let random_position = rand::thread_rng().gen_range(0..(text_length + 1));
                    doc.insert(text_obj, random_position, random_string)
                        .expect("Failed to insert into Automerge text object");
                    let _ = doc_changed_tx.send(());
                } else {
                    panic!(
                        "Automerge document doesn't have a text object, so I can't edit randomly"
                    );
                }
            }
            DocMessage::Delta(rev_delta) => {
                let text_obj = doc
                    .get(automerge::ROOT, "text")
                    .expect("Failed to get text object from Automerge document");
                if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
                    for op in rev_delta.delta {
                        let (position, length) = op.range.as_relative();
                        doc.splice_text(text_obj, position, length as isize, op.replacement)
                            .expect("Failed to splice Automerge text object");
                    }
                    // TODO: fill with meaningful values
                    ot_server.apply_editor_operation(rev_delta);
                    let _ = doc_changed_tx.send(());
                } else {
                    panic!("Automerge document doesn't have a text object, so I can't delete");
                }
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
                let patches = doc.make_patches(&mut patch_log);
                debug!(?patches);
                // TODO: Call apply_crdt_change in OT.
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

        let text = doc
            .get(automerge::ROOT, "text")
            .expect("Failed to get text object from the Automerge document");

        if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text {
            let text = doc
                .text(&text_obj)
                .expect("Failed to get string from Automerge text object");
            debug!(current_text = text);
        }
    }
}

async fn listen_tcp(tx: DocMessageSender, doc_changed_tx: DocChangedSender) -> Result<()> {
    tx.send(DocMessage::Init).await?;

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
    let (read, mut write) = tokio::io::split(stream);

    // TCP reader.
    let message_tx_clone = reader_message_tx.clone();
    tokio::spawn(async move {
        match sync_receive(read, message_tx_clone).await {
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
                        write.write_all(&message_len.to_be_bytes()).await?;
                        write.write_all(&message).await?;
                    }
                }
            },
        }
    }
}

// TODO: Write this in a nicer way...
fn parse_message_from_editor(s: &str) -> Result<DocMessage> {
    let json: serde_json::Value = serde_json::from_str(s)?;
    match json {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(method)) = map.get("method") {
                match method.as_str() {
                    "insert" => {
                        if let Some(serde_json::Value::Array(array)) = map.get("params") {
                            if let Some(serde_json::Value::Number(position)) = array.get(2) {
                                if let Some(serde_json::Value::String(text)) = array.get(3) {
                                    let position =
                                        position.as_u64().expect("Failed to parse position");
                                    let text = text.as_str().to_string();
                                    Ok(DocMessage::Insert {
                                        position: position as usize,
                                        text,
                                    })
                                } else {
                                    Err(anyhow::anyhow!("Could not find text param in position #3"))
                                }
                            } else {
                                Err(anyhow::anyhow!(
                                    "Could not find position param in position #2"
                                ))
                            }
                        } else {
                            Err(anyhow::anyhow!("Could not find params for insert method"))
                        }
                    }
                    "delete" => {
                        if let Some(serde_json::Value::Array(array)) = map.get("params") {
                            if let Some(serde_json::Value::Number(position)) = array.get(2) {
                                if let Some(serde_json::Value::Number(length)) = array.get(3) {
                                    let position =
                                        position.as_u64().expect("Failed to parse position");
                                    let length = length.as_u64().expect("Failed to parse length");
                                    Ok(DocMessage::Delete {
                                        position: position as usize,
                                        length: length as usize,
                                    })
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

async fn listen_socket(tx: DocMessageSender, editor_message_tx: EditorMessageSender) -> Result<()> {
    if Path::new(SOCKET_PATH).exists() {
        fs::remove_file(SOCKET_PATH)?;
    }
    let listener = UnixListener::bind(SOCKET_PATH)?;
    info!("Listening on UNIX socket: {}", SOCKET_PATH);

    loop {
        // TODO: Accept multiple connections.
        match listener.accept().await {
            Ok((stream, _addr)) => {
                info!("Client connection established.");

                let mut editor_message_rx = editor_message_tx.subscribe();

                let (mut read, mut write) = tokio::io::split(stream);
                let buf_reader = BufReader::new(&mut read);
                //for line in buf_reader.lines() {
                let mut lines = buf_reader.lines();

                loop {
                    tokio::select! {
                        line_maybe = lines.next_line() => {
                            match line_maybe {
                                Ok(Some(line)) => {
                                    match parse_message_from_editor(&line) {
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
                        editor_message = editor_message_rx.recv() => {
                            debug!("Received editor message.");
                            match editor_message {
                                Ok(EditorMessage::Insert {
                                    editor_revision,
                                    position,
                                    text,
                                }) => {
                                    let message = serde_json::json!({
                                        "method": "operation",
                                        "params": [
                                            editor_revision,
                                            [position, text]
                                        ],
                                    });
                                    write.write_all(format!("{}\n", message).as_bytes()).await?;
                                }
                                Ok(EditorMessage::Delete {
                                    editor_revision,
                                    position,
                                    length,
                                }) => {
                                    let message = serde_json::json!({
                                        "method": "operation",
                                        "params": [
                                            editor_revision,
                                            [position, -(length as i64)]
                                        ],
                                    });
                                    write.write_all(format!("{}\n", message).as_bytes()).await?;
                                }
                                Err(_) => {
                                    error!("Error receiving editor message.");
                                }
                            }
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
