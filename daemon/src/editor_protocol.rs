// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::types::{CursorId, EditorTextDelta, Range};
use anyhow::bail;
use serde::{Deserialize, Serialize};

type DocumentUri = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OutgoingMessage {
    Request(EditorProtocolMessageToEditor),
    Response(JSONRPCResponse),
}

impl OutgoingMessage {
    pub fn to_jsonrpc(&self) -> Result<String, anyhow::Error> {
        let json_value =
            serde_json::to_value(self).expect("Failed to convert editor message to a JSON value");
        if let serde_json::Value::Object(mut map) = json_value {
            map.insert("jsonrpc".to_string(), "2.0".into());
            let payload = serde_json::to_string(&map)?;
            Ok(payload)
        } else {
            bail!("EditorProtocolMessage was not serialized to a map");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IncomingMessage {
    Request {
        id: usize,
        #[serde(flatten)]
        payload: EditorProtocolMessageFromEditor,
    },
    Notification {
        #[serde(flatten)]
        payload: EditorProtocolMessageFromEditor,
    },
}
impl IncomingMessage {
    pub fn from_jsonrpc(jsonrpc: &str) -> Result<Self, anyhow::Error> {
        let message = serde_json::from_str(jsonrpc)?;
        Ok(message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum EditorProtocolMessageToEditor {
    Edit {
        uri: DocumentUri,
        revision: usize,
        delta: EditorTextDelta,
    },
    Cursor {
        userid: CursorId,
        name: Option<String>,
        uri: DocumentUri,
        ranges: Vec<Range>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JSONRPCResponse {
    RequestSuccess {
        id: usize,
        result: String,
    },
    RequestError {
        // id must be Null if there was an error detecting the id in the Request Object.
        id: Option<usize>,
        error: EditorProtocolMessageError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorProtocolMessageError {
    pub code: i32,
    pub message: String,
    pub data: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum EditorProtocolMessageFromEditor {
    Open {
        uri: DocumentUri,
        content: String,
    },
    Close {
        uri: DocumentUri,
    },
    Edit {
        uri: DocumentUri,
        revision: usize,
        delta: EditorTextDelta,
    },
    Cursor {
        uri: DocumentUri,
        ranges: Vec<Range>,
    },
}

#[cfg(test)]
mod test_serde {

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn open() {
        let message = IncomingMessage::from_jsonrpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"open","params":{"uri":"file:\/\/\/tmp\/file","content":"initial content"}}"#,
        );
        assert_eq!(
            message.unwrap(),
            IncomingMessage::Request {
                id: 1,
                payload: EditorProtocolMessageFromEditor::Open {
                    uri: "file:///tmp/file".into(),
                    content: "initial content".to_string(),
                }
            }
        );
    }

    #[test]
    fn success() {
        let message = OutgoingMessage::Response(JSONRPCResponse::RequestSuccess {
            id: 1,
            result: "success".to_string(),
        });
        let jsonrpc = message.to_jsonrpc();
        assert_eq!(
            jsonrpc.unwrap(),
            r#"{"id":1,"jsonrpc":"2.0","result":"success"}"#
        );
    }

    #[test]
    fn error() {
        let message = OutgoingMessage::Response(JSONRPCResponse::RequestError {
            id: Some(1),
            error: EditorProtocolMessageError {
                code: -1,
                message: "title".into(),
                data: Some("content".into()),
            },
        });
        let jsonrpc = message.to_jsonrpc();
        assert_eq!(
            jsonrpc.unwrap(),
            r#"{"error":{"code":-1,"data":"content","message":"title"},"id":1,"jsonrpc":"2.0"}"#
        );
    }
}
