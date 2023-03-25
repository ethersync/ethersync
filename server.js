var net = require("net")

var sockets = []

var server = net.createServer()

server.on("connection", handleConnection)

server.listen(9000, function () {
    console.log("server listening to %j", server.address())
})

function handleConnection(conn) {
    var remoteAddress = conn.remoteAddress + ":" + conn.remotePort

    sockets.push(conn)

    console.log("new client connection from %s", remoteAddress)
    conn.setEncoding("utf8")

    conn.on("data", onConnData)
    conn.once("close", onConnClose)
    conn.on("error", onConnError)

    function onConnData(d) {
        console.log("connection data from %s: %j", remoteAddress, d)

        // Forward to other clients.
        sockets.forEach(function (client) {
            if (client === conn) {
                return
            }
            client.write(d)
        })
    }
    function onConnClose() {
        console.log("connection from %s closed", remoteAddress)

        var pos = sockets.indexOf(conn)
        if (pos > 0) {
            //broadcast(conn.name + " has left the chat service\n")
            sockets.splice(pos, 1)
        }
    }
    function onConnError(err) {
        console.log("Connection %s error: %s", remoteAddress, err.message)
    }
}
