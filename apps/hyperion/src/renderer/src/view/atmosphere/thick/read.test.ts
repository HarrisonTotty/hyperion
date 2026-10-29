import { describe, expect, it } from "vitest";

import {
  patternedCorrection,
  patternedThickBake,
  thickCorrectionPatternAt,
  thickPatternAt,
} from "../../../test/thickTables";
import { POLARISATION_TABLE_SIZE, THICK_TABLE_SIZE, TABLE_SIZES } from "../hillaire";
import type { AtmosphereMedium } from "../medium";
import type { TablesBand } from "../tables";
import { THICK_BAKE_GRID, THICK_BAKE_MODES, type MsSourceTable } from "./bake";
import { POLARISATION_MODES, type PolarisationTable } from "./polarisation";
import {
  addThickLayer,
  packThickCurve,
  packThickTable,
  THICK_ALTITUDES,
  THICK_CORRECTION_MODES,
  THICK_INFO_ROW,
  THICK_KIND_WORD,
  THICK_LAYER_BYTES,
  THICK_ROWS,
  THICK_SOURCE_MODES,
  THICK_SUN_COSINES,
  THICK_TABLE_BYTES_A_SLOT,
  THICK_TRAVEL_COSINES,
  thickAnalyticShare,
  thickAnalyticWeight,
  thickBandsInto,
  thickCurveChannel,
  thickJInto,
  thickMultipleInto,
  thickRowAt,
  type ThickSource,
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

/** The words of one packed layer: its table's rows and its info row, four words a texel. */
const LAYER_WORDS = THICK_ROWS * THICK_TRAVEL_COSINES * 4;

/** The half float's rounding, relative: 2⁻¹¹. */
const HALF_ULP = 2 ** -11;

/** J⁰ and G of the J tests, with G₂ `curve`. */
const flat = (curve: [number, number, number]): ThickSource => ({
  level: [0.04, 0.02, 0.01],
  slope: [0.01, -0.02, 0.5],
  curve,
});

/** What the G₂ tests name their values. */
const where = (): string => "a test's G₂";

/** μ₀ at cell s and μ_v at cell v. */
const sunAt = (s: number): number => (s + 0.5) / THICK_SUN_COSINES;
const travelAt = (v: number): number => (v + 0.5) / (THICK_TRAVEL_COSINES / 2) - 1;

describe("the thick table's shape (R08.T14.c)", () => {
  it("is the bake's grid, and TABLE_SIZES holds it on both settings", () => {
    expect(THICK_ALTITUDES).toBe(THICK_BAKE_GRID.layers / THICK_BAKE_GRID.altitudeStride + 1);
    expect(THICK_SUN_COSINES).toBe(THICK_BAKE_GRID.sunCosines.length);
    expect(THICK_TRAVEL_COSINES).toBe(2 * THICK_BAKE_GRID.upwardCosines.length);
    expect(THICK_SOURCE_MODES).toBe(THICK_BAKE_MODES);
    expect(THICK_CORRECTION_MODES).toBe(POLARISATION_MODES);
    expect(THICK_TABLE_SIZE.modes).toBe(3);
    expect(TABLE_SIZES.high.thick).toBe(THICK_TABLE_SIZE);
    expect(TABLE_SIZES.low.thick).toBe(THICK_TABLE_SIZE);
  });

  it("holds the polarisation correction's grid on both settings, the thick table's (R08.T14.f)", () => {
    expect(POLARISATION_TABLE_SIZE).toEqual({ ...THICK_TABLE_SIZE, modes: POLARISATION_MODES });
    expect(TABLE_SIZES.high.polarisation).toBe(POLARISATION_TABLE_SIZE);
    expect(TABLE_SIZES.low.polarisation).toBe(POLARISATION_TABLE_SIZE);
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

  it("reads each cell's J⁰ and G back to the half float's rounding, per unit of the kernels' scattering as baked", () => {
    const out = thickSourceScratch();
    let worst = 0;
    for (const [b, layerBand] of [0, 1].entries()) {
      for (const k of [0, 1]) {
        for (const a of [0, 1, 7, 16, 31, 32]) {
          for (const s of [0, 5, 31]) {
            for (const v of [0, 7, 8, 15]) {
              out.level.fill(0);
              out.slope.fill(0);
              out.curve.fill(0);
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
    // f32 rounding of J, then the half's: within one half ulp and a little.
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

  it("is a thick table with no m = 2 in any layer where its bake holds none, and its kind in the info row's word 34", () => {
    expect(packed.kind).toBe("thick");
    const info = THICK_INFO_ROW * THICK_TRAVEL_COSINES * 4;
    expect(packed.texels[info + THICK_KIND_WORD]).toBe(0);
    const curves = packed.texels.filter(
      (word, i) => i % LAYER_WORDS < info && i % 4 === 3 && word !== 0,
    ).length;
    expect([packed.layers, curves]).toEqual([4, 0]);
  });

  it("forms J as J⁰ plus G times the horizontal product plus G₂ times its harmonic, not held at 0", () => {
    const out: [number, number, number] = [0, 0, 0];
    thickJInto(flat([0, 0, 0]), 0.6, -0.3, 0.5, out);
    expect(out).toEqual([0.045, 0.01, 0.26]);
    // h_⊥ = sin θ_v sin θ₀ cos Δφ; G₂ times 2h_⊥² − sin²θ_v sin²θ₀ is G₂ sin²θ_v sin²θ₀ cos 2Δφ.
    const mu0 = 0.6;
    const muV = -0.3;
    const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
    for (const phi of [0, 0.4, Math.PI / 2, 2.5, Math.PI]) {
      thickJInto(flat([0.2, -0.1, 0]), mu0, muV, sines * Math.cos(phi), out);
      const harmonic = sines * sines * Math.cos(2 * phi);
      expect(out[0]).toBeCloseTo(0.04 + 0.01 * sines * Math.cos(phi) + 0.2 * harmonic, 14);
      expect(out[1]).toBeCloseTo(0.02 - 0.02 * sines * Math.cos(phi) - 0.1 * harmonic, 14);
    }
    thickJInto(
      { level: [0.04, 0.02, 0.01], slope: [0.01, -0.08, -0.5], curve: [0, 0, 0] },
      0,
      0,
      0.5,
      out,
    );
    expect(out[1]).toBeCloseTo(-0.02, 15);
    expect(out[2]).toBeCloseTo(-0.24, 15);
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

  it("weighs the analytic term by its share on a thick table and whole beside a correction (R08.T14.f)", () => {
    for (const mu of [-0.3, 0, 1 / 64, 1 / 32, 0.7]) {
      expect(thickAnalyticWeight("thick", mu)).toBe(thickAnalyticShare(mu));
      expect(thickAnalyticWeight("correction", mu)).toBe(1);
    }
  });

  it("mixes a thick table's J, held at 0, with Ψ_ms at its share, and adds a correction to Ψ_ms whole (R08.T14.f)", () => {
    const psi: [number, number, number] = [0.03, 0.02, 0.001];
    const j: [number, number, number] = [0.05, -0.01, -0.004];
    const thick: [number, number, number] = [...psi];
    thickMultipleInto("thick", 1 / 128, j, thick);
    expect(thick[0]).toBeCloseTo(0.75 * 0.03 + 0.25 * 0.05, 15);
    expect(thick[1]).toBeCloseTo(0.75 * 0.02, 15);
    const corrected: [number, number, number] = [...psi];
    thickMultipleInto("correction", 1 / 128, j, corrected);
    expect(corrected[0]).toBeCloseTo(0.03 + 0.25 * 0.05, 15);
    expect(corrected[1]).toBeCloseTo(0.02 - 0.25 * 0.01, 15);
    // The sum is held at 0, never the correction alone.
    expect(corrected[2]).toBe(0);
    const high: [number, number, number] = [...psi];
    thickMultipleInto("correction", 0.5, j, high);
    expect(high[1]).toBeCloseTo(0.01, 15);
  });
});

describe("G₂'s shared-exponent word (R08.T14.f)", () => {
  it("reads each channel back to 1 in 255 of the largest, its sign kept, and 0 as 0", () => {
    expect(packThickCurve([0, 0, 0], where)).toBe(0);
    let worst = 0;
    for (const values of [
      [0.02, -0.013, 0.0004],
      [-3.1e-6, 2.2e-6, 0],
      [1_000, -7, 32_640],
      [2 ** -24, -(2 ** -24), 0],
      [0.25, 0.5, -1],
    ] as const) {
      const word = packThickCurve([values[0], values[1], values[2]], where);
      const largest = Math.max(...values.map(Math.abs));
      for (let c = 0; c < 3; c += 1) {
        const read = thickCurveChannel(word, c);
        const value = values[c] ?? Number.NaN;
        expect(Math.sign(read) === Math.sign(value) || read === 0).toBe(true);
        worst = Math.max(worst, Math.abs(read - value) / largest);
      }
    }
    // The least exponent leaves the largest channel at least 127.75 of its steps, so a half step
    // is at most 1 in 255.5 of it.
    expect(worst).toBeLessThanOrEqual(1 / 255);
    // Powers of two and their small multiples are exact.
    const exact = packThickCurve([0.25, 0.5, -1], where);
    expect([0, 1, 2].map((c) => thickCurveChannel(exact, c))).toEqual([0.25, 0.5, -1]);
  });

  it("takes the least exponent, so that a texel's largest channel keeps 8 bits", () => {
    const word = packThickCurve([3e-3, 0, 0], where);
    const mantissa = word & 0x1ff;
    expect(mantissa).toBeGreaterThan(127);
    expect(mantissa).toBeLessThanOrEqual(255);
  });

  it("refuses a value that is not finite, or past 32,640", () => {
    expect(() => packThickCurve([Number.NaN, 0, 0], where)).toThrow(/not finite/u);
    expect(() => packThickCurve([0, -40_000, 0], where)).toThrow(/32,640/u);
  });
});

describe("the polarisation correction's packing and read (R08.T14.f)", () => {
  const bands = patternedThickBake(TWO_BANDS, MEDIUM, 2);
  const correction = patternedCorrection(TWO_BANDS, MEDIUM, 2);
  const alone = packThickTable(null, MEDIUM, { bands: correction, medium: MEDIUM });
  const summed = packThickTable(bands, MEDIUM, { bands: correction, medium: MEDIUM });

  /** The read at a cell's centre, band `b`'s slot `k`. */
  const cellOf = (
    table: ReturnType<typeof packThickTable>,
    b: number,
    k: number,
    a: number,
    s: number,
    v: number,
  ): ThickSource => {
    const out = thickSourceScratch();
    const row = thickRowAt(table, table.altitudesM[a] ?? Number.NaN);
    addThickLayer(table, b * 2 + k, row, sunAt(s), travelAt(v), 1, out);
    return out;
  };

  it("holds a thin world's correction alone, as a correction table of the bake's size", () => {
    expect(alone.kind).toBe("correction");
    expect(alone.texels[THICK_INFO_ROW * THICK_TRAVEL_COSINES * 4 + THICK_KIND_WORD]).toBe(1);
    expect(thickTableBytes(alone)).toBe(4 * THICK_LAYER_BYTES);
    expect(summed.kind).toBe("thick");
    expect(thickTableBytes(summed)).toBe(thickTableBytes(packThickTable(bands, MEDIUM)));
  });

  it("reads each cell's ΔJ_0, ΔJ_1 and ΔJ_2 quotients back, alone and summed into the bake", () => {
    let worst = 0;
    for (const b of [0, 1]) {
      for (const k of [0, 1]) {
        for (const a of [0, 3, 16, 32]) {
          for (const s of [0, 9, 31]) {
            for (const v of [0, 7, 8, 15]) {
              const delta = thickCorrectionPatternAt(a, s, v, b, k);
              const thick = thickPatternAt(a, s, v, b, k);
              const one = cellOf(alone, b, k, a, s, v);
              const both = cellOf(summed, b, k, a, s, v);
              for (let c = 0; c < 3; c += 1) {
                const scale = thick.level[c] ?? Number.NaN;
                const curve = delta.curve[c] ?? Number.NaN;
                const largestCurve = Math.max(...delta.curve.map(Math.abs));
                worst = Math.max(
                  worst,
                  Math.abs((one.level[c] ?? 0) - (delta.level[c] ?? 0)) / scale,
                  Math.abs((one.slope[c] ?? 0) - (delta.slope[c] ?? 0)) / scale,
                  Math.abs((both.level[c] ?? 0) - (thick.level[c] ?? 0) - (delta.level[c] ?? 0)) /
                    scale,
                  Math.abs((both.slope[c] ?? 0) - (thick.slope[c] ?? 0) - (delta.slope[c] ?? 0)) /
                    scale,
                );
                // G₂ to 1 in 255 of its texel's largest, from either table.
                expect(Math.abs((one.curve[c] ?? 0) - curve)).toBeLessThanOrEqual(
                  (1 / 255) * largestCurve * (1 + 1e-6),
                );
                expect(both.curve[c]).toBe(one.curve[c]);
              }
            }
          }
        }
      }
    }
    expect(worst).toBeLessThan(1.2 * HALF_ULP);
  });

  it("multiplies the correction by the whole σ_s over the kernels', where a forward delta is out", () => {
    // A kernel medium whose second term lost half its scattering to a forward delta: the drawn
    // medium's whole σ_s is the first term's and twice the second's.
    const [gas, haze] = MEDIUM.terms;
    if (gas === undefined || haze === undefined) {
      throw new Error("the test medium has two terms");
    }
    const kernel: AtmosphereMedium = {
      ...MEDIUM,
      terms: [gas, { ...haze, scattering: [1e-5, 1e-5, 1e-5] }],
    };
    const drawn = alone;
    const halved = packThickTable(null, kernel, { bands: correction, medium: MEDIUM });
    const h = MEDIUM.terms.length;
    expect(h).toBe(2);
    const at = 12_000;
    const sigma = (medium: AtmosphereMedium, c: number): number =>
      medium.terms.reduce(
        (sum, term) =>
          sum +
          (term.scattering[c] ?? 0) *
            Math.exp(-at / (term.density.kind === "exponential" ? term.density.scaleHeightM : 1)),
        0,
      );
    const read = (table: ReturnType<typeof packThickTable>): ThickSource => {
      const out = thickSourceScratch();
      thickBandsInto(table, [0], at, 1, sunAt(4), travelAt(10), 0, out);
      return out;
    };
    for (let c = 0; c < 3; c += 1) {
      const ratio = (read(halved).level[c] ?? 0) / (read(drawn).level[c] ?? 0);
      // The packing's ratio at the rows about 12 km, near the exact one at 12 km.
      expect(ratio).toBeCloseTo(sigma(MEDIUM, c) / sigma(kernel, c), 2);
    }
  });

  it("refuses a correction off the grid, of other bands or slots than the bake's, or of another medium", () => {
    const [c0, c1] = correction;
    if (c0 === undefined || c1 === undefined) {
      throw new Error("a patterned correction has two bands");
    }
    const withCorrection =
      (
        tables: ReadonlyArray<PolarisationTable>,
        medium: AtmosphereMedium = MEDIUM,
      ): (() => unknown) =>
      () =>
        packThickTable(bands, MEDIUM, { bands: tables, medium });
    expect(() => packThickTable(null, MEDIUM, null)).toThrow(/a bake, a correction or both/u);
    expect(withCorrection([c0])).toThrow(/bands/u);
    expect(withCorrection([c0, { ...c1, modes: 2 }])).toThrow(/grid/u);
    expect(withCorrection([c0, { ...c1, slots: c1.slots.slice(1) }])).toThrow(/light slots/u);
    expect(withCorrection([c0, { ...c1, band: { ...c1.band, gravityRatio: 1.2 } }])).toThrow(
      /other bands/u,
    );
    expect(withCorrection(correction, { ...MEDIUM, terms: MEDIUM.terms.slice(1) })).toThrow(
      /another medium/u,
    );
  });
});

/** The bake's J² of the m = 2 tests, sr⁻¹: 0.7 of the pattern's J⁰ at most, of either sign. */
const curveAt = (a: number, s: number, v: number, b: number, k: number, c: number): number =>
  0.7 *
  (thickPatternAt(a, s, v, b, k).level[c] ?? Number.NaN) *
  Math.cos(0.29 * a + 0.37 * s - 0.23 * v + 0.9 * c + b + 0.4 * k);

/** sin² θ_v sin² θ₀ at cell (s, v), as the packing forms it. */
const sinesSquared = (s: number, v: number): number => (1 - sunAt(s) ** 2) * (1 - travelAt(v) ** 2);

/** The patterned bake on {@link TWO_BANDS} with {@link curveAt}'s J² set in place of its zeros. */
function curvedThickBake(): ReadonlyArray<MsSourceTable> {
  const bands = patternedThickBake(TWO_BANDS, MEDIUM, 2);
  for (const [b, band] of bands.entries()) {
    for (const [k, slot] of band.slots.entries()) {
      for (let a = 0; a < THICK_ALTITUDES; a += 1) {
        for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
          for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
            const cell = (a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v;
            for (let c = 0; c < 3; c += 1) {
              slot.values[cell * THICK_SOURCE_MODES * 3 + 6 + c] = curveAt(a, s, v, b, k, c);
            }
          }
        }
      }
    }
  }
  return bands;
}

describe("the bake's m = 2 in G₂ (R08.T14.d.1)", () => {
  const bands = curvedThickBake();
  const correction = patternedCorrection(TWO_BANDS, MEDIUM, 2);

  it("reads G₂ back against its f32 value to 1 in 255 of its texel's largest, alone and with the correction summed in", () => {
    const alone = packThickTable(bands, MEDIUM);
    const summed = packThickTable(bands, MEDIUM, { bands: correction, medium: MEDIUM });
    let checked = 0;
    for (const b of [0, 1]) {
      for (const k of [0, 1]) {
        for (const a of [0, 3, 16, 32]) {
          for (const s of [0, 9, 31]) {
            for (const v of [0, 7, 8, 15]) {
              const cell = (a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v;
              const bake = bands[b]?.slots[k]?.values;
              const delta = correction[b]?.slots[k]?.values;
              // The f32 control: each table's m = 2 over the cell's sines, unrounded past f32.
              const control = (withCorrection: boolean): number[] =>
                [0, 1, 2].map(
                  (c) =>
                    ((bake?.[cell * THICK_SOURCE_MODES * 3 + 6 + c] ?? Number.NaN) +
                      (withCorrection
                        ? (delta?.[cell * THICK_CORRECTION_MODES * 3 + 6 + c] ?? Number.NaN)
                        : 0)) /
                    sinesSquared(s, v),
                );
              for (const [table, expected] of [
                [alone, control(false)],
                [summed, control(true)],
              ] as const) {
                const out = thickSourceScratch();
                const row = thickRowAt(table, table.altitudesM[a] ?? Number.NaN);
                addThickLayer(table, b * 2 + k, row, sunAt(s), travelAt(v), 1, out);
                const largest = Math.max(...expected.map(Math.abs));
                for (let c = 0; c < 3; c += 1) {
                  expect(
                    Math.abs((out.curve[c] ?? 0) - (expected[c] ?? Number.NaN)),
                  ).toBeLessThanOrEqual((1 / 255) * largest * (1 + 1e-6));
                  checked += 1;
                }
              }
            }
          }
        }
      }
    }
    expect(checked).toBe(2 * 2 * 4 * 3 * 4 * 2 * 3);
  });

  it("packs the bake's J² into G₂ alone, every layer's J⁰ and G as without it", () => {
    const plain = packThickTable(patternedThickBake(TWO_BANDS, MEDIUM, 2), MEDIUM);
    const curved = packThickTable(bands, MEDIUM);
    const info = THICK_INFO_ROW * THICK_TRAVEL_COSINES * 4;
    const table = (i: number): boolean => i % LAYER_WORDS < info;
    const differs = curved.texels.some(
      (word, i) => table(i) && i % 4 !== 3 && word !== plain.texels[i],
    );
    const layersWithCurves = new Set(
      Array.from(curved.texels.keys())
        .filter((i) => table(i) && i % 4 === 3 && curved.texels[i] !== 0)
        .map((i) => Math.floor(i / LAYER_WORDS)),
    );
    expect([differs, layersWithCurves.size]).toEqual([false, curved.layers]);
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
    expect(() => packThickTable(withFirst({ modes: 2 }), MEDIUM)).toThrow(/grid/u);
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
    // A J past the half's largest, 65,504 sr⁻¹, at the top.
    const topAt =
      (THICK_ALTITUDES - 1) * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * THICK_SOURCE_MODES * 3;
    expect(() => packThickTable(broken(topAt, 1e5), MEDIUM)).toThrow(/no half float/u);
  });
});
