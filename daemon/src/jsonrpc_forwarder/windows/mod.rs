use crate::jsonrpc_forwarder::JsonRPCForwarder;
use std::path::Path;
use async_trait::async_trait;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, PipeMode};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

pub struct WindowsJsonRPCForwarder {
}

#[async_trait(?Send)]
impl JsonRPCForwarder<ReadHalf<NamedPipeClient>, WriteHalf<NamedPipeClient>> for WindowsJsonRPCForwarder {
    async fn connect_stream(&self, socket_path: &Path) -> anyhow::Result<(
        FramedRead<ReadHalf<NamedPipeClient>, LinesCodec>,
        FramedWrite<WriteHalf<NamedPipeClient>, LinesCodec>,
    )> {
        // Convert the Path to a UTF-8 string and prepend the named pipe prefix
        let pipe_name = format!(
            r"\\.\pipe\{}",
            socket_path.to_str().unwrap().split('\\').last().unwrap()
        );

        // Attempt to create the client
        // todo: check if there are security options we could set here
        let mut client_options = ClientOptions::new();
        client_options.pipe_mode(PipeMode::Byte);
        let client = client_options.open(&pipe_name)?;

        // Split the named pipe into read and write halves
        let (read_half, write_half) = tokio::io::split(client);

        // Create FramedRead and FramedWrite for line-based codec
        let reader = FramedRead::new(read_half, LinesCodec::new());
        let writer = FramedWrite::new(write_half, LinesCodec::new());

        Ok((reader, writer))
    }
}