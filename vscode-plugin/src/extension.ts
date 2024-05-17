// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from "vscode"

// This method is called when your extension is activated
// Your extension is activated the very first time the command is executed
export function activate(context: vscode.ExtensionContext) {
    // Use the console to output diagnostic information (console.log) and errors (console.error)
    // This line of code will only be executed once when your extension is activated
    console.log('Congratulations, your extension "ethersync" is now active!')

    let revision = 0

    // The command has been defined in the package.json file
    // Now provide the implementation of the command with registerCommand
    // The commandId parameter must match the command field in package.json
    let disposable = vscode.commands.registerCommand("ethersync.helloWorld", () => {
        // The code you place here will be executed every time your command is executed
        // Display a message box to the user
        vscode.window.showInformationMessage("Goodbye World from Ethersync!")
    })

    context.subscriptions.push(disposable)

    disposable = vscode.workspace.onDidChangeTextDocument((event) => {
        const filename = event.document.fileName
        for (const change of event.contentChanges) {
            //console.log(change.range)
            //console.log(change.range.start, change.range.end, change.text)
            let delta = {
                range: {
                    anchor: {line: change.range.start.line, character: change.range.start.character},
                    head: {line: change.range.end.line, character: change.range.end.character},
                },
                replacement: change.text,
            }
            let revDelta = {delta, revision}
            let edit = {uri: "file://" + filename, delta: revDelta}
            console.log(edit)
        }
    })

    context.subscriptions.push(disposable)
}

// This method is called when your extension is deactivated
export function deactivate() {}
