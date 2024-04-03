#![allow(dead_code)]
use operational_transform::{Operation as OTOperation, OperationSeq};

//TODO: change Position to usize alias
#[derive(Debug, Clone, PartialEq, Eq)]
struct Position {
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Range {
    anchor: Position,
    head: Position,
}

impl Range {
    fn empty(&self) -> bool {
        self.anchor == self.head
    }

    fn forward(&self) -> bool {
        (self.anchor.line < self.head.line)
            || (self.anchor.line == self.head.line && self.anchor.column <= self.head.column)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpElement {
    range: Range,
    replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Op {
    v: Vec<OpElement>,
}
// TODO: or should we do it as a TupleStruct?
// struct Op(Vec<OpElement>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct RevisionedOp {
    revision: usize,
    op: Op,
}

impl Op {
    fn len(&self) -> usize {
        self.v.len()
    }

    fn from(opseq: &OperationSeq) -> Self {
        let mut v = vec![];
        let mut position = 0;
        for op in opseq.ops().iter() {
            match op {
                OTOperation::Retain(n) => position += n,
                OTOperation::Delete(n) => {
                    v.push(OpElement {
                        range: Range {
                            anchor: Position {
                                line: 0,
                                column: position as usize,
                            },
                            head: Position {
                                line: 0,
                                column: (position + n) as usize,
                            },
                        },
                        replacement: "".to_string(),
                    });
                }
                OTOperation::Insert(s) => {
                    v.push(OpElement {
                        range: Range {
                            anchor: Position {
                                line: 0,
                                column: position as usize,
                            },
                            head: Position {
                                line: 0,
                                column: position as usize,
                            },
                        },
                        replacement: s.to_string(),
                    });
                    position += s.len() as u64;
                }
            }
        }

        Self { v }
    }

    fn to_ot(&self) -> OperationSeq {
        let mut opseq = OperationSeq::default();
        for op in self.v.iter() {
            let mut opseq_sub = OperationSeq::default();
            assert!(op.range.anchor.line == 0, "TODO: support lines.");
            if op.replacement != "" {
                if op.range.empty() {
                    // insert
                    opseq_sub.retain(op.range.anchor.column as u64);
                    opseq_sub.insert(&op.replacement);
                } else {
                    // replace.
                    todo!()
                }
            } else {
                if !op.range.empty() {
                    // delete
                    let from;
                    let to;
                    if op.range.forward() {
                        from = op.range.anchor.column;
                        to = op.range.head.column;
                    } else {
                        from = op.range.head.column;
                        to = op.range.anchor.column;
                    }
                    opseq_sub.retain(from as u64);
                    opseq_sub.delete((to - from) as u64);
                } // else: no-op.
            }
            // TODO: I *think* the other way around can't happen? Not sure tho...
            if opseq.target_len() < opseq_sub.base_len() {
                opseq.retain((opseq_sub.base_len() - opseq.target_len()) as u64);
            }
            opseq = opseq.compose(&opseq_sub).unwrap();
        }
        opseq
    }
}

#[derive(Debug, Default)]
struct OTServer {
    editor_revision: usize,
    daemon_revision: usize,
    /// "Source of truth" operations.
    operations: Vec<Op>,
    /// Operations that we have sent to the editor, but we're not sure whether it has
    /// accepted them. We have to keep them around until we know for sure, so that we
    /// can correctly transform operations for the editor.
    ///
    /// Design Note: The daemon should do the transformation because we want to spare
    /// the overhead of implementing the tranformation per editor plugin. In our case
    /// there's a small number of editors, so transforming it in the daemon is feasible.
    ///
    /// TODO: Could this just be a single (combined) Op as well? Given that we can have many
    /// OpElements in an Op.
    editor_queue: Vec<Op>,
    /// For debugging purposes we keep track of the string that would result from the given operations.
    document: String,
}

/*
 TODO: Rewrite the below properly (I copied it from node daemon), then turn into Docstring.

    This class receive operations from both the CRDT world, and one editor.
    It will make sure to send the correct operations back to them using the provided callbacks.

    Here's an example of how it works:

    1. The daemon starts with an empty document.
    2. It applies the d1 operation to it, which the editor also receives and applies.
    3. The daemon applies d2 and d3, and sends them to the editor (thinking these would put it
       into the same state).
       It also sends along the editor revision, which the number of ops received by the editor,
       which is basically the column, and specifies the point the ops apply to. (Here: 0).
    4. But the editor has made concurrent edits e1 and e2 in the meantime. It rejects d2 and d3.
       It sends e1 and e2 to the daemon, along with the daemon revision, which is the number of ops
       it has received from the daemon (the row).
    5. The daemon transforms e1 and e2 through d2 and d3, creating e1' and e2', and applies them
       to the document. It sends d2' and d3' to the editor, along with the editor revision (2).
    6. The editor receives d2' and applies it, but then makes edit e3, and sends it to the daemon.
       The editor rejects d3', because it is received after e3 was created.
    7. The daemon meanwhile makes edit d4. Upon reciving e3, it transforms it against d3' and d4,
       and sends d3'' and d4' to the editor. It applies d4 and e3'' to the document.
    8. The editor receives d3'' and d4', and applies them. Both sides now have the same document.


     ---- the right axis is the editor revision --->
    |
    | the down axis is the daemon revision
    |
    v

        *
        |
     d1 |
        v  e1      e2
        * ----> * ----> *
        |       |       |
     d2 |       |       | d2'
        v       v       v  e3
        * ----> * ----> * ----> *          Ops in the rightmost column need
        |       |       |       |          to be queued by us, because
     d3 |       |       | d3'   | d3''     we don't know whether the
        v  e1'  v  e2'  v  e3'  v          editor accepted them. (d3'' and d4')
        * ----> * ----> * ----> *
                        |       |
    The lower        d4 |       | d4'
    zig-zag is          v  e3'' v
    the operations      * ----> *
    log saved by the
    daemon.
    (d1, d2, d3, e1', e2', d4, e3'')

*/
impl OTServer {
    fn new() -> Self {
        Default::default()
    }

    fn new_with_doc(document: &str) -> Self {
        Self {
            document: document.to_string(),
            ..Default::default()
        }
    }

    fn apply_change_to_document(&mut self, op: &Op) {
        let mut ot_op = op.to_ot();
        if ot_op.base_len() < self.document.len() {
            ot_op.retain((self.document.len() - ot_op.base_len()) as u64);
        }
        self.document = ot_op.apply(&self.document).unwrap();
    }

    /// Called when the CRDT world makes a change to the document.
    fn apply_crdt_change(&mut self, op: Op) -> RevisionedOp {
        // We can apply the change immediately.
        self.operations.push(op.clone());
        self.editor_queue.push(op.clone());
        self.daemon_revision += 1;
        self.apply_change_to_document(&op);

        // We assume that the editor is up-to-date, and send the operation to it.
        // If it can't accept it, we will transform and send it later.
        RevisionedOp {
            revision: self.editor_revision,
            op,
        }
    }

    fn add_editor_operation(&mut self, op: &Op) {
        self.operations.push(op.clone());
        self.editor_revision += 1;
        self.apply_change_to_document(op)
    }

    /// Called when the editor sends us an operation.
    /// daemonRevision is the revision this operation applies to.
    fn apply_editor_operation(
        &mut self,
        daemon_revision: usize,
        mut op: Op,
    ) -> (Op, Vec<RevisionedOp>) {
        let mut to_editor = vec![];
        if daemon_revision > self.daemon_revision {
            panic!("must not happen, editor has seen a daemon revision from the future.");
        } else if daemon_revision == self.daemon_revision {
            // The sent operation applies to the current daemon revision. We can apply it immediately.
            self.add_editor_operation(&op);
        } else {
            // The operation applies to an older daemon revision.
            // We need to transform it through the daemon operations that have happened since then.

            // But we at least we know that the editor has seen all daemon operations until
            // daemon_revision. So we can remove them from the editor queue.
            let daemon_operations_to_transform = self.daemon_revision - daemon_revision;
            assert!(
                self.editor_queue.len() >= daemon_operations_to_transform,
                "Whoopsie, we don't have enough operations cached. Was this already processed?"
            );
            let seen_operations = self.editor_queue.len() - daemon_operations_to_transform;
            // TODO: should we use split_off instead and drop the first one?
            // What is the most efficient+readable way to cut off the first elements?
            self.editor_queue = self.editor_queue[seen_operations..].to_vec();

            (op, self.editor_queue) = transform_through_operations(op, &self.editor_queue);
            // Apply the transformed operation to the document.
            self.add_editor_operation(dbg!(&op));

            for editor_op in self.editor_queue.iter() {
                to_editor.push(RevisionedOp {
                    revision: self.editor_revision,
                    op: editor_op.clone(),
                });
            }
            /*
            // Send the transformed queue to the editor.
            for (let op of this.editorQueue) {
                this.sendToEditor(this.editorRevision, op)
            }
            */
        }
        (op, to_editor)
    }
}

/// This function takes operations t1 and m1 ... m_n,
/// and returns operations t1' and m1' ... m_n'.
///
///        t1
///     * ----> *
///     |       |
///  m1 |       | m1'
///     v       v
///     * ----> *
///     |       |
///  m2 |       | m2'
///     v       v
///     * ----> *
///     |       |
///  m3 |       | m3'
///     v  t1'  v
///     * ----> *
///
fn transform_through_operations(mut their_operation: Op, my_operations: &Vec<Op>) -> (Op, Vec<Op>) {
    let mut transformed_my_operations = vec![];
    for my_operation in my_operations.iter() {
        assert!(
            my_operation.len() == 1,
            "TODO: support operations that have more than one range"
        );
        let mut my_opseq = my_operation.to_ot();
        let mut their_opseq = their_operation.to_ot();
        dbg!(&my_opseq);
        dbg!(&their_opseq);
        // transform expects both operations to have the same base_len. See also:
        // https://docs.rs/operational-transform/0.6.1/src/operational_transform/lib.rs.html#345
        // Currently we are implementing this method on data that doesn't carry this 'global' knowledge.
        // So we'll workaround by manually fixing the base_len, if one of the operations is shorter.
        // We do so by simply retaining the required number of characters at the end
        if my_opseq.base_len() < their_opseq.base_len() {
            let diff = their_opseq.base_len() - my_opseq.base_len();
            my_opseq.retain(diff as u64);
        } else {
            let diff = my_opseq.base_len() - their_opseq.base_len();
            their_opseq.retain(diff as u64);
        }
        dbg!(&my_opseq);
        dbg!(&their_opseq);
        let (my_prime, their_prime) = my_opseq.transform(&their_opseq).unwrap();
        dbg!(&my_prime);
        dbg!(&their_prime);
        transformed_my_operations.push(Op::from(&my_prime));
        dbg!(&transformed_my_operations);
        their_operation = Op::from(&their_prime);
        dbg!(&their_operation);
    }
    // TODO: something like `self = their_operation;` doesn't work, does it?
    (their_operation, transformed_my_operations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert(at: usize, s: &str) -> Op {
        Op {
            v: vec![OpElement {
                range: Range {
                    anchor: Position {
                        line: 0,
                        column: at,
                    },
                    head: Position {
                        line: 0,
                        column: at,
                    },
                },
                replacement: s.to_owned(),
            }],
        }
    }

    fn dummy_insert(at: usize) -> Op {
        insert(at, "foo")
    }

    fn rev_op(rev: usize, op: Op) -> RevisionedOp {
        RevisionedOp { revision: rev, op }
    }

    fn delete(from: usize, length: usize) -> Op {
        delete_range(from, from + length)
    }

    fn compose(op1: Op, op2: Op) -> Op {
        Op {
            v: [op1.v, op2.v].concat(),
        }
    }

    fn delete_range(from: usize, to: usize) -> Op {
        Op {
            v: vec![OpElement {
                range: Range {
                    anchor: Position {
                        line: 0,
                        column: from,
                    },
                    head: Position {
                        line: 0,
                        column: to,
                    },
                },
                replacement: "".to_string(),
            }],
        }
    }

    #[test]
    fn crdt_change_increases_revision() {
        let mut ot_server = OTServer::new_with_doc("he");
        ot_server.apply_crdt_change(dummy_insert(2));
        assert_eq!(ot_server.daemon_revision, 1);
        assert_eq!(ot_server.editor_revision, 0);
    }

    #[test]
    fn crdt_change_tracks_in_queue() {
        let mut ot_server = OTServer::new_with_doc("he");
        let op = dummy_insert(2);
        ot_server.apply_crdt_change(op);
        assert_eq!(ot_server.editor_queue.len(), 1);
        // assert_eq!(ot_server.editor_queue[0], op); // How to compare?
    }

    #[test]
    fn routes_operations_through_server() {
        let mut ot_server = OTServer::new_with_doc("hello");

        let to_editor = ot_server.apply_crdt_change(insert(1, "x"));
        assert_eq!(to_editor, rev_op(0, insert(1, "x")));

        let (to_crdt, to_editor) = ot_server.apply_editor_operation(0, insert(2, "y"));
        assert_eq!(to_crdt, insert(3, "y"));
        assert_eq!(to_editor, vec![rev_op(1, insert(1, "x"))]);

        assert_eq!(ot_server.operations, vec![insert(1, "x"), insert(3, "y")]);
        assert_eq!(ot_server.document, "hxeyllo");

        let to_editor = ot_server.apply_crdt_change(insert(3, "z"));
        assert_eq!(to_editor, rev_op(1, insert(3, "z")));

        assert_eq!(ot_server.document, "hxezyllo");

        // editor thinks: hxeyllo -> hlo
        let (to_crdt, to_editor) = ot_server.apply_editor_operation(1, delete(1, 4));
        assert_eq!(to_crdt, compose(delete(1, 2), delete(2, 2)));
        assert_eq!(to_editor, vec![rev_op(2, insert(1, "z"))]);

        assert_eq!(ot_server.document, "hzlo");
        assert_eq!(
            ot_server.operations,
            vec![
                insert(1, "x"),
                insert(3, "y"),
                insert(3, "z"),
                compose(delete(1, 2), delete(2, 2))
            ]
        );
    }

    #[test]
    fn editor_operation_reduces_editor_queue() {
        let mut ot_server = OTServer::new_with_doc("he");

        ot_server.apply_crdt_change(dummy_insert(2));
        ot_server.apply_crdt_change(dummy_insert(5));
        ot_server.apply_crdt_change(dummy_insert(8));
        assert_eq!(ot_server.editor_queue.len(), 3);

        ot_server.apply_editor_operation(1, dummy_insert(2));
        // // we have already seen one op, so now the queue has only 2 left.
        // assert_eq!(ot_server.editor_queue.len(), 2);
    }

    #[test]
    fn conversion_from_ot_to_us() {
        // retain + insert
        let mut a = OperationSeq::default();
        a.retain(3);
        a.insert("foobar");
        // dbg!(&a);
        let ours = Op::from(&a);
        assert_eq!(ours.len(), 1);
        assert_eq!(ours.v[0].range.anchor.column, 3);

        // insert + retain
        let mut a = OperationSeq::default();
        a.insert("foobar");
        a.retain(3);
        // dbg!(&a);
        let ours = Op::from(&a);
        dbg!(&ours);
        assert_eq!(ours.len(), 1);
        assert_eq!(ours.v[0].range.anchor.column, 0);

        // retain + delete
        let mut a = OperationSeq::default();
        a.retain(3);
        a.delete(3);
        // dbg!(&a);
        let ours = Op::from(&a);
        assert_eq!(ours.len(), 1);
        assert_eq!(ours.v[0].range.anchor.column, 3);
        assert_eq!(ours.v[0].range.head.column, 6);

        // retain + delete + retain + delete
        let mut a = OperationSeq::default();
        a.retain(1);
        a.delete(1);
        a.retain(1);
        a.delete(2);
        let ours = Op::from(&a);
        assert_eq!(ours.len(), 2);
        dbg!(&ours);
        // dbg!(Op {
        //     v: vec![ours.v[0].clone()]
        // }
        // .to_ot());
        // dbg!(Op {
        //     v: vec![ours.v[1].clone()]
        // }
        // .to_ot());
        // dbg!(&ours.to_ot());
        // assert_eq!(&ours.v[0]
    }

    #[test]
    fn conversion_from_us_to_ot_insert() {
        let ours = dummy_insert(2);
        let ot = ours.to_ot();

        // dbg!(&ot);
        let ops = ot.ops();
        assert_eq!(ops.len(), 2);
        assert_eq!(OTOperation::Retain(2), ops[0]);
        assert_eq!(OTOperation::Insert("foo".to_string()), ops[1]);
    }

    #[test]
    fn conversion_from_us_to_ot_delete() {
        let ours = delete_range(2, 4);
        let ot = ours.to_ot();

        // dbg!(&ot);
        let ops = ot.ops();
        assert_eq!(ops.len(), 2);
        assert_eq!(OTOperation::Retain(2), ops[0]);
        assert_eq!(OTOperation::Delete(2), ops[1]);
    }

    #[test]
    fn transforms_operation_correctly() {
        let ours = vec![dummy_insert(0), dummy_insert(3)];
        let mut theirs = dummy_insert(0);
        theirs.v[0].replacement = "bar".to_string();
        let (theirs, ours_prime) = transform_through_operations(theirs, &ours);
        assert_eq!(theirs.len(), 1);
        // position of the insert has shifted after ours
        assert_eq!(theirs.v[0].range.anchor.column, 6);
        assert_eq!(ours_prime.len(), 2);
        // check that ours hasn't changed
        let mut ours_it = ours.iter();
        for op_prime in ours_prime.iter() {
            assert_eq!(op_prime.len(), 1);
            let op_prime = &op_prime.v[0];
            let op = &ours_it.next().unwrap().v[0];
            assert_eq!(op.range.anchor, op_prime.range.anchor);
            assert_eq!(op.range.head, op_prime.range.head);
            assert_eq!(op.replacement, op_prime.replacement);
        }
    }

    #[test]
    fn transforms_operation_correctly_different_base_lengths() {
        let ours = vec![dummy_insert(3)];
        let mut theirs = dummy_insert(0);
        theirs.v[0].replacement = "bar".to_string();
        let (theirs, ours_prime) = transform_through_operations(theirs, &ours);
        assert_eq!(theirs.len(), 1);
        // position of the insert hasn't shifted.
        assert_eq!(theirs.v[0].range.anchor.column, 0);
        assert_eq!(ours_prime.len(), 1);
        assert_eq!(ours_prime[0].len(), 1);
        assert_eq!(ours_prime[0].v[0].range.anchor.column, 6);
    }

    #[test]
    fn transforms_operation_correctly_splits_deletion() {
        let editor_op = insert(2, "x");
        let unacknowledged_ops = vec![delete(1, 3)];

        let (op_prime, queue_prime) = transform_through_operations(editor_op, &unacknowledged_ops);
        assert_eq!(op_prime, insert(1, "x"));
        assert_eq!(queue_prime, vec![compose(delete(1, 1), delete(2, 2))]);
    }

    #[test]
    fn ot_transform_does_what_we_think() {
        let mut a = OperationSeq::default();
        let mut b = OperationSeq::default();
        let mut c = OperationSeq::default();

        a.retain(2);
        a.insert("x");
        a.retain(1);

        b.retain(1);
        b.delete(2);

        // similar to a, but other character.
        c.retain(2);
        c.insert("y");
        c.retain(1);

        let (a_prime, b_prime) = a.transform(&b).unwrap();
        assert_eq!(
            a_prime.ops(),
            vec![OTOperation::Retain(1), OTOperation::Insert("x".to_string())]
        );
        assert_eq!(
            b_prime.ops(),
            vec![
                OTOperation::Retain(1),
                OTOperation::Delete(1),
                OTOperation::Retain(1),
                OTOperation::Delete(1)
            ]
        );

        // With inserts at the same position,
        // the operation that is transformed is applied "after" the other one.
        // If you want it the other way around, you'll need to swap a and c.
        let (a_prime, c_prime) = a.transform(&c).unwrap();
        assert_eq!(
            a_prime.ops(),
            vec![
                OTOperation::Retain(2),
                OTOperation::Insert("x".to_string()),
                OTOperation::Retain(2)
            ]
        );
        assert_eq!(
            c_prime.ops(),
            vec![
                OTOperation::Retain(3),
                OTOperation::Insert("y".to_string()),
                OTOperation::Retain(1)
            ]
        );
    }
}
