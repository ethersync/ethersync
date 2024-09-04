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

let connection: rpc.MessageConnection

let revisions: {[filename: string]: Revision} = {}
let contents: {[filename: string]: string[]} = {}

// TODO: if we load from disk, this will also cause edits :-/
let ignoreEdits = false
let t0 = Date.now()
const mutex = new Mutex()

const open = new rpc.NotificationType<{uri: string}>("open")
const close = new rpc.NotificationType<{uri: string}>("close")
const edit = new rpc.NotificationType<Edit>("edit")

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

function vsCodeRangeToEthersyncRange(content: string[], range: vscode.Range): Range {
    return {
        start: vsCodePositionToEthersyncPosition(content, range.start),
        end: vsCodePositionToEthersyncPosition(content, range.end)
    }
}

function vsCodePositionToEthersyncPosition(content: string[], position: vscode.Position): Position {
    let lineText = content[position.line]
    return {
        line: position.line,
        character: UTF16CodeUnitOffsetToCharOffset(position.character, lineText)
    }
}

function ethersyncPositionToVSCodePosition(editor: vscode.TextEditor, position: Position): vscode.Position {
    let lineText = editor.document.lineAt(position.line).text
    return new vscode.Position(position.line, charOffsetToUTF16CodeUnitOffset(position.character, lineText))
}

function ethersyncRangeToVSCodeRange(editor: vscode.TextEditor, range: Range): vscode.Range {
    // TODO: make this nicer / use the vscode interface already (which interface? --blinry)
    return new vscode.Range(
        ethersyncPositionToVSCodePosition(editor, range.start),
        ethersyncPositionToVSCodePosition(editor, range.end)
    )
}

function connect() {
    const ethersyncClient = cp.spawn("ethersync", ["client"])

    ethersyncClient.on("error", (err) => {
        console.error(`Failed to start ethersync client: ${err.message}`)
    })

    ethersyncClient.on("exit", () => {
        vscode.window.showErrorMessage("Connection to Ethersync daemon lost.")
    })

    connection = rpc.createMessageConnection(
        new rpc.StreamMessageReader(ethersyncClient.stdout),
        new rpc.StreamMessageWriter(ethersyncClient.stdin)
    )

    connection.onNotification("edit", processEditFromDaemon)

    // Start the connection
    connection.listen()
}

async function processEditFromDaemon(edit: Edit) {
    try {
        await mutex.runExclusive(async () => {
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
                ignoreEdits = true
                await applyEdit(openEditor, edit)
                ignoreEdits = false
            } else {
                throw new Error(`No open editor for URI ${uri}, why is the daemon sending me this?`)
            }
        })
    } catch (e) {
        debug("promise failed")
        console.error(e)
    }
}

async function applyEdit(editor: vscode.TextEditor, edit: Edit) {
    for (const delta of edit.delta.delta) {
        const range = ethersyncRangeToVSCodeRange(editor, delta.range)
        console.log(range)

        // Apply the edit
        await editor.edit((editBuilder) => {
            editBuilder.replace(range, delta.replacement)
        })
        debug(`Edit applied to open document ${editor.document.uri.toString()}`)
    }
    // TODO: Make this more efficient by replacing only the changed lines.
    // The challenge with that is that we need to compute how many lines are
    // left after the edit.
    updateContents(editor.document)
}

// TODO: check if belongs to project.
function processUserOpen(document: vscode.TextDocument) {
    const fileUri = document.uri.toString()
    debug("OPEN " + fileUri)
    revisions[document.fileName] = new Revision()
    updateContents(document)
    connection.sendNotification(open, {uri: fileUri})
    console.log(revisions)
}

function processUserClose(document: vscode.TextDocument) {
    if (!(document.fileName in revisions)) {
        return
    }
    const fileUri = document.uri.toString()
    connection.sendNotification(close, {uri: fileUri})

    delete revisions[document.fileName]
}

// NOTE: We might get multiple events per document.version,
// as the _state_ of the document might change (like isDirty).
function processUserEdit(event: vscode.TextDocumentChangeEvent) {
    if (ignoreEdits) {
        debug("ack")
        return
    }
    mutex
        .runExclusive(() => {
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

            let edits = vsCodeChangeEventToEthersyncEdits(event)

            for (const theEdit of edits) {
                // interestingly this seems to block when it can't send
                // TODO: Catch exceptions, for example when daemon disconnects/crashes.
                connection.sendNotification(edit, theEdit)
                revision.editor += 1

                debug(`sent edit for dR ${revision.daemon} (having edR ${revision.editor})`)
                console.log(revisions)
            }

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

function vsCodeChangeEventToEthersyncEdits(event: vscode.TextDocumentChangeEvent): Edit[] {
    let document = event.document
    let filename = document.fileName

    let revision = revisions[filename]
    // debug("document.version: " + document.version)
    // debug("document.isDirty: " + document.isDirty)
    // if (event.contentChanges.length == 0) { console.log(document) }

    let content = contents[filename]
    let edits = []

    for (const change of event.contentChanges) {
        let delta = vsCodeChangeToEthersyncDelta(content, change)
        let revDelta: RevisionedDelta = {delta: [delta], revision: revision.daemon}
        let uri = document.uri.toString()
        let theEdit: Edit = {uri, delta: revDelta}
        edits.push(theEdit)
    }
    return edits
}

function vsCodeChangeToEthersyncDelta(content: string[], change: vscode.TextDocumentContentChangeEvent): Delta {
    return {
        range: vsCodeRangeToEthersyncRange(content, change.range),
        replacement: change.text
    }
}

export function activate(context: vscode.ExtensionContext) {
    debug("Ethersync extension activated!")

    connect()

    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument(processUserEdit),
        vscode.workspace.onDidOpenTextDocument(processUserOpen),
        vscode.workspace.onDidCloseTextDocument(processUserClose)
    )

    debug("End of activation")
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
