/**
 * The switch between the clouds' two renderings (plan R11, R11.T7.d): the low setting's layer at
 * its octaves and the high setting's volume at its own read one coverage and one density field, so
 * that switching the setting, or the style, moves neither field and keeps a coverage cell's mean
 * brightness within T7.c's tolerance.
 */
import { describe, expect, it } from "vitest";

import { cloudFieldAt, cloudFieldParams, cloudSeedOf } from "./cloudDensity";
import { LAYER_OCTAVES, seasonCoverage } from "./cloudLayer";
import { cloudNoiseTexels } from "./cloudNoise";
import { deltaEddington } from "./cloudOptics";
import {
  correctedColumn,
  fluxReflectance,
  marchColumn,
  octaveCorrectionAt,
  octaveCorrectionOf,
} from "./cloudVolume";
import { STAND_IN_CONDENSATES } from "./condensates";

const WATER = STAND_IN_CONDENSATES.liquidWater;
const RADIUS_M = 6_378_137;

/** One body's field parameters at a time on a setting's octaves. */
function paramsOn(octaves: number) {
  return cloudFieldParams({
    seed: cloudSeedOf("0200080020000000.0103"),
    condensate: { inCloudOpticalDepth550: 8 },
    radiusM: RADIUS_M,
    time: { seconds: 77_777, nanos: 0 },
    rate: 1,
    windMps: 20,
    octaves,
  });
}

/** The provisional climatology's coverage at an equinox under plan 14's 67%. */
const COVERAGE = seasonCoverage({ subsolarLatitudeRad: 0 }, 0.67);

/** Places on the layer over a 1.4° cell about `latDeg` N, 30° E, `n` × `n` of them. */
function cellPlaces(latDeg: number, n: number) {
  const places = [];
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j < n; j += 1) {
      const lat = ((latDeg + (1.4 * (i + 0.5)) / n) * Math.PI) / 180;
      const lon = ((30 + (1.4 * (j + 0.5)) / n) * Math.PI) / 180;
      places.push({
        x: RADIUS_M * Math.cos(lat) * Math.cos(lon),
        y: RADIUS_M * Math.cos(lat) * Math.sin(lon),
        z: RADIUS_M * Math.sin(lat),
      });
    }
  }
  return places;
}

describe("the clouds' switch of setting (R11.T7.d)", () => {
  const noise = cloudNoiseTexels();
  const low = paramsOn(LAYER_OCTAVES.low);
  const high = paramsOn(LAYER_OCTAVES.high);

  it("keeps the coverage and every octave both settings draw, bit for bit", () => {
    // The coverage reads no setting at all.
    expect(seasonCoverage({ subsolarLatitudeRad: 0 }, 0.67).texels).toEqual(COVERAGE.texels);
    expect(high.rotation).toEqual(low.rotation);
    expect(high.advectionS).toEqual(low.advectionS);
    expect(high.phaseWeights).toEqual(low.phaseWeights);
    expect(high.offsets.slice(0, low.offsets.length)).toEqual(low.offsets);
    expect(high.octaveFades.slice(0, low.octaveFades.length)).toEqual(low.octaveFades);
  });

  // The resolved octaves are the same; only the norm over the octaves drawn, √(Σ ρ²ᵏ), differs, by
  // 0.06% between 9 and 12, which moves the cover and τ by under 0.5%.
  it("keeps the field's cover and mean τ at a footprint the low setting resolves, to 0.5%", () => {
    let lowShare = 0;
    let highShare = 0;
    let lowTau = 0;
    let highTau = 0;
    for (const latDeg of [-55, -25, 0, 25, 55]) {
      for (const place of cellPlaces(latDeg, 8)) {
        const a = cloudFieldAt(COVERAGE, noise, low, place, 8_000);
        const b = cloudFieldAt(COVERAGE, noise, high, place, 8_000);
        lowShare += a.fraction;
        highShare += b.fraction;
        lowTau += a.meanTau;
        highTau += b.meanTau;
      }
    }
    expect(Math.abs(highShare / lowShare - 1)).toBeLessThan(5e-3);
    expect(Math.abs(highTau / lowTau - 1)).toBeLessThan(5e-3);
  });

  // The layer draws f δE(τ_in) a texel (`cloudLayer.wgsl`); the volume marches the covered part
  // alone and weighs it by f (`cloudVolumeMarch.wgsl`): each the independent-pixel form, at a fine
  // footprint as at an orbit's.
  it.each([
    ["a fine footprint", 2_000],
    ["an orbit's", 50_000],
  ])(
    "keeps a coverage cell's mean brightness at %s, the low setting's layer against the high setting's march, to 2%",
    (_, footprintM) => {
      const correction = octaveCorrectionOf(WATER);
      const mu0 = 0.8;
      for (const latDeg of [0, 50]) {
        let byLayer = 0;
        let byVolume = 0;
        for (const place of cellPlaces(latDeg, 12)) {
          const layer = cloudFieldAt(COVERAGE, noise, low, place, footprintM);
          byLayer +=
            layer.fraction *
            deltaEddington({
              tau: layer.inCloudTau,
              asymmetry: WATER.asymmetry,
              singleScatterAlbedo: 1 - WATER.coAlbedo,
              mu0,
            }).reflectance;
          const volume = cloudFieldAt(COVERAGE, noise, high, place, footprintM);
          const k = octaveCorrectionAt(correction, volume.inCloudTau, mu0);
          byVolume +=
            volume.fraction *
            fluxReflectance(mu0, (muView, cosTheta) =>
              correctedColumn(
                marchColumn({
                  tau: volume.inCloudTau,
                  mu0,
                  muView,
                  cosTheta,
                  optics: WATER,
                  steps: 32,
                }),
                k,
              ),
            );
        }
        expect(byLayer).toBeGreaterThan(0);
        expect(Math.abs(byVolume / byLayer - 1)).toBeLessThan(0.02);
      }
    },
  );
});
