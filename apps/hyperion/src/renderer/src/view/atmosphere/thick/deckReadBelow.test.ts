/**
 * The reads below a deck (plan R08, R08.T15.b.2; `deckRead.ts`, "Below the deck"): the packed table
 * below a deck over a surface, its radiance, its ground's light and a path's depth, on the patterned
 * table (`test/thickTables.ts`), and the ground's whole downward flux through the read against the
 * whole column's solve on the NH₃-like deck. Every check is deterministic.
 */
import { describe, expect, it } from "vitest";

import { planckBakeSpectrum, SUN_TEFF_K } from "../../../test/atmosphereFixtures";
import { nh3DeckFixture, saturnBakeBody } from "../../../test/deckFixtures";
import { belowPatternAt, belowPatternDepth, patternedDeckTables } from "../../../test/thickTables";
import { kernelMedium, type TablesBand } from "../tables";
import { THICK_BAKE_GRID, THICK_BAKE_MODES, thickBakeBands, type ThickBakeLight } from "./bake";
import { bakeDeck, DECK_LAYERS, deckColumnSolve, type DeckTables, prepareDeck } from "./deck";
import {
  BELOW_DECK_ALTITUDES,
  BELOW_DECK_LAYER_BYTES,
  BELOW_DECK_MODES,
  BELOW_DECK_SUN_COSINES,
  BELOW_DECK_TABLE_ROWS,
  BELOW_DECK_TABLE_WIDTH,
  BELOW_DECK_TRAVEL_COSINES,
  belowDeckLightInto,
  belowDeckOpticalDepthInto,
  belowDeckPathDepthInto,
  belowDeckRadianceInto,
  belowDeckReadScratch,
  belowDeckSplitM,
  belowDeckTableBytes,
  packBelowDeckTable,
  upperMediumOf,
} from "./deckRead";

/** A medium for the pattern's J above the deck: one grey gas, which the reads below never take. */
const MEDIUM = {
  name: "below deck read test",
  topHeightM: 100_000,
  groundAlbedo: [0.2, 0.2, 0.2] as const,
  terms: [
    {
      name: "gas",
      density: { kind: "exponential" as const, scaleHeightM: 8_000 },
      scattering: [5.8e-6, 1.35e-5, 3.31e-5] as const,
      absorption: [0, 0, 0] as const,
      phase: { kind: "rayleigh" as const, depolarisation: [0.03, 0.03, 0.03] as const },
    },
  ],
};

/** The split: 6 km. */
const SPLIT_M = 6_000;

/** One band at s = 1, and two at s 0.95 and 1.05. */
const ONE_BAND: ReadonlyArray<TablesBand> = [
  { gravityRatio: 1, latitudeRad: 0, bottomRadiusM: 6.052e6, coefficientFactor: 1 },
];
const TWO_BANDS: ReadonlyArray<TablesBand> = [0.95, 1.05].map((s, b) => ({
  gravityRatio: s,
  latitudeRad: b,
  bottomRadiusM: 6e7,
  coefficientFactor: 1 / s,
}));

/** The patterned deck over a surface, its table below it, `slots` light slots. */
function decks(bands: ReadonlyArray<TablesBand>, slots: number): DeckTables[] {
  return patternedDeckTables(
    bands,
    kernelMedium(upperMediumOf(MEDIUM, SPLIT_M)),
    slots,
    SPLIT_M,
    true,
  );
}

/** An array's entry, or a test failure. */
function entry(values: ArrayLike<number>, i: number): number {
  const value = values[i];
  if (value === undefined) {
    throw new Error(`no entry ${i}`);
  }
  return value;
}

/** The pattern's sun cosine of row `s` and travel cosine of column `v`. */
const sunAt = (s: number): number => (s + 0.5) / BELOW_DECK_SUN_COSINES;
const travelAt = (v: number): number => (v + 0.5) / (BELOW_DECK_TRAVEL_COSINES / 2) - 1;
/** The pattern's altitude of row `a`. */
const altitudeAt = (a: number): number => (a * SPLIT_M) / (BELOW_DECK_ALTITUDES - 1);

/** h_⊥ = sin θ_v sin θ₀ cos Δφ of a travel cosine, a sun cosine and an azimuth. */
function horizontalOf(muTravel: number, muSun: number, dphi: number): number {
  return Math.sqrt((1 - muTravel * muTravel) * (1 - muSun * muSun)) * Math.cos(dphi);
}

describe("the table below a deck's shape (R08.T15.b.2)", () => {
  it("is the deck's grid: its 17 altitudes, the daylit 32 sun cosines, 16 travel cosines and the bake's modes", () => {
    expect([
      BELOW_DECK_ALTITUDES,
      BELOW_DECK_SUN_COSINES,
      BELOW_DECK_TRAVEL_COSINES,
      BELOW_DECK_MODES,
    ]).toEqual([
      DECK_LAYERS / THICK_BAKE_GRID.altitudeStride + 1,
      THICK_BAKE_GRID.sunCosines.filter((mu) => mu > 0).length,
      2 * THICK_BAKE_GRID.upwardCosines.length,
      THICK_BAKE_MODES,
    ]);
  });

  it("packs a band and light into 436,000 B, 50 × 545 texels, its values' f32 bits", () => {
    const packed = packBelowDeckTable(decks(TWO_BANDS, 2));
    expect([
      packed.widthTexels,
      packed.heightTexels,
      packed.layers,
      packed.texels.length,
      BELOW_DECK_LAYER_BYTES,
      belowDeckTableBytes(packed),
      belowDeckSplitM(packed),
    ]).toEqual([
      BELOW_DECK_TABLE_WIDTH,
      BELOW_DECK_TABLE_ROWS,
      4,
      4 * BELOW_DECK_TABLE_WIDTH * BELOW_DECK_TABLE_ROWS * 4,
      436_000,
      4 * 436_000,
      SPLIT_M,
    ]);
  });

  it("refuses an envelope's deck, which has none, a table off the grid, or a value not finite", () => {
    const [band] = decks(ONE_BAND, 1);
    const below = band?.below;
    if (band === undefined || below === null || below === undefined) {
      throw new Error("the pattern has a band with a table below");
    }
    const [slot] = below.slots;
    if (slot === undefined) {
      throw new Error("a slot");
    }
    const nan = Float32Array.from(slot.radiance, (x, i) => (i === 7 ? Number.NaN : x));
    expect(() => packBelowDeckTable([])).toThrow(RangeError);
    expect(() => packBelowDeckTable([{ ...band, below: null }])).toThrow(RangeError);
    expect(() =>
      packBelowDeckTable([{ ...band, below: { ...below, sunCosines: below.sunCosines.slice(1) } }]),
    ).toThrow(RangeError);
    expect(() =>
      packBelowDeckTable([{ ...band, below: { ...below, slots: [{ ...slot, radiance: nan }] } }]),
    ).toThrow(RangeError);
  });
});

describe("the reads below a deck (R08.T15.b.2)", () => {
  const packed = packBelowDeckTable(decks(ONE_BAND, 2));
  const scratch = belowDeckReadScratch();
  const out: [number, number, number] = [0, 0, 0];
  const sky: [number, number, number] = [0, 0, 0];
  const sun: [number, number, number] = [0, 0, 0];

  it("reads the radiance's modes at each cell's centre, every azimuth and slot, to f32", () => {
    let worst = 0;
    for (const k of [0, 1]) {
      for (const a of [0, 5, 16]) {
        for (const s of [0, 13, 31]) {
          for (const v of [0, 6, 9, 15]) {
            for (const dphi of [0, 0.7, Math.PI / 2, 2.4, Math.PI]) {
              const mu0 = sunAt(s);
              const mu = travelAt(v);
              belowDeckRadianceInto(
                packed,
                [0],
                1,
                altitudeAt(a),
                mu0,
                mu,
                horizontalOf(mu, mu0, dphi),
                k,
                scratch,
                out,
              );
              const sines = Math.sqrt((1 - mu0 * mu0) * (1 - mu * mu));
              for (let c = 0; c < 3; c += 1) {
                let expected = 0;
                for (let m = 0; m < BELOW_DECK_MODES; m += 1) {
                  expected +=
                    Math.fround(belowPatternAt(a, s, v, 0, k, m, c).radiance * sines ** m) *
                    Math.cos(m * dphi);
                }
                worst = Math.max(worst, Math.abs(entry(out, c) - Math.max(expected, 0)) / 0.02);
              }
            }
          }
        }
      }
    }
    expect(worst).toBeLessThan(1e-6);
  });

  it("reads linearly between the cells' centres in h*, μ₀ and μ", () => {
    // Halfway between rows 3 and 4, suns 10 and 11 and travel cosines 4 and 5, Δφ 0: the mean of
    // the eight corners' Σ_m Iᵐ.
    const h = 0.5 * (altitudeAt(3) + altitudeAt(4));
    const mu0 = 0.5 * (sunAt(10) + sunAt(11));
    const mu = 0.5 * (travelAt(4) + travelAt(5));
    belowDeckRadianceInto(packed, [0], 1, h, mu0, mu, horizontalOf(mu, mu0, 0), 0, scratch, out);
    const sines = Math.sqrt((1 - mu0 * mu0) * (1 - mu * mu));
    for (let c = 0; c < 3; c += 1) {
      let expected = 0;
      for (const a of [3, 4]) {
        for (const s of [10, 11]) {
          for (const v of [4, 5]) {
            const stored = Math.sqrt((1 - sunAt(s) ** 2) * (1 - travelAt(v) ** 2));
            for (let m = 0; m < BELOW_DECK_MODES; m += 1) {
              expected +=
                (Math.fround(belowPatternAt(a, s, v, 0, 0, m, c).radiance * stored ** m) /
                  stored ** m) *
                sines ** m;
            }
          }
        }
      }
      expect(entry(out, c)).toBeCloseTo(expected / 8, 6);
    }
  });

  it("scales the field and the sky to 0 under the first sun row, and holds none below the horizontal", () => {
    const first = sunAt(0);
    const at = (mu0: number): number[] => {
      // Near the split, where the pattern's beam at the lowest sun lies in f32's range.
      belowDeckRadianceInto(packed, [0], 1, 5_900, mu0, -0.9, 0, 0, scratch, out);
      belowDeckLightInto(packed, [0], 1, 5_900, mu0, 0, scratch, sky, sun);
      return [out[1], sky[1], sun[1]];
    };
    const [radiance, irradiance, beam] = at(first);
    const [halfRadiance, halfIrradiance, halfBeam] = at(first / 2);
    expect([
      (halfRadiance ?? Number.NaN) / (radiance ?? Number.NaN),
      (halfIrradiance ?? Number.NaN) / (irradiance ?? Number.NaN),
      (halfBeam ?? Number.NaN) / (beam ?? Number.NaN) ** 2,
      at(0),
      at(-0.2),
    ]).toEqual([
      // The radiance's m = 2 takes μ₀'s sine, so its share moves a little with μ₀.
      expect.closeTo(0.5, 5),
      expect.closeTo(0.5, 12),
      expect.closeTo(1, 6),
      [0, 0, 0],
      [0, 0, 0],
    ]);
  });

  it("reads the beam exactly for a column of even extinction, between the rows and beyond them, to f32", () => {
    let worst = 0;
    for (const h of [0, 777, 3_000, 4_100, SPLIT_M]) {
      for (const mu0 of [0.004, 0.0156, 0.031, 0.2, 0.33, 0.5517, 0.97, 0.99, 1]) {
        belowDeckLightInto(packed, [0], 1, h, mu0, 1, scratch, sky, sun);
        for (let c = 0; c < 3; c += 1) {
          const expected = Math.exp(-belowPatternDepth(h, SPLIT_M, 0, c) / mu0);
          // Where an altitude about it underflows f32 at that sun, the read is linear in h*: held
          // to 10⁻¹⁵ of the sun's light, nil beside the sky's.
          worst = Math.max(
            worst,
            expected > 1e-20
              ? Math.abs(entry(sun, c) / expected - 1)
              : Math.abs(entry(sun, c) - expected) / 1e-10,
          );
        }
      }
    }
    // The f32's rounding of e^(−τ ÷ μ₀) at the rows, raised to 1 ÷ μ₀ at the lowest suns.
    expect(worst).toBeLessThan(1e-5);
  });

  it("reads the sky's irradiance at each cell's centre, to f32", () => {
    let worst = 0;
    for (const a of [0, 7, 16]) {
      for (const s of [0, 1, 20, 31]) {
        belowDeckLightInto(packed, [0], 1, altitudeAt(a), sunAt(s), 1, scratch, sky, sun);
        for (let c = 0; c < 3; c += 1) {
          const expected = Math.fround(belowPatternAt(a, s, 0, 0, 1, 0, c).irradiance);
          worst = Math.max(worst, Math.abs(entry(sky, c) - expected) / expected);
        }
      }
    }
    expect(worst).toBeLessThan(1e-6);
  });

  it("reads a path's depth as the plane-parallel path's, slant and level", () => {
    const slant: [number, number, number] = [0, 0, 0];
    const level: [number, number, number] = [0, 0, 0];
    const vertical: [number, number, number] = [0, 0, 0];
    // From 500 m up to 2,500 m along 4 km, and 4 km level at 500 m, on a gravity ratio of 1.1.
    belowDeckPathDepthInto(packed, [0], 1.1, 550, 2_750, 4_000, scratch, slant);
    belowDeckPathDepthInto(packed, [0], 1.1, 550, 550, 4_000, scratch, level);
    belowDeckOpticalDepthInto(packed, [0], 1, 0, scratch, vertical);
    for (let c = 0; c < 3; c += 1) {
      const sigmaStar = (3 + 0.2 * c) / SPLIT_M;
      expect([entry(slant, c), entry(level, c), entry(vertical, c)]).toEqual([
        expect.closeTo(4_000 * 1.1 * sigmaStar, 5),
        expect.closeTo(4_000 * 1.1 * sigmaStar, 5),
        expect.closeTo(belowPatternDepth(0, SPLIT_M, 0, c), 5),
      ]);
    }
  });

  it("reads linearly in ln s between two bands", () => {
    const two = packBelowDeckTable(decks(TWO_BANDS, 1));
    const lnBands = TWO_BANDS.map((band) => Math.log(band.gravityRatio));
    const read = (s: number): number => {
      belowDeckRadianceInto(two, lnBands, s, 1_000, 0.5, -0.6, 0.1, 0, scratch, out);
      return out[0];
    };
    const middle = Math.sqrt(0.95 * 1.05);
    expect(read(middle)).toBeCloseTo(0.5 * (read(0.95) + read(1.05)), 9);
    expect([read(0.9), read(1.1)]).toEqual([read(0.95), read(1.05)]);
  });
});

describe("the table below a deck's light against the whole column's solve (R08.T15.b.2)", () => {
  /** The Sun with no absorber's curves: the fixture has no absorber term. */
  const SUN: ThickBakeLight = { spectrum: planckBakeSpectrum(SUN_TEFF_K), absorbers: [] };

  it(
    "gives the ground's whole downward flux, the sky plus μ₀ times the beam, at every altitude of the table, to 10⁻⁶",
    { timeout: 300_000 },
    () => {
      const fixture = nh3DeckFixture();
      const prepared = prepareDeck(fixture.column, [SUN], {
        altitudeM: fixture.deckAltitudeM,
        floor: "ground",
      });
      const body = saturnBakeBody();
      const band = thickBakeBands(body)[0];
      const light = prepared.upper.lights[0];
      if (band === undefined || light === undefined) {
        throw new Error("a band and a light");
      }
      const table = bakeDeck(prepared, body, 0);
      const below = table.below;
      if (below === null) {
        throw new Error("a deck over a surface has a table below it");
      }
      const packed = packBelowDeckTable([table]);
      const lnBands = [Math.log(band.gravityRatio)];
      const grid = prepared.upper.grid;
      const ground = grid.layers + (prepared.deck.interfaceHeightsM.length - 1);
      // Each bin's whole downward flux, diffuse and delta-M's beam, at the table's altitudes.
      const fluxes = light.bins.map((bin, b) => {
        const term = bin.terms[0];
        if (term === undefined) {
          throw new Error(`bin ${b}'s term`);
        }
        const { solution } = deckColumnSolve(prepared, band, b, term);
        return Array.from({ length: BELOW_DECK_ALTITUDES }, (_, a) =>
          solution.beams.map((beam) => {
            const k = ground - a * grid.altitudeStride;
            return entry(beam.fluxes.diffuseDown, k) + entry(beam.fluxes.direct, k);
          }),
        );
      });
      const scratch = belowDeckReadScratch();
      const sky: [number, number, number] = [0, 0, 0];
      const sun: [number, number, number] = [0, 0, 0];
      let worst = 0;
      for (let a = 0; a < BELOW_DECK_ALTITUDES; a += 1) {
        below.sunCosines.forEach((mu0, s) => {
          belowDeckLightInto(
            packed,
            lnBands,
            band.gravityRatio,
            entry(below.altitudesM, a),
            mu0,
            0,
            scratch,
            sky,
            sun,
          );
          light.conversion.factors.forEach((factors, c) => {
            let expected = 0;
            fluxes.forEach((perAltitude, b) => {
              expected += entry(factors, b) * entry(perAltitude[a] ?? [], s);
            });
            const read = entry(sky, c) + mu0 * entry(sun, c);
            worst = Math.max(worst, Math.abs(read - expected) / expected);
          });
        });
      }
      expect(worst).toBeLessThan(1e-6);
    },
  );
});
