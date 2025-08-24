use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{bail, Context};
use futures::StreamExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};
use tracing::{debug, info};
use crate::cli::ask;
use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::editor::{handle_editor_connection, Editor, EditorId, EditorProtocolCodec, EditorStream};

pub struct EditorWindows{
    pub pipe_name: PathBuf,
}

impl Editor for EditorWindows {

    fn get_socket_path(&self) -> PathBuf {
        self.pipe_name.clone()
    }

    /// # Panics
    ///
    /// Will panic if we fail to listen on the socket, or if we fail to accept an incoming connection.
    fn spawn_socket_listener(&self, document_handle: DocumentActorHandle) -> anyhow::Result<()> {

        let pipe_name = format!(
            r"\\.\pipe\{}",
            self.get_socket_path().to_str().unwrap().split('\\').last().unwrap()
        );

        tokio::spawn(async move {
            loop {
                let mut server_options = ServerOptions::new();
                server_options.pipe_mode(PipeMode::Byte);
                // todo reject_remote_clients(true); // only allow local connections
                // todo only allow current user to connect => custom security_desriptor
                let pipe: NamedPipeServer = server_options.create(&pipe_name).unwrap();
                info!("Listening for connections on named pipe: {}", pipe_name);
                // Wait asynchronously for a client to connect
                match pipe.connect().await {
                Ok(()) => {
                info!("Client connected!");
                let id = document_handle.clone().next_editor_id();
                let document_handle_clone = document_handle.clone();
                tokio::spawn(async move {
                    handle_editor_connection(pipe, document_handle_clone.clone(), id).await;
                });

                }
                    Err(err) => {
                    panic!("Error while accepting socket connection: {err}");
                    }
                };
            }
        });

        Ok(())
    }
}