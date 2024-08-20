import * as vscode from "vscode"
import * as cp from "child_process"
import * as rpc from "vscode-jsonrpc/node"
import * as path from "path"
import * as fs from "fs"

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
    anchor: Position
    head: Position
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

let ignoreEdits = false
let daemonRevision = 0
let editorRevision = 0
let t0 = Date.now()

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
        if (edit.delta.revision !== editorRevision) {
            debug(`Received edit for revision ${edit.delta.revision} (!= ${editorRevision}), ignoring`)
            return
        }

        daemonRevision += 1

        debug(`Received edit ${edit.delta.revision}`)
        const uri = edit.uri

        const openEditor = vscode.window.visibleTextEditors.find(
            (editor) => editor.document.uri.toString() === uri.toString()
        )
        if (openEditor) {
            ignoreEdits = true
            for (const delta of edit.delta.delta) {
                const range = new vscode.Range(
                    new vscode.Position(delta.range.anchor.line, delta.range.anchor.character),
                    new vscode.Position(delta.range.head.line, delta.range.head.character)
                )
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
            ignoreEdits = false
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

        // For some reason we get multipe events per edit caused by us.
        // Let's actively skip the empty ones to make debugging output below less noisy.
        if (event.contentChanges.length == 0) {
            if (event.document.isDirty == false) {
                debug("ignoring empty docChange. (probably saving...)")
            }
            return
        }

        const filename = event.document.fileName
        if (!isEthersyncEnabled(path.dirname(filename))) {
            return
        }

        // debug("event.document.version: " + event.document.version)
        // debug("event.document.isDirty: " + event.document.isDirty)
        // if (event.contentChanges.length == 0) { console.log(event.document) }
        for (const change of event.contentChanges) {
            let delta = {
                range: {
                    anchor: {line: change.range.start.line, character: change.range.start.character},
                    head: {line: change.range.end.line, character: change.range.end.character},
                },
                replacement: change.text,
            }
            let revDelta: RevisionedDelta = {delta: [delta], revision: daemonRevision}
            let uri = event.document.uri.toString()
            let theEdit: Edit = {uri, delta: revDelta}
            // console.log(edit)
            connection.sendNotification(edit, theEdit)
            editorRevision += 1
            debug(`sent edit for dR ${daemonRevision} (having edR ${editorRevision})`)
        }
    })

    context.subscriptions.push(disposable)

    let openDisposable = vscode.workspace.onDidOpenTextDocument((document) => {
        const fileUri = document.uri.toString()
        debug("OPEN " + fileUri)
        connection.sendNotification(open, {uri: fileUri})
    })

    context.subscriptions.push(openDisposable)

    let closeDisposable = vscode.workspace.onDidCloseTextDocument((document) => {
        const fileUri = document.uri.toString()
        connection.sendNotification(close, {uri: fileUri})
    })

    context.subscriptions.push(closeDisposable)
    debug("end of activation")
}

export function deactivate() {}

function debug(text: String) {
    console.log(Date.now() - t0 + " " + text)
}
