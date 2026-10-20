import { describe, expect, it } from "vitest";

import { seededRandom } from "../../test/seededRandom";
import { SLAB_CASES, type SlabCase } from "./slabFixtures";
import { slabMonteCarlo, type SlabMonteCarloInput } from "./slabMonteCarlo";

/**
 * The smoke's photons a case: the twelve cases took 1.12 s together under Vitest on the laptop at a
 * 1-minute load of 2.1, and 1.2 s in plain Node at 2.3 (2026-10-20), under the rulings' 2 s; the
 * bound is then 0.010–0.016.
 */
const SMOKE_PHOTONS = 20_000;

/** The smoke's seed; case i runs on `seededRandom(SMOKE_SEED + i)`. */
const SMOKE_SEED = 0x7a11;

/** The table's stated statistical error, read as one standard deviation. */
const TABLE_SIGMA = 0.002;

/**
 * The smoke's bound on |R_N − R_table|: four combined standard deviations, the run's own,
 * √(R(1 − R) ÷ N), and the table's.
 */
function smokeBound(tableReflectance: number, photons: number): number {
  const sigmaN = Math.sqrt((tableReflectance * (1 - tableReflectance)) / photons);
  return 4 * Math.sqrt(sigmaN * sigmaN + TABLE_SIGMA * TABLE_SIGMA);
}

function runCase(c: SlabCase, seed: number, asymmetry = c.g): number {
  const input: SlabMonteCarloInput = { tau: c.tau, asymmetry, mu0: c.mu0, photons: SMOKE_PHOTONS };
  return slabMonteCarlo(input, seededRandom(seed)).reflectance;
}

describe("slabMonteCarlo's smoke", () => {
  it.each(SLAB_CASES.map((c, i) => ({ ...c, seed: SMOKE_SEED + i })))(
    "reproduces the plan's Monte Carlo column at τ $tau, g $g, μ₀ $mu0",
    (c) => {
      expect(Math.abs(runCase(c, c.seed) - c.monteCarlo)).toBeLessThanOrEqual(
        smokeBound(c.monteCarlo, SMOKE_PHOTONS),
      );
    },
  );

  it("can fail: isotropic scattering falls outside the g = 0.85 row's bound at τ 10, μ₀ 1", () => {
    const row = SLAB_CASES.find((c) => c.tau === 10 && c.g === 0.85 && c.mu0 === 1);
    if (row === undefined) {
      throw new Error("the table has no τ 10, g 0.85, μ₀ 1 row");
    }
    expect(Math.abs(runCase(row, SMOKE_SEED, 0) - row.monteCarlo)).toBeGreaterThan(
      smokeBound(row.monteCarlo, SMOKE_PHOTONS),
    );
  });
});

describe("slabMonteCarlo", () => {
  it.each([0.5, 10])("sends every photon out through the top or the base at τ %f", (tau) => {
    const r = slabMonteCarlo({ tau, asymmetry: 0.85, mu0: 0.5, photons: 2_000 }, seededRandom(7));
    expect(r.reflected + r.transmitted).toBe(r.photons);
  });

  it("transmits the whole beam through a slab of τ = 0", () => {
    const r = slabMonteCarlo(
      { tau: 0, asymmetry: 0.85, mu0: 0.5, photons: 1_000 },
      seededRandom(7),
    );
    expect(r.transmitted).toBe(1_000);
  });

  const valid: SlabMonteCarloInput = { tau: 1, asymmetry: 0.85, mu0: 1, photons: 10 };
  it.each<[string, Partial<SlabMonteCarloInput>]>([
    ["a negative τ", { tau: -1 }],
    ["an infinite τ", { tau: Number.POSITIVE_INFINITY }],
    ["a negative g", { asymmetry: -0.1 }],
    ["g = 1", { asymmetry: 1 }],
    ["μ₀ = 0", { mu0: 0 }],
    ["μ₀ above 1", { mu0: 1.01 }],
    ["no photons", { photons: 0 }],
    ["a fractional photon count", { photons: 1.5 }],
  ])("refuses %s with a RangeError", (_, change) => {
    expect(() => slabMonteCarlo({ ...valid, ...change }, seededRandom(1))).toThrow(RangeError);
  });
});
