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
        let changes: Change[] = [] // These will be part of the resulting op.
        for (let theirChange of theirOp.changes) {
            let currentChange = {...theirChange}
            for (let myChange of myOp.changes) {
                currentChange = this.transformChange(currentChange, myChange)
            }
            changes.push(currentChange)
        }

        return new Operation(theirOp.sourceID, myOp.revision + 1, changes)
    }

    transformChange(theirChange: Change, myChange: Change): Change[] {
        if (myChange instanceof Deletion) {
            if (myChange.position > theirChange.position) {
                // do nothing for insertion :D
                if (theirChange instanceof Deletion) {
                    let theirChange2 = {...theirChange}
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
                }
            } else {
                /*
        "Transformation Algorithmus" (my_chg, their_chg)
	if my_chg is a delete(pos, n): 
		if my_chg.pos > their_chg.pos:
        ...
        else:
        	if my_chg.pos == their_chg.pos:
            	# TODO: dealbreaker (siehe unten...)
            else: # my_chg.pos < their_chg.pos
            	my_chg2 = my_chg
                their_chg2 = their_chg
                their_chg2.pos -= my_chg.n
                if my_chg.pos + my_chg.n > their_chg.pos:
                	# Kürze den chg, der vom editor kommt

                hello
                myChange: delete(0,4) -> o
                theirChange: delete(1,2) -> hlo
                OR theirChange: insert()
*/
                // myChange.position <= theirChange.position
                let theirChange2 = {...theirChange}
                theirChange2.position -= myChange.position
                if (
                    myChange.position + myChange.length >
                    theirChange.position
                ) {
                    // unsere
                }
            }
        }

        /*
    else: # my_chg is an insert(pos, content)
    	if my_chg.pos > their_chg.pos:
        	their_chg2 = their_chg
            if their_chg is an insert:
            	my_chg2 = my_chg
                my_chg2.pos += their_chg.content.length
			else if their_chg is a delete:
            	my_chg2 = my_chg
                my_chg2.pos -= their_chg.n
                # Hier kann es passieren, dass die fremde Löschung uns überlappt, und wir die Löschung in ZWEI STÜCKE splitten müssen...
        else:
        	if my_chg.pos == their_chg.pos:
            	if my_id=='daemon':
                	my_chg2 = my_chg
                    their_chg2 = their_chg
                    their_chg2.pos += my_chg.content.length 
                else: # my_id == 'editor'
                	# Ich muss nach hinten rücken
                    their_chg2 = their_chg
                    my_chg2 = my_chg
                    if their_chg is insert:
                    	my_chg2.pos += their_chg.content.length
                    if their_chg is delete:
                    	# Könnte uns überlappen -> Löschung muss evt. gesplitted werden
            else: # my_chg.pos < their_chg.pos
				my_chg2 = my_chg
                their_chg2 = their_chg
                their_chg2.pos += my_chg.content.length
	return my_chg2, their_chg2
    */
        return []
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
