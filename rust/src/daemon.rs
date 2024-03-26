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
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::UnixListener;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::mpsc;

const SOCKET_PATH: &str = "/tmp/ethersync";

type SharedState = Arc<Mutex<SyncState>>;

#[derive(Debug)]
enum DocMessage {
    Init,
    RandomEdit,
    Insert { position: usize, text: String },
}

#[derive(Clone)]
pub struct Doc {
    doc: Arc<Mutex<AutoCommit>>,
    doc_changed_tx: broadcast::Sender<()>,
    message_tx: mpsc::Sender<DocMessage>,
}

impl Doc {
    pub fn new() -> Self {
        let doc = Arc::new(Mutex::new(AutoCommit::new()));

        let (doc_changed_tx, _doc_changed_rx) = broadcast::channel::<()>(16);
        let (message_tx, mut message_rx) = mpsc::channel(1);

        let doc = Self {
            doc,
            message_tx,
            doc_changed_tx: doc_changed_tx.clone(),
        };

        let doc_clone = doc.clone();
        tokio::spawn(async move {
            loop {
                let message = message_rx.recv().await.unwrap();
                println!("Doc received message: {:#?}", message);
                match message {
                    DocMessage::Init => {
                        let _text = doc_clone
                            .doc
                            .lock()
                            .unwrap()
                            .put_object(automerge::ROOT, "text", ObjType::Text)
                            .unwrap();
                    }
                    DocMessage::RandomEdit => {
                        let mut doc = doc_clone.doc.lock().unwrap();
                        let text_obj = doc.get(automerge::ROOT, "text").unwrap();
                        if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj
                        {
                            let text_length = doc.text(&text_obj).unwrap().len();
                            let random_string: String = rand::thread_rng()
                                .sample_iter(&Alphanumeric)
                                .take(1)
                                .map(char::from)
                                .collect();
                            let random_position =
                                rand::thread_rng().gen_range(0..(text_length + 1));
                            doc.insert(text_obj, random_position, random_string)
                                .unwrap();
                        }
                    }
                    DocMessage::Insert { position, text } => {
                        let mut doc = doc_clone.doc.lock().unwrap();
                        let text_obj = doc.get(automerge::ROOT, "text").unwrap();
                        if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj
                        {
                            doc.insert(text_obj, position, text).unwrap();
                        }
                    }
                }

                // In the beginning, no-one might be interested in these messages, so the
                // send might fail, I think?
                let _ = doc_changed_tx.send(());
                println!("Processed message.");
            }
        });

        // Make edits to the document occasionally.
        if false {
            let doc_clone = doc.clone();
            tokio::spawn(async move {
                loop {
                    //let doc_clone2 = doc_clone.clone();
                    match doc_clone.message_tx.send(DocMessage::RandomEdit).await {
                        Ok(_) => {
                            println!("Random edit sent.");
                        }
                        Err(e) => {
                            println!("Error sending random edit: {:#?}", e);
                        }
                    }

                    thread::sleep(std::time::Duration::from_secs(2));
                }
            });
        }

        // When doc is changed, print content.
        let doc_clone = doc.clone();
        tokio::spawn(async move {
            let mut rx = doc_clone.doc_changed_tx.subscribe();
            loop {
                rx.recv().await.unwrap();
                let doc = doc_clone.doc.lock().unwrap();
                let text = doc.get(automerge::ROOT, "text").unwrap();
                if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text {
                    println!("My text is now: {}", doc.text(&text_obj).unwrap());
                }
            }
        });

        doc
    }
}

pub async fn launch(doc: Doc, peer: Option<String>) {
    if let Some(peer) = peer {
        dial_tcp(doc, peer).await.unwrap();
    } else {
        let doc_clone = doc.clone();
        tokio::spawn(async move {
            listen_socket(doc_clone).await;
        });
        listen_tcp(doc).await.unwrap();
    }
}

async fn listen_tcp(doc: Doc) -> io::Result<()> {
    //init_text(doc.clone());
    doc.message_tx.send(DocMessage::Init).await.unwrap();

    let listener = TcpListener::bind("0.0.0.0:4242").await?;
    println!("Listening on TCP port: {}", listener.local_addr().unwrap());

    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            println!("Error accepting connection.");
            continue;
        };

        // TODO: Allow more than one peer to dial us at the same time.
        let doc = doc.clone();
        tokio::spawn(async move {
            println!("Peer dialed us.");
            match start_sync(doc, stream).await {
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

// Connect to IP and port.
async fn dial_tcp(doc: Doc, addr: String) -> io::Result<()> {
    let stream = TcpStream::connect(addr).await?;

    //let result = self.sync_with_peer(&mut stream, true);
    start_sync(doc.clone(), stream).await?;

    Ok(())
}

async fn start_sync(doc: Doc, stream: TcpStream) -> io::Result<()> {
    let peer_state = SyncState::new();
    let peer_state = Arc::new(Mutex::new(peer_state));

    let (read, write) = tokio::io::split(stream);

    let doc_clone = doc.clone();
    let peer_state_clone = peer_state.clone();
    tokio::spawn(async move {
        match sync_receive(read, doc_clone, peer_state_clone).await {
            Ok(_) => {
                println!("Sync receive OK.");
            }
            Err(e) => {
                println!("Error sync_receive: {:#?}", e);
            }
        }
    });

    let doc_clone = doc.clone();
    sync_send(write, doc_clone, peer_state).await?;

    Ok(())
}

pub async fn listen_socket(doc: Doc) {
    fs::remove_file(SOCKET_PATH).unwrap();
    let listener = UnixListener::bind(SOCKET_PATH).unwrap();
    println!("Listening on UNIX socket: {}", SOCKET_PATH);

    loop {
        match listener.accept().await {
            Ok((mut stream, _addr)) => {
                println!("Client connection established.");

                let buf_reader = BufReader::new(&mut stream);
                //for line in buf_reader.lines() {
                let mut lines = buf_reader.lines();

                while let Some(line) = lines.next_line().await.unwrap() {
                    let json: serde_json::Value = serde_json::from_str(&line).unwrap();
                    println!("Request: {:#?}", json);
                    //doc.message_tx.send(DocMessage::RandomEdit).await;
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
                                                    doc.message_tx
                                                        .send(DocMessage::Insert { position, text })
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
    doc: Doc,
    state: SharedState,
) -> io::Result<()> {
    loop {
        let mut message_len_buf = [0; 4];
        reader.read_exact(&mut message_len_buf).await?;
        let message_len = i32::from_be_bytes(message_len_buf);
        let mut message_buf = vec![0; message_len as usize];
        reader.read_exact(&mut message_buf).await?;
        let message = Message::decode(&message_buf).unwrap();
        println!("Received message: {:?}", message);

        let mut patch_log = PatchLog::active(TextRepresentation::String);

        let mut docc = doc.doc.lock().unwrap();
        let mut state = state.lock().unwrap();

        docc.sync()
            .receive_sync_message_log_patches(&mut state, message, &mut patch_log)
            .unwrap();

        let patches = docc.make_patches(&mut patch_log);
        dbg!(&patches);

        doc.doc_changed_tx.send(()).unwrap();

        // TODO: Send these patches to the editors via the OT component.
    }
}

async fn sync_send(
    mut writer: WriteHalf<TcpStream>,
    doc: Doc,
    state: SharedState,
) -> io::Result<()> {
    let mut rx = doc.doc_changed_tx.subscribe();
    loop {
        loop {
            let message_maybe = {
                let mut doc = doc.doc.lock().unwrap();
                let mut state = state.lock().unwrap();

                let message = doc.sync().generate_sync_message(&mut state);
                message
            };

            if let Some(message) = message_maybe {
                // TODO: clone is only called to print the message later
                let message_buf = message.clone().encode();
                let message_len = message_buf.len() as i32;
                writer.write_all(&message_len.to_be_bytes()).await?;
                writer.write_all(&message_buf).await?;

                println!("Sent message: {:?}", &message);
            } else {
                break;
            }
        }

        // Wait for a message on the doc_changed channel.
        rx.recv().await.unwrap();
    }
}
