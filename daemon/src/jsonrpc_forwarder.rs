use futures::{SinkExt, StreamExt};
use std::path::Path;
use tokio::io::{BufReader, BufWriter};
use tokio::net::UnixStream;
use tokio_util::bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite, LinesCodec};

pub async fn connection(socket_path: &Path) -> anyhow::Result<()> {
    // Construct socket object, which send/receive newline-delimited messages.
    let stream = UnixStream::connect(socket_path).await?;
    let (socket_read, socket_write) = stream.into_split();
    let mut socket_read = FramedRead::new(socket_read, LinesCodec::new());
    let mut socket_write = FramedWrite::new(socket_write, LinesCodec::new());

    // Construct stdin/stdout objects, which send/receive messages with a Content-Length header.
    let mut stdin = FramedRead::new(BufReader::new(tokio::io::stdin()), ContentLengthCodec);
    let mut stdout = FramedWrite::new(BufWriter::new(tokio::io::stdout()), ContentLengthCodec);

    tokio::spawn(async move {
        while let Some(Ok(message)) = socket_read.next().await {
            stdout
                .send(message)
                .await
                .expect("Failed to write to stdout");
        }
        // Socket was closed.
        std::process::exit(0);
    });

    while let Some(Ok(message)) = stdin.next().await {
        socket_write.send(message).await?;
    }
    // Stdin was closed.
    std::process::exit(0);
}

struct ContentLengthCodec;

impl Encoder<String> for ContentLengthCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let content_length = item.len();
        dst.extend_from_slice(format!("Content-Length: {}\r\n\r\n", content_length).as_bytes());
        dst.extend_from_slice(item.as_bytes());
        Ok(())
    }
}

impl Decoder for ContentLengthCodec {
    type Item = String;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Find the position of the Content-Length header.
        let c = b"Content-Length: ";
        let start_of_header = match src.windows(c.len()).position(|window| window == c) {
            Some(pos) => pos,
            None => return Ok(None),
        };

        // Find the end of the line after that.
        let end_of_line = match src[start_of_header + c.len()..]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        {
            Some(pos) => pos,
            None => return Ok(None),
        };

        // Parse the content length.
        let content_length = std::str::from_utf8(
            &src[start_of_header + c.len()..start_of_header + c.len() + end_of_line],
        )?
        .parse()?;
        let content_start = start_of_header + c.len() + end_of_line + 4;

        // Recommended optimization, in anticipation for future calls to `decode`.
        src.reserve(content_start + content_length);

        // Check if we have enough content.
        if src.len() < content_start + content_length {
            return Ok(None);
        }

        // Return the body of the message.
        src.advance(content_start);
        let content = src.split_to(content_length);
        Ok(Some(std::str::from_utf8(&content)?.to_string()))
    }
}
