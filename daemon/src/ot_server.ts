import {cloneDeep} from "lodash"

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
       It rejects d3' which it receives later.
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
    operations: Operation[] = []

    // Operations that we have sent to the editor, but we're not sure whether it has
    // accepted them. We have to keep them around until we know for sure, so that we
    // can correctly transform operations for the editor.
    editorQueue: Operation[] = []

    constructor(
        public document: string,
        private sendToEditor: (editorRevision: number, o: Operation) => void,
        private sendToCRDT: (o: Operation) => void = () => {},
    ) {}

    reset() {
        this.operations = []
        this.editorQueue = []
    }

    // Called when the CRDT world makes a change to the document.
    applyCRDTChange(change: Change) {
        // We can apply the change immediately.
        let operation = new Operation("daemon", [change])
        this.operations.push(operation)
        this.editorQueue.push(operation)
        this.applyChangeToDocument(change)

        // We assume that the editor is up-to-date, and send the operation to it.
        // If it can't accept it, we will transform and send it later.
        let editorRevision = this.operations.filter(
            (o) => o.sourceID === "editor",
        ).length
        this.sendToEditor(editorRevision, operation)
    }

    // Called when the editor sends us an operation.
    // daemonRevision is the revision this operation applies to.
    applyEditorOperation(daemonRevision: number, operation: Operation) {
        // Find the current daemon revision. This is the number of daemon-source operations in this.operations.
        let currentDaemonRevision = this.operations.filter(
            (o) => o.sourceID === "daemon",
        ).length
        if (daemonRevision === currentDaemonRevision) {
            // The sent operation applies to the current daemon revision. We can apply it immediately.
            this.addEditorOperation(operation)
        } else {
            // The operation applies to an older daemon revision.
            // We need to transform it through the daemon operations that have happened since then.

            // But we at least we know that the editor has seen all daemon operations until
            // daemonOperation. So we can remove them from the editor queue.
            let daemonOperationsToTransform =
                currentDaemonRevision - daemonRevision
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

            // Find the editor revision. This is the number of editor-source operations in this.operations.
            let editorRevision = this.operations.filter(
                (o) => o.sourceID === "editor",
            ).length
            // Send the transformed queue to the editor.
            for (let op of this.editorQueue) {
                this.sendToEditor(editorRevision, op)
            }
        }
    }

    // Applies a change to the document content.
    private applyChangeToDocument(change: Change) {
        if (change instanceof Insertion) {
            this.document =
                this.document.slice(0, change.position) +
                change.content +
                this.document.slice(change.position)
        } else {
            // change is a Deletion!
            this.document =
                this.document.slice(0, change.position) +
                this.document.slice(change.position + change.length)
        }
    }

    // Adds an editor operation to the document.
    // Sends it to the CRDT world.
    private addEditorOperation(operation: Operation) {
        this.operations.push(operation)
        this.sendToCRDT(operation)
        for (let change of operation.changes) {
            this.applyChangeToDocument(change)
        }
    }

    /*
        This function takes changes t1 and m1, and returns changes t1',
        so that t1 + m1' is equivalent to m1 + t1'.

        Note that t1' can be multiple changes (in case of a split deletion).

           t1
        * ----> *
        |       |
     m1 |       | m1'
        v  t1'  v
        * ----> *

        If theyGoFirst is true, then:

        - If two inserts apply to the same position, the first change goes first.
        - If two deletes overlap, the first change is the one that isn't shortened.

    */
    transformChange(
        theirChange: Change,
        myChange: Change,
        theyGoFirst = false,
    ): Change[] {
        if (theirChange instanceof Insertion) {
            if (myChange instanceof Insertion) {
                return this.transformInsertInsert(
                    theirChange,
                    myChange,
                    theyGoFirst,
                )
            } else {
                return this.transformInsertDelete(
                    theirChange,
                    myChange,
                    theyGoFirst,
                )
            }
        } else {
            if (myChange instanceof Insertion) {
                return this.transformDeleteInsert(
                    theirChange,
                    myChange,
                    theyGoFirst,
                )
            } else {
                return this.transformDeleteDelete(
                    theirChange,
                    myChange,
                    theyGoFirst,
                )
            }
        }
    }

    // The following four helper fuction define the transformation rules.
    private transformInsertInsert(
        theirChange: Insertion,
        myChange: Insertion,
        theyGoFirst = false,
    ): Change[] {
        if (
            myChange.position > theirChange.position ||
            (myChange.position === theirChange.position && theyGoFirst)
        ) {
            // No need to transform.
            return [cloneDeep(theirChange)]
        } else {
            // myChange.position <= theirChange.position
            let theirChange2 = cloneDeep(theirChange)
            theirChange2.position += myChange.content.length
            return [theirChange2]
        }
    }

    private transformInsertDelete(
        theirChange: Insertion,
        myChange: Deletion,
        theyGoFirst = false,
    ): Change[] {
        if (
            myChange.position > theirChange.position ||
            (myChange.position === theirChange.position && theyGoFirst)
        ) {
            // No need to transform.
            return [cloneDeep(theirChange)]
        } else if (myChange.position + myChange.length > theirChange.position) {
            let endOfMyChange = myChange.position + myChange.length
            if (endOfMyChange > theirChange.position) {
                let theirChange2 = cloneDeep(theirChange)
                theirChange2.position = myChange.position
                return [theirChange2]
            } else {
                let theirChange2 = cloneDeep(theirChange)
                theirChange2.position -= myChange.length
                return [theirChange2]
            }
        } else {
            let theirChange2 = cloneDeep(theirChange)
            theirChange2.position -= myChange.length
            return [theirChange2]
        }
    }

    private transformDeleteInsert(
        theirChange: Deletion,
        myChange: Insertion,
        theyGoFirst = false,
    ): Change[] {
        if (
            myChange.position > theirChange.position ||
            (myChange.position === theirChange.position && theyGoFirst)
        ) {
            if (theirChange.position + theirChange.length > myChange.position) {
                // Split their deletion into two parts.
                // Example: "abcde"
                // myChange: insert(2, "x")
                // theirChange: delete(1, 3)
                // result: [delete(1, 1), delete(2, 2)]
                let theirChange2 = cloneDeep(theirChange)
                let theirChange3 = cloneDeep(theirChange)
                theirChange2.length = myChange.position - theirChange.position
                theirChange3.position =
                    myChange.position +
                    myChange.content.length -
                    theirChange2.length
                theirChange3.length -= theirChange2.length
                return [theirChange2, theirChange3]
            } else {
                // No need to transform.
                return [cloneDeep(theirChange)]
            }
        } else {
            // myChange.position <= theirChange.position
            let theirChange2 = cloneDeep(theirChange)
            theirChange2.position += myChange.content.length
            return [theirChange2]
        }
    }

    private transformDeleteDelete(
        theirChange: Deletion,
        myChange: Deletion,
        theyGoFirst = false,
    ): Change[] {
        if (
            myChange.position > theirChange.position ||
            (myChange.position === theirChange.position && theyGoFirst)
        ) {
            let theirChange2 = cloneDeep(theirChange)

            let endOfTheirChange = theirChange.position + theirChange.length
            let endOfMyChange = myChange.position + myChange.length

            if (theyGoFirst) {
                // They win, and we don't need to shorten them.
                return [theirChange2]
            } else {
                if (endOfTheirChange > myChange.position) {
                    if (endOfTheirChange > endOfMyChange) {
                        theirChange2.length -= myChange.length
                    } else {
                        theirChange2.length -=
                            endOfTheirChange - myChange.position
                    }
                }
                return [theirChange2]
            }
        } else if (myChange.position + myChange.length > theirChange.position) {
            let theirChange2 = cloneDeep(theirChange)
            theirChange2.position = myChange.position
            let endOfMyChange = myChange.position + myChange.length
            let endOfTheirChange = theirChange.position + theirChange.length

            if (theyGoFirst) {
                // They win, and we don't need to shorten them.
                return [theirChange2]
            } else {
                if (endOfMyChange > endOfTheirChange) {
                    theirChange2.length -= myChange.length
                } else {
                    theirChange2.length -= endOfMyChange - theirChange.position
                }

                if (theirChange2.length > 0) {
                    return [theirChange2]
                } else {
                    return []
                }
            }
        } else {
            let theirChange2 = cloneDeep(theirChange)
            theirChange2.position -= myChange.length
            return [theirChange2]
        }
    }

    /*
        This function takes changes t1 and m1 ... m_n,
        and returns changes t1' and m1' ... m_n'.

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
    transformChangeThroughChanges(
        theirChange: Change,
        myChanges: Change[],
    ): [Change[], Change[]] {
        if (myChanges.length === 0) {
            return [[theirChange], []]
        } else {
            // Take first myChange, and transform it through theirChange.
            let myChange = myChanges.shift() as Change
            let transformedTheirChanges = this.transformChange(
                theirChange,
                myChange,
                false,
            )
            let transformedMyChanges = this.transformChange(
                myChange,
                theirChange,
                true,
            )

            // Recursively transform the rest.
            let [transformedFinalTheirChanges, transformedRemainingMyChanges] =
                this.transformChanges(transformedTheirChanges, myChanges)
            return [
                transformedFinalTheirChanges,
                transformedMyChanges.concat(transformedRemainingMyChanges),
            ]
        }
    }

    /*
        This function takes changes t1 ... t_n and m1 ... m_n,
        and returns changes t1' ... t_n' and m1' ... m_n'.

           t1      t2      t3
        * ----> * ----> * ----> *
        |       |       |       |
     m1 |       |       |       | m1'
        v       v       v       v
        * ----> * ----> * ----> *
        |       |       |       |
     m2 |       |       |       | m2'
        v       v       v       v
        * ----> * ----> * ----> *
        |       |       |       |
     m3 |       |       |       | m3'
        v  t1'  v  t2'  v  t3'  v
        * ----> * ----> * ----> *

    */

    transformChanges(
        theirChanges: Change[],
        myChanges: Change[],
    ): [Change[], Change[]] {
        if (theirChanges.length === 0) {
            return [[], cloneDeep(myChanges)]
        } else if (myChanges.length === 0) {
            return [cloneDeep(theirChanges), []]
        } else {
            // Take first theirChange, and transform it through all myChanges.
            let currentTheirChange = theirChanges.shift() as Change
            let [transformedCurrentTheirChanges, transformedMyChanges] =
                this.transformChangeThroughChanges(
                    currentTheirChange,
                    myChanges,
                )

            // Recursively transform the rest.
            let [transformedRemainingTheirChanges, transformedFinalMyChanges] =
                this.transformChanges(theirChanges, transformedMyChanges)
            return [
                transformedCurrentTheirChanges.concat(
                    transformedRemainingTheirChanges,
                ),
                transformedFinalMyChanges,
            ]
        }
    }

    /*
        This function takes operations t1 and m1, and returns operations t1',
        so that t1 + m1' is equivalent to m1 + t1'.

           t1
        * ----> *
        |       |
     m1 |       | m1'
        v  t1'  v
        * ----> *

    */

    transformOperation(
        theirOp: Operation,
        myOp: Operation,
    ): [Operation, Operation] {
        let theirChanges = cloneDeep(theirOp.changes)
        let myChanges = cloneDeep(myOp.changes)
        let [transformedTheirChanges, transformedMyChanges] =
            this.transformChanges(theirChanges, myChanges)
        return [
            new Operation(theirOp.sourceID, transformedTheirChanges),
            new Operation(myOp.sourceID, transformedMyChanges),
        ]
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
        theirOperation: Operation,
        myOperations: Operation[],
    ): [Operation, Operation[]] {
        let theirOp = cloneDeep(theirOperation)
        let transformedMyOperations: Operation[] = []
        for (let myOperation of myOperations) {
            let [theirTransformedOp, myTransformedOp] = this.transformOperation(
                theirOp,
                myOperation,
            )
            theirOp = theirTransformedOp
            transformedMyOperations.push(myTransformedOp)
        }
        return [theirOp, transformedMyOperations]
    }
}

export class Operation {
    constructor(
        public sourceID: string,
        public changes: Change[],
    ) {}

    public static fromJSON() {}
}

export type Change = Insertion | Deletion

export class Insertion {
    constructor(
        public position: number,
        public content: string,
    ) {}
}

export class Deletion {
    constructor(
        public position: number,
        public length: number,
    ) {}
}
