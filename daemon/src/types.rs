#![allow(dead_code)]
use automerge::{Patch, PatchAction};
use operational_transform::{Operation as OTOperation, OperationSeq};
use ropey::Rope;
use serde::{Deserialize, Serialize};
use tracing::warn;

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

impl RevisionedEditorTextDelta {
    #[must_use]
    pub fn new(revision: usize, delta: EditorTextDelta) -> Self {
        Self { revision, delta }
    }
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

#[derive(Debug, Clone, PartialEq)]
pub struct FileTextDelta {
    pub file_path: String,
    pub delta: TextDelta,
}

impl FileTextDelta {
    #[must_use]
    pub fn new(file_path: String, delta: TextDelta) -> Self {
        Self { file_path, delta }
    }
}

type DocumentUri = String;
type UserId = Vec<u8>;
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct RevisionedRanges {
    revision: usize,
    ranges: Vec<Range>,
}

#[derive(Serialize, Deserialize)]
pub struct CursorState {
    pub userid: UserId,
    pub file_path: String,
    pub ranges: RevisionedRanges,
}

pub enum PatchEffect {
    FileChange(FileTextDelta),
    CursorChange(CursorState),
}

impl PatchEffect {
    pub fn from_crdt_patches(patches: Vec<Patch>) -> Vec<Self> {
        let mut file_deltas: Vec<Self> = vec![];

        for patch in patches {
            match patch.try_into() {
                Ok(result) => {
                    file_deltas.push(result);
                }
                Err(e) => {
                    warn!("Failed to convert patch to delta: {:#?}", e);
                }
            }
        }

        file_deltas
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
    Cursor {
        userid: UserId,
        uri: DocumentUri,
        ranges: RevisionedRanges,
    },
}

impl EditorProtocolMessage {
    /// # Errors
    ///
    /// Will return an error if the conversion to JSONRPC fails.
    pub fn to_jsonrpc(&self) -> Result<String, anyhow::Error> {
        let json_value =
            serde_json::to_value(self).expect("Failed to convert editor message to a JSON value");
        if let serde_json::Value::Object(mut map) = json_value {
            map.insert("jsonrpc".to_string(), "2.0".into());
            let payload =
                serde_json::to_string(&map).expect("Failed to serialize modified editor message");
            Ok(payload)
        } else {
            panic!("EditorProtocolMessage was not serialized to a map");
        }
    }

    pub fn from_jsonrpc(jsonrpc: &str) -> Result<Self, anyhow::Error> {
        let message = serde_json::from_str(jsonrpc).expect("Failed to deserialize editor message");
        Ok(message)
    }
}

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

        assert!(self.character <= rope.line(self.line).len_chars());

        rope.line_to_char(self.line) + self.character
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

    /// # Errors
    ///
    /// Will return an error if the composition of the deltas fails.
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

impl TryFrom<Patch> for PatchEffect {
    type Error = anyhow::Error;

    fn try_from(patch: Patch) -> Result<Self, Self::Error> {
        fn file_path_from_path_default(
            path: &[(automerge::ObjId, automerge::Prop)],
        ) -> Result<String, anyhow::Error> {
            if path.len() != 2 {
                return Err(anyhow::anyhow!(
                    "Unexpected path in Automerge patch, length is not 2"
                ));
            }
            let (_obj_id, prop) = &path[1];
            if let automerge::Prop::Map(file_path) = prop {
                return Ok(file_path.into());
            }
            Err(anyhow::anyhow!(
                "Unexpected path in Automerge patch: Prop is not a map"
            ))
        }

        let mut delta = TextDelta::default();

        if patch.path.is_empty() {
            return Err(anyhow::anyhow!(
                "Path is empty, this is probably a top-level change creating the files or states map"
            ));
        }

        match &patch.path[0] {
            (_, automerge::Prop::Map(key)) if key == "files" => {
                if patch.path.len() == 1 {
                    match patch.action {
                        PatchAction::PutMap { key, .. } => {
                            // This action happens when a new file is created.
                            // We return an empty delta on the new file, so that the file is created on disk when
                            // synced over to another peer. TODO: Is this the best way to solve this?
                            Ok(PatchEffect::FileChange(FileTextDelta::new(key, delta)))
                        }
                        other_action => Err(anyhow::anyhow!(
                            "Unsupported patch action for path 'files': {}",
                            other_action
                        )),
                    }
                } else if patch.path.len() == 2 {
                    match patch.action {
                        PatchAction::SpliceText { index, value, .. } => {
                            delta.retain(index);
                            delta.insert(&value.make_string());
                            Ok(PatchEffect::FileChange(FileTextDelta::new(
                                file_path_from_path_default(&patch.path)?,
                                delta,
                            )))
                        }
                        PatchAction::DeleteSeq { index, length } => {
                            delta.retain(index);
                            delta.delete(length);
                            Ok(PatchEffect::FileChange(FileTextDelta::new(
                                file_path_from_path_default(&patch.path)?,
                                delta,
                            )))
                        }
                        other_action => Err(anyhow::anyhow!(
                            "Unsupported patch action for path 'files/*': {}",
                            other_action
                        )),
                    }
                } else {
                    Err(anyhow::anyhow!(
                        "Unexpected path action for path 'files/**', expected it to be of length 1 or 2"
                    ))
                }
            }
            (_, automerge::Prop::Map(key)) if key == "states" => {
                if patch.path.len() == 2 {
                    match patch.action {
                        PatchAction::SpliceText { index, value, .. } => {
                            assert_eq!(index, 0);
                            let cursor_state = serde_json::from_str(&value.make_string())?;
                            Ok(PatchEffect::CursorChange(cursor_state))
                        }
                        other_action => Err(anyhow::anyhow!(
                            "Unsupported patch action for path 'states/*': {}",
                            other_action
                        )),
                    }
                } else {
                    Err(anyhow::anyhow!(
                        "Unexpected path action for path 'states/...', expected it to be of length 2"
                    ))
                }
            }
            (_, _) => Err(anyhow::anyhow!(
                "Unexpected path in Automerge patch, expected it to begin with 'files' or 'states'"
            )),
        }
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
    /// # Panics
    ///
    /// Will panic if the delta contains multiple operations.
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
                if !ed_op.replacement.is_empty() {
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
                    position += n;
                }
                TextOp::Insert(s) => {
                    editor_ops.push(EditorTextOp {
                        range: Range {
                            anchor: Position::from_offset(position, content),
                            head: Position::from_offset(position, content),
                        },
                        replacement: s.to_string(),
                    });
                }
            }
        }

        Self(editor_ops)
    }
}

pub mod factories {
    use super::*;

    pub fn insert(at: usize, s: &str) -> TextDelta {
        let mut delta = TextDelta::default();
        delta.retain(at);
        delta.insert(s);
        delta
    }

    pub fn delete(from: usize, length: usize) -> TextDelta {
        let mut delta = TextDelta::default();
        delta.retain(from);
        delta.delete(length);
        delta
    }

    pub fn replace(from: usize, length: usize, s: &str) -> TextDelta {
        let mut delta = TextDelta::default();
        delta.retain(from);
        delta.delete(length);
        delta.insert(s);
        delta
    }

    pub fn rev_delta(revision: usize, delta: TextDelta) -> RevisionedTextDelta {
        RevisionedTextDelta::new(revision, delta)
    }

    pub fn rev_ed_delta(revision: usize, delta: EditorTextDelta) -> RevisionedEditorTextDelta {
        RevisionedEditorTextDelta::new(revision, delta)
    }

    pub fn range(anchor: (usize, usize), head: (usize, usize)) -> Range {
        Range {
            anchor: Position {
                line: anchor.0,
                character: anchor.1,
            },
            head: Position {
                line: head.0,
                character: head.1,
            },
        }
    }

    pub fn ed_delta_single(
        anchor: (usize, usize),
        head: (usize, usize),
        replacement: &str,
    ) -> EditorTextDelta {
        EditorTextDelta(vec![replace_ed(anchor, head, replacement)])
    }

    pub fn replace_ed(
        anchor: (usize, usize),
        head: (usize, usize),
        replacement: &str,
    ) -> EditorTextOp {
        EditorTextOp {
            range: range(anchor, head),
            replacement: replacement.to_string(),
        }
    }

    pub fn rev_ed_delta_single(
        revision: usize,
        anchor: (usize, usize),
        head: (usize, usize),
        replacement: &str,
    ) -> RevisionedEditorTextDelta {
        rev_ed_delta(revision, ed_delta_single(anchor, head, replacement))
    }
}

#[cfg(test)]
mod tests {
    use super::factories::*;
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn compose_with_empty() {
        let empty = TextDelta::default();

        let mut other = TextDelta::default();
        other.retain(5);
        other.delete(1);
        other.insert("\nhello\n");

        // Note: Ordering of delete and insert is not important, as it leads to the same result.
        // Here, the operational-transform library returns the delete after the insert
        let mut expected_result = TextDelta::default();
        expected_result.retain(5);
        expected_result.insert("\nhello\n");
        expected_result.delete(1);

        assert_eq!(empty.compose(other.clone()), expected_result);
    }

    #[test]
    fn range_forward() {
        assert!(range((0, 0), (0, 1)).is_forward());
        assert!(range((0, 1), (1, 0)).is_forward());
        assert!(!range((0, 1), (0, 0)).is_forward());
        assert!(!range((1, 0), (0, 1)).is_forward());
    }

    #[test]
    fn conversion_editor_to_text_delta_insert() {
        let ed_delta = ed_delta_single((0, 1), (0, 1), "a");
        let delta = TextDelta::from_ed_delta(ed_delta, "foo");
        assert_eq!(delta, insert(1, "a"));
    }

    #[test]
    fn conversion_editor_to_text_delta_delete() {
        let ed_delta = ed_delta_single((0, 0), (0, 1), "");
        let delta = TextDelta::from_ed_delta(ed_delta, "foo");
        assert_eq!(delta, delete(0, 1));
    }

    #[test]
    fn conversion_editor_to_text_delta_replacement() {
        let ed_delta = ed_delta_single((0, 5), (1, 0), "\nhello\n");
        let delta = TextDelta::from_ed_delta(ed_delta, "hello\n");
        let mut expected_delta = TextDelta::default();
        expected_delta.retain(5);
        expected_delta.insert("\nhello\n");
        expected_delta.delete(1);
        assert_eq!(expected_delta, delta);
    }

    #[test]
    fn conversion_editor_to_text_delta_full_line_deletion() {
        let ed_delta = ed_delta_single((0, 0), (1, 0), "");
        let delta = TextDelta::from_ed_delta(ed_delta, "a\n");
        let mut expected_delta = TextDelta::default();
        expected_delta.delete(2);
        assert_eq!(expected_delta, delta);
    }

    #[test]
    #[should_panic]
    fn conversion_editor_to_text_delta_full_line_deletion_fails() {
        let ed_delta = ed_delta_single((0, 0), (1, 0), "");
        TextDelta::from_ed_delta(ed_delta, "a");
    }

    #[test]
    fn conversion_editor_to_text_delta_multiline_replacement() {
        let ed_delta = ed_delta_single((1, 0), (2, 0), "xzwei\nx");
        let delta = TextDelta::from_ed_delta(ed_delta, "xeins\nzwei\ndrei\n");
        let mut expected_delta = TextDelta::default();
        expected_delta.retain(6);
        expected_delta.insert("xzwei\nx");
        expected_delta.delete(5);
        assert_eq!(expected_delta, delta);
    }

    #[test]
    fn conversion_text_delta_to_editor_delta_multiline_replacement() {
        let content = "xeins\nzwei\ndrei\n";

        let mut delta = TextDelta::default();
        delta.retain(6);
        delta.insert("xzwei\nx");
        delta.delete(5);

        let ed_delta = EditorTextDelta::from_delta(delta, content);

        let expected_ed_delta = EditorTextDelta(vec![
            replace_ed((1, 0), (1, 0), "xzwei\nx"),
            replace_ed((1, 0), (2, 0), ""),
        ]);
        assert_eq!(ed_delta, expected_ed_delta);
    }

    #[test]
    fn conversion_text_delta_to_editor_delta_replacement() {
        let content = "blubb\n";

        let mut delta = TextDelta::default();
        delta.retain(5);
        delta.insert("\nhello\n");
        delta.delete(1);

        let ed_delta = EditorTextDelta::from_delta(delta, content);

        let expected_ed_delta = EditorTextDelta(vec![
            replace_ed((0, 5), (0, 5), "\nhello\n"),
            replace_ed((0, 5), (1, 0), ""),
        ]);

        assert_eq!(expected_ed_delta, ed_delta);
    }

    mod position {
        use super::Position;

        #[test]
        fn zero_offset() {
            assert_eq!(
                //       position         0123456 78901 2345
                //       character        0123456 01234 0124
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
            assert_eq!(
                Position {
                    line: 0,
                    character: 6
                }
                .to_offset("h🥕llo,\nneue\nwelt"),
                6
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
                Position::from_offset(11, "h🥕llo,\nneue\nwelt"),
                Position {
                    line: 1,
                    character: 4
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
        fn referencing_after_last_lines() {
            assert_eq!(
                Position {
                    line: 1,
                    character: 0
                },
                Position::from_offset(2, "a\n")
            );

            assert_eq!(
                Position {
                    line: 1,
                    character: 0
                }
                .to_offset("a\n"),
                2
            );
        }

        #[test]
        #[should_panic]
        fn referencing_after_last_line_fails() {
            Position {
                line: 1,
                character: 0,
            }
            .to_offset("a");
        }

        #[test]
        #[should_panic]
        fn offset_after_end_fails() {
            Position::from_offset(2, "a");
        }

        #[test]
        #[should_panic]
        fn referencing_two_lines_after_last_line_fails() {
            assert_eq!(
                Position {
                    line: 2,
                    character: 0
                }
                .to_offset("a"),
                1
            );
        }

        #[test]
        #[should_panic]
        fn offset_out_of_bounds_from_offset() {
            Position::from_offset(17, "h🥕llo,\nneue\nwelt");
        }

        #[test]
        #[should_panic]
        fn line_too_short() {
            Position {
                line: 1,
                character: 5,
            }
            .to_offset("h🥕llo\nwelt");
        }
    }
}
