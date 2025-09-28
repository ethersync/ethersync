// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! This module provides a [`ConnectionManager`], which can be used to connect to other daemons.

use self::sync::{Connection, SyncActor};
use crate::daemon::DocumentActorHandle;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use ethersync_shared::keypair::Keypair;
use ethersync_shared::messages::PeerMessage;
use ethersync_shared::secret_address::SecretAddress;
use iroh::endpoint::{RecvStream, SendStream};
use iroh::SecretKey;
use postcard::{from_bytes, to_allocvec};
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;
use tracing::{debug, info, warn};

mod sync;

const ALPN: &[u8] = b"/ethersync/0";

enum PeerAuth {
    MyPassphrase(SecretKey),
    YourPassphrase(SecretKey),
}

pub struct ConnectionManager {
    message_tx: mpsc::Sender<EndpointMessage>,
    secret_address: SecretAddress,
}

impl ConnectionManager {
    pub async fn new(document_handle: DocumentActorHandle, keypair: Keypair) -> Result<Self> {
        let (message_tx, message_rx) = mpsc::channel(1);

        let (endpoint, my_passphrase) = Self::build_endpoint(keypair).await?;

        let secret_address = SecretAddress { node_addr: endpoint.node_id().into(), passphrase: my_passphrase.clone() };

        let mut actor = EndpointActor::new(
            endpoint,
            message_rx,
            message_tx.clone(),
            document_handle,
            my_passphrase,
        );

        tokio::spawn(async move { actor.run().await });

        Ok(Self {
            message_tx,
            secret_address,
        })
    }

    #[must_use]
    pub fn secret_address(&self) -> &SecretAddress {
        &self.secret_address
    }

    pub async fn connect(&self, secret_address: String) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();

        self.message_tx
            .send(EndpointMessage::Connect {
                secret_address: SecretAddress::from_str(&secret_address)?,
                response_tx: Some(response_tx),
                previous_attempts: 0,
            })
            .await
            .expect("EndpointActor task has been killed");

        response_rx.await??;

        Ok(())
    }

    async fn build_endpoint(keypair: Keypair) -> Result<(iroh::Endpoint, SecretKey)> {
        let endpoint = iroh::Endpoint::builder()
            .secret_key(keypair.secret_key)
            .alpns(vec![ALPN.to_vec()])
            .discovery_n0()
            .bind()
            .await?;

        Ok((endpoint, keypair.passphrase))
    }
}

enum EndpointMessage {
    // Instruct the endpoint to connect to a new peer.
    Connect {
        // All information we need to connect to another peer.
        secret_address: SecretAddress,
        // On connection success, this channel will be pinged.
        // Used for the initial connection, where we want to fail if connecting fails.
        response_tx: Option<oneshot::Sender<Result<()>>>,
        // How many times have we already attempted to connect?
        previous_attempts: usize,
    },
}

// Owns the Iroh endpoint, accepts incoming connections, and can be instructed to connect to
// another daemon.
struct EndpointActor {
    endpoint: iroh::Endpoint,
    message_rx: mpsc::Receiver<EndpointMessage>,
    message_tx: mpsc::Sender<EndpointMessage>,
    document_handle: DocumentActorHandle,
    my_passphrase: SecretKey,
}

impl EndpointActor {
    fn new(
        endpoint: iroh::Endpoint,
        message_rx: mpsc::Receiver<EndpointMessage>,
        message_tx: mpsc::Sender<EndpointMessage>,
        document_handle: DocumentActorHandle,
        my_passphrase: SecretKey,
    ) -> Self {
        Self {
            endpoint,
            message_rx,
            message_tx,
            document_handle,
            my_passphrase,
        }
    }

    async fn handle_message(&self, message: EndpointMessage) -> Result<()> {
        match message {
            EndpointMessage::Connect {
                secret_address,
                response_tx,
                previous_attempts,
            } => {
                let node_addr = secret_address.node_addr.clone();
                let connect_result = self.endpoint.connect(node_addr, ALPN).await;
                let conn = match connect_result {
                    Ok(conn) => conn,
                    Err(err) => {
                        if let Some(response_tx) = response_tx {
                            response_tx
                                .send(Err(err))
                                .expect("Connect receiver dropped");
                        }
                        Self::reconnect(self.message_tx.clone(), secret_address, previous_attempts)
                            .await
                            .expect("Failed to initiate reconnection");
                        // Not really Ok, but Ok enough.
                        return Ok(());
                    }
                };

                info!(
                    "Connected to peer: {}",
                    conn.remote_node_id()
                        .expect("Connection should have a node ID")
                );

                if let Some(response_tx) = response_tx {
                    response_tx.send(Ok(())).expect("Connect receiver dropped");
                }

                let document_handle_clone = self.document_handle.clone();
                let message_tx_clone = self.message_tx.clone();
                tokio::spawn(async move {
                    if let Err(err) = Self::handle_peer(
                        document_handle_clone,
                        conn,
                        PeerAuth::YourPassphrase(secret_address.passphrase.clone()),
                    )
                    .await
                    {
                        debug!("Error while handling a peer: {:?}", err);
                    }
                    Self::reconnect(message_tx_clone, secret_address, 0)
                        .await
                        .expect("Failed to initiate reconnection");
                });
            }
        }
        Ok(())
    }

    async fn reconnect(
        message_tx: mpsc::Sender<EndpointMessage>,
        secret_address: SecretAddress,
        previous_attempts: usize,
    ) -> Result<()> {
        // Only log at "info" level if this is the first reconnection attempt.
        if previous_attempts == 0 {
            info!(
                "Connection to peer {} lost, will keep trying to reconnect...",
                secret_address.node_addr.node_id
            );
        } else {
            sleep(Duration::from_secs(10)).await;
            debug!(
                "Making another attempt to connect to peer {}...",
                secret_address.node_addr.node_id
            );
        }
        // We don't need to be notified, so we don't need to use the response channel.
        message_tx
            .send(EndpointMessage::Connect {
                secret_address,
                response_tx: None,
                previous_attempts: previous_attempts + 1,
            })
            .await?;
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
                                    self.handle_incoming_connection(conn);
                                }
                                Err(err) => {
                                    debug!("Error while accepting peer connection: {err}");
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

    fn handle_incoming_connection(&self, conn: iroh::endpoint::Connection) {
        let node_id = conn
            .remote_node_id()
            .expect("Connection should have a node ID");

        info!("Peer connected: {}", &node_id);

        let my_passphrase_clone = self.my_passphrase.clone();
        let document_handle_clone = self.document_handle.clone();
        tokio::spawn(async move {
            if let Err(err) = Self::handle_peer(
                document_handle_clone,
                conn,
                PeerAuth::MyPassphrase(my_passphrase_clone),
            )
            .await
            {
                warn!("Incoming connection failed: {err}");
            }

            info!("Peer disconnected: {node_id}",);
        });
    }

    async fn handle_peer(
        document_handle: DocumentActorHandle,
        conn: iroh::endpoint::Connection,
        auth: PeerAuth,
    ) -> Result<()> {
        let connection = IrohConnection::new(conn, auth).await?;
        let syncer = SyncActor::new(document_handle, Box::new(connection));
        syncer.run().await
    }
}

// Sends/receives PeerMessages to/from and Iroh connection.
struct IrohConnection {
    send: SendStream,
    message_rx: mpsc::Receiver<Result<PeerMessage>>,
}

impl IrohConnection {
    async fn new(conn: iroh::endpoint::Connection, auth: PeerAuth) -> Result<Self> {
        let (send, receive) = match auth {
            PeerAuth::YourPassphrase(passphrase) => {
                let (mut send, recv) = conn.open_bi().await?;

                send.write_all(&passphrase.to_bytes()).await?;

                (send, recv)
            }
            PeerAuth::MyPassphrase(passphrase) => {
                let (send, mut recv) = conn.accept_bi().await?;

                let mut received_passphrase = [0; 32];
                recv.read_exact(&mut received_passphrase).await?;

                // Guard against timing attacks.
                if !constant_time_eq::constant_time_eq(&received_passphrase, &passphrase.to_bytes())
                {
                    bail!("Peer provided incorrect passphrase.");
                }

                (send, recv)
            }
        };

        let (message_tx, message_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            let _ = Self::read_loop(receive, message_tx).await;
        });

        Ok(Self { send, message_rx })
    }

    async fn read_loop(
        mut receive: RecvStream,
        message_tx: mpsc::Sender<Result<PeerMessage>>,
    ) -> Result<()> {
        loop {
            let result = Self::read_next(&mut receive).await;

            message_tx.send(result).await?;
        }
    }

    async fn read_next(receive: &mut RecvStream) -> Result<PeerMessage> {
        let mut message_len_buf = [0; 4];
        receive.read_exact(&mut message_len_buf).await?;
        let byte_count = u32::from_be_bytes(message_len_buf);

        let mut bytes = vec![0; byte_count as usize];
        receive.read_exact(&mut bytes).await?;
        from_bytes(&bytes).context("Failed to convert bytes to PeerMessage")
    }
}

#[async_trait]
impl Connection<PeerMessage> for IrohConnection {
    async fn send(&mut self, message: PeerMessage) -> Result<()> {
        let bytes: Vec<u8> =
            to_allocvec(&message).context("Failed to convert PeerMessage to bytes")?;
        let byte_count =
            u32::try_from(bytes.len()).expect("Converting a length to u32 should work");

        self.send.write_all(&byte_count.to_be_bytes()).await?;
        self.send.write_all(&bytes).await?;

        Ok(())
    }

    async fn next(&mut self) -> Result<PeerMessage> {
        self.message_rx
            .recv()
            .await
            .context("Failed to await next peer message")?
    }
}
