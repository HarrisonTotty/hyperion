import { describe, expect, it } from "vitest";

import {
  MAP_RESOLUTIONS_PX,
  mapResolutionFor,
  paintLevels,
  reducedLevels,
  turnClockwise,
} from "./mapPicture";
import { buildRamp, codeToLevel, parseHexColour } from "./ramp";

/** An 8-bit map of the given size, every pixel `fill`, as `decodeDensityMap` gives it. */
function aMap(widthPx: number, heightPx: number, fill: number) {
  return {
    widthPx,
    heightPx,
    codes: new Uint8Array(widthPx * heightPx).fill(fill),
    maxCode: 255,
  };
}

describe("mapResolutionFor", () => {
  it("offers the protocol's four widths", () => {
    expect(MAP_RESOLUTIONS_PX).toEqual([128, 256, 512, 1_024]);
  });

  it.each([
    [1, 128],
    [128, 128],
    [129, 256],
    [244, 256],
    [256, 256],
    [320, 512],
    [512, 512],
    [733, 1_024],
    [1_024, 1_024],
    [1_600, 1_024],
  ])("asks for the narrowest map at least %i device pixels wide: %i", (backingPx, resolution) => {
    expect(mapResolutionFor(backingPx)).toBe(resolution);
  });
});

describe("turnClockwise", () => {
  it("puts the raster's top row down the right-hand side and its left column along the top", () => {
    // 3 wide, 2 tall:  1 2 3
    //                  4 5 6
    const turned = turnClockwise({
      widthPx: 3,
      heightPx: 2,
      codes: Uint8Array.from([1, 2, 3, 4, 5, 6]),
      maxCode: 255,
    });

    // 2 wide, 3 tall:  4 1
    //                  5 2
    //                  6 3
    expect(turned.widthPx).toBe(2);
    expect(turned.heightPx).toBe(3);
    expect([...turned.codes]).toEqual([4, 1, 5, 2, 6, 3]);
  });

  it("keeps 16-bit codes 16-bit", () => {
    const turned = turnClockwise({
      widthPx: 1,
      heightPx: 1,
      codes: Uint16Array.from([65_535]),
      maxCode: 65_535,
    });

    expect(turned.codes).toBeInstanceOf(Uint16Array);
    expect([...turned.codes]).toEqual([65_535]);
  });
});

describe("reducedLevels", () => {
  it("gives each code its own level when nothing is reduced", () => {
    const codes = Uint8Array.from({ length: 256 }, (_, code) => code);

    const levels = reducedLevels({ widthPx: 16, heightPx: 16, codes, maxCode: 255 }, 5, 16, 16);

    expect([...levels]).toEqual([...codes].map((code) => codeToLevel(code, 255)));
  });

  it("keeps a one-pixel-thick disc row of a 512 map when it is drawn 244 device pixels wide", () => {
    // An edge-on map at its floor everywhere but one bright row, the young disc, at the ceiling.
    const map = aMap(512, 256, 1);
    const discRow = 128;
    map.codes.fill(255, discRow * 512, (discRow + 1) * 512);

    const levels = reducedLevels(map, 7, 244, 122);

    const rowLevels = Array.from({ length: 122 }, (_, row) => levels[row * 244 + 100] ?? 0);
    const brightest = Math.max(...rowLevels);
    // The disc fills under half of one reduced row (244/512 of it); a 0.32 dex dimming at 7 dex
    // over 254 levels costs it about 12 levels, and it can never vanish into the floor.
    expect(brightest).toBeGreaterThanOrEqual(240);
    expect(rowLevels.filter((level) => level > 1)).toHaveLength(
      rowLevels.filter((level) => level >= 200).length,
    );
    // Every column carries it: the row is bright all the way across.
    const brightRow = rowLevels.indexOf(brightest);
    for (let column = 0; column < 244; column += 1) {
      expect(levels[brightRow * 244 + column]).toBe(brightest);
    }
  });

  it("averages linear density, not log codes", () => {
    // One pixel at the ceiling and one at the floor, 5 dex apart, reduced to one pixel: the mean
    // density is half the ceiling's, 0.30 dex below it, not the half-way code 2.5 dex below.
    const codes = Uint8Array.from([255, 1]);

    const [level] = reducedLevels({ widthPx: 2, heightPx: 1, codes, maxCode: 255 }, 5, 1, 1);

    const expectedDex = 5 + Math.log10(0.5 + 0.5 * 1e-5);
    expect(level).toBe(1 + Math.round((expectedDex / 5) * 254));
  });

  it("shows as background a mean that falls below the floor", () => {
    // Half the area empty and half at the floor: the mean lies below the floor.
    const codes = Uint8Array.from([0, 1]);

    const [level] = reducedLevels({ widthPx: 2, heightPx: 1, codes, maxCode: 255 }, 5, 1, 1);

    expect(level).toBe(0);
  });

  it("keeps a block of floor pixels at the floor, above the background", () => {
    const [level] = reducedLevels(aMap(3, 3, 1), 5, 1, 1);

    expect(level).toBe(1);
  });

  it("weights a pixel by the share of the reduced pixel it covers", () => {
    // Three pixels into two: the middle one is split between them.
    const codes = Uint8Array.from([255, 1, 1]);

    const levels = reducedLevels({ widthPx: 3, heightPx: 1, codes, maxCode: 255 }, 5, 2, 1);

    // The left reduced pixel covers the ceiling pixel and half the middle one: 2/3 and 1/3.
    const leftDex = Math.log10((2 / 3) * 1e5 + 1 / 3);
    expect(levels[0]).toBe(1 + Math.round((leftDex / 5) * 254));
    expect(levels[1]).toBe(1);
  });

  it("refuses to enlarge a map", () => {
    expect(() => reducedLevels(aMap(4, 2, 1), 5, 8, 4)).toThrow(RangeError);
  });
});

describe("paintLevels", () => {
  it("paints each level in its ramp colour", () => {
    const ramp = buildRamp(parseHexColour("#05080d"), parseHexColour("#c8d6e5"));

    const rgba = paintLevels(Uint8Array.from([0, 255]), ramp);

    expect([...rgba]).toEqual([0x05, 0x08, 0x0d, 255, 0xc8, 0xd6, 0xe5, 255]);
  });
});
