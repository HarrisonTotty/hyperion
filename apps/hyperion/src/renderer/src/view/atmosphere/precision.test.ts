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
import { type SpheroidFigure, skyViewHeight, TABLE_SIZES, viewExactOf } from "./hillaire";
import { MULTI_SCATTERING_SIZE, TRANSMITTANCE_SIZE } from "./tables";

const f = Math.fround;

/** f32's unit roundoff, 2⁻²⁴. */
const EPSILON = 2 ** -24;

/** The atmosphere's height above the ground the checks take, m: R05's Earth's. */
const ATMOSPHERE_M = 100_000;

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

// --- view/shaders/preciseTrig.wgsl --------------------------------------------------------------

// The WGSL's literals, 3.14159265358979 and 1.57079632679490, round to the same f32 as π and π/2.
const TRIG_PI = f(Math.PI);
const TRIG_HALF_PI = f(Math.PI / 2);
const SIN_SERIES = [
  -1 / 6,
  1 / 120,
  -1 / 5040,
  1 / 362_880,
  -1 / 39_916_800,
  1 / 6_227_020_800,
  -1 / 1_307_674_368_000,
].map(f);
const COS_SERIES = [
  -1 / 2,
  1 / 24,
  -1 / 720,
  1 / 40_320,
  -1 / 3_628_800,
  1 / 479_001_600,
  -1 / 87_178_291_200,
].map(f);
const ATAN_SERIES = [-1 / 3, 1 / 5, -1 / 7, 1 / 9, -1 / 11, 1 / 13, -1 / 15].map(f);

/** 1 + x2 (c₁ + x2 (c₂ + … + x2 c₇)), innermost first, as the WGSL nests it. */
function series(x2: number, coefficients: readonly number[]): number {
  let acc = f(x2 * (coefficients.at(-1) ?? Number.NaN));
  for (let k = coefficients.length - 2; k >= 0; k -= 1) {
    acc = f(x2 * f((coefficients[k] ?? Number.NaN) + acc));
  }
  return f(1 + acc);
}

/** `preciseSinCos`: (sin θ, cos θ). */
function preciseSinCos(theta: number): readonly [number, number] {
  const y = f(0.5 * theta);
  const y2 = f(y * y);
  const s = f(y * series(y2, SIN_SERIES));
  const c = series(y2, COS_SERIES);
  return [f(f(2 * s) * c), f(f(c - s) * f(c + s))];
}

function preciseAtanUnit(x: number): number {
  let t = f(x / f(1 + f(Math.sqrt(f(1 + f(x * x))))));
  t = f(t / f(1 + f(Math.sqrt(f(1 + f(t * t))))));
  const t2 = f(t * t);
  return f(4 * f(t * series(t2, ATAN_SERIES)));
}

function preciseAtan(x: number): number {
  return x > 1 ? f(TRIG_HALF_PI - preciseAtanUnit(f(1 / x))) : preciseAtanUnit(x);
}

function preciseAtan2(y: number, x: number): number {
  const ax = Math.abs(x);
  const a = preciseAtanUnit(f(Math.min(y, ax) / Math.max(y, ax)));
  const b = y > ax ? f(TRIG_HALF_PI - a) : a;
  return x < 0 ? f(TRIG_PI - b) : b;
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

/** The layer edges the check takes, m: the rebuilt Earth's ozone's four and two more. */
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

    // The rebuilt Earth's narrowest ramp is 10 m, its ozone's top (`earth.ts`): the split's own
    // error, measured 0.012–0.017 m on these spheres, is a few hundredths of a metre in height.
    it("puts every crossing ahead within 0.05 m of its edge in height, and misses only grazes", () => {
      expect(report.crossings).toBeGreaterThan(2_500);
      expect(report.worstM).toBeLessThan(0.05);
      expect(report.missedM).toBeLessThan(0.05);
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
const FIGURES: ReadonlyArray<readonly [string, SpheroidFigure]> = [
  ["WGS 84", WGS84],
  ["a Saturn-class figure", { equatorialRadiusM: 60_268_000, polarRadiusM: 54_364_000 }],
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
}> {
  const exact = viewExactOf(
    camera,
    figure,
    ATMOSPHERE_M,
    { radiusM: figure.equatorialRadiusM, heightM: 0, normal: { x: 1, y: 0, z: 0 } },
    0,
  );
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

/** The near form's worst heights on a figure, as fractions of the bound, and R05's. */
function nearReport(figure: SpheroidFigure): { readonly worst: number; readonly r05: number } {
  let worst = 0;
  let r05 = 0;
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
          }
        }
      }
    }
  }
  return { worst, r05 };
}

/**
 * The closest-approach form's worst heights near the limb and across the disc, as fractions of the
 * bound: rays whose tangent height is 0–100 km, and rays that meet the ground (a negative tangent
 * height), whose segment ends at the ground's near root. `far` forces the form; the kernel takes
 * the far form above h₀ = a ÷ 2, so at 1.5a, where h₀ is a ÷ 2 on a sphere, both are held.
 */
function farReport(figure: SpheroidFigure, distanceM: number, far: boolean): number {
  const a = figure.equatorialRadiusM;
  const k = a / figure.polarRadiusM;
  let worst = 0;
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
          }
        }
      }
    }
  }
  return worst;
}

describe.each(FIGURES)(
  "the ray march's quadric (rayMarch.wgsl) on %s, the camera narrowed and q_g, q_t from f64",
  (_name, figure) => {
    // One figure's near-form figures, computed once for its own tests.
    const near = nearReport(figure);

    it("keeps every radial height from −400 m to 400 km within 10⁻⁶ · max(t, |h|, 1 km)", () => {
      expect(near.worst).toBeLessThanOrEqual(1);
    });

    it("finds R05's heightAbove over that bound", () => {
      expect(near.r05).toBeGreaterThan(1);
    });

    it.each([1.5, 6.6, 60, 0])(
      "keeps every height near the limb and across the disc from %d a (0: 10⁹ m) within 4ε max(a, |P|) + 10⁻⁶ max(|t − t*|, 1 km)",
      (multiple) => {
        const distanceM = multiple > 0 ? multiple * figure.equatorialRadiusM : 1e9;
        expect(farReport(figure, distanceM, true)).toBeLessThanOrEqual(1);
      },
    );

    it("keeps the near form within the same bound at 1.5a, where h₀ = a ÷ 2 takes it", () => {
      expect(farReport(figure, 1.5 * figure.equatorialRadiusM, false)).toBeLessThanOrEqual(1);
    });
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

/** `skyViewUvToParams`: the ray's zenith cosine and sine and its azimuth's, in the sky frame. */
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
  const lightViewCos = f(1 - f(f(2 * ux) * ux));
  const lightViewSin = f(f(2 * ux) * f(Math.sqrt(Math.max(f(1 - f(ux * ux)), 0))));
  return { x: f(sin * lightViewCos), y: f(sin * lightViewSin), z: cos };
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
  const length = f(Math.sqrt(h2));
  const chordX = f(f(dir.x / length) - 1);
  const chordY = f(dir.y / length);
  const azimuthHalfSine = f(0.5 * f(Math.sqrt(f(f(chordX * chordX) + f(chordY * chordY)))));
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
    filterPosition(texelUvFromUnit(unitClamp(azimuthHalfSine), size[0]), size[0]),
    filterPosition(texelUvFromUnit(unitClamp(v), size[1]), size[1]),
  ];
}

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
            worst = Math.max(worst, Math.abs(y - j), vertical ? 0 : Math.abs(x - i));
          }
        }
      }
      expect(unread.filter((texel) => texel.length > 0)).toEqual([]);
      expect(worst).toBeLessThan(1e-3);
    },
  );
});
