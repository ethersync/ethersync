import {expect, test} from "vitest"
import {
    UTF16CodeUnitOffsetToCharOffset,
    charOffsetToUTF16CodeUnitOffset,
} from "./unicode_conversion"

test("UTF16 to char conversion", () => {
    expect(UTF16CodeUnitOffsetToCharOffset(0, "")).toBe(0)
    expect(UTF16CodeUnitOffsetToCharOffset(2, "world")).toBe(2)
    expect(UTF16CodeUnitOffsetToCharOffset(2, "🥕world")).toBe(1)
    expect(UTF16CodeUnitOffsetToCharOffset(4, "🥕world")).toBe(3)
    expect(UTF16CodeUnitOffsetToCharOffset(5, "🥕wörld")).toBe(4)
    expect(UTF16CodeUnitOffsetToCharOffset(4, "⚽world")).toBe(4)
    expect(UTF16CodeUnitOffsetToCharOffset(5, "world")).toBe(5)

    expect(() => UTF16CodeUnitOffsetToCharOffset(6, "world")).toThrowError(
        "Out of bounds",
    )
})

test("char to UTF16 conversion", () => {
    expect(charOffsetToUTF16CodeUnitOffset(0, "")).toBe(0)
    expect(charOffsetToUTF16CodeUnitOffset(0, "world")).toBe(0)
    expect(charOffsetToUTF16CodeUnitOffset(4, "world")).toBe(4)
    expect(charOffsetToUTF16CodeUnitOffset(4, "wörld")).toBe(4)
    expect(charOffsetToUTF16CodeUnitOffset(4, "w⚽rld")).toBe(4)
    // the carrot has two UTF16 code units
    expect(charOffsetToUTF16CodeUnitOffset(4, "w🥕rld")).toBe(5)

    expect(() => charOffsetToUTF16CodeUnitOffset(6, "world")).toThrowError(
        "Out of bounds",
    )
})
