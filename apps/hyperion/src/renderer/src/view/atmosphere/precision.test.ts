/**
 * R08.T6.g's precision test (plan R08, Design notes 18 and 19; `decision-r08-f32.md` §6.1,
 * `decision-r08-remap.md` §6): each helper of the atmosphere's kernels emulated in `f32`, operation
 * for operation in the WGSL's order, as R05's `vertexEmulation.ts` emulates the terrain's vertex
 * arithmetic, against the same quantity in `f64`.
 *
 * @remarks
 * `Math.fround` of a sum, difference, product, quotient or square root of two `f32` values is the
 * correctly rounded `f32` result (Figueroa 1995, "When is double rounding innocuous?"), so the
 * emulation holds each formula under IEEE-rounded operations. A GPU may contract a × b + c and
 * allows 2.5 ULP in a division and inherits `sqrt` from 1 ÷ `inverseSqrt` (W3C WGSL, CRD
 * 2026-09-21, §15.7.4.1, and §15.7.5 for contraction); the kernels' own agreement with the twin is the
 * smoke's (`smoke/atmosphere.ts`). R05's radius forms and sebh's texel remap, emulated the same
 * way, are asserted to fail, so that the test is known to see the defects it guards against. Each
 * check compares like with like: the `f64` side takes the same `f32` inputs (radius, height,
 * cosine, distance), so that it measures the arithmetic, not the inputs' rounding, except the ray
 * march's camera, which reaches the kernel narrowed while its quadric constants come from `f64`.
 */

import { describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import {
  type AtmosphereInputs,
  geodeticOf,
  type SpheroidFigure,
  skyViewHeight,
  TABLE_SIZES,
  viewExactOf,
} from "./hillaire";
import type { AtmosphereMedium } from "./medium";
import { marchPieces, marchPiecesSteps, marchSplit } from "./marchSteps";
import { MULTI_SCATTERING_SIZE, TRANSMITTANCE_SIZE } from "./tables";
import { preciseAtan, preciseAtan2, preciseSinCos, TRIG_HALF_PI } from "../../test/preciseTrigF32";

const f = Math.fround;

/** f32's unit roundoff, 2⁻²⁴. */
const EPSILON = 2 ** -24;

/** The atmosphere's height above the ground the checks take, m: R05's Earth's. */
const ATMOSPHERE_M = 100_000;

/** A medium of no term under {@link ATMOSPHERE_M}: what the ray march's quadric constants take. */
const BARE_MEDIUM: AtmosphereMedium = {
  name: "bare",
  topHeightM: ATMOSPHERE_M,
  groundAlbedo: [0, 0, 0],
  terms: [],
};

/** Lookup inputs at the datum on the equator, s ≡ 1: what `viewExactOf` reads of them. */
function bareInputsOf(figure: SpheroidFigure): AtmosphereInputs {
  return {
    radiusM: figure.equatorialRadiusM,
    heightM: 0,
    normal: { x: 1, y: 0, z: 0 },
    latitudeRad: 0,
    gravityRatio: 1,
  };
}

/** The spheres of `decision-r08-f32.md` §6.1: Earth's mean radius, 2 R⊕, Saturn's, 2 R_J, m. */
const SPHERES_M = [6_371_008.8, 12_742_017.6, 60_268_000, 142_984_000] as const;

/** The cameras' heights above a sphere, m. */
const CAMERA_HEIGHTS_M = [1, 2, 20, 200, 2_000, 20_000] as const;

// --- f32 vectors ------------------------------------------------------------------------------

function vec(x: number, y: number, z: number): Vec3 {
  return { x: f(x), y: f(y), z: f(z) };
}

function addV(a: Vec3, b: Vec3): Vec3 {
  return { x: f(a.x + b.x), y: f(a.y + b.y), z: f(a.z + b.z) };
}

function scaleV(v: Vec3, k: number): Vec3 {
  return { x: f(v.x * k), y: f(v.y * k), z: f(v.z * k) };
}

function mulV(a: Vec3, b: Vec3): Vec3 {
  return { x: f(a.x * b.x), y: f(a.y * b.y), z: f(a.z * b.z) };
}

function dotV(a: Vec3, b: Vec3): number {
  return f(f(f(a.x * b.x) + f(a.y * b.y)) + f(a.z * b.z));
}

function crossV(a: Vec3, b: Vec3): Vec3 {
  return {
    x: f(f(a.y * b.z) - f(a.z * b.y)),
    y: f(f(a.z * b.x) - f(a.x * b.z)),
    z: f(f(a.x * b.y) - f(a.y * b.x)),
  };
}

function lengthV(v: Vec3): number {
  return f(Math.sqrt(dotV(v, v)));
}

// --- common.wgsl: the shell from a height -----------------------------------------------------

// As the WGSL names them: `bottomM` the sphere's radius, `topHeightM` the atmosphere's height above
// it, `heightM` a point's height above the sphere, each m.

function aboveGround(bottomM: number, heightM: number): number {
  return f(heightM * f(f(2 * bottomM) + heightM));
}

function belowTop(bottomM: number, topHeightM: number, heightM: number): number {
  return f(f(topHeightM - heightM) * f(f(f(2 * bottomM) + topHeightM) + heightM));
}

function groundHit(bottomM: number, heightM: number, mu: number): boolean {
  const rMu = f(f(bottomM + heightM) * mu);
  return mu < 0 && f(rMu * rMu) >= aboveGround(bottomM, heightM);
}

function distanceToGround(bottomM: number, heightM: number, mu: number): number {
  const rMu = f(f(bottomM + heightM) * mu);
  const a = aboveGround(bottomM, heightM);
  return f(a / f(-rMu + f(Math.sqrt(Math.max(f(f(rMu * rMu) - a), 0)))));
}

function distanceToTop(bottomM: number, topHeightM: number, heightM: number, mu: number): number {
  const rMu = f(f(bottomM + heightM) * mu);
  const b = belowTop(bottomM, topHeightM, heightM);
  const root = f(Math.sqrt(Math.max(f(f(rMu * rMu) + b), 0)));
  return Math.max(mu > 0 ? f(b / f(rMu + root)) : f(-rMu + root), 0);
}

function heightAlong(bottomM: number, heightM: number, mu: number, t: number, rT: number): number {
  const rMu = f(f(bottomM + heightM) * mu);
  return f(f(f(f(t * t) + f(f(2 * rMu) * t)) + aboveGround(bottomM, heightM)) / f(rT + bottomM));
}

function unitClamp(x: number): number {
  return Math.min(Math.max(x, 0), 1);
}

/** `transmittanceUnitAt`: (x_μ, x_r). */
function transmittanceUnitAt(
  bottomM: number,
  topHeightM: number,
  heightM: number,
  mu: number,
): readonly [number, number] {
  const bigH = f(Math.sqrt(f(topHeightM * f(f(2 * bottomM) + topHeightM))));
  const rho = f(Math.sqrt(Math.max(aboveGround(bottomM, heightM), 0)));
  const d = distanceToTop(bottomM, topHeightM, heightM, mu);
  const dMin = f(topHeightM - heightM);
  const dMax = f(rho + bigH);
  return [f(f(d - dMin) / f(dMax - dMin)), f(rho / bigH)];
}

/** `multiScatteringUnitAt`, its 10 m ground offset (`GROUND_OFFSET_M`). */
function multiScatteringUnitAt(
  topHeightM: number,
  heightM: number,
  muSun: number,
): readonly [number, number] {
  const y = f(f(heightM - 10) / f(topHeightM - 10));
  return [unitClamp(f(f(muSun * 0.5) + 0.5)), unitClamp(y)];
}

/** `medium.wgsl`'s `transmittanceTexelOf`: a texel's height and zenith cosine. */
function transmittanceTexelOf(
  bottomM: number,
  topHeightM: number,
  unit: readonly [number, number],
): { readonly heightM: number; readonly mu: number } {
  const [ux, uy] = unit;
  const bigH = f(Math.sqrt(f(topHeightM * f(f(2 * bottomM) + topHeightM))));
  const rho = f(bigH * uy);
  const heightM = f(
    f(rho * rho) / f(f(Math.sqrt(f(f(rho * rho) + f(bottomM * bottomM)))) + bottomM),
  );
  const dMin = f(topHeightM - heightM);
  const dMax = f(rho + bigH);
  const d = f(dMin + f(ux * f(dMax - dMin)));
  const below = belowTop(bottomM, topHeightM, heightM);
  const mu = d === 0 ? 1 : f(f(below - f(d * d)) / f(f(2 * f(bottomM + heightM)) * d));
  return { heightM, mu: Math.min(Math.max(mu, -1), 1) };
}

// Bruneton's pair, and the filter's position in texels (`bilinear`'s uv × size − ½).
function unitFromTexelUv(uv: number, n: number): number {
  return f(f(f(uv * n) - 0.5) / f(n - 1));
}

function texelUvFromUnit(unit: number, n: number): number {
  return f(f(f(unit * f(n - 1)) + 0.5) / n);
}

function texelCentre(i: number, n: number): number {
  return f(f(i + 0.5) / n);
}

function filterPosition(uv: number, n: number): number {
  return f(f(uv * n) - 0.5);
}

/** sebh's `fromUnitToSubUvs`, R05's read of the multiple-scattering table and the sky view. */
function sebhUvFromUnit(unit: number, n: number): number {
  return f(f(unit + f(0.5 / n)) * f(n / f(n + 1)));
}

// --- R05's radius forms ---------------------------------------------------------------------------

function r05GroundHit(bottom: number, r: number, mu: number): boolean {
  return mu < 0 && f(f(f(r * r) * f(f(mu * mu) - 1)) + f(bottom * bottom)) >= 0;
}

/** R05's `rayMarch.wgsl` `heightAbove`, `inverseSqrt` as the correctly rounded 1 ÷ √x. */
function r05HeightAbove(p: Vec3, a: number, c: number): number {
  const r = lengthV(p);
  const d = { x: f(p.x / r), y: f(p.y / r), z: f(p.z / r) };
  const x = f(f(f(f(d.x * d.x) + f(d.y * d.y)) / f(a * a)) + f(f(d.z * d.z) / f(c * c)));
  return f(r - f(1 / Math.sqrt(x)));
}

// --- The spheres ------------------------------------------------------------------------------------

/** One sphere's worst figures, each as a fraction of its bound (at most 1 to pass). */
interface SphereReport {
  /** Heights against 10⁻⁶ · max(t, |h|, 1 km). */
  readonly height: number;
  /** Distances to the ground or the top against 10⁻⁵ of themselves. */
  readonly distance: number;
  /** Ground and shadow decisions outside the 10⁻⁶ band that differ from f64's. */
  readonly flips: number;
  readonly decisions: number;
  /** Transmittance and multiple-scattering coordinates against 10⁻³ of a texel. */
  readonly texel: number;
  /** R05's |p| − R against the height bound. */
  readonly r05Height: number;
  /** R05's r²(μ² − 1) + R² decisions that differ from f64's outside the band. */
  readonly r05Flips: number;
}

/** The zenith cosines a camera's rays take: even from −1 to 1, and dense about the horizon's dip. */
function cosinesFrom(bottomM: number, heightM: number): number[] {
  const r = bottomM + heightM;
  const dip = -Math.sqrt(heightM * (2 * bottomM + heightM)) / r;
  const even = Array.from({ length: 401 }, (_, i) => -1 + i / 200);
  const nearDip = Array.from({ length: 57 }, (_, k) => 10 ** ((k - 60) / 6)).flatMap((s) => [
    dip * (1 + s),
    dip * (1 - s),
  ]);
  return [...even, ...nearDip].map((mu) => f(Math.min(Math.max(mu, -1), 1)));
}

function sphereReport(radiusM: number): SphereReport {
  const bottom = f(radiusM);
  const top = f(ATMOSPHERE_M);
  const texelsU = TRANSMITTANCE_SIZE.widthTexels - 1;
  const texelsV = TRANSMITTANCE_SIZE.heightTexels - 1;
  const texelsMs = MULTI_SCATTERING_SIZE.heightTexels - 1;
  let height = 0;
  let distance = 0;
  let flips = 0;
  let decisions = 0;
  let texel = 0;
  let r05Height = 0;
  let r05Flips = 0;
  /** The coordinates' worst at a height and cosine, against f64 at the same inputs. */
  const coordinates = (h: number, mu: number): void => {
    const [u, v] = transmittanceUnitAt(bottom, top, h, mu);
    const bigH = Math.sqrt(top * (2 * bottom + top));
    const rho = Math.sqrt(h * (2 * bottom + h));
    const rMu = (bottom + h) * mu;
    const b = (top - h) * (2 * bottom + top + h);
    const root = Math.sqrt(Math.max(rMu * rMu + b, 0));
    const d = Math.max(mu > 0 ? b / (rMu + root) : -rMu + root, 0);
    const u64 = (d - (top - h)) / (rho + bigH - (top - h));
    const v64 = rho / bigH;
    const [, y] = multiScatteringUnitAt(top, h, mu);
    const y64 = unitClamp((h - 10) / (top - 10));
    texel = Math.max(
      texel,
      (Math.abs(unitClamp(u) - unitClamp(u64)) * texelsU) / 1e-3,
      (Math.abs(v - v64) * texelsV) / 1e-3,
      (Math.abs(y - y64) * texelsMs) / 1e-3,
    );
  };
  for (const cameraM of CAMERA_HEIGHTS_M) {
    const h0 = f(cameraM);
    const r = bottom + h0;
    for (const mu of cosinesFrom(bottom, h0)) {
      const a = h0 * (2 * bottom + h0);
      const rMu = r * mu;
      const disc = rMu * rMu - a;
      const hit = mu < 0 && disc >= 0;
      if (Math.abs(disc) >= 1e-6 * a) {
        decisions += 1;
        flips += groundHit(bottom, h0, mu) === hit ? 0 : 1;
        r05Flips += r05GroundHit(bottom, f(bottom + h0), mu) === hit ? 0 : 1;
      }
      const b = (top - h0) * (2 * bottom + top + h0);
      const tEnd = hit
        ? a / (-rMu + Math.sqrt(disc))
        : Math.max(
            mu > 0 ? b / (rMu + Math.sqrt(rMu * rMu + b)) : -rMu + Math.sqrt(rMu * rMu + b),
            0,
          );
      if (hit && disc >= 1e-3 * a) {
        distance = Math.max(
          distance,
          Math.abs(distanceToGround(bottom, h0, mu) - tEnd) / (1e-5 * tEnd),
        );
      } else if (!hit && tEnd > 0) {
        distance = Math.max(
          distance,
          Math.abs(distanceToTop(bottom, top, h0, mu) - tEnd) / (1e-5 * tEnd),
        );
      }
      coordinates(h0, mu);
      // The ray as the sky view marches it: from (0, 0, r) along (√(1 − μ²), 0, μ).
      const dx = f(Math.sqrt(f(1 - f(mu * mu))));
      const origin = f(bottom + h0);
      for (let k = 0; k <= 24; k += 1) {
        const t = f(tEnd * (k / 24) ** 2);
        const px = f(t * dx);
        const pz = f(origin + f(t * mu));
        const rT = f(Math.sqrt(f(f(px * px) + f(pz * pz))));
        const hEmulated = heightAlong(bottom, h0, mu, t, rT);
        const qx = t * dx;
        const qz = origin + t * mu;
        const length = Math.sqrt(qx * qx + qz * qz);
        const h64 = (qx * qx + (h0 + t * mu) * (qz + bottom)) / (length + bottom);
        const bound = 1e-6 * Math.max(t, Math.abs(h64), 1_000);
        height = Math.max(height, Math.abs(hEmulated - h64) / bound);
        r05Height = Math.max(r05Height, Math.abs(f(rT - bottom) - h64) / bound);
        // The reads clamp the height into the shell (`tableTransmittance`).
        coordinates(Math.min(Math.max(hEmulated, 0), top), mu);
      }
    }
  }
  return { height, distance, flips, decisions, texel, r05Height, r05Flips };
}

describe.each(SPHERES_M)(
  "the shell's forms from a height (common.wgsl), on a sphere of %d m",
  (radiusM) => {
    // One sphere's figures, computed once for its own tests.
    const report = sphereReport(radiusM);

    it("keeps every height along a ray within 10⁻⁶ · max(t, |h|, 1 km), from 1 m to 20 km", () => {
      expect(report.height).toBeLessThanOrEqual(1);
    });

    it("keeps every distance to the ground or the top within 10⁻⁵ of itself off the tangency", () => {
      expect(report.distance).toBeLessThanOrEqual(1);
    });

    it("decides every ground and shadow test as f64 does outside |(rμ)² − A| < 10⁻⁶ A", () => {
      expect(report.decisions).toBeGreaterThan(2_500);
      expect(report.flips).toBe(0);
    });

    it("keeps every transmittance coordinate and multiple-scattering row within 10⁻³ of a texel", () => {
      expect(report.texel).toBeLessThanOrEqual(1);
    });

    it("finds R05's |p| − R over the height bound", () => {
      expect(report.r05Height).toBeGreaterThan(1);
    });

    it("finds R05's r²(μ² − 1) + R² flipping decisions outside the band", () => {
      expect(report.r05Flips).toBeGreaterThan(0);
    });
  },
);

// --- The layer edges' crossings (common.wgsl, R08.T6.d) -----------------------------------------

/** `shellCrossings`: the signed distances, m, at which a ray crosses the sphere at `edgeM`. */
function shellCrossings(
  bottomM: number,
  heightM: number,
  mu: number,
  edgeM: number,
): readonly [number, number] | null {
  const rMu = f(f(bottomM + heightM) * mu);
  const c = f(f(heightM - edgeM) * f(f(f(2 * bottomM) + heightM) + edgeM));
  const disc = f(f(rMu * rMu) - c);
  if (disc < 0) {
    return null;
  }
  const q = -f(rMu + (rMu >= 0 ? 1 : -1) * f(Math.sqrt(disc)));
  if (q === 0) {
    return null;
  }
  const other = f(c / q);
  return [Math.min(q, other), Math.max(q, other)];
}

/** The layer edges the check takes, m: T6.d's ozone slab's four (`EARTH_OZONE_SLAB`) and two more. */
const EDGES_M = [15_000, 18_596, 18_674, 38_491, 38_501, 60_000] as const;

/** The zenith cosines about the ray tangent to an edge's sphere from a height, and every 0.05. */
function cosinesAboutEdge(bottomM: number, heightM: number, edgeM: number): number[] {
  const even = Array.from({ length: 41 }, (_, i) => -1 + i / 20);
  if (edgeM >= heightM) {
    return even.map(f);
  }
  const tangent = -Math.sqrt(1 - ((bottomM + edgeM) / (bottomM + heightM)) ** 2);
  const near = Array.from({ length: 49 }, (_, k) => 10 ** ((k - 56) / 6)).flatMap((s) => [
    tangent * (1 + s),
    tangent * (1 - s),
  ]);
  return [...even, ...near].map((mu) => f(Math.min(Math.max(mu, -1), 1)));
}

/**
 * The worst height off its edge, m, of a crossing ahead on a transmittance ray (one that misses the
 * ground, up to the top), their count, and the worst height off its edge of a ray's lowest point
 * where f64 finds crossings the f32 form does not.
 */
function crossingReport(radiusM: number): {
  readonly worstM: number;
  readonly crossings: number;
  readonly missedM: number;
} {
  const bottom = f(radiusM);
  let worstM = 0;
  let missedM = 0;
  let crossings = 0;
  for (const height of [...CAMERA_HEIGHTS_M, 30_000, 60_000, 99_000]) {
    const heightM = f(height);
    for (const edge of EDGES_M) {
      const edgeM = f(edge);
      for (const mu of cosinesAboutEdge(bottom, heightM, edgeM)) {
        // f64 at the same f32 inputs: the height along the ray at t, and the ray's crossings, of
        // the rays the transmittance table takes, which miss the ground, up to the top.
        const r = bottom + heightM;
        const along = (t: number): number =>
          Math.sqrt(r * r + t * t + 2 * r * mu * t) - bottom - edgeM;
        const disc64 = (r * mu) ** 2 - (heightM - edgeM) * (2 * bottom + heightM + edgeM);
        const toGround = (r * mu) ** 2 - heightM * (2 * bottom + heightM);
        const topM = bottom + ATMOSPHERE_M;
        const tTop = -r * mu + Math.sqrt((r * mu) ** 2 + topM * topM - r * r);
        if (mu < 0 && toGround >= 0) {
          continue;
        }
        const found = shellCrossings(bottom, heightM, mu, edgeM);
        if (found === null) {
          // A miss f64 does not make is a graze: the ray's lowest point stands at the edge.
          if (disc64 >= 0) {
            missedM = Math.max(missedM, Math.abs(along(Math.max(-r * mu, 0))));
          }
          continue;
        }
        for (const t of found.filter((root) => root > 0 && root < tTop)) {
          crossings += 1;
          worstM = Math.max(worstM, Math.abs(along(t)));
        }
      }
    }
  }
  return { worstM, crossings, missedM };
}

describe.each(SPHERES_M)(
  "the layer edges' crossings (common.wgsl), on a sphere of %d m",
  (radiusM) => {
    const report = crossingReport(radiusM);

    // The ozone slab's narrowest ramp is 10 m, at its top (`earth.ts`'s `EARTH_OZONE_SLAB`): the split's own
    // error, measured 0.012–0.017 m on these spheres, is a few hundredths of a metre in height.
    it("puts every crossing ahead within 0.05 m of its edge in height, and misses only grazes", () => {
      expect(report.crossings).toBeGreaterThan(2_500);
      expect(report.worstM).toBeLessThan(0.05);
      expect(report.missedM).toBeLessThan(0.05);
    });
  },
);

/** A camera's heights about an edge, m: at it and a metre either side. */
function nearEdgeM(edgeM: number): number[] {
  return [edgeM - 1, edgeM, edgeM + 1];
}

/**
 * The per-frame marches' piece boundaries at their crossings on a sphere (R08.T6.i): the sky view's
 * and the multiple-scattering kernel's, `marchWalkCrossSphere` and `marchWalkStep` in `f32`. Each
 * crossing ahead within the ray's segment, to the ground or the top, is taken as its offset from
 * t* = clamp(−rμ, 0, t_end) and stands back on the ray at t* plus that offset. Returns the worst
 * height off its edge there as a fraction of R08.T6.g's bound, 10⁻⁶ · max(t, e, 1 km), and the
 * count.
 */
function marchCrossingReport(radiusM: number): { readonly worst: number; readonly count: number } {
  const bottom = f(radiusM);
  const top = f(ATMOSPHERE_M);
  let worst = 0;
  let count = 0;
  // The cameras include each edge's own height, ±1 m, where a crossing's constant cancels most.
  const heights = [...CAMERA_HEIGHTS_M, 30_000, 60_000, 99_000, ...EDGES_M.flatMap(nearEdgeM)];
  for (const height of heights) {
    const heightM = f(height);
    for (const edge of EDGES_M) {
      const edgeM = f(edge);
      for (const mu of cosinesAboutEdge(bottom, heightM, edgeM)) {
        const r = bottom + heightM;
        const offEdge = (t: number): number =>
          Math.sqrt(r * r + t * t + 2 * r * mu * t) - bottom - edgeM;
        const tEnd = groundHit(bottom, heightM, mu)
          ? distanceToGround(bottom, heightM, mu)
          : distanceToTop(bottom, top, heightM, mu);
        const lowest = Math.min(Math.max(-f(f(bottom + heightM) * mu), 0), tEnd);
        const after = f(tEnd - lowest);
        for (const t of shellCrossings(bottom, heightM, mu, edgeM) ?? []) {
          const offset = f(t - lowest);
          const inside = (offset > -lowest && offset < 0) || (offset > 0 && offset < after);
          if (!(t > 0) || !inside) {
            continue;
          }
          const boundary = f(lowest + offset);
          count += 1;
          const bound = 1e-6 * Math.max(boundary, edgeM, 1_000);
          worst = Math.max(worst, Math.abs(offEdge(boundary)) / bound);
        }
      }
    }
  }
  return { worst, count };
}

describe.each(SPHERES_M)(
  "the per-frame marches' piece boundaries at layer edges (common.wgsl, R08.T6.i), on a sphere of %d m",
  (radiusM) => {
    const report = marchCrossingReport(radiusM);

    it("stand within 10⁻⁶ · max(t, e, 1 km) of their edge in height, the ground's rays among them", () => {
      // Measured 0.14–0.19 of the bound, over 9,062–9,444 crossings a sphere (2026-10-13).
      expect(report.count).toBeGreaterThan(8_000);
      expect(report.worst).toBeLessThanOrEqual(1);
    });
  },
);

// --- The ray march's quadric (rayMarch.wgsl) ---------------------------------------------------

/** WGS 84 (NIMA TR8350.2). */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257_223_563),
};

/** The figures of `decision-r08-f32.md` §6.1. */
/** A Saturn-class figure: Saturn's equatorial and polar radii at 1 bar, m. */
const SATURN_CLASS: SpheroidFigure = { equatorialRadiusM: 60_268_000, polarRadiusM: 54_364_000 };

const FIGURES: ReadonlyArray<readonly [string, SpheroidFigure]> = [
  ["WGS 84", WGS84],
  ["a Saturn-class figure", SATURN_CLASS],
  ["a 2 R⊕ sphere", { equatorialRadiusM: 12_742_017.6, polarRadiusM: 12_742_017.6 }],
  ["a 2 R_J sphere", { equatorialRadiusM: 142_984_000, polarRadiusM: 142_984_000 }],
];

/** `rayMarch.wgsl`'s `RayQuadric`. */
interface RayQuadric {
  readonly qa: number;
  readonly qb: number;
  readonly qc: number;
  readonly disc: number;
}

function rayQuadric(
  origin: Vec3,
  dir: Vec3,
  k: number,
  radius: number,
  qc: number,
  far: boolean,
): RayQuadric {
  const scale = { x: 1, y: 1, z: k };
  const o = mulV(origin, scale);
  const d = mulV(dir, scale);
  const qa = dotV(d, d);
  const qb = dotV(o, d);
  let disc: number;
  if (far) {
    const c = crossV(o, d);
    disc = f(f(qa * f(radius * radius)) - dotV(c, c));
  } else {
    disc = f(f(qb * qb) - f(qa * qc));
  }
  return { qa, qb, qc, disc };
}

function quadricHits(q: RayQuadric): readonly [number, number] {
  if (q.disc < 0) {
    return [-1, -1];
  }
  const root = f(Math.sqrt(q.disc));
  const s = f(-f(q.qb + (q.qb >= 0 ? root : -root)));
  if (s === 0) {
    return [0, 0];
  }
  const first = f(s / q.qa);
  const second = f(q.qc / s);
  return [Math.min(first, second), Math.max(first, second)];
}

function radialHeight(r: number, q: number): number {
  const root = f(Math.sqrt(Math.max(f(1 + q), 1e-12)));
  return f(f(r * q) / f(root * f(root + 1)));
}

/** `common.wgsl`'s `marchSplit` and `marchStep`: each step's t and signed offset from t*. */
function marchSteps(
  tStart: number,
  tEnd: number,
  nearest: number,
  samples: number,
  fromCamera: boolean,
): {
  readonly lowestM: number;
  readonly steps: ReadonlyArray<{ readonly tM: number; readonly fromLowestM: number }>;
} {
  const lowest = Math.min(Math.max(nearest, tStart), tEnd);
  const before = f(lowest - tStart);
  const after = f(tEnd - lowest);
  const rootBefore = f(Math.sqrt(before));
  const total = f(rootBefore + f(Math.sqrt(after)));
  const share = total > 0 ? Math.floor(f(f(f(samples * rootBefore) / total) + 0.5)) : 0;
  const least = before > 0 ? 1 : 0;
  const most = samples - (after > 0 ? 1 : 0);
  const stepsBefore = Math.min(Math.max(share, least), most);
  const towardStart = fromCamera && before > 0 && after > 0;
  const stepsAfter = samples - stepsBefore;
  const steps = Array.from({ length: samples }, (_, i) => {
    if (i < stepsBefore) {
      const unit = f(before / f(stepsBefore * stepsBefore));
      if (towardStart) {
        const fromStart = f(unit * f(f(f(i * i) + i) + 0.5));
        return { tM: f(tStart + fromStart), fromLowestM: f(fromStart - before) };
      }
      const k = stepsBefore - 1 - i;
      const fromLowest = f(unit * f(f(f(k * k) + k) + 0.5));
      return { tM: f(lowest - fromLowest), fromLowestM: -fromLowest };
    }
    const k = i - stepsBefore;
    const unit = f(after / f(stepsAfter * stepsAfter));
    const fromLowest = f(unit * f(f(f(k * k) + k) + 0.5));
    return { tM: f(lowest + fromLowest), fromLowestM: fromLowest };
  });
  return { lowestM: lowest, steps };
}

/**
 * `oblate.wgsl`'s `datumPointOf` height, emulated in f32 operation for operation: the root of the
 * exact quadratic along the similar spheroid's normal ν, a q ÷ (u + √(u² − κ″ q)), from the point's
 * ground quadric (R08.T6.f).
 */
function datumHeight(p: Vec3, quadricM2: number, a: number, k2: number): number {
  const g = vec(f(p.x / a), f(p.y / a), f(f(p.z * k2) / a));
  const u = lengthV(g);
  const nu = vec(f(g.x / u), f(g.y / u), f(g.z / u));
  const kappa = f(f(f(nu.x * nu.x) + f(nu.y * nu.y)) + f(k2 * f(nu.z * nu.z)));
  const q = f(quadricM2 / f(a * a));
  const root = f(Math.sqrt(Math.max(f(f(u * u) - f(kappa * q)), 0)));
  return f(f(a * q) / f(u + root));
}

/** {@link datumHeight} in f64, its quadric formed from the point itself. */
function datumHeight64(p: Vec3, figure: SpheroidFigure): number {
  const a = figure.equatorialRadiusM;
  const k = a / figure.polarRadiusM;
  const g = { x: p.x / a, y: p.y / a, z: (p.z * k * k) / a };
  const u = Math.hypot(g.x, g.y, g.z);
  const kappa = (g.x ** 2 + g.y ** 2 + k * k * g.z ** 2) / (u * u);
  const z = p.z * k;
  const q = (p.x * p.x + p.y * p.y + z * z - a * a) / (a * a);
  return (a * q) / (u + Math.sqrt(Math.max(u * u - kappa * q, 0)));
}

/** A sample's radial height above the true figure in f64 at P + t d. */
function radialHeight64(p: Vec3, figure: SpheroidFigure): number {
  const a = figure.equatorialRadiusM;
  const k = a / figure.polarRadiusM;
  const r = Math.hypot(p.x, p.y, p.z);
  const z = p.z * k;
  const q = (p.x * p.x + p.y * p.y + z * z - a * a) / (a * a);
  const root = Math.sqrt(1 + q);
  return (r * q) / (root * (root + 1));
}

/** The ray march's heights along one ray, emulated, with each step's t and t − t* in f64. */
function marchHeights(
  camera: Vec3,
  dir: Vec3,
  figure: SpheroidFigure,
  far: boolean,
  samples: number,
): Array<{
  readonly t: number;
  /** The emulated height's error, m. */
  readonly h: number;
  /** R05's `heightAbove`'s error, m. */
  readonly r05: number;
  /** |t − t*|, m, t* the ray's closest approach to the scaled centre in f64. */
  readonly fromStar: number;
  /** The sample's height in f64, m. */
  readonly height64: number;
  /** The emulated datum height's error against its own form in f64, m (R08.T6.f). */
  readonly datum: number;
}> {
  const exact = viewExactOf(camera, figure, BARE_MEDIUM, bareInputsOf(figure));
  const origin = vec(camera.x, camera.y, camera.z);
  const a = f(figure.equatorialRadiusM);
  const c = f(figure.polarRadiusM);
  const topM = f(ATMOSPHERE_M);
  const k = f(a / c);
  const shellQ = rayQuadric(
    origin,
    dir,
    f(f(a + topM) / f(c + topM)),
    f(a + topM),
    f(exact.topQuadricM2),
    far,
  );
  const groundQ = rayQuadric(origin, dir, k, a, f(exact.groundQuadricM2), far);
  const shell = quadricHits(shellQ);
  const ground = quadricHits(groundQ);
  const tStart = Math.max(shell[0], 0);
  let tEnd = shell[1];
  if (ground[0] > 0) {
    tEnd = Math.min(tEnd, ground[0]);
  }
  if (shell[1] <= 0 || tEnd <= tStart) {
    return [];
  }
  const nearest = f(-dotV(origin, dir));
  const split = marchSteps(tStart, tEnd, nearest, samples, shell[0] <= 0);
  const qStar = f(-groundQ.disc / groundQ.qa);
  const nearestFromStar = f(
    f(f(f(f(k - 1) * f(k + 1)) * dir.z) * crossV(dir, crossV(origin, dir)).z) / groundQ.qa,
  );
  let lowestFromStar = f(f(split.lowestM - nearest) + nearestFromStar);
  if (ground[0] > 0 && split.lowestM === ground[0]) {
    lowestFromStar = f(-f(Math.sqrt(Math.max(groundQ.disc, 0))) / groundQ.qa);
  }
  const a2 = f(a * a);
  const k2 = f(k * k);
  const kTrue = figure.equatorialRadiusM / figure.polarRadiusM;
  const sdx = dir.z * kTrue;
  const tStar =
    -(camera.x * dir.x + camera.y * dir.y + camera.z * kTrue * sdx) /
    (dir.x * dir.x + dir.y * dir.y + sdx * sdx);
  return split.steps.map(({ tM, fromLowestM }) => {
    const p = addV(origin, scaleV(dir, tM));
    let bigQ: number;
    if (far) {
      const fromStar = f(fromLowestM + lowestFromStar);
      bigQ = f(f(f(groundQ.qa * fromStar) * fromStar) + qStar);
    } else {
      bigQ = f(f(tM * f(f(groundQ.qa * tM) + f(2 * groundQ.qb))) + groundQ.qc);
    }
    const h = radialHeight(lengthV(p), f(bigQ / a2));
    const p64 = { x: camera.x + tM * dir.x, y: camera.y + tM * dir.y, z: camera.z + tM * dir.z };
    const h64 = radialHeight64(p64, figure);
    return {
      t: tM,
      h: h - h64,
      r05: r05HeightAbove(p, a, c) - h64,
      fromStar: Math.abs(tM - tStar),
      height64: h64,
      datum: datumHeight(p, bigQ, a, k2) - datumHeight64(p64, figure),
    };
  });
}

/** The body-fixed point at geodetic latitude φ, longitude λ and height h, and its local axes. */
function localFrame(
  figure: SpheroidFigure,
  latitudeDeg: number,
  heightM: number,
): { readonly p: Vec3; readonly up: Vec3; readonly north: Vec3; readonly east: Vec3 } {
  const a = figure.equatorialRadiusM;
  const e2 = 1 - (figure.polarRadiusM / a) ** 2;
  const phi = (latitudeDeg * Math.PI) / 180;
  const lambda = (20 * Math.PI) / 180;
  const n = a / Math.sqrt(1 - e2 * Math.sin(phi) ** 2);
  const cos = Math.cos(phi);
  const sin = Math.sin(phi);
  return {
    p: {
      x: (n + heightM) * cos * Math.cos(lambda),
      y: (n + heightM) * cos * Math.sin(lambda),
      z: (n * (1 - e2) + heightM) * sin,
    },
    up: { x: cos * Math.cos(lambda), y: cos * Math.sin(lambda), z: sin },
    north: { x: -sin * Math.cos(lambda), y: -sin * Math.sin(lambda), z: cos },
    east: { x: -Math.sin(lambda), y: Math.cos(lambda), z: 0 },
  };
}

/** A unit direction from zenith angle θ and azimuth α (from north towards east), narrowed. */
function directionOf(
  frame: { readonly up: Vec3; readonly north: Vec3; readonly east: Vec3 },
  zenithRad: number,
  azimuthRad: number,
): Vec3 {
  const s = Math.sin(zenithRad);
  const n = s * Math.cos(azimuthRad);
  const e = s * Math.sin(azimuthRad);
  const u = Math.cos(zenithRad);
  const d = {
    x: n * frame.north.x + e * frame.east.x + u * frame.up.x,
    y: n * frame.north.y + e * frame.east.y + u * frame.up.y,
    z: n * frame.north.z + e * frame.east.z + u * frame.up.z,
  };
  const length = Math.hypot(d.x, d.y, d.z);
  return vec(d.x / length, d.y / length, d.z / length);
}

/**
 * The near form's worst heights on a figure, as fractions of the bound, R05's, and the datum
 * heights' (R08.T6.f).
 */
function nearReport(figure: SpheroidFigure): {
  readonly worst: number;
  readonly r05: number;
  readonly datum: number;
} {
  let worst = 0;
  let r05 = 0;
  let datum = 0;
  const a = figure.equatorialRadiusM;
  for (const latitudeDeg of [0, 35, 70, 89.5]) {
    for (const heightM of [-400, 1, 2, 20, 200, 2_000, 20_000, 400_000]) {
      const frame = localFrame(figure, latitudeDeg, heightM);
      const dip = Math.acos(a / (a + Math.max(heightM, 1)));
      const zeniths = [0, 30, 60, 80, 85, 88, 89, 89.5, 90, 90.3, 91, 93, 100, 120, 150, 180].map(
        (deg) => (deg * Math.PI) / 180,
      );
      for (const s of [0.5, 0.9, 0.99, 1.01, 1.1, 2]) {
        zeniths.push(Math.PI / 2 + dip * s);
      }
      for (const zenith of zeniths) {
        for (const azimuthDeg of [0, 45, 90]) {
          const dir = directionOf(frame, zenith, (azimuthDeg * Math.PI) / 180);
          for (const sample of marchHeights(frame.p, dir, figure, false, 64)) {
            // A ray from below the datum straight down crosses the body's interior, where no
            // height means anything and terrain ends it: samples deeper than R09's 20 km are left.
            if (sample.height64 < -20_000) {
              continue;
            }
            const bound = 1e-6 * Math.max(sample.t, Math.abs(sample.height64), 1_000);
            worst = Math.max(worst, Math.abs(sample.h) / bound);
            r05 = Math.max(r05, Math.abs(sample.r05) / bound);
            datum = Math.max(datum, Math.abs(sample.datum) / bound);
          }
        }
      }
    }
  }
  return { worst, r05, datum };
}

/**
 * The closest-approach form's worst heights near the limb and across the disc, as fractions of the
 * bound: rays whose tangent height is 0–100 km, and rays that meet the ground (a negative tangent
 * height), whose segment ends at the ground's near root. `far` forces the form; the kernel takes
 * the far form above h₀ = a ÷ 2, so at 1.5a, where h₀ is a ÷ 2 on a sphere, both are held.
 */
function farReport(
  figure: SpheroidFigure,
  distanceM: number,
  far: boolean,
): { readonly worst: number; readonly datum: number } {
  const a = figure.equatorialRadiusM;
  const k = a / figure.polarRadiusM;
  let worst = 0;
  let datum = 0;
  for (const latitudeDeg of [0, 40]) {
    const phi = (latitudeDeg * Math.PI) / 180;
    const p = { x: distanceM * Math.cos(phi), y: 0, z: distanceM * Math.sin(phi) };
    // Scaled space, where the figure is a sphere of radius a.
    const sp = { x: p.x, y: p.y, z: p.z * k };
    const r = Math.hypot(sp.x, sp.y, sp.z);
    const toward = { x: -sp.x / r, y: -sp.y / r, z: -sp.z / r };
    for (const w of [
      { x: -Math.sin(phi), y: 0, z: Math.cos(phi) },
      { x: 0, y: 1, z: 0 },
    ]) {
      for (const tangentM of [
        -0.3 * a,
        -50_000,
        -2_000,
        0,
        500,
        2_000,
        10_000,
        30_000,
        60_000,
        100_000,
      ]) {
        const sinGamma = (a + tangentM) / r;
        const cosGamma = Math.sqrt(1 - sinGamma * sinGamma);
        const ds = {
          x: cosGamma * toward.x + sinGamma * w.x,
          y: cosGamma * toward.y + sinGamma * w.y,
          z: cosGamma * toward.z + sinGamma * w.z,
        };
        const d = { x: ds.x, y: ds.y, z: ds.z / k };
        const length = Math.hypot(d.x, d.y, d.z);
        const dir = vec(d.x / length, d.y / length, d.z / length);
        for (const samples of [32, 256]) {
          for (const sample of marchHeights(p, dir, figure, far, samples)) {
            const bound =
              4 * EPSILON * Math.max(a, distanceM) + 1e-6 * Math.max(sample.fromStar, 1_000);
            worst = Math.max(worst, Math.abs(sample.h) / bound);
            datum = Math.max(datum, Math.abs(sample.datum) / bound);
          }
        }
      }
    }
  }
  return { worst, datum };
}

describe.each(FIGURES)(
  "the ray march's quadric (rayMarch.wgsl) on %s, the camera narrowed and q_g, q_t from f64",
  (_name, figure) => {
    // One figure's near-form figures, computed once for its own tests.
    const near = nearReport(figure);

    it("keeps every radial height from −400 m to 400 km within 10⁻⁶ · max(t, |h|, 1 km)", () => {
      expect(near.worst).toBeLessThanOrEqual(1);
    });

    it("keeps every height on the datum from −400 m to 400 km within 10⁻⁶ · max(t, |h|, 1 km) of its own form (R08.T6.f)", () => {
      expect(near.datum).toBeLessThanOrEqual(1);
    });

    it("finds R05's heightAbove over that bound", () => {
      expect(near.r05).toBeGreaterThan(1);
    });

    it.each([1.5, 6.6, 60, 0])(
      "keeps every height near the limb and across the disc from %d a (0: 10⁹ m) within 4ε max(a, |P|) + 10⁻⁶ max(|t − t*|, 1 km)",
      (multiple) => {
        const distanceM = multiple > 0 ? multiple * figure.equatorialRadiusM : 1e9;
        const report = farReport(figure, distanceM, true);
        expect(report.worst).toBeLessThanOrEqual(1);
        // The datum heights the oblate march reads (R08.T6.f), within the same bound.
        expect(report.datum).toBeLessThanOrEqual(1);
      },
    );

    it("keeps the near form within the same bound at 1.5a, where h₀ = a ÷ 2 takes it", () => {
      expect(farReport(figure, 1.5 * figure.equatorialRadiusM, false).worst).toBeLessThanOrEqual(1);
    });
  },
);

// --- The walk through a segment's pieces (common.wgsl's MarchWalk, R08.T6.i) ----------------------

/** One step as `marchStep` and `marchWalkStep` return it, in f32: t, Δt and t − t*. */
interface WalkStep {
  readonly tM: number;
  readonly dtM: number;
  readonly fromLowestM: number;
}

/** `common.wgsl`'s `marchSplit`, emulated, as `marchSteps` above forms it. */
function splitOf(tStart: number, tEnd: number, nearest: number, samples: number, camera: boolean) {
  const lowest = Math.min(Math.max(nearest, tStart), tEnd);
  const before = f(lowest - tStart);
  const after = f(tEnd - lowest);
  const rootBefore = f(Math.sqrt(before));
  const total = f(rootBefore + f(Math.sqrt(after)));
  const share = total > 0 ? Math.floor(f(f(f(samples * rootBefore) / total) + 0.5)) : 0;
  const stepsBefore = Math.min(Math.max(share, before > 0 ? 1 : 0), samples - (after > 0 ? 1 : 0));
  return {
    start: tStart,
    lowest,
    before,
    after,
    stepsBefore,
    stepsAfter: samples - stepsBefore,
    towardStart: camera && before > 0 && after > 0,
  };
}

type Split = ReturnType<typeof splitOf>;

/** `common.wgsl`'s `marchStep`, emulated. */
function stepOf(split: Split, i: number): WalkStep {
  if (i < split.stepsBefore) {
    const n = split.stepsBefore;
    const unit = f(split.before / f(n * n));
    if (split.towardStart) {
      const fromStart = f(unit * f(f(f(i * i) + i) + 0.5));
      return {
        tM: f(split.start + fromStart),
        dtM: f(unit * f(2 * i + 1)),
        fromLowestM: f(fromStart - split.before),
      };
    }
    const k = n - 1 - i;
    const fromLowest = f(unit * f(f(f(k * k) + k) + 0.5));
    return {
      tM: f(split.lowest - fromLowest),
      dtM: f(unit * f(2 * k + 1)),
      fromLowestM: -fromLowest,
    };
  }
  const n = split.stepsAfter;
  const k = i - split.stepsBefore;
  const unit = f(split.after / f(n * n));
  const fromLowest = f(unit * f(f(f(k * k) + k) + 0.5));
  return { tM: f(split.lowest + fromLowest), dtM: f(unit * f(2 * k + 1)), fromLowestM: fromLowest };
}

/**
 * `common.wgsl`'s `marchWalkCross`, `marchWalkReady`, `marchSideShares`, `marchWalkEnter` and
 * `marchWalkStep`, emulated in f32 in the WGSL's order, over the whole segment.
 */
function walkSteps(split: Split, crossings: readonly number[]): WalkStep[] {
  const fromLowest: number[] = [];
  for (const raw of crossings) {
    const t = f(raw);
    if ((t > -split.before && t < 0) || (t > 0 && t < split.after)) {
      let at = fromLowest.length;
      fromLowest.push(t);
      while (at > 0 && (fromLowest[at - 1] ?? 0) > t) {
        fromLowest[at] = fromLowest[at - 1] ?? 0;
        at -= 1;
      }
      fromLowest[at] = t;
    }
  }
  const ends = fromLowest.filter((t, j) => j === 0 || t > (fromLowest[j - 1] ?? 0));
  const crossingsBefore = ends.filter((t) => t < 0).length;
  const shares: number[] = [];
  const sideShares = (
    first: number,
    count: number,
    start: number,
    end: number,
    steps: number,
    at: number,
  ): boolean => {
    const pieces = count + 1;
    if (count === 0 || pieces > steps) {
      return false;
    }
    let total = 0;
    let from = start;
    for (let p = 0; p < pieces; p += 1) {
      const to = p < count ? (ends[first + p] ?? 0) : end;
      const root = f(Math.sqrt(f(to - from)));
      shares[at + p] = root;
      total = f(total + root);
      from = to;
    }
    const rest = steps - pieces;
    let spent = 0;
    let cumulative = 0;
    for (let p = 0; p < pieces; p += 1) {
      cumulative = f(cumulative + (shares[at + p] ?? 0));
      const share = p + 1 < pieces ? Math.floor(f(f(f(rest * cumulative) / total) + 0.5)) : rest;
      shares[at + p] = 1 + share - spent;
      spent = share;
    }
    return true;
  };
  const firstAfter = crossingsBefore + 1;
  const piecedBefore = sideShares(0, crossingsBefore, -split.before, 0, split.stepsBefore, 0);
  const piecedAfter = sideShares(
    crossingsBefore,
    ends.length - crossingsBefore,
    0,
    split.after,
    split.stepsAfter,
    firstAfter,
  );
  // The walk's place, as `MarchWalk` holds it: its piece, the steps taken there, and the piece's
  // count, unit and anchor (`marchWalkEnter`).
  const walk = { piece: 0, taken: 0, n: 0, unit: 0, anchor: 0, towardLow: true };
  const enter = (to: number, before: boolean): void => {
    let low: number;
    let high: number;
    if (before) {
      low = to > 0 ? (ends[to - 1] ?? 0) : -split.before;
      high = to < crossingsBefore ? (ends[to] ?? 0) : 0;
    } else {
      const at = crossingsBefore + to - firstAfter;
      low = at > crossingsBefore ? (ends[at - 1] ?? 0) : 0;
      high = at < ends.length ? (ends[at] ?? 0) : split.after;
    }
    walk.n = shares[to] ?? 0;
    walk.piece = to;
    walk.taken = 0;
    walk.towardLow = !before || split.towardStart;
    walk.unit = f(f(high - low) / f(walk.n * walk.n));
    walk.anchor = walk.towardLow ? low : high;
  };
  if (piecedBefore) {
    enter(0, true);
  }
  const steps: WalkStep[] = [];
  for (let i = 0; i < split.stepsBefore + split.stepsAfter; i += 1) {
    const before = i < split.stepsBefore;
    if ((before && !piecedBefore) || (!before && !piecedAfter)) {
      steps.push(stepOf(split, i));
      continue;
    }
    if (!before && walk.piece < firstAfter) {
      enter(firstAfter, false);
    } else if (walk.taken >= walk.n) {
      enter(walk.piece + 1, before);
    }
    const j = walk.taken;
    walk.taken += 1;
    const k = walk.towardLow ? j : walk.n - 1 - j;
    const q = f(f(f(k * k) + k) + 0.5);
    const fromLowestM = walk.towardLow
      ? f(walk.anchor + f(walk.unit * q))
      : f(walk.anchor - f(walk.unit * q));
    steps.push({
      tM: f(split.lowest + fromLowestM),
      dtM: f(walk.unit * f(2 * k + 1)),
      fromLowestM,
    });
  }
  return steps;
}

describe("the walk through a segment's pieces (common.wgsl's MarchWalk, R08.T6.i)", () => {
  /** Segments of every kind: from orbit, from a camera in the air, upward, down to the ground. */
  const SEGMENTS = [
    [300_000, 2_600_000, 1_450_000, false],
    [0, 1_200_000, 79_000, true],
    [0, 400_000, -5, true],
    [0, 90_000, 120_000, true],
    [12, 4_000, 9_000, false],
  ] as const;

  it("places every step as marchPiecesSteps does, within f32's rounding of t", () => {
    let compared = 0;
    for (const [tStart, tEnd, nearest, camera] of SEGMENTS) {
      for (const samples of [16, 32, 75]) {
        const split = splitOf(f(tStart), f(tEnd), f(nearest), samples, camera);
        const twinSplit = marchSplit(f(tStart), f(tEnd), f(nearest), samples, camera);
        // Crossings on either side, near its ends and near t*, as offsets from t*.
        const length = split.before + split.after;
        for (const cut of [
          [],
          [0.3, 0.7],
          [-0.2, 0.05, 0.6],
          [-0.9, -0.5, -0.01, 0.01, 0.4, 0.95],
          [0.001, 0.002, 0.003],
        ]) {
          const crossings = cut.map((x) => x * length);
          const emulated = walkSteps(split, crossings);
          const twin = marchPiecesSteps(marchPieces(twinSplit, crossings.map(f)));
          expect(emulated).toHaveLength(twin.length);
          for (const [i, step] of emulated.entries()) {
            const expected = twin[i];
            const scaleM = Math.max(Math.abs(tStart), Math.abs(tEnd), 1);
            expect(Math.abs(step.tM - (expected?.tM ?? Number.NaN))).toBeLessThan(4e-7 * scaleM);
            expect(Math.abs(step.dtM - (expected?.dtM ?? Number.NaN))).toBeLessThan(4e-7 * scaleM);
            compared += 1;
          }
        }
      }
    }
    expect(compared).toBeGreaterThan(2_000);
  });

  it("keeps R05.T12.e's marchStep to the bit on a segment that crosses no edge", () => {
    for (const [tStart, tEnd, nearest, camera] of SEGMENTS) {
      const split = splitOf(f(tStart), f(tEnd), f(nearest), 32, camera);
      const placed = Array.from({ length: 32 }, (_, i) => stepOf(split, i));
      expect(walkSteps(split, [])).toEqual(placed);
    }
  });
});

// --- The ray march's layer-edge crossings (rayMarch.wgsl, R08.T6.i) --------------------------------

/** A point's radial height above the spheroid (a + e, a + e, c + e) in f64: the edge's shell. */
function shellHeight64(p: Vec3, figure: SpheroidFigure, edgeM: number): number {
  return radialHeight64(p, {
    equatorialRadiusM: figure.equatorialRadiusM + edgeM,
    polarRadiusM: figure.polarRadiusM + edgeM,
  });
}

/**
 * `rayMarch.wgsl`'s `marchWalkCrossShell` along one ray, emulated, with the split and the
 * closest-approach terms as `main` forms them: each crossing of the edge's shell quadric inside the
 * segment, where the walk puts its piece boundary (at t_low plus its offset within h₀ = a ÷ 2, and
 * at t* plus its offset from it beyond), as its height off the shell in f64 and |t − t*|.
 */
function shellCrossingHeights(
  camera: Vec3,
  dir: Vec3,
  figure: SpheroidFigure,
  far: boolean,
  edgeHeightM: number,
): Array<{ readonly t: number; readonly h: number; readonly fromStar: number }> {
  const exact = viewExactOf(camera, figure, BARE_MEDIUM, bareInputsOf(figure));
  const origin = vec(camera.x, camera.y, camera.z);
  const a = f(figure.equatorialRadiusM);
  const c = f(figure.polarRadiusM);
  const topM = f(ATMOSPHERE_M);
  const k = f(a / c);
  const shellQ = rayQuadric(
    origin,
    dir,
    f(f(a + topM) / f(c + topM)),
    f(a + topM),
    f(exact.topQuadricM2),
    far,
  );
  const groundQc = f(exact.groundQuadricM2);
  const groundQ = rayQuadric(origin, dir, k, a, groundQc, far);
  const shell = quadricHits(shellQ);
  const ground = quadricHits(groundQ);
  const tStart = Math.max(shell[0], 0);
  let tEnd = shell[1];
  if (ground[0] > 0) {
    tEnd = Math.min(tEnd, ground[0]);
  }
  if (shell[1] <= 0 || tEnd <= tStart) {
    return [];
  }
  const nearest = f(-dotV(origin, dir));
  const lowest = Math.min(Math.max(nearest, tStart), tEnd);
  const before = f(lowest - tStart);
  const after = f(tEnd - lowest);
  const acrossZ = crossV(dir, crossV(origin, dir)).z;
  const nearestFromStar = f(f(f(f(f(k - 1) * f(k + 1)) * dir.z) * acrossZ) / groundQ.qa);
  let lowestFromStar = f(f(lowest - nearest) + nearestFromStar);
  if (ground[0] > 0 && lowest === ground[0]) {
    lowestFromStar = f(-f(Math.sqrt(Math.max(groundQ.disc, 0))) / groundQ.qa);
  }
  const edgeM = f(edgeHeightM);
  const kE = f(f(a + edgeM) / f(c + edgeM));
  let offsets: number[];
  if (far) {
    const q = rayQuadric(origin, dir, kE, f(a + edgeM), 0, true);
    if (q.disc < 0) {
      return [];
    }
    const halfChord = f(f(Math.sqrt(q.disc)) / q.qa);
    const edgeFromStar = f(f(f(f(f(kE - 1) * f(kE + 1)) * dir.z) * acrossZ) / q.qa);
    const centre = f(f(nearestFromStar - edgeFromStar) - lowestFromStar);
    offsets = [f(centre - halfChord), f(centre + halfChord)];
  } else {
    const kGap = f(f(edgeM * f(c - a)) / f(c * f(c + edgeM)));
    const z2 = f(origin.z * origin.z);
    const qc = f(f(groundQc + f(f(kGap * f(kE + k)) * z2)) - f(edgeM * f(f(2 * a) + edgeM)));
    const q = rayQuadric(origin, dir, kE, f(a + edgeM), qc, false);
    if (q.disc < 0) {
      return [];
    }
    offsets = quadricHits(q).map((t) => f(t - lowest));
  }
  const kTrue = figure.equatorialRadiusM / figure.polarRadiusM;
  const sdx = dir.z * kTrue;
  const tStar =
    -(camera.x * dir.x + camera.y * dir.y + camera.z * kTrue * sdx) /
    (dir.x * dir.x + dir.y * dir.y + sdx * sdx);
  return offsets
    .filter((o) => (o > -before && o < 0) || (o > 0 && o < after))
    .map((offset) => {
      // Where the walk's boundary stands: the step's own t near the body, its offset from t*
      // beyond it, where the step's height is taken from that offset alone.
      const t = far ? tStar + f(offset + lowestFromStar) : f(lowest + offset);
      const p = { x: camera.x + t * dir.x, y: camera.y + t * dir.y, z: camera.z + t * dir.z };
      return { t, h: shellHeight64(p, figure, edgeHeightM), fromStar: Math.abs(t - tStar) };
    });
}

/**
 * The near form's worst crossing height as a fraction of 10⁻⁶ · max(t, e, 1 km), and the count,
 * over `nearReport`'s cameras, zeniths and azimuths.
 */
function nearShellReport(figure: SpheroidFigure): {
  readonly worst: number;
  readonly count: number;
} {
  let worst = 0;
  let count = 0;
  const a = figure.equatorialRadiusM;
  for (const latitudeDeg of [0, 35, 70, 89.5]) {
    // The cameras include each edge's own height, ±1 m, where q_g − e(2a + e) cancels most: a
    // descent through a layer's base.
    const heights = [-400, 1, 2, 20, 200, 2_000, 20_000, 400_000, ...EDGES_M.flatMap(nearEdgeM)];
    for (const heightM of heights) {
      const frame = localFrame(figure, latitudeDeg, heightM);
      const dip = Math.acos(a / (a + Math.max(heightM, 1)));
      const zeniths = [0, 30, 60, 80, 85, 88, 89, 89.5, 90, 90.3, 91, 93, 100, 120, 150, 180].map(
        (deg) => (deg * Math.PI) / 180,
      );
      for (const s of [0.5, 0.9, 0.99, 1.01, 1.1, 2]) {
        zeniths.push(Math.PI / 2 + dip * s);
      }
      for (const zenith of zeniths) {
        for (const azimuthDeg of [0, 45, 90]) {
          const dir = directionOf(frame, zenith, (azimuthDeg * Math.PI) / 180);
          for (const edgeM of EDGES_M) {
            for (const crossing of shellCrossingHeights(frame.p, dir, figure, false, edgeM)) {
              count += 1;
              const bound = 1e-6 * Math.max(crossing.t, edgeM, 1_000);
              worst = Math.max(worst, Math.abs(crossing.h) / bound);
            }
          }
        }
      }
    }
  }
  return { worst, count };
}

/**
 * The closest-approach form's worst crossing height as a fraction of
 * 4ε max(a, |P|) + 10⁻⁶ max(|t − t*|, 1 km), and the count, from `distanceM`, over `farReport`'s
 * rays and rays that graze each edge's shell.
 */
function farShellReport(
  figure: SpheroidFigure,
  distanceM: number,
): { readonly worst: number; readonly count: number } {
  const a = figure.equatorialRadiusM;
  const k = a / figure.polarRadiusM;
  let worst = 0;
  let count = 0;
  for (const latitudeDeg of [0, 40]) {
    const phi = (latitudeDeg * Math.PI) / 180;
    const p = { x: distanceM * Math.cos(phi), y: 0, z: distanceM * Math.sin(phi) };
    const sp = { x: p.x, y: p.y, z: p.z * k };
    const r = Math.hypot(sp.x, sp.y, sp.z);
    const toward = { x: -sp.x / r, y: -sp.y / r, z: -sp.z / r };
    const tangents = [-0.3 * a, -50_000, -2_000, 0, 500, 2_000, 10_000, 30_000, 60_000];
    for (const e of EDGES_M) {
      tangents.push(...[-100, -1, -0.01, 0.01, 1, 100].map((d) => e + d));
    }
    for (const w of [
      { x: -Math.sin(phi), y: 0, z: Math.cos(phi) },
      { x: 0, y: 1, z: 0 },
    ]) {
      for (const tangentM of tangents) {
        const sinGamma = (a + tangentM) / r;
        const cosGamma = Math.sqrt(1 - sinGamma * sinGamma);
        const ds = {
          x: cosGamma * toward.x + sinGamma * w.x,
          y: cosGamma * toward.y + sinGamma * w.y,
          z: cosGamma * toward.z + sinGamma * w.z,
        };
        const d = { x: ds.x, y: ds.y, z: ds.z / k };
        const length = Math.hypot(d.x, d.y, d.z);
        const dir = vec(d.x / length, d.y / length, d.z / length);
        for (const edgeM of EDGES_M) {
          for (const crossing of shellCrossingHeights(p, dir, figure, true, edgeM)) {
            count += 1;
            const bound =
              4 * EPSILON * Math.max(a, distanceM) + 1e-6 * Math.max(crossing.fromStar, 1_000);
            worst = Math.max(worst, Math.abs(crossing.h) / bound);
          }
        }
      }
    }
  }
  return { worst, count };
}

describe.each(FIGURES)(
  "the ray march's layer-edge crossings on their shell quadrics (rayMarch.wgsl, R08.T6.i) on %s",
  (_name, figure) => {
    it("put each piece boundary within 10⁻⁶ · max(t, e, 1 km) of its shell from −400 m to 400 km", () => {
      // Measured 0.26–0.41 of the bound, over 25,116–26,107 crossings a figure (2026-10-13).
      const report = nearShellReport(figure);
      expect(report.count).toBeGreaterThan(20_000);
      expect(report.worst).toBeLessThanOrEqual(1);
    });

    it.each([1.5, 6.6, 60, 0])(
      "put each piece boundary from %d a (0: 10⁹ m) within 4ε max(a, |P|) + 10⁻⁶ max(|t − t*|, 1 km) of its shell",
      (multiple) => {
        const distanceM = multiple > 0 ? multiple * figure.equatorialRadiusM : 1e9;
        // Measured 0.10–0.59 of the bound, over 926–1,272 crossings each (2026-10-13).
        const report = farShellReport(figure, distanceM);
        expect(report.count).toBeGreaterThan(800);
        expect(report.worst).toBeLessThanOrEqual(1);
      },
    );
  },
);

// --- The precise forms -----------------------------------------------------------------------------

describe("the precise forms (view/shaders/preciseTrig.wgsl)", () => {
  it("give cos θ and sin θ within 5 × 10⁻⁷ of f64 over [0, π]", () => {
    let worst = 0;
    for (let i = 0; i <= 40_000; i += 1) {
      const theta = f((Math.PI * i) / 40_000);
      const [sin, cos] = preciseSinCos(theta);
      worst = Math.max(worst, Math.abs(cos - Math.cos(theta)), Math.abs(sin - Math.sin(theta)));
    }
    expect(worst).toBeLessThan(5e-7);
  });

  it("give atan x within 5 × 10⁻⁷ of f64 for x from 0 to 10⁸", () => {
    const xs = [
      0,
      ...Array.from({ length: 4_001 }, (_, i) => 10 ** (-8 + (16 * i) / 4_000)),
      ...Array.from({ length: 4_001 }, (_, i) => (2 * i) / 4_000),
    ].map(f);
    const worst = Math.max(...xs.map((x) => Math.abs(preciseAtan(x) - Math.atan(x))));
    expect(worst).toBeLessThan(5e-7);
  });

  it("give atan2(y, x) within 5 × 10⁻⁷ of f64 over the half-plane y ≥ 0", () => {
    let worst = 0;
    for (const radius of [1e-6, 1, 6.4e6]) {
      for (let i = 0; i <= 20_000; i += 1) {
        const angle = (Math.PI * i) / 20_000;
        const y = f(radius * Math.sin(angle));
        const x = f(radius * Math.cos(angle));
        if (y > 0 || x !== 0) {
          worst = Math.max(
            worst,
            Math.abs(preciseAtan2(Math.max(y, 0), x) - Math.atan2(Math.max(y, 0), x)),
          );
        }
      }
    }
    expect(worst).toBeLessThan(5e-7);
  });
});

// --- The texel mapping -----------------------------------------------------------------------------

/** Every axis the pair maps: transmittance, multiple scattering, the sky views and the smoke's. */
const MAPPED_AXES = [256, 64, 32, 192, 108, 128, 96, 54] as const;

describe("Bruneton's texel mapping (common.wgsl, Design note 19)", () => {
  it.each(MAPPED_AXES)("returns every texel centre of an axis of %d to 10⁻³ of a texel", (n) => {
    let worst = 0;
    for (let i = 0; i < n; i += 1) {
      const unit = unitFromTexelUv(texelCentre(i, n), n);
      worst = Math.max(worst, Math.abs(filterPosition(texelUvFromUnit(unit, n), n) - i));
    }
    expect(worst).toBeLessThan(1e-3);
  });

  it.each(MAPPED_AXES)("gives 0 and 1 at the first and last of %d texels, to 10⁻⁶", (n) => {
    expect(Math.abs(unitFromTexelUv(texelCentre(0, n), n))).toBeLessThan(1e-6);
    expect(Math.abs(unitFromTexelUv(texelCentre(n - 1, n), n) - 1)).toBeLessThan(1e-6);
  });

  it.each(MAPPED_AXES)(
    "R05's read, sebh's, misses by (x − ½) ÷ (n + 1), over 10⁻³ of a texel at both ends of %d",
    (n) => {
      let formula = 0;
      for (let i = 0; i < n; i += 1) {
        // The miss itself, exact in f64; f32's own rounding of a position, about 10⁻⁵ of a texel
        // at these sizes, is the pair's whole error.
        const x = i / (n - 1);
        const miss64 = (x + 0.5 / n) * (n / (n + 1)) * n - 0.5 - i;
        formula = Math.max(formula, Math.abs(miss64 - (x - 0.5) / (n + 1)));
      }
      expect(formula).toBeLessThan(1e-9);
      const ends = [0, n - 1].map((i) =>
        Math.abs(filterPosition(sebhUvFromUnit(unitFromTexelUv(texelCentre(i, n), n), n), n) - i),
      );
      expect(Math.min(...ends)).toBeGreaterThan(1e-3);
    },
  );

  it.each(SPHERES_M)(
    "takes every transmittance texel on a %d m sphere through its (h, μ) back to itself",
    (radiusM) => {
      const bottom = f(radiusM);
      const top = f(ATMOSPHERE_M);
      const { widthTexels: w, heightTexels: h } = TRANSMITTANCE_SIZE;
      let worst = 0;
      for (let j = 0; j < h; j += 1) {
        for (let i = 0; i < w; i += 1) {
          const unit = [
            unitClamp(unitFromTexelUv(texelCentre(i, w), w)),
            unitClamp(unitFromTexelUv(texelCentre(j, h), h)),
          ] as const;
          const texel = transmittanceTexelOf(bottom, top, unit);
          const [u, v] = transmittanceUnitAt(bottom, top, texel.heightM, texel.mu);
          worst = Math.max(
            worst,
            Math.abs(filterPosition(texelUvFromUnit(unitClamp(u), w), w) - i),
            Math.abs(filterPosition(texelUvFromUnit(unitClamp(v), h), h) - j),
          );
        }
      }
      expect(worst).toBeLessThan(1e-3);
    },
  );

  it("takes every multiple-scattering texel through its (h, μ_s) back to itself", () => {
    const { widthTexels: w, heightTexels: h } = MULTI_SCATTERING_SIZE;
    let worst = 0;
    for (const topM of [ATMOSPHERE_M, 1_000_000]) {
      const top = f(topM);
      for (let j = 0; j < h; j += 1) {
        for (let i = 0; i < w; i += 1) {
          const ux = unitClamp(unitFromTexelUv(texelCentre(i, w), w));
          const uy = unitClamp(unitFromTexelUv(texelCentre(j, h), h));
          const heightM = f(10 + f(uy * f(top - 10)));
          const muSun = f(f(ux * 2) - 1);
          const [x, y] = multiScatteringUnitAt(top, heightM, muSun);
          worst = Math.max(
            worst,
            Math.abs(filterPosition(texelUvFromUnit(x, w), w) - i),
            Math.abs(filterPosition(texelUvFromUnit(y, h), h) - j),
          );
        }
      }
    }
    expect(worst).toBeLessThan(1e-3);
  });

  it("takes the aerial volume's froxels through the composite's read back to themselves", () => {
    let worst = 0;
    const n = TABLE_SIZES.high.aerialPerspective.widthTexels;
    for (let i = 0; i < n; i += 1) {
      const screen = unitFromTexelUv(texelCentre(i, n), n);
      worst = Math.max(worst, Math.abs(filterPosition(texelUvFromUnit(screen, n), n) - i));
    }
    for (const setting of ["high", "low"] as const) {
      const sizes = TABLE_SIZES[setting];
      const slices = sizes.aerialPerspective.slices;
      const reach = f(sizes.aerialPerspectiveReachM);
      for (let k = 0; k < slices; k += 1) {
        // Slice k holds the march to (k + 1) reach ÷ slices; the composite reads it there at
        // w = distance ÷ reach − ½ ÷ slices.
        const distance = f(f((k + 1) * reach) / slices);
        const w = unitClamp(f(f(distance / reach) - f(0.5 / slices)));
        worst = Math.max(worst, Math.abs(filterPosition(w, slices) - k));
      }
    }
    expect(worst).toBeLessThan(1e-3);
  });
});

// --- The sky view's write and read (view.wgsl) -----------------------------------------------------

const PI_VIEW = f(Math.PI);

/**
 * `skyViewUvToParams`: the ray in the sky frame, its zenith's sine and cosine and its local
 * azimuth's, φ = 2π u over the whole circle (R08.T7).
 */
function skyViewUvToParams(
  uv: readonly [number, number],
  size: readonly [number, number],
  beta: number,
): Vec3 {
  const ux = unitClamp(unitFromTexelUv(uv[0], size[0]));
  const uy = unitClamp(unitFromTexelUv(uv[1], size[1]));
  const zenithHorizon = f(PI_VIEW - beta);
  let zenith: number;
  if (uy < 0.5) {
    let c = f(1 - f(2 * uy));
    c = f(1 - f(c * c));
    zenith = f(zenithHorizon * c);
  } else {
    const c = f(f(uy * 2) - 1);
    zenith = f(zenithHorizon + f(f(beta * c) * c));
  }
  const [sin, cos] = preciseSinCos(zenith);
  const phi = f(f(2 * PI_VIEW) * ux);
  let azimuthSin: number;
  let azimuthCos: number;
  if (phi <= PI_VIEW) {
    [azimuthSin, azimuthCos] = preciseSinCos(phi);
  } else {
    const [mirroredSin, mirroredCos] = preciseSinCos(f(f(2 * PI_VIEW) - phi));
    azimuthSin = -mirroredSin;
    azimuthCos = mirroredCos;
  }
  return { x: f(sin * azimuthCos), y: f(sin * azimuthSin), z: cos };
}

/** The composite's read of a ray in the sky frame, as `fragmentMain` and `skyViewParamsToUv`. */
function skyViewRead(
  dir: Vec3,
  size: readonly [number, number],
  beta: number,
  bottom: number,
  heightM: number,
): readonly [number, number] | null {
  const up = { x: 0, y: 0, z: 1 };
  const viewZenithCos = dotV(dir, up);
  const viewZenith = preciseAtan2(lengthV(crossV(dir, up)), viewZenithCos);
  const h2 = f(f(dir.x * dir.x) + f(dir.y * dir.y));
  if (!(h2 > 1e-12)) {
    return null;
  }
  // `skyViewAzimuthUnit`: preciseAtan2(|y|, x), mirrored where y < 0, over 2π.
  const a = preciseAtan2(Math.abs(dir.y), dir.x);
  const azimuthUnit = f((dir.y < 0 ? f(f(2 * PI_VIEW) - a) : a) / f(2 * PI_VIEW));
  const ground = groundHit(bottom, heightM, viewZenithCos);
  const zenithHorizon = f(PI_VIEW - beta);
  let v: number;
  if (ground) {
    const c = f(Math.sqrt(Math.max(f(f(viewZenith - zenithHorizon) / Math.max(beta, 1e-6)), 0)));
    v = f(f(c * 0.5) + 0.5);
  } else {
    const c = f(1 - f(Math.sqrt(Math.max(f(1 - f(viewZenith / zenithHorizon)), 0))));
    v = f(c * 0.5);
  }
  return [
    filterPosition(texelUvFromUnit(unitClamp(azimuthUnit), size[0]), size[0]),
    filterPosition(texelUvFromUnit(unitClamp(v), size[1]), size[1]),
  ];
}

describe("a point's height on the datum from its ground quadric (oblate.wgsl, R08.T6.f)", () => {
  /** The worst |root − geodetic| over latitudes 0–90° at a geodetic height, m, and of latitude. */
  function rootReport(figure: SpheroidFigure, heightM: number): readonly [number, number] {
    let height = 0;
    let latitude = 0;
    for (let latitudeDeg = 0; latitudeDeg <= 90; latitudeDeg += 5) {
      const { p } = localFrame(figure, latitudeDeg, heightM);
      const geodetic = geodeticOf(p, figure);
      height = Math.max(height, Math.abs(datumHeight64(p, figure) - geodetic.heightM));
      // The root's foot's normal, whose z is the sine of the latitude the kernels take.
      const a = figure.equatorialRadiusM;
      const k2 = (a / figure.polarRadiusM) ** 2;
      const g = { x: p.x / a, y: p.y / a, z: (p.z * k2) / a };
      const u = Math.hypot(g.x, g.y, g.z);
      const h = datumHeight64(p, figure);
      const foot = { x: p.x - (h * g.x) / u, y: p.y - (h * g.y) / u, z: p.z - (h * g.z) / u };
      const n = { x: foot.x, y: foot.y, z: foot.z * k2 };
      const sin = n.z / Math.hypot(n.x, n.y, n.z);
      latitude = Math.max(latitude, Math.abs(Math.asin(sin) - geodetic.latitudeRad));
    }
    return [height, latitude];
  }

  it("is within 2 × 10⁻⁶ of the geodetic height to 1,000 km on a Saturn-class figure, 1.7 mm at 100 km", () => {
    const saturn = SATURN_CLASS;
    // Measured (f64, against `geodeticOf`'s Vermeille): 1.6 × 10⁻⁶ of h at 1,000 km, 2.6 × 10⁻⁷ at
    // 400 km, 1.63 mm at 100 km; its foot's latitude within 3.2 × 10⁻⁵ rad at 1,000 km and 3.2 ×
    // 10⁻⁷ at 100 km, which moves s by under 9 × 10⁻⁶ (8.8 × 10⁻⁶ at 45° from 1,000 km).
    expect(rootReport(saturn, 1_000_000)[0]).toBeLessThan(2e-6 * 1_000_000);
    expect(rootReport(saturn, 100_000)[0]).toBeLessThan(1.7e-3);
    expect(rootReport(saturn, 1_000_000)[1]).toBeLessThan(3.5e-5);
    expect(rootReport(saturn, 100_000)[1]).toBeLessThan(3.5e-7);
  });

  it("is the height itself on a sphere and on WGS 84 near the ground", () => {
    for (const [, figure] of FIGURES) {
      expect(rootReport(figure, 2_000)[0]).toBeLessThan(1e-6);
    }
  });
});

/**
 * `oblate.wgsl`'s `skyViewColumnOf`, emulated in f32: R_α of M and N along the direction's
 * azimuth at the camera's normal `n`, and β = π ÷ 2 − preciseAtan(√(h (2 R_α + h)) ÷ R_α).
 */
function skyViewColumn(
  meridionalM: number,
  primeVerticalM: number,
  n: Vec3,
  d: Vec3,
  heightM: number,
): readonly [number, number] {
  const mu = dotV(d, n);
  const flat = vec(f(d.x - f(mu * n.x)), f(d.y - f(mu * n.y)), f(d.z - f(mu * n.z)));
  const cos2 = f(f(n.x * n.x) + f(n.y * n.y));
  const cos = f(Math.sqrt(cos2));
  const across = f(f(f(flat.y * n.x) - f(flat.x * n.y)) / cos);
  const along = f(f(f(flat.z * cos2) - f(n.z * f(f(flat.x * n.x) + f(flat.y * n.y)))) / cos);
  const horizontal = f(f(along * along) + f(across * across));
  const gap = f(f(primeVerticalM - meridionalM) / primeVerticalM);
  const radiusM = f(meridionalM / f(1 - f(gap * f(f(across * across) / horizontal))));
  const horizon = f(Math.sqrt(Math.max(f(heightM * f(f(2 * radiusM) + heightM)), 0)));
  const beta = f(TRIG_HALF_PI - preciseAtan(f(horizon / radiusM)));
  return [radiusM, beta];
}

describe("the sky view's column spheres (oblate.wgsl, R08.T6.f)", () => {
  /** A figure's principal radii M and N at latitude φ, m, and its normal there. */
  function cameraAt(
    figure: SpheroidFigure,
    latitudeDeg: number,
  ): { readonly m: number; readonly n: number; readonly up: Vec3; readonly north: Vec3 } {
    const a = figure.equatorialRadiusM;
    const e2 = 1 - (figure.polarRadiusM / a) ** 2;
    const phi = (latitudeDeg * Math.PI) / 180;
    const w2 = 1 - e2 * Math.sin(phi) ** 2;
    return {
      m: (a * (1 - e2)) / (w2 * Math.sqrt(w2)),
      n: a / Math.sqrt(w2),
      up: vec(Math.cos(phi), 0, Math.sin(phi)),
      north: vec(-Math.sin(phi), 0, Math.cos(phi)),
    };
  }

  const cases = [
    ["WGS 84", WGS84, 45],
    ["a Saturn-class figure", SATURN_CLASS, 45],
    ["a Saturn-class figure", SATURN_CLASS, 0],
  ] as const;

  it.each(cases)(
    "gives each column's sphere and β on %s at %d° to f32's precision, β within 5 × 10⁻⁷",
    (_name, figure, latitudeDeg) => {
      const { m, n, up, north } = cameraAt(figure, latitudeDeg);
      const east = crossV(up, north);
      for (const heightM of [1, 2_000, 99_000]) {
        for (let azimuthDeg = 0; azimuthDeg < 360; azimuthDeg += 15) {
          const alpha = (azimuthDeg * Math.PI) / 180;
          const d = vec(
            Math.cos(alpha) * north.x + Math.sin(alpha) * east.x,
            Math.cos(alpha) * north.y + Math.sin(alpha) * east.y,
            Math.cos(alpha) * north.z + Math.sin(alpha) * east.z,
          );
          const [radiusM, beta] = skyViewColumn(f(m), f(n), up, d, f(heightM));
          // Euler's theorem in f64, and β from it.
          const r64 = (m * n) / (n * Math.cos(alpha) ** 2 + m * Math.sin(alpha) ** 2);
          const beta64 = Math.atan2(r64, Math.sqrt(heightM * (2 * r64 + heightM)));
          expect(Math.abs(radiusM / r64 - 1)).toBeLessThan(4 * EPSILON);
          expect(Math.abs(beta - beta64)).toBeLessThan(5e-7);
        }
      }
    },
  );

  it.each(cases)(
    "inverts the sky view's write at every texel centre on %s at %d°, each column on its own sphere, to 10⁻³ of a texel",
    (_name, figure, latitudeDeg) => {
      const { m, n, up, north } = cameraAt(figure, latitudeDeg);
      // The sky frame: x the sky axis, here north, y = up × x, z up, as `skyFrame` takes it.
      const east = crossV(up, north);
      const size = [
        TABLE_SIZES.high.skyView.widthTexels,
        TABLE_SIZES.high.skyView.heightTexels,
      ] as const;
      const [w, h] = size;
      let worst = 0;
      for (const heightM of [1, 2_000, 99_000]) {
        const heightF = Math.min(Math.max(f(heightM), 1), f(f(ATMOSPHERE_M) - 1));
        for (let j = 1; j < h - 1; j += 1) {
          for (let i = 0; i < w; i += 1) {
            // The write: the column's azimuth (any zenith's ray shares it), its sphere and β, then
            // its ray.
            const azimuth = skyViewUvToParams([texelCentre(i, w), texelCentre(j, h)], size, 1);
            const horizontalBody = vec(
              f(f(azimuth.x * north.x) + f(azimuth.y * east.x)),
              f(f(azimuth.x * north.y) + f(azimuth.y * east.y)),
              f(f(azimuth.x * north.z) + f(azimuth.y * east.z)),
            );
            const [bottom, beta] = skyViewColumn(f(m), f(n), up, horizontalBody, heightF);
            const dir = skyViewUvToParams([texelCentre(i, w), texelCentre(j, h)], size, beta);
            // The read: the ray in body axes, its own column's sphere and β.
            const body = vec(
              f(f(f(dir.x * north.x) + f(dir.y * east.x)) + f(dir.z * up.x)),
              f(f(f(dir.x * north.y) + f(dir.y * east.y)) + f(dir.z * up.y)),
              f(f(f(dir.x * north.z) + f(dir.y * east.z)) + f(dir.z * up.z)),
            );
            const [readBottom, readBeta] = skyViewColumn(f(m), f(n), up, body, heightF);
            const read = skyViewRead(dir, size, readBeta, readBottom, heightF);
            if (read === null) {
              continue;
            }
            const [x, y] = read;
            const column = i === w - 1 && x < 0.5 ? x + (w - 1) : x;
            worst = Math.max(worst, Math.abs(y - j), Math.abs(column - i));
            expect(Math.abs(readBottom / bottom - 1)).toBeLessThan(1e-6);
          }
        }
      }
      expect(worst).toBeLessThan(1e-3);
    },
  );
});

describe("the sky view's read inverts its write (view.wgsl, Design notes 18 and 19)", () => {
  const sizes = [
    [TABLE_SIZES.high.skyView.widthTexels, TABLE_SIZES.high.skyView.heightTexels],
    [TABLE_SIZES.low.skyView.widthTexels, TABLE_SIZES.low.skyView.heightTexels],
  ] as const;
  const cases = sizes.flatMap((size) =>
    [6_371_008.8, 60_268_000].flatMap((radiusM) =>
      [1, 2_000, 99_000].map((heightM) => [size, radiusM, heightM] as const),
    ),
  );

  it.each(cases)(
    "at every texel centre of %j, on a %d m sphere from %d m, to 10⁻³ of a texel",
    (size, radiusM, heightM) => {
      const [w, h] = size;
      const bottom = f(radiusM);
      const skyHeight = skyViewHeight(heightM, ATMOSPHERE_M);
      const beta = f(Math.atan2(radiusM, Math.sqrt(skyHeight * (2 * radiusM + skyHeight))));
      const heightF = Math.min(Math.max(f(heightM), 1), f(f(ATMOSPHERE_M) - 1));
      let worst = 0;
      const unread: string[] = [];
      for (let j = 0; j < h; j += 1) {
        for (let i = 0; i < w; i += 1) {
          const dir = skyViewUvToParams([texelCentre(i, w), texelCentre(j, h)], size, beta);
          const read = skyViewRead(dir, size, beta, bottom, heightF);
          // The zenith and nadir rows hold one ray whatever the column: only the row is read.
          const vertical = j === 0 || j === h - 1;
          if (read === null) {
            unread.push(vertical ? "" : `(${String(i)}, ${String(j)})`);
          } else {
            const [x, y] = read;
            // The first and last columns are the one azimuth, the sky axis, read at either.
            const column = i === w - 1 && x < 0.5 ? x + (w - 1) : x;
            worst = Math.max(worst, Math.abs(y - j), vertical ? 0 : Math.abs(column - i));
          }
        }
      }
      expect(unread.filter((texel) => texel.length > 0)).toEqual([]);
      expect(worst).toBeLessThan(1e-3);
    },
  );
});
