import { describe, expect, it } from "vitest";

import type { GalacticPosition } from "./generated/GalacticPosition";
import { galacticDeltaLy, galacticPositionFromLy, METRES_PER_LIGHT_YEAR } from "./position";
import { SECONDS_PER_JULIAN_YEAR } from "./time";

const SPEED_OF_LIGHT_M_PER_S = 299_792_458;

describe("METRES_PER_LIGHT_YEAR", () => {
  it("is the distance light travels in one Julian year", () => {
    expect(METRES_PER_LIGHT_YEAR).toBe(SPEED_OF_LIGHT_M_PER_S * SECONDS_PER_JULIAN_YEAR);
  });
});

describe("galacticPositionFromLy", () => {
  it("floors each coordinate to its cell and keeps the rest as metres", () => {
    expect(galacticPositionFromLy([26_000.5, 12.25, 0])).toEqual({
      cell_ly: [26_000, 12, 0],
      offset_m: [0.5 * METRES_PER_LIGHT_YEAR, 0.25 * METRES_PER_LIGHT_YEAR, 0],
    });
  });

  it("puts negative coordinates in the cell below with a positive offset", () => {
    expect(galacticPositionFromLy([-0.25, -1, -1.5])).toEqual({
      cell_ly: [-1, -1, -2],
      offset_m: [0.75 * METRES_PER_LIGHT_YEAR, 0, 0.5 * METRES_PER_LIGHT_YEAR],
    });
  });

  it("carries a fraction that rounds up to a whole light-year into the cell", () => {
    const position = galacticPositionFromLy([-1e-300, 0, 0]);

    expect(position.cell_ly).toEqual([0, 0, 0]);
    expect(position.offset_m).toEqual([0, 0, 0]);
  });

  it("writes negative zero as zero", () => {
    const position = galacticPositionFromLy([-0, -0, -0]);

    expect(JSON.stringify(position)).toBe('{"cell_ly":[0,0,0],"offset_m":[0,0,0]}');
    expect(Object.is(position.cell_ly[0], 0)).toBe(true);
  });

  it("keeps every offset within one light-year", () => {
    for (const ly of [-50_000.999_999_999, -0.999_999_999_999, 0.999_999_999_999_999_9, 65_535.9]) {
      const [offsetM] = galacticPositionFromLy([ly, 0, 0]).offset_m;
      expect(offsetM).toBeGreaterThanOrEqual(0);
      expect(offsetM).toBeLessThan(METRES_PER_LIGHT_YEAR);
    }
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY, 2 ** 31, -(2 ** 31) - 1])(
    "refuses a coordinate of %s ly",
    (ly) => {
      expect(() => galacticPositionFromLy([0, ly, 0])).toThrow(RangeError);
    },
  );
});

describe("galacticDeltaLy", () => {
  it("is exact for two points 3 ly apart 50,000 ly from the centre", () => {
    const from: GalacticPosition = {
      cell_ly: [50_000, -50_001, 12],
      offset_m: [0.1 * METRES_PER_LIGHT_YEAR, 0.9 * METRES_PER_LIGHT_YEAR, 123_456_789],
    };
    const to: GalacticPosition = {
      cell_ly: [50_003, -50_001, 12],
      offset_m: [0.1 * METRES_PER_LIGHT_YEAR, 0.9 * METRES_PER_LIGHT_YEAR, 123_456_789],
    };

    const [dx, dy, dz] = galacticDeltaLy(from, to);

    expect(Math.abs(dx - 3)).toBeLessThan(1e-9);
    expect(Math.abs(dy)).toBeLessThan(1e-9);
    expect(Math.abs(dz)).toBeLessThan(1e-9);
  });

  it("gives the offset between points built from light-years", () => {
    const from = galacticPositionFromLy([49_999.4, 0.5, -3.25]);
    const to = galacticPositionFromLy([50_001.4, -0.5, -1.25]);

    const delta = galacticDeltaLy(from, to);

    expect(delta[0]).toBeCloseTo(2, 9);
    expect(delta[1]).toBeCloseTo(-1, 9);
    expect(delta[2]).toBeCloseTo(2, 9);
  });

  it("is the negation of the reverse delta", () => {
    const a = galacticPositionFromLy([-12_345.678, 26_000.001, 3.5]);
    const b = galacticPositionFromLy([-12_340.178, 25_999.001, -2.5]);

    const forward = galacticDeltaLy(a, b);
    const backward = galacticDeltaLy(b, a);

    for (const axis of [0, 1, 2] as const) {
      expect(forward[axis]).toBeCloseTo(-backward[axis], 12);
    }
  });
});
