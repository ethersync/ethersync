import * as fs from "fs"
import * as path from "path"
import * as Y from "yjs"
import {WebsocketProvider} from "y-websocket"
import {LeveldbPersistence} from "y-leveldb"
import {JSONRPCServer, JSONRPCClient, JSONRPCServerAndClient} from "json-rpc-2.0"
import {insert, remove, TextOp} from "ot-text-unicode"

import {JSONServer} from "./json_server"
import {OTServer} from "./ot_server"

import parse from "ini-simple-parser"
import {textOpToYjsDelta, yjsDeltaToTextOp} from "./conversion"

export class Daemon {
    etherwikiURL: string
    ydoc = new Y.Doc()
    server = new JSONServer(9000)
    clientID = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER)

    serverAndClient = new JSONRPCServerAndClient(
        new JSONRPCServer(),
        new JSONRPCClient((request) => {
            try {
                this.server.write(request)
                return Promise.resolve()
            } catch (error) {
                return Promise.reject(error)
            }
        }),
    )

    ot_documents: {[filename: string]: OTServer} = {}

    // Provide an "Ethersync directory", which should contain an .ethersync/config file.
    constructor(public directory: string | null = null) {
        if (directory === null) {
            console.log("No directory provided, not connecting to Etherwiki.")
            return
        }

        let configPath = path.join(directory, ".ethersync/config")
        if (!fs.existsSync(configPath)) {
            throw new Error(`No config file found at ${configPath}.`)
        }

        let config = parse(fs.readFileSync(configPath, "utf8"))
        if (!config["etherwiki"]) {
            throw new Error(`No etherwiki property found in config file at ${configPath}.`)
        }

        if (typeof config["etherwiki"] !== "string") {
            throw new Error(`Property 'etherwiki' in config file at ${configPath} is not a string.`)
        }

        this.etherwikiURL = config["etherwiki"]
    }

    start(): Promise<void> {
        return new Promise((resolve, _) => {
            this.addMethods()
            if (this.etherwikiURL) {
                this.connectToEtherwikiServer()
                this.startPersistence()
            }

            // TODO: Time timeout is a hack we use to have "more content" before we write it to disk.
            setTimeout(() => {
                this.pullAllPages()
                this.setupEditorServer()
                this.startObserving()
                console.log("Started.")
                resolve()
            }, 1000)
        })
    }

    initializeOTDocumentServer(filename: string) {
        let content = this.findPage(filename).get("content").toString()
        this.ot_documents[filename] = new OTServer(
            content,
            // sendToEditor
            (editorRevision: number, operation: TextOp) => {
                let parameters = [editorRevision, operation]
                // TODO: add filename, s.t. it applies to a certain buffer?
                // we first need plugin support for that, I guess.
                console.log("Sending op: ", JSON.stringify(parameters))
                this.serverAndClient.notify("operation", parameters)
            },
            // sendToCRDT
            (operation: TextOp) => {
                console.log("Applying op to document: ", JSON.stringify(operation))
                let ytext = this.findPage(filename).get("content")
                let delta = textOpToYjsDelta(operation, ytext.toString())
                this.ydoc.transact(() => {
                    ytext.applyDelta(delta)
                }, this.clientID)
            },
        )
    }

    addMethods() {
        this.serverAndClient.addMethod("debug", (params: any) => {
            console.log("DEBUG MESSAGE FROM EDITOR:")
            console.log(JSON.stringify(params, null, 2))
        })

        this.serverAndClient.addMethod("debug", (params: any) => {
            // Just for debugging purposes.
        })

        this.serverAndClient.addMethod("insert", (params: any) => {
            let filename = params[0]
            let daemonRevision = params[1]
            let index = params[2]
            let text = params[3]

            this.ot_documents[filename].applyEditorOperation(daemonRevision, insert(index, text))
        })

        this.serverAndClient.addMethod("delete", (params: any) => {
            let filename = params[0]
            let daemonRevision = params[1]
            let index = params[2]
            let length = params[3]

            this.ot_documents[filename].applyEditorOperation(daemonRevision, remove(index, length))
        })

        this.serverAndClient.addMethod("open", (params: any) => {
            let filename = params[0]

            if (this.findPage(filename) === undefined) {
                this.createPage(filename)
            }
            this.initializeOTDocumentServer(filename)
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
    }

    connectToEtherwikiServer() {
        let url = new URL(this.etherwikiURL)
        let domain = url.host
        let room = url.hash.slice(1)

        console.log(`Connecting to Etherwiki server at ${domain}#${room}`)

        let provider = new WebsocketProvider(`wss://${domain}`, room, this.ydoc, {
            WebSocketPolyfill: require("ws"),
        })

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

    setupEditorServer() {
        this.server.onConnection(() => {
            console.log("new connection")
        })

        this.server.onMessage((message: any) => {
            console.log(message)
            this.serverAndClient.receiveAndSend(message)
        })

        this.server.onClose(() => {
            console.log("connection closed")
        })
    }

    findPage(name: string): any {
        let page = this.ydoc
            .getArray("pages")
            .toArray()
            .find((p: any) => {
                return p.get("title").toString() == name
            })
        return page
    }

    createPage(filename: string, content: string = "") {
        let page = new Y.Map<Y.Text>()
        page.set("title", new Y.Text(filename))
        page.set("content", new Y.Text(content))
        this.ydoc.getArray("pages").insert(0, [page])
    }

    dropPage(filename: string) {
        let page = this.findPage(filename)
        if (page === undefined) {
            return
        }
        let pages = this.ydoc.getArray("pages")
        let i = pages.toArray().indexOf(page)
        pages.delete(i)
    }

    startObserving() {
        this.ydoc.getArray("pages").observeDeep(async (events: Array<Y.YEvent<any>>) => {
            for (const event of events) {
                let clientID = event.transaction.origin
                if (clientID == this.clientID) {
                    // Don't feed our own changes back to the editor.
                    continue
                }

                let key = event.path[event.path.length - 1]
                if (key == "content") {
                    let filename = event.target.parent.get("title").toString()

                    if (!(filename in this.ot_documents)) {
                        // Skip edits for files that are not opened.
                        continue
                    }

                    let content = this.ot_documents[filename].document
                    let operation = yjsDeltaToTextOp(event.delta, content)
                    this.ot_documents[filename].applyCRDTChange(operation)
                }
            }
        })
    }

    pullAllPages() {
        for (const page of this.ydoc.getArray("pages").toArray()) {
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

    async startPersistence() {
        const persistenceDir = "output/.ethersync/persistence"
        const ldb = new LeveldbPersistence(persistenceDir)

        const room = new URL(this.etherwikiURL).hash.slice(1)
        const persistedYdoc = await ldb.getYDoc(room)
        const newUpdates = Y.encodeStateAsUpdate(this.ydoc)
        await ldb.storeUpdate(room, newUpdates)
        Y.applyUpdate(this.ydoc, Y.encodeStateAsUpdate(persistedYdoc))

        this.ydoc.on("update", (update) => {
            ldb.storeUpdate(room, update)
        })
    }
}
