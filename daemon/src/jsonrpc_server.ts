import {createServer, Server, Socket} from "net"

// A simple server that communicates with clients using JSON-RPC over TCP.
export class JSONRPCServer {
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
                    // For a complete message, we expect a Content-Length: <int> header, a \r\n\r\n, and some content of the given length.
                    // Check that the header is there, and if we have enough content.
                    // Shorten the buffer to remove the message we just parsed.
                    // Then, parse JSON and call the message callback.

                    let headerEnd = buffer.indexOf("\r\n\r\n")
                    if (headerEnd < 0) {
                        break
                    }
                    let header = buffer.slice(0, headerEnd)
                    let match = header.match(/Content-Length: (\d+)/)
                    if (!match) {
                        break
                    }
                    let contentLength = parseInt(match[1])
                    let messageLength = headerEnd + 4 + contentLength
                    if (buffer.length < messageLength) {
                        break
                    }
                    let json = buffer.slice(headerEnd + 4, messageLength)
                    buffer = buffer.slice(messageLength)

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
        let payload = JSON.stringify(message)
        let header = `Content-Length: ${payload.length}\r\n\r\n`
        this.client?.write(header)
        this.client?.write(payload)
    }
}
