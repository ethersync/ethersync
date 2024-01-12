import fs from "node:fs"
import {join} from "node:path"
import {tmpdir} from "node:os"

import {expect, test, afterAll, beforeAll, beforeEach} from "vitest"
import cp from "child_process"
import {attach, NeovimClient} from "neovim"

import {Daemon} from "./daemon"

let daemon: Daemon
let nvim: NeovimClient

function delay(time: number) {
    return new Promise((resolve) => setTimeout(resolve, time))
}

beforeAll(async () => {
    let directory = fs.mkdtempSync(join(tmpdir(), "ethersync-"))
    const configDir = join(directory, ".ethersync")
    fs.mkdirSync(configDir)
    // TODO: replace by local test instance once we have integrated the server component.
    fs.writeFileSync(join(configDir, "config"), "etherwiki=https://etherwiki.blinry.org#playground")
    daemon = new Daemon(directory)

    await daemon.start()

    const nvim_proc = cp.spawn("nvim", ["--embed", "--headless"], {})

    nvim = await attach({proc: nvim_proc})
    // Allow some wakeup time for vim.
    await delay(500)
})

beforeEach(async () => {
    daemon.dropPage("integrationtest")
    daemon.createPage("integrationtest", "hallo")
    daemon.writeAllPages()
    await nvim.command(`edit! ${daemon.directory}/integrationtest`)
    await nvim.command("EthersyncReload")
    await delay(100)
})

afterAll(async () => {
    nvim.quit()
    // nvim_proc.disconnect()
    if (daemon.directory) {
        fs.rmSync(daemon.directory, {recursive: true})
    }
})

test("can make edits from ydoc", async () => {
    daemon.findPage("integrationtest").get("content").insert(0, "cool")
    // await delay(500)
    expect(await nvim.buffer.lines).toEqual(["coolhallo"])
})

test("can make edits in nvim", async () => {
    // await nvim.input("ggdGihalloh")
    await nvim.request("nvim_buf_set_text", [0, 0, 0, 0, 1, ["x"]])

    const newLines = await nvim.buffer.lines
    expect(newLines).toEqual(["xallo"])
})

test("can execute Lua in NVim", async () => {
    expect(await nvim.request("nvim_exec_lua", ["return 1+1", []])).toEqual(2)

    expect(await nvim.request("nvim_exec_lua", ["return vim.api.nvim_get_color_by_name('Pink')", []])).toEqual(0xffc0cb)

    await nvim.request("nvim_exec_lua", ["require('utils').insert(select(1, ...), select(2, ...))", [1, "bla"]])
    expect(await nvim.buffer.lines).toEqual(["hblaallo"])

    await nvim.request("nvim_exec_lua", ["require('utils').delete(select(1, ...), select(2, ...))", [2, 3]])
    expect(await nvim.buffer.lines).toEqual(["hbllo"])
})

test("can make edits in parallel", async () => {
    nvim.command("EthersyncGoOffline")
    await nvim.request("nvim_buf_set_text", [0, 0, 2, 0, 2, ["x"]])
    expect(await nvim.buffer.lines).toEqual(["haxllo"])

    daemon.findPage("integrationtest").get("content").delete(1, 3)
    nvim.command("EthersyncGoOnline")
    await delay(500)
    expect(await nvim.buffer.lines).toEqual(["hxo"])
})

test("can insert at end of file in Vim", async () => {
    await nvim.request("nvim_exec_lua", ["require('utils').insert(select(1, ...), select(2, ...))", [5, "!"]])

    await delay(500)

    let daemonContent = daemon.findPage("integrationtest").get("content").toString()

    expect(daemonContent).toEqual("hallo!\n") // The newline is there because of Vim's 'fixeol' setting.
})
