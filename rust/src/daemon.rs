use automerge::{
    sync::{Message, State as SyncState, SyncDoc},
    transaction::Transactable,
    AutoCommit, ObjType, ReadDoc,
};
use jsonrpc_core::IoHandler;
use rand::{distributions::Alphanumeric, Rng};
use std::fs;
use std::io;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::thread;

const SOCKET_PATH: &str = "/tmp/ethersync";

pub struct Daemon {
    doc: Arc<Mutex<automerge::AutoCommit>>,
}

impl Daemon {
    pub fn new() -> Self {
        let doc = AutoCommit::new();
        let doc = Arc::new(Mutex::new(doc));

        Self { doc }
    }

    pub fn launch(&mut self, peer: Option<String>) {
        thread::spawn(|| {
            Self::listen_socket().unwrap();
        });

        if let Some(peer) = peer {
            self.dial_tcp(peer).unwrap();
        } else {
            self.listen_tcp().unwrap();
        }
    }

    pub fn listen_socket() -> io::Result<()> {
        fs::remove_file(SOCKET_PATH).unwrap();
        let listener = UnixListener::bind(SOCKET_PATH).unwrap();
        println!("Listening on UNIX socket: {}", SOCKET_PATH);

        for stream in listener.incoming() {
            let stream = stream.unwrap();

            thread::spawn(move || {
                println!("Client connection established.");

                let mut io = IoHandler::new();
                io.add_notification("insert", |params| {
                    println!("insert called: {:#?}", params);
                });

                let buf_reader = BufReader::new(&stream);
                for line in buf_reader.lines() {
                    let line = line.unwrap();
                    println!("Request: {:#?}", line);
                    let response = io.handle_request_sync(&line);
                    println!("Response: {:#?}", response);
                }
                println!("Client connection closed.");
            });
        }

        Ok(())
    }

    fn init_text(&mut self) {
        let _text = self
            .doc
            .lock()
            .unwrap()
            .put_object(automerge::ROOT, "text", ObjType::Text)
            .unwrap();
    }

    fn edit_text(doc: Arc<Mutex<AutoCommit>>) {
        println!("Trying to edit text!");
        let mut doc = doc.lock().unwrap();
        println!("Editing text!");
        let text = doc.get(automerge::ROOT, "text").unwrap();
        if let Some((automerge::Value::Object(ObjType::Text), text)) = text {
            let text_length = doc.text(&text).unwrap().len();
            let random_string: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(1)
                .map(char::from)
                .collect();
            let random_position = rand::thread_rng().gen_range(0..(text_length + 1));
            doc.insert(text, random_position, random_string).unwrap();
        } else {
            println!("Text object not found.");
        }
    }

    fn listen_tcp(&mut self) -> io::Result<()> {
        self.init_text();
        //Self::edit_text(&mut self.doc);

        let listener = TcpListener::bind("0.0.0.0:4242").unwrap();
        println!("Listening on TCP port: {}", listener.local_addr().unwrap());

        for stream in listener.incoming() {
            let stream = stream.unwrap();

            // TODO: Allow more than one peer to dial us at the same time.
            println!("Peer dialed us.");
            self.start_sync(stream)?;
        }

        Ok(())
    }

    // Connect to IP and port.
    fn dial_tcp(&mut self, addr: String) -> io::Result<()> {
        let stream = TcpStream::connect(addr)?;
        //let result = self.sync_with_peer(&mut stream, true);
        self.start_sync(stream)?;
        Ok(())
    }

    fn start_sync(&mut self, stream: TcpStream) -> io::Result<()> {
        let peer_state = SyncState::new();
        let peer_state = Arc::new(Mutex::new(peer_state));

        let stream2 = stream.try_clone().unwrap();

        let peer_state_clone = peer_state.clone();
        let doc_clone = self.doc.clone();
        thread::spawn(move || {
            Self::sync_receive(stream, doc_clone, peer_state_clone).unwrap();
        });

        let doc_clone = self.doc.clone();
        thread::spawn(move || loop {
            {
                let doc_clone = doc_clone.clone();
                Self::edit_text(doc_clone);
                thread::sleep(std::time::Duration::from_secs(5));
            }
        });

        let doc_clone = self.doc.clone();
        Self::sync_send(stream2, doc_clone, peer_state).unwrap();

        Ok(())
    }

    fn sync_receive(
        mut reader: TcpStream,
        doc: Arc<Mutex<AutoCommit>>,
        state: Arc<Mutex<SyncState>>,
    ) -> io::Result<()> {
        loop {
            let mut message_len_buf = [0; 4];
            reader.read_exact(&mut message_len_buf)?;
            let message_len = i32::from_be_bytes(message_len_buf);
            let mut message_buf = vec![0; message_len as usize];
            reader.read_exact(&mut message_buf)?;
            let message = Message::decode(&message_buf).unwrap();
            println!("Received message: {:?}", message);

            println!("Trying to lock doc");
            let mut doc = doc.lock().unwrap();
            let mut state = state.lock().unwrap();
            println!("Locked doc");

            doc.sync()
                .receive_sync_message(&mut state, message)
                .unwrap();

            let text = doc.get(automerge::ROOT, "text").unwrap();
            if let Some((automerge::Value::Object(ObjType::Text), text)) = text {
                println!("My text is now: {}", doc.text(&text).unwrap());
            } else {
                println!("I don't have text yet.");
            }
        }
    }

    fn sync_send(
        mut writer: TcpStream,
        doc: Arc<Mutex<AutoCommit>>,
        state: Arc<Mutex<SyncState>>,
    ) -> io::Result<()> {
        loop {
            let message_maybe = {
                let mut doc = doc.lock().unwrap();
                let mut state = state.lock().unwrap();

                let message = doc.sync().generate_sync_message(&mut state);
                message
            };

            if let Some(message) = message_maybe {
                let message_buf = message.encode();
                let message_len = message_buf.len() as i32;
                writer.write_all(&message_len.to_be_bytes())?;
                writer.write_all(&message_buf)?;

                println!("Sent message: {:?}", &message_buf);
            } else {
                thread::sleep(std::time::Duration::from_secs(1));
                println!("No message to send.");
            };
        }
    }
}
