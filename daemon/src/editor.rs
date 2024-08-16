//! This module is all about daemon to editor communication.
use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::types::EditorProtocolObject;
use futures::StreamExt;
use tokio::{io::WriteHalf, net::UnixStream};
use tokio_util::{
    bytes::BytesMut,
    codec::{Encoder, FramedRead, FramedWrite, LinesCodec},
};
use tracing::info;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct EditorId(pub usize);

pub type EditorHandle = FramedWrite<WriteHalf<UnixStream>, EditorProtocolCodec>;

pub struct EditorProtocolCodec;

impl Encoder<EditorProtocolObject> for EditorProtocolCodec {
    type Error = anyhow::Error;

    fn encode(
        &mut self,
        item: EditorProtocolObject,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        let payload = item.to_jsonrpc()?;
        dst.extend_from_slice(format!("{payload}\n").as_bytes());
        Ok(())
    }
}

pub async fn spawn_editor_connection(
    stream: UnixStream,
    document_handle: DocumentActorHandle,
    editor_id: EditorId,
) {
    tokio::spawn(async move {
        let (stream_read, stream_write) = tokio::io::split(stream);
        let mut reader = FramedRead::new(stream_read, LinesCodec::new());
        let writer = FramedWrite::new(stream_write, EditorProtocolCodec);

        document_handle
            .send_message(DocMessage::NewEditorConnection(editor_id, writer))
            .await;
        info!("Client #{} connected", editor_id.0);

        while let Some(Ok(line)) = reader.next().await {
            document_handle
                .send_message(DocMessage::FromEditor(editor_id, line))
                .await;
        }

        document_handle
            .send_message(DocMessage::CloseEditorConnection(editor_id))
            .await;
        info!("Client #{} disconnected", editor_id.0);
    });
}
