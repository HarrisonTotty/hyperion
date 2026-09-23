import { describe, expect, it } from "vitest";

import { formatBodyId, hexToU64, isBodyId, isHex64, parseBodyId, u64ToHex } from "./hex";

describe("u64 hex form", () => {
  it.each([
    [0n, "0000000000000000"],
    [1234n, "00000000000004d2"],
    [0x0123_4567_89ab_cdefn, "0123456789abcdef"],
    [0xffff_ffff_ffff_ffffn, "ffffffffffffffff"],
  ])("writes %s as %s and reads it back", (value, hex) => {
    expect(u64ToHex(value)).toBe(hex);
    expect(hexToU64(hex)).toBe(value);
    expect(isHex64(hex)).toBe(true);
  });

  it("keeps values above 2^53 exact", () => {
    const value = 2n ** 53n + 1n;

    expect(hexToU64(u64ToHex(value))).toBe(value);
  });

  it.each([
    ["upper case", "00000000000004D2"],
    ["too short", "4d2"],
    ["too long", "000000000000004d2"],
    ["a 0x prefix", "0x00000000000004d2"],
    ["a 0x prefix within 16 characters", "0x000000000004d2"],
    ["a sign", "-000000000000001"],
    ["whitespace", " 0000000000004d2"],
    ["a non-hex letter", "000000000000004g"],
    ["the empty string", ""],
  ])("refuses a string with %s", (_reason, text) => {
    expect(isHex64(text)).toBe(false);
    expect(() => hexToU64(text)).toThrow(SyntaxError);
  });

  it.each([-1n, 2n ** 64n])("refuses to format %s, which is not a u64", (value) => {
    expect(() => u64ToHex(value)).toThrow(RangeError);
  });
});

describe("body ID form", () => {
  // The same IDs as the Rust wire-form tests of `BodyIdHex`.
  it.each([
    ["0200080020000000.0100", "0200080020000000", 0x0100],
    ["0000000000000000.0000", "0000000000000000", 0],
    ["ffffffffffffffff.ffff", "ffffffffffffffff", 0xffff],
    ["8000000000000000.8000", "8000000000000000", 0x8000],
    ["0123456789abcdef.0a0f", "0123456789abcdef", 0x0a0f],
  ])("reads %s as system %s, body index %s, and writes it back", (text, system, bodyIndex) => {
    expect(isBodyId(text)).toBe(true);
    expect(parseBodyId(text)).toEqual({ system, bodyIndex });
    expect(formatBodyId({ system, bodyIndex })).toBe(text);
  });

  it("writes every body index as four digits and reads it back", () => {
    const system = "0200080020000000";
    for (const bodyIndex of [0, 1, 0x0f, 0x10, 0xff, 0x100, 0x0fff, 0x1000, 0xffff]) {
      const text = formatBodyId({ system, bodyIndex });

      expect(text).toHaveLength(21);
      expect(parseBodyId(text)).toEqual({ system, bodyIndex });
    }
  });

  it.each([
    ["the empty string", ""],
    ["no body index", "0200080020000000"],
    ["an empty body index", "0200080020000000."],
    ["a short body index", "0200080020000000.100"],
    ["a long body index", "0200080020000000.00100"],
    ["a short system", "200080020000000.0100"],
    ["a colon for the full stop", "0200080020000000:0100"],
    ["the full stop out of place", "02000800200000000.100"],
    ["upper case in the index", "0200080020000000.010A"],
    ["upper case in the system", "020008002000000A.0100"],
    ["a 0x prefix on the index", "0200080020000000.0x10"],
    ["a 0x prefix on the system", "0x00080020000000.0100"],
    ["whitespace", "0200080020000000. 100"],
    ["a sign", "0200080020000000.-100"],
    ["a trailing newline", "0200080020000000.0100\n"],
    ["a non-hex letter", "020008002000000g.0100"],
    ["accented letters", "0200080020000000.éé"],
  ])("refuses a string with %s", (_reason, text) => {
    expect(isBodyId(text)).toBe(false);
    expect(() => parseBodyId(text)).toThrow(SyntaxError);
  });

  it("refuses to write a body of a malformed system", () => {
    expect(() => formatBodyId({ system: "200080020000000", bodyIndex: 0 })).toThrow(SyntaxError);
  });

  it.each([-1, 65_536, 1.5, Number.NaN, Number.POSITIVE_INFINITY])(
    "refuses to write body index %s, which is not a u16",
    (bodyIndex) => {
      expect(() => formatBodyId({ system: "0200080020000000", bodyIndex })).toThrow(RangeError);
    },
  );
});
