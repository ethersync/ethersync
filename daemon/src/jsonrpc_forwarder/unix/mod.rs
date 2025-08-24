use crate::jsonrpc_forwarder::JsonRPCForwarder;
use async_trait::async_trait;
use std::path::Path;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

pub struct UnixJsonRPCForwarder {}

#[async_trait(?Send)]
impl JsonRPCForwarder<OwnedReadHalf, OwnedWriteHalf> for UnixJsonRPCForwarder {
    async fn connect_stream(
        &self,
        socket_path: &Path,
    ) -> anyhow::Result<(
        FramedRead<OwnedReadHalf, LinesCodec>,
        FramedWrite<OwnedWriteHalf, LinesCodec>,
    )> {
        // Unix domain socket approach
        let stream = UnixStream::connect(socket_path).await?;
        let (read_half, write_half) = stream.into_split();
        let reader = FramedRead::new(read_half, LinesCodec::new());
        let writer = FramedWrite::new(write_half, LinesCodec::new());

        Ok((reader, writer))
    }
}
