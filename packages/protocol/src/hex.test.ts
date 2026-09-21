import { describe, expect, it } from "vitest";

import { hexToU64, isHex64, u64ToHex } from "./hex";

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
