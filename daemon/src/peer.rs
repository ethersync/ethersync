//! A peer is another daemon. This module is all about daemon to daemon communication.

use crate::daemon::{DocMessage, DocumentActorHandle};
use anyhow::{Context, Result};
use automerge::sync::{Message as AutomergeSyncMessage, State as SyncState};
use futures::StreamExt;
use futures::{AsyncReadExt, AsyncWriteExt};
use ini::Ini;
use libp2p::core::transport::upgrade::Version;
use libp2p::core::ConnectedPoint;
use libp2p::multiaddr::{Multiaddr, Protocol};
use libp2p::Stream;
use libp2p::StreamProtocol;
use libp2p::Transport;
use libp2p::{identity::Keypair, noise, pnet, tcp, yamux};
use libp2p_stream as stream;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
#[cfg(unix)]
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::mem;
use std::net::Ipv4Addr;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

const ETHERSYNC_PROTOCOL: StreamProtocol = StreamProtocol::new("/ethersync");
// Used for "easy try out" purposes that are not security critical.
const DEFAULT_PASSPHRASE: &str = "default-passphrase";

/// Responsible for offering peer-to-peer connectivity to the outside world. Uses libp2p.
/// For every new connection, spawns and runs a `SyncActor`.
#[derive(Clone)]
pub struct PeerConnectionInfo {
    pub port: Option<u16>,
    pub peer: Option<String>,
    pub passphrase: Option<String>,
}

impl PeerConnectionInfo {
    // TODO: It feels like this function would fit better into main.rs.
    // Should the whole type live there?
    pub fn from_config_file(config_file: &Path) -> Option<Self> {
        if config_file.exists() {
            let conf = Ini::load_from_file(config_file)
                .expect("Could not access config file, even though it exists");
            let general_section = conf.general_section();
            return Some(Self {
                port: general_section.get("port").map(|p| {
                    p.parse()
                        .expect("Failed to parse port in config file as an integer")
                }),
                peer: general_section.get("peer").map(|p| p.to_string()),
                passphrase: general_section.get("secret").map(|p| p.to_string()),
            });
        } else {
            info!("No config file found, please provide everything through CLI options");
            None
        }
    }

    pub fn takes_precedence_over(self, other: Self) -> Self {
        Self {
            port: self.port.or(other.port),
            peer: self.peer.or(other.peer),
            passphrase: self.passphrase.or(other.passphrase),
        }
    }

    #[must_use]
    pub const fn is_host(&self) -> bool {
        self.peer.is_none()
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

    pub async fn run(mut self) -> Result<()> {
        let keypair = self.get_keypair();
        let passphrase = self
            .connection_info
            .passphrase
            .clone()
            .unwrap_or(DEFAULT_PASSPHRASE.to_string());
        let is_default_passphrase = passphrase == DEFAULT_PASSPHRASE;
        if is_default_passphrase {
            warn!("\n\n\tSECURITY WARNING: Running without a secret is only recommended when trying out this software locally.\n\tYou can put secret = <secret> in .ethersync/config.\n");
        }
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_other_transport(|keypair| {
                self.connection_info.passphrase = Some(passphrase.clone());

                let psk = pnet::PreSharedKey::new(Self::passphrase_to_bytes(&passphrase));

                let transport = tcp::tokio::Transport::new(tcp::Config::new())
                    .and_then(move |socket, _| pnet::PnetConfig::new(psk).handshake(socket));
                let auth = noise::Config::new(keypair)?;
                let mux = yamux::Config::default();

                let tcp_transport = transport
                    .upgrade(Version::V1Lazy)
                    .authenticate(auth)
                    .multiplex(mux);

                Ok(tcp_transport)
            })?
            .with_behaviour(|_| stream::Behaviour::new())?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(10)))
            .build();

        // When the port is 0, libp2p randomly assigns a port.
        let listen_addr = format!(
            "/ip4/0.0.0.0/tcp/{}",
            self.connection_info.port.unwrap_or(0)
        )
        .parse()?;

        swarm.listen_on(listen_addr)?;

        let mut incoming_streams = swarm
            .behaviour()
            .new_control()
            .accept(ETHERSYNC_PROTOCOL)
            .unwrap();

        if let Some(ref address) = self.connection_info.peer {
            let multiaddr = address
                .parse::<Multiaddr>()
                .expect("Failed to parse argument as `Multiaddr`");

            swarm.dial(multiaddr)?;
        }

        // Poll the swarm to make progress.
        loop {
            let event = swarm.next().await.expect("never terminates");

            match event {
                libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                    let listen_address = address.with_p2p(*swarm.local_peer_id()).unwrap();
                    // Filter for not useful address.
                    let is_localhost = listen_address
                        .iter()
                        .any(|component| component == Protocol::Ip4(Ipv4Addr::new(127, 0, 0, 1)));
                    if !is_localhost {
                        let secret_parameter = if is_default_passphrase {
                            ""
                        } else {
                            " (They need put secret = <your-secret> in the .ethersync/config file.)"
                        };
                        info!(
                            "Others can connect with:\n\n\tethersync daemon --peer {}{}\n",
                            listen_address, secret_parameter
                        );
                    }
                }
                libp2p::swarm::SwarmEvent::ConnectionEstablished {
                    peer_id,
                    endpoint: ConnectedPoint::Dialer { .. },
                    ..
                } => {
                    let mut control = swarm.behaviour().new_control();
                    let stream = control
                        .open_stream(peer_id, ETHERSYNC_PROTOCOL)
                        .await
                        .context("Failed to open stream")?;

                    info!("Connected to peer {}", peer_id);

                    self.spawn_peer_sync(stream);
                }
                libp2p::swarm::SwarmEvent::ConnectionEstablished {
                    endpoint: ConnectedPoint::Listener { .. },
                    ..
                } => {
                    if let Some((peer, stream)) = incoming_streams.next().await {
                        info!("Peer connected: {}", peer);
                        self.spawn_peer_sync(stream);
                    }
                }
                libp2p::swarm::SwarmEvent::OutgoingConnectionError { error, .. } => {
                    error!("Failed to connect, the peer multiaddress or secret you provided might be wrong?");
                    debug!("{:?}", error);
                }
                libp2p::swarm::SwarmEvent::IncomingConnectionError { error, .. } => {
                    error!("Someone tried to connect to you, but failed. The secret they provided might be wrong?");
                    debug!("{:?}", error);
                }
                event => debug!(?event),
            }
        }
    }

    /// Returns an existing keypair, or generates a new one.
    /// On Unix, we check for permissions. On Windows, do nothing special.
    fn get_keypair(&self) -> Keypair {
        let keyfile = self.base_dir.join(".ethersync").join("key");
        if keyfile.exists() {
            // Check file perms only on Unix
            #[cfg(unix)]
            {
                let current_permissions = fs::metadata(&keyfile)
                    .expect("Expected to have access to metadata of the keyfile")
                    .permissions()
                    .mode();
                let allowed_permissions = 0o100600;
                if current_permissions != allowed_permissions {
                    panic!("For security reasons, please make sure to set the key file to user-readable only (set the permissions to 600).");
                }
            }
            // On Windows, do nothing
            info!("Re-using existing keypair");
            let bytes = std::fs::read(keyfile).expect("Failed to read key file");
            Keypair::from_protobuf_encoding(&bytes).expect("Failed to deserialize key file")
        } else {
            info!("Generating new keypair");
            // TODO: Is this the best algorithm?
            let keypair = Keypair::generate_ed25519();
            let bytes = keypair
                .to_protobuf_encoding()
                .expect("Failed to serialize keypair");

            // On Unix, set mode. On Windows, skip.
            #[cfg(unix)]
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(keyfile)
                .expect("Should have been able to create key file that did not exist before");

            #[cfg(windows)]
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&keyfile)
                .expect("Could not create key file on Windows");

            file.write_all(&bytes).expect("Failed to write to key file");

            keypair
        }
    }

    // This "stretches" the passphrase to fill the 32 bytes required by the pnet crate.
    fn passphrase_to_bytes(passphrase: &str) -> [u8; 32] {
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(
            passphrase.as_bytes(),
            b"ethersync", // TODO: Is it bad to re-use the salt here?
            1000,         // TODO: How often should we iterate?
            &mut key,
        );
        key
    }

    fn spawn_peer_sync(&self, stream: Stream) {
        let (to_peer_tx, to_peer_rx) = mpsc::channel(16);
        let (from_peer_tx, from_peer_rx) = mpsc::channel(16);

        let syncer = SyncActor::new(self.document_handle.clone(), from_peer_rx, to_peer_tx);
        tokio::spawn(async move {
            let syncer_handle = tokio::spawn(async move {
                // The syncer can fail when the protocol_handler below has
                // stopped. But in that case, both components will stop, so we can
                // ignore the error.
                let _ = syncer.run().await;
            });

            // This is a function that either runs forever, or errors.
            // But errors just mean that the connection was closed/interrupted, so we ignore them.
            let _ = Self::protocol_handler(stream, from_peer_tx, to_peer_rx).await;

            info!("Peer disconnected");
            // TODO: Do we still this abort? The syncer should stop anyway once it cannot use its
            // to_peer_tx anymore.
            syncer_handle.abort_handle().abort();
        });
    }

    /// Core low-level syncing protocol.
    async fn protocol_handler(
        mut stream: Stream,
        from_peer_tx: mpsc::Sender<AutomergeSyncMessage>,
        mut to_peer_rx: mpsc::Receiver<AutomergeSyncMessage>,
    ) -> Result<()> {
        loop {
            let mut message_len_buf = [0; 4];

            tokio::select! {
                message_maybe = to_peer_rx.recv() => {
                    match message_maybe {
                        Some(message) => {
                            let message = message.encode();
                            let message_len = u32::try_from(message.len());
                            stream
                                .write_all(&message_len?.to_be_bytes())
                                .await?;
                            stream
                                .write_all(&message)
                                .await?;
                            }
                        None => {
                            // TODO: What should we do?
                            error!("None on to_peer_rx");
                        }
                    }
                }
                _ = stream.read_exact(&mut message_len_buf) => {
                    let message_len = u32::from_be_bytes(message_len_buf);
                    let mut message_buf = vec![0; message_len as usize];
                    stream.read_exact(&mut message_buf).await?;

                    let message =
                        AutomergeSyncMessage::decode(&message_buf)?;
                    from_peer_tx.send(message).await?;
                }
            }
        }
    }
}

/// Transport-agnostic logic of how to sync with another peer.
/// Receives Automerge sync messages on one channel, and sends some out on another.
/// Maintains the sync state, and communicates with the document.
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
        let (response_tx, response_rx) = oneshot::channel();
        self.document_handle
            .send_message(DocMessage::ReceiveSyncMessage {
                message,
                state: mem::take(&mut self.peer_state),
                response_tx: response_tx,
            })
            .await;
        self.peer_state = response_rx
            .await
            .expect("Couldn't read response from Document channel");
    }

    async fn generate_sync_message(&mut self) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.document_handle
            .send_message(DocMessage::GenerateSyncMessage {
                state: mem::take(&mut self.peer_state),
                response_tx,
            })
            .await;
        let (ps, message) = response_rx
            .await
            .context("Could not read response from Document channel")?;
        self.peer_state = ps;
        if let Some(message) = message {
            self.syncer_sender
                .send(message)
                .await
                .context("Failed to send on syncer_sender channel")?;
        }
        Ok(())
    }

    async fn run(mut self) -> Result<()> {
        let mut doc_changed_ping_rx = self.document_handle.subscribe_document_changes();

        // Kick off initial synchronization with peer.
        self.generate_sync_message().await?;

        loop {
            tokio::select! {
                // As doc_changed_ping_rx is a broadcast channel our understanding is,
                // that this breaks a potential cyclic deadlock between SyncerActor
                // and TCPActor (e.g. when TCPWriteActor.send blocks).
                doc_ping = doc_changed_ping_rx.recv() => {
                    match doc_ping {
                        Ok(()) => { self.generate_sync_message().await?; }
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
