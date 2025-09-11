// SPDX-FileCopyrightText: 2016 Microsoft Corporation
//
// SPDX-License-Identifier: MIT

// These functions are adapted from the VS Code source code.

import {TextEdit, Range, TextDocument} from "vscode"
import {stringDiff} from "./vscode/diff"

// Adapted from https://github.com/microsoft/vscode/blob/main/src/vs/editor/common/core/range.ts
// TODO: Likely can be simplified.
export function compareRangesUsingStarts(a: Range | null | undefined, b: Range | null | undefined): number {
    if (a && b) {
        const aStartLineNumber = a.start.line | 0
        const bStartLineNumber = b.start.line | 0

        if (aStartLineNumber === bStartLineNumber) {
            const aStartColumn = a.start.character | 0
            const bStartColumn = b.start.character | 0

            if (aStartColumn === bStartColumn) {
                const aEndLineNumber = a.end.line | 0
                const bEndLineNumber = b.end.line | 0

                if (aEndLineNumber === bEndLineNumber) {
                    const aEndColumn = a.end.character | 0
                    const bEndColumn = b.end.character | 0
                    return aEndColumn - bEndColumn
                }
                return aEndLineNumber - bEndLineNumber
            }
            return aStartColumn - bStartColumn
        }
        return aStartLineNumber - bStartLineNumber
    }
    const aExists = a ? 1 : 0
    const bExists = b ? 1 : 0
    return aExists - bExists
}

const _diffLimit = 100000

// Adapted from https://github.com/microsoft/vscode/blob/main/src/vs/editor/common/services/editorWebWorker.ts
export function computeMoreMinimalEdits(document: TextDocument, edits: TextEdit[]): TextEdit[] {
    const pretty = false // It seems?

    const result: TextEdit[] = []

    /* TODO?
    let lastEol: EndOfLineSequence | undefined = undefined
    */

    edits = edits.slice(0).sort((a, b) => {
        if (a.range && b.range) {
            return compareRangesUsingStarts(a.range, b.range)
        }
        // eol only changes should go to the end
        const aRng = a.range ? 0 : 1
        const bRng = b.range ? 0 : 1
        return aRng - bRng
    })

    // merge adjacent edits
    let writeIndex = 0
    for (let readIndex = 1; readIndex < edits.length; readIndex++) {
        if (edits[writeIndex].range.end.isEqual(edits[readIndex].range.start)) {
            edits[writeIndex].range = new Range(edits[writeIndex].range.start, edits[readIndex].range.end)
            edits[writeIndex].newText += edits[readIndex].newText
        } else {
            writeIndex++
            edits[writeIndex] = edits[readIndex]
        }
    }
    edits.length = writeIndex + 1

    for (let {range, newText} of edits) {
        /* TODO?
        if (typeof eol === "number") {
            lastEol = eol
        }
        */

        if (range.isEmpty && !newText) {
            // empty change
            continue
        }

        const original = document.getText(range)

        /* TODO: Should we check this?
        text = text.replace(/\r\n|\n|\r/g, document.getEOL())

        if (original === text) {
            // noop
            continue
        }
        */

        // make sure diff won't take too long
        if (Math.max(newText.length, original.length) > _diffLimit) {
            result.push({range, newText})
            continue
        }

        // compute diff between original and edit.text
        const changes = stringDiff(original, newText, pretty)
        const editOffset = document.offsetAt(range.start)

        for (const change of changes) {
            const start = document.positionAt(editOffset + change.originalStart)
            const end = document.positionAt(editOffset + change.originalStart + change.originalLength)
            const newEdit: TextEdit = {
                newText: newText.substr(change.modifiedStart, change.modifiedLength),
                range: new Range(start, end),
            }

            if (document.getText(newEdit.range) !== newEdit.newText) {
                result.push(newEdit)
            }
        }
    }

    /* TODO? What happens here?
    if (typeof lastEol === "number") {
        result.push({
            eol: lastEol,
            text: "",
            range: {startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0},
        })
    }
    */

    return result
}
