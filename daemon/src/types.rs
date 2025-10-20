// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::path::RelativePath;
use automerge::{patches::TextRepresentation, ConcreteTextValue, Patch, PatchAction, TextEncoding};
use dissimilar::Chunk;
use operational_transform::{Operation as OTOperation, OperationSeq};
use ropey::Rope;
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{debug, warn};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextDelta(pub Vec<TextOp>);

impl fmt::Display for TextDelta {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let op_strings: Vec<String> = self.0.iter().map(|op| format!("{op}")).collect();
        write!(f, "[{}]", op_strings.join(", "))
    }
}

impl IntoIterator for TextDelta {
    type Item = TextOp;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextOp {
    Retain(usize),
    Insert(String),
    Delete(usize),
}

impl fmt::Display for TextOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Retain(n) => write!(f, "{n}"),
            Self::Insert(str) => write!(f, "\"{str}\""),
            Self::Delete(n) => write!(f, "-{n}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorTextDelta(pub Vec<EditorTextOp>);

impl IntoIterator for EditorTextDelta {
    type Item = EditorTextOp;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionedTextDelta {
    pub revision: usize,
    pub delta: TextDelta,
}

impl RevisionedTextDelta {
    #[must_use]
    pub fn new(revision: usize, delta: TextDelta) -> Self {
        Self { revision, delta }
    }

    pub fn from_rev_ed_delta(rev_ed_delta: RevisionedEditorTextDelta, content: &str) -> Self {
        Self::new(
            rev_ed_delta.revision,
            TextDelta::from_ed_delta(rev_ed_delta.delta, content),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTextDelta {
    pub file_path: RelativePath,
    pub delta: TextDelta,
}

impl FileTextDelta {
    #[must_use]
    pub fn new(file_path: RelativePath, delta: TextDelta) -> Self {
        Self { file_path, delta }
    }
}

pub type CursorId = String;

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct CursorState {
    pub name: Option<String>,
    pub file_path: RelativePath,
    pub ranges: Vec<Range>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct EphemeralMessage {
    pub cursor_id: CursorId,
    pub sequence_number: usize,
    pub cursor_state: CursorState,
}

#[derive(Debug)]
pub enum PatchEffect {
    FileChange(FileTextDelta),
    FileRemoval(RelativePath),
    /// Emitted when a binary file's content is set.
    FileBytes(RelativePath, Vec<u8>),
    NoEffect,
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
                    panic!("Failed to convert patch to delta: {e:#?}");
                }
            }
        }

        file_deltas
    }
}

/// These messages are "internally" passed between the components that the daemon consists of -
/// namely, the connected editors and the CRDT document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentMessage {
    Open {
        file_path: RelativePath,
        content: String,
    },
    Close {
        file_path: RelativePath,
    },
    Edit {
        file_path: RelativePath,
        delta: TextDelta,
    },
    Cursor {
        cursor_id: CursorId,
        cursor_state: CursorState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorTextOp {
    pub range: Range,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    #[must_use]
    pub fn is_forward(&self) -> bool {
        (self.start.line < self.end.line)
            || (self.start.line == self.end.line && self.start.character <= self.end.character)
    }

    #[must_use]
    pub fn as_relative(&self, content: &str) -> (usize, usize) {
        let start_offset = self.start.to_offset(content);
        let end_offset = self.end.to_offset(content);
        if self.is_forward() {
            (start_offset, end_offset - start_offset)
        } else {
            (end_offset, start_offset - end_offset)
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

// TODO: This feels like it should go into another file, close to where Document handles writing to
// the Automerge document. Both places need to know about our chosen structure.
impl TryFrom<Patch> for PatchEffect {
    type Error = anyhow::Error;

    fn try_from(patch: Patch) -> Result<Self, Self::Error> {
        fn file_path_from_path_default(
            path: &[(automerge::ObjId, automerge::Prop)],
        ) -> Result<RelativePath, anyhow::Error> {
            if path.len() != 2 {
                return Err(anyhow::anyhow!(
                    "Unexpected path in Automerge patch, length is not 2"
                ));
            }
            let (_obj_id, prop) = &path[1];
            if let automerge::Prop::Map(file_path) = prop {
                return Ok(RelativePath::new(file_path));
            }
            Err(anyhow::anyhow!(
                "Unexpected path in Automerge patch: Prop is not a map"
            ))
        }

        if patch.path.is_empty() {
            return match patch.action {
                PatchAction::PutMap { key, .. } => {
                    if key == "files" {
                        Ok(Self::NoEffect)
                    } else {
                        Err(anyhow::anyhow!(
                            "Path is empty and action is PutMap, but key is not 'files'",
                        ))
                    }
                }
                other_action => Err(anyhow::anyhow!(
                    "Unsupported patch action for empty path: {}",
                    other_action
                )),
            };
        }

        match &patch.path[0] {
            (_, automerge::Prop::Map(key)) if key == "files" => {
                if patch.path.len() == 1 {
                    match patch.action {
                        PatchAction::PutMap {
                            key,
                            conflict,
                            value,
                        } => {
                            // This action happens when a new file is created.

                            let relative_path = RelativePath::new(&key);

                            match value {
                                (automerge::Value::Object(automerge::ObjType::Text), _) => {
                                    if conflict {
                                        // In this case, the peer receiving this PutMap should
                                        // remove all existing content of this file in open
                                        // editors. So we emit a FileRemovel.
                                        warn!("Resolved conflict for file {relative_path} by overwriting your version.");
                                        Ok(Self::FileRemoval(relative_path))
                                    } else {
                                        // We return an empty delta on the new file, so that the file is created on disk when
                                        // synced over to another peer. TODO: Is this the best way to solve this?
                                        Ok(Self::FileChange(FileTextDelta::new(
                                            relative_path,
                                            TextDelta::default(),
                                        )))
                                    }
                                }
                                (
                                    automerge::Value::Scalar(std::borrow::Cow::Owned(
                                        automerge::ScalarValue::Bytes(bytes),
                                    )),
                                    _,
                                ) => Ok(Self::FileBytes(relative_path, bytes)),
                                _ => {
                                    Err(anyhow::anyhow!("Unexpected value in path {relative_path}"))
                                }
                            }
                        }
                        PatchAction::DeleteMap { key } => {
                            // This action happens when a file is deleted.
                            debug!("Got file removal from patch: {key}");
                            Ok(Self::FileRemoval(RelativePath::new(&key)))
                        }
                        PatchAction::Conflict { prop } => {
                            // This can happen when both sides create the same file.
                            match prop {
                                automerge::Prop::Map(file_name) => {
                                    // We assume that conflict resolution works the way, that the
                                    // side that gets the PatchAction is the one that "wins".
                                    warn!("Conflict for file '{file_name}' resolved. Taking your version.");
                                    Ok(Self::NoEffect)
                                }
                                automerge::Prop::Seq(seq) => Err(anyhow::anyhow!(
                                    "Got a Seq-type prop as a conflict, expected Map: {seq}"
                                )),
                            }
                        }
                        other_action => Err(anyhow::anyhow!(
                            "Unsupported patch action for path 'files': {other_action}"
                        )),
                    }
                } else if patch.path.len() == 2 {
                    let mut delta = TextDelta::default();
                    match patch.action {
                        PatchAction::SpliceText { index, value, .. } => {
                            delta.retain(index);
                            delta.insert(&value.make_string());
                            Ok(Self::FileChange(FileTextDelta::new(
                                file_path_from_path_default(&patch.path)?,
                                delta,
                            )))
                        }
                        PatchAction::DeleteSeq { index, length } => {
                            delta.retain(index);
                            delta.delete(length);
                            Ok(Self::FileChange(FileTextDelta::new(
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
            (_, _) => Err(anyhow::anyhow!(
                "Unexpected path in Automerge patch, expected it to begin with 'files'"
            )),
        }
    }
}

impl From<TextDelta> for Vec<PatchAction> {
    fn from(delta: TextDelta) -> Self {
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
                        value: ConcreteTextValue::new(
                            &s,
                            TextRepresentation::String(TextEncoding::default()),
                        ),
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
        let mut delta = Self::default();
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
    fn from(delta: TextDelta) -> Self {
        let mut op_seq = Self::default();
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
        let mut delta = Self::default();
        // TODO: add support, when needed
        assert!(
            ed_delta.0.len() <= 1,
            "We don't yet support EditorTextDelta with multiple operations."
        );
        for ed_op in ed_delta {
            let mut delta_step = Self::default();
            if ed_op.range.is_empty() {
                if !ed_op.replacement.is_empty() {
                    // insert
                    delta_step.retain(ed_op.range.start.to_offset(content));
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

impl From<Vec<Chunk<'_>>> for TextDelta {
    fn from(chunks: Vec<Chunk>) -> Self {
        let mut delta = Self::default();
        for chunk in chunks {
            match chunk {
                Chunk::Equal(s) => {
                    delta.retain(s.chars().count());
                }
                Chunk::Delete(s) => {
                    delta.delete(s.chars().count());
                }
                Chunk::Insert(s) => {
                    delta.insert(s);
                }
            }
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
                            start: Position::from_offset(position, content),
                            end: Position::from_offset(position + n, content),
                        },
                        replacement: String::new(),
                    });
                    position += n;
                }
                TextOp::Insert(s) => {
                    editor_ops.push(EditorTextOp {
                        range: Range {
                            start: Position::from_offset(position, content),
                            end: Position::from_offset(position, content),
                        },
                        replacement: s.clone(),
                    });
                }
            }
        }

        Self(editor_ops)
    }
}

pub mod factories {
    use super::{
        EditorTextDelta, EditorTextOp, Position, Range, RevisionedEditorTextDelta,
        RevisionedTextDelta, TextDelta,
    };

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

    pub fn range(start: (usize, usize), end: (usize, usize)) -> Range {
        Range {
            start: Position {
                line: start.0,
                character: start.1,
            },
            end: Position {
                line: end.0,
                character: end.1,
            },
        }
    }

    pub fn ed_delta_single(
        start: (usize, usize),
        end: (usize, usize),
        replacement: &str,
    ) -> EditorTextDelta {
        EditorTextDelta(vec![replace_ed(start, end, replacement)])
    }

    pub fn replace_ed(
        start: (usize, usize),
        end: (usize, usize),
        replacement: &str,
    ) -> EditorTextOp {
        EditorTextOp {
            range: range(start, end),
            replacement: replacement.to_string(),
        }
    }

    pub fn rev_ed_delta_single(
        revision: usize,
        start: (usize, usize),
        end: (usize, usize),
        replacement: &str,
    ) -> RevisionedEditorTextDelta {
        rev_ed_delta(revision, ed_delta_single(start, end, replacement))
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

    // Test conversion from the difference crate.
    mod dissimilar {
        use super::TextDelta;
        use dissimilar::diff;

        #[test]
        fn same() {
            let mut delta = TextDelta::default();
            delta.retain(6);

            assert_eq!(delta, diff("tö🥕s\nt", "tö🥕s\nt").into());
        }

        #[test]
        fn insertion() {
            let mut delta = TextDelta::default();
            delta.retain(3);
            delta.insert("ü");
            delta.retain(3);

            assert_eq!(delta, diff("tö🥕s\nt", "tö🥕üs\nt").into());
        }

        #[test]
        fn deletion() {
            let mut delta = TextDelta::default();
            delta.retain(2);
            delta.delete(1);
            delta.retain(3);

            assert_eq!(delta, diff("tö🥕s\nt", "tös\nt").into());
        }

        #[test]
        fn complex() {
            let mut delta = TextDelta::default();
            // word => werd
            delta.retain(1);
            delta.delete(1);
            delta.insert("e");

            // word => wordle
            delta.retain(7);
            delta.insert("le");

            // word => word
            delta.retain(6);

            // word => vorort
            delta.delete(1);
            delta.insert("vor");
            delta.retain(2);
            delta.delete(1);
            delta.insert("t");
            delta.retain(1);

            assert_eq!(
                delta,
                diff("word\nword\nword\nword\n", "werd\nwordle\nword\nvorort\n").into()
            );
        }
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
