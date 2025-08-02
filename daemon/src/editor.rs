// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! This module is all about daemon to editor communication.
use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::sandbox;
use crate::types::EditorProtocolObject;
use anyhow::{bail, Context, Result};
use futures::StreamExt;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use tokio::{
    io::WriteHalf,
    net::{UnixListener, UnixStream},
};
use tokio_util::{
    bytes::BytesMut,
    codec::{Encoder, FramedRead, FramedWrite, LinesCodec},
};
use tracing::{debug, info};

pub type EditorId = usize;

pub type EditorWriter = FramedWrite<WriteHalf<UnixStream>, EditorProtocolCodec>;

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
pub async fn spawn_socket_listener(
    socket_path: PathBuf,
    document_handle: DocumentActorHandle,
) -> Result<()> {
    // Make sure the parent directory of the socket is only accessible by the current user.
    if let Err(description) = is_user_readable_only(&socket_path) {
        panic!("{}", description);
    }

    // Using the sandbox method here is technically unnecessary,
    // but we want to really run all path operations through the sandbox module.
    // TODO: Use correct directory as guard.
    if sandbox::exists(Path::new("/"), Path::new(&socket_path))
        .expect("Failed to check existence of path")
    {
        let socket_path = socket_path.display();
        bail!("Detected an existing socket '{socket_path}'. Do you have a daemon running already? If not, you need to remove it manually.");
    }

    let listener = UnixListener::bind(&socket_path)?;
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
    let mut reader = FramedRead::new(stream_read, LinesCodec::new());
    let writer = FramedWrite::new(stream_write, EditorProtocolCodec);

    document_handle
        .send_message(DocMessage::NewEditorConnection(editor_id, writer))
        .await;
    info!("Editor #{editor_id} connected.");

    while let Some(Ok(line)) = reader.next().await {
        document_handle
            .send_message(DocMessage::FromEditor(editor_id, line))
            .await;
    }

    document_handle
        .send_message(DocMessage::CloseEditorConnection(editor_id))
        .await;
    info!("Editor #{editor_id} disconnected.");
}
