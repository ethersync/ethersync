use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::str::from_utf8;
use std::thread;

// Read/write JSON-RPC requests that have a Content-Length header from/to stdin/stdout.
// Read/write newline-delimited JSON-RPC from/to the Unix socket.
pub fn connection(socket_path: &Path) {
    let mut stream = UnixStream::connect(socket_path).expect("Failed to connect to socket");

    let stream2 = stream.try_clone().expect("Failed to clone socket stream");
    let reader = BufReader::new(stream2);

    thread::spawn(|| {
        for line in reader.lines() {
            let line = line.expect("Failed to read line");
            let length = line.len();
            print!("Content-Length: {length}\r\n\r\n{line}");
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
