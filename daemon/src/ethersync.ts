import {Daemon} from "./daemon"

let dir = process.env.npm_config_directory || "./output"

let daemon = new Daemon(dir)
await daemon.start()
