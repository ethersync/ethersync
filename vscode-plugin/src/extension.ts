import * as vscode from "vscode"
import * as cp from "child_process"
import * as rpc from "vscode-jsonrpc/node"
import * as path from "path"
import * as fs from "fs"
var Mutex = require("async-mutex").Mutex

function isEthersyncEnabled(dir: string) {
    if (fs.existsSync(path.join(dir, ".ethersync"))) {
        return true
    }

    const parentDir = path.resolve(dir, "..")

    // If we are at the root directory, stop the recursion.
    if (parentDir === dir) {
        return false
    }

    return isEthersyncEnabled(parentDir)
}

interface Position {
    line: number
    character: number
}

interface Range {
    start: Position
    end: Position
}

interface Delta {
    range: Range
    replacement: string
}

interface RevisionedDelta {
    delta: Delta[]
    revision: number
}

interface Edit {
    uri: string
    delta: RevisionedDelta
}

class Revision {
    daemon = 0
    editor = 0
}

let revisions: {[filename: string]: Revision} = {}
let contents: {[filename: string]: string[]} = {}

// TODO: if we load from disk, this will also cause edits :-/
let ignoreEdits = false
let t0 = Date.now()
const mutex = new Mutex()

function uri_to_fname(uri: string): string {
    const prefix = "file://"
    if (!uri.startsWith(prefix)) {
        console.error(`expected URI prefix for '${uri}'`)
    }
    return uri.slice(prefix.length)
}

// helpful: https://stackoverflow.com/a/54369605
function UTF16CodeUnitOffsetToCharOffset(utf16CodeUnitOffset: number, content: string): number {
    if (utf16CodeUnitOffset > content.length) {
        throw new Error(
            `Could not convert UTF-16 code unit offset ${utf16CodeUnitOffset} to char offset in string '${content}'`
        )
    }
    return [...content.slice(0, utf16CodeUnitOffset)].length
}

function charOffsetToUTF16CodeUnitOffset(charOffset: number, content: string): number {
    let utf16Offset = 0
    let chars = [...content]
    if (charOffset > chars.length) {
        throw new Error(`Could not convert char offset ${charOffset} to UTF-16 code unit offset in string '${content}'`)
    }
    for (const char of [...content].slice(0, charOffset)) {
        utf16Offset += char.length
    }
    return utf16Offset
}

export function activate(context: vscode.ExtensionContext) {
    debug("Ethersync extension activated!")
    const ethersyncClient = cp.spawn("ethersync", ["client"])

    ethersyncClient.on("error", (err) => {
        console.error(`Failed to start ethersync client: ${err.message}`)
    })

    ethersyncClient.on("exit", () => {
        vscode.window.showErrorMessage("Connection to Ethersync daemon lost.")
    })

    const connection = rpc.createMessageConnection(
        new rpc.StreamMessageReader(ethersyncClient.stdout),
        new rpc.StreamMessageWriter(ethersyncClient.stdin)
    )

    const open = new rpc.NotificationType<{uri: string}>("open")
    const close = new rpc.NotificationType<{uri: string}>("close")
    const edit = new rpc.NotificationType<Edit>("edit")

    connection.onNotification("edit", async (edit: Edit) => {
        const filename = uri_to_fname(edit.uri)
        let revision = revisions[filename]
        if (edit.delta.revision !== revision.editor) {
            debug(`Received edit for revision ${edit.delta.revision} (!= ${revision.editor}), ignoring`)
            return
        }

        revision.daemon += 1

        debug(`Received edit ${edit.delta.revision}`)
        console.log(revisions)
        const uri = edit.uri

        const openEditor = vscode.window.visibleTextEditors.find(
            (editor) => editor.document.uri.toString() === uri.toString()
        )
        if (openEditor) {
            try {
                await mutex.runExclusive(async () => {
                    ignoreEdits = true
                    for (const delta of edit.delta.delta) {
                        // TODO: make this nicer / use the vscode interface already
                        console.log(delta)
                        let startLineText = openEditor.document.lineAt(delta.range.start.line).text
                        let endLineText
                        if (delta.range.start.line == delta.range.end.line) {
                            endLineText = startLineText
                        } else {
                            endLineText = openEditor.document.lineAt(delta.range.end.line).text
                        }
                        const range = new vscode.Range(
                            new vscode.Position(
                                delta.range.start.line,
                                charOffsetToUTF16CodeUnitOffset(delta.range.start.character, startLineText)
                            ),
                            new vscode.Position(
                                delta.range.end.line,
                                charOffsetToUTF16CodeUnitOffset(delta.range.end.character, endLineText)
                            )
                        )
                        console.log(range)
                        // TODO: this logic seems redundant and `else` will never happen. Fix later.
                        if (openEditor) {
                            // Apply the edit if the document is open
                            await openEditor.edit((editBuilder) => {
                                editBuilder.replace(range, delta.replacement)
                            })
                            debug(`Edit applied to open document: ${uri.toString()}`)
                        } else {
                            debug(`Document not open: ${uri.toString()}`)
                        }
                    }
                    // TODO: Make this more efficient by replacing only the changed lines.
                    // The challenge with that is that we need to compute how many lines are
                    // left after the edit.
                    updateContents(openEditor.document)
                    ignoreEdits = false
                })
            } catch (e) {
                debug("promise failed")
                console.error(e)
            }
        }
    })

    // Start the connection
    connection.listen()

    let disposable = vscode.commands.registerCommand("ethersync.helloWorld", () => {
        vscode.window.showInformationMessage("Ethersync activated!")
    })

    context.subscriptions.push(disposable)

    // NOTE: We might get multiple events per document.version,
    // as the _state_ of the document might change (like isDirty).
    disposable = vscode.workspace.onDidChangeTextDocument((event) => {
        if (ignoreEdits) {
            debug("ack")
            return
        }
        let document = event.document

        // For some reason we get multipe events per edit caused by us.
        // Let's actively skip the empty ones to make debugging output below less noisy.
        if (event.contentChanges.length == 0) {
            if (document.isDirty == false) {
                debug("ignoring empty docChange. (probably saving...)")
            }
            return
        }

        const filename = document.fileName
        if (!isEthersyncEnabled(path.dirname(filename))) {
            return
        }

        let revision = revisions[filename]
        // debug("document.version: " + document.version)
        // debug("document.isDirty: " + document.isDirty)
        // if (event.contentChanges.length == 0) { console.log(document) }
        for (const change of event.contentChanges) {
            mutex
                .runExclusive(() => {
                    console.log(change)
                    let content = contents[filename]
                    console.log(content)
                    console.log(content[0])
                    let startLine = change.range.start.line
                    let endLine = change.range.end.line

                    debug("startLineText")
                    console.log(content[startLine])
                    let startLineText = content[startLine]
                    let endLineText
                    if (startLine == endLine) {
                        endLineText = startLineText
                    } else {
                        endLineText = content[endLine]
                    }
                    const range = new vscode.Range(
                        new vscode.Position(
                            startLine,
                            UTF16CodeUnitOffsetToCharOffset(change.range.start.character, startLineText)
                        ),
                        new vscode.Position(
                            endLine,
                            UTF16CodeUnitOffsetToCharOffset(change.range.end.character, endLineText)
                        )
                    )
                    console.log(change.range)
                    console.log(range)
                    let delta = {
                        range,
                        replacement: change.text
                    }
                    let revDelta: RevisionedDelta = {delta: [delta], revision: revision.daemon}
                    let uri = document.uri.toString()
                    let theEdit: Edit = {uri, delta: revDelta}
                    console.log(theEdit)
                    // interestingly this seems to block when it can't send
                    // TODO: Catch exceptions, for example when daemon disconnects/crashes.
                    connection.sendNotification(edit, theEdit)
                    revision.editor += 1
                    debug(`sent edit for dR ${revision.daemon} (having edR ${revision.editor})`)
                    console.log(revisions)

                    // TODO: Make this more efficient by replacing only the changed lines.
                    // The challenge with that is that we need to compute how many lines are
                    // left after the edit.
                    updateContents(document)
                })
                .catch((e: Error) => {
                    debug("promise failed!")
                    console.error(e)
                })
        }
    })

    context.subscriptions.push(disposable)

    // TODO: check if belongs to project.
    let openDisposable = vscode.workspace.onDidOpenTextDocument((document) => {
        const fileUri = document.uri.toString()
        debug("OPEN " + fileUri)
        revisions[document.fileName] = new Revision()
        updateContents(document)
        connection.sendNotification(open, {uri: fileUri})
        console.log(revisions)
    })

    context.subscriptions.push(openDisposable)

    let closeDisposable = vscode.workspace.onDidCloseTextDocument((document) => {
        if (!(document.fileName in revisions)) {
            return
        }
        const fileUri = document.uri.toString()
        connection.sendNotification(close, {uri: fileUri})

        delete revisions[document.fileName]
    })

    context.subscriptions.push(closeDisposable)
    debug("end of activation")
}

export function deactivate() {}

function debug(text: String) {
    console.log(Date.now() - t0 + " " + text)
}

function updateContents(document: vscode.TextDocument) {
    contents[document.fileName] = new Array(document.lineCount)
    for (let line = 0; line < document.lineCount; line++) {
        // TODO: text does not contain the newline character yet.
        contents[document.fileName][line] = document.lineAt(line).text
    }
}
