use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::str::from_utf8;
use std::thread;

const SOCKET_PATH: &str = "/tmp/ethersync";

// Read JSON-RPC requests that have a Content-Length header from standard input.
// Write newline-delimited JSON-RPC to the Unix socket.
pub fn connection() {
    let mut stream = UnixStream::connect(SOCKET_PATH).expect("Failed to connect to socket");

    let stream2 = stream.try_clone().expect("Failed to clone socket stream");

    thread::spawn(|| {
        for byte in stream2.bytes() {
            let byte = byte.expect("Failed to read byte");
            print!("{}", byte as char);
            std::io::stdout().flush().expect("Failed to flush stdout");
        }
    });

    let mut data = vec![];
    let mut reading_header = true;
    let mut content_length = 0;

    for byte in io::stdin().lock().bytes() {
        let byte = byte.expect("Failed to read byte");
        data.push(byte);

        if reading_header {
            if data.ends_with(&[b'\r', b'\n', b'\r', b'\n']) {
                let header_string = from_utf8(&data).expect("Failed to parse header as UTF-8");
                content_length = 0;
                for line in header_string.lines() {
                    if let Some(line) = line.strip_prefix("Content-Length: ") {
                        content_length = line
                            .parse()
                            .expect("Failed to parse Content-Length as integer");
                    }
                }
                println!("Content-Length: {}", content_length);
                data.clear();
                reading_header = false;
            }
        } else if data.len() == content_length {
            stream.write_all(&data).expect("Failed to write to socket");
            stream.write_all(b"\n").expect("Failed to write to socket");
            data.clear();
            reading_header = true;
        }
    }
}
