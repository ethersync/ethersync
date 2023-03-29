const Y = require("yjs")
const Yws = require("y-websocket")
const fs = require("fs")
const {watch} = require("node:fs/promises")
const diff = require("diff")

let didFullSync = false

var ydoc = new Y.Doc()

var provider = new Yws.WebsocketProvider(
    "wss://etherwiki.blinry.org",
    "playground",
    ydoc,
    {WebSocketPolyfill: require("ws")}
)
provider.awareness.setLocalStateField("user", {
    name: "anonymous",
    color: "#ff00ff",
})

var ypages = ydoc.getArray("pages")

ypages.observeDeep(function (events) {
    fullSync()
    //if (!didFullSync) {
    //    fullSync()
    //    didFullSync = true
    //} else {
    //    for (const event of events) {
    //        let key = event.path[event.path.length - 1]
    //        if (key == "content") {
    //            file = event.target.parent.get("title").toString()

    //            console.log(event.delta)

    //            let index = 0
    //            if (event.delta[0]["retain"]) {
    //                index = event.delta[0]["retain"]
    //                event.delta.shift()
    //            }

    //            if (event.delta[0]["insert"]) {
    //                let text = event.delta[0]["insert"]
    //                insertFS(file, index, text)
    //            } else if (event.delta[0]["delete"]) {
    //                let length = event.delta[0]["delete"]
    //                deleteFS(file, index, length)
    //            }
    //        } else {
    //            console.log("Unhandled event", key)
    //        }
    //    }
    //}
})
;(async () => {
    let watcher = watch("output")
    for await (const event of watcher) {
        console.log(event)
        if (event.eventType == "change") {
            let file = event.filename
            if (file[file.length - 1] == "~") {
                continue
            }
            console.log("File changed:", file)

            let newContent = fs.readFileSync("output/" + file, "utf8")
            let oldContent = ypages
                .toArray()
                .find((page) => {
                    return page.get("title").toString() == file
                })
                .get("content")
                .toString()

            let parts = diff.diffChars(oldContent, newContent)

            let index = 0
            for (const part of parts) {
                if (part.added) {
                    insertY(file, index, part.value)
                } else if (part.removed) {
                    deleteY(file, index, part.value.length)
                }
                index += part.value.length
            }
        }
    }
})()

function fullSync() {
    for (const page of ypages) {
        let title = page.get("title").toString()
        let newContent = page.get("content").toString()
        let oldContent = fs.readFileSync("output/" + title, "utf8")
        if (oldContent !== newContent) {
            fs.writeFileSync("output/" + title, newContent, "utf8")
        }
    }
    console.log("Full sync complete")
}

function insertFS(file, index, text) {
    console.log("Inserting", text, "at", index, "in", file)

    let content = fs.readFileSync("output/" + file, "utf8")
    content = content.slice(0, index) + text + content.slice(index)
    fs.writeFileSync("output/" + file, content, "utf8")
}

function deleteFS(file, index, length) {
    console.log("Deleting", length, "characters at", index, "in", file)

    let content = fs.readFileSync("output/" + file, "utf8")
    content = content.slice(0, index) + content.slice(index + length)
    fs.writeFileSync("output/" + file, content, "utf8")
}

function insertY(file, index, text) {
    console.log("Y-Inserting", text, "at", index, "in", file)

    let page = ypages.toArray().find((page) => {
        return page.get("title").toString() == file
    })
    page.get("content").insert(index, text)
}

function deleteY(file, index, length) {
    console.log("Y-Deleting", length, "characters at", index, "in", file)

    let page = ypages.toArray().find((page) => {
        return page.get("title").toString() == file
    })
    page.get("content").delete(index, length)
}
