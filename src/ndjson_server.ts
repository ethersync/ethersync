import {createServer, Server, Socket} from "net"

// A simple server that communicates with clients using newline-delimited JSON.
export class NDJSONServer {
    server: Server
    client?: Socket // TODO: support multiple clients

    connectionCallback = () => {}
    messageCallback = (_: any) => {}
    closeCallback = () => {}

    constructor(port: number) {
        this.server = createServer()
        this.server.listen(port)
        this.server.on("connection", (conn: Socket) => {
            conn.setEncoding("utf8")
            this.client = conn
            this.connectionCallback()

            let buffer = ""
            conn.on("data", (chunk: string) => {
                buffer += chunk
                while (true) {
                    let i = buffer.indexOf("\n")
                    if (i == -1) {
                        break
                    }
                    let json = buffer.slice(0, i)
                    buffer = buffer.slice(i + 1)
                    let data = JSON.parse(json)
                    this.messageCallback(data)
                }
            })

            conn.on("close", () => {
                this.closeCallback()
            })
        })
    }
    onConnection(callback: () => void) {
        this.connectionCallback = callback
    }
    onMessage(callback: (message: any) => void) {
        this.messageCallback = callback
    }
    onClose(callback: () => void) {
        this.closeCallback = callback
    }
    write(message: any) {
        let data = JSON.stringify(message) + "\n"
        this.client?.write(data)
    }
}
