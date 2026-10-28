import { describe, expect, it } from "vitest";

import type { Vec3 } from "../../../geometry/vec3";
import type { AtmosphereMedium } from "../medium";
import { marchTwin, type TwinRay, type TwinRgb, type TwinSources } from "../tablesCpu";

/** A sphere of Earth's size under one Rayleigh-like term: no absorber, no lobe. */
const BOTTOM_M = 6.371e6;
const MEDIUM: AtmosphereMedium = {
  name: "thick twin test",
  topHeightM: 100_000,
  groundAlbedo: [0.3, 0.3, 0.3],
  terms: [
    {
      name: "gas",
      density: { kind: "exponential", scaleHeightM: 8_000 },
      scattering: [5.8e-6, 1.35e-5, 3.31e-5],
      absorption: [0, 0, 0],
      phase: { kind: "rayleigh", depolarisation: [0.03, 0.03, 0.03] },
    },
  ],
};

/** Ψ_ms of the stand-in, and the table's J: constants per channel. */
const PSI: TwinRgb = [0.01, 0.02, 0.03];
const J: TwinRgb = [0.05, 0.04, 0.07];

/** The sources' reads, the table's recording each read's arguments when it has one. */
function sources(thick: boolean, reads: number[][] = []): TwinSources {
  return {
    bottomRadiusM: BOTTOM_M,
    sunTransmittance: (_h, _mu, out) => {
      out.fill(1);
    },
    multiScattering: (_h, _mu, out) => {
      out[0] = PSI[0];
      out[1] = PSI[1];
      out[2] = PSI[2];
    },
    absorberPathDepth: (_c, _p, out) => {
      out.fill(0);
    },
    ...(thick
      ? {
          thickSource: (h, muSun, muTravel, horizontal, out) => {
            reads.push([h, muSun, muTravel, horizontal]);
            out[0] = J[0];
            out[1] = J[1];
            out[2] = J[2];
          },
        }
      : {}),
  };
}

/** The same stand-in with its Ψ_ms read as the table's J: what a march over the table alone gives. */
const TABLE_AS_PSI: TwinSources = {
  ...sources(false),
  multiScattering: (_h, _mu, out) => {
    out[0] = J[0];
    out[1] = J[1];
    out[2] = J[2];
  },
};

/** A horizontal ray from 2 km up, its sun at elevation `elevationRad` across it. */
function rayWith(elevationRad: number): TwinRay {
  const sun: Vec3 = { x: 0, y: Math.cos(elevationRad), z: Math.sin(elevationRad) };
  return {
    originM: { x: 0, y: 0, z: BOTTOM_M + 2_000 },
    direction: { x: 0.6, y: 0.8, z: 0 },
    sun,
    tStartM: 0,
    tEndM: 20_000,
    fromCamera: true,
  };
}

const STEPS = Array.from({ length: 4 }, (_, i) => ({ tM: 2_500 + i * 5_000, dtM: 5_000 }));

function march(source: TwinSources, ray: TwinRay): TwinRgb {
  const { luminance } = marchTwin(
    MEDIUM,
    source,
    { kind: "sphere", bottomRadiusM: BOTTOM_M },
    ray,
    STEPS,
  );
  return [luminance[0], luminance[1], luminance[2]];
}

describe("the twin's march over the thick table (R08.T14.c)", () => {
  it("reads Ψ_ms alone for a source at or below the horizontal, as the thin march does", () => {
    const ray = rayWith(-0.05);
    expect(march(sources(true), ray)).toEqual(march(sources(false), ray));
  });

  it("reads the table's J alone from the first sun cosine cell up", () => {
    const ray = rayWith(0.5);
    expect(march(sources(true), ray)).toEqual(march(TABLE_AS_PSI, ray));
  });

  it("blends the two across the first cell, linearly in the multiple scattering", () => {
    // At 2 km and 2.5–17.5 km along a horizontal ray the local sun cosine is near the zenith one;
    // the blend is linear in Ψ and J at each step, so the march lies between the two.
    const ray = rayWith(Math.asin(1 / 64));
    const both = march(sources(true), ray);
    const thin = march(sources(false), ray);
    const table = march(TABLE_AS_PSI, ray);
    for (const c of [0, 1, 2] as const) {
      expect(both[c]).toBeGreaterThan(Math.min(thin[c], table[c]));
      expect(both[c]).toBeLessThan(Math.max(thin[c], table[c]));
    }
  });

  it("gives the table the view's travel cosine and the horizontal product of view and sun", () => {
    const reads: number[][] = [];
    const ray = rayWith(0.5);
    march(sources(true, reads), ray);
    expect(reads).toHaveLength(STEPS.length);
    for (const [k, [h, muSun, muTravel, horizontal]] of reads.entries()) {
      const t = STEPS[k]?.tM ?? Number.NaN;
      const p = { x: 0.6 * t, y: 0.8 * t, z: BOTTOM_M + 2_000 };
      const r = Math.hypot(p.x, p.y, p.z);
      const up = { x: p.x / r, y: p.y / r, z: p.z / r };
      const d = ray.direction;
      const s = ray.sun;
      const muView = d.x * up.x + d.y * up.y + d.z * up.z;
      const mu0 = s.x * up.x + s.y * up.y + s.z * up.z;
      expect(h).toBeCloseTo(r - BOTTOM_M, 6);
      expect(muSun).toBeCloseTo(mu0, 12);
      expect(muTravel).toBeCloseTo(-muView, 12);
      expect(horizontal).toBeCloseTo(d.x * s.x + d.y * s.y + d.z * s.z - muView * mu0, 12);
    }
  });
});
