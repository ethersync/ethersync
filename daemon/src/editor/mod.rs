// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! This module is all about daemon to editor communication.
#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::types::EditorProtocolObject;
use anyhow::{Context, Result};
use futures::StreamExt;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tokio::io::WriteHalf;
#[cfg(windows)]
use tokio::net::windows::named_pipe::NamedPipeServer;
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::{
    bytes::BytesMut,
    codec::{Encoder, FramedWrite},
};
use tracing::info;

pub type EditorId = usize;

#[cfg(windows)]
pub type EditorWriter = FramedWrite<WriteHalf<NamedPipeServer>, EditorProtocolCodec>;
#[cfg(windows)]
pub type EditorStream = NamedPipeServer;
#[cfg(unix)]
pub type EditorWriter = FramedWrite<WriteHalf<UnixStream>, EditorProtocolCodec>;
#[cfg(unix)]
pub type EditorStream = tokio::net::UnixStream;

#[derive(Debug)]
pub struct EditorProtocolCodec;

impl Encoder<EditorProtocolObject> for EditorProtocolCodec {
    type Error = anyhow::Error;

    fn encode(
        &mut self,
        item: EditorProtocolObject,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        let payload = item.to_jsonrpc()?;
        dst.extend_from_slice(format!("{payload}\n").as_bytes());
        Ok(())
    }
}

pub trait Editor {
    fn get_socket_path(&self) -> PathBuf;
    fn spawn_socket_listener(&self, document_handle: DocumentActorHandle) -> Result<()>;
}

async fn handle_editor_connection(
    stream: EditorStream,
    document_handle: DocumentActorHandle,
    editor_id: EditorId,
) {
    let (stream_read, stream_write) = tokio::io::split(stream);
    let mut reader = FramedRead::new(stream_read, LinesCodec::new());
    let writer = FramedWrite::new(stream_write, EditorProtocolCodec);

    document_handle
        .send_message(DocMessage::NewEditorConnection(editor_id, writer))
        .await;
    info!("Editor #{editor_id} connected.");

    while let Some(Ok(line)) = reader.next().await {
        document_handle
            .send_message(DocMessage::FromEditor(editor_id, line))
            .await;
    }

    document_handle
        .send_message(DocMessage::CloseEditorConnection(editor_id))
        .await;
    info!("Editor #{editor_id} disconnected.");
}
