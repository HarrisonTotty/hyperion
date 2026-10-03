import { describe, expect, it } from "vitest";

import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { BAND_LIMIT_M } from "./cube";
import { MAX_LEVEL } from "./patchKey";
import {
  bandLimitM,
  LEVEL_TABLE_STRIDE,
  levelBoundM,
  levelHeightRangeM,
  lowestHeightM,
  planetGeometry,
} from "./planet";

const view = new DataView(new ArrayBuffer(8));

function bits(x: number): bigint {
  view.setFloat64(0, x);
  return view.getBigUint64(0);
}

describe("the planet's geometry", () => {
  it("reproduces every value of the golden level table bit for bit", () => {
    for (const ridges of ["off", "on"] as const) {
      const table = goldenLevelTable(ridges);
      expect(table).toHaveLength(LEVEL_TABLE_STRIDE * (MAX_LEVEL + 1));
      const planet = planetGeometry(WGS84_FIGURE, table);
      for (let level = 0; level <= MAX_LEVEL; level += 1) {
        const row = table.subarray(4 * level, 4 * level + 3);
        const read = [levelBoundM(planet, level), ...levelHeightRangeM(planet, level)];
        expect(read.map(bits)).toEqual([...row].map(bits));
      }
    }
  });

  it("puts the test planet's finest level at 19", () => {
    expect(planetGeometry(WGS84_FIGURE, goldenLevelTable("off")).finestLevel).toBe(19);
  });

  it("gives a zero-height spheroid no bound and no relief without a table", () => {
    const planet = planetGeometry(WGS84_FIGURE, null);
    for (let level = 0; level <= MAX_LEVEL; level += 1) {
      expect([levelBoundM(planet, level), ...levelHeightRangeM(planet, level)]).toEqual([0, 0, 0]);
    }
    expect(lowestHeightM(planet)).toBe(0);
  });

  it("finds the lowest height any level reaches", () => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const lows = Array.from({ length: MAX_LEVEL + 1 }, (_, n) => levelHeightRangeM(planet, n)[0]);
    expect(lowestHeightM(planet)).toBe(Math.min(...lows));
    expect(lowestHeightM(planet)).toBeLessThan(0);
  });

  it("refuses a table of the wrong length and a figure with c above a", () => {
    expect(() => planetGeometry(WGS84_FIGURE, new Float64Array(99))).toThrow(/level table/u);
    expect(() =>
      planetGeometry({ equatorialRadiusM: 1, polarRadiusM: 2, pole: null }, null),
    ).toThrow(/figure/u);
  });

  it("gives the module's band limit", () => {
    expect(bandLimitM()).toBe(BAND_LIMIT_M);
    expect(bandLimitM()).toBe(2);
  });
});
