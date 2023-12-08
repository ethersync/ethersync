import {cloneDeep} from "lodash"

export class OTServer {
    operations: Operation[] = []
    editorQueue: Operation[] = []

    constructor(
        public document: string,
        private sendToClient: (o: Operation) => void,
    ) {}

    applyChange(change: Change) {
        if (change instanceof Insertion) {
            this.document =
                this.document.slice(0, change.position) +
                change.content +
                this.document.slice(change.position)
        } else {
            // Deletion
            this.document =
                this.document.slice(0, change.position) +
                this.document.slice(change.position + change.length)
        }
    }

    applyCRDTChange(change: Change) {
        let operation = new Operation("daemon", [change])
        this.operations.push(operation)
        this.editorQueue.push(operation)
        this.applyChange(change)
        this.sendToClient(operation)
    }

    applyEditorOperation(daemonRevision: number, operation: Operation) {
        // Find the current daemon revision. This is the number of daemon-source operations in this.operations.
        let currentDaemonRevision = this.operations.filter(
            (o) => o.sourceID === "daemon",
        ).length
        if (daemonRevision === currentDaemonRevision) {
            // The sent operation applies to the current daemon revision. We can apply it immediately.
            this.operations.push(operation)
            for (let change of operation.changes) {
                this.applyChange(change)
            }
        } else {
            let daemonOperationsToTransform =
                currentDaemonRevision - daemonRevision
            this.editorQueue.splice(
                0,
                this.editorQueue.length - daemonOperationsToTransform,
            )
            let [transformedOperation, transformedQueue] =
                this.transformThroughOperations(operation, this.editorQueue)
            this.operations.push(transformedOperation)
            for (let change of transformedOperation.changes) {
                this.applyChange(change)
            }
            this.editorQueue = transformedQueue
        }
    }

    transformThroughOperations(
        theirOperation: Operation,
        myOperations: Operation[],
    ): [Operation, Operation[]] {
        let theirOp = cloneDeep(theirOperation)
        let transformedMyOperations: Operation[] = []
        for (let myOperation of myOperations) {
            let [theirTransformedOp, myTransformedOp] =
                this.transformOperationPair(theirOp, myOperation)
            theirOp = theirTransformedOp
            transformedMyOperations.push(myTransformedOp)
        }
        return [theirOp, transformedMyOperations]
    }

    transformOperationPair(
        theirOp: Operation,
        myOp: Operation,
    ): [Operation, Operation] {
        let theirChanges = cloneDeep(theirOp.changes)
        let myChanges = cloneDeep(myOp.changes)
        let [transformedTheirChanges, transformedMyChanges] =
            this.transformChanges(theirChanges, myChanges)
        return [
            new Operation(theirOp.sourceID, transformedTheirChanges),
            new Operation(theirOp.sourceID, transformedMyChanges),
        ]
    }

    transformOperation(theirOp: Operation, myOp: Operation): Operation {
        let theirChanges = cloneDeep(theirOp.changes)
        let myChanges = cloneDeep(myOp.changes)
        let [transformedTheirChanges, _] = this.transformChanges(
            theirChanges,
            myChanges,
        )
        return new Operation(theirOp.sourceID, transformedTheirChanges)
    }

    // Transforms theirOps by myOps, and return the transformed theirOps and the transformed myOps.
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
                this.transformOneChange(currentTheirChange, myChanges)

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
        throw new Error("We should never get here.")
    }

    transformOneChange(
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
        throw new Error("We should never get here.")
    }

    transformChange(
        theirChange: Change,
        myChange: Change,
        theyGoFirst = false,
    ): Change[] {
        if (myChange instanceof Deletion) {
            if (
                myChange.position > theirChange.position ||
                (myChange.position === theirChange.position && theyGoFirst)
            ) {
                if (theirChange instanceof Deletion) {
                    let theirChange2 = cloneDeep(theirChange)

                    let endOfTheirChange =
                        theirChange.position + theirChange.length
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
                } else {
                    // No need to transform.
                    return [cloneDeep(theirChange)]
                }
            } else {
                // myChange.position <= theirChange.position
                if (
                    myChange.position + myChange.length >
                    theirChange.position
                ) {
                    if (theirChange instanceof Deletion) {
                        let theirChange2 = cloneDeep(theirChange)
                        theirChange2.position = myChange.position
                        let endOfMyChange = myChange.position + myChange.length
                        let endOfTheirChange =
                            theirChange.position + theirChange.length

                        if (theyGoFirst) {
                            // They win, and we don't need to shorten them.
                            return [theirChange2]
                        } else {
                            if (endOfMyChange > endOfTheirChange) {
                                theirChange2.length -= myChange.length
                            } else {
                                theirChange2.length -=
                                    endOfMyChange - theirChange.position
                            }

                            if (theirChange2.length > 0) {
                                return [theirChange2]
                            } else {
                                return []
                            }
                        }
                    } else {
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
                    }
                } else {
                    let theirChange2 = cloneDeep(theirChange)
                    theirChange2.position -= myChange.length
                    return [theirChange2]
                }
            }
        } else {
            // myChange is an Insertion
            if (
                myChange.position > theirChange.position ||
                (myChange.position === theirChange.position && theyGoFirst)
            ) {
                if (theirChange instanceof Insertion) {
                    // No need to transform.
                    return [cloneDeep(theirChange)]
                } else {
                    if (
                        theirChange.position + theirChange.length >
                        myChange.position
                    ) {
                        // Split their deletion into two parts.
                        // Example: "abcde"
                        // myChange: insert(2, "x")
                        // theirChange: delete(1, 3)
                        // result: [delete(1, 1), delete(2, 2)]
                        let theirChange2 = cloneDeep(theirChange)
                        let theirChange3 = cloneDeep(theirChange)
                        theirChange2.length =
                            myChange.position - theirChange.position
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
                }
            } else {
                // myChange.position <= theirChange.position
                let theirChange2 = cloneDeep(theirChange)
                theirChange2.position += myChange.content.length
                return [theirChange2]
            }
        }
        // We should never get here.
        throw new Error("transformChange: unreachable")
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
