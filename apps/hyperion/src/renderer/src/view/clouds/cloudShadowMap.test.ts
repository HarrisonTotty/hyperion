import { describe, expect, it } from "vitest";

import { add, normalise, scale, type Vec3 } from "../../geometry/vec3";
import { cloudMapFrameOf } from "./cloudMap";
import type { CloudGroundLight } from "./cloudShadow";
import {
  CLOUD_SHADOW_ROWS,
  type CloudShadowFrame,
  cloudGroundLightAt,
  packCloudShadow,
  SHADOW_BASE_BLEND,
  SHADOW_READ_LEAST_MU,
  type ShadowLevelRead,
} from "./cloudShadowMap";

/** A spherical datum's radius, m. */
const A = 6_371_000;

const UP: Vec3 = { x: 1, y: 0, z: 0 };

/** The frame of a map about the camera 2 m over the datum at `UP`, its deck from 1 km to 3 km. */
function frameFor(sun: Vec3): CloudShadowFrame {
  return {
    map: cloudMapFrameOf(UP, scale(UP, A + 2), A),
    sun,
    datumRadiusM: A,
    axisRatio: 1,
    deck: { baseM: 1_000, topM: 3_000 },
  };
}

/** The layer's tap the read falls back on. */
const LAYER: CloudGroundLight = { direct: 0.5, sky: 0.5, diffuse: 0.5 };

/** A map whose clouds shadow the plane west of its anchor's meridian: x < 0. */
const EDGE: ShadowLevelRead = (_, plane) =>
  plane.x < 0 ? { direct: 0.25, sky: 0.3, diffuse: 0.2 } : { direct: 1, sky: 1, diffuse: 0 };

/** A place `heightM` over the datum, `eastM` along the map's east from its anchor. */
function placeAt(frame: CloudShadowFrame, eastM: number, heightM: number): Vec3 {
  return scale(normalise(add(scale(UP, A), scale(frame.map.east, eastM))), A + heightM);
}

describe("the cloud volume's shadow map (R11.T7.c)", () => {
  it("packs the deck's base and top, the base's blend and the read's least cosine in its last row", () => {
    const rows = packCloudShadow(frameFor(UP));
    expect(rows.length).toBe(4 * CLOUD_SHADOW_ROWS);
    expect([...rows.subarray(32, 36)]).toEqual([
      1_000,
      3_000,
      Math.fround(SHADOW_BASE_BLEND),
      Math.fround(SHADOW_READ_LEAST_MU),
    ]);
  });

  it("reads a place on the datum at its own texel", () => {
    const frame = frameFor(UP);
    expect(cloudGroundLightAt(frame, placeAt(frame, -100, 0), UP, LAYER, EDGE)).toEqual({
      direct: 0.25,
      sky: 0.3,
      diffuse: 0.2,
    });
    expect(cloudGroundLightAt(frame, placeAt(frame, 100, 0), UP, LAYER, EDGE)).toEqual({
      direct: 1,
      sky: 1,
      diffuse: 0,
    });
  });

  it("reads the direct light of ground above the datum back along its ray, its sky's at its own column", () => {
    // The sun 45° up in the east: ground 500 m up and 100 m east of the shadow's edge has its ray
    // cross the datum 400 m west of the edge, in the shadow, under a clear column.
    const sun = normalise(add(UP, frameFor(UP).map.east));
    const frame = frameFor(sun);
    const light = cloudGroundLightAt(frame, placeAt(frame, 100, 500), sun, LAYER, EDGE);
    expect(light.direct).toBe(0.25);
    expect([light.sky, light.diffuse]).toEqual([1, 0]);
    // Ground 500 m up and 600 m east of the edge stands in the sun.
    expect(cloudGroundLightAt(frame, placeAt(frame, 600, 500), sun, LAYER, EDGE).direct).toBe(1);
  });

  it("gives way to the layer's tap as the ground rises to the deck's base, and within the deck", () => {
    const frame = frameFor(UP);
    const at = (heightM: number): CloudGroundLight =>
      cloudGroundLightAt(frame, placeAt(frame, -100, heightM), UP, LAYER, EDGE);
    expect(at(1_000)).toEqual(LAYER);
    expect(at(2_000)).toEqual(LAYER);
    const near = at(1_000 - (SHADOW_BASE_BLEND * 2_000) / 2);
    expect(near.direct).toBeGreaterThan(0.25);
    expect(near.direct).toBeLessThan(0.5);
    expect(at(1_000 - SHADOW_BASE_BLEND * 2_000).direct).toBeCloseTo(0.25, 12);
  });

  it("takes the layer's tap off the map and for another source", () => {
    const frame = frameFor(UP);
    expect(cloudGroundLightAt(frame, placeAt(frame, 200_000, 0), UP, LAYER, EDGE)).toEqual(LAYER);
    expect(cloudGroundLightAt(frame, scale(UP, -A), UP, LAYER, EDGE)).toEqual(LAYER);
    const other = normalise({ x: 1, y: 0.1, z: 0 });
    expect(cloudGroundLightAt(frame, placeAt(frame, -100, 0), other, LAYER, EDGE)).toEqual(LAYER);
  });
});
