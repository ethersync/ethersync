import * as vscode from "vscode"
import * as cp from "child_process"
import * as rpc from "vscode-jsonrpc/node"

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

export function activate(context: vscode.ExtensionContext) {
    console.log('Congratulations, your extension "ethersync" is now active!')

    const ethersyncClient = cp.spawn("ethersync", ["client"])

    ethersyncClient.on("error", (err) => {
        console.error(`Failed to start ethersync client: ${err.message}`)
    })

    const connection = rpc.createMessageConnection(
        new rpc.StreamMessageReader(ethersyncClient.stdout),
        new rpc.StreamMessageWriter(ethersyncClient.stdin),
    )

    const open = new rpc.NotificationType<{uri: string}>("open")
    const close = new rpc.NotificationType<{uri: string}>("close")
    const edit = new rpc.NotificationType<Edit>("edit")

    connection.onNotification("edit", async (edit: Edit) => {
        if (edit.delta.revision !== editorRevision) {
            console.log(`Received edit for revision ${edit.delta.revision} (!= ${editorRevision}), ignoring`)
        }

        daemonRevision += 1

        console.log(edit)
        const uri = edit.uri

        const openEditor = vscode.window.visibleTextEditors.find(
            (editor) => editor.document.uri.toString() === uri.toString(),
        )

        if (openEditor) {
            ignoreEdits = true
            for (const delta of edit.delta.delta) {
                const range = new vscode.Range(
                    new vscode.Position(delta.range.anchor.line, delta.range.anchor.character),
                    new vscode.Position(delta.range.head.line, delta.range.head.character),
                )
                if (openEditor) {
                    // Apply the edit if the document is open
                    await openEditor.edit((editBuilder) => {
                        editBuilder.replace(range, delta.replacement)
                    })
                    console.log(`Edit applied to open document: ${uri.toString()}`)
                } else {
                    console.log(`Document not open: ${uri.toString()}`)
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

    disposable = vscode.workspace.onDidChangeTextDocument((event) => {
        if (ignoreEdits) {
            return
        }

        const filename = event.document.fileName
        console.log(event.document.version)
        for (const change of event.contentChanges) {
            let delta = {
                range: {
                    anchor: {line: change.range.start.line, character: change.range.start.character},
                    head: {line: change.range.end.line, character: change.range.end.character},
                },
                replacement: change.text,
            }
            let revDelta: RevisionedDelta = {delta: [delta], revision: daemonRevision}
            let uri = "file://" + filename
            let theEdit: Edit = {uri, delta: revDelta}
            console.log(edit)
            connection.sendNotification(edit, theEdit)
            editorRevision += 1
        }
    })

    context.subscriptions.push(disposable)

    let openDisposable = vscode.workspace.onDidOpenTextDocument((document) => {
        const fileUri = document.uri.toString()
        connection.sendNotification(open, {uri: fileUri})
    })

    context.subscriptions.push(openDisposable)

    let closeDisposable = vscode.workspace.onDidCloseTextDocument((document) => {
        const fileUri = document.uri.toString()
        connection.sendNotification(close, {uri: fileUri})
    })

    context.subscriptions.push(closeDisposable)
}

export function deactivate() {}
