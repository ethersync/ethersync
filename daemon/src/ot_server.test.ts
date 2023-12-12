import {expect, test} from "vitest"
import {OTServer, Insertion, Deletion, Operation} from "./ot_server"
import {type, insert, remove, TextOp} from "ot-text-unicode"

test("routes operations through server", () => {
    let opsSentToEditor: [number, TextOp][] = []

    let ot = new OTServer("hello", (editorRevision, op) => {
        opsSentToEditor.push([editorRevision, op])
    })

    ot.applyCRDTChange(insert(1, "x"))
    ot.applyEditorOperation(0, insert(2, "y"))

    expect(ot.operations).toEqual([insert(1, "x"), insert(3, "y")])

    expect(ot.document).toEqual("hxeyllo")

    expect(opsSentToEditor).toEqual([
        [0, insert(1, "x")],
        [1, insert(1, "x")],
    ])

    ot.applyCRDTChange(insert(3, "z")) // hxezyllo
    ot.applyEditorOperation(1, remove(1, 4)) // editor thinks: hxeyllo -> hlo

    expect(ot.operations).toEqual([
        insert(1, "x"), // hxello
        insert(3, "y"), // hxeyllo
        insert(3, "z"), // hxezyllo
        type.compose(remove(1, 2), remove(2, 2)), // hzlo
    ])

    expect(opsSentToEditor).toEqual([
        [0, insert(1, "x")],
        [1, insert(1, "x")],
        [1, insert(3, "z")],
        [2, insert(1, "z")],
    ])

    ot.applyEditorOperation(1, insert(1, "!")) // editor thinks: hlo -> h!lo

    expect(ot.document).toEqual("hz!lo")
})
