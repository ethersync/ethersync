use crate::daemon::{DocumentActorHandle, SyncActorHandle, TCPActorHandle};
use local_ip_address::local_ip;
use std::io;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tracing::info;

pub async fn make_connection(
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
    let (my_send, my_recv) = oneshot::channel();
    let tcp_handle = TCPActorHandle::start_sync(stream, my_recv);
    let sync_handle = SyncActorHandle::new(document_handle, tcp_handle);
    let _ = my_send.send(sync_handle);
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
        let (my_send, my_recv) = oneshot::channel();
        let tcp_handle = TCPActorHandle::start_sync(stream, my_recv);
        let sync_handle = SyncActorHandle::new(document_handle.clone(), tcp_handle);
        let _ = my_send.send(sync_handle);
    }
}
