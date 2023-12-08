import {expect, test} from "vitest"
import {OTServer, Insertion, Deletion, Operation} from "./ot_server"

test("transforms individual changes", () => {
    let ot = new OTServer("hello", () => {})

    expect(
        ot.transformChange(new Deletion(1, 3), new Insertion(2, "x")),
    ).toEqual([new Deletion(1, 1), new Deletion(2, 2)])

    expect(
        ot.transformChange(new Insertion(1, "x"), new Insertion(0, "x")),
    ).toEqual([new Insertion(2, "x")])

    expect(
        ot.transformChange(new Insertion(1, "x"), new Insertion(2, "x")),
    ).toEqual([new Insertion(1, "x")])

    expect(
        ot.transformChange(new Insertion(1, "x"), new Insertion(0, "xxx")),
    ).toEqual([new Insertion(4, "x")])

    expect(
        ot.transformChange(new Insertion(0, "x"), new Insertion(0, "x")),
    ).toEqual([new Insertion(1, "x")])

    expect(
        ot.transformChange(new Insertion(1, "abc"), new Deletion(0, 3)),
    ).toEqual([new Insertion(0, "abc")])

    expect(
        ot.transformChange(new Deletion(2, 1), new Insertion(0, "x")),
    ).toEqual([new Deletion(3, 1)])

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
            new Operation("editor", 0, [
                new Insertion(0, "x"),
                new Insertion(1, "y"),
            ]),
            new Operation("server", 0, [new Insertion(0, "s")]),
        ),
    ).toEqual(
        new Operation("editor", 1, [
            new Insertion(1, "x"),
            new Insertion(2, "y"),
        ]),
    )

    expect(
        ot.transformOperation(
            new Operation("editor", 0, [
                new Insertion(1, "abc"), // habcello
                new Deletion(0, 5), // llo
            ]),
            new Operation("server", 0, [
                new Deletion(0, 3), // lo
            ]),
        ),
    ).toEqual(
        new Operation("editor", 1, [
            new Insertion(0, "abc"), // abclo
            new Deletion(0, 3), // lo
        ]),
    )

    expect(
        ot.transformOperation(
            new Operation("editor", 0, [
                new Deletion(1, 1), // hllo
                new Insertion(2, "x"), // hlxlo
            ]),
            new Operation("server", 0, [
                new Insertion(0, "y"), // yhello
                new Deletion(1, 3), // ylo
            ]),
        ),
    ).toEqual(
        new Operation("editor", 1, [
            new Insertion(1, "x"), // yxlo
        ]),
    )
})

test("routes operations through server", () => {
    let opsSentToEditor: Operation[] = []

    let ot = new OTServer("hello", (op) => {
        opsSentToEditor.push(op)
    })

    ot.applyCRDTChange(new Insertion(1, "x"))
    ot.applyEditorOperation(new Operation("editor", 0, [new Insertion(2, "y")]))

    expect(ot.operations).toEqual([
        new Operation("daemon", 0, [new Insertion(1, "x")]),
        new Operation("editor", 1, [new Insertion(3, "y")]),
    ])

    expect(ot.document).toEqual("hxeyllo")

    ot.applyCRDTChange(new Insertion(3, "z")) // hxezyllo
    ot.applyEditorOperation(new Operation("editor", 2, [new Deletion(1, 4)])) // editor thinks: hxeyllo -> hlo

    expect(ot.operations).toEqual([
        new Operation("daemon", 0, [new Insertion(1, "x")]), // hxello
        new Operation("editor", 1, [new Insertion(3, "y")]), // hxeyllo
        new Operation("daemon", 2, [new Insertion(3, "z")]), // hxezyllo
        new Operation("editor", 3, [new Deletion(1, 2), new Deletion(2, 2)]), // hzlo
    ])

    expect(opsSentToEditor).toEqual([
        new Operation("daemon", 0, [new Insertion(1, "x")]),
        new Operation("editor", 1, [new Insertion(3, "y")]),
        new Operation("daemon", 2, [new Insertion(3, "z")]),
        new Operation("editor", 3, [new Deletion(1, 2), new Deletion(2, 2)]),
    ])
})
