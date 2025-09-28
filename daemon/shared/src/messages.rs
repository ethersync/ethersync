// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later
use serde::{Deserialize, Serialize};
use crate::types::{CursorId, CursorState};


#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct EphemeralMessage {
    pub cursor_id: CursorId,
    pub sequence_number: usize,
    pub cursor_state: CursorState,
}

#[derive(Deserialize, Serialize)]
/// The `PeerMessage` is used for peer to peer data exchange.
pub enum PeerMessage {
    /// The Sync message contains the changes to the CRDT
    Sync(Vec<u8>),
    /// The Ephemeral message currently is used for cursor messages, but can later be used for
    /// other things that should not be persisted.
    Ephemeral(EphemeralMessage),
}