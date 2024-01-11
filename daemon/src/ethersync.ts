import {Daemon} from "./daemon"

let dir = process.env.npm_config_directory

if (dir === undefined) {
    console.error("Please specify a directory to sync with --directory.")
    process.exit(1)
}

let daemon = new Daemon(dir)
await daemon.start()
