use local_ip_address::local_ip;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tokio::net::{TcpListener, TcpStream, UnixListener};
use tracing::{error, info};

use crate::daemon::{DocMessage, DocumentActorHandle, EditorHandle};
use crate::peer::spawn_peer_sync;

pub async fn make_peer_connection(
    port: Option<u16>,
    peer: Option<String>,
    document_handle: DocumentActorHandle,
) {
    let result = if let Some(peer) = peer {
        connect_with_peer(peer, document_handle).await
    } else {
        let port = port.unwrap_or(4242);
        accept_peer_loop(port, document_handle).await
    };
    match result {
        Ok(()) => { /* successfully connected/started accept loop */ }
        Err(err) => {
            panic!("Failed to make connection: {err}");
        }
    }
}

pub async fn make_editor_connection(
    socket_path: PathBuf,
    file_path: PathBuf,
    document_handle: DocumentActorHandle,
) {
    if Path::new(&socket_path).exists() {
        fs::remove_file(&socket_path).expect("Could not remove/re-initialize socket");
    }
    let result = accept_editor_loop(&socket_path, &file_path, document_handle).await;
    match result {
        Ok(()) => {}
        Err(err) => {
            error!("Failed to make editor connection: {err}");
        }
    }
}

async fn accept_editor_loop(
    socket_path: &Path,
    file_path: &Path,
    document_handle: DocumentActorHandle,
) -> Result<(), io::Error> {
    let listener = UnixListener::bind(socket_path)?;
    info!("Listening on UNIX socket: {}", socket_path.display());

    loop {
        let (stream, _addr) = listener.accept().await?;
        info!("Client connection established.");

        let editor_handle = EditorHandle::new(stream, document_handle.clone(), file_path);
        document_handle
            .send_message(DocMessage::NewEditorConnection(editor_handle))
            .await;
    }
}

async fn connect_with_peer(
    address: String,
    document_handle: DocumentActorHandle,
) -> Result<(), io::Error> {
    let stream = TcpStream::connect(address).await?;
    info!("Connected to Peer.");
    spawn_peer_sync(stream, document_handle);
    Ok(())
}

async fn accept_peer_loop(
    port: u16,
    document_handle: DocumentActorHandle,
) -> Result<(), io::Error> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    if let Ok(ip) = local_ip() {
        info!("Listening on local TCP: {}:{}", ip, port);
    }

    if let Some(ip) = public_ip::addr().await {
        info!("Listening on public TCP: {}:{}", ip, port);
    }

    loop {
        let (stream, _addr) = listener.accept().await?;
        info!("Peer dialed us.");
        spawn_peer_sync(stream, document_handle.clone());
    }
}
