import cp from "child_process"
import {attach, NeovimClient} from "neovim"
import {Daemon} from "./daemon"

const PAGE = "fuzzing"

function delay(time: number) {
    return new Promise((resolve) => setTimeout(resolve, time))
}

class Fuzzer {
    // TODO: Give proper types.
    daemon: any = undefined
    nvim: any = undefined

    constructor() {}

    randomString(length: number): string {
        let chars = ["a", "x", "ö", "🥕", "字"]
        let result = ""
        for (let i = 0; i < length; i++) {
            result += chars[Math.floor(Math.random() * chars.length)]
        }
        return result
    }

    randomDaemonEdit() {
        let documentLength = this.daemonContent().length
        if (Math.random() < 0.5) {
            let start = Math.floor(Math.random() * documentLength)
            let length = Math.floor(Math.random() * (documentLength - start))
            this.daemon.findPage(PAGE).get("content").delete(start, length)
        } else {
            let start = Math.floor(Math.random() * documentLength)
            let length = Math.floor(Math.random() * 10)
            let text = this.randomString(length)
            this.daemon.findPage(PAGE).get("content").insert(start, text)
        }
    }

    async randomVimEdit() {
        let documentLength = (await this.vimContent()).length
        if (Math.random() < 0.5) {
            let start = Math.floor(Math.random() * documentLength)
            let length = Math.floor(Math.random() * (documentLength - start))
            // TODO: Call the functions from the Vim plugin for convenience? Doesn't work yet.
            await this.nvim.request("nvim_exec_lua", [
                `require('utils').delete(${start}, ${length})`,
            ])
        } else {
            let start = Math.floor(Math.random() * documentLength)
            let length = Math.floor(Math.random() * 10)
            let text = this.randomString(length)
            await this.nvim.request("nvim_exec_lua", [
                `require('utils').insert(${start}, "${text}")`,
            ])
        }
    }

    async vimContent(): Promise<string> {
        return (await this.nvim.buffer.lines).join("\n")
    }

    daemonContent(): string {
        return this.daemon.findPage("fuzzing").get("content").toString()
    }

    async run() {
        this.daemon = new Daemon(false)
        await this.daemon.start()

        const nvim_proc = cp.spawn("nvim", ["--embed", "--headless"], {})
        this.nvim = attach({proc: nvim_proc})
        // Allow some wakeup time for vim.
        await delay(500)

        this.daemon.createPage(PAGE, "")
        this.daemon.pullAllPages()

        await this.nvim.command(`edit! output/${PAGE}`)

        for (let i = 0; i < 100; i++) {
            if (Math.random() < 0.5) {
                this.randomDaemonEdit()
            } else {
                this.randomVimEdit()
            }
        }

        let vimContent = await this.vimContent()
        let daemonContent = this.daemonContent()
        if (vimContent !== daemonContent) {
            console.log("Fuzzing failed!")
            console.log("vim:", vimContent)
            console.log("daemon:", daemonContent)
        } else {
            console.log("Fuzzing successful!")
        }
    }
}

await new Fuzzer().run()
