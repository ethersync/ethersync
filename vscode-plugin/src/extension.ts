import * as vscode from "vscode"
import * as cp from "child_process"
import * as rpc from "vscode-jsonrpc/node"
import * as path from "path"
import * as fs from "fs"
var Mutex = require("async-mutex").Mutex

import {setCursor, getCursorInfo} from "./cursor"

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

interface Edit {
    uri: string
    revision: number
    delta: Delta[]
}

interface Cursor {
    uri: string
    ranges: Range[]
}

interface CursorFromDaemon {
    userid: number
    name?: string
    uri: string
    ranges: Range[]
}

class Revision {
    daemon = 0
    editor = 0
}

let connection: rpc.MessageConnection

let revisions: {[filename: string]: Revision} = {}
let contents: {[filename: string]: string[]} = {}

// TODO: if we load from disk, this will also cause edits :-/
let t0 = Date.now()
const mutex = new Mutex()
let attemptedRemoteEdits: Set<vscode.TextEdit[]> = new Set()

const openType = new rpc.RequestType<{uri: string}, string, void>("open")
const closeType = new rpc.RequestType<{uri: string}, string, void>("close")
const editType = new rpc.RequestType<Edit, string, void>("edit")
const cursorType = new rpc.NotificationType<Cursor>("cursor")

function uriToFname(uri: string): string {
    let prefix = "file://"
    if (uri.startsWith(prefix)) {
        return uri.slice(prefix.length)
    }
    prefix = "file:///"
    if (uri.startsWith(prefix)) {
        return uri.slice(prefix.length)
    }
    debug(`expected URI prefix for '${uri}'`)
    return uri
}

// helpful: https://stackoverflow.com/a/54369605
function UTF16CodeUnitOffsetToCharOffset(utf16CodeUnitOffset: number, content: string): number {
    if (utf16CodeUnitOffset > content.length) {
        throw new Error(
            `Could not convert UTF-16 code unit offset ${utf16CodeUnitOffset} to char offset in string '${content}'`,
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
        end: vsCodePositionToEthersyncPosition(content, range.end),
    }
}

function vsCodePositionToEthersyncPosition(content: string[], position: vscode.Position): Position {
    let lineText = content[position.line]
    return {
        line: position.line,
        character: UTF16CodeUnitOffsetToCharOffset(position.character, lineText),
    }
}

function ethersyncPositionToVSCodePosition(document: vscode.TextDocument, position: Position): vscode.Position {
    let lineText = document.lineAt(position.line).text
    return new vscode.Position(position.line, charOffsetToUTF16CodeUnitOffset(position.character, lineText))
}

function ethersyncRangeToVSCodeRange(document: vscode.TextDocument, range: Range): vscode.Range {
    return new vscode.Range(
        ethersyncPositionToVSCodePosition(document, range.start),
        ethersyncPositionToVSCodePosition(document, range.end),
    )
}

function ethersyncDeltasToVSCodeTextEdits(document: vscode.TextDocument, deltas: Delta[]): vscode.TextEdit[] {
    return deltas.map((delta) => {
        let range = ethersyncRangeToVSCodeRange(document, delta.range)
        return vscode.TextEdit.replace(range, delta.replacement)
    })
}

function vsCodeRangeToSelection(range: vscode.Range): vscode.Selection {
    let anchor = range.start
    let active = range.end
    return new vscode.Selection(anchor, active)
}

function connect() {
    const ethersyncClient = cp.spawn("ethersync", ["client"])

    ethersyncClient.on("error", (err) => {
        vscode.window.showErrorMessage(`Failed to start ethersync client: ${err.message}`)
    })

    ethersyncClient.on("exit", () => {
        vscode.window.showErrorMessage("Connection to Ethersync daemon lost.")
    })

    connection = rpc.createMessageConnection(
        new rpc.StreamMessageReader(ethersyncClient.stdout),
        new rpc.StreamMessageWriter(ethersyncClient.stdin),
    )

    connection.onNotification("edit", processEditFromDaemon)
    connection.onNotification("cursor", processCursorFromDaemon)

    // Start the connection
    connection.listen()
}

function openCurrentTextDocuments() {
    vscode.workspace.textDocuments.map(processUserOpen)
}

function documentForUri(uri: string): vscode.TextDocument | undefined {
    return vscode.workspace.textDocuments.find((doc) => uriToFname(getDocumentUri(doc)) === uriToFname(cleanUriFormatting(uri)))
}

async function processEditFromDaemon(edit: Edit) {
    try {
        await mutex.runExclusive(async () => {
            const filename = cleanUriFormatting(uriToFname(edit.uri))
            let revision = revisions[filename]
            if (edit.revision !== revision.editor) {
                debug(`Received edit for revision ${edit.revision} (!= ${revision.editor}), ignoring`)
                return
            }

            debug(`Received edit ${edit.revision}`)

            const uri = edit.uri

            const document = documentForUri(uri)
            if (document) {
                let textEdit = ethersyncDeltasToVSCodeTextEdits(document, edit.delta)
                attemptedRemoteEdits.add(textEdit)
                let worked = await applyEdit(document, edit)
                if (worked) {
                    revision.daemon += 1
                    // Attempt auto-save to avoid the situation where one user closes a modified
                    // document without saving, which will cause VS Code to undo the dirty changes.
                    document.save()
                } else {
                    debug("rejected an applyEdit, sending empty delta")
                    let theEdit: Edit = {uri, revision: revision.daemon, delta: []}
                    connection.sendRequest(editType, theEdit)
                    revision.editor += 1
                }
            } else {
                throw new Error(`No document for URI ${uri}, why is the daemon sending me this?`)
            }
        })
    } catch (e) {
        vscode.window.showErrorMessage(`Error while processing edit from Ethersync daemon: ${e}`)
    }
}

async function processCursorFromDaemon(cursor: CursorFromDaemon) {
    let uri = cleanUriFormatting(cursor.uri);

    const document = documentForUri(uri)

    try {
        let selections: vscode.Selection[] = []
        if (document) {
            selections = cursor.ranges.map((r) => ethersyncRangeToVSCodeRange(document, r)).map(vsCodeRangeToSelection)
        }
        setCursor(cursor.userid, cursor.name || "anonymous", vscode.Uri.parse(uri), selections)
    } catch {
        // If we couldn't convert ethersyncRangeToVSCodeRange, it's probably because
        // we received the cursor message before integrating the edits, typing at the end of a line.
        // In practice, this isn't a problem, as the cursor decoration is pushed to the back automatically.
        // TODO: Make the ethersyncRangeToVSCodeRange function still return a proper range?
    }
}

async function applyEdit(document: vscode.TextDocument, edit: Edit): Promise<boolean> {
    let edits = []
    for (const delta of edit.delta) {
        const range = ethersyncRangeToVSCodeRange(document, delta.range)
        let edit = new vscode.TextEdit(range, delta.replacement)
        debug(`Edit applied to document ${getDocumentUri(document)}`)
        edits.push(edit)
    }
    let workspaceEdit = new vscode.WorkspaceEdit()
    workspaceEdit.set(document.uri, edits)
    let worked = await vscode.workspace.applyEdit(workspaceEdit)

    // TODO: Make this more efficient by replacing only the changed lines.
    // The challenge with that is that we need to compute how many lines are
    // left after the edit.
    if (worked) {
        updateContents(document)
    }
    return worked
}

// TODO: check if belongs to project.
async function processUserOpen(document: vscode.TextDocument) {
    const fileUri = getDocumentUri(document)
    connection
        .sendRequest(openType, {uri: fileUri})
        .then(() => {
            revisions[getDocumentFileName(document)] = new Revision()
            updateContents(document)
            debug("Successfully opened. Tracking changes.")
        })
        .catch(() => {
            debug("OPEN rejected by daemon")
        })
}

function processUserClose(document: vscode.TextDocument) {
    if (!(getDocumentFileName(document) in revisions)) {
        // File is not currently tracked in ethersync.
        return
    }
    const fileUri = getDocumentUri(document)
    connection.sendRequest(closeType, {uri: fileUri})

    delete revisions[getDocumentFileName(document)]
}

function isTextEditsEqualToVSCodeContentChanges(
    textEdits: vscode.TextEdit[],
    changes: readonly vscode.TextDocumentContentChangeEvent[],
): boolean {
    if (textEdits.length !== changes.length) {
        return false
    }
    for (let i = 0; i < textEdits.length; i++) {
        let textEdit = textEdits[i]
        let change = changes[i]
        if (textEdit.newText !== change.text || !textEdit.range.isEqual(change.range)) {
            return false
        }
    }
    return true
}

function isRemoteEdit(event: vscode.TextDocumentChangeEvent): boolean {
    let found: vscode.TextEdit[] | null = null
    for (let attemptedEdit of attemptedRemoteEdits) {
        if (isTextEditsEqualToVSCodeContentChanges(attemptedEdit, event.contentChanges)) {
            found = attemptedEdit
            break
        }
    }
    if (found !== null) {
        attemptedRemoteEdits.delete(found)
        return true
    }
    return false
}

// NOTE: We might get multiple events per document.version,
// as the _state_ of the document might change (like isDirty).
function processUserEdit(event: vscode.TextDocumentChangeEvent) {
    if (!(getDocumentFileName(event.document) in revisions)) {
        // File is not currently tracked in Ethersync.
        return
    }
    if (isRemoteEdit(event)) {
        debug("Ignoring remote event (we have caused it)")
        return
    }
    mutex
        .runExclusive(() => {
            let document = event.document

            // For some reason we get multipe events per edit caused by us.
            // Let's actively skip the empty ones to make debugging output below less noisy.
            if (event.contentChanges.length == 0) {
                if (document.isDirty == false) {
                    debug("Ignoring empty docChange. (probably saving...)")
                }
                return
            }

            const filename = getDocumentFileName(document)
            if (!isEthersyncEnabled(path.dirname(filename))) {
                return
            }

            let revision = revisions[filename]

            let edits = vsCodeChangeEventToEthersyncEdits(event)

            for (const theEdit of edits) {
                // interestingly this seems to block when it can't send
                // TODO: Catch exceptions, for example when daemon disconnects/crashes.
                connection.sendRequest(editType, theEdit)
                revision.editor += 1

                debug(`sent edit for dR ${revision.daemon} (having edR ${revision.editor})`)
            }

            // TODO: Make this more efficient by replacing only the changed lines.
            // The challenge with that is that we need to compute how many lines are
            // left after the edit.
            updateContents(document)

            // Attempt auto-save to avoid the situation where one user closes a modified
            // document without saving, which will cause VS Code to undo the dirty changes.
            document.save()
        })
        .catch((e: Error) => {
            vscode.window.showErrorMessage(`Error while sending edit to Ethersync daemon: ${e}`)
        })
}

function processSelection(event: vscode.TextEditorSelectionChangeEvent) {
    if (!(getDocumentFileName(event.textEditor.document) in revisions)) {
        // File is not currently tracked in ethersync.
        return
    }
    let uri = getDocumentUri(event.textEditor.document)
    let content = contents[getDocumentFileName(event.textEditor.document)]
    let ranges = event.selections.map((s) => {
        return vsCodeRangeToEthersyncRange(content, s)
    })
    connection.sendNotification(cursorType, {uri, ranges})
}

function vsCodeChangeEventToEthersyncEdits(event: vscode.TextDocumentChangeEvent): Edit[] {
    let document = event.document
    let filename = getDocumentFileName(document)

    let revision = revisions[filename]

    let content = contents[filename]
    let edits = []

    for (const change of event.contentChanges) {
        let delta = vsCodeChangeToEthersyncDelta(content, change)
        let uri = getDocumentUri(document)
        let theEdit: Edit = {uri, revision: revision.daemon, delta: [delta]}
        edits.push(theEdit)
    }
    return edits
}

function vsCodeChangeToEthersyncDelta(content: string[], change: vscode.TextDocumentContentChangeEvent): Delta {
    return {
        range: vsCodeRangeToEthersyncRange(content, change.range),
        replacement: change.text,
    }
}

function showCursorNotification() {
    vscode.window.showInformationMessage(getCursorInfo(), {modal: true})
}

export function activate(context: vscode.ExtensionContext) {
    debug("Ethersync extension activated!")

    connect()

    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument(processUserEdit),
        vscode.workspace.onDidOpenTextDocument(processUserOpen),
        vscode.workspace.onDidCloseTextDocument(processUserClose),
        vscode.window.onDidChangeTextEditorSelection(processSelection),
        vscode.commands.registerCommand("ethersync.showCursors", showCursorNotification),
    )

    openCurrentTextDocuments()

    debug("End of activation")
}

export function deactivate() {}

function debug(text: string) {
    // Disabled because we don't need it right now.
    // console.log(Date.now() - t0 + " " + text)
}

function updateContents(document: vscode.TextDocument) {
    let filename = getDocumentFileName(document)
    contents[filename] = new Array(document.lineCount)
    for (let line = 0; line < document.lineCount; line++) {
        contents[filename][line] = document.lineAt(line).text
    }
}

function getDocumentFileName(document: vscode.TextDocument){
    return cleanUriFormatting(document.fileName.toString());
}

function getDocumentUri(document: vscode.TextDocument){
    return cleanUriFormatting(document.uri.toString());
}

function cleanUriFormatting(uri: string){
    return decodeURI(uri).replaceAll("%3A",":").replaceAll("\\","/");
}