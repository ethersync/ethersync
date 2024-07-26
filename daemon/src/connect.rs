use local_ip_address::local_ip;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tokio::net::{TcpListener, TcpStream, UnixListener};
use tokio::time::{timeout, Duration};
use tracing::{info, warn};

use crate::daemon::DocumentActorHandle;
use crate::editor::spawn_editor_connection;
use crate::peer::spawn_peer_sync;

pub struct PeerConnectionInfo {
    port: Option<u16>,
    peer: Option<String>,
}
impl PeerConnectionInfo {
    #[must_use]
    pub fn new(port: Option<u16>, peer: Option<String>) -> Self {
        Self { port, peer }
    }
}

/// # Panics
///
/// Will panic if we fail to dial the peer, of if we fail to accept incoming connections.
pub async fn make_peer_connection(
    connection_info: PeerConnectionInfo,
    document_handle: DocumentActorHandle,
) {
    let result = if let Some(peer) = connection_info.peer {
        connect_with_peer(peer, document_handle).await
    } else {
        let port = connection_info.port.unwrap_or(4242);
        accept_peer_loop(port, document_handle).await
    };
    match result {
        Ok(()) => { /* successfully connected/started accept loop */ }
        Err(err) => {
            panic!("Failed to make connection: {err}");
        }
    }
}

/// # Panics
///
/// Will panic if we fail to listen on the socket, or if we fail to accept an incoming connection.
pub async fn make_editor_connection(socket_path: PathBuf, document_handle: DocumentActorHandle) {
    if Path::new(&socket_path).exists() {
        fs::remove_file(&socket_path).expect("Could not remove/re-initialize socket");
    }
    let result = accept_editor_loop(&socket_path, document_handle).await;
    match result {
        Ok(()) => {}
        Err(err) => {
            panic!("Failed to make editor connection: {err}");
        }
    }
}

async fn accept_editor_loop(
    socket_path: &Path,
    document_handle: DocumentActorHandle,
) -> Result<(), io::Error> {
    let listener = UnixListener::bind(socket_path)?;
    info!("Listening on UNIX socket: {}", socket_path.display());

    loop {
        let (stream, _addr) = listener.accept().await?;

        let id = document_handle.next_editor_id();
        info!("Editor connection established (#{})", id.0);

        spawn_editor_connection(stream, document_handle.clone(), id).await;
    }
}

async fn connect_with_peer(
    address: String,
    document_handle: DocumentActorHandle,
) -> Result<(), io::Error> {
    let stream = TcpStream::connect(address).await?;
    info!("Connected to Peer.");
    spawn_peer_sync(stream, &document_handle);
    Ok(())
}

async fn accept_peer_loop(
    port: u16,
    document_handle: DocumentActorHandle,
) -> Result<(), io::Error> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    if let Ok(ip) = local_ip() {
        info!("Listening on local TCP: {}:{}", ip, port);
    }

    timeout(Duration::from_secs(1), async {
        if let Some(ip) = public_ip::addr().await {
            info!("Listening on public TCP: {ip}:{port}");
        }
    })
    .await
    .unwrap_or_else(|_| {
        warn!("Getting public IP address timed out");
    });

    loop {
        let (stream, _addr) = listener.accept().await?;
        info!("Peer dialed us.");
        spawn_peer_sync(stream, &document_handle);
    }
}
