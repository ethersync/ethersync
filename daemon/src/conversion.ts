import {insert, remove, type, TextOp} from "ot-text-unicode"

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
export function yjsDeltaToTextOp(delta: any, content: string): TextOp {
    let operation: TextOp = []

    let index = 0 // in Unicode code points
    let indexUTF16 = 0 // in UTF-16 code units

    while (delta[0]) {
        if (delta[0]["retain"]) {
            index += UTF16CodeUnitOffsetToCharOffset(delta[0]["retain"], content.slice(indexUTF16))
            indexUTF16 += delta[0]["retain"]
        } else if (delta[0]["insert"]) {
            let text = delta[0]["insert"]
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
