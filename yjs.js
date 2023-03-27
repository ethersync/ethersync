const Y = require("yjs")
const Yws = require("y-websocket")
const fs = require("fs")

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
    if (!didFullSync) {
        fullSync()
        didFullSync = true
    } else {
        for (const event of events) {
            let key = event.path[event.path.length - 1]
            if (key == "content") {
                file = event.target.parent.get("title").toString()

                console.log(event.delta)

                let index = 0
                if (event.delta[0]["retain"]) {
                    index = event.delta[0]["retain"]
                    event.delta.shift()
                }

                if (event.delta[0]["insert"]) {
                    let text = event.delta[0]["insert"]
                    insertIn(file, index, text)
                } else if (event.delta[0]["delete"]) {
                    let length = event.delta[0]["delete"]
                    deleteIn(file, index, length)
                }
            } else {
                console.log("Unhandled event", key)
            }
        }
    }
})

function fullSync() {
    for (const page of ypages) {
        let title = page.get("title").toString()
        let content = page.get("content").toString()

        fs.writeFileSync("output/" + title, content, "utf8")
    }
}

function insertIn(file, index, text) {
    console.log("Inserting", text, "at", index, "in", file)

    let content = fs.readFileSync("output/" + file, "utf8")
    content = content.slice(0, index) + text + content.slice(index)
    fs.writeFileSync("output/" + file, content, "utf8")
}

function deleteIn(file, index, length) {
    console.log("Deleting", length, "characters at", index, "in", file)

    let content = fs.readFileSync("output/" + file, "utf8")
    content = content.slice(0, index) + content.slice(index + length)
    fs.writeFileSync("output/" + file, content, "utf8")
}
