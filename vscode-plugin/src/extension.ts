// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import * as vscode from "vscode"
import * as cp from "child_process"
import * as rpc from "vscode-jsonrpc/node"
import * as path from "path"
import * as fs from "fs"
var Mutex = require("async-mutex").Mutex

import {setCursor, getCursorInfo, drawCursors} from "./cursor"

function findMarkerDirectory(dir: string, marker: string) {
    if (fs.existsSync(path.join(dir, marker))) {
        return dir
    }

    const parentDir = path.resolve(dir, "..")

    // If we are at the root directory, stop the recursion.
    if (parentDir === dir) {
        return null
    }

    return findMarkerDirectory(parentDir, marker)
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
    userid: string
    name?: string
    uri: string
    ranges: Range[]
}

class Revision {
    daemon = 0
    editor = 0
}

interface Configuration {
    name: string
    cmd: string[]
    enabled: boolean
    rootMarkers: string[]
    activatePattern: string | null
}

class Client {
    name: string
    revisions: {[filename: string]: Revision} = {}
    contents: {[filename: string]: string[]} = {}
    ethersyncClient: cp.ChildProcess
    connection: rpc.MessageConnection
    directory: string

    constructor(name: string, cmd: string[], directory: string) {
        this.name = name
        this.directory = directory

        this.ethersyncClient = cp.spawn(cmd[0], cmd.slice(1), {cwd: directory})

        this.ethersyncClient.on("error", (err) => {
            vscode.window.showErrorMessage(
                `Failed to start Ethersync client. Maybe the command is not in your PATH?: ${err.message}`,
            )
        })

        this.ethersyncClient.on("exit", () => {
            vscode.window.showErrorMessage(
                `Connection to Ethersync daemon in '${directory}' lost or failed to initiate. Maybe there's no daemon running there?`,
            )
        })

        if (!this.ethersyncClient.stdout || !this.ethersyncClient.stdin) {
            die("Failed to spawn Ethersync client (no stdin/stdout)")
        }

        this.connection = rpc.createMessageConnection(
            new rpc.StreamMessageReader(this.ethersyncClient.stdout),
            new rpc.StreamMessageWriter(this.ethersyncClient.stdin),
        )

        this.connection.onNotification("edit", (edit) => processEditFromDaemon(this, edit))
        this.connection.onNotification("cursor", processCursorFromDaemon)

        // Start the connection
        this.connection.listen()
    }
}

let configurations: Configuration[] = []
let clients: Client[] = []

// TODO: if we load from disk, this will also cause edits :-/
let t0 = Date.now()
const mutex = new Mutex()
let attemptedRemoteEdits: Set<vscode.TextEdit[]> = new Set()

const openType = new rpc.RequestType<{uri: string; content: string}, string, void>("open")
const closeType = new rpc.RequestType<{uri: string}, string, void>("close")
const editType = new rpc.RequestType<Edit, string, void>("edit")
const cursorType = new rpc.NotificationType<Cursor>("cursor")

function uriToFname(uri: string): string {
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

function openCurrentTextDocuments() {
    vscode.workspace.textDocuments.map(processUserOpen)
}

function documentForUri(uri: string): vscode.TextDocument | undefined {
    return vscode.workspace.textDocuments.find((doc) => doc.uri.toString() === uri)
}

async function processEditFromDaemon(client: Client, edit: Edit) {
    try {
        await mutex.runExclusive(async () => {
            const filename = uriToFname(edit.uri)
            let revision = client.revisions[filename]
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
                    client.connection.sendRequest(editType, theEdit)
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
    let uri = cursor.uri

    const document = documentForUri(uri)

    try {
        let selections: vscode.DecorationOptions[] = []
        if (document) {
            selections = cursor.ranges
                .map((r) => ethersyncRangeToVSCodeRange(document, r))
                .map(vsCodeRangeToSelection)
                .map((s) => {
                    return {
                        range: s,
                        hoverMessage: cursor.name,
                    }
                })
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
    let client = findOrCreateClient(document)
    if (!client) {
        return false
    }

    let edits = []
    for (const delta of edit.delta) {
        const range = ethersyncRangeToVSCodeRange(document, delta.range)
        let edit = new vscode.TextEdit(range, delta.replacement)
        debug(`Edit applied to document ${decodeURI(document.uri.toString())}`)
        edits.push(edit)
    }
    let workspaceEdit = new vscode.WorkspaceEdit()
    workspaceEdit.set(document.uri, edits)
    let worked = await vscode.workspace.applyEdit(workspaceEdit)

    // TODO: Make this more efficient by replacing only the changed lines.
    // The challenge with that is that we need to compute how many lines are
    // left after the edit.
    if (worked) {
        updateContents(client, document)
    }
    return worked
}

function findValidConfiguration(document: vscode.TextDocument): [Configuration | null, string] {
    for (let configuration of configurations) {
        if (configuration.activatePattern) {
            let regexp = new RegExp(configuration.activatePattern)
            if (regexp.test(document.fileName)) {
                // TODO: /tmp?
                return [name, configuration, "/tmp"]
            }
        } else if (configuration.rootMarkers) {
            const filename = document.fileName
            for (let rootMarker of configuration.rootMarkers) {
                const directory = findMarkerDirectory(path.dirname(filename), rootMarker)
                if (directory) {
                    return [name, configuration, directory]
                }
            }
        }
    }
    return [null, "/tmp"]
}

function findOrCreateClient(document: vscode.TextDocument): Client | null {
    let [configuration, directory] = findValidConfiguration(document)
    if (!configuration) {
        return null
    }

    // We re-use connections for configs with the same name and directory.
    for (let client of clients) {
        if (client.name === configuration.name && client.directory === directory) {
            return client
        }
    }

    // Otherwise, we create a new config.
    let client = new Client(name, configuration.cmd, directory)
    clients.push(client)
    return client
}

async function processUserOpen(document: vscode.TextDocument) {
    let client = findOrCreateClient(document)
    if (!client) {
        return false
    }

    // In particular, ignore documents using the git: scheme,
    // which is used by VS Code's Git integration.
    if (document.uri.scheme !== "file") {
        return
    }

    const fileUri = document.uri.toString()
    debug("OPEN " + decodeURI(fileUri))
    const content = document.getText()
    client.connection
        .sendRequest(openType, {uri: fileUri, content: content})
        .then(() => {
            client.revisions[document.fileName] = new Revision()
            updateContents(client, document)
            debug("Successfully opened. Tracking changes.")
        })
        .catch(() => {
            debug("OPEN rejected by daemon")
        })
}

function processUserClose(document: vscode.TextDocument) {
    let client = findOrCreateClient(document)
    if (!client) {
        return false
    }

    if (!(document.fileName in client.revisions)) {
        // File is not currently tracked in ethersync.
        return
    }
    const fileUri = document.uri.toString()
    client.connection.sendRequest(closeType, {uri: fileUri})

    delete client.revisions[document.fileName]
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
    let client = findOrCreateClient(event.document)
    if (!client) {
        return false
    }

    if (!(event.document.fileName in client.revisions)) {
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

            const filename = document.fileName

            let revision = client.revisions[filename]

            let edits = vsCodeChangeEventToEthersyncEdits(client, event)

            for (const theEdit of edits) {
                // interestingly this seems to block when it can't send
                // TODO: Catch exceptions, for example when daemon disconnects/crashes.
                client.connection.sendRequest(editType, theEdit)
                revision.editor += 1

                debug(`sent edit for dR ${revision.daemon} (having edR ${revision.editor})`)
            }

            // TODO: Make this more efficient by replacing only the changed lines.
            // The challenge with that is that we need to compute how many lines are
            // left after the edit.
            updateContents(client, document)

            // Attempt auto-save to avoid the situation where one user closes a modified
            // document without saving, which will cause VS Code to undo the dirty changes.
            document.save()
        })
        .catch((e: Error) => {
            vscode.window.showErrorMessage(`Error while sending edit to Ethersync daemon: ${e}`)
        })
}

function processSelection(event: vscode.TextEditorSelectionChangeEvent) {
    let client = findOrCreateClient(event.textEditor.document)
    if (!client) {
        return false
    }

    if (!(event.textEditor.document.fileName in client.revisions)) {
        // File is not currently tracked in ethersync.
        return
    }
    let uri = event.textEditor.document.uri.toString()
    let content = client.contents[event.textEditor.document.fileName]
    let ranges = event.selections.map((s) => {
        return vsCodeRangeToEthersyncRange(content, s)
    })
    client.connection.sendNotification(cursorType, {uri, ranges})
}

function vsCodeChangeEventToEthersyncEdits(client: Client, event: vscode.TextDocumentChangeEvent): Edit[] {
    let document = event.document
    let filename = document.fileName

    let revision = client.revisions[filename]

    let content = client.contents[filename]
    let edits = []

    for (const change of event.contentChanges) {
        let delta = vsCodeChangeToEthersyncDelta(content, change)
        let uri = document.uri.toString()
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

    configurations = vscode.workspace
        .getConfiguration("ethersync")
        .get<Map<string, Configuration>>("configs", new Map())

    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument(processUserEdit),
        vscode.workspace.onDidOpenTextDocument(processUserOpen),
        vscode.workspace.onDidCloseTextDocument(processUserClose),
        vscode.window.onDidChangeTextEditorSelection(processSelection),
        vscode.window.onDidChangeActiveTextEditor(drawCursors),
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

function die(text: string): never {
    vscode.window.showErrorMessage(text)
    throw new Error(text)
}

function updateContents(client: Client, document: vscode.TextDocument) {
    client.contents[document.fileName] = new Array(document.lineCount)
    for (let line = 0; line < document.lineCount; line++) {
        client.contents[document.fileName][line] = document.lineAt(line).text
    }
}
