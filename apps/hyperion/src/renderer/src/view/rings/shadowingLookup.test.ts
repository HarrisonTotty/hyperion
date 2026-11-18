import { describe, expect, it } from "vitest";

import {
  muIndex,
  phaseIndex,
  SHADOWING_NODES,
  SHADOWING_RATIO_TEXTURE_SIZE,
  SHADOWING_TEXTURE_SIZE,
  shadowingLookup,
  shadowingRatios,
  shadowingTexels,
  tauIndex,
} from "./shadowingLookup";
import { SHADOWING_CODES, SHADOWING_HEADER } from "./shadowingTable";

const { tau, mu, phase, codesPerUnit } = SHADOWING_HEADER;

/** Node k's τ_phot, m's μ_eff and j's v, by the table's own spacings (its **Axes**). */
const tauNode = (k: number): number =>
  tau.scale * ((1 + tau.max / tau.scale) ** (k / (tau.nodes - 1)) - 1);
const muNode = (m: number): number => mu.min + (1 - mu.min) * (m / (mu.nodes - 1)) ** 2;
const phaseNode = (j: number): number =>
  phase.scale * ((1 + 1 / phase.scale) ** (j / (phase.nodes - 1)) - 1);

/** A value as the GPU's half-float holds it. */
const half = (v: number): number => new Float16Array([v])[0] ?? Number.NaN;

const CELLS = tau.nodes * mu.nodes * phase.nodes;

describe("the shadowing table's lookups (R11.T9.c)", () => {
  it("inverts each axis's spacing at its nodes", () => {
    for (let k = 0; k < tau.nodes; k += 1) {
      expect(tauIndex(tauNode(k))).toBeCloseTo(k, 9);
    }
    for (let m = 0; m < mu.nodes; m += 1) {
      expect(muIndex(muNode(m))).toBeCloseTo(m, 9);
    }
    for (let j = 0; j < phase.nodes; j += 1) {
      expect(phaseIndex(phaseNode(j))).toBeCloseTo(j, 9);
    }
  });

  it("clamps each index to its axis", () => {
    expect([tauIndex(50), muIndex(0), phaseIndex(2), phaseIndex(-1)]).toEqual([
      tau.nodes - 1,
      0,
      phase.nodes - 1,
      0,
    ]);
  });

  it("reads each node's code, as half-floats, on the lit face", () => {
    const [k, m, j] = [6, 4, 9];
    const at = (k * mu.nodes + m) * phase.nodes + j;
    const sample = shadowingLookup("lit", tauNode(k), muNode(m), phaseNode(j));
    expect(sample.shadowing).toBeCloseTo(
      half((SHADOWING_CODES[at] ?? 0) / codesPerUnit.shadowing),
      6,
    );
    expect(sample.order2).toBeCloseTo(
      half((SHADOWING_CODES[CELLS + at] ?? 0) / codesPerUnit.orders),
      6,
    );
    const unlit = shadowingLookup("unlit", tauNode(k), muNode(m), phaseNode(j));
    expect(unlit.shadowing).toBe(1);
    expect(unlit.order3).toBeCloseTo(
      half((SHADOWING_CODES[4 * CELLS + at] ?? 0) / codesPerUnit.orders),
      6,
    );
  });

  it("leaves the classical layer alone with no layer", () => {
    expect(shadowingLookup("lit", 0, 0.3, 0.2).shadowing).toBe(1);
  });

  it("reads the ratios below 1, per face", () => {
    const lit = shadowingRatios("lit", 1.5, 0.2);
    const unlit = shadowingRatios("unlit", 1.5, 0.2);
    for (const q of [lit.even, lit.odd, unlit.even, unlit.odd]) {
      expect(q).toBeGreaterThanOrEqual(0);
      expect(q).toBeLessThan(1);
    }
  });

  it("lays its texels out as its textures' sizes", () => {
    const texels = shadowingTexels();
    expect(texels.quantities).toHaveLength(SHADOWING_TEXTURE_SIZE.reduce((a, b) => a * b, 4));
    expect(texels.ratios).toHaveLength(SHADOWING_RATIO_TEXTURE_SIZE.reduce((a, b) => a * b, 4));
    expect(SHADOWING_TEXTURE_SIZE).toEqual([
      SHADOWING_NODES.phase,
      SHADOWING_NODES.mu,
      2 * SHADOWING_NODES.tau,
    ]);
  });
});
