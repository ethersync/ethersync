const fs = require("fs")
const path = require("path")
const diff = require("diff")
const {watch} = require("node:fs/promises")
const Mutex = require("async-mutex").Mutex

const Y = require("yjs")
const Yws = require("y-websocket")

let didFullSync = false

var ydoc = new Y.Doc()

function connectToServer() {
    var provider = new Yws.WebsocketProvider(
        "wss://etherwiki.blinry.org",
        "playground",
        ydoc,
        {WebSocketPolyfill: require("ws")}
    )

    provider.awareness.setLocalStateField("user", {
        name: process.env.USER + " (via ethersync)" || "anonymous",
        color: "#ff00ff",
    })

    provider.awareness.on("change", () => {
        //console.log([...provider.awareness.getStates()])
        for (const [clientID, state] of provider.awareness.getStates()) {
            //console.log(clientID, state)
            if (state?.cursor?.head) {
                let head = Y.createAbsolutePositionFromRelativePosition(
                    JSON.parse(state.cursor.head),
                    ydoc
                )
                let anchor = Y.createAbsolutePositionFromRelativePosition(
                    JSON.parse(state.cursor.anchor),
                    ydoc
                )
                //console.log(position)
                if (clientID != provider.awareness.clientID) {
                    if (anchor.index < head.index) {
                        sendCursor(anchor.index, head.index - anchor.index)
                    } else {
                        sendCursor(head.index, anchor.index - head.index)
                    }
                }
            }
        }
    })
}
connectToServer()

var ypages = ydoc.getArray("pages")

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
            console.log("File changed via Y:", filename)

            let index = 0
            if (event.delta[0]["retain"]) {
                index = event.delta[0]["retain"]
                event.delta.shift()
            }

            if (event.delta[0]["insert"]) {
                let text = event.delta[0]["insert"]
                insertVim(filename, index, text)
            } else if (event.delta[0]["delete"]) {
                let length = event.delta[0]["delete"]
                deleteVim(filename, index, length)
            }
        }
    }
})

function insertVim(file, index, text) {
    if (client) {
        let message = ["insert", file, index, text].join("\t")
        console.log("Sending message:", message)
        client.socket.write(message)
    }
}

function deleteVim(file, index, length) {
    if (client) {
        let message = ["delete", file, index, length].join("\t")
        console.log("Sending message:", message)
        client.socket.write(message)
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

/*
const mutex = new Mutex()
async function syncFile(filename) {
    await mutex.runExclusive(() => {
        filename = path.join("output", filename)
        let cacheFilename = getCacheFile(filename)

        // Create the file if it doesn't exist.
        if (!fs.existsSync(filename)) {
            fs.writeFileSync(filename, "")
        }

        // Create the cache file if it doesn't exist.
        if (!fs.existsSync(cacheFilename)) {
            let dirname = path.dirname(cacheFilename)
            if (!fs.existsSync(dirname)) {
                fs.mkdir(dirname, {recursive: true})
            }
            let fileContent = fs.readFileSync(filename, "utf8")
            fs.writeFileSync(cacheFilename, fileContent)

            // Set modified time of cache file to that of other file.
            const fileModTime = fs.statSync(filename).mtimeMs
            const date = new Date(fileModTime)
            fs.utimesSync(cacheFilename, date, date)
        }

        const fileModTime = fs.statSync(filename).mtimeMs
        const cacheFileModTime = fs.statSync(cacheFilename).mtimeMs

        let page = findPage(filename)

        if (fileModTime > cacheFileModTime) {
            // File was changed externally.
            const fileContent = fs.readFileSync(filename, "utf8")
            const cacheFileContent = fs.readFileSync(cacheFilename, "utf8")
            const delta = getDeltaOperations(cacheFileContent, fileContent)
            if (delta.length > 0) {
                console.log("Applying delta", delta)
                page.get("content").applyDelta(delta)
            }
        }

        let newContent = page.get("content").toString()
        writeToFile(filename, newContent)
    })
}
*/

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

async function fullSync() {
    for (const page of ypages) {
        let title = page.get("title").toString()
        await syncFile(title)
        //let newContent = page.get("content").toString()
        //let oldContent = fs.readFileSync("output/" + title, "utf8")
        //if (oldContent !== newContent) {
        //    fs.writeFileSync("output/" + title, newContent, "utf8")
        //}
    }
    console.log("Full sync complete")
}

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
        console.log("received", d)

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
        }

        //sockets.forEach(function (client) {
        //    if (client === conn) {
        //        return
        //    }
        //    client.write(d)
        //})
    }
    function onConnClose() {
        console.log("connection from %s closed", remoteAddress)

        client = null
        //var pos = sockets.indexOf(conn)
        //if (pos > 0) {
        //    sockets.splice(pos, 1)
        //}
    }
    function onConnError(err) {
        console.log("Connection %s error: %s", remoteAddress, err.message)
    }
}

function sendCursor(index, length = 1) {
    if (client) {
        client.socket.write(["cursor", "filenameTBD", index, length].join("\t"))
    }
}
