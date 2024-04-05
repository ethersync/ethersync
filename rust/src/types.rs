#![allow(dead_code)] // TODO: consider the dead code.
use automerge::PatchAction;
use operational_transform::{Operation as OTOperation, OperationSeq};
use serde_json::Value as JSONValue;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextDelta(pub Vec<TextOp>);

#[derive(Debug, Clone, PartialEq)]
pub enum TextOp {
    Retain(usize),
    Insert(String),
    Delete(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorTextDelta(pub Vec<EditorTextOp>);

#[derive(Debug, Clone, PartialEq)]
pub struct RevisionedEditorTextDelta {
    pub revision: usize,
    pub delta: EditorTextDelta,
}

/// When doing OT, many TextDeltas need a revision metadata, to see whether they apply.
#[derive(Debug, Clone, PartialEq)]
pub struct RevisionedTextDelta {
    pub revision: usize,
    pub delta: TextDelta,
}

impl RevisionedTextDelta {
    pub fn new(revision: usize, delta: TextDelta) -> Self {
        Self { revision, delta }
    }
}

impl From<RevisionedEditorTextDelta> for RevisionedTextDelta {
    fn from(rev_ed_delta: RevisionedEditorTextDelta) -> Self {
        Self::new(rev_ed_delta.revision, rev_ed_delta.delta.into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorTextOp {
    pub range: Range,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    anchor: Position,
    head: Position,
}

impl Range {
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn is_forward(&self) -> bool {
        self.anchor <= self.head
    }

    pub fn as_relative(&self) -> (usize, usize) {
        if self.is_forward() {
            (self.anchor, self.head - self.anchor)
        } else {
            (self.head, self.anchor - self.head)
        }
    }
}

// TODO: Expand this type to be a line + column description.
// Right now, we use the Vim plugin as it is, which only uses a character position.
type Position = usize;

//#[derive(Debug, Clone, PartialEq)]
//struct Position {
//    line: usize,
//    column: usize,
//}

/// Used to encapsulate our understanding of an OT change
impl TextDelta {
    pub fn retain(&mut self, n: usize) {
        self.0.push(TextOp::Retain(n));
    }
    pub fn insert(&mut self, s: &str) {
        self.0.push(TextOp::Insert(s.to_string()));
    }
    pub fn delete(&mut self, n: usize) {
        self.0.push(TextOp::Delete(n));
    }

    pub fn compose(self, other: Self) -> Self {
        let mut my_op_seq: OperationSeq = self.into();
        let other_op_seq: OperationSeq = other.into();
        if my_op_seq.target_len() < other_op_seq.base_len() {
            my_op_seq.retain((other_op_seq.base_len() - my_op_seq.target_len()) as u64);
        }
        my_op_seq
            .compose(&other_op_seq)
            .expect("Composition of deltas failed. Lengths messed up?")
            .into()
    }

    //fn transform(&mut self, other: Self) -> Self;
    // +some way of looking into the data
    // +invert?
    // +transform_position?
}

impl TryFrom<PatchAction> for TextDelta {
    type Error = anyhow::Error;

    fn try_from(patch_action: PatchAction) -> Result<Self, Self::Error> {
        let mut delta = TextDelta::default();

        match patch_action {
            PatchAction::SpliceText { index, value, .. } => {
                delta.retain(index);
                delta.insert(&value.make_string());
            }
            PatchAction::DeleteSeq { index, length } => {
                delta.retain(index);
                delta.delete(length);
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported patch action: {}",
                    patch_action
                ));
            }
        }

        Ok(delta)
    }
}

impl From<TextDelta> for Vec<PatchAction> {
    fn from(delta: TextDelta) -> Vec<PatchAction> {
        let mut patch_actions = vec![];
        let mut position = 0;
        for op in delta.0 {
            match op {
                TextOp::Retain(n) => {
                    position += n;
                }
                TextOp::Insert(s) => {
                    patch_actions.push(PatchAction::SpliceText {
                        index: position,
                        value: s.clone().into(),
                        marks: None,
                    });
                    // TODO: Can we avoid calculating this length?
                    position += s.chars().count();
                }
                TextOp::Delete(n) => {
                    patch_actions.push(PatchAction::DeleteSeq {
                        index: position,
                        length: n,
                    });
                }
            }
        }
        patch_actions
    }
}

impl From<OperationSeq> for TextDelta {
    fn from(op_seq: OperationSeq) -> Self {
        let mut delta = TextDelta::default();
        for op in op_seq.ops() {
            match op {
                OTOperation::Retain(n) => {
                    delta.retain(*n as usize);
                }
                OTOperation::Insert(s) => {
                    delta.insert(s);
                }
                OTOperation::Delete(n) => {
                    delta.delete(*n as usize);
                }
            }
        }
        delta
    }
}

impl From<TextDelta> for OperationSeq {
    fn from(delta: TextDelta) -> OperationSeq {
        let mut op_seq = OperationSeq::default();
        for op in delta.0 {
            match op {
                TextOp::Retain(n) => {
                    op_seq.retain(n as u64);
                }
                TextOp::Insert(s) => {
                    op_seq.insert(&s);
                }
                TextOp::Delete(n) => {
                    op_seq.delete(n as u64);
                }
            }
        }
        op_seq
    }
}

impl From<EditorTextDelta> for TextDelta {
    fn from(ed_delta: EditorTextDelta) -> Self {
        let mut delta = TextDelta::default();
        // TODO: add support, when needed
        assert!(
            ed_delta.0.len() == 1,
            "We don't yet support EditorTextDelta with multiple operations."
        );
        for ed_op in ed_delta.0 {
            let mut delta_step = TextDelta::default();
            if ed_op.range.is_empty() {
                if ed_op.replacement.is_empty() {
                    // insert
                    delta_step.retain(ed_op.range.anchor);
                    delta_step.insert(&ed_op.replacement);
                }
            } else {
                // delete or replace
                let (position, length) = ed_op.range.as_relative();
                delta_step.retain(position);
                delta_step.delete(length);
                if ed_op.replacement.is_empty() {
                    // replace
                    delta_step.insert(&ed_op.replacement);
                }
            }
            delta = delta.compose(delta_step);
        }
        delta
    }
}

impl From<TextDelta> for EditorTextDelta {
    fn from(delta: TextDelta) -> Self {
        let mut editor_ops = vec![];
        let mut position = 0;
        for op in delta.0 {
            match op {
                TextOp::Retain(n) => position += n,
                TextOp::Delete(n) => {
                    editor_ops.push(EditorTextOp {
                        range: Range {
                            anchor: position,
                            head: (position + n),
                        },
                        replacement: "".to_string(),
                    });
                }
                TextOp::Insert(s) => {
                    editor_ops.push(EditorTextOp {
                        range: Range {
                            anchor: position,
                            head: position,
                        },
                        replacement: s.to_string(),
                    });
                    position += s.chars().count();
                }
            }
        }

        Self(editor_ops)
    }
}

// TODO: Write this in a nicer way...
impl TryFrom<JSONValue> for RevisionedEditorTextDelta {
    type Error = anyhow::Error;
    fn try_from(json: JSONValue) -> Result<Self, Self::Error> {
        match json {
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::String(method)) = map.get("method") {
                    match method.as_str() {
                        "insert" => {
                            if let Some(serde_json::Value::Array(array)) = map.get("params") {
                                if let Some(serde_json::Value::Number(revision)) = array.get(1) {
                                    if let Some(serde_json::Value::Number(position)) = array.get(2)
                                    {
                                        if let Some(serde_json::Value::String(text)) = array.get(3)
                                        {
                                            let revision = revision
                                                .as_u64()
                                                .expect("Failed to parse revision");
                                            let position = position
                                                .as_u64()
                                                .expect("Failed to parse position");
                                            let text = text.as_str().to_string();
                                            let op = EditorTextOp {
                                                range: Range {
                                                    anchor: position as usize,
                                                    head: position as usize,
                                                },
                                                replacement: text,
                                            };
                                            let delta = EditorTextDelta(vec![op]);
                                            Ok(RevisionedEditorTextDelta {
                                                revision: revision as usize,
                                                delta,
                                            })
                                        } else {
                                            Err(anyhow::anyhow!(
                                                "Could not find text param in position #3"
                                            ))
                                        }
                                    } else {
                                        Err(anyhow::anyhow!(
                                            "Could not find position param in position #2"
                                        ))
                                    }
                                } else {
                                    Err(anyhow::anyhow!(
                                        "Could not find revision param in position #1"
                                    ))
                                }
                            } else {
                                Err(anyhow::anyhow!("Could not find params for insert method"))
                            }
                        }
                        "delete" => {
                            if let Some(serde_json::Value::Array(array)) = map.get("params") {
                                if let Some(serde_json::Value::Number(revision)) = array.get(1) {
                                    if let Some(serde_json::Value::Number(position)) = array.get(2)
                                    {
                                        if let Some(serde_json::Value::Number(length)) =
                                            array.get(3)
                                        {
                                            let revision = revision
                                                .as_u64()
                                                .expect("Failed to parse revision");
                                            let position = position
                                                .as_u64()
                                                .expect("Failed to parse position");

                                            let length =
                                                length.as_u64().expect("Failed to parse length");

                                            let op = EditorTextOp {
                                                range: Range {
                                                    anchor: position as usize,
                                                    head: position as usize + length as usize,
                                                },
                                                replacement: "".to_string(),
                                            };
                                            let delta = EditorTextDelta(vec![op]);
                                            Ok(RevisionedEditorTextDelta {
                                                revision: revision as usize,
                                                delta,
                                            })
                                        } else {
                                            Err(anyhow::anyhow!(
                                                "Could not find length param in position #3"
                                            ))
                                        }
                                    } else {
                                        Err(anyhow::anyhow!(
                                            "Could not find position param in position #2"
                                        ))
                                    }
                                } else {
                                    Err(anyhow::anyhow!("Could not find params for delete method"))
                                }
                            } else {
                                Err(anyhow::anyhow!(
                                    "Could not find revision param in position #1"
                                ))
                            }
                        }
                        _ => Err(anyhow::anyhow!("Unknown JSON method: {}", method)),
                    }
                } else {
                    Err(anyhow::anyhow!("Could not find method in JSON message"))
                }
            }
            _ => Err(anyhow::anyhow!(
                "JSON message is not an object: {:#?}",
                json
            )),
        }
    }
}

#[cfg(test)]
pub mod factories {
    use super::*;

    pub fn insert(at: usize, s: &str) -> TextDelta {
        let mut delta: TextDelta = Default::default();
        delta.retain(at);
        delta.insert(s);
        delta
    }

    pub fn delete(from: usize, length: usize) -> TextDelta {
        let mut delta: TextDelta = Default::default();
        delta.retain(from);
        delta.delete(length);
        delta
    }
    pub fn rev_delta(revision: usize, delta: TextDelta) -> RevisionedTextDelta {
        RevisionedTextDelta::new(revision, delta)
    }
}

#[cfg(test)]
mod tests {
    use super::factories::*;
    use super::*;

    #[test]
    fn range_forward() {
        assert!(Range { anchor: 0, head: 1 }.is_forward());
        assert!(!Range { anchor: 1, head: 0 }.is_forward());
    }

    #[test]
    fn conversion_editor_to_text_delta_insert() {
        let ed_delta = EditorTextDelta(vec![EditorTextOp {
            range: Range { anchor: 1, head: 1 },
            replacement: "a".to_string(),
        }]);
        let delta: TextDelta = ed_delta.into();
        assert_eq!(delta, insert(1, "a"));
    }

    #[test]
    fn conversion_editor_to_text_delta_delete() {
        let ed_delta = EditorTextDelta(vec![EditorTextOp {
            range: Range { anchor: 1, head: 3 },
            replacement: "".to_string(),
        }]);
        let delta: TextDelta = ed_delta.into();
        assert_eq!(delta, delete(1, 2));
    }

    #[test]
    #[ignore] // TODO: enable, when we support multiple ops in one delta
    fn conversion_editor_to_text_delta_insert_twice() {
        let ed_delta = EditorTextDelta(vec![
            EditorTextOp {
                range: Range { anchor: 1, head: 1 },
                replacement: "long".to_string(),
            },
            EditorTextOp {
                range: Range { anchor: 2, head: 2 },
                replacement: "short".to_string(),
            },
        ]);
        let delta: TextDelta = ed_delta.into();
        let mut expected = TextDelta::default();
        expected.retain(1);
        expected.insert("l");
        expected.insert("short");
        expected.insert("ong");
        assert_eq!(delta, expected);
    }

    #[test]
    fn json_to_revisioned_editor_text_delta() {
        let json = serde_json::json!({
            "method": "insert",
            "params": ["", 0, 1, "a"]
        });

        let delta = RevisionedEditorTextDelta::try_from(json).unwrap();
        assert_eq!(delta.revision, 0);
        assert_eq!(
            delta.delta,
            EditorTextDelta(vec![EditorTextOp {
                range: Range { anchor: 1, head: 1 },
                replacement: "a".to_string(),
            }])
        );

        let json = serde_json::json!({
            "method": "delete",
            "params": ["", 2, 1, 3]
        });

        let delta = RevisionedEditorTextDelta::try_from(json).unwrap();
        assert_eq!(delta.revision, 2);
        assert_eq!(
            delta.delta,
            EditorTextDelta(vec![EditorTextOp {
                range: Range { anchor: 1, head: 4 },
                replacement: "".to_string(),
            }])
        );
    }
}
