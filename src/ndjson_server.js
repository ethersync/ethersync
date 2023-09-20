import net from "net"

export class NDJSONServer {
    constructor(port) {
        this.server = net.createServer()
        this.server.listen(port)
        this.connectionCallback = () => { }
        this.server.on("connection", (conn) => {
            conn.setEncoding("utf8")
            this.connectionCallback(conn)

            let buffer = ""
            conn.on("data", (chunk) => {
                buffer += chunk
                while (true) {
                    let i = buffer.indexOf("\n")
                    if (i == -1) {
                        break
                    }
                    let json = buffer.substr(0, i)
                    buffer = buffer.substr(i + 1)
                    let data = JSON.parse(json)
                    this.messageCallback(conn, data)
                }
            })

            conn.on("close", () => {
                this.closeCallback(conn)
            })
        })
    }
    onConnection(callback) {
        this.connectionCallback = callback
    }
    onMessage(callback) {
        this.messageCallback = callback
    }
    onClose(callback) {
        this.closeCallback = callback
    }
    write(client, message) {
        let data = JSON.stringify(message) + "\n"
        client.write(data)
    }
}
