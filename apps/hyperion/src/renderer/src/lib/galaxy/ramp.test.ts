import { describe, expect, it } from "vitest";

import {
  buildRamp,
  codeToLevel,
  ColourTokenError,
  parseHexColour,
  rampColour,
  rasterise,
} from "./ramp";

// The guide's --surface-0 and --text; literals are allowed in tests only.
const SURFACE_0 = parseHexColour("#05080d");
const TEXT = parseHexColour("#c8d6e5");

describe("parseHexColour", () => {
  it("reads a computed custom property, leading space and all", () => {
    expect(parseHexColour(" #c8d6e5")).toEqual({ r: 0xc8, g: 0xd6, b: 0xe5 });
    expect(parseHexColour("#C8D6E5\n")).toEqual({ r: 0xc8, g: 0xd6, b: 0xe5 });
  });

  it.each(["", "c8d6e5", "#c8d", "#c8d6e5ff", "rgb(1 2 3)", "#gg0000"])(
    "refuses %j with a ColourTokenError",
    (text) => {
      expect(() => parseHexColour(text)).toThrow(ColourTokenError);
    },
  );

  it("names the error", () => {
    expect(() => parseHexColour("blue")).toThrow(
      expect.objectContaining({ name: "ColourTokenError" }),
    );
  });
});

describe("buildRamp", () => {
  const ramp = buildRamp(SURFACE_0, TEXT);

  it("has 256 opaque RGBA levels", () => {
    expect(ramp).toHaveLength(1_024);
    for (let level = 0; level < 256; level += 1) {
      expect(ramp[level * 4 + 3]).toBe(255);
    }
  });

  it("starts exactly at --surface-0 and ends exactly at --text", () => {
    expect([...ramp.subarray(0, 3)]).toEqual([0x05, 0x08, 0x0d]);
    expect([...ramp.subarray(255 * 4, 255 * 4 + 3)]).toEqual([0xc8, 0xd6, 0xe5]);
  });

  it("brightens monotonically in every channel", () => {
    for (let level = 1; level < 256; level += 1) {
      for (let channel = 0; channel < 3; channel += 1) {
        expect(ramp[level * 4 + channel]).toBeGreaterThanOrEqual(
          ramp[(level - 1) * 4 + channel] ?? 256,
        );
      }
    }
  });

  it("makes level 1 differ from the background", () => {
    expect([...ramp.subarray(4, 7)]).not.toEqual([...ramp.subarray(0, 3)]);
  });

  it("gives a level as a CSS colour for legends", () => {
    expect(rampColour(ramp, 0)).toBe("rgb(5 8 13)");
    expect(rampColour(ramp, 255)).toBe("rgb(200 214 229)");
    expect(() => rampColour(ramp, 256)).toThrow(RangeError);
  });
});

describe("codeToLevel", () => {
  it.each([
    [0, 255, 0],
    [1, 255, 1],
    [128, 255, 128],
    [255, 255, 255],
    [0, 65_535, 0],
    [1, 65_535, 1],
    [2, 65_535, 1],
    // Either side of a rounding boundary: (130 − 1) ÷ 65,534 × 254 is 0.49997 and 0.50386 for 131.
    [130, 65_535, 1],
    [131, 65_535, 2],
    [65_535, 65_535, 255],
  ])("paints code %i of %i at level %i", (code, maxCode, level) => {
    expect(codeToLevel(code, maxCode)).toBe(level);
  });

  it("never paints a code above the floor as the background", () => {
    for (const maxCode of [255, 65_535]) {
      expect(codeToLevel(1, maxCode)).toBeGreaterThan(0);
    }
  });

  it("refuses a largest code below 2", () => {
    expect(() => codeToLevel(1, 1)).toThrow(RangeError);
  });
});

describe("rasterise", () => {
  const ramp = buildRamp(SURFACE_0, TEXT);

  it("paints a 2 × 2 map row by row from the top", () => {
    const rgba = rasterise(
      { widthPx: 2, heightPx: 2, codes: Uint8Array.from([255, 0, 1, 128]), maxCode: 255 },
      ramp,
    );

    expect(rgba).toHaveLength(16);
    expect([...rgba.subarray(0, 4)]).toEqual([0xc8, 0xd6, 0xe5, 255]);
    expect([...rgba.subarray(4, 8)]).toEqual([0x05, 0x08, 0x0d, 255]);
    expect([...rgba.subarray(8, 12)]).toEqual([...ramp.subarray(4, 8)]);
  });

  it("paints a 16-bit map on the same ramp", () => {
    const rgba = rasterise(
      { widthPx: 3, heightPx: 1, codes: Uint16Array.from([0, 1, 65_535]), maxCode: 65_535 },
      ramp,
    );

    expect([...rgba.subarray(0, 4)]).toEqual([...ramp.subarray(0, 4)]);
    expect([...rgba.subarray(4, 8)]).toEqual([...ramp.subarray(4, 8)]);
    expect([...rgba.subarray(8, 12)]).toEqual([...ramp.subarray(1_020, 1_024)]);
  });

  it("refuses codes that do not fill the map", () => {
    expect(() =>
      rasterise({ widthPx: 2, heightPx: 2, codes: Uint8Array.from([1, 2, 3]), maxCode: 255 }, ramp),
    ).toThrow(RangeError);
  });
});
