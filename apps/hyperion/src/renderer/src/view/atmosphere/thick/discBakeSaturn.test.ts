import { beforeAll, describe, expect, it } from "vitest";

import {
  bakedDiscAir,
  type BakedDiscAir,
  type DiscAirCase,
  fullPassTwins,
  type FullPassTwins,
  integratedRadianceAt,
} from "../../../test/discAirCases";
import { discTableAt, discTexelIndex } from "../discTable";
import { tableRadiusM } from "../hillaire";
import {
  SATURN_OBLATE_GRAVITY,
  SATURN_OBLATE_REFERENCE,
  SATURN_OBLATE_REFERENCE_SOURCES,
  SATURN_OBLATE_SPHEROID,
} from "../reference/cases";
import { tablesPlanOf } from "../tables";
import { log } from "../../../lib/pinnedMath";

const SATURN: DiscAirCase = {
  name: "the Saturn-class figure",
  medium: SATURN_OBLATE_REFERENCE,
  sources: SATURN_OBLATE_REFERENCE_SOURCES,
  curves: [],
  body: { figure: SATURN_OBLATE_SPHEROID, gravity: SATURN_OBLATE_GRAVITY },
};

describe("the Saturn-class figure's disc table (R08.T16.b)", () => {
  let baked: BakedDiscAir | undefined;
  let twins: FullPassTwins | undefined;
  beforeAll(() => {
    baked = bakedDiscAir(SATURN);
    twins = fullPassTwins(SATURN, baked);
  }, 300_000);
  const bakedOf = (): BakedDiscAir => {
    if (baked === undefined) {
      throw new Error("the Saturn-class table is baked before the tests");
    }
    return baked;
  };
  const twinsOf = (): FullPassTwins => {
    if (twins === undefined) {
      throw new Error("the Saturn-class twins are built before the tests");
    }
    return twins;
  };

  it("holds a layer a band, read at a band's own s as that layer and between them in ln s", () => {
    const { table } = bakedOf();
    const plan = tablesPlanOf(SATURN.body, tableRadiusM(SATURN_OBLATE_SPHEROID));
    expect([table.bands, table.slots]).toEqual([4, 1]);
    const lnBands = plan.bands.map((band) => log(band.gravityRatio));
    // At a node, a band's own s reads that layer's texel.
    for (const [b, band] of plan.bands.entries()) {
      const at = discTableAt(table, lnBands, band.gravityRatio, 0, 1, 1, 0);
      const texel = discTexelIndex(b, 31, 15, 0);
      for (const c of [0, 1, 2, 3] as const) {
        expect(at[c]).toBeCloseTo(table.values[texel + c] ?? Number.NaN, 6);
      }
    }
    // Halfway between two bands in ln s, the layers' mean.
    const [first, second] = plan.bands;
    if (first === undefined || second === undefined) {
      throw new Error("the figure has bands");
    }
    const s = Math.sqrt(first.gravityRatio * second.gravityRatio);
    const mid = discTableAt(table, lnBands, s, 0, 0.5, 0.5, 1);
    const one = discTableAt(table, lnBands, first.gravityRatio, 0, 0.5, 0.5, 1);
    const two = discTableAt(table, lnBands, second.gravityRatio, 0, 0.5, 0.5, 1);
    for (const c of [0, 1, 2, 3] as const) {
      expect(mid[c]).toBeCloseTo(0.5 * (one[c] + two[c]), 10);
    }
  });

  for (const phaseDeg of [0, 90]) {
    it(`agrees with the full passes' integrated radiance to 5% from 10⁹ m at a phase of ${phaseDeg}°, routed; Ψ_ms's passes as measured`, () => {
      const sums = integratedRadianceAt(SATURN, bakedOf(), twinsOf(), phaseDeg);
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(sums.disc[c] / sums.routed[c] - 1)).toBeLessThan(0.05);
        expect(sums.limb[c] / sums.routed[c]).toBeLessThan(0.03);
        // Against the passes as drawn today, Ψ_ms the multiple scattering: pinned as measured
        // (+1.3% to +6.8%, blue at 90° the most), the analytic term's own error, which R08.T14.d's
        // routing removes (plan R08's record of T16.b.1).
        const unrouted = sums.disc[c] / sums.unrouted[c] - 1;
        expect(unrouted).toBeGreaterThan(0);
        expect(unrouted).toBeLessThan(0.08);
      }
    }, 120_000);
  }
});
