import {expect, test} from "vitest"
import {
    UTF16CodeUnitOffsetToCharOffset,
    charOffsetToUTF16CodeUnitOffset,
    yjsDeltaToTextOp,
    textOpToYjsDelta,
    YjsDelta,
} from "./conversion"
import {insert, remove, TextOp} from "ot-text-unicode"
import {cloneDeep} from "lodash"

test("UTF16 to char conversion", () => {
    expect(UTF16CodeUnitOffsetToCharOffset(0, "")).toBe(0)
    expect(UTF16CodeUnitOffsetToCharOffset(2, "world")).toBe(2)
    expect(UTF16CodeUnitOffsetToCharOffset(2, "🥕world")).toBe(1)
    expect(UTF16CodeUnitOffsetToCharOffset(4, "🥕world")).toBe(3)
    expect(UTF16CodeUnitOffsetToCharOffset(5, "🥕wörld")).toBe(4)
    expect(UTF16CodeUnitOffsetToCharOffset(4, "⚽world")).toBe(4)
    expect(UTF16CodeUnitOffsetToCharOffset(5, "world")).toBe(5)

    expect(() => UTF16CodeUnitOffsetToCharOffset(6, "world")).toThrow()
})

test("char to UTF16 conversion", () => {
    expect(charOffsetToUTF16CodeUnitOffset(0, "")).toBe(0)
    expect(charOffsetToUTF16CodeUnitOffset(0, "world")).toBe(0)

    expect(charOffsetToUTF16CodeUnitOffset(4, "world")).toBe(4)
    expect(charOffsetToUTF16CodeUnitOffset(4, "wörld")).toBe(4)
    expect(charOffsetToUTF16CodeUnitOffset(4, "w⚽rld")).toBe(4)
    // the carrot has two UTF16 code units
    expect(charOffsetToUTF16CodeUnitOffset(4, "w🥕rld")).toBe(5)

    expect(charOffsetToUTF16CodeUnitOffset(5, "world")).toBe(5)
    expect(charOffsetToUTF16CodeUnitOffset(5, "wörld")).toBe(5)
    expect(charOffsetToUTF16CodeUnitOffset(5, "w⚽rld")).toBe(5)
    // the carrot has two UTF16 code units
    expect(charOffsetToUTF16CodeUnitOffset(5, "w🥕rld")).toBe(6)

    expect(() => charOffsetToUTF16CodeUnitOffset(6, "world")).toThrow()
})

type YjsOTOperationEquivalence = {
    string: string
    yjsDelta: YjsDelta
    otOperation: TextOp
}
const transfomationTestcases = [
    {string: "hello", yjsDelta: [{retain: 3}, {insert: "x"}], otOperation: insert(3, "x")},
    {string: "höllo", yjsDelta: [{retain: 3}, {insert: "x"}], otOperation: insert(3, "x")},
    {string: "h🥕llo", yjsDelta: [{retain: 3}, {insert: "x"}], otOperation: insert(2, "x")},
    {string: "hello", yjsDelta: [{retain: 3}, {delete: 2}], otOperation: remove(3, 2)},
    {string: "helöo", yjsDelta: [{retain: 3}, {delete: 2}], otOperation: remove(3, 2)},
    {string: "æöäüß", yjsDelta: [{retain: 3}, {delete: 2}], otOperation: remove(3, 2)},
    {string: "hel🥕o", yjsDelta: [{retain: 3}, {delete: 2}], otOperation: remove(3, 1)},
    {string: "h🥕🥒o", yjsDelta: [{retain: 3}, {delete: 2}], otOperation: remove(2, 1)},
    {string: "hello", yjsDelta: [{insert: "x"}, {retain: 1}, {insert: "y"}], otOperation: ["x", 1, "y"]},
    {
        string: "h🥕🥒ox",
        yjsDelta: [{retain: 3}, {delete: 2}, {retain: 1}, {delete: 1}],
        otOperation: [2, {d: 1}, 1, {d: 1}],
    },
    {
        string: "h🥕🥒ox",
        yjsDelta: [{retain: 3}, {insert: "ö"}, {retain: 1}, {delete: 1}],
        otOperation: [2, "ö", 1, {d: 1}],
    },
]

test.each(cloneDeep(transfomationTestcases))(
    "YjsDelta ($yjsDelta) -> TextOp ($otOperation) for $string",
    ({yjsDelta, otOperation, string}) => {
        expect(yjsDeltaToTextOp(yjsDelta, string)).toEqual(otOperation)
    },
)

test.each(cloneDeep(transfomationTestcases))(
    "TextOp ($otOperation) -> YjsDelta ($yjsDelta) for $string",
    ({yjsDelta, otOperation, string}) => {
        expect(textOpToYjsDelta(otOperation, string)).toEqual(yjsDelta)
    },
)
