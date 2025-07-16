// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import * as vscode from "vscode"

const selectionDecorationType = vscode.window.createTextEditorDecorationType({
    backgroundColor: "#1a4978",
    borderRadius: "0.1rem",
    rangeBehavior: vscode.DecorationRangeBehavior.ClosedClosed,
    before: {
        color: "#548abf",
        contentText: "ᛙ",
        margin: "0px 0px 0px -0.35ch",
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

export function getCursorInfo(): string {
    if (cursors.size == 0) {
        return "(No cursors.)"
    } else {
        let message: string[] = []
        cursors.forEach((usersCursors, _userid) => {
            for (let cursor of usersCursors) {
                let line1 = cursor.selection.active.line + 1
                let line2 = cursor.selection.anchor.line + 1
                if (line1 > line2) {
                    ;[line1, line2] = [line2, line1]
                }
                let position = line1 == line2 ? `${line1}` : `${line1}-${line2}`

                // TODO: Trim URI to the relevant parts inside the project.
                message.push(`${cursor.name} @ ${cursor.uri}:${position}`)
            }
        })
        return message.join("\n")
    }
}
