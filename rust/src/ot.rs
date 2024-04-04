use crate::types::{RevisionedTextDelta, TextDelta};
use operational_transform::OperationSeq;
use std::cmp::Ordering;

///    OTServer receives operations from both the CRDT world, and one editor and makes sure that
///    the editor operations (which might be based on an older document) are applicable to the
///    state that the CRDT is tracking.
///
///    Here's a broader example of how it works, when employed in a context of a daemon (the
///    caller, which needs to make sure, that operations are communicated both ways).
///
///    1. The daemon starts with an empty list of operations (corresponding to an empty/unchanged document)
///    2. It applies the d1 operation to it, which the editor also receives and applies.
///    3. The daemon applies d2 and d3, and sends them to the editor (thinking these would put it
///       into the same state).
///       It also sends along the editor revision, which is the number of ops received by the editor,
///       which is basically the column, and specifies the point the ops apply to. (Here: 0).
///    4. But the editor has made concurrent edits e1 and e2 in the meantime. It rejects d2 and d3.
///       It sends e1 and e2 to the daemon, along with the daemon revision, which is the number of ops
///       it has received from the daemon (the row).
///    5. The daemon transforms e1 and e2 through d2 and d3, creating e1' and e2', and applies them
///       to the document. It sends d2' and d3' to the editor, along with the editor revision (2).
///    6. The editor receives d2' and applies it, but then makes edit e3, and sends it to the daemon.
///       The editor rejects d3', because it is received after e3 was created.
///    7. The daemon meanwhile makes edit d4. Upon reciving e3, it transforms it against d3' and d4,
///       and sends d3'' and d4' to the editor. It applies d4 and e3'' to the document.
///    8. The editor receives d3'' and d4', and applies them. Both sides now have the same document.
///
///
///     ---- the right axis is the editor revision --->
///    |
///    | the down axis is the daemon revision
///    |
///    v
///
///        *
///        |
///     d1 |
///        v  e1      e2
///        * ----> * ----> *
///        |       |       |
///     d2 |       |       | d2'
///        v       v       v  e3
///        * ----> * ----> * ----> *          Ops in the rightmost column need
///        |       |       |       |          to be queued by us, because
///     d3 |       |       | d3'   | d3''     we don't know whether the
///        v  e1'  v  e2'  v  e3'  v          editor accepted them. (d3'' and d4')
///        * ----> * ----> * ----> *
///                        |       |
///    The lower        d4 |       | d4'
///    zig-zag is          v  e3'' v
///    the operations      * ----> *
///    log saved by the
///    daemon.
///    (d1, d2, d3, e1', e2', d4, e3'')
///
///
#[derive(Debug, Default)]
pub struct OTServer {
    editor_revision: usize,
    daemon_revision: usize,
    /// "Source of truth" operations.
    operations: Vec<OperationSeq>,
    /// Operations that we have sent to the editor, but we're not sure whether it has
    /// accepted them. We have to keep them around until we know for sure, so that we
    /// can correctly transform operations for the editor.
    ///
    /// Design Note: The daemon should do the transformation because we want to spare
    /// the overhead of implementing the tranformation per editor plugin. In our case
    /// there's a small number of editors, so transforming it in the daemon is feasible.
    editor_queue: Vec<OperationSeq>,
}

#[allow(dead_code)] // TODO: remove, once OTServer is in use.
impl OTServer {
    /// Called when the CRDT world makes a change to the document.
    pub fn apply_crdt_change(&mut self, delta: TextDelta) -> RevisionedTextDelta {
        // We can apply the change immediately.
        self.operations.push(delta.clone().into());
        self.editor_queue.push(delta.clone().into());
        self.daemon_revision += 1;

        // We assume that the editor is up-to-date, and send the operation to it.
        // If it can't accept it, we will transform and send it later.
        RevisionedTextDelta {
            revision: self.editor_revision,
            delta,
        }
    }

    /// Called when the editor sends us an operation.
    /// daemonRevision is the revision this operation applies to.
    pub fn apply_editor_operation(
        &mut self,
        rev_delta: RevisionedTextDelta,
    ) -> (TextDelta, Vec<RevisionedTextDelta>) {
        let mut to_editor = vec![];
        let mut op_seq: OperationSeq = rev_delta.delta.into();
        let daemon_revision = rev_delta.revision;
        self.editor_revision += 1;
        match daemon_revision.cmp(&self.daemon_revision) {
            Ordering::Greater => {
                panic!("must not happen, editor has seen a daemon revision from the future.");
            }
            Ordering::Equal => {
                // The sent operation applies to the current daemon revision. We can apply it immediately.
            }
            Ordering::Less => {
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

                (op_seq, self.editor_queue) =
                    transform_through_operations(op_seq, &self.editor_queue);

                for editor_op in &self.editor_queue {
                    to_editor.push(RevisionedTextDelta::new(
                        self.editor_revision,
                        editor_op.clone().into(),
                    ));
                }
            }
        }
        self.operations.push(op_seq.clone());
        (op_seq.into(), to_editor)
    }

    pub fn apply_to_string(&mut self, mut document: String) -> String {
        for op_seq in &self.operations {
            let mut op_seq = op_seq.clone();
            if op_seq.base_len() < document.len() {
                op_seq.retain((document.len() - op_seq.base_len()) as u64);
            }
            document = op_seq.apply(&document).expect(&format!(
                "Could not apply operation expecting length {} to string with length {}.",
                op_seq.base_len(),
                document.len()
            ));
        }
        document
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
fn transform_through_operations(
    mut their_op_seq: OperationSeq,
    my_operations: &Vec<OperationSeq>,
) -> (OperationSeq, Vec<OperationSeq>) {
    let mut transformed_my_operations = vec![];
    for my_op_seq in my_operations {
        let mut my_op_seq = my_op_seq.clone();
        // transform expects both operations to have the same base_len. See also:
        // https://docs.rs/operational-transform/0.6.1/src/operational_transform/lib.rs.html#345
        // Currently we are implementing this method on data that doesn't carry this 'global' knowledge.
        // So we'll workaround by manually fixing the base_len, if one of the operations is shorter.
        // We do so by simply retaining the required number of characters at the end
        if my_op_seq.base_len() < their_op_seq.base_len() {
            let diff = their_op_seq.base_len() - my_op_seq.base_len();
            my_op_seq.retain(diff as u64);
        } else {
            let diff = my_op_seq.base_len() - their_op_seq.base_len();
            their_op_seq.retain(diff as u64);
        }
        let (my_prime, their_prime) = my_op_seq.transform(&their_op_seq).expect(&format!(
            "Could not transform operations {:?} on top of {:?}.",
            &their_op_seq.ops(),
            &my_op_seq.ops()
        ));
        transformed_my_operations.push(my_prime);
        their_op_seq = their_prime;
    }
    (their_op_seq, transformed_my_operations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::factories::*;

    fn compose(delta1: TextDelta, delta2: TextDelta) -> TextDelta {
        operational_transform_internals::ot_compose(delta1.into(), delta2.into()).into()
    }

    mod ot_server_public_interface {
        use super::*;

        #[test]
        fn routes_operations_through_server() {
            let mut ot_server: OTServer = Default::default();

            let to_editor = ot_server.apply_crdt_change(insert(1, "x"));
            assert_eq!(to_editor, rev_delta(0, insert(1, "x")));

            let (to_crdt, to_editor) =
                ot_server.apply_editor_operation(rev_delta(0, insert(2, "y")));
            assert_eq!(to_crdt, insert(3, "y"));
            let mut expected = insert(1, "x");
            expected.retain(2);
            assert_eq!(to_editor, vec![rev_delta(1, expected)]);

            assert_eq!(
                ot_server.operations,
                vec![insert(1, "x").into(), insert(3, "y").into()]
            );
            assert_eq!(ot_server.apply_to_string("hello".into()), "hxeyllo");

            let to_editor = ot_server.apply_crdt_change(insert(3, "z"));
            assert_eq!(to_editor, rev_delta(1, insert(3, "z")));

            assert_eq!(ot_server.apply_to_string("hello".into()), "hxezyllo");

            // editor thinks: hxeyllo -> hlo
            let (to_crdt, to_editor) = ot_server.apply_editor_operation(rev_delta(1, delete(1, 4)));
            assert_eq!(to_crdt, compose(delete(1, 2), delete(2, 2)));
            assert_eq!(to_editor, vec![rev_delta(2, insert(1, "z"))]);

            assert_eq!(ot_server.apply_to_string("hello".into()), "hzlo");
            assert_eq!(
                ot_server.operations,
                vec![
                    insert(1, "x").into(),
                    insert(3, "y").into(),
                    insert(3, "z").into(),
                    compose(delete(1, 2), delete(2, 2)).into()
                ]
            );
        }
    }

    mod ot_server_internal_state {
        use super::*;

        fn dummy_insert(at: usize) -> TextDelta {
            insert(at, "foo")
        }

        #[test]
        fn crdt_change_increases_revision() {
            let mut ot_server: OTServer = Default::default();
            ot_server.apply_crdt_change(dummy_insert(2));
            assert_eq!(ot_server.daemon_revision, 1);
            assert_eq!(ot_server.editor_revision, 0);
        }

        #[test]
        fn editor_operation_tracks_revision() {
            let mut ot_server: OTServer = Default::default();
            ot_server.apply_editor_operation(rev_delta(0, dummy_insert(2)));
            assert_eq!(ot_server.editor_revision, 1);
            assert_eq!(ot_server.daemon_revision, 0);
        }

        #[test]
        fn crdt_change_tracks_in_queue() {
            let mut ot_server: OTServer = Default::default();
            ot_server.apply_crdt_change(dummy_insert(2));
            assert_eq!(ot_server.editor_queue, vec![dummy_insert(2).into()]);
        }

        #[test]
        fn editor_operation_reduces_editor_queue() {
            let mut ot_server: OTServer = Default::default();

            ot_server.apply_crdt_change(dummy_insert(2));
            ot_server.apply_crdt_change(dummy_insert(5));
            ot_server.apply_crdt_change(dummy_insert(8));
            assert_eq!(ot_server.editor_queue.len(), 3);

            ot_server.apply_editor_operation(rev_delta(1, dummy_insert(2)));
            // we have already seen one op, so now the queue has only 2 left.
            assert_eq!(ot_server.editor_queue.len(), 2);
        }
    }

    mod operational_transform_internals {
        use super::*;
        use operational_transform::Operation as OTOperation;

        fn ot_insert(at: usize, s: &str) -> OperationSeq {
            let mut op_seq: OperationSeq = Default::default();
            op_seq.retain(at as u64);
            op_seq.insert(s);
            op_seq
        }

        fn ot_delete(from: usize, length: usize) -> OperationSeq {
            let mut op_seq: OperationSeq = Default::default();
            op_seq.retain(from as u64);
            op_seq.delete(length as u64);
            op_seq
        }

        pub fn ot_compose(mut op1: OperationSeq, op2: OperationSeq) -> OperationSeq {
            if op1.target_len() < op2.base_len() {
                op1.retain((op2.base_len() - op1.target_len()) as u64);
            }
            op1.compose(&op2)
                .expect("Composition failed. Lengths messed up?")
        }

        #[test]
        fn transforms_operation_correctly() {
            let mut ours = vec![ot_insert(0, "foo"), ot_insert(3, "foo")];
            // an insert at the same position as the first operation => clash.
            let theirs = ot_insert(0, "bar");
            let (theirs, ours_prime) = transform_through_operations(theirs, &ours);
            // expect the insert to be moved to the end
            assert_eq!(theirs, ot_insert(6, "bar"));
            // check that ours hasn't changed (besides retains that had to be inserted)
            ours[0].retain(3);
            ours[1].retain(3);
            assert_eq!(ours_prime, ours);
        }

        #[test]
        fn transforms_operation_correctly_different_base_lengths() {
            let ours = vec![ot_insert(3, "foo")];
            let mut theirs = ot_insert(0, "bar");
            let (theirs_prime, ours_prime) = transform_through_operations(theirs.clone(), &ours);
            // position of the insert hasn't shifted, but we got a retain added.
            theirs.retain(6);
            assert_eq!(theirs, theirs_prime);
            assert_eq!(ours_prime, vec![ot_insert(6, "foo")]);
        }

        #[test]
        fn transforms_operation_correctly_splits_deletion() {
            let editor_op = ot_insert(2, "x");
            let unacknowledged_ops = vec![ot_delete(1, 3)];

            let (op_prime, queue_prime) =
                transform_through_operations(editor_op, &unacknowledged_ops);
            assert_eq!(op_prime, ot_insert(1, "x"));
            assert_eq!(
                queue_prime,
                vec![ot_compose(ot_delete(1, 1), ot_delete(2, 2))]
            );
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

            let (a_prime, b_prime) = a
                .transform(&b)
                .expect("Transform failed. Do the lengths fit?");
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
            let (a_prime, c_prime) = a
                .transform(&c)
                .expect("Transform failed. Do the lengths fit?");
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
}
