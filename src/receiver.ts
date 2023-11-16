import {NDJSONServer} from "./ndjson_server.js"

// Represents a plaintext document.
class Document {
    content = ""

    insert(pos: number, text: string) {
        this.content =
            this.content.slice(0, pos) + text + this.content.slice(pos)
    }

    delete(pos: number, len: number) {
        this.content =
            this.content.slice(0, pos) + this.content.slice(pos + len)
    }

    print() {
        console.log("---")
        console.log(this.content)
        console.log("---")
    }
}

// Set up a server that an editor can connect to. When receiving the proper events, update the document.
var server = new NDJSONServer(9000)
var document = new Document()

server.onConnection(() => {
    console.log("new connection")
})

server.onMessage((message: any) => {
    console.log("received %j", message)
    let typ = message[0]
    if (typ == "insert") {
        document.insert(message[2], message[3])
    } else if (typ == "delete") {
        document.delete(message[2], message[3])
    }
    document.print()
})

server.onClose(() => {
    console.log("connection closed")
    document.content = ""
    document.print()
})
