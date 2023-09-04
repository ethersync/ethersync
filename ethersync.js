const fs = require("fs")
const path = require("path")
const diff = require("diff")
const {watch} = require("node:fs/promises")

const Y = require("yjs")
const Yws = require("y-websocket")
const Ydb = require("y-leveldb")

let didFullSync = false

var ydoc = new Y.Doc()
var awareness

function connectToServer() {
    provider = new Yws.WebsocketProvider(
        "wss://etherwiki.blinry.org",
        "playground",
        ydoc,
        {WebSocketPolyfill: require("ws")}
    )
    awareness = provider.awareness

    provider.awareness.setLocalStateField("user", {
        name: process.env.USER + " (via ethersync)" || "anonymous",
        color: "#ff00ff",
    })

    provider.awareness.on("change", () => {
        for (const [clientID, state] of provider.awareness.getStates()) {
            if (state?.cursor?.head) {
                let head = Y.createAbsolutePositionFromRelativePosition(
                    JSON.parse(state.cursor.head),
                    ydoc
                )
                let anchor = Y.createAbsolutePositionFromRelativePosition(
                    JSON.parse(state.cursor.anchor),
                    ydoc
                )
                if (clientID != provider.awareness.clientID) {
                    sendCursor(head.index, anchor.index)
                }
            }
        }
    })
}

;(async () => {
    await startPersistence()
    await fullSync()
    startObserving()
    connectToServer()
})()

var ypages = ydoc.getArray("pages")

function startObserving() {
    ypages.observeDeep(async function (events) {
        for (const event of events) {
            let clientID = event.transaction.origin
            if (clientID == ydoc.clientID) {
                // Don't feed our own changes back to the editor.
                continue
            }

            let key = event.path[event.path.length - 1]
            if (key == "content") {
                filename = event.target.parent.get("title").toString()

                let index = 0

                while (event.delta[0]) {
                    if (event.delta[0]["retain"]) {
                        index += event.delta[0]["retain"]
                    } else if (event.delta[0]["insert"]) {
                        let text = event.delta[0]["insert"]
                        insertLocally(filename, index, text)
                    } else if (event.delta[0]["delete"]) {
                        let length = event.delta[0]["delete"]
                        deleteLocally(filename, index, length)
                    }
                    event.delta.shift()
                }
            }
        }
    })
}

function insertLocally(file, index, text) {
    if (client) {
        let message = ["insert", file, index, text].join("\t")
        client.socket.write(message)
    } else {
        let content = fs.readFileSync("output/" + file, "utf8")
        content = content.slice(0, index) + text + content.slice(index)
        fs.writeFileSync("output/" + file, content, "utf8")
    }
}

function deleteLocally(file, index, length) {
    if (client) {
        let message = ["delete", file, index, length].join("\t")
        client.socket.write(message)
    } else {
        let content = fs.readFileSync("output/" + file, "utf8")
        content = content.slice(0, index) + content.slice(index + length)
        fs.writeFileSync("output/" + file, content, "utf8")
    }
}

/*
ypages.observeDeep(async function (events) {
    if (!didFullSync) {
        await fullSync()
        didFullSync = true
    } else {
        for (const event of events) {
            let key = event.path[event.path.length - 1]
            if (key == "content") {
                filename = event.target.parent.get("title").toString()
                console.log("File changed via Y:", filename)
                await syncFile(filename)
                console.log("synced", filename)

                //console.log(event.delta)

                //let index = 0
                //if (event.delta[0]["retain"]) {
                //    index = event.delta[0]["retain"]
                //    event.delta.shift()
                //}

                //if (event.delta[0]["insert"]) {
                //    let text = event.delta[0]["insert"]
                //    insertFS(file, index, text)
                //} else if (event.delta[0]["delete"]) {
                //    let length = event.delta[0]["delete"]
                //    deleteFS(file, index, length)
                //}
            } else {
                console.log("Unhandled event", key)
            }
        }
    }
})

;(async () => {
    let watcher = watch("output")
    for await (const event of watcher) {
        console.log(event)
        if (event.eventType == "change") {
            let filename = event.filename
            let basename = path.basename(filename)
            if (basename[basename.length - 1] == "~" || basename == "4913") {
                // Never sync Vim backup files.
                // 4913 is a temporary file created by Vim in some situations.
                continue
            }

            console.log("File changed:", filename)
            await syncFile(filename)
            console.log("synced", filename)
        }
    }
})()
*/

function getDeltaOperations(initialText, finalText) {
    if (initialText === finalText) {
        return []
    }

    const edits = diff.diffChars(initialText, finalText)
    let prevOffset = 0
    let deltas = []

    // Map the edits onto Yjs delta operations
    for (const edit of edits) {
        if (edit.removed && edit.value) {
            deltas = [
                ...deltas,
                ...[
                    ...(prevOffset > 0 ? [{retain: prevOffset}] : []),
                    {delete: edit.value.length},
                ],
            ]
            prevOffset = 0
        } else if (edit.added && edit.value) {
            deltas = [
                ...deltas,
                ...[{retain: prevOffset}, {insert: edit.value}],
            ]
            prevOffset = edit.value.length
        } else {
            prevOffset = edit.value.length
        }
    }
    return deltas
}

function findPage(filename) {
    filename = path.basename(filename)
    let page = ypages.toArray().find((page) => {
        return page.get("title").toString() == filename
    })
    return page
}

async function fullSync() {
    for (const page of ypages) {
        let title = page.get("title").toString()
        await syncFile(title)
    }
}

async function syncFile(filename) {
    filename = path.join("output", filename)
    console.log("Syncing", filename)

    // Create the file if it doesn't exist.
    if (!fs.existsSync(filename)) {
        console.log("Creating file", filename)
        fs.writeFileSync(filename, "")
    }

    //// Create the cache file if it doesn't exist.
    //if (!fs.existsSync(cacheFilename)) {
    //    let dirname = path.dirname(cacheFilename)
    //    if (!fs.existsSync(dirname)) {
    //        fs.mkdir(dirname, {recursive: true})
    //    }
    //    let fileContent = fs.readFileSync(filename, "utf8")
    //    fs.writeFileSync(cacheFilename, fileContent)

    //    // Set modified time of cache file to that of other file.
    //    const fileModTime = fs.statSync(filename).mtimeMs
    //    const date = new Date(fileModTime)
    //    fs.utimesSync(cacheFilename, date, date)
    //}

    //const fileModTime = fs.statSync(filename).mtimeMs
    //const cacheFileModTime = fs.statSync(cacheFilename).mtimeMs

    let page = findPage(filename)
    let contentY = page.get("content").toString()

    let contentFile = fs.readFileSync(filename, "utf8")

    if (contentY !== contentFile) {
        const delta = getDeltaOperations(contentY, contentFile)
        if (delta.length > 0) {
            ydoc.transact(() => {
                page.get("content").applyDelta(delta, {sanitize: false})
            }, ydoc.clientID)
        }
    }

    //if (fileModTime > cacheFileModTime) {
    //    // File was changed externally.
    //    const fileContent = fs.readFileSync(filename, "utf8")
    //    const cacheFileContent = fs.readFileSync(cacheFilename, "utf8")
    //    const delta = getDeltaOperations(cacheFileContent, fileContent)
    //    if (delta.length > 0) {
    //        console.log("Applying delta", delta)
    //        page.get("content").applyDelta(delta)
    //    }
    //}

    //let newContent = page.get("content").toString()
    //writeToFile(filename, newContent)
}

/*
function getCacheFile(filename) {
    let dirname = path.dirname(filename)
    let basename = path.basename(filename)
    return path.join(dirname, ".ethersync", "cache", basename)
}

function writeToFile(filename, content) {
    let fileContent = fs.readFileSync(filename, "utf8")
    if (fileContent !== content) {
        fs.writeFileSync(filename, content)
        console.log("Wrote", filename)
    }
    let cacheFilename = getCacheFile(filename)
    fs.writeFileSync(cacheFilename, content)

    // Set modified time of both files to be the same.
    const fileModTime = fs.statSync(filename).mtimeMs
    const date = new Date(fileModTime)
    fs.utimesSync(cacheFilename, date, date)
    console.log("Wrote", cacheFilename)
}

*/

//function insertFS(file, index, text) {
//    console.log("Inserting", text, "at", index, "in", file)
//
//    let content = fs.readFileSync("output/" + file, "utf8")
//    content = content.slice(0, index) + text + content.slice(index)
//    fs.writeFileSync("output/" + file, content, "utf8")
//}
//
//function deleteFS(file, index, length) {
//    console.log("Deleting", length, "characters at", index, "in", file)
//
//    let content = fs.readFileSync("output/" + file, "utf8")
//    content = content.slice(0, index) + content.slice(index + length)
//    fs.writeFileSync("output/" + file, content, "utf8")
//}
//
//function insertY(file, index, text) {
//    console.log("Y-Inserting", text, "at", index, "in", file)
//
//    let page = ypages.toArray().find((page) => {
//        return page.get("title").toString() == file
//    })
//    page.get("content").insert(index, text)
//}
//
//function deleteY(file, index, length) {
//    console.log("Y-Deleting", length, "characters at", index, "in", file)
//
//    let page = ypages.toArray().find((page) => {
//        return page.get("title").toString() == file
//    })
//    page.get("content").delete(index, length)
//}

var net = require("net")

var client = null

var server = net.createServer()

server.on("connection", handleConnection)

server.listen(9000, function () {
    console.log("server listening to %j", server.address())
})

function handleConnection(conn) {
    var remoteAddress = conn.remoteAddress + ":" + conn.remotePort

    client = {socket: conn}

    console.log("new client connection from %s", remoteAddress)
    conn.setEncoding("utf8")

    conn.on("data", onConnData)
    conn.once("close", onConnClose)
    conn.on("error", onConnError)

    function onConnData(d) {
        console.log("received data: %s", d)
        let parts = d.split("\t")

        if (parts[0] === "insert") {
            let filename = parts[1]
            let index = parseInt(parts[2])
            let text = parts[3]

            ydoc.transact(() => {
                findPage(filename).get("content").insert(index, text)
            }, ydoc.clientID)
        } else if (parts[0] === "delete") {
            let filename = parts[1]
            let index = parseInt(parts[2])
            let length = parseInt(parts[3])

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
                    anchorPos
                )
            )
            let head = JSON.stringify(
                Y.createRelativePositionFromTypeIndex(
                    findPage(filename).get("content"),
                    headPos
                )
            )

            if (awareness) {
                awareness.setLocalStateField("cursor", {
                    anchor,
                    head,
                })
            }
        }
    }
    function onConnClose() {
        console.log("connection from %s closed", remoteAddress)

        client = null
    }
    function onConnError(err) {
        console.log("Connection %s error: %s", remoteAddress, err.message)
    }
}

function sendCursor(head, anchor) {
    if (client) {
        client.socket.write(["cursor", "filenameTBD", head, anchor].join("\t"))
    }
}

async function startPersistence() {
    const persistenceDir = "output/.ethersync/persistence"
    const LeveldbPersistence = require("y-leveldb").LeveldbPersistence
    const ldb = new LeveldbPersistence(persistenceDir)

    const persistedYdoc = await ldb.getYDoc("playground")
    const newUpdates = Y.encodeStateAsUpdate(ydoc)
    await ldb.storeUpdate("playground", newUpdates)
    Y.applyUpdate(ydoc, Y.encodeStateAsUpdate(persistedYdoc))

    ydoc.on("update", (update) => {
        ldb.storeUpdate("playground", update)
    })
}
