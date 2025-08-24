// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import * as vscode from "vscode"
import { cleanUriFormatting, uriToFname } from "./extension"

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
    selection: vscode.DecorationOptions
}

let cursors: Map<string, RemoteCursor[]> = new Map()

export function setCursor(userid: string, name: string, uri: vscode.Uri, selections: vscode.DecorationOptions[]) {
    let usersCursors = cursors.get(userid)
    if (usersCursors) {
        // Remove all decorations by this user.
        for (let cursor of usersCursors) {
            // TODO: Refactor this into drawCursors below?
            const editors = vscode.window.visibleTextEditors.filter(
                (editor) => uriToFname(editor.document.uri.toString(), true) === uriToFname(cursor.uri.toString(), true)
            )
            for (const editor of editors) {
                editor.setDecorations(selectionDecorationType, [])
            }
        }
    }

    let newCursors = selections.map((s) => {
        return {name, selection: s, uri}
    })
    cursors.set(userid, newCursors)

    const editors = vscode.window.visibleTextEditors.filter((editor) => uriToFname(editor.document.uri.toString(), true) === uriToFname(uri.toString(), true))
    for (let editor of editors) {
        drawCursors(editor)
    }
}

export function getCursorInfo(): string {
    if (cursors.size == 0) {
        return "(No cursors.)"
    } else {
        let message: string[] = []
        cursors.forEach((usersCursors, _userid) => {
            for (let cursor of usersCursors) {
                let line1 = cursor.selection.range.start.line + 1
                let line2 = cursor.selection.range.end.line + 1
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

export function drawCursors(editor: vscode.TextEditor | undefined) {
    if (editor) {
        let uri = editor.document.uri;
        let allSelections = Array.from(cursors.values())
            .flat()
            .filter(cursor => uriToFname(cursor.uri.toString(), true) === uriToFname(uri.toString(), true))
            .map(cursor => cursor.selection);
        editor.setDecorations(selectionDecorationType, allSelections)
    }
}