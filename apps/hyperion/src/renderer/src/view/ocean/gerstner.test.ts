import { describe, expect, it } from "vitest";

import {
  DIRECTION_SPREAD_RAD,
  eulerianMeanHeightM,
  FINCH_LIMIT,
  gerstnerDisplacement,
  gerstnerJacobian,
  type GerstnerSet,
  gerstnerSet,
  resolvedSlopeVariance,
  STOKES_LIMIT,
  summedSteepness,
  unresolvedSlopeVariance,
} from "./gerstner";
import { EARTH_SEA, type SeaState, seaState, SLOPE_VARIANCE_FLOOR } from "./seaState";

/** Earth's sea at a 10 m wind and gravity. */
function sea(windMps: number, gravityMps2 = EARTH_SEA.gravityMps2): SeaState {
  return seaState({
    windMps,
    windHeightM: 10,
    gravityMps2,
    liquid: EARTH_SEA.liquid,
    air: EARTH_SEA.air,
    fetchM: null,
  });
}

/** The longest component's wavelength of a sea, m: twice its peak's. */
function longestM(state: SeaState): number {
  return (2 * 2 * Math.PI) / state.peakWavenumberRadPerM;
}

const WINDS_MPS = [0, 0.5, 1, 2, 3, 5, 8, 10, 15, 20, 25, 30];
const GRAVITIES_G = [0.1, 0.3, 1, 2, 3];
const COUNTS = [8, 32];

/** Every set of the test grid: winds 0–30 m/s, 0.1–3 g, 8 and 32 components, two cuts. */
function everySet(): ReadonlyArray<{ readonly state: SeaState; readonly set: GerstnerSet }> {
  const sets: { state: SeaState; set: GerstnerSet }[] = [];
  for (const wind of WINDS_MPS) {
    for (const g of GRAVITIES_G) {
      const state = sea(wind, g * EARTH_SEA.gravityMps2);
      for (const count of COUNTS) {
        for (const cut of [0.02, longestM(state) / 200]) {
          sets.push({ state, set: gerstnerSet(state, count, cut) });
        }
      }
    }
  }
  return sets;
}

describe("gerstnerSet", () => {
  it("keeps every set within Finch's sum and each component within the Stokes limit", () => {
    let drawn = 0;
    for (const { set } of everySet()) {
      expect(summedSteepness(set)).toBeLessThanOrEqual(FINCH_LIMIT);
      for (const c of set.components) {
        expect(c.wavenumberRadPerM * c.amplitudeM).toBeLessThanOrEqual(STOKES_LIMIT);
        expect(c.steepness).toBeGreaterThan(0);
        expect(c.steepness).toBeLessThanOrEqual(1);
        expect(Math.abs(c.directionRad)).toBeLessThanOrEqual(DIRECTION_SPREAD_RAD);
      }
      drawn += set.components.length;
    }
    expect(drawn).toBeGreaterThan(0);
  });

  it("makes up each axis's Cox–Munk total, the unresolved never below the floor", () => {
    for (const { state, set } of everySet()) {
      const resolved = resolvedSlopeVariance(set);
      const unresolved = unresolvedSlopeVariance(set);
      expect(
        Math.abs(resolved.upwind + unresolved.upwind - state.slopeVariance.upwind),
      ).toBeLessThan(1e-6);
      expect(
        Math.abs(resolved.crosswind + unresolved.crosswind - state.slopeVariance.crosswind),
      ).toBeLessThan(1e-6);
      expect(unresolved.upwind).toBeGreaterThanOrEqual(SLOPE_VARIANCE_FLOOR);
      expect(unresolved.crosswind).toBeGreaterThanOrEqual(SLOPE_VARIANCE_FLOOR);
    }
  });

  it("scales the amplitudes down where the resolved slopes would pass the budget", () => {
    // A budget below what the spectrum's tail resolves: a third of Cox and Munk's at 20 m/s.
    const strong = sea(20);
    const state: SeaState = {
      ...strong,
      slopeVariance: {
        upwind: strong.slopeVariance.upwind / 3,
        crosswind: strong.slopeVariance.crosswind / 3,
      },
    };
    const set = gerstnerSet(state, 32, 0.005);
    expect(set.amplitudeScale).toBeLessThan(1);
    const unresolved = unresolvedSlopeVariance(set);
    expect(Math.min(unresolved.upwind, unresolved.crosswind)).toBeCloseTo(SLOPE_VARIANCE_FLOOR, 9);
  });

  it("draws nothing on a glassy sea or past the longest component", () => {
    expect(gerstnerSet(sea(0.5), 32, 0.02).components).toEqual([]);
    const state = sea(10);
    expect(gerstnerSet(state, 32, 2 * longestM(state)).components).toEqual([]);
  });

  it("draws the same set for the same sea", () => {
    expect(gerstnerSet(sea(9), 32, 0.05)).toEqual(gerstnerSet(sea(9), 32, 0.05));
  });

  it("spans twice the peak's wavelength down to the cut", () => {
    const state = sea(10);
    const set = gerstnerSet(state, 8, 1);
    const wavelengths = set.components.map((c) => (2 * Math.PI) / c.wavenumberRadPerM);
    expect(Math.max(...wavelengths)).toBeLessThan(longestM(state));
    expect(Math.min(...wavelengths)).toBeGreaterThan(1);
  });
});

describe("the mean surface", () => {
  it("has zero Lagrangian mean height and an Eulerian mean of −QkA² ÷ 2 for one component", () => {
    const one: GerstnerSet = {
      components: [
        {
          wavenumberRadPerM: 0.1,
          amplitudeM: 2,
          angularFrequencyRadS: Math.sqrt(EARTH_SEA.gravityMps2 * 0.1),
          directionRad: 0.3,
          steepness: 0.8,
          phaseRad: 0.7,
        },
      ],
      amplitudeScale: 1,
      totalSlopeVariance: { upwind: 0.03, crosswind: 0.02 },
    };
    // One period along the component's direction, at the midpoints of 4,096 steps, exact for its
    // trigonometric polynomial.
    const steps = 4096;
    const along = (2 * Math.PI) / 0.1;
    let lagrangian = 0;
    let weighted = 0;
    let area = 0;
    for (let i = 0; i < steps; i++) {
      const s = ((i + 0.5) / steps) * along;
      const x0 = s * Math.cos(0.3);
      const y0 = s * Math.sin(0.3);
      const z = gerstnerDisplacement(one, x0, y0, 12)[2];
      const j = gerstnerJacobian(one, x0, y0, 12);
      lagrangian += z;
      weighted += z * j;
      area += j;
    }
    expect(Math.abs(lagrangian / steps)).toBeLessThan(1e-12);
    expect(weighted / area).toBeCloseTo(-(0.8 * 0.1 * 4) / 2, 12);
    expect(eulerianMeanHeightM(one)).toBeCloseTo(-0.16, 15);
  });

  it("keeps a whole set's Lagrangian mean at zero and its Eulerian mean within Σ kA²", () => {
    const set = gerstnerSet(sea(12), 32, 0.2);
    const bound = set.components.reduce(
      (acc, c) => acc + c.wavenumberRadPerM * c.amplitudeM * c.amplitudeM,
      0,
    );
    const side = 4096;
    const n = 400;
    let lagrangian = 0;
    let weighted = 0;
    let area = 0;
    let squares = 0;
    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        const x0 = ((i + 0.5) / n) * side;
        const y0 = ((j + 0.5) / n) * side;
        const z = gerstnerDisplacement(set, x0, y0, 0)[2];
        const jac = gerstnerJacobian(set, x0, y0, 0);
        lagrangian += z;
        squares += z * z;
        weighted += z * jac;
        area += jac;
      }
    }
    const count = n * n;
    // The grid's sum of incommensurate waves is a sample, its error about σ ÷ √N.
    const noise = 4 * Math.sqrt(squares / count / count);
    expect(Math.abs(lagrangian / count)).toBeLessThan(noise);
    expect(Math.abs(weighted / area)).toBeLessThan(bound + noise);
  });
});
