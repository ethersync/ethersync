import * as vscode from "vscode"

const selectionDecorationType = vscode.window.createTextEditorDecorationType({
    backgroundColor: "#1a4978",
    borderRadius: "0.1rem",
    rangeBehavior: vscode.DecorationRangeBehavior.ClosedClosed,
    before: {
        color: "#548abf",
        contentText: "ᛙ",
        margin: "0px 0px 0px -0.5ch",
        textDecoration: "font-weight: bold; position: absolute; top: 0; font-size: 200%; z-index: 0;",
    },
})

interface RemoteCursor {
    name: string
    uri: vscode.Uri
    selection: vscode.Selection
}

let cursors: Map<number, RemoteCursor[]> = new Map()

export function setCursor(userid: number, name: string, uri: vscode.Uri, selections: vscode.Selection[]) {
    let usersCursors = cursors.get(userid)
    if (usersCursors) {
        for (let cursor of usersCursors) {
            const editor = vscode.window.visibleTextEditors.find(
                (editor) => editor.document.uri.toString() === cursor.uri.toString(),
            )
            if (editor) {
                editor.setDecorations(selectionDecorationType, [])
            }
        }
    }

    let newCursors = selections.map((s) => {
        return {name, selection: s, uri}
    })
    cursors.set(userid, newCursors)

    const editor = vscode.window.visibleTextEditors.find((editor) => editor.document.uri.toString() === uri.toString())
    if (editor) {
        editor.setDecorations(selectionDecorationType, selections)
    }
}
