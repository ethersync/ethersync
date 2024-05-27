use futures::StreamExt;
use libp2p::StreamProtocol;
use libp2p::{multiaddr::Protocol, Multiaddr};
use libp2p_stream as stream;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tokio::net::UnixListener;
use tokio::time::Duration;
use tracing::info;

use crate::daemon::DocumentActorHandle;
use crate::editor::spawn_editor_connection;
use crate::peer::{self};

const ETHERSYNC_PROTOCOL: StreamProtocol = StreamProtocol::new("/ethersync");

pub struct PeerConnectionInfo {
    peer: Option<String>,
}
impl PeerConnectionInfo {
    #[must_use]
    pub fn new(peer: Option<String>) -> Self {
        Self { peer }
    }
}

/// # Panics
///
/// Will panic if we fail to dial the peer, of if we fail to accept incoming connections.
pub async fn make_peer_connection(
    connection_info: PeerConnectionInfo,
    document_handle: DocumentActorHandle,
) -> anyhow::Result<()> {
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| stream::Behaviour::new())?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(10)))
        .build();

    swarm.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse()?)?;

    let mut incoming_streams = swarm
        .behaviour()
        .new_control()
        .accept(ETHERSYNC_PROTOCOL)
        .unwrap();

    // In this demo application, the dialing peer initiates the protocol.
    if let Some(address) = connection_info.peer {
        let multiaddr = address
            .parse::<Multiaddr>()
            .expect("Failed to parse argument as `Multiaddr`");

        let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last() else {
            anyhow::bail!("Provided address does not end in `/p2p`");
        };

        tokio::spawn(async move {
            while let Some((_peer, stream)) = incoming_streams.next().await {
                // No need to do anything.
            }
        });

        swarm.dial(multiaddr)?;

        let mut control = swarm.behaviour().new_control();

        tokio::spawn(async move {
            let stream = match control.open_stream(peer_id, ETHERSYNC_PROTOCOL).await {
                Ok(stream) => stream,
                Err(error @ stream::OpenStreamError::UnsupportedProtocol(_)) => {
                    tracing::info!(%peer_id, %error);
                    panic!("Unsupported protocol");
                }
                Err(error) => {
                    // Other errors may be temporary.
                    // In production, something like an exponential backoff / circuit-breaker may be more appropriate.
                    tracing::debug!(%peer_id, %error);
                    panic!("Maybe an temporary error? TODO");
                }
            };

            peer::spawn_peer_sync(stream, &document_handle);
        });
    } else {
        // Deal with incoming streams.
        // Spawning a dedicated task is just one way of doing this.
        // libp2p doesn't care how you handle incoming streams but you _must_ handle them somehow.
        // To mitigate DoS attacks, libp2p will internally drop incoming streams if your application cannot keep up processing them.
        tokio::spawn(async move {
            // This loop handles incoming streams _sequentially_ but that doesn't have to be the case.
            // You can also spawn a dedicated task per stream if you want to.
            // Be aware that this breaks backpressure though as spawning new tasks is equivalent to an unbounded buffer.
            // Each task needs memory meaning an aggressive remote peer may force you OOM this way.

            while let Some((_peer, stream)) = incoming_streams.next().await {
                peer::spawn_peer_sync(stream, &document_handle);
            }
        });
    }

    // Poll the swarm to make progress.
    loop {
        let event = swarm.next().await.expect("never terminates");

        match event {
            libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                let listen_address = address.with_p2p(*swarm.local_peer_id()).unwrap();
                tracing::info!(%listen_address);
            }
            event => tracing::trace!(?event),
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
        info!("Editor connection established");

        // TODO: we need to get rid of this await to accept multiple editors.
        spawn_editor_connection(stream, document_handle.clone()).await;
    }
}
