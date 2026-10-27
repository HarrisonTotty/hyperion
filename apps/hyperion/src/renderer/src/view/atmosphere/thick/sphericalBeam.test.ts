import { describe, expect, it } from "vitest";

import { planeParallelBeam } from "./discreteOrdinates";
import { SLANT_FACTOR_FLOOR, sphericalBeamPath } from "./sphericalBeam";

/** A column of `count` layers, its interfaces from the top down, evenly over `topM`. */
function interfaces(topM: number, count: number): Float64Array {
  return Float64Array.from({ length: count + 1 }, (_, k) => topM * (1 - k / count));
}

/** An exponential air's layers' optical depths: a scale height of 8 km, `total` in all. */
function exponentialDepths(heightsM: Float64Array, total: number): Float64Array {
  const scaleM = 8_000;
  const above = (h: number): number => Math.exp(-h / scaleM);
  const whole = above(0) - above(heightsM[0] ?? Number.NaN);
  return Float64Array.from({ length: heightsM.length - 1 }, (_, p) => {
    const top = heightsM[p] ?? Number.NaN;
    const bottom = heightsM[p + 1] ?? Number.NaN;
    return (total * (above(bottom) - above(top))) / whole;
  });
}

/** The cosines the beam is held at, the grazing ones included. */
const COSINES = [1 / 64, 0.05, 0.2, 0.5, 0.9, 1];

describe("the pseudo-spherical beam", () => {
  const heights = interfaces(100_000, 64);
  const depths = exponentialDepths(heights, 2);

  it("is the plane-parallel beam on a sphere a billion times the column's height, its slant depths to 10⁻⁶", () => {
    const radiusM = 1e14;
    for (const mu0 of COSINES) {
      const spherical = sphericalBeamPath(radiusM, heights, depths, mu0);
      const flat = planeParallelBeam(
        Array.from(depths, (opticalDepth) => ({
          opticalDepth,
          singleScatteringAlbedo: 0,
          moments: [1],
        })),
        mu0,
      );
      for (let p = 0; p < depths.length; p += 1) {
        const lambda = spherical.slantFactor[p] ?? Number.NaN;
        const lambdaFlat = flat.slantFactor[p] ?? Number.NaN;
        expect(Math.abs(lambda - lambdaFlat) / lambdaFlat).toBeLessThan(1e-6);
        // In slant optical depth, relative: e^(−τ) moves by τ times τ's relative error.
        const slant = -Math.log(spherical.topTransmission[p] ?? Number.NaN);
        const slantFlat = -Math.log(flat.topTransmission[p] ?? Number.NaN);
        expect(Math.abs(slant - slantFlat)).toBeLessThanOrEqual(1e-6 * slantFlat);
      }
    }
  });

  it("keeps T_{p+1} = T_p e^(−λ_p τ_p) at every layer and at the ground, to 10⁻¹²", () => {
    for (const radiusM of [2_574_730, 6_051_800, 60_268_000]) {
      for (const mu0 of COSINES) {
        const beam = sphericalBeamPath(radiusM, heights, depths, mu0);
        for (let p = 0; p < depths.length; p += 1) {
          const top = beam.interfaceTransmission[p] ?? Number.NaN;
          const next = beam.interfaceTransmission[p + 1] ?? Number.NaN;
          expect(beam.topTransmission[p]).toBe(top);
          const carried =
            top * Math.exp(-(beam.slantFactor[p] ?? Number.NaN) * (depths[p] ?? Number.NaN));
          expect(Math.abs(carried - next)).toBeLessThanOrEqual(1e-12 * next);
        }
      }
    }
  });

  it("is the spherical beam e^(−τ_s) at every interface where no floor is taken", () => {
    for (const mu0 of COSINES) {
      const beam = sphericalBeamPath(6_051_800, heights, depths, mu0);
      expect(beam.flooredLayers).toBe(0);
      for (let k = 0; k < heights.length; k += 1) {
        const exact = Math.exp(-(beam.slantOpticalDepth[k] ?? Number.NaN));
        const t = beam.interfaceTransmission[k] ?? Number.NaN;
        expect(Math.abs(t - exact)).toBeLessThanOrEqual(1e-12 * exact);
      }
    }
  });

  it("takes a vertical beam through every layer at a slant of 1", () => {
    const beam = sphericalBeamPath(6_051_800, heights, depths, 1);
    for (const lambda of Array.from(beam.slantFactor)) {
      expect(Math.abs(lambda - 1)).toBeLessThan(1e-12);
    }
  });

  it("floors the slant of a clear layer under a thick one at a grazing sun, and catches the beam up below it", () => {
    // A thick layer over a nearly clear one: the deeper point's ray crosses the thick layer more
    // steeply, so the spherical slant depth falls downward there.
    const column = Float64Array.of(80_000, 60_000, 59_000, 58_000, 0);
    const layers = Float64Array.of(5, 1e-6, 1e-6, 50);
    const beam = sphericalBeamPath(6_051_800, column, layers, 0.05);
    expect(beam.flooredLayers).toBeGreaterThan(0);
    expect(Math.min(...Array.from(beam.slantFactor))).toBe(SLANT_FACTOR_FLOOR);
    expect(beam.largestDeficit).toBeGreaterThan(0);
    const last = column.length - 1;
    const exact = Math.exp(-(beam.slantOpticalDepth[last] ?? Number.NaN));
    expect(Math.abs((beam.interfaceTransmission[last] ?? Number.NaN) - exact)).toBeLessThanOrEqual(
      1e-12 * exact,
    );
  });

  it("refuses a cosine outside (0, 1], interfaces that do not descend, and depths of another count", () => {
    expect(() => sphericalBeamPath(6e6, heights, depths, 0)).toThrow(RangeError);
    expect(() => sphericalBeamPath(6e6, heights, depths, 1.01)).toThrow(RangeError);
    expect(() => sphericalBeamPath(6e6, Float64Array.of(0, 1000), Float64Array.of(1), 0.5)).toThrow(
      RangeError,
    );
    expect(() => sphericalBeamPath(6e6, heights, depths.subarray(1), 0.5)).toThrow(RangeError);
    expect(() => sphericalBeamPath(-1, heights, depths, 0.5)).toThrow(RangeError);
  });
});
