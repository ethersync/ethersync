import * as fs from "fs"
import * as path from "path"
import * as Y from "yjs"
import {WebsocketProvider} from "y-websocket"
import {LeveldbPersistence} from "y-leveldb"
import {
    JSONRPCServer,
    JSONRPCClient,
    JSONRPCServerAndClient,
} from "json-rpc-2.0"

import {JSONServer} from "./json_server"
import {OTServer, Operation, Deletion, Insertion} from "./ot_server"

var ydoc = new Y.Doc()
var server = new JSONServer(9000)
var provider: WebsocketProvider

const serverAndClient = new JSONRPCServerAndClient(
    new JSONRPCServer(),
    new JSONRPCClient((request) => {
        try {
            server.write(request)
            return Promise.resolve()
        } catch (error) {
            return Promise.reject(error)
        }
    }),
)

var ot = new OTServer(
    "",
    (editorRevision: number, operation: Operation) => {
        let parameters = [editorRevision, operation.changes]
        console.log("Sending op: ", JSON.stringify(parameters))
        serverAndClient.notify("operation", parameters)
    },
    (operation: Operation) => {
        console.log("Applying op to document: ", JSON.stringify(operation))
        for (const change of operation.changes) {
            if (change instanceof Insertion) {
                ydoc.transact(() => {
                    findPage("file")
                        .get("content")
                        .insert(change.position, change.content)
                }, ydoc.clientID)
            } else if (change instanceof Deletion) {
                ydoc.transact(() => {
                    findPage("file")
                        .get("content")
                        .delete(change.position, change.length)
                }, ydoc.clientID)
            }
        }
    },
)

serverAndClient.addMethod("debug", (params: any) => {
    console.log("DEBUG MESSAGE FROM EDITOR:")
    console.log(JSON.stringify(params, null, 2))
})

serverAndClient.addMethod("insert", (params: any) => {
    // TODO: Implement filename support...
    let daemonRevision = params[1]
    let index = params[2]
    let text = params[3]

    ot.applyEditorOperation(
        daemonRevision,
        new Operation("editor", [new Insertion(index, text)]),
    )
})

serverAndClient.addMethod("delete", (params: any) => {
    // TODO: Implement filename support...
    let daemonRevision = params[1]
    let index = params[2]
    let length = params[3]

    ot.applyEditorOperation(
        daemonRevision,
        new Operation("editor", [new Deletion(index, length)]),
    )
})

/*serverAndClient.addMethod("cursor", (params: any) => {
    let filename = params[0]
    let headPos = parseInt(params[1])
    let anchorPos = parseInt(params[2])

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
})*/

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

    /*provider.awareness.on("change", async () => {
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
                        await editorCursor(
                            "filenameTBD",
                            head.index,
                            anchor.index,
                        )
                    }
                }
            }
        }
    })*/
}

function setupEditorServer() {
    server.onConnection(() => {
        console.log("new connection, resetting OT")
        ot.reset()
    })

    server.onMessage((message: any) => {
        console.log(message)
        serverAndClient.receiveAndSend(message)
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

function startObserving() {
    ydoc.getArray("pages").observeDeep(async function (events: any) {
        for (const event of events) {
            let clientID = event.transaction.origin
            if (clientID == ydoc.clientID) {
                // Don't feed our own changes back to the editor.
                continue
            }

            let key = event.path[event.path.length - 1]
            if (key == "content") {
                //let filename = event.target.parent.get("title").toString()

                let index = 0

                while (event.delta[0]) {
                    if (event.delta[0]["retain"]) {
                        index += event.delta[0]["retain"]
                    } else if (event.delta[0]["insert"]) {
                        let text = event.delta[0]["insert"]
                        ot.applyCRDTChange(new Insertion(index, text))
                    } else if (event.delta[0]["delete"]) {
                        let length = event.delta[0]["delete"]
                        ot.applyCRDTChange(new Deletion(index, length))
                    }
                    event.delta.shift()
                }
            }
        }
    })
}

async function pullAllPages() {
    for (const page of ydoc.getArray("pages").toArray()) {
        let filename = (page as any).get("title").toString()
        filename = path.join("output", filename)
        console.log("Syncing", filename)

        // Create the file if it doesn't exist.
        if (!fs.existsSync(filename)) {
            console.log("Creating file", filename)
            fs.writeFileSync(filename, "")
        }

        let contentY = (page as any).get("content").toString()
        let contentFile = fs.readFileSync(filename, "utf8")

        if (contentY !== contentFile) {
            // TODO: Incorporate changes that have been made while the daemon was offline.
            fs.writeFileSync(filename, contentY)
        }
    }
}

async function startPersistence() {
    const persistenceDir = "output/.ethersync/persistence"
    const ldb = new LeveldbPersistence(persistenceDir)

    const persistedYdoc = await ldb.getYDoc("playground")
    const newUpdates = Y.encodeStateAsUpdate(ydoc)
    await ldb.storeUpdate("playground", newUpdates)
    Y.applyUpdate(ydoc, Y.encodeStateAsUpdate(persistedYdoc))

    ydoc.on("update", (update) => {
        ldb.storeUpdate("playground", update)
    })
}

connectToEtherwikiServer()
startPersistence()

// TODO: Time timeout is a hack we use to have "more content" before we write it to disk.
setTimeout(() => {
    pullAllPages()
    setupEditorServer()
    startObserving()
    console.log("Started.")
}, 1000)
