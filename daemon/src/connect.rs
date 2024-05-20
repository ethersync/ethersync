use local_ip_address::local_ip;
use std::io;
use tokio::net::{TcpListener, TcpStream};
use tracing::info;

use crate::daemon::DocumentActorHandle;
use crate::peer::spawn_peer_sync;

pub async fn make_peer_connection(
    port: Option<u16>,
    peer: Option<String>,
    document_handle: DocumentActorHandle,
) {
    let result = if let Some(peer) = peer {
        connect_with_peer(peer, document_handle.clone()).await
    } else {
        let port = port.unwrap_or(4242);
        accept_peer_loop(port, document_handle.clone()).await
    };
    match result {
        Ok(()) => { /* successfully connected/started accept loop */ }
        Err(err) => {
            panic!("Failed to make connection: {err}");
        }
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
