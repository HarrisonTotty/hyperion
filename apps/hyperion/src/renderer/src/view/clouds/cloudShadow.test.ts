import { describe, expect, it } from "vitest";

import { cloudFieldAt, cloudFieldParams, cloudSeedOf, coverageTexels } from "./cloudDensity";
import { cloudShellOf } from "./cloudLayer";
import { cloudNoiseTexels } from "./cloudNoise";
import { deltaEddington } from "./cloudOptics";
import { CLEAR_GROUND, type CloudLightInput, cloudGroundLight, cloudLightAt } from "./cloudShadow";
import { STAND_IN_CONDENSATES } from "./condensates";

const WATER = STAND_IN_CONDENSATES.liquidWater;
const A = 6_378_137;
const C = 6_356_752.3;

/**
 * A body's clouds of one coverage everywhere, at 1.5 km, liquid water's optics, τ̄ 8, at a time
 * rate (above about 10⁶ every octave fades to the mean).
 */
function overcast(fraction: number, rate = 1): CloudLightInput {
  const widthCells = 32;
  const heightCells = 16;
  return {
    coverage: coverageTexels(
      {
        widthCells,
        heightCells,
        fractions: new Float64Array(widthCells * heightCells).fill(fraction),
        offset: 0,
      },
      () => ({ eastMps: 0, northMps: 0 }),
    ),
    noise: cloudNoiseTexels(),
    params: cloudFieldParams({
      seed: cloudSeedOf("0200080020000000.0103"),
      condensate: { inCloudOpticalDepth550: 8 },
      radiusM: A,
      time: { seconds: 5_000, nanos: 0 },
      rate,
      windMps: 20,
      octaves: 12,
    }),
    shell: cloudShellOf({ equatorialRadiusM: A, polarRadiusM: C }, 1_500),
    optics: WATER,
    datumRadiusM: A,
  };
}

describe("cloudGroundLight", () => {
  it("under a τ = 10 cover and a zenith sun lets through under 10⁻⁴ of the beam and T_total of the light", () => {
    const light = cloudGroundLight(1, 10, WATER, 1);
    expect(light.direct).toBeLessThan(1e-4);
    const slab = deltaEddington({
      tau: 10,
      asymmetry: WATER.asymmetry,
      singleScatterAlbedo: 1 - WATER.coAlbedo,
      mu0: 1,
    });
    const total = slab.directTransmittance + slab.diffuseTransmittance;
    // A clear ground's beam on a horizontal surface and its sky, in any one unit.
    for (const [beam, sky] of [
      [1, 0.15],
      [0.6, 0.4],
    ] as const) {
      const ground = beam * light.direct + sky * light.sky + beam * light.diffuse;
      expect(Math.abs(ground / (total * (beam + sky)) - 1)).toBeLessThan(0.02);
    }
  });

  it("keeps every part under no cover", () => {
    expect(cloudGroundLight(0, 10, WATER, 0.5)).toBe(CLEAR_GROUND);
  });

  it("blends the clear and the covered shares", () => {
    const covered = cloudGroundLight(1, 3, WATER, 0.7);
    const half = cloudGroundLight(0.5, 3, WATER, 0.7);
    expect(half.direct).toBeCloseTo(0.5 + 0.5 * covered.direct, 12);
    expect(half.diffuse).toBeCloseTo(0.5 * covered.diffuse, 12);
  });
});

describe("cloudLightAt", () => {
  it("keeps every part above the clouds' shell", () => {
    const place = { x: (A + 2_000) / A, y: 0, z: 0 };
    expect(cloudLightAt(overcast(0.5), place, { x: 1, y: 0, z: 0 }, 4.65e-3)).toBe(CLEAR_GROUND);
  });

  it("takes a time-warped overcast's mean τ̄ at the sun ray's crossing of the shell", () => {
    const clouds = overcast(1, 1e9);
    const place = { x: 1, y: 0, z: 0 };
    const zenith = cloudLightAt(clouds, place, { x: 1, y: 0, z: 0 }, 4.65e-3);
    expect(zenith.direct).toBeCloseTo(Math.exp(-8), 8);
    // At 60° the ray crosses the shell where its normal is turned by the arc, under 0.02°.
    const oblique = cloudLightAt(clouds, place, { x: 0.5, y: Math.sqrt(0.75), z: 0 }, 4.65e-3);
    expect(oblique.direct).toBeCloseTo(Math.exp(-8 / 0.5), 8);
  });

  it("reads the field over the source's disc seen across the crossing's distance", () => {
    const clouds = overcast(0.5);
    const crossing = { x: A + 1_500, y: 0, z: 0 };
    for (const [radiusRad, footprintM] of [
      [1e-6, 1],
      [0.5, 1_500],
    ] as const) {
      const field = cloudFieldAt(
        clouds.coverage,
        clouds.noise,
        clouds.params,
        crossing,
        footprintM,
      );
      const want = cloudGroundLight(field.fraction, field.inCloudTau, WATER, 1);
      const light = cloudLightAt(clouds, { x: 1, y: 0, z: 0 }, { x: 1, y: 0, z: 0 }, radiusRad);
      expect(light.direct).toBeCloseTo(want.direct, 9);
      expect(light.sky).toBeCloseTo(want.sky, 9);
      expect(light.diffuse).toBeCloseTo(want.diffuse, 9);
    }
  });
});
