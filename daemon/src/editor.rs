// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! This module is all about daemon to editor communication.
use crate::cli::ask;
use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::editor_protocol::{
    EditorProtocolMessageError, EditorProtocolMessageFromEditor, IncomingMessage, JSONRPCResponse,
    OutgoingMessage,
};
use crate::sandbox;
use anyhow::{bail, Context, Result};
use futures::{SinkExt, StreamExt};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};
use tokio::{
    io::WriteHalf,
    net::{UnixListener, UnixStream},
};
use tokio_util::{
    bytes::BytesMut,
    codec::{Decoder, Encoder, FramedRead, FramedWrite, LinesCodec},
};
use tracing::{debug, error, info};

pub type EditorId = usize;

pub type EditorWriter = FramedWrite<WriteHalf<UnixStream>, OutgoingProtocolCodec>;

#[derive(Debug)]
pub struct OutgoingProtocolCodec;

impl Encoder<OutgoingMessage> for OutgoingProtocolCodec {
    type Error = anyhow::Error;

    fn encode(&mut self, item: OutgoingMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let payload = item.to_jsonrpc()?;
        dst.extend_from_slice(format!("{payload}\n").as_bytes());
        Ok(())
    }
}

#[derive(Debug)]
pub struct IncomingProtocolCodec;

impl Decoder for IncomingProtocolCodec {
    type Item = IncomingMessage;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        LinesCodec::new()
            .decode(src)?
            .map(|line| IncomingMessage::from_jsonrpc(&line))
            .transpose()
    }
}

fn is_user_readable_only(socket_path: &Path) -> Result<()> {
    let parent_dir = socket_path
        .parent()
        .context("The socket path should not be the root directory")?;
    let current_permissions = fs::metadata(parent_dir)
        .context("Expected to have access to metadata of the socket path's parent")?
        .permissions()
        .mode();
    // Group and others should not have any permissions.
    let allowed_permissions = 0o77700u32;
    if current_permissions | allowed_permissions != allowed_permissions {
        bail!("For security reasons, the parent directory of the socket must only be accessible by the current user. Please run `chmod go-rwx {:?}`", parent_dir);
    }
    Ok(())
}

/// # Panics
///
/// Will panic if we fail to listen on the socket, or if we fail to accept an incoming connection.
pub fn spawn_socket_listener(
    socket_path: &Path,
    document_handle: DocumentActorHandle,
) -> Result<()> {
    // Make sure the parent directory of the socket is only accessible by the current user.
    if let Err(description) = is_user_readable_only(socket_path) {
        panic!("{}", description);
    }

    // Using the sandbox method here is technically unnecessary,
    // but we want to really run all path operations through the sandbox module.
    // TODO: Use correct directory as guard.
    if sandbox::exists(Path::new("/"), Path::new(&socket_path))
        .expect("Failed to check existence of path")
    {
        let socket_path_display = socket_path.display();
        let remove_socket = ask(&format!("Detected an existing socket '{socket_path_display}'. There might be a daemon running already for this directory, or the previous one crashed. Do you want to continue?"));
        if remove_socket? {
            sandbox::remove_file(Path::new("/"), socket_path).expect("Could not remove socket");
        } else {
            bail!("Not continuing, make sure to stop all other daemons on this directory");
        }
    }

    let listener = UnixListener::bind(socket_path)?;
    debug!("Listening on UNIX socket: {}", socket_path.display());

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let id = document_handle.clone().next_editor_id();
                    let document_handle_clone = document_handle.clone();
                    tokio::spawn(async move {
                        handle_editor_connection(stream, document_handle_clone.clone(), id).await;
                    })
                }
                Err(err) => {
                    panic!("Error while accepting socket connection: {err}");
                }
            };
        }
    });

    Ok(())
}

async fn handle_editor_connection(
    stream: UnixStream,
    document_handle: DocumentActorHandle,
    editor_id: EditorId,
) {
    let (stream_read, stream_write) = tokio::io::split(stream);
    let mut reader = FramedRead::new(stream_read, IncomingProtocolCodec);
    let mut writer = FramedWrite::new(stream_write, OutgoingProtocolCodec);

    debug!("Editor #{editor_id} connected to socket. Awaiting initialization.");

    if let Some(message) = reader.next().await {
        match message {
            Ok(incoming_message) => {
                // TODO: refactor this into a helper
                let response = match incoming_message {
                    IncomingMessage::Request {
                        id,
                        payload: EditorProtocolMessageFromEditor::Initialize { version },
                    } => {
                        let expected_protocol_version = "0.8";
                        if version == expected_protocol_version {
                            info!("Editor #{editor_id} connected.");
                            JSONRPCResponse::RequestSuccess {
                                id,
                                result: "success".to_string(),
                            }
                        } else {
                            let response = JSONRPCResponse::RequestError {
                                id: None,
                                error: EditorProtocolMessageError {
                                    code: -1,
                                    message: "Wrong Version".into(),
                                    data: Some(format!(
                                        "Got {version}, wanted {expected_protocol_version}"
                                    )),
                                },
                            };
                            error!("Error for JSON-RPC request: {:?}", response);
                            response
                        }
                    }
                    // wrong initial request
                    IncomingMessage::Request { .. } | IncomingMessage::Notification { .. } => {
                        let response = JSONRPCResponse::RequestError {
                                id: None,
                                error: EditorProtocolMessageError {
                                    code: -32700,
                                    message: "Send 'initialize' request first".into(),
                                    data: Some(
                                        "Before anything else, the client needs to introduce itself by sending the expected version".into()
                                    ),
                                },
                            };
                        error!("Error for JSON-RPC request: {:?}", response);
                        response
                    }
                };
                let message = OutgoingMessage::Response(response);
                writer.send(message).await.unwrap_or_else(|err| {
                    error!("Failed to send message to editor: {err} Removing editor.");
                });
            }
            Err(e) => {
                // let response = JSONRPCResponse::RequestError {
                //     id: None,
                //     error: EditorProtocolMessageError {
                //         code: -32700,
                //         message: format!("Invalid request: {e}"),
                //         data: None,
                //     },
                // };
                error!("Error for JSON-RPC request: {:?}", e);
                todo!("handle error case");
                //self.send_to_editor_client(&editor_id, OutgoingMessage::Response(response))
                //    .await;
            }
        }

        document_handle
            .send_message(DocMessage::NewEditorConnection(editor_id, writer))
            .await;

        while let Some(message) = reader.next().await {
            match message {
                Ok(message) => {
                    document_handle
                        .send_message(DocMessage::FromEditor(editor_id, message))
                        .await;
                }
                Err(e) => {
                    let response = JSONRPCResponse::RequestError {
                        id: None,
                        error: EditorProtocolMessageError {
                            code: -32700,
                            message: format!("Invalid request: {e}"),
                            data: None,
                        },
                    };
                    error!("Error for JSON-RPC request: {:?}", response);
                    let message = OutgoingMessage::Response(response);
                    document_handle
                        .send_message(DocMessage::ToEditor(editor_id, message))
                        .await;
                }
            }
        }

        document_handle
            .send_message(DocMessage::CloseEditorConnection(editor_id))
            .await;
    }
    // Err(e) => {
    // }

    info!("Editor #{editor_id} disconnected.");
}
