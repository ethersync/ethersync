/// This module is all about daemon to editor communication.
use std::path::PathBuf;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf},
    net::UnixStream,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::daemon::DocumentActorHandle;
use crate::types::{EditorProtocolMessage, RevisionedEditorTextDelta};

pub type EditorMessageSender = mpsc::Sender<RevisionedEditorTextDelta>;
pub type EditorMessageReceiver = mpsc::Receiver<RevisionedEditorTextDelta>;

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
                    self.document_handle.send_message(jsonrpc.into()).await;
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
    file_path: PathBuf,
}

impl SocketWriteActor {
    pub fn new(
        writer: WriteHalf<UnixStream>,
        editor_message_receiver: EditorMessageReceiver,
        shutdown_token: CancellationToken,
        file_path: PathBuf,
    ) -> Self {
        Self {
            writer,
            editor_message_receiver,
            shutdown_token,
            file_path,
        }
    }

    async fn write_to_socket(&mut self, rev_delta: RevisionedEditorTextDelta) {
        debug!("Received editor message to send to it.");
        let message = EditorProtocolMessage::Edit {
            uri: format!("file://{}", self.file_path.display()),
            delta: rev_delta,
        };
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
