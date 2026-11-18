import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { aViewRing } from "../../test/ringFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { LitBodyInput } from "../bodies/draw";
import { type ProjectionCamera, project } from "../camera/projection";
import { lookAlong, rotationRows } from "../camera/quaternion";
import { sunLikeHostDisc } from "../lighting/hostDisc";
import { lightsAt } from "../lighting/hostLights";
import { ringEllipse } from "../wireframe/bodies";
import { oppositionSurge, SURGE_HALF_WIDTH_RAD } from "./ringPhotometry";
import { ringProfile } from "./ringProfile";
import { planetShadowOnRing } from "./ringShadows";
import {
  MAX_RING_OCCLUDERS,
  packRingSlabRecords,
  pixelRay,
  profileSample,
  profileTauScale,
  RING_MESH_SEGMENTS,
  RING_PROFILE_LEVELS,
  RING_PROFILE_TEXELS,
  RING_SLAB_ROWS,
  ringOccludersOf,
  ringPlanetVisible,
  ringProfileTexels,
  ringSlabFragment,
  ringSlabMesh,
  type RingOccluderCandidate,
  type RingSlabRecord,
  ringSlabRecordOf,
  surgeAt,
  transparentSteps,
} from "./ringSlab";

/** A Saturn-like planet's equatorial radius, m. */
const A_M = 6.0e7;
const AU_M = 1.495978707e11;
const PLANET = "0300080020000000.0001";

/** The ring in the planet's equatorial plane, z = 0 along the galactic axes. */
const NORMAL = vec3(0, 0, 1);
const RING = aViewRing(PLANET, 1.24 * A_M, 2.27 * A_M, NORMAL);

/** A lit planet at `centreM` from the camera, a sphere of radius {@link A_M}. */
function planetAt(centreM: Vec3): LitBodyInput {
  return {
    id: PLANET,
    centreM,
    figure: { equatorialRadiusM: A_M, polarRadiusM: A_M, pole: NORMAL },
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
  };
}

/** The star 10 au from the planet towards `towards`. */
function starsOf(centreM: Vec3, towards: Vec3): ReturnType<typeof lightsAt> {
  const hosts = [
    { disc: sunLikeHostDisc(), centreM: add(centreM, scale(normalise(towards), 10 * AU_M)) },
  ];
  return lightsAt(centreM, hosts, 2);
}

/** A record of the ring seen from `cameraOffset` (from the planet), the star towards `towards`. */
function recordFrom(cameraOffset: Vec3, towards: Vec3): RingSlabRecord {
  const centreM = scale(cameraOffset, -1);
  return ringSlabRecordOf(RING, planetAt(centreM), starsOf(centreM, towards), [], {
    annuli: 4,
    exposureScale: 1e-4,
  });
}

/** The ray from the camera to a point given from the planet's centre ÷ a. */
function rayTo(record: RingSlabRecord, point: Vec3): Vec3 {
  return normalise(add(record.centre, point));
}

const TEXELS = ringProfileTexels(ringProfile(RING));

/** A value as a half-float holds it. */
function half(v: number): number {
  return new Float16Array([v])[0] ?? Number.NaN;
}

describe("a ring's profile texels (R11.T9.c)", () => {
  const profile = ringProfile(RING);
  const texelAt = (rM: number): number =>
    Math.floor(((rM - profile.innerM) / (profile.outerM - profile.innerM)) * RING_PROFILE_TEXELS);
  const texel = (row: number, i: number, c: number): number =>
    TEXELS[(row * RING_PROFILE_TEXELS + i) * 4 + c] ?? Number.NaN;

  const tauScale = profileTauScale(profile);

  it("holds each band's τ, ϖ₀ and surge half-width at level 0 inside it, alone", () => {
    for (const band of profile.bands) {
      const i = texelAt((band.innerM + band.outerM) / 2);
      expect([texel(0, i, 0), texel(0, i, 1), texel(0, i, 2), texel(0, i, 3)]).toEqual(
        [...band.albedo, band.tau / tauScale].map(half),
      );
      const width = band.region === "dusty" ? 0 : SURGE_HALF_WIDTH_RAD[band.region];
      const mix = 2 * RING_PROFILE_LEVELS;
      expect([texel(mix, i, 0), texel(mix, i, 1)]).toEqual([0, half(width)]);
    }
  });

  it("holds the ring and its gap apart at its top level, the gap's share their lengths'", () => {
    const top = RING_PROFILE_LEVELS - 1;
    const middle = RING_PROFILE_TEXELS / 2;
    const gaps = profile.bands.filter((band) => band.region === "gap");
    const gapLength = gaps.reduce((sum, band) => sum + band.outerM - band.innerM, 0);
    // The top level's window spans the whole annulus from the middle texel.
    expect(texel(top, middle, 3) * tauScale).toBeCloseTo(RING.opticalDepth, 2);
    expect(texel(RING_PROFILE_LEVELS + top, middle, 3) * tauScale).toBeCloseTo(
      gaps[0]?.tau ?? 0,
      3,
    );
    expect(texel(2 * RING_PROFILE_LEVELS + top, middle, 0)).toBeCloseTo(
      gapLength / (profile.outerM - profile.innerM),
      3,
    );
  });

  it("lets through, where a footprint mixes the ring and its gap, the area average of exp(−τ ÷ μ)", () => {
    const gap = profile.bands.find((band) => band.region === "gap");
    if (gap === undefined) {
      throw new Error("the fixture's ring has no gap");
    }
    const level = 6;
    const texelM = (profile.outerM - profile.innerM) / RING_PROFILE_TEXELS;
    // The texel whose window, 2⁶ texels wide, straddles the gap's inner edge.
    const i = texelAt(gap.innerM);
    const sample = profileSample(TEXELS, (i + 0.5) / RING_PROFILE_TEXELS, level, tauScale);
    const middle = profile.innerM + (i + 0.5) * texelM;
    const halfM = (texelM * 2 ** level) / 2;
    const mu = Math.sin((4.5 * Math.PI) / 180);
    let through = 0;
    for (const band of profile.bands) {
      const overlap = Math.min(middle + halfM, band.outerM) - Math.max(middle - halfM, band.innerM);
      through += overlap > 0 ? overlap * Math.exp(-band.tau / mu) : 0;
    }
    through /= 2 * halfM;
    const mixed =
      (1 - sample.share) * Math.exp(-sample.first.tau / mu) +
      sample.share * Math.exp(-sample.second.tau / mu);
    expect(sample.share).toBeGreaterThan(0.2);
    expect(sample.share).toBeLessThan(0.8);
    expect(mixed).toBeCloseTo(through, 3);
  });

  it("is sampled at a texel's centre as that texel, and halfway between two as their mean", () => {
    const i = texelAt(profile.innerM + 0.3 * (profile.outerM - profile.innerM));
    const centre = (i + 0.5) / RING_PROFILE_TEXELS;
    expect(profileSample(TEXELS, centre, 0, tauScale).first.tau).toBe(texel(0, i, 3) * tauScale);
    const between = profileSample(TEXELS, (i + 1) / RING_PROFILE_TEXELS, 0.5, tauScale).first.tau;
    const expected =
      ((texel(0, i, 3) + texel(0, i + 1, 3) + texel(1, i, 3) + texel(1, i + 1, 3)) / 4) * tauScale;
    expect(between).toBeCloseTo(expected, 12);
  });

  it("keeps a dusty ring's optical depth to a half-float's precision", () => {
    const dusty = ringProfile({ ...RING, ringKind: "dusty", opticalDepth: 2e-7, gapRadiiM: [] });
    const texels = ringProfileTexels(dusty);
    const sample = profileSample(texels, 0.5, 0, profileTauScale(dusty));
    expect(sample.first.tau / 2e-7).toBeCloseTo(1, 3);
  });
});

describe("the slab's surge (R11.T9.c)", () => {
  it("is ringPhotometry's opposition surge at each region's half-width", () => {
    for (const region of ["dense", "sparse", "gap"] as const) {
      for (const alphaDeg of [0, 0.1, 0.3, 2]) {
        const alpha = (alphaDeg * Math.PI) / 180;
        expect(surgeAt(alpha, 1.2, SURGE_HALF_WIDTH_RAD[region], false)).toBeCloseTo(
          oppositionSurge(alpha, region, 1.2),
          14,
        );
      }
    }
    expect(surgeAt(0, 0.1, SURGE_HALF_WIDTH_RAD.dense, true)).toBe(1);
  });
});

describe("a ring's record (R11.T9.c)", () => {
  it("packs its rows where ringSlab.wgsl reads them", () => {
    const record = recordFrom(vec3(0, -5 * A_M, 2 * A_M), vec3(1, 0, 0.3));
    const rows = packRingSlabRecords([record, record]);
    expect(rows).toHaveLength(2 * RING_SLAB_ROWS * 4);
    const row = (i: number): number[] => Array.from(rows.subarray(i * 4, i * 4 + 4));
    expect(row(0)[3]).toBeCloseTo(-2, 6);
    expect(row(1)).toEqual([0, 0, 1, Math.fround(1.24)]);
    expect(row(3).slice(0, 3)).toEqual([Math.fround(2.27), 1, 0]);
    expect(row(4)).toEqual([0, 4, 1.5, 0]);
    // Light 0's distance ÷ a, 10 au away.
    expect(row(5)[3]).toBeCloseTo((10 * AU_M) / A_M, -1);
  });

  it("takes the disc bodies that may stand before it, the largest first, never its planet", () => {
    const centreM = vec3(0, 0, -10 * A_M);
    const candidates = [
      moon(PLANET, centreM, A_M),
      moon("0300080020000000.0002", vec3(0, 0, -5 * A_M), 1e6),
      moon("0300080020000000.0003", vec3(0, 0, -6 * A_M), 3e6),
      // Off to the side, clear of the ring on the view.
      moon("0300080020000000.0004", vec3(40 * A_M, 0, -6 * A_M), 3e6),
      // Beyond the ring's far side.
      moon("0300080020000000.0005", vec3(0, 0, -20 * A_M), 3e6),
      ...[6, 7, 8, 9].map((k) => moon(`0300080020000000.000${k}`, vec3(0, A_M, -7 * A_M), 1e5 * k)),
    ];
    const picked = ringOccludersOf(PLANET, centreM, RING.outerRadiusM, candidates).map((b) => b.id);
    expect(picked).toHaveLength(MAX_RING_OCCLUDERS);
    expect(picked.slice(0, 2)).toEqual(["0300080020000000.0003", "0300080020000000.0002"]);
    expect(picked).not.toContain(PLANET);
    expect(picked).not.toContain("0300080020000000.0004");
    expect(picked).not.toContain("0300080020000000.0005");
  });
});

/** A body that may stand before the ring. */
function moon(id: string, at: Vec3, radiusM: number): RingOccluderCandidate {
  return { id, centreM: at, radiusM };
}

/** The ring's radius ÷ a along a ray. */
function radiusAt(record: RingSlabRecord, ray: Vec3): number {
  const t = record.planeOffset / dot(record.normal, ray);
  return norm(sub(scale(ray, t), record.centre));
}

describe("the annulus's mesh (R11.T9.c)", () => {
  it("holds the inner edge outside its inner polygon and the outer edge inside its outer one", () => {
    const mesh = ringSlabMesh("test");
    const corners = Array.from({ length: mesh.positions.length / 3 }, (_, i) => ({
      r: Math.hypot(mesh.positions[3 * i] ?? 0, mesh.positions[3 * i + 1] ?? 0),
      edge: mesh.positions[3 * i + 2] ?? 0,
    }));
    expect(corners).toHaveLength(2 * RING_MESH_SEGMENTS);
    const inner = corners.filter((c) => c.edge === 0).map((c) => c.r);
    const outer = corners.filter((c) => c.edge === 1).map((c) => c.r);
    expect(Math.max(...inner)).toBeLessThanOrEqual(1 + 1e-6);
    // The outer polygon's sides touch the unit circle at their middles: r cos(π ÷ N) = 1.
    expect(Math.min(...outer) * Math.cos(Math.PI / RING_MESH_SEGMENTS)).toBeGreaterThanOrEqual(
      1 - 1e-6,
    );
    expect(mesh.indices).toHaveLength(6 * RING_MESH_SEGMENTS);
  });
});

describe("the slab's twin (R11.T9.c)", () => {
  // The camera 8 a out and 3 a above the plane, the star along +x and 20° above it.
  const camera = vec3(-8 * A_M, 0, 3 * A_M);
  const towards = vec3(Math.cos(0.35), 0, Math.sin(0.35));
  const record = recordFrom(camera, towards);
  const footprint = 1e-4;

  it("draws nothing off the annulus or behind the planet", () => {
    expect(ringSlabFragment(record, TEXELS, rayTo(record, vec3(1.1, 0, 0)), footprint)).toBeNull();
    expect(ringSlabFragment(record, TEXELS, rayTo(record, vec3(2.4, 0, 0)), footprint)).toBeNull();
    // The far side of the ring, behind the planet's disc from here.
    const hidden = ringSlabFragment(record, TEXELS, rayTo(record, vec3(1.5, 0, 0)), footprint);
    expect(hidden).toBeNull();
    expect(
      ringSlabFragment(record, TEXELS, rayTo(record, vec3(-1.5, 0, 0)), footprint),
    ).not.toBeNull();
  });

  it("lets the light behind it through by exp(−τ ÷ μ)", () => {
    const point = vec3(0, -1.4, 0);
    const ray = rayTo(record, point);
    const fragment = ringSlabFragment(record, TEXELS, ray, footprint);
    const mu = Math.abs(dot(ray, NORMAL));
    expect(fragment?.alpha).toBeCloseTo(1 - Math.exp(-1.5 / mu), 3);
    expect(fragment?.coverage).toBe(1);
  });

  it("is dark in the planet's shadow and lit beside it", () => {
    // The planet's shadow falls towards −x; from 20° above the plane it reaches the ring at 1.24 a.
    const shadowed = vec3(-1.3, 0, 0);
    const lit = vec3(0, 1.3, 0);
    const radianceAt = (p: Vec3): number =>
      ringSlabFragment(record, TEXELS, rayTo(record, p), footprint)?.radiance[1] ?? Number.NaN;
    expect(radianceAt(shadowed)).toBe(0);
    expect(radianceAt(lit)).toBeGreaterThan(0);
  });

  it("takes the planet's shadow as ringShadows' planetShadowOnRing does", () => {
    const light = record.lights[0];
    if (light === undefined) {
      throw new Error("the record has no light");
    }
    const disc = sunLikeHostDisc();
    const [, , r] = disc.limb;
    const star = {
      centreM: scale(light.direction, light.distance * A_M),
      radiusM: light.radius * A_M,
      limbC: r.c,
      limbAlpha: r.alpha,
    };
    const planet = { centreM: vec3(0, 0, 0), radiusM: A_M, pole: NORMAL, polarOverEquatorial: 1 };
    // Across the shadow's edge on the ring, at 1.6 a, where its penumbra is some 300 km wide.
    for (let y = 0.95; y <= 1.05; y += 0.0005) {
      const x = vec3(-Math.sqrt(1.6 * 1.6 - y * y), y, 0);
      expect(ringPlanetVisible(record, light, x)[0]).toBeCloseTo(
        planetShadowOnRing(scale(x, A_M), star, planet, 4),
        6,
      );
    }
  });

  it("is brighter from its lit face than a dense ring is from its unlit one", () => {
    const below = recordFrom(vec3(-8 * A_M, 0, -3 * A_M), towards);
    const point = vec3(0, 1.4, 0);
    const lit = ringSlabFragment(record, TEXELS, rayTo(record, point), footprint)?.radiance[1] ?? 0;
    const unlit = ringSlabFragment(below, TEXELS, rayTo(below, point), footprint)?.radiance[1] ?? 0;
    expect(unlit).toBeGreaterThan(0);
    expect(lit).toBeGreaterThan(5 * unlit);
  });

  it("keeps a fragment in the draw of its part by the planet's air shell", () => {
    const shell = { shellOverA: 1.05 };
    // Before the planet's disc on the near side: in front of the air.
    const near = rayTo(record, vec3(-1.5, 0, 0));
    expect(
      ringSlabFragment(record, TEXELS, near, footprint, { part: "before-air", ...shell }),
    ).not.toBeNull();
    expect(
      ringSlabFragment(record, TEXELS, near, footprint, { part: "beyond-air", ...shell }),
    ).toBeNull();
    // Seen past the limb behind the planet, through its air: beyond it. A point of the far side
    // whose ray passes between the limb and the shell's top.
    const grazing = (x: Vec3): boolean => {
      const ray = rayTo(record, x);
      const across = norm(cross(record.centre, ray));
      return (
        across > 1 &&
        across < shell.shellOverA &&
        dot(ray, record.centre) < dot(ray, add(record.centre, x))
      );
    };
    const found = Array.from({ length: 720 }, (_, k) => (k * Math.PI) / 360)
      .flatMap((theta) =>
        [1.3, 1.6, 1.9, 2.2].map((r) => vec3(r * Math.cos(theta), r * Math.sin(theta), 0)),
      )
      .find((x) => grazing(x));
    if (found === undefined) {
      throw new Error("no point of the ring is seen through the planet's air from here");
    }
    const beyond = rayTo(record, found);
    expect(
      ringSlabFragment(record, TEXELS, beyond, footprint, { part: "beyond-air", ...shell }),
    ).not.toBeNull();
    expect(
      ringSlabFragment(record, TEXELS, beyond, footprint, { part: "before-air", ...shell }),
    ).toBeNull();
  });
});

describe("the slab's edges against R02's ellipses (R11.T9.c)", () => {
  it.each([
    ["orbit, 6 a out and 1.5 a above", vec3(-6 * A_M, -1 * A_M, 1.5 * A_M)],
    ["the far annulus beyond 10⁹ m", vec3(-2e9, 3e8, 4e8)],
    ["just above the plane, 20 a out", vec3(-20 * A_M, 5 * A_M, 0.3 * A_M)],
  ])("projects its edges to the wireframe's ellipse to half a pixel, %s", (_, offset) => {
    const record = recordFrom(offset, vec3(1, 0, 0.3));
    const centreM = scale(offset, -1);
    const viewport = { widthPx: 1920, heightPx: 1080 };
    const cameraOf: ProjectionCamera = {
      orientation: lookAlong(centreM, vec3(0, 0, 1)),
      fovXRad: Math.PI / 3,
    };
    const rows = rotationRows(cameraOf.orientation);
    const { edges } = ringEllipse({ ...RING, centreM }, cameraOf, viewport);
    let worst = 0;
    let points = 0;
    for (const [k, polyline] of edges.entries()) {
      const edgeOverA = k < edges.length / 2 ? record.innerOverA : record.outerOverA;
      for (const point of polyline) {
        const p = project(point, cameraOf, viewport);
        if (!p.inFront) {
          continue;
        }
        const r = radiusAt(record, pixelRay(p.xPx, p.yPx, rows, cameraOf.fovXRad, viewport));
        // The radius's change across one pixel there, the coverage's scale.
        const dx =
          radiusAt(record, pixelRay(p.xPx + 1, p.yPx, rows, cameraOf.fovXRad, viewport)) - r;
        const dy =
          radiusAt(record, pixelRay(p.xPx, p.yPx + 1, rows, cameraOf.fovXRad, viewport)) - r;
        const footprint = Math.max(Math.abs(dx), Math.abs(dy));
        worst = Math.max(worst, Math.abs(r - edgeOverA) / footprint);
        points += 1;
      }
    }
    expect(points).toBeGreaterThan(50);
    // The coverage's half falls within this many pixels of the ellipse: well inside half a pixel.
    expect(worst).toBeLessThan(0.5);
  });
});

describe("the order of a frame's airs and rings (R11.T9.c)", () => {
  const far = "0300080020000000.0011";
  const near = "0300080020000000.0012";
  const distance = (body: string): number => (body === far ? 1e9 : 1e8);

  it("draws a body's ring beyond its air first, then its air, then the ring's rest", () => {
    const steps = transparentSteps(
      [{ body: near, radiusM: 7e7, air: "near air" }],
      [{ index: 0, body: near }],
      distance,
    );
    expect(steps).toEqual([
      { kind: "ring", ring: 0, part: "beyond-air" },
      { kind: "air", air: "near air" },
      { kind: "ring", ring: 0, part: "before-air" },
    ]);
  });

  it("draws the farther body's layers before the nearer's, and a ring with no air whole", () => {
    const steps = transparentSteps(
      [{ body: near, radiusM: 7e7, air: "near air" }],
      [
        { index: 0, body: near },
        { index: 1, body: far },
      ],
      distance,
    );
    expect(steps).toEqual([
      { kind: "ring", ring: 1, part: "all" },
      { kind: "ring", ring: 0, part: "beyond-air" },
      { kind: "air", air: "near air" },
      { kind: "ring", ring: 0, part: "before-air" },
    ]);
  });
});
