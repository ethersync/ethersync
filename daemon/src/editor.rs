//! This module is all about daemon to editor communication.
use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::types::EditorProtocolObject;
use anyhow::{Result};
use futures::StreamExt;
use std::{
    io,
    path::{Path, PathBuf},
};
use tokio::io::WriteHalf;
use tokio_util::{
    bytes::BytesMut,
    codec::{Encoder, FramedRead, FramedWrite, LinesCodec},
};
use tracing::info;

pub type EditorId = usize;

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

// ------------------------------------------------------------------------------------
// Unix-specific imports and definitions
// ------------------------------------------------------------------------------------
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use crate::sandbox;
#[cfg(unix)]
use anyhow::{bail, Context};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

#[cfg(unix)]
pub type EditorWriter = FramedWrite<WriteHalf<UnixStream>, EditorProtocolCodec>;

#[cfg(unix)]
fn get_fallback_socket_dir() -> String {
    let socket_dir = format!(
        "/tmp/ethersync-{}",
        std::env::var("USER").expect("$USER should be set")
    );
    if !fs::exists(&socket_dir).expect("Should be able to test for existence of directory in /tmp")
    {
        fs::create_dir(&socket_dir).expect("Should be able to create a directory in /tmp");
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&socket_dir, permissions)
            .expect("Should be able to set permissions for a directory we just created");
    }
    socket_dir
}

#[cfg(unix)]
fn is_valid_socket_name(socket_name: &Path) -> Result<()> {
    if socket_name.components().count() != 1 {
        bail!("The socket name must be a single path component");
    }
    if let std::path::Component::Normal(_) = socket_name
        .components()
        .next()
        .expect("The component count of socket_name was previously checked to be non-empty")
    {
        // All good :)
    } else {
        bail!("The socket name must be a plain filename");
    }
    Ok(())
}

#[cfg(unix)]
pub fn get_socket_path(socket_name: &Path) -> PathBuf {
    let socket_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| get_fallback_socket_dir());
    let socket_dir = Path::new(&socket_dir);
    if let Err(description) = is_valid_socket_name(&socket_name) {
        panic!("{}", description);
    }
    socket_dir.join(socket_name)
}

#[cfg(unix)]
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
        bail!("For security reasons, the parent directory of the socket must only be accessible by the current user");
    }
    Ok(())
}

/// # Panics
///
/// Will panic if we fail to listen on the socket, or if we fail to accept an incoming connection.
#[cfg(unix)]
pub async fn make_editor_connection(socket_path: PathBuf, document_handle: DocumentActorHandle) {
    // Make sure the parent directory of the socket is only accessible by the current user.
    if let Err(description) = is_user_readable_only(&socket_path) {
        panic!("{}", description);
    }

    // Using the sandbox method here is technically unnecessary,
    // but we want to really run all path operations through the sandbox module.
    if sandbox::exists(Path::new("/"), Path::new(&socket_path))
        .expect("Failed to check existence of path")
    {
        sandbox::remove_file(Path::new("/"), &socket_path).expect("Could not remove socket");
    }

    let result = accept_editor_loop(&socket_path, document_handle).await;
    match result {
        Ok(()) => {}
        Err(err) => {
            panic!("Failed to make editor connection: {err}");
        }
    }
}

#[cfg(unix)]
async fn accept_editor_loop(
    socket_path: &Path,
    document_handle: DocumentActorHandle,
) -> Result<(), io::Error> {
    let listener = UnixListener::bind(socket_path)?;
    info!("Listening on UNIX socket: {}", socket_path.display());

    loop {
        let (stream, _addr) = listener.accept().await?;

        let id = document_handle.next_editor_id();

        spawn_editor_connection(stream, document_handle.clone(), id);
    }
}

#[cfg(unix)]
fn spawn_editor_connection(
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
        info!("Client #{editor_id} connected");

        while let Some(Ok(line)) = reader.next().await {
            document_handle
                .send_message(DocMessage::FromEditor(editor_id, line))
                .await;
        }

        document_handle
            .send_message(DocMessage::CloseEditorConnection(editor_id))
            .await;
        info!("Client #{editor_id} disconnected");
    });
}

// ------------------------------------------------------------------------------------
// Windows-specific imports and definitions
// ------------------------------------------------------------------------------------
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions, PipeMode};

#[cfg(windows)]
pub type EditorWriter = FramedWrite<WriteHalf<NamedPipeServer>, EditorProtocolCodec>;

#[cfg(windows)]
pub fn get_socket_path(_socket_name: &Path) -> PathBuf {
    PathBuf::from("ethersync_pipe")
}

#[cfg(windows)]
pub async fn make_editor_connection(socket_path: PathBuf, document_handle: DocumentActorHandle) {
    let result = accept_editor_loop(socket_path, document_handle).await;
    match result {
        Ok(()) => {}
        Err(err) => {
            panic!("Failed to make editor connection: {err}");
        }
    }
}

#[cfg(windows)]
async fn accept_editor_loop(socket_path: PathBuf, document_handle: DocumentActorHandle) -> io::Result<()> {
    let pipe_name = format!(r"\\.\pipe\{}", socket_path.to_str().unwrap().split('\\').last().unwrap());
    loop {
        let mut server_options = ServerOptions::new();
        server_options.pipe_mode(PipeMode::Byte);
        let pipe: NamedPipeServer = server_options.create(&pipe_name)?;
        info!("Listening for connections on named pipe: {}", pipe_name);

        // Wait asynchronously for a client to connect
        pipe.connect().await?;
        info!("Client connected!");
        let id = document_handle.next_editor_id();
        spawn_editor_connection(pipe, document_handle.clone(), id);
    }
}

#[cfg(windows)]
fn spawn_editor_connection(stream: NamedPipeServer, document_handle: DocumentActorHandle, editor_id: EditorId) {
    tokio::spawn(async move {
        let (stream_read, stream_write) = tokio::io::split(stream);
        let mut reader = FramedRead::new(stream_read, LinesCodec::new());
        let writer = FramedWrite::new(stream_write, EditorProtocolCodec);

        document_handle.send_message(DocMessage::NewEditorConnection(editor_id, writer)).await;
        info!("Client #{editor_id} connected");

        while let Some(Ok(line)) = reader.next().await {
            document_handle
                .send_message(DocMessage::FromEditor(editor_id, line))
                .await;
        }

        document_handle
            .send_message(DocMessage::CloseEditorConnection(editor_id))
            .await;
        info!("Client #{editor_id} disconnected");
    });
}
