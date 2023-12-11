import {createServer, Server, Socket} from "net"

// A simple server that communicates with clients using JSON messages over TCP.
// This follows the message format used by JSON-RPC.
export class JSONServer {
    server: Server
    client?: Socket // TODO: support multiple clients

    connectionCallback = () => {}
    messageCallback = (_: any) => {}
    closeCallback = () => {}

    constructor(port: number) {
        this.server = createServer()
        this.server.listen(port)

        // Called when the client sends us a JSON message.
        this.server.on("connection", (conn: Socket) => {
            conn.setEncoding("utf8")
            this.client = conn
            this.connectionCallback()

            let buffer = Buffer.alloc(0)

            conn.on("data", (chunk: string) => {
                let chunkBuffer = Buffer.from(chunk, "utf8")
                buffer = Buffer.concat([buffer, chunkBuffer])

                while (true) {
                    // For a complete message, we expect a Content-Length: <int> header, a \r\n\r\n, and some content of the given length.

                    let headerEnd = buffer.indexOf("\r\n\r\n")
                    if (headerEnd < 0) {
                        break
                    }
                    let header = buffer.slice(0, headerEnd)
                    let headerString = header.toString("utf8")
                    let match = headerString.match(/Content-Length: (\d+)/)
                    if (!match) {
                        break
                    }

                    // Note: This length is in UTF-8 bytes!
                    let contentLength = parseInt(match[1])
                    let messageLength = headerEnd + 4 + contentLength
                    let bufferLength = buffer.length
                    if (bufferLength < messageLength) {
                        break
                    }

                    let json = buffer.slice(headerEnd + 4, messageLength)
                    buffer = buffer.slice(messageLength)

                    let jsonString = json.toString("utf8")
                    let data = JSON.parse(jsonString)
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

    // Send a JSON message to the client.
    write(message: any) {
        let payload = JSON.stringify(message)
        let length = Buffer.byteLength(payload, "utf8")
        let header = `Content-Length: ${length}\r\n\r\n`

        this.client?.write(header)
        this.client?.write(payload)
    }
}
