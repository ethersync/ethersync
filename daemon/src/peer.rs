//! A peer is another daemon. This module is all about daemon to daemon communication.

use crate::daemon::{DocMessage, DocumentActorHandle};
use automerge::sync::{Message as AutomergeSyncMessage, State as SyncState};
use futures::StreamExt;
use futures::{AsyncReadExt, AsyncWriteExt};
use libp2p::Stream;
use libp2p::StreamProtocol;
use libp2p::{multiaddr::Protocol, Multiaddr};
use libp2p_identity::Keypair;
use libp2p_stream as stream;
use std::mem;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::Duration;
//use tokio_util::sync::CancellationToken;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info};

const ETHERSYNC_PROTOCOL: StreamProtocol = StreamProtocol::new("/ethersync");

#[derive(Clone)]
pub enum PeerConnectionInfo {
    /// Port
    Listen(u16),
    /// Peer, Port
    Dial(String, u16),
}

impl PeerConnectionInfo {
    #[must_use]
    pub const fn is_host(&self) -> bool {
        matches!(self, Self::Listen(_))
    }

    #[must_use]
    fn port(&self) -> &u16 {
        match self {
            Self::Listen(p) => p,
            Self::Dial(_, p) => p,
        }
    }
}

#[derive(Clone)]
pub struct P2PActor {
    connection_info: PeerConnectionInfo,
    document_handle: DocumentActorHandle,
    base_dir: PathBuf,
}

impl P2PActor {
    pub fn new(
        connection_info: PeerConnectionInfo,
        document_handle: DocumentActorHandle,
        base_dir: &Path,
    ) -> Self {
        Self {
            connection_info,
            document_handle,
            base_dir: base_dir.to_path_buf(),
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let keypair = self.get_keypair();
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_quic()
            .with_behaviour(|_| stream::Behaviour::new())?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(10)))
            .build();

        let listen_addr =
            format!("/ip4/127.0.0.1/udp/{}/quic-v1", self.connection_info.port()).parse()?;

        swarm.listen_on(listen_addr)?;

        let mut incoming_streams = swarm
            .behaviour()
            .new_control()
            .accept(ETHERSYNC_PROTOCOL)
            .unwrap();

        if let PeerConnectionInfo::Dial(ref address, _) = self.connection_info {
            let multiaddr = address
                .parse::<Multiaddr>()
                .expect("Failed to parse argument as `Multiaddr`");

            let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last() else {
                anyhow::bail!("Provided address does not end in `/p2p`");
            };

            tokio::spawn(async move {
                while let Some((peer, _stream)) = incoming_streams.next().await {
                    info!("Peer connected: {}", peer);
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

                info!("Connected to peer {}", peer_id);

                self.spawn_peer_sync(stream).await;
            });
        } else {
            tokio::spawn(async move {
                while let Some((peer, stream)) = incoming_streams.next().await {
                    info!("Peer connected: {}", peer);
                    self.spawn_peer_sync(stream).await;
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

    fn get_keypair(&self) -> Keypair {
        let keyfile = self.base_dir.join(".ethersync").join("key");
        if keyfile.exists() {
            info!("Re-using existing keypair");
            let bytes = std::fs::read(keyfile).expect("Failed to read key file");
            Keypair::from_protobuf_encoding(&bytes).expect("Failed to deserialize key file")
        } else {
            info!("Generating new keypair");
            let keypair = Keypair::generate_ed25519();
            let bytes = keypair
                .to_protobuf_encoding()
                .expect("Failed to serialize keypair");
            std::fs::write(keyfile, bytes).expect("Failed to write key file");
            keypair
        }
    }

    async fn spawn_peer_sync(&self, stream: Stream) {
        let (we_to_peer_tx, we_to_peer_rx) = mpsc::channel(16);
        let (peer_to_us_tx, peer_to_us_rx) = mpsc::channel(16);

        let syncer = SyncActor::new(self.document_handle.clone(), we_to_peer_rx, peer_to_us_tx);
        tokio::spawn(async move {
            syncer.run().await;

            match Self::protocol_handler(stream, peer_to_us_rx, we_to_peer_tx).await {
                Ok(()) => {
                    info!("Sync successful! Do we ever get here?");
                }
                Err(e) => {
                    error!("Sync failed or interrupted: {e}");
                }
            }
        });
    }

    async fn protocol_handler(
        mut stream: Stream,
        mut peer_to_us_rx: mpsc::Receiver<AutomergeSyncMessage>,
        we_to_peer_tx: mpsc::Sender<AutomergeSyncMessage>,
    ) -> anyhow::Result<()> {
        loop {
            let mut message_len_buf = [0; 4];

            tokio::select! {
                message_maybe = peer_to_us_rx.recv() => {
                    match message_maybe {
                        Some(message) => {
                            let message = message.encode();
                            let message_len = message.len() as u32;
                            stream
                                .write_all(&message_len.to_be_bytes())
                                .await?;
                            stream
                                .write_all(&message)
                                .await?;
                            }
                        None => {
                            // TODO: ?
                        }
                    }
                }
                _ = stream.read_exact(&mut message_len_buf) => {
                    let message_len = u32::from_be_bytes(message_len_buf);
                    let mut message_buf = vec![0; message_len as usize];
                    stream.read_exact(&mut message_buf).await?;

                    let message =
                        AutomergeSyncMessage::decode(&message_buf)?;
                    we_to_peer_tx.send(message).await?;
                }
            }
        }
    }
}

struct SyncActor {
    peer_state: SyncState,
    document_handle: DocumentActorHandle,
    syncer_receiver: mpsc::Receiver<AutomergeSyncMessage>,
    syncer_sender: mpsc::Sender<AutomergeSyncMessage>,
}

impl SyncActor {
    fn new(
        document_handle: DocumentActorHandle,
        syncer_receiver: mpsc::Receiver<AutomergeSyncMessage>,
        syncer_sender: mpsc::Sender<AutomergeSyncMessage>,
    ) -> Self {
        Self {
            peer_state: SyncState::new(),
            document_handle,
            syncer_receiver,
            syncer_sender,
        }
    }

    async fn receive_sync_message(&mut self, message: AutomergeSyncMessage) {
        let (reponse_tx, response_rx) = oneshot::channel();
        self.document_handle
            .send_message(DocMessage::ReceiveSyncMessage {
                message,
                state: mem::take(&mut self.peer_state),
                response_tx: reponse_tx,
            })
            .await;
        self.peer_state = response_rx
            .await
            .expect("Couldn't read response from Document channel");
    }

    async fn generate_sync_message(&mut self) {
        let (reponse_tx, response_rx) = oneshot::channel();
        self.document_handle
            .send_message(DocMessage::GenerateSyncMessage {
                state: mem::take(&mut self.peer_state),
                response_tx: reponse_tx,
            })
            .await;
        let (ps, message) = response_rx
            .await
            .expect("Could not read response from Document channel");
        self.peer_state = ps;
        if let Some(message) = message {
            self.syncer_sender
                .send(message)
                .await
                .expect("Failed to send on syncer_sender channel");
        }
    }

    async fn run(mut self) {
        let mut doc_changed_ping_rx = self.document_handle.subscribe_document_changes();

        // Kick off initial synchronization with peer.
        self.generate_sync_message().await;

        loop {
            tokio::select! {
                // As doc_changed_ping_rx is a broadcast channel our understanding is,
                // that this breaks a potential cyclic deadlock between SyncerActor
                // and TCPActor (e.g. when TCPWriteActor.send blocks).
                doc_ping = doc_changed_ping_rx.recv() => {
                    match doc_ping {
                        Ok(()) => { self.generate_sync_message().await; }
                        Err(broadcast::error::RecvError::Closed) => {
                            panic!("Doc changed channel has been closed");
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // This is fine, the messages in this channel are just pings.
                            // It's fine if we miss some.
                            debug!("Doc changed ping channel lagged (this is probably fine)");
                        }
                    }
                }
                Some(message) = self.syncer_receiver.recv() => {
                    self.receive_sync_message(message).await;
                }
            }
        }
    }
}
