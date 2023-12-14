import {expect, test, afterAll, beforeAll} from "vitest"
import cp from "child_process"
import {attach, NeovimClient} from "neovim"
import {Daemon} from "./daemon"

let daemon = new Daemon(false)
let nvim: NeovimClient

function delay(time: number) {
    return new Promise((resolve) => setTimeout(resolve, time))
}
beforeAll(async () => {
    await daemon.start()

    daemon.createPage("integrationtest", "hallo")
    daemon.pullAllPages()

    const nvim_proc = cp.spawn(
        "nvim",
        ["--embed", "--headless", "output/integrationtest"],
        {},
    )

    nvim = await attach({proc: nvim_proc})
    // Allow some wakeup time for vim.
    await delay(500)
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
    const buf = await nvim.buffer
    const lines = await buf.lines

    await nvim.input("ggdGihalloh")
    await nvim.request("nvim_buf_set_text", [0, 0, 0, 0, 1, ["x"]])

    const newLines = await buf.lines
    expect(newLines).toEqual(["xallo"])
})
