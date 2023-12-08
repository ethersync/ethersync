import {cloneDeep} from "lodash"

export class OTServer {
    operations: Operation[] = []
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
        let operation = new Operation("daemon", this.operations.length, [
            change,
        ])
        this.operations.push(operation)
        this.applyChange(change)
        this.sendToClient(operation)
    }

    applyEditorOperation(operation: Operation) {
        if (operation.revision <= this.operations.length) {
            operation = this.transformThroughAllOperations(operation)
        }
        for (let change of operation.changes) {
            this.applyChange(change)
        }

        this.operations.push(operation)
        this.sendToClient(operation)
    }

    transformThroughAllOperations(operation: Operation): Operation {
        let theirOp = operation
        for (
            let revision = operation.revision;
            revision < this.operations.length;
            revision++
        ) {
            let myOp = this.operations[revision]
            theirOp = this.transformOperation(theirOp, myOp)
        }
        return theirOp
    }

    transformOperation(theirOp: Operation, myOp: Operation): Operation {
        let theirChanges = cloneDeep(theirOp.changes)
        let myChanges = cloneDeep(myOp.changes)
        let [transformedTheirChanges, _] = this.transformChanges(
            theirChanges,
            myChanges,
        )
        return new Operation(
            theirOp.sourceID,
            myOp.revision + 1,
            transformedTheirChanges,
        )
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
            )
            let transformedMyChanges = this.transformChange(
                myChange,
                theirChange,
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

    transformChange(theirChange: Change, myChange: Change): Change[] {
        if (myChange instanceof Deletion) {
            if (myChange.position > theirChange.position) {
                if (theirChange instanceof Deletion) {
                    let theirChange2 = cloneDeep(theirChange)

                    let endOfTheirChange =
                        theirChange.position + theirChange.length
                    let endOfMyChange = myChange.position + myChange.length

                    if (endOfTheirChange > myChange.position) {
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
                        let theirChange2 = cloneDeep(theirChange)
                        theirChange2.position = myChange.position
                        let endOfMyChange = myChange.position + myChange.length
                        let endOfTheirChange =
                            theirChange.position + theirChange.length

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
        public revision: number,
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
