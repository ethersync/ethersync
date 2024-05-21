/// This module is all about daemon to editor communication.
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf},
    net::UnixStream,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::types::EditorProtocolMessage;

pub type EditorMessageSender = mpsc::Sender<EditorProtocolMessage>;
pub type EditorMessageReceiver = mpsc::Receiver<EditorProtocolMessage>;

pub async fn spawn_editor_connection(stream: UnixStream, document_handle: DocumentActorHandle) {
    let editor_handle = EditorHandle::new(stream, document_handle.clone());
    document_handle
        .send_message(DocMessage::NewEditorConnection(editor_handle))
        .await;
}

pub struct EditorHandle {
    editor_message_tx: EditorMessageSender,
}

impl EditorHandle {
    pub fn new(stream: UnixStream, document_handle: DocumentActorHandle) -> Self {
        // The document task will send messages intended for the socket connection on this channel.
        let (socket_message_tx, socket_message_rx) = mpsc::channel::<EditorProtocolMessage>(1);
        let (stream_read, stream_write) = tokio::io::split(stream);
        let shutdown_token = CancellationToken::new();

        let mut reader = SocketReadActor::new(stream_read, shutdown_token.clone(), document_handle);
        tokio::spawn(async move { reader.run().await });

        let mut writer = SocketWriteActor::new(stream_write, socket_message_rx, shutdown_token);
        tokio::spawn(async move { writer.run().await });
        Self {
            editor_message_tx: socket_message_tx,
        }
    }

    pub async fn send(&self, message: EditorProtocolMessage) {
        self.editor_message_tx
            .send(message)
            .await
            .expect("Failed to send to editor.");
    }
}

pub struct SocketReadActor {
    reader: ReadHalf<UnixStream>,
    shutdown_token: CancellationToken,
    document_handle: DocumentActorHandle,
}

impl SocketReadActor {
    pub fn new(
        reader: ReadHalf<UnixStream>,
        shutdown_token: CancellationToken,
        document_handle: DocumentActorHandle,
    ) -> Self {
        Self {
            reader,
            shutdown_token,
            document_handle,
        }
    }

    pub async fn run(&mut self) {
        let buf_reader = BufReader::new(&mut self.reader);
        let mut lines = buf_reader.lines();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    debug!("Got a line from the client: {:#?}", line);
                    let jsonrpc = EditorProtocolMessage::from_jsonrpc(&line)
                        .expect("Failed to parse JSON-RPC message");
                    self.document_handle
                        .send_message(DocMessage::FromEditor(jsonrpc))
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
        info!("Client disconnect.");
    }
}

pub struct SocketWriteActor {
    writer: WriteHalf<UnixStream>,
    shutdown_token: CancellationToken,
    editor_message_receiver: EditorMessageReceiver,
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

    async fn write_to_socket(&mut self, message: EditorProtocolMessage) {
        debug!("Received editor message to send to it.");
        let payload = message
            .to_jsonrpc()
            .expect("Failed to serialize JSON-RPC message");
        debug!("Sending message to editor: {:#?}", payload);
        self.writer
            .write_all(format!("{payload}\n").as_bytes())
            .await
            .expect("Failed to write to socket");
    }

    pub async fn run(&mut self) {
        // We're sending an editor message to the client.
        loop {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => {
                    debug!("Shutting down JSON-RPC sender (due to socket disconnet)");
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
