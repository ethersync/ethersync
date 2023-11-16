import {NDJSONServer} from "./ndjson_server.js"
import * as Y from "yjs"
import {WebsocketProvider} from "y-websocket"

var ydoc = new Y.Doc()
var server = new NDJSONServer(9000)
var provider: WebsocketProvider

function connectToEtherwikiServer() {
    provider = new WebsocketProvider(
        "wss://etherwiki.blinry.org",
        "playground",
        ydoc,
        {
            WebSocketPolyfill: require("ws"),
        },
    )

    provider.awareness.setLocalStateField("user", {
        name: process.env.USER + " (via ethersync)" || "anonymous",
        color: "#ff00ff",
    })

    provider.awareness.on("change", () => {
        for (const [clientID, state] of provider.awareness.getStates()) {
            if (state?.cursor?.head) {
                let head = Y.createAbsolutePositionFromRelativePosition(
                    JSON.parse(state.cursor.head),
                    ydoc,
                )
                let anchor = Y.createAbsolutePositionFromRelativePosition(
                    JSON.parse(state.cursor.anchor),
                    ydoc,
                )
                if (head && anchor) {
                    if (clientID != provider.awareness.clientID) {
                        editorCursor("filenameTBD", head.index, anchor.index)
                    }
                }
            }
        }
    })
}

function setupEditorServer() {
    server.onConnection(() => {
        console.log("new connection")
    })

    server.onMessage((message: any) => {
        console.log(message)
        parseMessage(message)
    })

    server.onClose(() => {
        console.log("connection closed")
    })
}

function findPage(name: string): any {
    let page = ydoc
        .getArray("pages")
        .toArray()
        .find((p: any) => {
            return p.get("title").toString() == name
        })
    return page
}

function parseMessage(message: any) {
    // If it's not an array, its a debug messge. No need to interpret it.
    if (!Array.isArray(message)) {
        return
    }

    // Otherwise, it's a proper message for us.
    let parts: any[] = message
    if (parts[0] === "insert") {
        let filename = parts[1]
        let index = parts[2]
        let text = parts[3]

        ydoc.transact(() => {
            findPage(filename).get("content").insert(index, text)
        }, ydoc.clientID)
    } else if (parts[0] === "delete") {
        let filename = parts[1]
        let index = parts[2]
        let length = parts[3]

        ydoc.transact(() => {
            findPage(filename).get("content").delete(index, length)
        }, ydoc.clientID)
    } else if (parts[0] === "cursor") {
        let filename = parts[1]
        let headPos = parseInt(parts[2])
        let anchorPos = parseInt(parts[3])

        let anchor = JSON.stringify(
            Y.createRelativePositionFromTypeIndex(
                findPage(filename).get("content"),
                anchorPos,
            ),
        )
        let head = JSON.stringify(
            Y.createRelativePositionFromTypeIndex(
                findPage(filename).get("content"),
                headPos,
            ),
        )

        if (provider.awareness) {
            provider.awareness.setLocalStateField("cursor", {
                anchor,
                head,
            })
        }
    } else {
        console.log("unknown message type: %s", parts[0])
    }
}

function startObserving() {
    ydoc.getArray("pages").observeDeep(function (events: any) {
        for (const event of events) {
            let clientID = event.transaction.origin
            if (clientID == ydoc.clientID) {
                // Don't feed our own changes back to the editor.
                continue
            }

            let key = event.path[event.path.length - 1]
            if (key == "content") {
                let filename = event.target.parent.get("title").toString()

                let index = 0

                while (event.delta[0]) {
                    if (event.delta[0]["retain"]) {
                        index += event.delta[0]["retain"]
                    } else if (event.delta[0]["insert"]) {
                        let text = event.delta[0]["insert"]
                        editorInsert(filename, index, text)
                    } else if (event.delta[0]["delete"]) {
                        let length = event.delta[0]["delete"]
                        editorDelete(filename, index, length)
                    }
                    event.delta.shift()
                }
            }
        }
    })
}

function editorInsert(filename: string, index: number, text: string) {
    server.write(["insert", filename, index, text])
}

function editorDelete(filename: string, index: number, length: number) {
    server.write(["delete", filename, index, length])
}

function editorCursor(filename: string, head: number, anchor: number) {
    server.write(["cursor", filename, head, anchor])
}

connectToEtherwikiServer()
setupEditorServer()
startObserving()
