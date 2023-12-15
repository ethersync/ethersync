import {cloneDeep} from "lodash"
import {type, insert, remove, TextOp, TextOpComponent} from "ot-text-unicode"

/*
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
export class OTServer {
    // "Source of truth" operations.
    operations: TextOp[] = []

    // Operations that we have sent to the editor, but we're not sure whether it has
    // accepted them. We have to keep them around until we know for sure, so that we
    // can correctly transform operations for the editor.
    //
    // Design Note: The daemon should do the transformation because we want to spare
    // the overhead of implementing the tranformation per editor plugin. In our case
    // there's a small number of editors, so transforming it in the daemon is feasible.
    editorQueue: TextOp[] = []

    constructor(
        public document: string,
        private sendToEditor: (editorRevision: number, o: TextOp) => void,
        private sendToCRDT: (o: TextOp) => void = () => {},
        private editorRevision: number = 0,
        private daemonRevision: number = 0,
    ) {}

    reset() {
        this.operations = []
        this.editorQueue = []
        this.editorRevision = 0
        this.daemonRevision = 0
    }

    // Called when the CRDT world makes a change to the document.
    applyCRDTChange(op: TextOp) {
        // We can apply the change immediately.
        this.operations.push(op)
        this.editorQueue.push(op)
        this.daemonRevision++
        this.applyChangeToDocument(op)

        // We assume that the editor is up-to-date, and send the operation to it.
        // If it can't accept it, we will transform and send it later.
        this.sendToEditor(this.editorRevision, op)
    }

    // Called when the editor sends us an operation.
    // daemonRevision is the revision this operation applies to.
    applyEditorOperation(daemonRevision: number, operation: TextOp) {
        if (daemonRevision === this.daemonRevision) {
            // The sent operation applies to the current daemon revision. We can apply it immediately.
            this.addEditorOperation(operation)
        } else {
            // The operation applies to an older daemon revision.
            // We need to transform it through the daemon operations that have happened since then.

            // But we at least we know that the editor has seen all daemon operations until
            // daemonOperation. So we can remove them from the editor queue.
            let daemonOperationsToTransform =
                this.daemonRevision - daemonRevision
            this.editorQueue.splice(
                0,
                this.editorQueue.length - daemonOperationsToTransform,
            )

            // Do the transformation!
            let [transformedOperation, transformedQueue] =
                this.transformOperationThroughOperations(
                    operation,
                    this.editorQueue,
                )
            // Apply the transformed operation to the document.
            this.addEditorOperation(transformedOperation)
            // And replace the editor queue with the transformed queue.
            this.editorQueue = transformedQueue

            // Send the transformed queue to the editor.
            for (let op of this.editorQueue) {
                this.sendToEditor(this.editorRevision, op)
            }
        }
    }

    // Applies a change to the document content.
    private applyChangeToDocument(op: TextOp) {
        this.document = type.apply(this.document, op)
    }

    // Adds an editor operation to the document.
    // Sends it to the CRDT world.
    private addEditorOperation(operation: TextOp) {
        this.operations.push(operation)
        this.editorRevision++
        this.sendToCRDT(operation)
        this.applyChangeToDocument(operation)
    }

    /*
        This function takes operations t1 and m1 ... m_n,
        and returns operations t1' and m1' ... m_n'.

           t1
        * ----> *
        |       |
     m1 |       | m1'
        v       v
        * ----> *
        |       |
     m2 |       | m2'
        v       v
        * ----> *
        |       |
     m3 |       | m3'
        v  t1'  v
        * ----> *

    */
    transformOperationThroughOperations(
        theirOperation: TextOp,
        myOperations: TextOp[],
    ): [TextOp, TextOp[]] {
        let transformedMyOperations: TextOp[] = []
        for (let myOperation of myOperations) {
            let myTransformedOp = type.transform(
                myOperation,
                theirOperation,
                "left",
            )
            theirOperation = type.transform(
                theirOperation,
                myOperation,
                "right",
            )
            transformedMyOperations.push(myTransformedOp)
        }
        return [theirOperation, transformedMyOperations]
    }
}
