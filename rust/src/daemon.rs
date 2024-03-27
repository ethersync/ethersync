use automerge::{
    patches::TextRepresentation,
    sync::{Message, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, PatchLog, ReadDoc,
};
use rand::{distributions::Alphanumeric, Rng};
use std::fs;
use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf},
    net::UnixListener,
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, oneshot},
};

const SOCKET_PATH: &str = "/tmp/ethersync";

type SharedPeerState = Arc<Mutex<SyncState>>;

#[derive(Debug)]
enum DocMessage {
    Init,
    RandomEdit,
    Insert {
        position: usize,
        text: String,
    },
    ReceiveSyncMessage {
        message: Message,
        state: SharedPeerState,
    },
    GenerateSyncMessage {
        state: SharedPeerState,
        response: oneshot::Sender<Option<Message>>,
    },
}

type DocMessageSender = mpsc::Sender<DocMessage>;
type DocChangedSender = broadcast::Sender<()>;

// Launch the daemon. Optionally, connect to given peer.
pub async fn launch(peer: Option<String>) {
    let mut doc = AutoCommit::new();

    let (doc_changed_tx, _doc_changed_rx) = broadcast::channel::<()>(16);
    let doc_changed_tx_clone = doc_changed_tx.clone();
    let (message_tx, mut message_rx) = mpsc::channel(1);

    tokio::spawn(async move {
        loop {
            let message = message_rx.recv().await.unwrap();
            match message {
                DocMessage::Init => {
                    let _text = doc
                        .put_object(automerge::ROOT, "text", ObjType::Text)
                        .unwrap();
                    // In the beginning, no-one might be interested in these messages, so the
                    // send might fail, I think?
                    let _ = doc_changed_tx.send(());
                }
                DocMessage::RandomEdit => {
                    let text_obj = doc.get(automerge::ROOT, "text").unwrap();
                    if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
                        let text_length = doc.text(&text_obj).unwrap().len();
                        let random_string: String = rand::thread_rng()
                            .sample_iter(&Alphanumeric)
                            .take(1)
                            .map(char::from)
                            .collect();
                        let random_position = rand::thread_rng().gen_range(0..(text_length + 1));
                        doc.insert(text_obj, random_position, random_string)
                            .unwrap();
                        let _ = doc_changed_tx.send(());
                    }
                }
                DocMessage::Insert { position, text } => {
                    let text_obj = doc.get(automerge::ROOT, "text").unwrap();
                    if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj {
                        doc.insert(text_obj, position, text).unwrap();
                        let _ = doc_changed_tx.send(());
                    }
                }
                DocMessage::ReceiveSyncMessage { message, state } => {
                    let mut patch_log = PatchLog::active(TextRepresentation::String);
                    let mut state = state.lock().unwrap();
                    doc.sync()
                        .receive_sync_message_log_patches(&mut state, message, &mut patch_log)
                        .unwrap();
                    let patches = doc.make_patches(&mut patch_log);
                    dbg!(&patches);
                    // TODO: Send patches to OT.
                    let _ = doc_changed_tx.send(());
                }
                DocMessage::GenerateSyncMessage { state, response } => {
                    let mut state = state.lock().unwrap();
                    let message = doc.sync().generate_sync_message(&mut state);
                    response.send(message).unwrap();
                }
            }

            let text = doc.get(automerge::ROOT, "text").unwrap();
            if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text {
                println!("My text is now: {}", doc.text(&text_obj).unwrap());
            }
        }
    });

    // Make edits to the document occasionally. TODO: Seems to slow something down.
    if false {
        let tx = message_tx.clone();
        tokio::spawn(async move {
            loop {
                //let doc_clone2 = doc_clone.clone();
                {
                    match tx.send(DocMessage::RandomEdit).await {
                        Ok(_) => {
                            println!("Random edit sent.");
                        }
                        Err(e) => {
                            println!("Error sending random edit: {:#?}", e);
                        }
                    }
                }

                thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    }

    // Dial peer, or listen for incoming connections.
    let tx = message_tx.clone();
    if let Some(peer) = peer {
        dial_tcp(tx, doc_changed_tx_clone, peer).await.unwrap();
    } else {
        let tx_clone = tx.clone();
        tokio::spawn(async {
            listen_socket(tx_clone).await;
        });
        listen_tcp(tx, doc_changed_tx_clone).await.unwrap();
    }
}

async fn listen_tcp(tx: DocMessageSender, doc_changed_tx: DocChangedSender) -> io::Result<()> {
    tx.send(DocMessage::Init).await.unwrap();

    let listener = TcpListener::bind("0.0.0.0:4242").await?;
    println!("Listening on TCP port: {}", listener.local_addr().unwrap());

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
) -> io::Result<()> {
    let stream = TcpStream::connect(addr).await?;

    start_sync(tx, doc_changed_tx, stream).await?;

    Ok(())
}

async fn start_sync(
    tx: DocMessageSender,
    doc_changed_tx: DocChangedSender,
    stream: TcpStream,
) -> io::Result<()> {
    // TODO: To avoid having to put this into an Arc<Mutex>, implement a channel-based mechanism.
    // A "syncer" thread would own the state. A "receive" thread just listens for messages, and
    // sends them to the syncer. The syncer subscribes to the doc_changed channel, and, if pinged,
    // requests sync messages from the document, and sends them to the "send" thread.
    let peer_state = SyncState::new();
    let peer_state = Arc::new(Mutex::new(peer_state));

    let (read, write) = tokio::io::split(stream);

    let peer_state_clone = peer_state.clone();
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        match sync_receive(read, tx_clone, peer_state_clone).await {
            Ok(_) => {
                println!("Sync receive OK.");
            }
            Err(e) => {
                println!("Error sync_receive: {:#?}", e);
            }
        }
    });

    sync_send(write, tx, doc_changed_tx, peer_state).await?;

    Ok(())
}

async fn listen_socket(tx: DocMessageSender) {
    fs::remove_file(SOCKET_PATH).unwrap();
    let listener = UnixListener::bind(SOCKET_PATH).unwrap();
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
                while let Some(line) = lines.next_line().await.unwrap() {
                    let json: serde_json::Value = serde_json::from_str(&line).unwrap();
                    match json {
                        serde_json::Value::Object(map) => {
                            if let Some(serde_json::Value::String(method)) = map.get("method") {
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
                                                    let position =
                                                        position.as_u64().unwrap() as usize;
                                                    let text = text.as_str().to_string();
                                                    tx.send(DocMessage::Insert { position, text })
                                                        .await
                                                        .unwrap();
                                                } else {
                                                    println!("Invalid text param.");
                                                }
                                            } else {
                                                println!("Invalid position param.");
                                            }
                                        } else {
                                            println!("Invalid insert params.");
                                        }
                                    }
                                    _ => {
                                        println!("Unknown method: {}", method);
                                    }
                                }
                            }
                        }
                        _ => {
                            println!("Invalid JSON: {:#?}", json);
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

async fn sync_receive(
    mut reader: ReadHalf<TcpStream>,
    tx: DocMessageSender,
    state: SharedPeerState,
) -> io::Result<()> {
    loop {
        let mut message_len_buf = [0; 4];
        reader.read_exact(&mut message_len_buf).await?;
        let message_len = i32::from_be_bytes(message_len_buf);
        let mut message_buf = vec![0; message_len as usize];
        reader.read_exact(&mut message_buf).await?;
        let message = Message::decode(&message_buf).unwrap();

        tx.send(DocMessage::ReceiveSyncMessage {
            message,
            state: state.clone(),
        })
        .await
        .unwrap();
    }
}

async fn sync_send(
    mut writer: WriteHalf<TcpStream>,
    tx: DocMessageSender,
    doc_changed_tx: DocChangedSender,
    state: SharedPeerState,
) -> io::Result<()> {
    let mut rx = doc_changed_tx.subscribe();
    loop {
        loop {
            let message_maybe = {
                let (response_tx, response_rx) = oneshot::channel();

                tx.send(DocMessage::GenerateSyncMessage {
                    state: state.clone(),
                    response: response_tx,
                })
                .await
                .unwrap();

                response_rx.await.unwrap()
            };

            if let Some(message) = message_maybe {
                // TODO: clone is only called to print the message later
                let message_buf = message.clone().encode();
                let message_len = message_buf.len() as i32;
                writer.write_all(&message_len.to_be_bytes()).await?;
                writer.write_all(&message_buf).await?;
            } else {
                break;
            }
        }

        // Wait for a message on the doc_changed channel.
        rx.recv().await.unwrap();
    }
}
