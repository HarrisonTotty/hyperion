import { describe, expect, it } from "vitest";

import fixture from "../fixtures/density_map_4x2.json" with { type: "json" };
import { decodeDensityMap } from "./densityMap";
import type { DensityMap } from "./generated/DensityMap";

/** The fixture's map, checked into the generated type. */
function fixtureMap(): DensityMap {
  const { map } = fixture;
  const [centreX, centreZ] = map.centre_ly;
  if (map.view !== "edge_on" || map.population !== "all") {
    throw new Error("the fixture is no longer an edge-on map of all systems");
  }
  if (centreX === undefined || centreZ === undefined) {
    throw new Error("the fixture's centre lost a coordinate");
  }
  return { ...map, view: map.view, population: map.population, centre_ly: [centreX, centreZ] };
}

/** An 8-bit map `widthPx` by `heightPx`, spanning `floor` to `ceiling`, of the given payload. */
function eightBitMap(
  dataBase64: string,
  [widthPx, heightPx]: readonly [number, number],
  floor: number,
  ceiling: number,
): DensityMap {
  return {
    universe: "0123456789abcdef",
    view: "face_on",
    population: "young",
    width_px: widthPx,
    height_px: heightPx,
    centre_ly: [0, 0],
    ly_per_px: 131_072 / widthPx,
    bits: 8,
    floor_log10_per_ly2: floor,
    ceiling_log10_per_ly2: ceiling,
    data_base64: dataBase64,
  };
}

describe("decodeDensityMap", () => {
  it("decodes the shared 16-bit fixture", () => {
    const decoded = decodeDensityMap(fixtureMap());

    expect(decoded.widthPx).toBe(4);
    expect(decoded.heightPx).toBe(2);
    expect(decoded.maxCode).toBe(65_535);
    expect(decoded.codes).toBeInstanceOf(Uint16Array);
    expect([...decoded.codes]).toEqual(fixture.codes);
  });

  it("recovers the fixture's densities to within half a code", () => {
    const decoded = decodeDensityMap(fixtureMap());
    const { floor_log10_per_ly2: floor, ceiling_log10_per_ly2: ceiling } = fixture.map;
    const halfStep = (ceiling - floor) / (decoded.maxCode - 1) / 2;
    const values = [...decoded.codes].map((code) => decoded.log10PerLy2(code));
    const raw = fixture.log10_per_ly2;

    expect(values.map((value) => value === null)).toEqual(
      raw.map((value) => value === null || value <= floor),
    );
    const errors = raw.flatMap((value, index) =>
      value === null || value <= floor ? [] : [Math.abs((values[index] ?? Number.NaN) - value)],
    );
    expect(Math.max(...errors)).toBeLessThanOrEqual(halfStep);
  });

  it("decodes an 8-bit map row by row from the top", () => {
    // The codes 0, 1, 2, 255, 128 and 64.
    const decoded = decodeDensityMap(eightBitMap("AAEC/4BA", [3, 2], -4, 1));

    expect(decoded.codes).toBeInstanceOf(Uint8Array);
    expect([...decoded.codes]).toEqual([0, 1, 2, 255, 128, 64]);
    expect(decoded.widthPx).toBe(3);
    expect(decoded.heightPx).toBe(2);
    expect(decoded.maxCode).toBe(255);
  });

  it("maps code 1 to the floor, the largest code to the ceiling and code 0 to nothing", () => {
    // The codes 0, 1, 255 and 0.
    const decoded = decodeDensityMap(eightBitMap("AAH/AA==", [2, 2], -5.25, 1.75));

    expect(decoded.log10PerLy2(0)).toBeNull();
    expect(decoded.log10PerLy2(1)).toBeCloseTo(-5.25, 12);
    expect(decoded.log10PerLy2(decoded.maxCode)).toBeCloseTo(1.75, 12);
    expect(decoded.log10PerLy2(128)).toBeCloseTo(-5.25 + (127 * 7) / 254, 12);
  });

  it("maps the 16-bit fixture's floor and ceiling to codes 1 and the largest code", () => {
    const decoded = decodeDensityMap(fixtureMap());

    expect(decoded.log10PerLy2(1)).toBeCloseTo(fixture.map.floor_log10_per_ly2, 12);
    expect(decoded.log10PerLy2(65_535)).toBeCloseTo(fixture.map.ceiling_log10_per_ly2, 12);
  });

  it.each([-1, 256, 1.5])("refuses to interpret %s as an 8-bit code", (code) => {
    const decoded = decodeDensityMap(eightBitMap("AAA=", [2, 1], 0, 1));

    expect(() => decoded.log10PerLy2(code)).toThrow(RangeError);
  });

  it("throws on truncated data", () => {
    const map = fixtureMap();

    expect(() => decodeDensityMap({ ...map, data_base64: map.data_base64.slice(0, 12) })).toThrow(
      /holds 9 bytes where 4 × 2 pixels at 16 bits need 16/,
    );
  });

  it("throws when the header claims more pixels than the data holds", () => {
    expect(() => decodeDensityMap({ ...fixtureMap(), height_px: 3 })).toThrow(/need 24/);
  });

  it("throws on base64 of the wrong length", () => {
    expect(() => decodeDensityMap({ ...fixtureMap(), data_base64: "AAAAA" })).toThrow(
      /not a whole number of quads/,
    );
  });

  it.each([
    ["a character outside the alphabet", "AAAA*AAA"],
    ["padding before the end", "AA=AAAAA"],
    ["too much padding", "AAAAA==="],
  ])("throws on base64 with %s", (_reason, data) => {
    expect(() => decodeDensityMap({ ...fixtureMap(), data_base64: data })).toThrow(
      /invalid base64 character/,
    );
  });

  it("throws on a depth other than 8 or 16 bits", () => {
    expect(() => decodeDensityMap({ ...fixtureMap(), bits: 12 })).toThrow(/12 bits/);
  });
});
