/**
 * Shared fixtures of R11's clouds (plan R11, R11.T7): an Earth-like body's clouds, a clock that
 * steps their making, and a layer of one coverage everywhere as the ground's CPU tap reads it.
 */
import { cloudFieldParams, cloudSeedOf, coverageTexels } from "../view/clouds/cloudDensity";
import { cloudShellOf } from "../view/clouds/cloudLayer";
import { cloudNoiseTexels } from "../view/clouds/cloudNoise";
import type { CloudLightInput } from "../view/clouds/cloudShadow";
import { type BodyClouds, STAND_IN_CONDENSATES } from "../view/clouds/condensates";

/** An Earth-like body's clouds: a liquid water deck from 1 km to 3 km under 67% cover. */
export const EARTH_CLOUDS: BodyClouds = {
  cloudFraction: 0.67,
  surfacePressurePa: 101_325,
  decks: [
    {
      condensate: "H2O",
      phase: "liquid",
      inCloudOpticalDepth550: 8,
      coverage: 0.67,
      baseAltitudeM: 1_000,
      topAltitudeM: 3_000,
      optics: null,
    },
  ],
};

/**
 * A clock that moves a second each time it is read, so that a frame makes one slice of the clouds'
 * noise tile, or one step of the volume's tables, whatever the machine.
 */
export function steppingClock(): () => number {
  let nowMs = 0;
  return () => {
    nowMs += 1_000;
    return nowMs;
  };
}

/**
 * A body's clouds of one coverage `fraction` everywhere, at 1.5 km over a datum of radii
 * `equatorialRadiusM` and `polarRadiusM`, liquid water's stand-in optics, τ̄ 8, as the ground's
 * CPU tap reads them (`cloudShadow.ts`' `cloudLightAt`).
 */
export function uniformClouds(
  fraction: number,
  equatorialRadiusM: number,
  polarRadiusM: number,
): CloudLightInput {
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
      radiusM: equatorialRadiusM,
      time: { seconds: 5_000, nanos: 0 },
      rate: 1,
      windMps: 20,
      octaves: 12,
    }),
    shell: cloudShellOf({ equatorialRadiusM, polarRadiusM }, 1_500),
    optics: STAND_IN_CONDENSATES.liquidWater,
    datumRadiusM: equatorialRadiusM,
  };
}
