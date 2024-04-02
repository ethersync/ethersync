#![allow(dead_code)]
use automerge::PatchAction;
use operational_transform::{Operation as OTOperation, OperationSeq};

#[derive(Debug, Clone, PartialEq, Default)]
struct TextDelta(Vec<TextOp>);

#[derive(Debug, Clone, PartialEq)]
enum TextOp {
    Retain(usize),
    Insert(String),
    Delete(usize),
}

struct EditorTextDelta(Vec<EditorTextOp>);

#[derive(Debug, Clone, PartialEq)]
struct EditorTextOp {
    range: Range,
    replacement: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Range {
    anchor: Position,
    head: Position,
}

impl Range {
    fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    fn is_forward(&self) -> bool {
        self.anchor <= self.head
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
    fn retain(&mut self, n: usize) {
        self.0.push(TextOp::Retain(n));
    }
    fn insert(&mut self, s: &str) {
        self.0.push(TextOp::Insert(s.to_string()));
    }
    fn delete(&mut self, n: usize) {
        self.0.push(TextOp::Delete(n));
    }
    //fn transform(&mut self, other: Self) -> Self;
    //fn compose(&mut self, other: Self) -> Self;
    // +some way of looking into the data
    // +invert?
    // +transform_position?
}

impl From<PatchAction> for TextDelta {
    fn from(patch: PatchAction) -> Self {
        let mut delta = TextDelta::default();

        match patch {
            PatchAction::SpliceText { index, value, .. } => {
                delta.retain(index);
                delta.insert(&value.make_string());
            }
            PatchAction::DeleteSeq { index, length } => {
                delta.retain(index);
                delta.delete(length);
            }
            _ => {
                panic!("Unsupported patch action.");
            }
        }

        delta
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_forward() {
        assert!(Range { anchor: 0, head: 1 }.is_forward());
        assert!(!Range { anchor: 1, head: 0 }.is_forward());
    }
}
