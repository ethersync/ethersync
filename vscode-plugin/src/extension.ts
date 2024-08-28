import * as vscode from "vscode"

const t0 = Date.now()

function uri_to_fname(uri: string): string {
    const prefix = "file://"
    if (!uri.startsWith(prefix)) {
        console.error(`expected URI prefix for '${uri}'`)
    }
    return uri.slice(prefix.length)
}

export async function activate(context: vscode.ExtensionContext) {
    debug("Ethersync extension activated!")
    const ws = vscode.workspace
    const win = vscode.window
    if (ws.workspaceFolders) {
        let wsPath = ws.workspaceFolders[0].uri.path
        const filePath = vscode.Uri.file(wsPath + "/test_position_waeCah7i.txt")
        const filePathBefore = vscode.Uri.file(wsPath + "/test_position_waeCah7i_before.txt")
        // const content = '01234\n🥕1234\n0🥕234\n01🥕34\n012🥕4\n0123🥕\n⛄1234\n0⛄234\n01⛄34\n012⛄4\n0123⛄\n💚1234\n0💚234\n01💚34\n012💚4\n0123💚\n'
        const content = "01🥕34\n01🥕34\n01🥕34\n01🥕34\n01🥕34\n01🥕34\n01234\n01234\n01234\n01234\n01234\n01234\n"
        await ws.fs.writeFile(filePath, Buffer.from(content))
        await ws.fs.writeFile(filePathBefore, Buffer.from(content))

        // Close all editors from a previous run.
        //vscode.window.visibleTextEditors.map((ed) => { ed.})
        vscode.commands.executeCommand("workbench.action.closeActiveEditor")

        const textDocument = await ws.openTextDocument(filePath)

        debug("Looping over offset to show line/character Position")
        for (let i = 0; i < content.length; i++) {
            const pos = textDocument.positionAt(i)
            console.log(i, pos.line, pos.character)
        }

        const editor = await win.showTextDocument(textDocument)
        const nLines = (content.match(/\n/g) || []).length
        editor.edit((editBuilder) => {
            for (let line = 0; line < nLines; line++) {
                const character = line % 6
                const range = new vscode.Range(
                    new vscode.Position(line, character),
                    new vscode.Position(line, character)
                )
                editBuilder.replace(range, ".")
            }
        })
    }

    let disposable = vscode.commands.registerCommand("ethersync.helloWorld", () => {
        vscode.window.showInformationMessage("Ethersync activated!")
    })

    context.subscriptions.push(disposable)

    debug("end of activation")
}

export function deactivate() {}

function debug(text: String) {
    console.log(Date.now() - t0 + " " + text)
}
