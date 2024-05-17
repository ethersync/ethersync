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

export function activate(context: vscode.ExtensionContext) {
    console.log('Congratulations, your extension "ethersync" is now active!')

    // Launch ethersync client binary
    const ethersyncClient = cp.spawn("ethersync", ["client"])

    ethersyncClient.on("error", (err) => {
        console.error(`Failed to start ethersync client: ${err.message}`)
    })

    // Create a JSON-RPC connection
    const connection = rpc.createMessageConnection(
        new rpc.StreamMessageReader(ethersyncClient.stdout),
        new rpc.StreamMessageWriter(ethersyncClient.stdin),
    )

    const open = new rpc.NotificationType<{uri: string}>("open")
    const close = new rpc.NotificationType<{uri: string}>("close")
    const edit = new rpc.NotificationType<Edit>("edit")

    // Listen for pong messages
    connection.onNotification("edit", (edit: Edit) => {
        console.log(edit)
    })

    // Start the connection
    connection.listen()

    let revision = 0

    let disposable = vscode.commands.registerCommand("ethersync.helloWorld", () => {
        vscode.window.showInformationMessage("Goodbye World from Ethersync!")
    })

    context.subscriptions.push(disposable)

    disposable = vscode.workspace.onDidChangeTextDocument((event) => {
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
            let revDelta: RevisionedDelta = {delta: [delta], revision}
            let uri = "file://" + filename
            let theEdit: Edit = {uri, delta: revDelta}
            console.log(edit)
            connection.sendNotification(edit, theEdit)
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
