use jsonrpc_core::IoHandler;
use std::fs;
use std::io;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::thread;

const SOCKET_PATH: &str = "/tmp/ethersync";

pub fn run() -> io::Result<()> {
    fs::remove_file(SOCKET_PATH).unwrap();
    let listener = UnixListener::bind(SOCKET_PATH).unwrap();
    println!("Daemon started.");

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        thread::spawn(move || {
            println!("Connection established.");

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
            println!("Connection closed.");
        });
    }

    Ok(())
}
