// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A peer is another daemon. This module is all about daemon to daemon communication.

use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::types::EphemeralMessage;
use anyhow::{Context, Result};
use automerge::sync::{Message as AutomergeSyncMessage, State as SyncState};
use iroh::SecretKey;
use postcard::{from_bytes, to_allocvec};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::mem;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::str::FromStr;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info, warn};

const ALPN: &[u8] = b"/ethersync/0";

#[derive(Deserialize, Serialize)]
/// The PeerMessage is used for peer to peer data exchange.
enum PeerMessage {
    /// The Sync message contains the changes to the CRDT
    Sync(Vec<u8>),
    /// The Ephemeral message currently is used for cursor messages, but can later be used for
    /// other things that should not be persisted.
    Ephemeral(EphemeralMessage),
}

pub struct ConnectionManager {
    message_tx: mpsc::Sender<EndpointMessage>,
    secret_address: String,
}

impl ConnectionManager {
    pub async fn new(document_handle: DocumentActorHandle, base_dir: &Path) -> Result<Self> {
        let (message_tx, message_rx) = mpsc::channel(1);

        let (endpoint, my_passphrase) = Self::build_endpoint(base_dir).await?;

        let secret_address = format!("{}#{}", endpoint.node_id(), my_passphrase);

        let mut actor = EndpointActor::new(endpoint, message_rx, document_handle, my_passphrase);

        tokio::spawn(async move { actor.run().await });

        Ok(Self {
            message_tx,
            secret_address,
        })
    }

    pub fn secret_address(&self) -> String {
        self.secret_address.clone()
    }

    pub async fn connect(&self, secret_address: String) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();

        self.message_tx
            .send(EndpointMessage::Connect {
                secret_address,
                response_tx,
            })
            .await
            .expect("EndpointActor task has been killed");

        response_rx.await??;

        Ok(())
    }

    async fn build_endpoint(base_dir: &Path) -> Result<(iroh::Endpoint, SecretKey)> {
        let (secret_key, my_passphrase) = Self::get_keypair(base_dir);

        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .discovery_n0()
            .bind()
            .await?;

        Ok((endpoint, my_passphrase))
    }

    fn get_keypair(base_dir: &Path) -> (SecretKey, SecretKey) {
        let keyfile = base_dir.join(".ethersync").join("key");
        if keyfile.exists() {
            let metadata =
                fs::metadata(&keyfile).expect("Expected to have access to metadata of the keyfile");

            let current_permissions = metadata.permissions().mode();
            let allowed_permissions = 0o100600;
            if current_permissions != allowed_permissions {
                panic!("For security reasons, please make sure to set the key file to user-readable only (set the permissions to 600).");
            }

            if metadata.len() != 64 {
                panic!("Your keyfile is not 64 bytes long. This is a sign that it was created by an Ethersync version older than 0.7.0, which is not compatible. Please remove .ethersync/key, and try again.");
            }

            debug!("Re-using existing keypair.");
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
            debug!("Generating new keypair.");
            let secret_key = SecretKey::generate(rand::rngs::OsRng);
            let passphrase = SecretKey::generate(rand::rngs::OsRng);

            let mut file = OpenOptions::new()
                .create_new(true)
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
}

enum EndpointMessage {
    // Instruct the endpoint to connect to a new peer.
    Connect {
        secret_address: String,
        response_tx: oneshot::Sender<Result<()>>,
    },
}

struct EndpointActor {
    endpoint: iroh::Endpoint,
    message_rx: mpsc::Receiver<EndpointMessage>,
    document_handle: DocumentActorHandle,
    my_passphrase: SecretKey,
}

impl EndpointActor {
    fn new(
        endpoint: iroh::Endpoint,
        message_rx: mpsc::Receiver<EndpointMessage>,
        document_handle: DocumentActorHandle,
        my_passphrase: SecretKey,
    ) -> Self {
        Self {
            endpoint,
            message_rx,
            document_handle,
            my_passphrase,
        }
    }

    async fn handle_message(&mut self, message: EndpointMessage) -> Result<()> {
        match message {
            EndpointMessage::Connect {
                secret_address,
                response_tx,
            } => {
                let parts: Vec<&str> = secret_address.split("#").collect();
                if parts.len() != 2 {
                    panic!("Peer string must have format <node_id>#<passphrase>");
                }

                dbg!(&secret_address);
                let public_key = iroh::PublicKey::from_str(parts[0])?;
                let peer_passphrase = iroh::SecretKey::from_str(parts[1])?;

                let node_addr: iroh::NodeAddr = public_key.into();
                dbg!(&node_addr);
                let conn = self.endpoint.connect(node_addr, ALPN).await?;

                info!(
                    "Connected to peer: {}",
                    conn.remote_node_id()
                        .expect("Connection should have a node ID")
                );

                response_tx.send(Ok(())).expect("Connect receiver dropped");

                let my_passphrase_clone = self.my_passphrase.clone();
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
        }
        Ok(())
    }

    async fn run(&mut self) {
        loop {
            tokio::select! {
                maybe_incoming = self.endpoint.accept() => {
                    match maybe_incoming {
                        Some(incoming) => {
                            match incoming.await {
                                Ok(conn) => {
                                    let node_id = conn
                                        .remote_node_id()
                                        .expect("Connection should have a node ID");

                                    info!("Peer connected: {}", &node_id);

                                    let my_passphrase_clone = self.my_passphrase.clone();
                                    let document_handle_clone = self.document_handle.clone();
                                    tokio::spawn(async move {
                                        Self::handle_peer(
                                            document_handle_clone,
                                            conn,
                                            my_passphrase_clone,
                                            None,
                                        )
                                        .await;

                                        info!("Peer disconnected: {node_id}",);
                                    });
                                }
                                Err(err) => {
                                    error!("Error while accepting peer connection: {err}");
                                }
                            }
                        }
                        None => {
                            // Endpoint was closed. Let's shut down.
                            break
                        }
                    }
                }
                maybe_message = self.message_rx.recv() => {
                    match maybe_message {
                        Some(message) => {
                            self.handle_message(message).await.expect("Failed to handle endpoint message");
                        }
                        None => {
                            // Our message channel was closed? Let's shut down.
                            break
                        }
                    }
                }
            }
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
            if let Err(e) = syncer.run().await {
                error!("Syncing failed with: {e}");
            }
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

        // TODO: Do we still this abort? The syncer should stop anyway once it cannot use its
        // to_peer_tx anymore.
        syncer_handle.abort_handle().abort();
    }

    /// Core low-level syncing protocol.
    async fn protocol_handler(
        conn: iroh::endpoint::Connection,
        from_peer_tx: mpsc::Sender<PeerMessage>,
        mut to_peer_rx: mpsc::Receiver<PeerMessage>,
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
                warn!("Peer provided incorrect passphrase.");
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
                            let bytes: Vec<u8> = to_allocvec(&message)?;
                            let byte_count = u32::try_from(bytes.len());
                            send
                                .write_all(&byte_count?.to_be_bytes())
                                .await?;
                            send
                                .write_all(&bytes)
                                .await?;
                            }
                        None => {
                            // TODO: What should we do?
                            error!("None on to_peer_rx");
                        }
                    }
                }
                _ = recv.read_exact(&mut message_len_buf) => {
                    let byte_count = u32::from_be_bytes(message_len_buf);
                    let mut bytes = vec![0; byte_count as usize];
                    recv.read_exact(&mut bytes).await?;

                    let message = from_bytes(&bytes)?;

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
    syncer_receiver: mpsc::Receiver<PeerMessage>,
    syncer_sender: mpsc::Sender<PeerMessage>,
}

impl SyncActor {
    fn new(
        document_handle: DocumentActorHandle,
        syncer_receiver: mpsc::Receiver<PeerMessage>,
        syncer_sender: mpsc::Sender<PeerMessage>,
    ) -> Self {
        Self {
            peer_state: SyncState::new(),
            document_handle,
            syncer_receiver,
            syncer_sender,
        }
    }

    async fn receive_peer_message(&mut self, message: PeerMessage) -> Result<()> {
        let (reponse_tx, response_rx) = oneshot::channel();
        match message {
            PeerMessage::Sync(message_buf) => {
                let message = AutomergeSyncMessage::decode(&message_buf)?;
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
            PeerMessage::Ephemeral(cursor) => {
                self.document_handle
                    .send_message(DocMessage::ReceiveEphemeral(cursor))
                    .await;
            }
        }
        Ok(())
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
                .send(PeerMessage::Sync(message.encode()))
                .await
                .context("Failed to send sync message on syncer_sender channel")?;
        }
        Ok(())
    }

    async fn run(mut self) -> Result<()> {
        let mut doc_changed_ping_rx = self.document_handle.subscribe_document_changes();
        let mut ephemeral_messages_rx = self.document_handle.subscribe_ephemeral_messages();

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
                            debug!("Doc changed ping channel lagged (this is probably fine).");
                        }
                    }
                }
                ephemeral_message = ephemeral_messages_rx.recv() => {
                    match ephemeral_message {
                        Ok(ephemeral_message) => {
                            self.syncer_sender.send(PeerMessage::Ephemeral(ephemeral_message))
                                .await
                                .context("Failed to send ephemeral message on syncer_sender channel")?;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            panic!("Ephemeral message channel has been closed");
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // We missed some cursor states, because of the limited
                            // capacity of the channel.
                            debug!("Ephemeral message channel lagged (this is unfortunate, but okay).");
                        }
                    }
                }
                Some(message) = self.syncer_receiver.recv() => {
                    self.receive_peer_message(message).await?;
                }
            }
        }
    }
}
