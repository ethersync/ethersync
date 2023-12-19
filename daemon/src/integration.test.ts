import {expect, test, afterAll, beforeAll, beforeEach} from "vitest"
import cp from "child_process"
import {attach, NeovimClient} from "neovim"
import {Daemon} from "./daemon"

let daemon = new Daemon()
let nvim: NeovimClient

function delay(time: number) {
    return new Promise((resolve) => setTimeout(resolve, time))
}

beforeAll(async () => {
    await daemon.start()

    const nvim_proc = cp.spawn("nvim", ["--embed", "--headless"], {})

    nvim = await attach({proc: nvim_proc})
    // Allow some wakeup time for vim.
    await delay(500)
})

beforeEach(async () => {
    daemon.dropPage("integrationtest")
    daemon.createPage("integrationtest", "hallo")
    daemon.pullAllPages()
    await nvim.command("edit! output/integrationtest")
    await nvim.command("EthersyncReload")
    await delay(100)
})

afterAll(async () => {
    nvim.quit()
    // nvim_proc.disconnect()
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
