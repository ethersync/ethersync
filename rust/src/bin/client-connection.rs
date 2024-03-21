use curl::easy::{Easy, List};
use serde_json::Value;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::str::from_utf8;

fn add_id(mut value: Value) -> Value {
    if let Value::Object(ref mut obj) = &mut value {
        obj.insert("id".to_string(), Value::String("42".to_string()));
    }
    value
}

fn request(data: &[u8]) {
    let data: Value = serde_json::from_str(from_utf8(data).unwrap()).unwrap();
    let data = dbg!(add_id(data).to_string());

    let mut easy = Easy::new();
    easy.url("http://127.0.0.1:9000").unwrap();
    easy.post(true).unwrap();
    easy.post_field_size(data.len() as u64).unwrap();
    let mut list = List::new();
    list.append("Content-type:application/json").unwrap();
    easy.http_headers(list).unwrap();

    let mut transfer = easy.transfer();
    transfer
        .read_function(|buf| Ok(data.as_bytes().read(buf).unwrap_or(0)))
        .unwrap();
    transfer.perform().unwrap();
}

fn main() -> io::Result<()> {
    let bytes = io::stdin().bytes();
    let mut file = File::create("/Users/mn/code/ethersync/ethersync/output-from-vim.txt")?;
    let mut in_json = false;
    let mut byte_incoming = vec![];

    request(r#"{"jsonrpc": "2.0", "method": "insert", "params": ["Hello, world!"]}"#.as_bytes());
    for byte in bytes {
        // println!("got a line: {}", line.unwrap());
        // DO NOT TRY THIS AT HOME!!
        let byte = byte.unwrap();
        if byte == b'{' {
            in_json = true;
        }
        if byte == b'}' {
            in_json = false;
            // file.write_all(&[byte])?;
            byte_incoming.push(byte);
            // send here and start over!
            file.write_all(&byte_incoming)?;
            request(&byte_incoming);
            // dbg!(res.body_string().await?);
            byte_incoming.clear();
        }
        if in_json {
            byte_incoming.push(byte);
            // file.write_all(&[byte])?;
        }
    }
    Ok(())
}
