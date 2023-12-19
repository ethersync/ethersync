import {expect, test, beforeAll} from "vitest"
import {Daemon} from "./daemon"
import * as Y from "yjs"
import {insert, TextOp} from "ot-text-unicode"

const daemon = new Daemon()

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
