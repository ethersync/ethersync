import {expect, test} from "vitest"
import {OTServer} from "./ot_server"
import {type, insert, remove, TextOp} from "ot-text-unicode"

test("op transformation does what we think", () => {
    let a = insert(2, "x")
    let b = remove(1, 3)
    let c = insert(2, "y")

    expect(type.transform(a, b, "right")).toEqual(insert(1, "x"))
    expect(type.transform(a, b, "left")).toEqual(insert(1, "x"))
    expect(type.transform(b, a, "right")).toEqual(
        type.compose(remove(1, 1), remove(2, 2)),
    )
    expect(type.transform(b, a, "left")).toEqual(
        type.compose(remove(1, 1), remove(2, 2)),
    )

    // with inserts at the same position it makes a difference whether
    // you pass in "left" or "right"
    expect(type.transform(a, c, "right")).toEqual(insert(3, "x"))
    expect(type.transform(a, c, "left")).toEqual(insert(2, "x"))
})

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

test("transforms operations correctly", () => {
    let ot = new OTServer("hello", (editorRevision, op) => {})
    let editorOp = insert(2, "x")
    let unacknowledgedOps = [remove(1, 3)]

    let [transformedOperation, transformedQueue] =
        ot.transformOperationThroughOperations(editorOp, unacknowledgedOps)
    expect(transformedOperation).toEqual(insert(1, "x"))
    expect(transformedQueue).toEqual([type.compose(remove(1, 1), remove(2, 2))])
})

test("does not have any bugs", () => {
    let opsSentToEditor: [number, TextOp][] = []

    let ot = new OTServer("hello", (editorRevision, op) => {
        opsSentToEditor.push([editorRevision, op])
    })

    ot.applyCRDTChange(remove(1, 3)) // crdt: hello -> ho

    expect(opsSentToEditor).toEqual([
        [0, remove(1, 3)], // But the editor rejects it.
    ])

    ot.applyEditorOperation(0, insert(2, "x")) // editor thinks: hello -> hexllo

    expect(ot.operations).toEqual([
        remove(1, 3), // ho
        insert(1, "x"), // hxo
    ])

    expect(opsSentToEditor).toEqual([
        [0, remove(1, 3)], // But the editor rejects it.
        [1, type.compose(remove(1, 1), remove(2, 2))],
    ])

    expect(ot.document).toEqual("hxo")
})
