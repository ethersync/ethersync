#![allow(dead_code)]
use automerge::PatchAction;
use operational_transform::{Operation as OTOperation, OperationSeq};
use ropey::Rope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextDelta(pub Vec<TextOp>);

impl IntoIterator for TextDelta {
    type Item = TextOp;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextOp {
    Retain(usize),
    Insert(String),
    Delete(usize),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorTextDelta(pub Vec<EditorTextOp>);

impl IntoIterator for EditorTextDelta {
    type Item = EditorTextOp;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionedEditorTextDelta {
    pub revision: usize,
    pub delta: EditorTextDelta,
}

/// When doing OT, many `TextDelta`s need a revision metadata, to see whether they apply.
#[derive(Debug, Clone, PartialEq)]
pub struct RevisionedTextDelta {
    pub revision: usize,
    pub delta: TextDelta,
}

impl RevisionedTextDelta {
    #[must_use]
    pub fn new(revision: usize, delta: TextDelta) -> Self {
        Self { revision, delta }
    }
}

impl RevisionedTextDelta {
    pub fn from_rev_ed_delta(rev_ed_delta: RevisionedEditorTextDelta, content: &str) -> Self {
        Self::new(
            rev_ed_delta.revision,
            TextDelta::from_ed_delta(rev_ed_delta.delta, content),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum EditorProtocolMessage {
    Open {
        uri: DocumentUri,
    },
    Close {
        uri: DocumentUri,
    },
    Edit {
        uri: DocumentUri,
        delta: RevisionedEditorTextDelta,
    },
    // TODO coming later:
    // Cursor{uri: DocumentUri, ranges: RevisionedRanges}
}

type DocumentUri = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorTextOp {
    pub range: Range,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub anchor: Position,
    pub head: Position,
}

impl Range {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    #[must_use]
    pub fn is_forward(&self) -> bool {
        (self.anchor.line < self.head.line)
            || (self.anchor.line == self.head.line && self.anchor.character <= self.head.character)
    }

    #[must_use]
    pub fn as_relative(&self, content: &str) -> (usize, usize) {
        let anchor_offset = self.anchor.to_offset(content);
        let head_offset = self.head.to_offset(content);
        if self.is_forward() {
            (anchor_offset, head_offset - anchor_offset)
        } else {
            (head_offset, anchor_offset - head_offset)
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

impl Position {
    /// will panic when used with not matching offset/content
    fn from_offset(full_offset: usize, content: &str) -> Self {
        let rope = Rope::from_str(content);
        let line = rope.char_to_line(full_offset);
        let character = full_offset - rope.line_to_char(line);
        Self { line, character }
    }

    fn to_offset(&self, content: &str) -> usize {
        let rope = Rope::from_str(content);
        rope.line_to_char(self.line) + self.character
    }
}

#[cfg(test)]
mod ropey_test {
    use super::Position;

    #[test]
    fn zero_offset() {
        assert_eq!(
            //       position           0123456 78901 2345
            //       character             0123456 01234 0124
            Position::from_offset(0, "hallo,\nneue\nwelt"),
            Position {
                line: 0,
                character: 0
            }
        );
        assert_eq!(
            Position {
                line: 0,
                character: 0
            }
            .to_offset("hallo,\nneue\nwelt"),
            0
        );
    }

    #[test]
    fn more_offset_first_line() {
        assert_eq!(
            Position::from_offset(3, "hallo,\nneue\nwelt"),
            Position {
                line: 0,
                character: 3
            }
        );
        assert_eq!(
            Position::from_offset(3, "h🥕llo,\nneue\nwelt"),
            Position {
                line: 0,
                character: 3
            }
        );
        assert_eq!(
            Position {
                line: 0,
                character: 3
            }
            .to_offset("hallo,\nneue\nwelt"),
            3
        );
        assert_eq!(
            Position {
                line: 0,
                character: 3
            }
            .to_offset("h🥕llo,\nneue\nwelt"),
            3
        );
    }

    #[test]
    fn offset_second_line() {
        assert_eq!(
            Position::from_offset(7, "hallo,\nneue\nwelt"),
            Position {
                line: 1,
                character: 0
            }
        );
        assert_eq!(
            Position::from_offset(7, "h🥕llo,\nneue\nwelt"),
            Position {
                line: 1,
                character: 0
            }
        );
        assert_eq!(
            Position::from_offset(9, "hallo,\nneue\nwelt"),
            Position {
                line: 1,
                character: 2
            }
        );
        assert_eq!(
            Position::from_offset(9, "h🥕llo,\nneue\nwelt"),
            Position {
                line: 1,
                character: 2
            }
        );
        assert_eq!(
            Position {
                line: 1,
                character: 0
            }
            .to_offset("hallo,\nneue\nwelt"),
            7
        );
        assert_eq!(
            Position {
                line: 1,
                character: 0
            }
            .to_offset("h🥕llo,\nneue\nwelt"),
            7
        );
        assert_eq!(
            Position {
                line: 1,
                character: 2
            }
            .to_offset("hallo,\nneue\nwelt"),
            9
        );
        assert_eq!(
            Position {
                line: 1,
                character: 2
            }
            .to_offset("h🥕llo,\nneue\nwelt"),
            9
        );
    }

    #[test]
    fn offset_third_line() {
        assert_eq!(
            Position::from_offset(12, "hallo,\nneue\nwelt"),
            Position {
                line: 2,
                character: 0
            }
        );
        assert_eq!(
            Position::from_offset(12, "h🥕llo,\nneue\nwelt"),
            Position {
                line: 2,
                character: 0
            }
        );
        assert_eq!(
            Position::from_offset(15, "hallo,\nneue\nwelt"),
            Position {
                line: 2,
                character: 3
            }
        );
        assert_eq!(
            Position::from_offset(15, "h🥕llo,\nneue\nwelt"),
            Position {
                line: 2,
                character: 3
            }
        );
        assert_eq!(
            Position {
                line: 2,
                character: 0
            }
            .to_offset("hallo,\nneue\nwelt"),
            12
        );
        assert_eq!(
            Position {
                line: 2,
                character: 0
            }
            .to_offset("h🥕llo,\nneue\nwelt"),
            12
        );
        assert_eq!(
            Position {
                line: 2,
                character: 3
            }
            .to_offset("hallo,\nneue\nwelt"),
            15
        );
        assert_eq!(
            Position {
                line: 2,
                character: 3
            }
            .to_offset("h🥕llo,\nneue\nwelt"),
            15
        );
    }

    #[test]
    fn last_implicit_newline_does_not_panic() {
        assert_eq!(
            Position::from_offset(16, "h🥕llo,\nneue\nwelt"),
            Position {
                line: 2,
                character: 4
            }
        );
        assert_eq!(
            Position {
                line: 2,
                character: 4
            }
            .to_offset("h🥕llo,\nneue\nwelt"),
            16
        );
    }

    #[test]
    #[should_panic]
    fn offset_out_of_bounds_from_offset() {
        Position::from_offset(17, "h🥕llo,\nneue\nwelt");
    }

    #[ignore] // WIP, see below.
    #[test]
    #[should_panic]
    fn offset_out_of_bounds_to_offset() {
        // TODO: do we want this to panic?
        Position {
            line: 2,
            character: 5,
        }
        .to_offset("h🥕llo,\nneue\nwelt");
        // even this doesn't panic, that surprises me. Check.
        Position {
            line: 3,
            character: 5,
        }
        .to_offset("h🥕llo,\nneue\nwelt");
    }
}

/// Used to encapsulate our understanding of an OT change
impl TextDelta {
    pub fn retain(&mut self, n: usize) {
        if n != 0 {
            self.0.push(TextOp::Retain(n));
        }
    }
    pub fn insert(&mut self, s: &str) {
        if !s.is_empty() {
            self.0.push(TextOp::Insert(s.to_string()));
        }
    }
    pub fn delete(&mut self, n: usize) {
        if n != 0 {
            self.0.push(TextOp::Delete(n));
        }
    }

    #[must_use]
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
        for op in delta {
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
        for op in delta {
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

impl TextDelta {
    pub fn from_ed_delta(ed_delta: EditorTextDelta, content: &str) -> Self {
        let mut delta = TextDelta::default();
        // TODO: add support, when needed
        assert!(
            ed_delta.0.len() == 1,
            "We don't yet support EditorTextDelta with multiple operations."
        );
        for ed_op in ed_delta {
            let mut delta_step = TextDelta::default();
            if ed_op.range.is_empty() {
                if !ed_op.replacement.is_empty() {
                    // insert
                    delta_step.retain(ed_op.range.anchor.to_offset(content));
                    delta_step.insert(&ed_op.replacement);
                }
            } else {
                // delete or replace
                let (position, length) = ed_op.range.as_relative(content);
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

impl EditorTextDelta {
    pub fn from_delta(delta: TextDelta, content: &str) -> Self {
        let mut editor_ops = vec![];
        let mut position = 0;
        for op in delta {
            match op {
                TextOp::Retain(n) => position += n,
                TextOp::Delete(n) => {
                    editor_ops.push(EditorTextOp {
                        range: Range {
                            anchor: Position::from_offset(position, content),
                            head: Position::from_offset(position + n, content),
                        },
                        replacement: String::new(),
                    });
                }
                TextOp::Insert(s) => {
                    editor_ops.push(EditorTextOp {
                        range: Range {
                            anchor: Position::from_offset(position, content),
                            head: Position::from_offset(position, content),
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

    pub fn replacement(
        anchor: (usize, usize),
        head: (usize, usize),
        replacement: &str,
    ) -> EditorTextOp {
        EditorTextOp {
            range: Range {
                anchor: Position {
                    line: anchor.0,
                    character: anchor.1,
                },
                head: Position {
                    line: head.0,
                    character: head.1,
                },
            },
            replacement: replacement.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::factories::*;
    use super::*;

    #[test]
    fn range_forward() {
        assert!(Range {
            anchor: Position {
                line: 0,
                character: 0
            },
            head: Position {
                line: 0,
                character: 1
            }
        }
        .is_forward());
        assert!(Range {
            anchor: Position {
                line: 0,
                character: 1
            },
            head: Position {
                line: 1,
                character: 0
            }
        }
        .is_forward());
        assert!(!Range {
            anchor: Position {
                line: 0,
                character: 1
            },
            head: Position {
                line: 0,
                character: 0
            }
        }
        .is_forward());
        assert!(!Range {
            anchor: Position {
                line: 1,
                character: 0
            },
            head: Position {
                line: 0,
                character: 1
            }
        }
        .is_forward());
    }

    #[test]
    fn conversion_editor_to_text_delta_insert() {
        let ed_delta = EditorTextDelta(vec![EditorTextOp {
            range: Range {
                anchor: Position {
                    line: 0,
                    character: 1,
                },
                head: Position {
                    line: 0,
                    character: 1,
                },
            },
            replacement: "a".to_string(),
        }]);
        let delta: TextDelta = TextDelta::from_ed_delta(ed_delta, "foo");
        assert_eq!(delta, insert(1, "a"));
    }

    /*
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
    */
}
