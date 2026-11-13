import { describe, expect, it } from "vitest";

import type { LatticePlace } from "./decorationLattice";
import {
  biplanarWeights,
  type DetailProjection,
  type DetailSample,
  detailFirst,
  detailProjected,
  detailSecond,
  triplanarWeights,
} from "./detailTextures";

const PROJECTION: DetailProjection = {
  period: 3,
  offsets: [7, 1_000_003, 41, 4_000_000_007],
  heightCells: 12.3,
};

/** A zero-mean, periodic set: sinusoids of whole turns across the tile in every channel. */
const SAMPLE: DetailSample = (u, v) => [
  Math.sin(2 * Math.PI * (u + 2 * v)),
  Math.cos(2 * Math.PI * (3 * u - v)),
  0.1 * Math.sin(2 * Math.PI * (5 * u)),
  0.05 * Math.cos(2 * Math.PI * (u + v)),
];

/** The place of a point x, y (cells) of the set's lattice. */
function placeAt(x: number, y: number): LatticePlace {
  const cx = Math.floor(x);
  const cy = Math.floor(y);
  return { x: cx, y: cy, fx: x - cx, fy: y - cy };
}

describe("the detail textures' coordinates", () => {
  it("run on across a cell's edge with no step, on either scale", () => {
    for (const [x, y] of [
      [5, 2.25],
      [9, 7.5],
      [104, 3.125],
    ] as const) {
      const before = placeAt(x - 1e-9, y);
      const after: LatticePlace = { x, y: Math.floor(y), fx: 0, fy: y - Math.floor(y) };
      for (const scale of [detailFirst, detailSecond]) {
        const a = scale(before, PROJECTION);
        const b = scale(after, PROJECTION);
        // Coordinates wrap at the tile's edge, so compare them on the circle.
        for (let c = 0; c < 2; c += 1) {
          const d = Math.abs((a[c] ?? 0) - (b[c] ?? 0));
          expect(Math.min(d, 1 - d)).toBeLessThan(1e-6);
        }
      }
    }
  });

  it("repeat every period cells on the first scale and every three periods on the second", () => {
    const at = placeAt(2.3, 4.6);
    const period = PROJECTION.period;
    const first = detailFirst(at, PROJECTION);
    const next = detailFirst(placeAt(2.3 + period, 4.6), PROJECTION);
    expect(Math.abs(next[0] - first[0])).toBeLessThan(1e-12);
    expect(Math.abs(next[1] - first[1])).toBeLessThan(1e-12);
    const second = detailSecond(at, PROJECTION);
    const later = detailSecond({ ...at, x: at.x + 15 * period }, PROJECTION);
    expect(Math.abs(later[0] - second[0])).toBeLessThan(1e-12);
    expect(Math.abs(later[1] - second[1])).toBeLessThan(1e-12);
  });
});

describe("the detail textures' weights", () => {
  it("sum to 1 and weigh the plane the normal faces most", () => {
    for (const weights of [triplanarWeights, biplanarWeights]) {
      const w = weights([0.2, -0.3, 0.93]);
      expect(w[0] + w[1] + w[2]).toBeCloseTo(1, 12);
      expect(w[2]).toBeGreaterThan(w[1]);
      expect(w[1]).toBeGreaterThan(w[0]);
    }
  });

  it("drop the least plane in the biplanar variant, with no step where it changes", () => {
    expect(biplanarWeights([0.2, -0.3, 0.93])[0]).toBe(0);
    const a = biplanarWeights([0.3 - 1e-9, 0.3 + 1e-9, 0.9]);
    const b = biplanarWeights([0.3 + 1e-9, 0.3 - 1e-9, 0.9]);
    for (let c = 0; c < 3; c += 1) {
      expect(Math.abs((a[c] ?? 0) - (b[c] ?? 0))).toBeLessThan(1e-6);
    }
  });
});

describe("the detail textures' projection (Design note 3)", () => {
  // A patch of 45 × 45 of the set's cells, whole periods of both scales (3 and 15 × 3 cells),
  // sampled 8 times a cell.
  it.each([
    ["flat ground", [0, 0, 1]],
    ["a steep slope", [0.6, 0.3, 0.74]],
  ] as const)("is zero-mean over a patch to 10⁻³ on %s, by both projections", (_name, along) => {
    for (const weights of [triplanarWeights, biplanarWeights]) {
      const w = weights(along);
      const n = 45 * 8;
      const sums = [0, 0, 0, 0];
      for (let a = 0; a < n; a += 1) {
        for (let b = 0; b < n; b += 1) {
          const d = detailProjected(SAMPLE, placeAt((a + 0.5) / 8, (b + 0.5) / 8), PROJECTION, w);
          for (let c = 0; c < 4; c += 1) {
            sums[c] = (sums[c] ?? 0) + (d[c] ?? 0);
          }
        }
      }
      for (const sum of sums) {
        expect(Math.abs(sum / (n * n))).toBeLessThan(1e-3);
      }
    }
  });
});
