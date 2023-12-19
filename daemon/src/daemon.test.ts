import {expect, test, beforeAll} from "vitest"
import {Daemon} from "./daemon"
import * as Y from "yjs"
import {insert, remove, TextOp} from "ot-text-unicode"

const daemon = new Daemon()

test("transforms from Yjs update to OT update", () => {
    expect(daemon.crdtEventToTextOp([{retain: 3}, {insert: "x"}], "hello")).toEqual(insert(3, "x"))
    expect(daemon.crdtEventToTextOp([{retain: 3}, {insert: "x"}], "höllo")).toEqual(insert(3, "x"))
    expect(daemon.crdtEventToTextOp([{retain: 3}, {insert: "x"}], "h🥕llo")).toEqual(insert(2, "x"))
    expect(daemon.crdtEventToTextOp([{retain: 3}, {delete: 2}], "hello")).toEqual(remove(3, 2))
    expect(daemon.crdtEventToTextOp([{retain: 3}, {delete: 2}], "helöo")).toEqual(remove(3, 2))
    expect(daemon.crdtEventToTextOp([{retain: 3}, {delete: 2}], "hel🥕o")).toEqual(remove(3, 1))
    expect(daemon.crdtEventToTextOp([{retain: 3}, {delete: 2}], "h🥕🥕o")).toEqual(remove(2, 1))
})

function ytextOpTest(content: string, op: TextOp, result: string) {
    let ydoc = new Y.Doc()
    let ytext = ydoc.getText("test")
    ytext.insert(0, content)
    daemon.applyTextOpToCRDT(insert(3, "x"), ytext)
    expect(ytext.toString()).toEqual(result)
}

test("applies OT update to Y.Text", () => {
    ytextOpTest("hello", insert(3, "x"), "helxlo")
})
