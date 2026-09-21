import { describe, expect, it } from "vitest";

import { formatHex64, type ParsedSeed, parseSeedHex } from "./seed";

describe("parseSeedHex", () => {
  it.each<[string, ParsedSeed]>([
    ["beef", { ok: true, seed: "000000000000beef" }],
    ["BEEF", { ok: true, seed: "000000000000beef" }],
    ["4D2", { ok: true, seed: "00000000000004d2" }],
    ["  4d2  ", { ok: true, seed: "00000000000004d2" }],
    ["0", { ok: true, seed: "0000000000000000" }],
    ["000beef", { ok: true, seed: "000000000000beef" }],
    ["ffffffffffffffff", { ok: true, seed: "ffffffffffffffff" }],
    ["0000000000000001", { ok: true, seed: "0000000000000001" }],
    ["", { ok: false }],
    ["   ", { ok: false }],
    ["10000000000000000", { ok: false }],
    ["00000000000000001", { ok: false }],
    ["g", { ok: false }],
    ["be ef", { ok: false }],
    ["0xbeef", { ok: false }],
    ["-1", { ok: false }],
  ])("reads %j as %j", (text, parsed) => {
    expect(parseSeedHex(text)).toEqual(parsed);
  });
});

describe("formatHex64", () => {
  it("writes the digits in upper case", () => {
    expect(formatHex64("000000000000beef")).toBe("000000000000BEEF");
  });
});
