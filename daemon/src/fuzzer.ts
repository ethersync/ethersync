import fs from "node:fs"
import {join} from "node:path"
import {tmpdir} from "node:os"
import cp from "child_process"

import {attach} from "neovim"
import {Daemon} from "./daemon"
import {charOffsetToUTF16CodeUnitOffset} from "./conversion"

const PAGE = "fuzzing"

function delay(time: number) {
    return new Promise((resolve) => setTimeout(resolve, time))
}

export class Fuzzer {
    // TODO: Give proper types.
    daemon: any = undefined
    nvim: any = undefined

    constructor() {}

    // length is in Unicode characters.
    randomString(length: number): string {
        let chars = ["x", "ö", "🥕", "字", " ", "\n"]
        let result = ""
        for (let i = 0; i < length; i++) {
            result += chars[Math.floor(Math.random() * chars.length)]
        }
        return result
    }

    randomDaemonEdit() {
        let ytext = this.daemon.findPage(PAGE).get("content")
        let content = ytext.toString()
        let documentLength = [...content].length // in Unicode characters
        if (Math.random() < 0.5) {
            let start = Math.floor(Math.random() * documentLength)
            let maxDeleteLength = documentLength - start
            if (maxDeleteLength > 0) {
                let length = 1 + Math.floor(Math.random() * (maxDeleteLength - 1))

                console.log(`daemon: delete(${start}, ${length}) in '${content}'`)

                let utf16Start = charOffsetToUTF16CodeUnitOffset(start, content)
                let utf16Length = charOffsetToUTF16CodeUnitOffset(start + length, content) - utf16Start
                ytext.delete(utf16Start, utf16Length)
            }
        } else {
            let start = Math.floor(Math.random() * documentLength)
            let length = Math.floor(Math.random() * 20)
            let text = this.randomString(length)

            console.log(`daemon: insert(${start}, ${text}) in ${content}`)

            let utf16Start = charOffsetToUTF16CodeUnitOffset(start, content)
            ytext.insert(utf16Start, text)
        }
    }

    async randomVimEdit() {
        let content = await this.vimContent()
        let documentLength = [...content].length
        if (Math.random() < 0.5) {
            let start = Math.floor(Math.random() * documentLength)
            let maxDeleteLength = documentLength - start
            if (maxDeleteLength > 0) {
                let length = 1 + Math.floor(Math.random() * (maxDeleteLength - 1))
                console.log(`editor: delete(${start}, ${length}) in ${content}`)
                this.nvim.request("nvim_exec_lua", [
                    "require('utils').delete(select(1, ...), select(2, ...))",
                    [start, length],
                ])
            }
        } else {
            let start = Math.floor(Math.random() * documentLength)
            let length = Math.floor(Math.random() * 20)
            let text = this.randomString(length)
            console.log(`editor: insert(${start}, ${text}) in ${content}`)
            this.nvim.request("nvim_exec_lua", [
                "require('utils').insert(select(1, ...), select(2, ...))",
                [start, text],
            ])
        }
    }

    async vimContent(): Promise<string> {
        return (await this.nvim.buffer.lines).join("\n")
    }

    daemonContent(): string {
        return this.daemon.findPage(PAGE).get("content").toString()
    }

    async vimGoOffline() {
        console.log("editor: going offline")
        await this.nvim.command("EthersyncGoOffline")
    }

    async vimGoOnline() {
        console.log("editor: going online")
        await this.nvim.command("EthersyncGoOnline")
    }

    async run() {
        let directory = fs.mkdtempSync(join(tmpdir(), "ethersync-"))
        const configDir = join(directory, ".ethersync")
        fs.mkdirSync(configDir)
        fs.writeFileSync(join(configDir, "config"), "etherwiki=")

        this.daemon = new Daemon(directory)
        await this.daemon.start()

        const nvim_proc = cp.spawn("nvim", ["--embed", "--headless"], {})
        this.nvim = attach({proc: nvim_proc})
        // Allow some wakeup time for vim.
        await delay(500)

        this.daemon.createPage(PAGE, "hello")
        this.daemon.writeAllPages()

        await this.nvim.command(`edit! ${this.daemon.directory}/${PAGE}`)
        await this.nvim.command("EthersyncReload")

        for (let i = 0; i < 5000; i++) {
            if (Math.random() < 0.5) {
                this.randomDaemonEdit()
            } else {
                await this.randomVimEdit()
            }

            let r = Math.random()
            if (r < 0.1) {
                await this.vimGoOffline()
            } else if (r < 0.2) {
                await this.vimGoOnline()
            }
        }

        this.vimGoOnline()
        await delay(1000)

        let vimContent = await this.vimContent()
        let daemonContent = this.daemonContent()

        console.log("vim:")
        console.log("-----------------------")
        console.log(vimContent)
        console.log("-----------------------")

        console.log("daemon:")
        console.log("-----------------------")
        console.log(daemonContent)
        console.log("-----------------------")

        if (vimContent !== daemonContent) {
            console.log("Fuzzing failed!")
        } else {
            console.log("Fuzzing successful!")
        }

        if (this.daemon.directory) {
            fs.rmSync(this.daemon.directory, {recursive: true})
        }

        return
    }
}
