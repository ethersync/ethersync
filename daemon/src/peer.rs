//! A peer is another daemon. This module is all about daemon to daemon communication.

use crate::daemon::{DocMessage, DocumentActorHandle};
use automerge::sync::{Message as AutomergeSyncMessage, State as SyncState};
use futures::{AsyncReadExt, AsyncWriteExt};
use libp2p::Stream;
use std::io;
use std::mem;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::debug;

pub fn spawn_peer_sync(mut stream: Stream, document_handle: &DocumentActorHandle) {
    stream.ignore_for_keep_alive();
    let (my_send, my_recv) = oneshot::channel();
    let p2p_handle = P2PActorHandle::new(stream, my_recv);
    let sync_handle = SyncActorHandle::new(document_handle, &p2p_handle);
    let _ = my_send.send(sync_handle);
}

/*
/// Reads from a TCP stream and forwards it to the Syncer
struct P2PReadActor {
    sync_handle: SyncActorHandle,
    stream: Stream,
    shutdown_token: CancellationToken,
}

impl P2PReadActor {
    fn new(
        stream: Stream,
        sync_handle: SyncActorHandle,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            sync_handle,
            stream,
            shutdown_token,
        }
    }

    async fn forward_sync_message(&self, message: Vec<u8>) {
        let message =
            AutomergeSyncMessage::decode(&message).expect("Failed to decode automerge message");
        self.sync_handle.send(message).await;
    }

    async fn read_message(&mut self) -> Result<Vec<u8>> {
        let mut message_len_buf = [0; 4];
        self.reader.read_exact(&mut message_len_buf).await?;
        let message_len = i32::from_be_bytes(message_len_buf);
        let mut message_buf = vec![0; message_len as usize];
        self.reader.read_exact(&mut message_buf).await?;
        Ok(message_buf)
    }

    async fn run(&mut self) {
        while let Ok(message) = self.read_message().await {
            self.forward_sync_message(message).await;
        }
        info!("Sync Receive loop stopped (peer disconnected)");
        self.shutdown_token.cancel();
    }
}
*/

/*
struct TCPWriteActor {
    writer: WriteHalf<TcpStream>,
    automerge_message_receiver: mpsc::Receiver<AutomergeSyncMessage>,
}

impl TCPWriteActor {
    fn new(
        writer: WriteHalf<TcpStream>,
        automerge_message_receiver: mpsc::Receiver<AutomergeSyncMessage>,
    ) -> Self {
        Self {
            writer,
            automerge_message_receiver,
        }
    }

    async fn run(&mut self) {
        while let Some(message) = self.automerge_message_receiver.recv().await {
            // TODO: move encode to Syncer for symmetry?
            let message = message.encode();
            let message_len = message.len() as i32;
            self.writer
                .write_all(&message_len.to_be_bytes())
                .await
                .expect("GenerateSyncMessage: write message len failed");
            self.writer
                .write_all(&message)
                .await
                .expect("GenerateSyncMessage: write message failed");
        }
        // At this point, our channel has been closed, which is the signal for us to stop.
        debug!("TCPWriteActor stopped (channel closed)");
    }
}
*/

struct SyncActor {
    syncer_receiver: mpsc::Receiver<AutomergeSyncMessage>,
    document_handle: DocumentActorHandle,
    p2p_handle: P2PActorHandle,
    peer_state: SyncState,
}

impl SyncActor {
    fn new(
        syncer_receiver: mpsc::Receiver<AutomergeSyncMessage>,
        document_handle: DocumentActorHandle,
        p2p_handle: P2PActorHandle,
    ) -> Self {
        Self {
            syncer_receiver,
            document_handle,
            p2p_handle,
            peer_state: SyncState::new(),
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
            self.p2p_handle.send(message).await;
        }
    }

    async fn run(mut self) {
        let mut doc_changed_ping_rx = self.document_handle.subscribe_document_changes();

        // Kick off initial synchronization with peer.
        self.generate_sync_message().await;

        loop {
            tokio::select! {
                () = self.p2p_handle.shutdown_token.cancelled() => {
                    debug!("Shutting down main SyncActor loop");
                    break;
                }
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

#[derive(Clone)]
pub struct SyncActorHandle {
    syncer_message_tx: mpsc::Sender<AutomergeSyncMessage>,
}

impl SyncActorHandle {
    pub fn new(document_handle: &DocumentActorHandle, p2p_handle: &P2PActorHandle) -> Self {
        let (syncer_message_tx, syncer_message_rx) = mpsc::channel(16);

        // Sync actor.
        let syncer = SyncActor::new(
            syncer_message_rx,
            document_handle.clone(),
            p2p_handle.clone(),
        );
        tokio::spawn(syncer.run());

        Self { syncer_message_tx }
    }

    async fn send(&self, message: AutomergeSyncMessage) {
        self.syncer_message_tx
            .send(message)
            .await
            .expect("Channel closed (TODO)");
    }
}

#[derive(Clone)]
pub struct P2PActorHandle {
    automerge_message_tx: mpsc::Sender<AutomergeSyncMessage>,
    shutdown_token: CancellationToken,
}

impl P2PActorHandle {
    pub fn new(mut stream: Stream, sync_handle: oneshot::Receiver<SyncActorHandle>) -> Self {
        let shutdown_token = CancellationToken::new();

        let _read_shutdown_token = shutdown_token.clone();

        let (automerge_message_tx, mut automerge_message_rx) =
            mpsc::channel::<AutomergeSyncMessage>(16);

        tokio::spawn(async move {
            let sync_handle = sync_handle
                .await
                .expect("Failed to receive SyncActorHandle");

            // kick off protocl!
            // if we receive on automerge_message_rx, write it to the stream
            // if we receive a size and a message, send it to the sync_handle
            loop {
                let mut message_len_buf = [0; 4];

                tokio::select! {
                    message_maybe = automerge_message_rx.recv() => {
                        match message_maybe {
                            Some(message) => {
                                let message = message.encode();
                                let message_len = message.len() as i32;
                                stream
                                    .write_all(&message_len.to_be_bytes())
                                    .await
                                    .expect("GenerateSyncMessage: write message len failed");
                                stream
                                    .write_all(&message)
                                    .await
                                    .expect("GenerateSyncMessage: write message failed");
                                }
                            None => {
                                // TODO: ?
                            }
                        }
                    }
                    _ = stream.read_exact(&mut message_len_buf) => {
                        let message_len = i32::from_be_bytes(message_len_buf);
                        let mut message_buf = vec![0; message_len as usize];
                        stream.read_exact(&mut message_buf).await.expect("Failed to read Automerge message");

                        let message =
                            AutomergeSyncMessage::decode(&message_buf).expect("Failed to decode automerge message");
                        sync_handle.send(message).await;
                    }
                }
            }
        });
        Self {
            automerge_message_tx,
            shutdown_token,
        }
    }

    async fn send(&mut self, message: AutomergeSyncMessage) {
        self.automerge_message_tx
            .send(message)
            .await
            .expect("Channel to P2PActor(s) closed.");
    }
}
