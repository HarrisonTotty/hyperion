import { beforeAll, describe, expect, it } from "vitest";

import {
  authoritativeRockDiameterM,
  initSync,
  rockNumberBetweenPerM2,
} from "../../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../../generated/surface/hyperion_surface_bg.wasm?inline";
import {
  cellPebbles,
  numberDensityPerM2PerM,
  PEBBLE_LARGEST_DIAMETER_M,
  pebbleCellHash,
  proposalsPerM2,
} from "./pebbleLaw";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

/** The pebbles of octave 0, 0.2 to 0.4 m, over `cells` cells of `mean` proposals at `k`. */
function topOctave(k: number, cells: number, mean: number): number[] {
  const out: number[] = [];
  const shares = [0, 1, 0, 0, 0, 0, 0, 0, 0];
  const abundances = [k, 0, 0, 0, 0, 0, 0, 0];
  for (let c = 0; c < cells; c += 1) {
    for (const p of cellPebbles(
      pebbleCellHash(0xc0ffee, 0, 4, 22, c, c ^ 0x55),
      0,
      mean,
      shares,
      abundances,
      1,
    )) {
      out.push(p.diameterM);
    }
  }
  return out;
}

describe("the pebbles against the authoritative rocks' law (R11.T3)", () => {
  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  it("split at the rocks' authoritative diameter, read from the surface crate", () => {
    expect(PEBBLE_LARGEST_DIAMETER_M).toBe(authoritativeRockDiameterM());
  });

  it("take the rocks' n(D), the slope of their N(>D)", () => {
    for (const k of [0.01, 0.039, 0.07, 0.18, 0.4]) {
      for (const d of [0.02, 0.1, 0.3, 0.39]) {
        const h = 1e-5 * d;
        const slope = rockNumberBetweenPerM2(k, d - h, d + h) / (2 * h);
        expect(Math.abs(numberDensityPerM2PerM(k, d) / slope - 1)).toBeLessThan(1e-5);
      }
    }
  });

  it("run on across 0.4 m: the count per size bin just below it the rocks' law's, within the Poisson interval", () => {
    for (const k of [0.07, 0.18]) {
      const cells = 40_000;
      const mean = 8;
      const area = (cells * mean) / proposalsPerM2(0);
      const pebbles = topOctave(k, cells, mean);
      // Bins of 1 cm below the split, each against the law's count there (the rocks' own,
      // `RockAbundance::number_between_per_m2`), and the law's first bin above it, where the rocks
      // stand, its count per centimetre the last bin's to within the law's slope over 1 cm.
      for (const [from, to] of [
        [0.37, 0.38],
        [0.38, 0.39],
        [0.39, 0.4],
      ] as const) {
        const counted = pebbles.filter((d) => d >= from && d < to).length;
        const expected = area * rockNumberBetweenPerM2(k, from, to);
        expect(expected).toBeGreaterThan(400);
        expect(Math.abs(counted - expected)).toBeLessThan(4.5 * Math.sqrt(expected));
      }
      const below = pebbles.filter((d) => d >= 0.39 && d < 0.4).length;
      const above = area * rockNumberBetweenPerM2(k, 0.4, 0.41);
      // The law's own ratio across the split, e^(−q × 1 cm) times (0.395 ÷ 0.405)² about.
      const lawRatio = rockNumberBetweenPerM2(k, 0.4, 0.41) / rockNumberBetweenPerM2(k, 0.39, 0.4);
      expect(lawRatio).toBeGreaterThan(0.9);
      expect(lawRatio).toBeLessThan(1);
      expect(Math.abs(below * lawRatio - above)).toBeLessThan(4.5 * Math.sqrt(above));
    }
  });
});
