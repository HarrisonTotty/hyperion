import { describe, expect, it } from "vitest";

import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { LimbDarkenedDisc } from "../lighting/annuli";
import { ringProfile } from "./ringProfile";
import {
  MAX_RING_SHADOW_BANDS,
  packRingShadowRows,
  planetShadowOnRing,
  RING_SHADOW_ROWS,
  ringShadowOnBody,
  ringShadowRecordOf,
  type RingShadowRecord,
  type ShadowingPlanet,
  shadowingRing,
} from "./ringShadows";

/** A ring in the plane z = 0, in planetary radii: C, B, the Division and A. */
const RING: RingShadowRecord = {
  normal: vec3(0, 0, 1),
  innerOverA: 1.24,
  bands: [
    { outerOverA: 1.53, tau: 0.1 },
    { outerOverA: 1.95, tau: 2.2 },
    { outerOverA: 2.03, tau: 0.1 },
    { outerOverA: 2.27, tau: 0.6 },
  ],
};

/** The band's τ at radius r by the record, as an oracle independent of the scan. */
function tauOracle(r: number): number {
  if (r < 1.24 || r >= 2.27) {
    return 0;
  }
  if (r < 1.53) {
    return 0.1;
  }
  if (r < 1.95) {
    return 2.2;
  }
  return r < 2.03 ? 0.1 : 0.6;
}

describe("the ring's shadow on its planet", () => {
  it("is exp(−τ(r) ÷ |μ₀|) at the plane crossing", () => {
    // Points of the southern hemisphere, the star above the plane at elevations 10–40°.
    let shaded = 0;
    for (let i = 0; i < 400; i += 1) {
      const theta = 0.05 + (i % 20) * 0.07;
      const phi = Math.floor(i / 20) * 0.31;
      const p = vec3(
        Math.sin(theta) * Math.cos(phi),
        Math.sin(theta) * Math.sin(phi),
        -Math.cos(theta),
      );
      const b = (10 + (i % 7) * 5) * (Math.PI / 180);
      const sun = vec3(Math.cos(b) * Math.cos(phi), Math.cos(b) * Math.sin(phi), Math.sin(b));
      const mu0 = sun.z;
      const t = -p.z / mu0;
      const crossing = add(p, scale(sun, t));
      const r = Math.hypot(crossing.x, crossing.y);
      const tau = tauOracle(r);
      const expected = tau > 0 ? Math.exp(-tau / mu0) : 1;
      expect(ringShadowOnBody(RING, p, sun)).toBeCloseTo(expected, 14);
      shaded += tau > 0 ? 1 : 0;
    }
    expect(shaded).toBeGreaterThan(50);
  });

  it("casts none where the ray crosses behind the point, off the annulus or nowhere", () => {
    const p = vec3(0, 0.6, -0.8);
    // Towards a star below the plane the ray never crosses it ahead of a southern point.
    expect(ringShadowOnBody(RING, p, normalise(vec3(0, 1, -0.3)))).toBe(1);
    // A star in the plane.
    expect(ringShadowOnBody(RING, p, vec3(0, 1, 0))).toBe(1);
    // A crossing beyond the outer edge.
    expect(ringShadowOnBody(RING, vec3(0, 0.99, -0.14), normalise(vec3(0, 1, 0.02)))).toBe(1);
    // A point above the plane, its star above it too.
    expect(ringShadowOnBody(RING, vec3(0, 0.6, 0.8), normalise(vec3(0, 1, 0.5)))).toBe(1);
  });

  it("is built from a profile in units of a, at most sixteen bands", () => {
    const a = 60.268e6;
    const profile = ringProfile({
      ringKind: "massive",
      material: "porous_ice",
      innerRadiusM: 74.66e6,
      outerRadiusM: 136.78e6,
      opticalDepth: 1.5,
      gapRadiiM: [117.58e6],
    });
    const record = ringShadowRecordOf(profile, vec3(0, 0, 1), a);
    expect(record.innerOverA).toBeCloseTo(74.66e6 / a, 15);
    expect(record.bands.map((band) => band.tau)).toEqual([1.5, 0.15000000000000002, 1.5]);
    expect(record.bands.at(-1)?.outerOverA).toBeCloseTo(136.78e6 / a, 15);
    // Ten gaps make 21 bands; the first 15 are kept and the 16th runs to the outer edge.
    const many = ringProfile({
      ringKind: "massive",
      material: "porous_ice",
      innerRadiusM: 1e8,
      outerRadiusM: 2e8,
      opticalDepth: 1,
      gapRadiiM: Array.from({ length: 10 }, (_, k) => 1.05e8 + k * 0.09e8),
    });
    expect(many.bands).toHaveLength(21);
    const capped = ringShadowRecordOf(many, vec3(0, 0, 1), 1e8);
    expect(capped.bands).toHaveLength(MAX_RING_SHADOW_BANDS);
    // The 16th runs to the outer edge at the ring's own τ, not at the gap's that stood there.
    expect(many.bands[MAX_RING_SHADOW_BANDS - 1]?.region).toBe("gap");
    expect(capped.bands.at(-1)).toEqual({ outerOverA: 2, tau: 1 });
    expect(capped.bands.slice(0, -1)).toEqual(
      many.bands
        .slice(0, MAX_RING_SHADOW_BANDS - 1)
        .map((band) => ({ outerOverA: band.outerM / 1e8, tau: band.tau })),
    );
  });

  it("packs as ringShadow.wgsl's rows: the normal and inner edge, the count, two bands a row", () => {
    const rows = packRingShadowRows(RING);
    expect(rows).toHaveLength(RING_SHADOW_ROWS * 4);
    expect(Array.from(rows.subarray(0, 8))).toEqual([0, 0, 1, Math.fround(1.24), 4, 0, 0, 0]);
    expect(Array.from(rows.subarray(8, 16))).toEqual(
      [1.53, 0.1, 1.95, 2.2, 2.03, 0.1, 2.27, 0.6].map(Math.fround),
    );
    expect(Array.from(packRingShadowRows(undefined))).toEqual(
      Array.from({ length: RING_SHADOW_ROWS * 4 }, () => 0),
    );
  });

  it("is cast by a planet's densest ring", () => {
    expect(shadowingRing([{ opticalDepth: 3e-6 }, { opticalDepth: 1.2 }])).toEqual({
      opticalDepth: 1.2,
    });
    expect(shadowingRing([])).toBeNull();
  });
});

/** The Sun from far off, B band's limb darkening (Maxted 2018, Table 2), at angular radius ρ. */
function starAt(direction: Vec3, angularRadiusRad: number): LimbDarkenedDisc {
  const distanceM = 1.4e12;
  return {
    centreM: scale(normalise(direction), distanceM),
    radiusM: Math.sin(angularRadiusRad) * distanceM,
    limbC: 0.846,
    limbAlpha: 0.83,
  };
}

/** Whether the ray from `from` towards `towards` meets the spheroid of `planet`. */
function raySpheroidHits(planet: ShadowingPlanet, from: Vec3, towards: Vec3): boolean {
  // In the frame stretched along the pole by a ÷ c the spheroid is the sphere of radius a.
  const k = 1 / planet.polarOverEquatorial - 1;
  const stretch = (v: Vec3): Vec3 => add(v, scale(planet.pole, k * dot(v, planet.pole)));
  const o = stretch(sub(from, planet.centreM));
  const d = stretch(towards);
  const a = dot(d, d);
  const b = dot(o, d);
  const c = dot(o, o) - planet.radiusM * planet.radiusM;
  const disc = b * b - a * c;
  return disc >= 0 && -b + Math.sqrt(disc) > 0 && (-b - Math.sqrt(disc) > 0 || c < 0);
}

describe("the planet's shadow on its ring", () => {
  const sphere: ShadowingPlanet = {
    centreM: vec3(0, 0, 0),
    radiusM: 6e7,
    pole: vec3(0, 0, 1),
    polarOverEquatorial: 1,
  };

  it.each([1, 0.9])(
    "agrees with a ray–spheroid oracle for a star of 0.2 arcseconds, c ÷ a %f",
    (polarOverEquatorial) => {
      const planet = { ...sphere, polarOverEquatorial };
      const towards = normalise(vec3(1, 0.2, 0.35));
      const star = starAt(towards, 1e-6);
      let dark = 0;
      for (let i = 0; i < 60; i += 1) {
        for (let j = 0; j < 60; j += 1) {
          // Points of a ring plane tilted 20° about x, 1.1–2.4 radii out on the night side.
          const r = 6e7 * (1.1 + (1.3 * i) / 59);
          const phi = Math.PI * (0.6 + (0.8 * j) / 59);
          const tilt = 20 * (Math.PI / 180);
          const point = vec3(
            r * Math.cos(phi),
            r * Math.sin(phi) * Math.cos(tilt),
            r * Math.sin(phi) * Math.sin(tilt),
          );
          const hit = raySpheroidHits(planet, point, towards);
          const visible = planetShadowOnRing(point, star, planet, 4);
          // Within a kilometre of the limb's edge the star's penumbra, 300 m wide at most here, may
          // fall either way.
          const nudged = (by: number): boolean =>
            raySpheroidHits(planet, add(point, vec3(0, by, 0)), towards) === hit &&
            raySpheroidHits(planet, add(point, vec3(0, 0, by)), towards) === hit;
          if (!nudged(1e3) || !nudged(-1e3)) {
            continue;
          }
          expect(visible).toBeCloseTo(hit ? 0 : 1, 9);
          dark += hit ? 1 : 0;
        }
      }
      expect(dark).toBeGreaterThan(200);
    },
  );

  it("has a penumbra of the star's angular diameter times the distance from the limb", () => {
    const rho = 0.005;
    const star = starAt(vec3(1, 0, 0), rho);
    const lengthM = 10 * sphere.radiusM;
    const visibleAt = (yM: number): number =>
      planetShadowOnRing(vec3(-lengthM, yM, 0), star, sphere, 4);
    // Bisect for the last y wholly in shadow and the first wholly in light.
    const edge = (wanted: (v: number) => boolean): number => {
      let lo = 0.5 * sphere.radiusM;
      let hi = 2 * sphere.radiusM;
      for (let k = 0; k < 80; k += 1) {
        const mid = (lo + hi) / 2;
        if (wanted(visibleAt(mid))) {
          hi = mid;
        } else {
          lo = mid;
        }
      }
      return hi;
    };
    const inner = edge((v) => v > 0);
    const outer = edge((v) => v >= 1);
    const y = (inner + outer) / 2;
    const limbDistanceM = Math.sqrt(lengthM * lengthM + y * y - sphere.radiusM ** 2);
    expect((outer - inner) / (2 * rho * limbDistanceM)).toBeCloseTo(1, 1);
    expect(Math.abs((outer - inner) / (2 * rho * limbDistanceM) - 1)).toBeLessThan(0.02);
    // Half the star is hidden near the penumbra's middle.
    expect(visibleAt(y)).toBeGreaterThan(0.3);
    expect(visibleAt(y)).toBeLessThan(0.7);
  });
});
