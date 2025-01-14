# SPDX-FileCopyrightText: NONE
#
# SPDX-License-Identifier: CC0-1.0

import json
import argparse
from pathlib import Path

parser = argparse.ArgumentParser(description="Generate JSON-RPC message.")
parser.add_argument(
    "--message-type",
    choices=["open", "edit"],
    help='The type of message to generate: "open" or "edit".',
    default="open",
)
parser.add_argument("file", help="The file which this message is for")
args = parser.parse_args()
uri = "file://{}".format(Path(args.file).resolve())

messages = {
    "open": {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "open",
        "params": {
            "uri": uri,
        },
    },
    "edit": {
        "jsonrpc": "2.0",
        "method": "edit",
        "id": 2,
        "params": {
            "uri": uri,
            "revision": 0,
            "delta": [
                {
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 0, "character": 0},
                    },
                    "replacement": "hello, world",
                }
            ],
        },
    },
}

# Convert the dictionary to a JSON string
message_json = json.dumps(messages[args.message_type])

# Print the Content-Length and the JSON message
print(f"Content-Length: {len(message_json)}\r\n\r\n{message_json}")
