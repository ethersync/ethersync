import {cloneDeep} from "lodash"

export class OTServer {
    operations: Operation[] = []
    constructor(
        public document: string,
        private sendToClient: (o: Operation) => void,
    ) {}

    applyCRDTChange(change: Change) {
        let operation = new Operation("daemon", this.operations.length, [
            change,
        ])
        this.operations.push(operation)
        this.sendToClient(operation)
    }

    applyEditorOperation(operation: Operation) {
        if (operation.revision <= this.operations.length) {
            operation = this.transformThroughAllOperations(operation)
        }
        for (let change of operation.changes) {
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

        this.operations.push(operation)
        // confirm operation + broadcast
    }

    transformThroughAllOperations(operation: Operation): Operation {
        let theirOp = operation
        for (
            let revision = operation.revision + 1;
            revision < this.operations.length;
            revision++
        ) {
            let myOp = this.operations[revision]
            theirOp = this.transformOperation(theirOp, myOp)
        }
        return theirOp
    }

    transformOperation(theirOp: Operation, myOp: Operation): Operation {
        let inputChanges = theirOp.changes
        let outputChanges: Change[] = []
        while (inputChanges.length > 0) {
            let currentChange = cloneDeep(inputChanges.shift())
            for (let myChange of myOp.changes) {
                let resultingChanges = this.transformChange(
                    currentChange,
                    myChange,
                )
                currentChange = resultingChanges.shift()
                inputChanges = [...resultingChanges, ...inputChanges]
            }
            outputChanges.push(currentChange)
        }

        return new Operation(theirOp.sourceID, myOp.revision + 1, outputChanges)
    }

    transformChange(theirChange: Change, myChange: Change): Change[] {
        if (myChange instanceof Deletion) {
            if (myChange.position > theirChange.position) {
                if (theirChange instanceof Deletion) {
                    let theirChange2 = cloneDeep(theirChange)
                    if (
                        theirChange.position + theirChange.length >
                        myChange.position
                    ) {
                        let endOfTheirChange =
                            theirChange.position + theirChange.length
                        let endOfMyChange = myChange.position + myChange.length

                        if (endOfTheirChange > endOfMyChange) {
                            theirChange2.length -= myChange.length
                        } else {
                            theirChange2.length -=
                                endOfTheirChange - myChange.position
                        }
                    }
                    return [theirChange2]
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
                        let theirChange2 = {...theirChange}
                        theirChange2.position -= myChange.position
                        let endOfMyChange = myChange.position + myChange.length
                        let endOfTheirChange =
                            theirChange.position + theirChange.length

                        if (endOfMyChange > endOfTheirChange) {
                            theirChange2.length -= myChange.length
                        } else {
                            theirChange2.length -=
                                endOfMyChange - theirChange.position
                        }
                        return [theirChange2]
                    } else {
                        let theirChange2 = {...theirChange}
                        theirChange2.position -= myChange.position
                        return [theirChange2]
                    }
                }
            }
        } else {
            // myChange is an Insertion
            if (myChange.position > theirChange.position) {
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
                        // result: [delete(1, 1), delete(3, 2)]
                        let theirChange2 = cloneDeep(theirChange)
                        let theirChange3 = cloneDeep(theirChange)
                        theirChange2.length =
                            myChange.position - theirChange.position
                        theirChange3.position =
                            myChange.position + myChange.content.length
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

class Operation {
    constructor(
        public sourceID: string,
        public revision: number,
        public changes: Change[],
    ) {}

    public static fromJSON() {}
}

export type Change = Insertion | Deletion

class Insertion {
    constructor(
        public position: number,
        public content: string,
    ) {}
}
class Deletion {
    constructor(
        public position: number,
        public length: number,
    ) {}
}
