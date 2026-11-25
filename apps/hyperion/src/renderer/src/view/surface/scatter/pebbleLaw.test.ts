import { describe, expect, it } from "vitest";

import { latticeHash } from "../decoration/decorationLattice";
import {
  cellPebbles,
  cellProposals,
  MAX_ROCK_ABUNDANCE,
  numberDensityPerM2PerM,
  PEBBLE_AXES,
  PEBBLE_LARGEST_DIAMETER_M,
  PEBBLE_OCTAVES,
  PEBBLE_PROPOSAL_COEFFICIENT,
  PEBBLE_SALT,
  pebbleAcceptance,
  pebbleCellHash,
  pebbleDiameter,
  pebbleFade,
  pebbleLatticeLevels,
  pebbleTint,
  poissonCount,
  proposalsPerM2,
  scatterDensity,
  slopeFilter,
} from "./pebbleLaw";

/** The slope's filter at a slope of `deg` degrees. */
function at(deg: number): number {
  return slopeFilter(Math.cos((deg * Math.PI) / 180));
}

/** A ground of one palette slot at `k`, its share whole. */
function ground(k: number): { readonly shares: number[]; readonly abundances: number[] } {
  return { shares: [0, 1, 0, 0, 0, 0, 0, 0, 0], abundances: [k, 0, 0, 0, 0, 0, 0, 0] };
}

/** The pebbles of `cells` cells of each octave at `mean` proposals, on `k`'s ground at `density`. */
function field(
  k: number,
  density: number,
  cells: number,
  mean = 8,
): Array<{ readonly octave: number; readonly diameterM: number }> {
  const { shares, abundances } = ground(k);
  const out: Array<{ readonly octave: number; readonly diameterM: number }> = [];
  PEBBLE_OCTAVES.forEach((_, octave) => {
    for (let c = 0; c < cells; c += 1) {
      const hash = pebbleCellHash(0x5eed, octave, 1, 20, c, 7 * c + 3);
      for (const p of cellPebbles(hash, octave, mean, shares, abundances, density)) {
        out.push({ octave, diameterM: p.diameterM });
      }
    }
  });
  return out;
}

describe("the pebbles' law", () => {
  it("draws no decorative pebble 0.4 m across or more, over 10⁵ draws and at the top of every octave", () => {
    let draws = 0;
    let largest = 0;
    PEBBLE_OCTAVES.forEach(([low, high], octave) => {
      for (let c = 0; c < 2_500; c += 1) {
        for (const p of cellProposals(pebbleCellHash(1, octave, 2, 22, c, c), octave, 10)) {
          draws += 1;
          largest = Math.max(largest, p.diameterM);
          expect(p.diameterM).toBeGreaterThanOrEqual(Math.fround(low));
          expect(p.diameterM).toBeLessThan(high);
        }
      }
      // The largest uniform a word gives, 1 − 2⁻²⁴, and the edge of the f32 range.
      expect(pebbleDiameter(octave, 1 - 2 ** -24)).toBeLessThan(high);
      // At u = 0, low to its f32 rounding.
      expect(Math.abs(pebbleDiameter(octave, 0) - low)).toBeLessThan(1e-6 * low);
    });
    expect(draws).toBeGreaterThan(100_000);
    expect(largest).toBeLessThan(PEBBLE_LARGEST_DIAMETER_M);
    expect(PEBBLE_OCTAVES[0]?.[1]).toBe(PEBBLE_LARGEST_DIAMETER_M);
  });

  it("proposes at an intensity above the law at every abundance and diameter, so that thinning keeps it", () => {
    for (const k of [0.001, 0.01, 0.05, 0.1, 0.2, 0.3, MAX_ROCK_ABUNDANCE]) {
      for (const d of [0.0125, 0.02, 0.05, 0.1, 0.2, 0.3, 0.399]) {
        const law = numberDensityPerM2PerM(k, d);
        const proposed = PEBBLE_PROPOSAL_COEFFICIENT / (d * d);
        expect(law).toBeLessThanOrEqual(proposed);
        const { shares, abundances } = ground(k);
        expect(pebbleAcceptance(shares, abundances, d)).toBeCloseTo(law / proposed, 12);
      }
    }
  });

  it("keeps per size bin the law's count within its Poisson interval", () => {
    const k = 0.18;
    const cells = 4_000;
    const mean = 8;
    const pebbles = field(k, 1, cells, mean);
    PEBBLE_OCTAVES.forEach(([low, high], octave) => {
      // The cells' area, m², from their mean proposals: mean ÷ (proposals per m²).
      const area = (cells * mean) / proposalsPerM2(octave);
      const bins = 4;
      for (let b = 0; b < bins; b += 1) {
        const from = low * (high / low) ** (b / bins);
        const to = low * (high / low) ** ((b + 1) / bins);
        // ∫ n(D) dD over the bin, by Simpson's rule on 64 panels.
        const n = 64;
        const h = (to - from) / n;
        let integral = 0;
        for (let s = 0; s <= n; s += 1) {
          const weight = s === 0 || s === n ? 1 : s % 2 === 1 ? 4 : 2;
          integral += weight * numberDensityPerM2PerM(k, from + s * h);
        }
        const expected = (area * integral * h) / 3;
        const counted = pebbles.filter(
          (p) => p.octave === octave && p.diameterM >= from && p.diameterM < to,
        ).length;
        // 4.5σ of a Poisson count, a 10⁻⁵ chance of a false alarm over the 20 bins.
        expect(Math.abs(counted - expected)).toBeLessThan(4.5 * Math.sqrt(expected) + 1);
      }
    });
  });

  it("scales the count linearly with the scatter density, every pebble of a lower density in the higher's set", () => {
    const k = 0.2;
    const full = field(k, 1, 600);
    const half = field(k, 0.5, 600);
    const quarter = field(k, 0.25, 600);
    expect(full.length).toBeGreaterThan(5_000);
    for (const [part, share] of [
      [half, 0.5],
      [quarter, 0.25],
    ] as const) {
      // A binomial thinning of the full set at `share`: within 4.5σ.
      const sigma = Math.sqrt(full.length * share * (1 - share));
      expect(Math.abs(part.length - share * full.length)).toBeLessThan(4.5 * sigma);
    }
    expect(field(k, 0, 600)).toHaveLength(0);
    // Thinning by one keep word: the lower density's pebbles are the higher's.
    const { shares, abundances } = ground(k);
    const placesAt = (hash: number, d: number): string[] =>
      cellPebbles(hash, 2, 8, shares, abundances, d).map((p) => `${p.at[0]},${p.at[1]}`);
    for (let c = 0; c < 50; c += 1) {
      const hash = pebbleCellHash(9, 2, 3, 21, c, c + 1);
      const whole = new Set(placesAt(hash, 1));
      expect(placesAt(hash, 0.5).every((p) => whole.has(p))).toBe(true);
    }
    expect(scatterDensity(2)).toBe(1);
    expect(scatterDensity(-1)).toBe(0);
    expect(scatterDensity(Number.NaN)).toBe(0);
  });

  it("keeps no pebble where the ground bears no rocks, nor on the body's uniform law", () => {
    expect(pebbleAcceptance([0, 1, 0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0, 0, 0], 0.05)).toBe(0);
    expect(pebbleAcceptance([1, 0, 0, 0, 0, 0, 0, 0, 0], [0.2, 0.2, 0, 0, 0, 0, 0, 0], 0.05)).toBe(
      0,
    );
    // Two slots by their shares: the mixture of their laws.
    const mixed = pebbleAcceptance(
      [0, 0.25, 0.75, 0, 0, 0, 0, 0, 0],
      [0.2, 0.05, 0, 0, 0, 0, 0, 0],
      0.1,
    );
    const rocky = ground(0.2);
    const dusty = ground(0.05);
    const a = pebbleAcceptance(rocky.shares, rocky.abundances, 0.1);
    const b = pebbleAcceptance(dusty.shares, dusty.abundances, 0.1);
    expect(mixed).toBeCloseTo(0.25 * a + 0.75 * b, 12);
  });

  it("draws Poisson counts of their mean", () => {
    const mean = 6;
    let sum = 0;
    let squares = 0;
    const n = 50_000;
    for (let c = 0; c < n; c += 1) {
      const count = poissonCount(mean, ((c + 0.5) / n) % 1);
      sum += count;
      squares += count * count;
    }
    const m = sum / n;
    expect(m).toBeCloseTo(mean, 2);
    expect(squares / n - m * m).toBeCloseTo(mean, 1);
    expect(poissonCount(0, 0.5)).toBe(0);
  });

  it("keys a cell by the decoration's lattice hash under the octave's salt, the same from any patch", () => {
    expect(pebbleCellHash(5, 3, 2, 24, 100, 200)).toBe(
      latticeHash(5, PEBBLE_SALT + 3, 2, 24, 100, 200),
    );
    expect(pebbleCellHash(5, 3, 2, 24, 100, 200)).not.toBe(pebbleCellHash(5, 4, 2, 24, 100, 200));
  });

  it("sets each octave's lattice so that a cell takes a few proposals", () => {
    const finestPatchM = 32;
    const levels = pebbleLatticeLevels(19, finestPatchM);
    levels.forEach((level, octave) => {
      const side = finestPatchM / 2 ** (level - 19);
      const mean = proposalsPerM2(octave) * side * side;
      expect(level).toBeGreaterThanOrEqual(19);
      expect(mean).toBeLessThanOrEqual(12);
      expect(mean).toBeGreaterThan(2);
    });
  });

  it("keeps pebbles off slopes past the angle of repose", () => {
    expect(at(0)).toBe(1);
    expect(at(30)).toBeCloseTo(1, 12);
    expect(at(35)).toBeGreaterThan(0);
    expect(at(35)).toBeLessThan(1);
    expect(at(40)).toBeCloseTo(0, 12);
    expect(at(60)).toBe(0);
  });

  it("fades a pebble across one octave of footprint, whole from 4 px and gone at 2 px", () => {
    const pxPerRad = 1000;
    expect(pebbleFade(0.04, 10, pxPerRad)).toBe(1);
    expect(pebbleFade(0.02, 10, pxPerRad)).toBe(0);
    expect(pebbleFade(0.03, 10, pxPerRad)).toBeCloseTo(Math.log2(1.5), 12);
  });

  it("is a half-buried ellipsoid of the rocks' middle proportions, its outline π D² ÷ 4", () => {
    const [a, b, c] = PEBBLE_AXES;
    expect(a * b).toBeCloseTo(1, 12);
    expect(b / a).toBeCloseTo(Math.SQRT1_2, 12);
    expect(c).toBe(0.75);
    expect(pebbleTint(0)).toBe(0.75);
    expect(pebbleTint(255)).toBe(1.25);
  });
});
