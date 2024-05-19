/// A peer is another daemon. This module is all about daemon to daemon communication.
use tokio::{net::TcpStream, sync::oneshot};

use crate::daemon::{DocumentActorHandle, SyncActorHandle, TCPActorHandle};

pub fn spawn_peer_sync(stream: TcpStream, document_handle: DocumentActorHandle) {
    let (my_send, my_recv) = oneshot::channel();
    let tcp_handle = TCPActorHandle::start_sync(stream, my_recv);
    let sync_handle = SyncActorHandle::new(document_handle.clone(), tcp_handle);
    let _ = my_send.send(sync_handle);
}
