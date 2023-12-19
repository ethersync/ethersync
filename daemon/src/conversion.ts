import {insert, remove, type, TextOp} from "ot-text-unicode"

// helpful: https://stackoverflow.com/a/54369605
export function UTF16CodeUnitOffsetToCharOffset(utf16CodeUnitOffset: number, content: string): number {
    if (utf16CodeUnitOffset > content.length) {
        throw new Error("Out of bounds")
    }
    return [...content.slice(0, utf16CodeUnitOffset)].length
}

export function charOffsetToUTF16CodeUnitOffset(charOffset: number, content: string): number {
    let utf16Offset = 0
    let chars = [...content]
    if (charOffset > chars.length) {
        throw new Error("Out of bounds")
    }
    for (const char of [...content].slice(0, charOffset)) {
        utf16Offset += char.length
    }
    return utf16Offset
}

// delta is a Yjs update (counting in UTF-16 code units).
// We convert it to an OT operation (counting in Unicode code points).
// content is the document content before the update.
export type YjsDelta = Array<{
    insert?: string | object
    retain?: number
    delete?: number
}>
export function yjsDeltaToTextOp(delta: YjsDelta, content: string): TextOp {
    let operation: TextOp = []

    let index = 0 // in Unicode code points
    let indexUTF16 = 0 // in UTF-16 code units

    while (delta[0]) {
        if (delta[0]["retain"]) {
            index += UTF16CodeUnitOffsetToCharOffset(delta[0]["retain"], content.slice(indexUTF16))
            indexUTF16 += delta[0]["retain"]
        } else if (delta[0]["insert"]) {
            let text = delta[0]["insert"]
            if (typeof text !== "string") {
                throw new Error("Can only handle string insertions.")
            }
            operation = type.compose(operation, insert(index, text))
        } else if (delta[0]["delete"]) {
            let length = UTF16CodeUnitOffsetToCharOffset(delta[0]["delete"], content.slice(indexUTF16))
            indexUTF16 += delta[0]["delete"]
            operation = type.compose(operation, remove(index, length))
        }
        delta.shift()
    }
    return operation
}

// Applies a TextOp to a Yjs CRDT.
// We have to convert TextOp's counting in Unicode code points to Yjs's
// counting in UTF-16 code units.
export function textOpToYjsDelta(operation: TextOp, content: string): YjsDelta {
    let indexUTF16 = 0 // in UTF-16 code units
    let delta = []
    for (const change of operation) {
        switch (typeof change) {
            case "number":
                let offset = charOffsetToUTF16CodeUnitOffset(change, content.slice(indexUTF16))
                indexUTF16 += offset
                delta.push({retain: offset})
                break
            case "string":
                indexUTF16 += change.length
                delta.push({insert: change})
                break
            case "object":
                if (typeof change.d !== "number") {
                    throw new Error("Cannot handle string based deletions")
                }
                let length = charOffsetToUTF16CodeUnitOffset(change.d, content.slice(indexUTF16))
                indexUTF16 += length
                delta.push({delete: length})
                break
        }
    }
    return delta
}
