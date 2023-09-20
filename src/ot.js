var sockets = []
import {NDJSONServer} from "./ndjson_server.js"

var server = new NDJSONServer(9000)

server.onConnection((conn) => {
    sockets.push(conn)
    console.log("new client connection")
})

server.onMessage((conn, message) => {
    console.log("received %j", message)
    sockets.forEach(function (client) {
        if (client === conn) {
            return
        }
        server.write(client, message)
    })
})

server.onClose((conn) => {
    console.log("connection closed")

    var pos = sockets.indexOf(conn)
    if (pos > 0) {
        sockets.splice(pos, 1)
    }
})
