import { describe, expect, it } from "vitest";

import { patternedThickBake, thickPatternAt } from "../../../test/thickTables";
import { THICK_TABLE_SIZE, TABLE_SIZES } from "../hillaire";
import type { AtmosphereMedium } from "../medium";
import type { TablesBand } from "../tables";
import { THICK_BAKE_GRID, THICK_BAKE_MODES, type MsSourceTable } from "./bake";
import {
  addThickLayer,
  packThickTable,
  THICK_ALTITUDES,
  THICK_LAYER_BYTES,
  THICK_SUN_COSINES,
  THICK_TABLE_BYTES_A_SLOT,
  THICK_TRAVEL_COSINES,
  thickAnalyticShare,
  thickBandsInto,
  thickJInto,
  thickRowAt,
  thickSourceScratch,
  thickTableBytes,
} from "./read";

/** A two-term medium, its scattering falling with height: the kernels' frame, no forward delta. */
const MEDIUM: AtmosphereMedium = {
  name: "thick read test",
  topHeightM: 100_000,
  groundAlbedo: [0.3, 0.3, 0.3],
  terms: [
    {
      name: "gas",
      density: { kind: "exponential", scaleHeightM: 8_000 },
      scattering: [5.8e-6, 1.35e-5, 3.31e-5],
      absorption: [0, 0, 0],
      phase: { kind: "rayleigh", depolarisation: [0.03, 0.03, 0.03] },
    },
    {
      name: "haze",
      density: { kind: "exponential", scaleHeightM: 1_200 },
      scattering: [2e-5, 2e-5, 2e-5],
      absorption: [2e-6, 2e-6, 2e-6],
      phase: { kind: "cornette-shanks", asymmetry: 0.6 },
    },
  ],
};

/** One band at s = 1, as a sphere's tables plan holds it. */
const ONE_BAND: TablesBand = {
  gravityRatio: 1,
  latitudeRad: 0,
  bottomRadiusM: 6.371e6,
  coefficientFactor: 1,
};

/** Two bands at s 0.98 and 1.03. */
const TWO_BANDS: ReadonlyArray<TablesBand> = [0.98, 1.03].map((s, b) => ({
  gravityRatio: s,
  latitudeRad: b,
  bottomRadiusM: 6e7,
  coefficientFactor: 1 / s,
}));

/** The half float's rounding, relative: 2⁻¹¹. */
const HALF_ULP = 2 ** -11;

/** μ₀ at cell s and μ_v at cell v. */
const sunAt = (s: number): number => (s + 0.5) / THICK_SUN_COSINES;
const travelAt = (v: number): number => (v + 0.5) / (THICK_TRAVEL_COSINES / 2) - 1;

describe("the thick table's shape (R08.T14.c)", () => {
  it("is the bake's grid, and TABLE_SIZES holds it on both settings", () => {
    expect(THICK_ALTITUDES).toBe(THICK_BAKE_GRID.layers / THICK_BAKE_GRID.altitudeStride + 1);
    expect(THICK_SUN_COSINES).toBe(THICK_BAKE_GRID.sunCosines.length);
    expect(THICK_TRAVEL_COSINES).toBe(2 * THICK_BAKE_GRID.upwardCosines.length);
    expect(THICK_TABLE_SIZE.modes).toBe(THICK_BAKE_MODES);
    expect(TABLE_SIZES.high.thick).toBe(THICK_TABLE_SIZE);
    expect(TABLE_SIZES.low.thick).toBe(THICK_TABLE_SIZE);
  });

  it("takes 270,336 B a band and light slot, as rgba16float would, and 256 B more a layer", () => {
    expect(THICK_TABLE_BYTES_A_SLOT).toBe(270_336);
    expect(THICK_LAYER_BYTES).toBe(270_592);
    const packed = packThickTable(patternedThickBake(TWO_BANDS, MEDIUM, 2), MEDIUM);
    expect(packed.layers).toBe(4);
    expect(thickTableBytes(packed)).toBe(4 * 270_592);
    expect(packed.texels.byteLength).toBe(thickTableBytes(packed));
  });
});

describe("the thick table's packing and read (R08.T14.c)", () => {
  const bands = patternedThickBake(TWO_BANDS, MEDIUM, 2);
  const packed = packThickTable(bands, MEDIUM);

  it("reads each cell's J⁰ and G back to the half float's rounding, divided by the scattering", () => {
    const out = thickSourceScratch();
    let worst = 0;
    for (const [b, layerBand] of [0, 1].entries()) {
      for (const k of [0, 1]) {
        for (const a of [0, 1, 7, 16, 31, 32]) {
          for (const s of [0, 5, 31]) {
            for (const v of [0, 7, 8, 15]) {
              out.level.fill(0);
              out.slope.fill(0);
              const row = thickRowAt(packed, packed.altitudesM[a] ?? Number.NaN);
              addThickLayer(packed, layerBand * 2 + k, row, sunAt(s), travelAt(v), 1, out);
              const expected = thickPatternAt(a, s, v, b, k);
              for (let c = 0; c < 3; c += 1) {
                const level = expected.level[c] ?? Number.NaN;
                const slope = expected.slope[c] ?? Number.NaN;
                worst = Math.max(
                  worst,
                  Math.abs((out.level[c] ?? Number.NaN) - level) / level,
                  Math.abs((out.slope[c] ?? Number.NaN) - slope) / level,
                );
              }
            }
          }
        }
      }
    }
    // f32 rounding of E and σ, then the half's: within one half ulp and a little.
    expect(worst).toBeLessThan(1.2 * HALF_ULP);
  });

  it("is linear between the cells' centres in each axis, and held at the table's edges", () => {
    const at = (h: number, mu0: number, muV: number): readonly [number, number] => {
      const out = thickSourceScratch();
      addThickLayer(packed, 0, thickRowAt(packed, h), mu0, muV, 1, out);
      return [out.level[1], out.slope[1]];
    };
    const h4 = packed.altitudesM[4] ?? Number.NaN;
    const h5 = packed.altitudesM[5] ?? Number.NaN;
    const mid = at(0.5 * (h4 + h5), sunAt(3), travelAt(9));
    const lower = at(h4, sunAt(3), travelAt(9));
    const upper = at(h5, sunAt(3), travelAt(9));
    expect(mid[0]).toBeCloseTo(0.5 * (lower[0] + upper[0]), 12);
    const between = at(h4, 0.5 * (sunAt(3) + sunAt(4)), travelAt(9));
    expect(between[0]).toBeCloseTo(0.5 * (lower[0] + at(h4, sunAt(4), travelAt(9))[0]), 12);
    const across = at(h4, sunAt(3), 0.5 * (travelAt(9) + travelAt(10)));
    expect(across[1]).toBeCloseTo(0.5 * (lower[1] + at(h4, sunAt(3), travelAt(10))[1]), 12);
    // Past the outer cells' centres, and below the ground or above the top, the edge holds.
    expect(at(h4, 0, travelAt(9))).toEqual(at(h4, sunAt(0), travelAt(9)));
    expect(at(h4, 1, 1)).toEqual(at(h4, sunAt(31), travelAt(15)));
    expect(at(-50, sunAt(3), -1)).toEqual(at(0, sunAt(3), travelAt(0)));
    expect(at(2 * MEDIUM.topHeightM, sunAt(3), travelAt(9))).toEqual(
      at(MEDIUM.topHeightM, sunAt(3), travelAt(9)),
    );
  });

  it("finds a height's rows by bisection, the lower at most 31 and the share in [0, 1]", () => {
    expect(thickRowAt(packed, -1)).toEqual({ lower: 0, t: 0 });
    expect(thickRowAt(packed, 0)).toEqual({ lower: 0, t: 0 });
    expect(thickRowAt(packed, packed.altitudesM[17] ?? Number.NaN)).toEqual({ lower: 17, t: 0 });
    expect(thickRowAt(packed, packed.altitudesM[32] ?? Number.NaN)).toEqual({ lower: 31, t: 1 });
    expect(thickRowAt(packed, 1e9)).toEqual({ lower: 31, t: 1 });
    const h = 0.25 * (packed.altitudesM[9] ?? Number.NaN) + 0.75 * (packed.altitudesM[10] ?? 0);
    const row = thickRowAt(packed, h);
    expect(row.lower).toBe(9);
    expect(row.t).toBeCloseTo(0.75, 6);
  });

  it("reads the bands linearly in ln s between them, and the nearer band beyond them", () => {
    const ln = Float64Array.from(TWO_BANDS, (band) => Math.log(band.gravityRatio));
    const read = (s: number): number => {
      const out = thickSourceScratch();
      thickBandsInto(packed, ln, 20_000, s, 0.4, -0.3, 1, out);
      return out.level[0];
    };
    const s0 = TWO_BANDS[0]?.gravityRatio ?? Number.NaN;
    const s1 = TWO_BANDS[1]?.gravityRatio ?? Number.NaN;
    expect(read(0.9)).toBe(read(s0));
    expect(read(1.2)).toBe(read(s1));
    const middle = Math.exp(0.5 * (Math.log(s0) + Math.log(s1)));
    expect(read(middle)).toBeCloseTo(0.5 * (read(s0) + read(s1)), 12);
    // A light slot past the table's takes its last.
    const out2 = thickSourceScratch();
    const out9 = thickSourceScratch();
    thickBandsInto(packed, ln, 20_000, s0, 0.4, -0.3, 1, out2);
    thickBandsInto(packed, ln, 20_000, s0, 0.4, -0.3, 9, out9);
    expect(out9).toEqual(out2);
  });

  it("forms J as J⁰ plus G times the horizontal product, held at 0", () => {
    const out: [number, number, number] = [0, 0, 0];
    thickJInto({ level: [0.04, 0.02, 0.01], slope: [0.01, -0.02, 0.5] }, 0.5, out);
    expect(out).toEqual([0.045, 0.01, 0.26]);
    thickJInto({ level: [0.04, 0.02, 0.01], slope: [0.01, -0.08, -0.5] }, 0.5, out);
    expect(out).toEqual([0.045, 0, 0]);
  });
});

describe("the analytic term's share below the horizon (R08.T14.c)", () => {
  it("is 1 at and below the horizontal, 0 from the first cell's upper edge, linear across it", () => {
    expect(thickAnalyticShare(-0.3)).toBe(1);
    expect(thickAnalyticShare(0)).toBe(1);
    expect(thickAnalyticShare(1 / 64)).toBeCloseTo(0.5, 15);
    expect(thickAnalyticShare(1 / 32)).toBe(0);
    expect(thickAnalyticShare(0.7)).toBe(0);
  });
});

describe("the thick table's refusals (R08.T14.c)", () => {
  const good = patternedThickBake([ONE_BAND], MEDIUM, 1);
  const first = good[0];
  if (first === undefined) {
    throw new Error("a patterned bake has its band");
  }
  const withFirst = (change: Partial<MsSourceTable>): MsSourceTable[] => [{ ...first, ...change }];

  it("refuses no band, no light slot and a table off the bake's grid", () => {
    expect(() => packThickTable([], MEDIUM)).toThrow(RangeError);
    expect(() => packThickTable(withFirst({ slots: [] }), MEDIUM)).toThrow(RangeError);
    expect(() =>
      packThickTable(withFirst({ altitudesM: first.altitudesM.map((h) => h + 1) }), MEDIUM),
    ).toThrow(/ascend from the ground/u);
    expect(() =>
      packThickTable(withFirst({ sunCosines: first.sunCosines.map((mu) => mu * 0.99) }), MEDIUM),
    ).toThrow(/grid/u);
    expect(() => packThickTable(withFirst({ modes: 3 }), MEDIUM)).toThrow(/grid/u);
  });

  it("refuses bands of other altitudes or light slots", () => {
    const two = patternedThickBake(TWO_BANDS, MEDIUM, 2);
    const [b0, b1] = two;
    if (b0 === undefined || b1 === undefined) {
      throw new Error("a patterned bake has two bands");
    }
    expect(() => packThickTable([b0, { ...b1, slots: b1.slots.slice(1) }], MEDIUM)).toThrow(
      /light slots/u,
    );
    const moved = b1.altitudesM.slice();
    moved[5] = (moved[5] ?? 0) + 1;
    expect(() => packThickTable([b0, { ...b1, altitudesM: moved }], MEDIUM)).toThrow(
      /other altitudes/u,
    );
  });

  it("refuses a value that is not finite, or that no half float holds", () => {
    const slot = first.slots[0];
    if (slot === undefined) {
      throw new Error("a patterned bake has its slot");
    }
    const broken = (index: number, value: number): MsSourceTable[] => {
      const values = slot.values.slice();
      values[index] = value;
      return withFirst({ slots: [{ ...slot, values }] });
    };
    expect(() => packThickTable(broken(10, Number.NaN), MEDIUM)).toThrow(RangeError);
    // E at the top over its tiny scattering: a J of 10⁹.
    const topAt = (THICK_ALTITUDES - 1) * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * 2 * 3;
    expect(() => packThickTable(broken(topAt, 1), MEDIUM)).toThrow(/no half float/u);
  });
});
