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

/** The cosines below the horizontal it is held at: twilight's, to past the table's top's shadow. */
const BELOW = [-1 / 64, -0.05, -0.1, -0.15, -0.17];

/**
 * τ_s from interface `k` by the ray's own intersections with every interface's sphere, independently
 * of the module's chords: the textbook roots s = −r_k μ₀ ± √(r_k² μ₀² − r_k² + r²) along the ray
 * towards the sun, sorted, each segment's shell found at its middle; +∞ where the ray meets the
 * ground.
 */
function slantByIntersections(
  radiusM: number,
  heightsM: Float64Array,
  depths: Float64Array,
  k: number,
  mu0: number,
): number {
  const rk = radiusM + (heightsM[k] ?? Number.NaN);
  const roots: number[] = [0];
  for (const h of heightsM) {
    const r = radiusM + h;
    const discriminant = rk * rk * mu0 * mu0 - rk * rk + r * r;
    if (discriminant >= 0) {
      for (const s of [-rk * mu0 - Math.sqrt(discriminant), -rk * mu0 + Math.sqrt(discriminant)]) {
        if (s > 0) {
          roots.push(s);
        }
      }
    }
  }
  const ground = rk * rk * mu0 * mu0 - rk * rk + radiusM * radiusM;
  if (mu0 < 0 && ground >= 0) {
    return Number.POSITIVE_INFINITY;
  }
  roots.sort((a, b) => a - b);
  const top = heightsM[0] ?? Number.NaN;
  let tau = 0;
  for (let i = 0; i + 1 < roots.length; i += 1) {
    const from = roots[i] ?? Number.NaN;
    const to = roots[i + 1] ?? Number.NaN;
    const middle = (from + to) / 2;
    const x = rk + middle * mu0;
    const y = middle * Math.sqrt(1 - mu0 * mu0);
    const h = Math.hypot(x, y) - radiusM;
    if (h > top) {
      continue;
    }
    for (let p = 0; p < depths.length; p += 1) {
      const upper = heightsM[p] ?? Number.NaN;
      const lower = heightsM[p + 1] ?? Number.NaN;
      if (h <= upper && h >= lower) {
        tau += ((depths[p] ?? Number.NaN) * (to - from)) / (upper - lower);
        break;
      }
    }
  }
  return tau;
}

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

  it("refuses a cosine outside [−1, 1], interfaces that do not descend, and depths of another count", () => {
    expect(() => sphericalBeamPath(6e6, heights, depths, -1.01)).toThrow(RangeError);
    expect(() => sphericalBeamPath(6e6, heights, depths, Number.NaN)).toThrow(RangeError);
    expect(() => sphericalBeamPath(6e6, heights, depths, 1.01)).toThrow(RangeError);
    expect(() => sphericalBeamPath(6e6, Float64Array.of(0, 1000), Float64Array.of(1), 0.5)).toThrow(
      RangeError,
    );
    expect(() => sphericalBeamPath(6e6, heights, depths.subarray(1), 0.5)).toThrow(RangeError);
    expect(() => sphericalBeamPath(-1, heights, depths, 0.5)).toThrow(RangeError);
  });
});

describe("the pseudo-spherical beam below the horizontal (R08.T14.d.2)", () => {
  const radiusM = 6_371_000;
  const heights = interfaces(100_000, 64);
  const depths = exponentialDepths(heights, 2);
  const shadowed = (k: number, mu0: number): boolean => {
    const r = radiusM + (heights[k] ?? Number.NaN);
    return r * r * (1 - mu0 * mu0) <= radiusM * radiusM;
  };

  it("takes the ray through its tangent point, τ_s as the ray's own intersections give it, to 10⁻⁹", () => {
    let worst = 0;
    let darkMisses = 0;
    for (const mu0 of [...BELOW, ...COSINES]) {
      const beam = sphericalBeamPath(radiusM, heights, depths, mu0);
      for (let k = 0; k < heights.length; k += 1) {
        const exact = slantByIntersections(radiusM, heights, depths, k, mu0);
        const slant = beam.slantOpticalDepth[k] ?? Number.NaN;
        const dark = exact === Number.POSITIVE_INFINITY;
        darkMisses += dark === (slant === Number.POSITIVE_INFINITY) ? 0 : 1;
        worst = dark ? worst : Math.max(worst, Math.abs(slant - exact) / (exact + 1e-15));
      }
    }
    expect(darkMisses).toBe(0);
    expect(worst).toBeLessThanOrEqual(1e-9);
  });

  it("dims the top below 1 and is e^(−τ_s) there, the ray crossing the column under it twice", () => {
    for (const mu0 of BELOW) {
      const beam = sphericalBeamPath(radiusM, heights, depths, mu0);
      const top = beam.interfaceTransmission[0] ?? Number.NaN;
      expect(top).toBeLessThan(1);
      expect(beam.topTransmission[0]).toBe(top);
      expect(Math.abs(top - Math.exp(-(beam.slantOpticalDepth[0] ?? Number.NaN)))).toBeLessThan(
        1e-15,
      );
    }
  });

  it("puts every interface whose ray meets the ground in the shadow, T 0, as the twin's ground shadow", () => {
    for (const mu0 of BELOW) {
      const beam = sphericalBeamPath(radiusM, heights, depths, mu0);
      const rows = Array.from({ length: heights.length }, (_, k) => ({
        shadowed: shadowed(k, mu0),
        t: beam.interfaceTransmission[k] ?? Number.NaN,
        slant: beam.slantOpticalDepth[k] ?? Number.NaN,
      }));
      const dark = rows.filter((row) => row.shadowed);
      const lit = rows.filter((row) => !row.shadowed);
      expect(dark.every((row) => row.t === 0 && row.slant === Number.POSITIVE_INFINITY)).toBe(true);
      expect(lit.every((row) => row.t > 0)).toBe(true);
      // The ground always lies in the shadow of a sun below its horizontal.
      expect(rows.at(-1)?.shadowed).toBe(true);
    }
  });

  it("gives no layer of the shadow a beam", () => {
    for (const mu0 of BELOW) {
      const beam = sphericalBeamPath(radiusM, heights, depths, mu0);
      const dark = Array.from({ length: depths.length }, (_, p) => p).filter((p) =>
        shadowed(p, mu0),
      );
      expect(dark.map((p) => [beam.topTransmission[p], beam.slantFactor[p]])).toEqual(
        dark.map(() => [0, 1]),
      );
    }
  });

  it("puts the column's top in the shadow just past −√(1 − (R ÷ (R + h_top))²), and no further up", () => {
    const limit = -Math.sqrt(1 - (radiusM / (radiusM + 100_000)) ** 2);
    const past = sphericalBeamPath(radiusM, heights, depths, limit - 1e-6);
    const before = sphericalBeamPath(radiusM, heights, depths, limit + 1e-6);
    expect([
      Math.max(...Array.from(past.interfaceTransmission)),
      (before.interfaceTransmission[0] ?? Number.NaN) > 0,
    ]).toEqual([0, true]);
  });

  it("is e^(−τ_s) at every lit interface where no floor is taken, to 10⁻¹²", () => {
    let worst = 0;
    let floored = 0;
    for (const mu0 of BELOW) {
      const beam = sphericalBeamPath(radiusM, heights, depths, mu0);
      floored += beam.flooredLayers;
      for (let k = 0; k < heights.length && !shadowed(k, mu0); k += 1) {
        const exact = Math.exp(-(beam.slantOpticalDepth[k] ?? Number.NaN));
        const t = beam.interfaceTransmission[k] ?? Number.NaN;
        worst = Math.max(worst, Math.abs(t - exact) / exact);
      }
    }
    expect([floored, worst <= 1e-12]).toEqual([0, true]);
  });

  it("floors the slant of a clear layer under a thick one below the horizontal, and catches the beam up below it", () => {
    // Under a thick layer a deeper point's ray, past its dip, crosses the thick layer more steeply.
    const column = Float64Array.of(80_000, 60_000, 59_000, 58_000, 0);
    const layers = Float64Array.of(5, 1e-6, 1e-6, 50);
    const beam = sphericalBeamPath(6_051_800, column, layers, -0.01);
    const exact = Math.exp(-(beam.slantOpticalDepth[3] ?? Number.NaN));
    expect([
      beam.flooredLayers > 0,
      Math.min(...Array.from(beam.slantFactor)),
      beam.largestDeficit > 0,
      Math.abs((beam.interfaceTransmission[3] ?? Number.NaN) - exact) <= 1e-12 * exact,
    ]).toEqual([true, SLANT_FACTOR_FLOOR, true, true]);
  });

  it("gives a clear layer below the horizontal its chord through the layer over its thickness", () => {
    // 50 to 60 km at μ₀ −0.05: the ray from the base dips, and climbs back through it 2 r_b |μ₀| on.
    const r = radiusM + 50_000;
    const mu0 = -0.05;
    const toTop = -r * mu0 + Math.sqrt(r * r * mu0 * mu0 - r * r + (radiusM + 60_000) ** 2);
    const chord = toTop - 2 * r * Math.abs(mu0);
    const beam = sphericalBeamPath(
      radiusM,
      Float64Array.of(100_000, 60_000, 50_000, 0),
      Float64Array.of(0.01, 0, 0.3),
      mu0,
    );
    expect(Math.abs((beam.slantFactor[1] ?? Number.NaN) - chord / 10_000)).toBeLessThan(1e-9);
  });

  it("gives a clear layer the shadow's edge cuts its chord from the shadow height over that part", () => {
    const mu0 = -0.05;
    const s = Math.sqrt(1 - mu0 * mu0);
    const shadowM = radiusM / s - radiusM;
    const r = radiusM + shadowM;
    const toTop = -r * mu0 + Math.sqrt(r * r * mu0 * mu0 - r * r + (radiusM + 10_000) ** 2);
    const chord = toTop - 2 * r * Math.abs(mu0);
    const beam = sphericalBeamPath(
      radiusM,
      Float64Array.of(100_000, 10_000, 0),
      Float64Array.of(0.01, 0),
      mu0,
    );
    expect(Math.abs((beam.slantFactor[1] ?? Number.NaN) - chord / (10_000 - shadowM))).toBeLessThan(
      1e-6,
    );
  });

  it("takes μ₀ = 0 itself: the top lit at T 1, every interface above the ground lit, the ground dark", () => {
    const beam = sphericalBeamPath(radiusM, heights, depths, 0);
    const lit = Array.from(beam.interfaceTransmission.subarray(0, -1));
    expect([
      beam.interfaceTransmission[0],
      lit.every((t) => t > 0),
      beam.interfaceTransmission.at(-1),
    ]).toEqual([1, true, 0]);
  });

  it("refuses an optical depth not finite or below 0 though the column lies wholly in the shadow", () => {
    for (const bad of [Number.NaN, -3]) {
      const layers = Float64Array.from(depths, (tau, p) => (p === 5 ? bad : tau));
      expect(() => sphericalBeamPath(radiusM, heights, layers, -1)).toThrow(/optical depth/u);
    }
  });

  it("keeps T_{p+1} = T_p e^(−λ_p τ_p) at every lit layer, to 10⁻¹²", () => {
    for (const mu0 of BELOW) {
      const beam = sphericalBeamPath(radiusM, heights, depths, mu0);
      for (let p = 0; p < depths.length && !shadowed(p + 1, mu0); p += 1) {
        const top = beam.interfaceTransmission[p] ?? Number.NaN;
        const next = beam.interfaceTransmission[p + 1] ?? Number.NaN;
        expect(beam.topTransmission[p]).toBe(top);
        const carried =
          top * Math.exp(-(beam.slantFactor[p] ?? Number.NaN) * (depths[p] ?? Number.NaN));
        expect(Math.abs(carried - next)).toBeLessThanOrEqual(1e-12 * next);
      }
    }
  });

  it("gives the layer the shadow's edge cuts the λ that deposits its lit part's beam, to 10⁻⁶", () => {
    for (const mu0 of BELOW) {
      const beam = sphericalBeamPath(radiusM, heights, depths, mu0);
      const p = Array.from({ length: depths.length }, (_, q) => q).find(
        (q) => !shadowed(q, mu0) && shadowed(q + 1, mu0),
      );
      if (p === undefined) {
        continue;
      }
      // The lit part, from the layer's top down to the shadow height, at its own average secant.
      const s = Math.sqrt(1 - mu0 * mu0);
      const shadowM = radiusM / s - radiusM;
      const top = heights[p] ?? Number.NaN;
      const bottom = heights[p + 1] ?? Number.NaN;
      const tau = depths[p] ?? Number.NaN;
      const litDepth = (tau * (top - shadowM)) / (top - bottom);
      // The column with an interface at the shadow height, a hair above it so that its ray just
      // clears the ground, which it grazes through every layer below twice.
      const edgeM = shadowM + 1e-9 * (top - shadowM);
      const at = Float64Array.of(...heights.subarray(0, p + 1), edgeM, ...heights.subarray(p + 1));
      const split = Float64Array.of(
        ...depths.subarray(0, p),
        (tau * (top - edgeM)) / (top - bottom),
        (tau * (edgeM - bottom)) / (top - bottom),
        ...depths.subarray(p + 1),
      );
      const toEdge = slantByIntersections(radiusM, at, split, p + 1, mu0);
      const fromTop = beam.slantOpticalDepth[p] ?? Number.NaN;
      const litLambda = (toEdge - fromTop) / litDepth;
      const t = beam.topTransmission[p] ?? Number.NaN;
      const lambda = beam.slantFactor[p] ?? Number.NaN;
      const deposited = (t * -Math.expm1(-lambda * tau)) / lambda;
      const litDeposit = (t * -Math.expm1(-litLambda * litDepth)) / litLambda;
      expect(t).toBeGreaterThan(0);
      expect(lambda).toBeGreaterThan(litLambda);
      expect(Math.abs(deposited - litDeposit)).toBeLessThanOrEqual(1e-6 * litDeposit);
    }
  });

  it("joins the beam above the horizontal continuously at μ₀ = 0, its slant depths to 10⁻⁵", () => {
    const above = sphericalBeamPath(radiusM, heights, depths, 1e-9);
    const below = sphericalBeamPath(radiusM, heights, depths, -1e-9);
    for (let k = 1; k < heights.length - 1; k += 1) {
      const a = above.slantOpticalDepth[k] ?? Number.NaN;
      const b = below.slantOpticalDepth[k] ?? Number.NaN;
      expect(Math.abs(a - b)).toBeLessThanOrEqual(1e-5 * a);
    }
  });
});
