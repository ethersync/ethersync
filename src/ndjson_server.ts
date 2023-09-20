import { createServer } from "net"

export class NDJSONServer {
    server: any
    connectionCallback = (_: any) => { }
    messageCallback = (_: any, __: any) => { }
    closeCallback = (_: any) => { }

    constructor(port: number) {
        this.server = createServer()
        this.server.listen(port)
        this.server.on("connection", (conn: any) => {
            conn.setEncoding("utf8")
            this.connectionCallback(conn)

            let buffer = ""
            conn.on("data", (chunk: string) => {
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
    onConnection(callback: any) {
        this.connectionCallback = callback
    }
    onMessage(callback: any) {
        this.messageCallback = callback
    }
    onClose(callback: any) {
        this.closeCallback = callback
    }
    write(client: any, message: any) {
        let data = JSON.stringify(message) + "\n"
        client.write(data)
    }
}
