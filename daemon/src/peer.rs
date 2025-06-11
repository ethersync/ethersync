// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A peer is another daemon. This module is all about daemon to daemon communication.

use crate::daemon::{DocMessage, DocumentActorHandle};
use anyhow::{Context, Result};
use automerge::sync::{Message as AutomergeSyncMessage, State as SyncState};
use ini::Ini;
use iroh::SecretKey;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::mem;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info, warn};

const ALPN: &[u8] = b"/ethersync/0";

/// Responsible for offering peer-to-peer connectivity to the outside world. Uses libp2p.
/// For every new connection, spawns and runs a `SyncActor`.
#[derive(Clone)]
pub struct PeerConnectionInfo {
    pub peer: Option<String>,
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
                peer: general_section.get("peer").map(|p| p.to_string()),
            });
        } else {
            info!("No config file found, please provide everything through CLI options");
            None
        }
    }

    pub fn takes_precedence_over(self, other: Self) -> Self {
        Self {
            peer: self.peer.or(other.peer),
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

    // Returns the connection address.
    pub async fn run(self) -> Result<String> {
        let (secret_key, my_passphrase) = self.get_keypair();

        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .discovery_n0()
            .bind()
            .await?;

        let address = format!("{}#{}", endpoint.node_id(), my_passphrase);

        info!(
            "Others can connect with `ethersync join` providing the following ticket\n\n\t{}\n",
            address
        );

        if let Some(ref peer) = self.connection_info.peer {
            let parts: Vec<&str> = peer.split("#").collect();
            if parts.len() != 2 {
                panic!("Peer string must have format <node_id>#<passphrase>");
            }

            let public_key = iroh::PublicKey::from_str(&parts[0])?;
            let peer_passphrase = iroh::SecretKey::from_str(&parts[1])?;

            let node_addr: iroh::NodeAddr = public_key.into();
            let conn = endpoint.connect(node_addr, ALPN).await?;

            info!(
                "Connected to peer: {}",
                conn.remote_node_id()
                    .expect("Connection should have a node ID")
            );

            let my_passphrase_clone = my_passphrase.clone();
            let document_handle_clone = self.document_handle.clone();
            tokio::spawn(async move {
                Self::handle_peer(
                    document_handle_clone,
                    conn,
                    my_passphrase_clone,
                    Some(peer_passphrase),
                )
                .await;
            });
        }

        tokio::spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                match incoming.await {
                    Ok(conn) => {
                        info!(
                            "Peer connected: {}",
                            conn.remote_node_id()
                                .expect("Connection should have a node ID")
                        );

                        let my_passphrase_clone = my_passphrase.clone();
                        let document_handle_clone = self.document_handle.clone();
                        tokio::spawn(async move {
                            Self::handle_peer(
                                document_handle_clone,
                                conn,
                                my_passphrase_clone,
                                None,
                            )
                            .await;
                        });
                    }
                    Err(err) => {
                        panic!("Error while accepting peer connection: {err}");
                    }
                }
            }
        });

        Ok(address)
    }

    /// Returns an existing secret key + passphrase, or generates new ones.
    fn get_keypair(&self) -> (SecretKey, SecretKey) {
        let keyfile = self.base_dir.join(".ethersync").join("key");
        if keyfile.exists() {
            let current_permissions = fs::metadata(&keyfile)
                .expect("Expected to have access to metadata of the keyfile")
                .permissions()
                .mode();
            let allowed_permissions = 0o100600;
            if current_permissions != allowed_permissions {
                panic!("For security reasons, please make sure to set the key file to user-readable only (set the permissions to 600).");
            }
            info!("Re-using existing keypair");
            let mut file = File::open(keyfile).expect("Failed to open key file");

            let mut secret_key = [0; 32];
            file.read_exact(&mut secret_key)
                .expect("Failed to read from key file");

            let mut passphrase = [0; 32];
            file.read_exact(&mut passphrase)
                .expect("Failed to read from key file");

            (
                SecretKey::from_bytes(&secret_key),
                SecretKey::from_bytes(&passphrase),
            )
        } else {
            info!("Generating new keypair");
            let secret_key = SecretKey::generate(rand::rngs::OsRng);
            let passphrase = SecretKey::generate(rand::rngs::OsRng);

            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .mode(0o600)
                .open(keyfile)
                .expect("Should have been able to create key file that did not exist before");

            file.write_all(&secret_key.to_bytes())
                .expect("Failed to write to key file");
            file.write_all(&passphrase.to_bytes())
                .expect("Failed to write to key file");

            (secret_key, passphrase)
        }
    }

    async fn handle_peer(
        document_handle: DocumentActorHandle,
        conn: iroh::endpoint::Connection,
        my_passphrase: SecretKey,
        peer_passphrase: Option<SecretKey>,
    ) {
        let (to_peer_tx, to_peer_rx) = mpsc::channel(16);
        let (from_peer_tx, from_peer_rx) = mpsc::channel(16);

        let syncer = SyncActor::new(document_handle, from_peer_rx, to_peer_tx);

        let syncer_handle = tokio::spawn(async move {
            // The syncer can fail when the protocol_handler below has
            // stopped. But in that case, both components will stop, so we can
            // ignore the error.
            let _ = syncer.run().await;
        });

        // This is a function that either runs forever, or errors.
        // But errors just mean that the connection was closed/interrupted, so we ignore them.
        let _ = Self::protocol_handler(
            conn,
            from_peer_tx,
            to_peer_rx,
            my_passphrase,
            peer_passphrase,
        )
        .await;

        info!("Peer disconnected");
        // TODO: Do we still this abort? The syncer should stop anyway once it cannot use its
        // to_peer_tx anymore.
        syncer_handle.abort_handle().abort();
    }

    /// Core low-level syncing protocol.
    async fn protocol_handler(
        conn: iroh::endpoint::Connection,
        from_peer_tx: mpsc::Sender<AutomergeSyncMessage>,
        mut to_peer_rx: mpsc::Receiver<AutomergeSyncMessage>,
        my_passphrase: SecretKey,
        peer_passphrase: Option<SecretKey>,
    ) -> Result<()> {
        let (mut send, mut recv) = if let Some(peer_passphrase) = peer_passphrase {
            let (mut send, recv) = conn.open_bi().await?;

            send.write_all(&peer_passphrase.to_bytes()).await?;

            (send, recv)
        } else {
            let (send, mut recv) = conn.accept_bi().await?;

            let mut received_passphrase = [0; 32];
            recv.read_exact(&mut received_passphrase).await?;

            // Guard against timing attacks.
            if !constant_time_eq::constant_time_eq(&received_passphrase, &my_passphrase.to_bytes())
            {
                warn!("Peer provided incorrect passphrase");
                return Ok(());
            }

            (send, recv)
        };

        loop {
            let mut message_len_buf = [0; 4];

            tokio::select! {
                message_maybe = to_peer_rx.recv() => {
                    match message_maybe {
                        Some(message) => {
                            let message = message.encode();
                            let message_len = u32::try_from(message.len());
                            send
                                .write_all(&message_len?.to_be_bytes())
                                .await?;
                            send
                                .write_all(&message)
                                .await?;
                            }
                        None => {
                            // TODO: What should we do?
                            error!("None on to_peer_rx");
                        }
                    }
                }
                _ = recv.read_exact(&mut message_len_buf) => {
                    let message_len = u32::from_be_bytes(message_len_buf);
                    let mut message_buf = vec![0; message_len as usize];
                    recv.read_exact(&mut message_buf).await?;

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

    async fn generate_sync_message(&mut self) -> Result<()> {
        let (reponse_tx, response_rx) = oneshot::channel();
        self.document_handle
            .send_message(DocMessage::GenerateSyncMessage {
                state: mem::take(&mut self.peer_state),
                response_tx: reponse_tx,
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
