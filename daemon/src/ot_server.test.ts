import {expect, test} from "vitest"
import {OTServer, Insertion, Deletion, Operation} from "./ot_server"

test("transforms individual changes", () => {
    let ot = new OTServer("hello", () => {})

    expect(
        ot.transformChange(new Insertion(1, "x"), new Insertion(0, "y")),
    ).toEqual([new Insertion(2, "x")])

    expect(
        ot.transformChange(new Insertion(1, "x"), new Insertion(2, "y")),
    ).toEqual([new Insertion(1, "x")])

    expect(
        ot.transformChange(new Insertion(1, "x"), new Insertion(0, "yyy")),
    ).toEqual([new Insertion(4, "x")])

    expect(
        ot.transformChange(new Insertion(0, "x"), new Insertion(0, "y")),
    ).toEqual([new Insertion(1, "x")])

    expect(
        ot.transformChange(new Insertion(0, "xxx"), new Insertion(0, "y")),
    ).toEqual([new Insertion(1, "xxx")])

    expect(
        ot.transformChange(new Insertion(0, "x"), new Insertion(0, "yyy")),
    ).toEqual([new Insertion(3, "x")])

    expect(
        ot.transformChange(new Insertion(1, "abc"), new Deletion(0, 3)),
    ).toEqual([new Insertion(0, "abc")])

    expect(
        ot.transformChange(new Deletion(2, 1), new Insertion(0, "x")),
    ).toEqual([new Deletion(3, 1)])

    expect(
        ot.transformChange(new Deletion(2, 1), new Insertion(3, "x")),
    ).toEqual([new Deletion(2, 1)])

    expect(
        ot.transformChange(new Deletion(1, 3), new Insertion(2, "x")),
    ).toEqual([new Deletion(1, 1), new Deletion(2, 2)])

    expect(ot.transformChange(new Deletion(1, 5), new Deletion(1, 3))).toEqual([
        new Deletion(1, 2),
    ])

    expect(ot.transformChange(new Deletion(1, 5), new Deletion(0, 3))).toEqual([
        new Deletion(0, 3),
    ])

    expect(ot.transformChange(new Deletion(0, 3), new Deletion(0, 5))).toEqual(
        [],
    )

    expect(ot.transformChange(new Deletion(0, 3), new Deletion(0, 3))).toEqual(
        [],
    )

    expect(ot.transformChange(new Deletion(1, 1), new Deletion(0, 3))).toEqual(
        [],
    )
})

test("transforms insertions that apply to the same position", () => {
    let ot = new OTServer("hello", () => {})

    expect(
        ot.transformChange(new Insertion(1, "d"), new Insertion(1, "e"), true),
    ).toEqual([new Insertion(1, "d")])

    expect(
        ot.transformChange(new Insertion(1, "e"), new Insertion(1, "d"), false),
    ).toEqual([new Insertion(2, "e")])
})

test("transforms deletions that apply to the same position", () => {
    let ot = new OTServer("hello", () => {})

    expect(
        ot.transformChange(new Deletion(1, 1), new Deletion(1, 1), true),
    ).toEqual([new Deletion(1, 1)])
    expect(
        ot.transformChange(new Deletion(1, 1), new Deletion(1, 1), false),
    ).toEqual([])

    expect(
        ot.transformChange(new Deletion(0, 3), new Deletion(1, 3), true),
    ).toEqual([new Deletion(0, 3)])
    expect(
        ot.transformChange(new Deletion(0, 3), new Deletion(1, 3), false),
    ).toEqual([new Deletion(0, 1)])

    expect(
        ot.transformChange(new Deletion(1, 3), new Deletion(0, 3), true),
    ).toEqual([new Deletion(0, 3)])
    expect(
        ot.transformChange(new Deletion(1, 3), new Deletion(0, 3), false),
    ).toEqual([new Deletion(0, 1)])

    expect(
        ot.transformChange(new Deletion(2, 1), new Deletion(0, 5), true),
    ).toEqual([new Deletion(0, 1)])
    expect(
        ot.transformChange(new Deletion(2, 1), new Deletion(0, 5), false),
    ).toEqual([])

    expect(
        ot.transformChange(new Deletion(0, 5), new Deletion(2, 1), true),
    ).toEqual([new Deletion(0, 5)])
    expect(
        ot.transformChange(new Deletion(0, 5), new Deletion(2, 1), false),
    ).toEqual([new Deletion(0, 4)])
})

test("transforms two lists of changes", () => {
    let ot = new OTServer("hello", () => {})

    expect(
        ot.transformChanges([new Deletion(1, 3)], [new Insertion(2, "x")]),
    ).toEqual([
        [new Deletion(1, 1), new Deletion(2, 2)],
        [new Insertion(1, "x")],
    ])

    expect(
        ot.transformChanges(
            [new Deletion(1, 3), new Insertion(2, "y")], // hoy
            [new Insertion(2, "x")], // hexllo
        ),
    ).toEqual([
        [
            new Deletion(1, 1), // hxllo
            new Deletion(2, 2), // hxo
            new Insertion(3, "y"), // hxoy
        ],
        [new Insertion(1, "x")], // hxoy
    ])
})

test("transforms operations", () => {
    let ot = new OTServer("hello", () => {})

    expect(
        ot.transformOperation(
            new Operation("editor", [
                new Insertion(0, "x"),
                new Insertion(1, "y"),
            ]),
            new Operation("daemon", [new Insertion(0, "s")]),
        ),
    ).toEqual([
        new Operation("editor", [new Insertion(1, "x"), new Insertion(2, "y")]),
        new Operation("daemon", [new Insertion(0, "s")]),
    ])

    /*
        TODO: fix these tests! Are they correct?

    expect(
        ot.transformOperation(
            new Operation("editor", [
                new Insertion(1, "abc"), // habcello
                new Deletion(0, 5), // llo
            ]),
            new Operation("daemon", [
                new Deletion(0, 3), // lo
            ]),
        ),
    ).toEqual([
        new Operation("editor", [
            new Insertion(0, "abc"), // abclo
            new Deletion(0, 3), // lo
        ]),
        new Operation("daemon", [
            new Deletion(0, 1), // lo
        ]),
    ])

    expect(
        ot.transformOperation(
            new Operation("editor", [
                new Deletion(1, 1), // hllo
                new Insertion(2, "x"), // hlxlo
            ]),
            new Operation("daemon", [
                new Insertion(0, "y"), // yhello
                new Deletion(1, 3), // ylo
            ]),
        ),
    ).toEqual([
        new Operation("editor", [
            new Insertion(1, "x"), // yxlo
        ]),
        new Operation("daemon", [
            new Insertion(0, "y"), // yhlxlo
            new Deletion(1, 2), // yxlo
        ]),
    ])
    */
})

test("routes operations through server", () => {
    let opsSentToEditor: [number, Operation][] = []

    let ot = new OTServer("hello", (editorRevision, op) => {
        opsSentToEditor.push([editorRevision, op])
    })

    ot.applyCRDTChange(new Insertion(1, "x"))
    ot.applyEditorOperation(0, new Operation("editor", [new Insertion(2, "y")]))

    expect(ot.operations).toEqual([
        new Operation("daemon", [new Insertion(1, "x")]),
        new Operation("editor", [new Insertion(3, "y")]),
    ])

    expect(ot.document).toEqual("hxeyllo")

    expect(opsSentToEditor).toEqual([
        [0, new Operation("daemon", [new Insertion(1, "x")])],
        [1, new Operation("daemon", [new Insertion(1, "x")])],
    ])

    ot.applyCRDTChange(new Insertion(3, "z")) // hxezyllo
    ot.applyEditorOperation(1, new Operation("editor", [new Deletion(1, 4)])) // editor thinks: hxeyllo -> hlo

    expect(ot.operations).toEqual([
        new Operation("daemon", [new Insertion(1, "x")]), // hxello
        new Operation("editor", [new Insertion(3, "y")]), // hxeyllo
        new Operation("daemon", [new Insertion(3, "z")]), // hxezyllo
        new Operation("editor", [new Deletion(1, 2), new Deletion(2, 2)]), // hzlo
    ])

    expect(opsSentToEditor).toEqual([
        [0, new Operation("daemon", [new Insertion(1, "x")])],
        [1, new Operation("daemon", [new Insertion(1, "x")])],
        [1, new Operation("daemon", [new Insertion(3, "z")])],
        [2, new Operation("daemon", [new Insertion(1, "z")])],
    ])

    ot.applyEditorOperation(1, new Operation("editor", [new Insertion(1, "!")])) // editor thinks: hlo -> h!lo

    expect(ot.document).toEqual("hz!lo")
})

test("sends correct messages to editor", () => {
    let opsSentToEditor: [number, Operation][] = []

    let ot = new OTServer("12345", (editorRevision, op) => {
        opsSentToEditor.push([editorRevision, op])
    })

    ot.applyCRDTChange(new Insertion(2, "x"))
    ot.applyEditorOperation(1, new Operation("editor", [new Insertion(5, "y")]))

    expect(ot.document).toEqual("12x34y5")

    expect(opsSentToEditor).toEqual([
        [0, new Operation("daemon", [new Insertion(2, "x")])],
    ])
})
