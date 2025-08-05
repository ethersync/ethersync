use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::types::EphemeralMessage;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use automerge::sync::{Message as AutomergeSyncMessage, State as SyncState};
use iroh::endpoint::{RecvStream, SendStream};
use iroh::SecretKey;
use postcard::{from_bytes, to_allocvec};
use serde::{Deserialize, Serialize};
use std::mem;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::debug;

pub enum PeerAuth {
    MyPassphrase(SecretKey),
    YourPassphrase(SecretKey),
}

#[derive(Deserialize, Serialize)]
/// The PeerMessage is used for peer to peer data exchange.
pub enum PeerMessage {
    /// The Sync message contains the changes to the CRDT
    Sync(Vec<u8>),
    /// The Ephemeral message currently is used for cursor messages, but can later be used for
    /// other things that should not be persisted.
    Ephemeral(EphemeralMessage),
}

// Sends/receives PeerMessages to/from and Iroh connection.
pub struct IrohConnection {
    send: SendStream,
    message_rx: mpsc::Receiver<PeerMessage>,
}

impl IrohConnection {
    pub async fn new(conn: iroh::endpoint::Connection, auth: PeerAuth) -> Result<Self> {
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
            let _ = Self::read(receive, message_tx).await;
        });

        Ok(Self { send, message_rx })
    }

    async fn read(mut receive: RecvStream, message_tx: mpsc::Sender<PeerMessage>) -> Result<()> {
        loop {
            let mut message_len_buf = [0; 4];

            receive.read_exact(&mut message_len_buf).await?;
            let byte_count = u32::from_be_bytes(message_len_buf);
            let mut bytes = vec![0; byte_count as usize];
            receive.read_exact(&mut bytes).await?;
            message_tx.send(from_bytes(&bytes)?).await?;
        }
    }
}

#[async_trait]
pub trait Connection<T>: Send + Sync {
    async fn send(&mut self, message: T) -> Result<()>;
    async fn next(&mut self) -> Result<Option<T>>;
}

#[async_trait]
impl Connection<PeerMessage> for IrohConnection {
    async fn send(&mut self, message: PeerMessage) -> Result<()> {
        let bytes: Vec<u8> = to_allocvec(&message)?;
        let byte_count = u32::try_from(bytes.len());
        self.send.write_all(&byte_count?.to_be_bytes()).await?;
        self.send.write_all(&bytes).await?;
        Ok(())
    }

    async fn next(&mut self) -> Result<Option<PeerMessage>> {
        Ok(self.message_rx.recv().await)
    }
}

/// Transport-agnostic logic of how to sync with another peer.
/// Exchanges PeerMessages with the connection, and communicates with the document on the other side.
/// Maintains the sync state.
pub struct SyncActor {
    peer_state: SyncState,
    document_handle: DocumentActorHandle,
    connection: Box<dyn Connection<PeerMessage>>,
}

impl SyncActor {
    pub fn new(
        document_handle: DocumentActorHandle,
        connection: Box<dyn Connection<PeerMessage>>,
    ) -> Self {
        Self {
            peer_state: SyncState::new(),
            document_handle,
            connection,
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
            self.connection
                .send(PeerMessage::Sync(message.encode()))
                .await
                .context("Failed to send sync message on syncer_sender channel")?;
        }
        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
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
                            self.connection.send(PeerMessage::Ephemeral(ephemeral_message))
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
                Ok(Some(message)) = self.connection.next() => {
                    self.receive_peer_message(message).await?;
                }
            }
        }
    }
}
