use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::str::from_utf8;

const SOCKET_PATH: &str = "/tmp/ethersync";

// Read JSON-RPC requests that have a Content-Length header from standard input.
// Write newline-delimited JSON-RPC to the Unix socket.
pub fn connection() -> io::Result<()> {
    let mut stream = UnixStream::connect(SOCKET_PATH).unwrap();

    let mut data = vec![];
    let mut reading_header = true;
    let mut content_length = 0;

    for byte in io::stdin().lock().bytes() {
        let byte = byte?;
        data.push(byte);

        if reading_header {
            if data.ends_with(&[b'\r', b'\n', b'\r', b'\n']) {
                let header_string = from_utf8(&data).unwrap();
                content_length = 0;
                for line in header_string.lines() {
                    if let Some(line) = line.strip_prefix("Content-Length: ") {
                        content_length = line.parse().unwrap();
                    }
                }
                println!("Content-Length: {}", content_length);
                data.clear();
                reading_header = false;
            }
        } else if data.len() == content_length {
            stream.write_all(&data)?;
            stream.write_all(b"\n")?;
            data.clear();
            reading_header = true;
        }
    }

    Ok(())
}
