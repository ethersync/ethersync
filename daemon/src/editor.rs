/// This module is all about daemon to editor communication.
use std::io;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf},
    net::UnixStream,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace};

use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::types::{EditorProtocolObject, JSONRPCFromEditor};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct EditorId(pub usize);

type EditorMessageSender = mpsc::Sender<EditorProtocolObject>;
type EditorMessageReceiver = mpsc::Receiver<EditorProtocolObject>;

pub async fn spawn_editor_connection(
    stream: UnixStream,
    document_handle: DocumentActorHandle,
    editor_id: EditorId,
) {
    let editor_handle = EditorHandle::new(editor_id, stream, document_handle.clone());
    document_handle
        .send_message(DocMessage::NewEditorConnection(editor_handle))
        .await;
}

pub struct EditorHandle {
    pub id: EditorId,
    editor_message_tx: EditorMessageSender,
    shutdown_token: CancellationToken,
}

impl EditorHandle {
    pub fn new(id: EditorId, stream: UnixStream, document_handle: DocumentActorHandle) -> Self {
        // The document task will send messages intended for the socket connection on this channel.
        let (socket_message_tx, socket_message_rx) = mpsc::channel::<EditorProtocolObject>(1);
        let (stream_read, stream_write) = tokio::io::split(stream);
        let shutdown_token = CancellationToken::new();

        let mut reader =
            SocketReadActor::new(stream_read, shutdown_token.clone(), document_handle, id);
        tokio::spawn(async move { reader.run().await });

        let mut writer =
            SocketWriteActor::new(stream_write, socket_message_rx, shutdown_token.clone());
        tokio::spawn(async move { writer.run().await });
        Self {
            id,
            editor_message_tx: socket_message_tx,
            shutdown_token,
        }
    }

    pub async fn send(&self, message: EditorProtocolObject) -> Result<(), io::Error> {
        // Can fail during shutdown or editor disconnect, when Actors already have been killed/closed
        if self.editor_message_tx.send(message).await.is_err() {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Can't keep up or dead",
            ))
        } else {
            Ok(())
        }
    }
}

impl Drop for EditorHandle {
    fn drop(&mut self) {
        debug!("Editor Handle dropped, shutting down socket actors");
        self.shutdown_token.cancel();
    }
}

pub struct SocketReadActor {
    reader: ReadHalf<UnixStream>,
    shutdown_token: CancellationToken,
    document_handle: DocumentActorHandle,
    editor_id: EditorId,
}

impl SocketReadActor {
    pub fn new(
        reader: ReadHalf<UnixStream>,
        shutdown_token: CancellationToken,
        document_handle: DocumentActorHandle,
        editor_id: EditorId,
    ) -> Self {
        Self {
            reader,
            shutdown_token,
            document_handle,
            editor_id,
        }
    }

    pub async fn run(&mut self) {
        let buf_reader = BufReader::new(&mut self.reader);
        let mut lines = buf_reader.lines();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    trace!("Got a line from the client: {:#?}", line);
                    let jsonrpc = JSONRPCFromEditor::from_jsonrpc(&line)
                        .expect("Failed to parse JSON-RPC message");
                    self.document_handle
                        .send_message(DocMessage::FromEditor(self.editor_id, jsonrpc))
                        .await;
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    error!("Error reading line: {:#?}", e);
                }
            }
        }
        self.shutdown_token.cancel();
        self.document_handle
            .send_message(DocMessage::CloseEditorConnection(self.editor_id))
            .await;
        info!("Client disconnected");
    }
}

pub struct SocketWriteActor {
    writer: WriteHalf<UnixStream>,
    editor_message_receiver: EditorMessageReceiver,
    shutdown_token: CancellationToken,
}

impl SocketWriteActor {
    pub fn new(
        writer: WriteHalf<UnixStream>,
        editor_message_receiver: EditorMessageReceiver,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            writer,
            editor_message_receiver,
            shutdown_token,
        }
    }

    async fn write_to_socket(&mut self, message: EditorProtocolObject) {
        let payload = message
            .to_jsonrpc()
            .expect("Failed to serialize JSON-RPC message");
        trace!("Sending message to editor: {:#?}", payload);
        self.writer
            .write_all(format!("{payload}\n").as_bytes())
            .await
            .expect("Failed to write to socket");
    }

    pub async fn run(&mut self) {
        // We're sending an editor message to the client.
        loop {
            tokio::select! {
                () = self.shutdown_token.cancelled() => {
                    debug!("Shutting down JSON-RPC sender (due to socket disconnect)");
                    break;
                }
                editor_message_maybe = self.editor_message_receiver.recv() => match editor_message_maybe {
                    None => {
                        panic!("Editor message channel has been closed. How did this happen?");
                    }
                    Some(editor_message) => {
                        self.write_to_socket(editor_message).await;
                    }
                }
            }
        }
    }
}
