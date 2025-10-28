# SPDX-FileCopyrightText: NONE
#
# SPDX-License-Identifier: CC0-1.0

#!/usr/bin/python

# This script listens on a specific `socket_path`, accepts any "open", and then sends a specific edit.
# It can be used to debug plugin behavior.

import socket
import os, os.path
import json

socket_path = "/tmp/teamtype-test/.teamtype/socket"

# Returns the next JSON-RPC object.
def read_line(conn):
    bytes = conn.recv(1024)
    lines = bytes.strip().split(b"\n")
    result = json.loads(lines[0])
    print("Got JSON:", result)
    return result


def send(conn, json_):
    message_json = json.dumps(json_)
    print(message_json)
    conn.send(message_json.encode("utf-8"))
    conn.send(b"\n")

if os.path.exists(socket_path):
    os.remove(socket_path)

server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(socket_path)
while True:
    server.listen()
    conn, addr = server.accept()

    open = read_line(conn)
    print(open)
    response = {"jsonrpc": "2.0", "id": open["id"], "result": "success"}
    send(conn, response)
    the_beautiful_edit = {
        "jsonrpc": "2.0",
        "method": "edit",
        "params": {
            "uri": open["params"]["uri"],
            "revision": 0,
            "delta": [
                {
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 0, "character": 1},
                    },
                    "replacement": "AaA",
                }
            ],
        },
    }
    send(conn, the_beautiful_edit)
    while True:
        read_line(conn)
