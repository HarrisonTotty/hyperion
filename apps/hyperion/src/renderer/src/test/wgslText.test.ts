import { describe, expect, it } from "vitest";

import { collapseWhitespace, wgslFunction } from "./wgslText";

/** A module of two functions, one with a nested block, its line ends `eol` and its indent `tab`. */
function moduleOf(eol: string, tab: string): string {
  return [
    "// The first.",
    "fn first(x : f32) -> f32 {",
    `${tab}if (x > 0.0) {`,
    `${tab}${tab}return x;`,
    `${tab}}`,
    `${tab}return 0.0;`,
    "}",
    "",
    "fn first_again() -> f32 {",
    `${tab}return first(1.0);`,
    "}",
    "",
  ].join(eol);
}

describe("WGSL read blind to its whitespace", () => {
  it("gives a function's body by its braces, the same under LF and CRLF and any indentation", () => {
    const bodies = [
      moduleOf("\n", "  "),
      moduleOf("\r\n", "  "),
      moduleOf("\r\n", "\t"),
      moduleOf("\n", "    "),
    ].map((source) => [wgslFunction(source, "first"), wgslFunction(source, "first_again")]);
    expect(bodies).toEqual(
      bodies.map(() => ["{ if (x > 0.0) { return x; } return 0.0; }", "{ return first(1.0); }"]),
    );
  });

  it("collapses each run of whitespace, line ends included, to one space", () => {
    expect(collapseWhitespace("a  b\r\n\tc\n\n d")).toBe("a b c d");
  });

  it("refuses a function the source does not have, or does not close", () => {
    expect(() => wgslFunction(moduleOf("\n", "  "), "second")).toThrow(/no function second/u);
    expect(() => wgslFunction("fn open() {\n  return;\n", "open")).toThrow(/open is not closed/u);
  });
});
