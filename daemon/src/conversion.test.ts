import {expect, test} from "vitest"
import {UTF16CodeUnitOffsetToCharOffset, charOffsetToUTF16CodeUnitOffset, yjsDeltaToTextOp} from "./conversion"
import {insert, remove, TextOp} from "ot-text-unicode"

test("UTF16 to char conversion", () => {
    expect(UTF16CodeUnitOffsetToCharOffset(0, "")).toBe(0)
    expect(UTF16CodeUnitOffsetToCharOffset(2, "world")).toBe(2)
    expect(UTF16CodeUnitOffsetToCharOffset(2, "🥕world")).toBe(1)
    expect(UTF16CodeUnitOffsetToCharOffset(4, "🥕world")).toBe(3)
    expect(UTF16CodeUnitOffsetToCharOffset(5, "🥕wörld")).toBe(4)
    expect(UTF16CodeUnitOffsetToCharOffset(4, "⚽world")).toBe(4)
    expect(UTF16CodeUnitOffsetToCharOffset(5, "world")).toBe(5)

    expect(() => UTF16CodeUnitOffsetToCharOffset(6, "world")).toThrowError("Out of bounds")
})

test("char to UTF16 conversion", () => {
    expect(charOffsetToUTF16CodeUnitOffset(0, "")).toBe(0)
    expect(charOffsetToUTF16CodeUnitOffset(0, "world")).toBe(0)
    expect(charOffsetToUTF16CodeUnitOffset(4, "world")).toBe(4)
    expect(charOffsetToUTF16CodeUnitOffset(4, "wörld")).toBe(4)
    expect(charOffsetToUTF16CodeUnitOffset(4, "w⚽rld")).toBe(4)
    // the carrot has two UTF16 code units
    expect(charOffsetToUTF16CodeUnitOffset(4, "w🥕rld")).toBe(5)

    expect(() => charOffsetToUTF16CodeUnitOffset(6, "world")).toThrowError("Out of bounds")
})

test("transforms from Yjs update to OT update", () => {
    expect(yjsDeltaToTextOp([{retain: 3}, {insert: "x"}], "hello")).toEqual(insert(3, "x"))
    expect(yjsDeltaToTextOp([{retain: 3}, {insert: "x"}], "höllo")).toEqual(insert(3, "x"))
    expect(yjsDeltaToTextOp([{retain: 3}, {insert: "x"}], "h🥕llo")).toEqual(insert(2, "x"))
    expect(yjsDeltaToTextOp([{retain: 3}, {delete: 2}], "hello")).toEqual(remove(3, 2))
    expect(yjsDeltaToTextOp([{retain: 3}, {delete: 2}], "helöo")).toEqual(remove(3, 2))
    expect(yjsDeltaToTextOp([{retain: 3}, {delete: 2}], "hel🥕o")).toEqual(remove(3, 1))
    expect(yjsDeltaToTextOp([{retain: 3}, {delete: 2}], "h🥕🥕o")).toEqual(remove(2, 1))
})
