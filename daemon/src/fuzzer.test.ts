import {expect, test} from "vitest"
import {Fuzzer} from "./fuzzer"

test("generates random strings of correct length", () => {
    const fuzzer = new Fuzzer()

    for (let i = 0; i < 100; i++) {
        expect([...fuzzer.randomString(i)].length).toBe(i)
    }
})
