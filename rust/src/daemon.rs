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
use std::thread;

const SOCKET_PATH: &str = "/tmp/ethersync";

pub struct Daemon {
    doc: automerge::AutoCommit,
}

impl Daemon {
    pub fn new() -> Self {
        let doc = AutoCommit::new();

        Self { doc }
    }

    pub fn listen_socket(&self) -> io::Result<()> {
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
            .put_object(automerge::ROOT, "text", ObjType::Text)
            .unwrap();
    }

    fn edit_text(&mut self) {
        let text = self.doc.get(automerge::ROOT, "text").unwrap();
        if let Some((automerge::Value::Object(ObjType::Text), text)) = text {
            let text_length = self.doc.text(&text).unwrap().len();
            let random_string: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(1)
                .map(char::from)
                .collect();
            let random_position = rand::thread_rng().gen_range(0..(text_length + 1));
            self.doc
                .insert(text, random_position, random_string)
                .unwrap();
        } else {
            panic!("Text object not found.");
        }
    }

    pub fn listen_tcp(&mut self) -> io::Result<()> {
        self.init_text();
        self.edit_text();

        let listener = TcpListener::bind("0.0.0.0:4242").unwrap();
        println!("Listening on TCP port: {}", listener.local_addr().unwrap());

        for stream in listener.incoming() {
            let mut stream = stream.unwrap();

            // TODO: Allow more than one peer to dial us at the same time.
            println!("Peer dialed us.");
            let result = self.sync_with_peer(&mut stream, false);
            if let Err(e) = result {
                println!("Error: {:?}", e);
            }
        }

        Ok(())
    }

    // Connect to IP and port.
    pub fn dial_tcp(&mut self, addr: &str) -> io::Result<()> {
        let mut stream = TcpStream::connect(addr)?;
        let result = self.sync_with_peer(&mut stream, true);
        if let Err(e) = result {
            println!("Error: {:?}", e);
        }
        Ok(())
    }

    fn sync_with_peer(
        &mut self,
        stream: &mut TcpStream,
        mut skip_first_send: bool,
    ) -> io::Result<()> {
        let mut peer_state = SyncState::new();
        loop {
            // Send sync message.
            if skip_first_send {
                skip_first_send = false;
            } else if let Some(message) = self.doc.sync().generate_sync_message(&mut peer_state) {
                let message_buf = message.encode();
                let message_len = message_buf.len() as u32;
                stream.write_all(&message_len.to_be_bytes())?;
                stream.write_all(&message_buf)?;
            } else {
                let message_len = 0i32;
                stream.write_all(&message_len.to_be_bytes())?;
            }

            // Receive sync message.
            let mut message_len_buf = [0; 4];
            stream.read_exact(&mut message_len_buf)?;
            let message_len = u32::from_be_bytes(message_len_buf);
            if (message_len as usize) == 0 {
            } else {
                let mut message_buf = vec![0; message_len as usize];
                stream.read_exact(&mut message_buf)?;
                let message = Message::decode(&message_buf).unwrap();
                self.doc
                    .sync()
                    .receive_sync_message(&mut peer_state, message)
                    .unwrap();
            }
            let text = self.doc.get(automerge::ROOT, "text").unwrap();
            if let Some((automerge::Value::Object(ObjType::Text), text)) = text {
                println!("My text is now: {}", self.doc.text(&text).unwrap());

                // Sometimes edit the text.
                if rand::thread_rng().gen_bool(0.5) {
                    self.edit_text();
                }
            } else {
                println!("I don't have text yet.");
            }

            thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}
