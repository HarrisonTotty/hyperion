import { describe, expect, it } from "vitest";

import { norm, type Vec3 } from "../../geometry/vec3";
import {
  type CloudMapTexel,
  CLOUD_MAP_FINEST_M,
  CLOUD_MAP_LEVELS,
  CLOUD_MAP_REANCHOR_RAD,
  CLOUD_MAP_SIDE,
  CloudMapAnchor,
  cloudMapTwinOf,
  cloudMapUniforms,
  mapLevelAt,
  mapTexelM,
  onMapPlane,
  texelPlace,
} from "./cloudMap";

const A = 6_378_137;
const C = 6_356_752.3;
const SHELL = { equatorialRadiusM: A + 1_500, polarRadiusM: C + 1_500 };

/** A place at geocentric latitude and longitude, degrees, `heightM` above a sphere of radius a. */
function placeAt(latitudeDeg: number, longitudeDeg: number, heightM: number): Vec3 {
  const phi = (latitudeDeg * Math.PI) / 180;
  const lambda = (longitudeDeg * Math.PI) / 180;
  const r = A + heightM;
  return {
    x: r * Math.cos(phi) * Math.cos(lambda),
    y: r * Math.cos(phi) * Math.sin(lambda),
    z: r * Math.sin(phi),
  };
}

describe("the cloud map's plane and levels (R11.T7.c)", () => {
  it("reaches from 20 km to 5,200 km either side at 160 m to 41 km a texel", () => {
    expect(mapTexelM(0)).toBe(CLOUD_MAP_FINEST_M);
    expect(mapTexelM(CLOUD_MAP_LEVELS - 1)).toBe(40_960);
    expect((CLOUD_MAP_SIDE / 2) * mapTexelM(0)).toBe(20_480);
    expect((CLOUD_MAP_SIDE / 2) * mapTexelM(CLOUD_MAP_LEVELS - 1)).toBeGreaterThan(5e6);
  });

  it("finds a texel's place on the shell where it projects its centre", () => {
    const frame = new CloudMapAnchor().frameFor(placeAt(30, -40, 2_000), SHELL.equatorialRadiusM);
    for (const level of [0, 2, 4]) {
      const place = texelPlace(frame, SHELL, level, 17, 200);
      const plane = onMapPlane(frame, place);
      const origin = frame.origins[level] ?? { x: 0, y: 0 };
      expect(plane?.x).toBeCloseTo(origin.x + 17.5 * mapTexelM(level), 3);
      expect(plane?.y).toBeCloseTo(origin.y + 200.5 * mapTexelM(level), 3);
      // On the shell's spheroid.
      const k = SHELL.equatorialRadiusM / SHELL.polarRadiusM;
      expect(Math.hypot(place.x, place.y, place.z * k)).toBeCloseTo(SHELL.equatorialRadiusM, 3);
    }
  });

  it("holds its anchor and its texels' places as the camera moves, and moves it far away", () => {
    const anchor = new CloudMapAnchor();
    const first = anchor.frameFor(placeAt(10, 20, 2_000), SHELL.equatorialRadiusM);
    const near = anchor.frameFor(placeAt(10.001, 20.0013, 2_050), SHELL.equatorialRadiusM);
    expect(near.anchor).toEqual(first.anchor);
    // Each level's origin a whole number of its texels from the last frame's.
    for (let level = 0; level < CLOUD_MAP_LEVELS; level += 1) {
      const a = first.origins[level] ?? { x: 0, y: 0 };
      const b = near.origins[level] ?? { x: 0, y: 0 };
      const texel = mapTexelM(level);
      expect(Math.abs((b.x - a.x) / texel - Math.round((b.x - a.x) / texel))).toBeLessThan(1e-6);
    }
    const degrees = (1.5 * CLOUD_MAP_REANCHOR_RAD * 180) / Math.PI;
    const far = anchor.frameFor(placeAt(10 + degrees, 20, 2_000), SHELL.equatorialRadiusM);
    expect(far.anchor).not.toEqual(first.anchor);
    expect(norm(far.anchor)).toBeCloseTo(1, 12);
  });

  it("reads the level whose texel is the footprint, or the first whose extent holds the place", () => {
    const frame = new CloudMapAnchor().frameFor(placeAt(0, 0, 2_000), SHELL.equatorialRadiusM);
    const origin = frame.origins[0] ?? { x: 0, y: 0 };
    const centre = { x: origin.x + 20_480, y: origin.y + 20_480 };
    expect(mapLevelAt(frame, centre, 10)).toBe(0);
    expect(mapLevelAt(frame, centre, 4 * CLOUD_MAP_FINEST_M)).toBeCloseTo(1, 9);
    expect(mapLevelAt(frame, centre, 1e9)).toBe(CLOUD_MAP_LEVELS - 1);
    const out = { x: centre.x + 100_000, y: centre.y };
    expect(mapLevelAt(frame, out, 10)).toBeGreaterThan(1);
    expect(mapLevelAt(frame, { x: centre.x + 6e6, y: centre.y }, 10)).toBeNull();
  });

  it("packs the plane's six rows for its pass and its readers", () => {
    const frame = new CloudMapAnchor().frameFor(placeAt(0, 0, 2_000), SHELL.equatorialRadiusM);
    const uniforms = cloudMapUniforms(frame);
    expect(uniforms["mapEast"]?.[3]).toBe(CLOUD_MAP_FINEST_M);
    expect(uniforms["mapNorth"]?.[3]).toBe(CLOUD_MAP_LEVELS);
    expect(uniforms["mapOrigins4"]?.[2]).toBe(CLOUD_MAP_SIDE);
  });
});

/** A texel whose resolved sum grows across the plane with y, still air, threshold 0. */
const field = (point: Vec3): CloudMapTexel => ({
  resolvedSum: point.y / 50_000,
  threshold: 0,
  eastMps: 0,
  northMps: 0,
});

describe("the cloud map's twin (R11.T7.c)", () => {
  it("reads a texel's own at its centre, at level 0's size", () => {
    const frame = new CloudMapAnchor().frameFor(placeAt(0, 0, 2_000), SHELL.equatorialRadiusM);
    const twin = cloudMapTwinOf(frame, SHELL, field);
    const place = texelPlace(frame, SHELL, 0, 128, 128);
    const read = twin.readAt(place, 10);
    expect(read?.texel.resolvedSum).toBeCloseTo(twin.texel(0, 128, 128).resolvedSum, 9);
    expect([read?.fineM, read?.blend]).toEqual([CLOUD_MAP_FINEST_M, 0]);
  });

  it("blends a field that is linear across the plane linearly, to f16's step", () => {
    const frame = new CloudMapAnchor().frameFor(placeAt(0, 0, 2_000), SHELL.equatorialRadiusM);
    const twin = cloudMapTwinOf(frame, SHELL, field);
    const between = placeAt(0, 0.05, 1_500);
    expect(twin.readAt(between, 10)?.texel.resolvedSum).toBeCloseTo(field(between).resolvedSum, 3);
  });

  it("reads nothing beyond its last level", () => {
    const frame = new CloudMapAnchor().frameFor(placeAt(0, 0, 2_000), SHELL.equatorialRadiusM);
    const twin = cloudMapTwinOf(frame, SHELL, field);
    expect(twin.readAt(placeAt(0, 170, 1_500), 10)).toBeNull();
  });
});
