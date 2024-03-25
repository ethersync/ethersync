use serde_json::Value;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::str::from_utf8;

fn request(mut stream: &UnixStream, data: &[u8]) {
    let data: Value = serde_json::from_str(from_utf8(data).unwrap()).unwrap();
    stream.write_all(data.to_string().as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
}

fn main() -> io::Result<()> {
    let bytes = io::stdin().bytes();
    let mut in_json = false;
    let mut byte_incoming = vec![];

    let stream = UnixStream::connect("/tmp/ethersync").unwrap();

    for byte in bytes {
        let byte = byte.unwrap();
        if byte == b'{' {
            in_json = true;
        }
        if byte == b'}' {
            in_json = false;
            byte_incoming.push(byte);
            io::stderr().write_all(&byte_incoming)?;
            request(&stream, &byte_incoming);
            byte_incoming.clear();
        }
        if in_json {
            byte_incoming.push(byte);
        }
    }

    Ok(())
}
