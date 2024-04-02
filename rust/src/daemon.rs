use anyhow::Result;
use automerge::{
    patches::TextRepresentation,
    sync::{Message, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, PatchLog, ReadDoc,
};
use rand::{distributions::Alphanumeric, Rng};
use std::fs;
use std::thread;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf},
    net::UnixListener,
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, oneshot},
};

const SOCKET_PATH: &str = "/tmp/ethersync";

// These messages are sent to the task that owns the document.
enum DocMessage {
    Init,
    RandomEdit,
    Insert {
        position: usize,
        text: String,
    },
    Delete {
        position: usize,
        length: usize,
    },
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

type SyncerMessageSender = mpsc::Sender<SyncerMessage>;

// Launch the daemon. Optionally, connect to given peer.
pub async fn launch(peer: Option<String>) {
    let mut doc = AutoCommit::new();

    let (doc_changed_tx, _doc_changed_rx) = broadcast::channel::<()>(16);
    let doc_changed_tx_clone = doc_changed_tx.clone();
    let (message_tx, mut message_rx) = mpsc::channel(1);

    tokio::spawn(async move {
        loop {
            let message = message_rx
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
                        panic!("Automerge document doesn't have a text object, so I can't edit randomly");
                    }
                }
                DocMessage::Insert { position, text } => {
                    let text_obj = doc
                        .get(automerge::ROOT, "text")
                        .expect("Failed to get text object from Automerge document");
                    if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
                        doc.insert(text_obj, position, text)
                            .expect("Failed to insert into Automerge text object");
                        // TODO: Call apply_editor_operation in OT.
                        let _ = doc_changed_tx.send(());
                    } else {
                        panic!("Automerge document doesn't have a text object, so I can't insert");
                    }
                }
                DocMessage::Delete { position, length } => {
                    let text_obj = doc
                        .get(automerge::ROOT, "text")
                        .expect("Failed to get text object from Automerge document");
                    if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
                        doc.splice_text(text_obj, position, length as isize, "")
                            .expect("Failed to splice Automerge text object");
                        // TODO: Call apply_editor_operation in OT.
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
                    dbg!(&patches);
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
                    response_tx
                        .send((peer_state, message))
                        .expect("Failed to send peer state and sync message in response to GenerateSyncMessage");
                }
            }

            let text = doc
                .get(automerge::ROOT, "text")
                .expect("Failed to get text object from the Automerge document");

            if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text {
                println!(
                    "My text is now: {}",
                    doc.text(&text_obj)
                        .expect("Failed to get string from Automerge text object")
                );
            }
        }
    });

    // Make edits to the document occasionally. TODO: Seems to slow something down.
    if false {
        let tx = message_tx.clone();
        tokio::spawn(async move {
            loop {
                tx.send(DocMessage::RandomEdit)
                    .await
                    .expect("Failed to send random edit");

                thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    }

    // Dial peer, or listen for incoming connections.
    let tx = message_tx.clone();
    if let Some(peer) = peer {
        dial_tcp(tx, doc_changed_tx_clone, peer)
            .await
            .expect("Failed to dial peer");
    } else {
        let tx_clone = tx.clone();
        tokio::spawn(async {
            listen_socket(tx_clone)
                .await
                .expect("Failed to listen on UNIX socket");
        });
        listen_tcp(tx, doc_changed_tx_clone)
            .await
            .expect("Failed to listen on TCP port");
    }
}

async fn listen_tcp(tx: DocMessageSender, doc_changed_tx: DocChangedSender) -> Result<()> {
    tx.send(DocMessage::Init).await?;

    let listener = TcpListener::bind("0.0.0.0:4242").await?;
    println!("Listening on TCP port: {}", listener.local_addr()?);

    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            println!("Error accepting connection.");
            continue;
        };

        let tx = tx.clone();
        let doc_changed_tx = doc_changed_tx.clone();
        tokio::spawn(async move {
            println!("Peer dialed us.");
            match start_sync(tx, doc_changed_tx, stream).await {
                Ok(_) => {
                    println!("Sync OK?!");
                }
                Err(e) => {
                    println!("Error: {:#?}", e);
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
                println!("Sync receive OK.");
            }
            Err(e) => {
                println!("Error sync_receive: {:#?}", e);
            }
        }
    });

    // Generate sync message when doc changes.
    let reader_message_tx_clone = reader_message_tx.clone();
    tokio::spawn(async move {
        loop {
            reader_message_tx_clone
                .send(SyncerMessage::GenerateSyncMessage {})
                .await
                .expect("Failed to send GenerateSyncMessage to document task");
            doc_changed_tx
                .subscribe()
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

async fn listen_socket(tx: DocMessageSender) -> Result<()> {
    fs::remove_file(SOCKET_PATH)?;
    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("Listening on UNIX socket: {}", SOCKET_PATH);

    loop {
        // TODO: Accept multiple connections.
        match listener.accept().await {
            Ok((mut stream, _addr)) => {
                println!("Client connection established.");

                let buf_reader = BufReader::new(&mut stream);
                //for line in buf_reader.lines() {
                let mut lines = buf_reader.lines();

                // TODO: Write this in a nicer way...
                while let Some(line) = lines.next_line().await? {
                    let json: serde_json::Value = serde_json::from_str(&line)?;
                    match json {
                        serde_json::Value::Object(map) => {
                            if let Some(serde_json::Value::String(method)) = map.get("method") {
                                // TODO: Make this prettier, maybe with a Serde JSON schema?
                                match method.as_str() {
                                    "insert" => {
                                        if let Some(serde_json::Value::Array(array)) =
                                            map.get("params")
                                        {
                                            if let Some(serde_json::Value::Number(position)) =
                                                array.get(2)
                                            {
                                                if let Some(serde_json::Value::String(text)) =
                                                    array.get(3)
                                                {
                                                    let position = position
                                                        .as_u64()
                                                        .expect("Failed to parse position");
                                                    let text = text.as_str().to_string();
                                                    tx.send(DocMessage::Insert {
                                                        position: position as usize,
                                                        text,
                                                    })
                                                    .await?;
                                                } else {
                                                    panic!("Invalid text param");
                                                }
                                            } else {
                                                panic!("Invalid position param");
                                            }
                                        } else {
                                            panic!("Invalid insert params");
                                        }
                                    }
                                    "delete" => {
                                        if let Some(serde_json::Value::Array(array)) =
                                            map.get("params")
                                        {
                                            if let Some(serde_json::Value::Number(position)) =
                                                array.get(2)
                                            {
                                                if let Some(serde_json::Value::Number(length)) =
                                                    array.get(3)
                                                {
                                                    let position = position
                                                        .as_u64()
                                                        .expect("Failed to parse position");
                                                    let length = length
                                                        .as_u64()
                                                        .expect("Failed to parse length");
                                                    tx.send(DocMessage::Delete {
                                                        position: position as usize,
                                                        length: length as usize,
                                                    })
                                                    .await?;
                                                } else {
                                                    panic!("Invalid length param");
                                                }
                                            } else {
                                                panic!("Invalid position param");
                                            }
                                        } else {
                                            panic!("Invalid delete params");
                                        }
                                    }
                                    _ => {
                                        println!("Unknown method: {}", method);
                                    }
                                }
                            }
                        }
                        _ => {
                            panic!("Invalid JSON: {:#?}", json);
                        }
                    }
                }
                println!("Client connection closed.");
            }
            Err(e) => {
                println!("Error: {:#?}", e);
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
