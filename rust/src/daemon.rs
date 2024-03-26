use automerge::{
    patches::TextRepresentation,
    sync::{Message, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, PatchLog, ReadDoc,
};
use jsonrpc_core::IoHandler;
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

type SharedDoc = Arc<Mutex<automerge::AutoCommit>>;
type SharedState = Arc<Mutex<SyncState>>;

#[derive(Debug)]
enum DocMessage {
    Init,
    Insert { position: usize, text: String },
}

#[derive(Clone)]
pub struct Doc {
    doc: Arc<Mutex<AutoCommit>>,
    //doc_changed_tx: broadcast::Sender<()>,
    //doc_changed_rx: broadcast::Receiver<()>,
    message_tx: mpsc::Sender<DocMessage>,
    //message_rx: mpsc::Receiver<DocMessage>,
}

impl Doc {
    pub fn new() -> Self {
        let doc = Arc::new(Mutex::new(AutoCommit::new()));

        let (_doc_changed_tx, _doc_changed_rx) = broadcast::channel::<()>(16);
        let (message_tx, mut message_rx) = mpsc::channel(1);

        let doc = Self { doc, message_tx };

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
                        //doc_changed_tx.send(());
                    }
                    DocMessage::Insert { position, text } => {
                        let mut doc = doc_clone.doc.lock().unwrap();
                        let text_obj = doc.get(automerge::ROOT, "text").unwrap();
                        if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text_obj
                        {
                            doc.insert(text_obj, position, text).unwrap();
                            //doc_changed_tx.send(());

                            //println!("My text is now: {}", doc.text(&text_obj).unwrap());
                        }
                    }
                }
                println!("Processed message.");
            }
        });

        // Make edits to the document occasionally.
        let doc_clone = doc.clone();
        tokio::spawn(async move {
            loop {
                let doc_clone2 = doc_clone.clone();
                edit_text(doc_clone2).await;

                //show text
                let doc_clone2 = doc_clone.clone();
                let doc = doc_clone2.doc.lock().unwrap();
                let text = doc.get(automerge::ROOT, "text").unwrap();
                if let Some((automerge::Value::Object(ObjType::Text), text_obj)) = text {
                    println!("My text is now: {}", doc.text(&text_obj).unwrap());
                }

                thread::sleep(std::time::Duration::from_secs(2));
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
    //edit_text(&mut self.doc);
    doc.message_tx.send(DocMessage::Init).await.unwrap();

    let listener = TcpListener::bind("0.0.0.0:4242").await?;
    println!("Listening on TCP port: {}", listener.local_addr().unwrap());

    loop {
        let (stream, _addr) = listener.accept().await?;

        // TODO: Allow more than one peer to dial us at the same time.
        println!("Peer dialed us.");
        start_sync(doc.clone(), stream).await?;
    }
}

// Connect to IP and port.
async fn dial_tcp(doc: Doc, addr: String) -> io::Result<()> {
    let stream = TcpStream::connect(addr).await.unwrap();
    //let result = self.sync_with_peer(&mut stream, true);
    start_sync(doc, stream).await?;
    Ok(())
}

async fn start_sync(doc: Doc, stream: TcpStream) -> io::Result<()> {
    let peer_state = SyncState::new();
    let peer_state = Arc::new(Mutex::new(peer_state));

    let (read, write) = tokio::io::split(stream);

    let doc_clone = doc.clone();
    let peer_state_clone = peer_state.clone();
    tokio::spawn(async move {
        sync_receive(read, doc_clone, peer_state_clone)
            .await
            .unwrap();
    });

    let doc_clone = doc.clone();
    sync_send(write, doc_clone, peer_state).await.unwrap();

    Ok(())
}

async fn edit_text(doc: Doc) {
    doc.message_tx
        .send(DocMessage::Insert {
            position: 0,
            text: "a".to_string(),
        })
        .await;
    //println!("Trying to edit text!");
    //let mut doc = doc.lock().unwrap();
    //println!("Editing text!");
    //let text = doc.get(automerge::ROOT, "text").unwrap();
    //if let Some((automerge::Value::Object(ObjType::Text), text)) = text {
    //    let text_length = doc.text(&text).unwrap().len();
    //    let random_string: String = rand::thread_rng()
    //        .sample_iter(&Alphanumeric)
    //        .take(1)
    //        .map(char::from)
    //        .collect();
    //    let random_position = rand::thread_rng().gen_range(0..(text_length + 1));
    //    doc.insert(text, random_position, random_string).unwrap();
    //} else {
    //    println!("Text object not found.");
    //}
}

pub async fn listen_socket(doc: Doc) {
    fs::remove_file(SOCKET_PATH).unwrap();
    let listener = UnixListener::bind(SOCKET_PATH).unwrap();
    println!("Listening on UNIX socket: {}", SOCKET_PATH);

    loop {
        match listener.accept().await {
            Ok((mut stream, _addr)) => {
                println!("Client connection established.");

                //let mut io = IoHandler::new();
                //let tx = doc.message_tx.clone();
                //io.add_notification("insert", move |params| {
                //    tokio::spawn(async move {
                //        println!("insert called: {:#?}", params);

                //        // TODO: For now, interpret all insert calls as insert(0, "a").
                //        tx.send(DocMessage::Insert {
                //            position: 0,
                //            text: "a".to_string(),
                //        })
                //        .await;
                //        //let mut doc = doc_clone.lock().unwrap();
                //        //let text = doc.get(automerge::ROOT, "text").unwrap();
                //        //if let Some((automerge::Value::Object(ObjType::Text), text)) = text {
                //        //    doc.insert(text, 0, "a".to_string()).unwrap();
                //        //}
                //    });
                //});

                let buf_reader = BufReader::new(&mut stream);
                //for line in buf_reader.lines() {
                let mut lines = buf_reader.lines();

                while let Some(line) = lines.next_line().await.unwrap() {
                    println!("Request: {:#?}", line);

                    //let response = io.handle_request_sync(&line);
                    doc.message_tx
                        .send(DocMessage::Insert {
                            position: 0,
                            text: "a".to_string(),
                        })
                        .await;

                    //println!("Response: {:#?}", response);
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
        reader.read_exact(&mut message_len_buf).await;
        let message_len = i32::from_be_bytes(message_len_buf);
        let mut message_buf = vec![0; message_len as usize];
        reader.read_exact(&mut message_buf).await;
        let message = Message::decode(&message_buf).unwrap();
        println!("Received message: {:?}", message);

        let mut patch_log = PatchLog::active(TextRepresentation::String);

        let mut doc = doc.doc.lock().unwrap();
        let mut state = state.lock().unwrap();

        doc.sync()
            .receive_sync_message_log_patches(&mut state, message, &mut patch_log)
            .unwrap();
        let patches = doc.make_patches(&mut patch_log);
        dbg!(&patches);
        // TODO: Send these patches to the editors via the OT component.
    }
}

async fn sync_send(
    mut writer: WriteHalf<TcpStream>,
    doc: Doc,
    state: SharedState,
) -> io::Result<()> {
    loop {
        let message_maybe = {
            let mut doc = doc.doc.lock().unwrap();
            let mut state = state.lock().unwrap();

            let message = doc.sync().generate_sync_message(&mut state);
            message
        };

        if let Some(message) = message_maybe {
            let message_buf = message.encode();
            let message_len = message_buf.len() as i32;
            writer.write_all(&message_len.to_be_bytes()).await;
            writer.write_all(&message_buf).await;

            println!("Sent message: {:?}", &message_buf);
        } else {
            thread::sleep(std::time::Duration::from_secs(1));
            println!("No message to send.");
        };
    }
}
