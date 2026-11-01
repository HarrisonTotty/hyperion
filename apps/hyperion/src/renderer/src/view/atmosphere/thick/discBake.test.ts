import { beforeAll, describe, expect, it } from "vitest";

import {
  bakedDiscAir,
  type BakedDiscAir,
  type DiscAirCase,
  fullPassTwins,
  type FullPassTwins,
  geometricAlbedoOf,
  groundGeometricAlbedo,
  type IntegratedRadiance,
  integratedRadianceAt,
} from "../../../test/discAirCases";
import { earthBody } from "../../../test/atmosphereFixtures";
import { bitsOf } from "../../../test/bits";
import { patternedDeckTables, patternedThickBake } from "../../../test/thickTables";
import { seededRandom } from "../../../test/seededRandom";
import {
  DISC_AZIMUTHS,
  DISC_SUN_COSINES,
  DISC_SUN_ROWS_BELOW,
  DISC_VIEW_COSINES,
  discAzimuthAt,
  discSunCosineAt,
  discTableAt,
  discViewCosineAt,
} from "../discTable";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES, EARTH_REFERENCE_SOURCES } from "../earth";
import { tableRadiusM } from "../hillaire";
import { type AtmosphereMedium, phaseAt } from "../medium";
import { kernelMedium, tablesPlanOf } from "../tables";
import { thickBakeBands, type ThickBakeBody } from "./bake";
import { deckGroundInto, deckReadScratch, packDeckTable } from "./deckRead";
import {
  decodedThickLayer,
  decodedReadScratch,
  decodedThickRead,
  discAirMarcher,
  discSunFloorOf,
  DISC_VIEW_STEPS,
} from "./discBake";
import {
  addThickLayer,
  packThickTable,
  thickRowAt,
  thickSourceScratch,
  THICK_TRAVEL_COSINES,
} from "./read";

const EARTH: DiscAirCase = {
  name: "the measured Earth",
  medium: EARTH_REFERENCE,
  sources: EARTH_REFERENCE_SOURCES,
  curves: EARTH_REFERENCE_CURVES,
  body: earthBody(),
};

/** Earth's one band as the bake takes it. */
function earthBakeBody(): ThickBakeBody {
  const body = earthBody();
  return { ...body, bottomRadiusM: tableRadiusM(body.figure) };
}

/** A thin pure-Rayleigh air of vertical optical depth `tau` in every channel over a black ground. */
function thinRayleigh(tau: number): AtmosphereMedium {
  const scaleHeightM = 8_000;
  const topHeightM = 100_000;
  const sigma = tau / (scaleHeightM * -Math.expm1(-topHeightM / scaleHeightM));
  return {
    name: "thin rayleigh",
    topHeightM,
    groundAlbedo: [0, 0, 0],
    terms: [
      {
        name: "rayleigh",
        density: { kind: "exponential", scaleHeightM },
        scattering: [sigma, sigma, sigma],
        absorption: [0, 0, 0],
        phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
      },
    ],
  };
}

describe("the disc's bake's read of J (R08.T16.b)", () => {
  it("reads a decoded layer as read.ts's addThickLayer reads the packed one, to 10⁻¹²", () => {
    const plan = tablesPlanOf(null, 6_371_000);
    const medium = thinRayleigh(0.1);
    const packed = packThickTable(patternedThickBake(plan.bands, medium, 2), medium);
    const random = seededRandom(7);
    for (const layer of [0, 1]) {
      const decoded = decodedThickLayer(packed, layer);
      const scratch = decodedReadScratch();
      for (let i = 0; i < 400; i += 1) {
        const heightM = random() * 100_000;
        // A third of the points below μ₀ = 1/64, the geometric step's.
        const muSun = i % 3 === 0 ? -0.22 + 0.24 * random() : random();
        const muTravel = 2 * random() - 1;
        const row = thickRowAt(packed, heightM);
        const want = thickSourceScratch();
        addThickLayer(packed, layer, row, muSun, muTravel, 1, want);
        const got = thickSourceScratch();
        decodedThickRead(decoded, row, muSun, muTravel, scratch, got);
        for (const c of [0, 1, 2] as const) {
          for (const key of ["level", "slope", "curve"] as const) {
            const w = want[key][c];
            expect(Math.abs(got[key][c] - w)).toBeLessThanOrEqual(1e-12 * Math.max(Math.abs(w), 1));
          }
        }
      }
    }
    expect(THICK_TRAVEL_COSINES).toBe(16);
  });
});

describe("the disc's bake's march (R08.T16.b)", () => {
  it("gives a thin Rayleigh air over a black ground its single scattering and mean transmittance, the plane's to within the sphere's 2%", () => {
    const tau = 0.002;
    const medium = thinRayleigh(tau);
    const plan = tablesPlanOf(null, 6_371_000);
    // J of 0 everywhere: the single scattering alone.
    const zero = patternedThickBake(plan.bands, medium, 1);
    for (const band of zero) {
      for (const slot of band.slots) {
        slot.values.fill(0);
      }
    }
    const march = discAirMarcher(
      {
        medium,
        curves: [],
        bands: plan.bands,
        source: packThickTable(zero, medium),
      },
      0,
    );
    for (const muView of [1, 0.6, 0.3]) {
      for (const muSun of [1, 0.5, 0.3]) {
        for (const azimuthRad of [0, 1.2, Math.PI]) {
          const [r, g, b, mean] = march(muView, muSun, azimuthRad, 0);
          // The ray's direction is towards the datum: cos θ = d · s = −v · s.
          const cosTheta = -(
            Math.sqrt((1 - muView * muView) * (1 - muSun * muSun)) * Math.cos(azimuthRad) +
            muView * muSun
          );
          const p = phaseAt(medium.terms[0]?.phase ?? { kind: "none" }, cosTheta)[0];
          // Plane-parallel single scattering, π p μ₀ ÷ (μ₀ + μ_v) (1 − e^(−τ (1 ÷ μ₀ + 1 ÷ μ_v))),
          // which the sphere's slant paths shorten by about (H ÷ R) tan² of the zenith angles.
          const expected =
            ((Math.PI * p * muSun) / (muSun + muView)) *
            -Math.expm1(-tau * (1 / muSun + 1 / muView));
          for (const value of [r, g, b]) {
            expect(Math.abs(value / expected - 1)).toBeLessThan(0.02);
          }
          // The sphere's slant column, a little under the plane's τ ÷ μ_v.
          expect(Math.abs(-Math.log(mean) / (tau / muView) - 1)).toBeLessThan(0.02);
        }
      }
    }
  });

  it("reads a deck at the datum in the bare ground's place, through the ray's transmittance, its alpha 0 (R08.T16.b.2)", () => {
    const tau = 0.05;
    const medium = thinRayleigh(tau);
    const plan = tablesPlanOf(null, 6_371_000);
    const source = packThickTable(patternedThickBake(plan.bands, medium, 1), medium);
    const deck = packDeckTable(patternedDeckTables(plan.bands, medium, 1, 1_000));
    const lnBands = Float64Array.of(0);
    // One step count on both sides, so that the deck's read alone sets them apart.
    const steps = { viewSteps: DISC_VIEW_STEPS };
    const bare = discAirMarcher({ medium, curves: [], bands: plan.bands, source }, 0, steps);
    const decked = discAirMarcher(
      { medium, curves: [], bands: plan.bands, source, deck: { table: deck, lnBands } },
      0,
      steps,
    );
    const scratch = deckReadScratch();
    const reflected: [number, number, number] = [0, 0, 0];
    let lit = 0;
    for (const muView of [1, 0.6, 0.3]) {
      for (const muSun of [0.9, 0.5, 0.1]) {
        for (const azimuthRad of [0, 1.2, Math.PI]) {
          const plain = bare(muView, muSun, azimuthRad, 0);
          const over = decked(muView, muSun, azimuthRad, 0);
          // The march's d = −v, v = (sin θ_v, 0, μ_v) towards the camera, at the datum's normal z.
          const sinView = Math.sqrt(1 - muView * muView);
          const sinSun = Math.sqrt(1 - muSun * muSun);
          const along = -(sinView * sinSun * Math.cos(azimuthRad) + muView * muSun);
          // The sun's transmittance does not enter: the deck's slot is the bake's own.
          deckGroundInto(
            deck,
            lnBands,
            1,
            muSun,
            muView,
            along + muView * muSun,
            along,
            0,
            [1, 1, 1],
            scratch,
            reflected,
          );
          for (const c of [0, 1, 2] as const) {
            // The bare ray's transmittance to the datum, its alpha, in every channel alike.
            const added = Math.PI * plain[3] * reflected[c];
            expect(over[c]).toBeCloseTo(plain[c] + added, 12);
            lit += added > 0 ? 1 : 0;
          }
          expect(over[3]).toBe(0);
        }
      }
    }
    expect(lit).toBeGreaterThan(0);
  });

  it("puts its floor at the deepest sun that lights the top of any band's air", () => {
    const bands = thickBakeBands(earthBakeBody());
    const floor = discSunFloorOf(bands, EARTH_REFERENCE.topHeightM);
    const r = bands[0]?.bottomRadiusM ?? Number.NaN;
    expect(floor).toBeCloseTo(-Math.sqrt(1 - (r / (r + 100_000)) ** 2), 12);
    expect(discSunCosineAt(0, floor)).toBe(floor);
    expect(discSunCosineAt(DISC_SUN_ROWS_BELOW, floor)).toBe(0);
    expect(discSunCosineAt(DISC_SUN_COSINES - 1, floor)).toBe(1);
  });
});

describe("the measured Earth's disc table (R08.T16.b)", () => {
  let baked: BakedDiscAir | undefined;
  let twins: FullPassTwins | undefined;
  beforeAll(() => {
    baked = bakedDiscAir(EARTH);
    twins = fullPassTwins(EARTH, baked);
  }, 240_000);
  const bakedOf = (): BakedDiscAir => {
    if (baked === undefined) {
      throw new Error("Earth's table is baked before the tests");
    }
    return baked;
  };
  const twinsOf = (): FullPassTwins => {
    if (twins === undefined) {
      throw new Error("Earth's twins are built before the tests");
    }
    return twins;
  };
  // Each phase's sums, taken once: the geometric albedo's test reads phase 0's again.
  const sums = new Map<number, IntegratedRadiance>();
  const sumsAt = (phaseDeg: number): IntegratedRadiance => {
    const known = sums.get(phaseDeg);
    if (known !== undefined) {
      return known;
    }
    const taken = integratedRadianceAt(EARTH, bakedOf(), twinsOf(), phaseDeg);
    sums.set(phaseDeg, taken);
    return taken;
  };

  it("bakes the bits it baked before the march's read took its sums in place (R08.T16.b.2's lever)", async () => {
    // Its rows below the horizontal take the read's geometric step, those above its linear one.
    expect(await bitsOf(bakedOf().table)).toBe(
      "ec3ff49c1c2b22a1c88f2033c0090828d0e2b97e28dbe487995b950e57840a15",
    );
  });

  it("has one layer, a body of one band, which reads as before at every latitude", () => {
    const { table } = bakedOf();
    expect([table.bands, table.slots]).toEqual([1, 1]);
    for (const [muView, muSun, azimuth] of [
      [0.7, 0.4, 0.3],
      [0.05, -0.02, 2.5],
    ] as const) {
      const at = discTableAt(table, [0], 1, 0, muView, muSun, azimuth);
      for (const s of [0.997, 1.003]) {
        expect(discTableAt(table, [0], s, 0, muView, muSun, azimuth)).toEqual(at);
      }
      // A light slot past the table's reads its last.
      expect(discTableAt(table, [0], 1, 1, muView, muSun, azimuth)).toEqual(at);
    }
  });

  it("reads between its nodes within 1.5% rms of the march by day and 7% at worst (μ₀ ≥ 0.05)", () => {
    const { table } = bakedOf();
    const march = discAirMarcher(
      {
        medium: kernelMedium(EARTH.medium),
        curves: EARTH.curves,
        bands: thickBakeBands(earthBakeBody()),
        source: bakedOf().source,
      },
      0,
    );
    let peak = 0;
    for (let i = 0; i < table.values.length; i += 4) {
      peak = Math.max(peak, table.values[i + 2] ?? 0);
    }
    let sumSq = 0;
    let n = 0;
    let worst = 0;
    for (let s = 0; s < DISC_SUN_COSINES - 1; s += 1) {
      const muSun =
        0.5 * (discSunCosineAt(s, table.sunFloor) + discSunCosineAt(s + 1, table.sunFloor));
      if (muSun < 0.05) {
        continue;
      }
      for (let v = 0; v < DISC_VIEW_COSINES - 1; v += 3) {
        const muView = 0.5 * (discViewCosineAt(v) + discViewCosineAt(v + 1));
        for (let k = 0; k < DISC_AZIMUTHS - 1; k += 2) {
          const azimuth = 0.5 * (discAzimuthAt(k) + discAzimuthAt(k + 1));
          const read = discTableAt(table, [0], 1, 0, muView, muSun, azimuth);
          const want = march(muView, muSun, azimuth, 0);
          for (const c of [0, 1, 2] as const) {
            const e = Math.abs(read[c] - want[c]) / Math.max(want[c], 0.02 * peak);
            sumSq += e * e;
            n += 1;
            worst = Math.max(worst, e);
          }
        }
      }
    }
    expect(Math.sqrt(sumSq / n)).toBeLessThan(0.015);
    expect(worst).toBeLessThan(0.07);
  }, 60_000);

  for (const phaseDeg of [0, 90]) {
    it(`agrees with the full passes' integrated radiance to 5% from 10⁹ m at a phase of ${phaseDeg}°, routed and as drawn today`, () => {
      const at = sumsAt(phaseDeg);
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(at.disc[c] / at.routed[c] - 1)).toBeLessThan(0.05);
        expect(Math.abs(at.disc[c] / at.unrouted[c] - 1)).toBeLessThan(0.05);
        // The air beyond the datum's limb, which the disc does not draw, is under 3% of the light.
        expect(at.limb[c] / at.routed[c]).toBeLessThan(0.03);
      }
    }, 120_000);
  }

  it("has a geometric albedo above its ground's (R07's `BodyPhotometry`), the air adding blue", () => {
    const at = sumsAt(0);
    const p = geometricAlbedoOf(at.disc, at.solidAngleSr);
    const ground = groundGeometricAlbedo(EARTH);
    for (const c of [0, 1, 2] as const) {
      expect(p[c]).toBeGreaterThan(ground[c]);
    }
    expect(p[2]).toBeGreaterThan(p[0]);
  }, 120_000);
});
