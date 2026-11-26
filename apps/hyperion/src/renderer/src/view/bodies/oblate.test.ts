import { describe, expect, it } from "vitest";

import { cross, dot, norm, normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { shapePhase } from "../appearance/shapes";
import { oblateAlbedoScale, projectedSecondMoment, spheroidGeometricIntegral } from "./oblate";

/**
 * K as the point's quadrature formed it before perf-long-tasks-5, a vector of each point's normal
 * and its azimuth's cosine and sine taken afresh: the twin its tabulated form must equal, bit for bit.
 */
function integralAsMadeEachPoint(
  share: number,
  cOverA: number,
  pole: Vec3,
  toStar: Vec3,
  toCamera: Vec3,
): number {
  const seed = Math.abs(pole.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const ex = normalise(cross(seed, pole));
  const ey = cross(pole, ex);
  const local = (v: Vec3): Vec3 => vec3(dot(v, ex), dot(v, ey), dot(v, pole));
  const s = local(toStar);
  const v = local(toCamera);
  const c = cOverA;
  const dTheta = Math.PI / 96;
  const dPhi = (2 * Math.PI) / 192;
  let k = 0;
  for (let i = 0; i < 96; i += 1) {
    const theta = (i + 0.5) * dTheta;
    const sinT = Math.sin(theta);
    const cosT = Math.cos(theta);
    for (let j = 0; j < 192; j += 1) {
      const phi = (j + 0.5) * dPhi;
      const n = vec3(c * sinT * sinT * Math.cos(phi), c * sinT * sinT * Math.sin(phi), sinT * cosT);
      const area = norm(n);
      const mu = dot(n, v) / area;
      const mu0 = dot(n, s) / area;
      if (mu <= 0 || mu0 <= 0) {
        continue;
      }
      const disc = (share * 2 * mu0) / (mu0 + mu) + (1 - share) * mu0;
      k += disc * mu * area * dTheta * dPhi;
    }
  }
  return k;
}

describe("the spheroid's disc integral", () => {
  it("forms the point's integral with its azimuths tabulated to the bit of each point's own (perf-long-tasks-5)", () => {
    let compared = 0;
    for (const [share, cOverA] of [
      [0, 0.902],
      [0.37, 0.935],
      [1, 0.98],
    ] as const) {
      for (let n = 0; n < 6; n += 1) {
        const pole = normalise(vec3(Math.sin(0.4 + n), 0.3 * Math.cos(n), 1 + 0.2 * n));
        const toStar = normalise(
          vec3(Math.cos(0.7 * n + share), Math.sin(0.7 * n + share), 0.1 * n),
        );
        const toCamera = normalise(vec3(1, 0.25 * n - 0.6, Math.sin(n + cOverA)));
        const k = spheroidGeometricIntegral(share, cOverA, pole, toStar, toCamera);
        expect(Object.is(k, integralAsMadeEachPoint(share, cOverA, pole, toStar, toCamera))).toBe(
          true,
        );
        compared += k > 0 ? 1 : 0;
      }
    }
    expect(compared).toBeGreaterThan(12);
  });

  it("holds the bits the quadrature made with each point's own cosine and sine (perf-long-tasks-2)", () => {
    // FNV-1a over the f64 bytes, little-endian, of m at 65 latitudes for three figures, as the
    // tables were made before their azimuths' cosines and sines were tabulated.
    const values: number[] = [];
    for (const cOverA of [0.902, 0.935, 0.98]) {
      for (let k = 0; k <= 64; k += 1) {
        values.push(projectedSecondMoment(cOverA, k / 64));
      }
    }
    let digest = 0x811c9dc5;
    for (const word of new Uint32Array(Float64Array.from(values).buffer)) {
      for (let byte = 0; byte < 4; byte += 1) {
        digest = Math.imul(digest ^ ((word >>> (8 * byte)) & 0xff), 0x01000193) >>> 0;
      }
    }
    expect(digest.toString(16)).toBe("8bc95c34");
  });

  it("reads m equator-on as its table's first entry, bit for bit, with no table (perf-long-tasks-5)", () => {
    for (const cOverA of [0.9, 0.95]) {
      const equatorOn = projectedSecondMoment(cOverA, 0);
      // The least latitude above 0 reads the table, at its first entry to well under an ulp.
      expect(projectedSecondMoment(cOverA, Number.MIN_VALUE)).toBe(equatorOn);
      expect(projectedSecondMoment(cOverA, -0)).toBe(equatorOn);
    }
  });

  it("gives a sphere m = ⅔ at every latitude, and its quadrature agrees", () => {
    expect(projectedSecondMoment(1, 0.4)).toBe(2 / 3);
    expect(projectedSecondMoment(1 - 1e-9, 0)).toBeCloseTo(2 / 3, 4);
    expect(projectedSecondMoment(1 - 1e-9, 1)).toBeCloseTo(2 / 3, 4);
  });

  it("leaves a sphere's albedo unscaled", () => {
    expect(oblateAlbedoScale(0, 1)).toBe(1);
    expect(oblateAlbedoScale(0.5, 1)).toBe(1);
  });

  it("makes an equator-on spheroid's zero-phase disc reach p against π a c", () => {
    const cOverA = 1 - 0.098;
    const pole = vec3(0, 0, 1);
    const view = vec3(1, 0, 0);
    for (const share of [0, 0.5, 1]) {
      // F = (E ÷ π)(a ÷ Δ)² A′ K = E p (a c ÷ Δ²), so p = A′ K ÷ (π c ÷ a), against A [L + ⅔(1 − L)].
      const k = spheroidGeometricIntegral(share, cOverA, pole, view, view);
      const albedo = (oblateAlbedoScale(share, cOverA) * k) / (Math.PI * cOverA);
      expect(albedo / (share + (2 / 3) * (1 - share))).toBeCloseTo(1, 3);
    }
  });

  it("reduces to the sphere's closed form K = π [L + ⅔(1 − L)] Φ_shape(α)", () => {
    const pole = vec3(0, 0, 1);
    const view = vec3(1, 0, 0);
    for (const alphaDeg of [0, 60, 120]) {
      const alpha = (alphaDeg * Math.PI) / 180;
      const star = vec3(Math.cos(alpha), Math.sin(alpha), 0);
      const k = spheroidGeometricIntegral(0, 1 - 1e-12, pole, star, view);
      expect(k / (Math.PI * (2 / 3) * shapePhase(0, alpha))).toBeCloseTo(1, 2);
    }
  });
});
