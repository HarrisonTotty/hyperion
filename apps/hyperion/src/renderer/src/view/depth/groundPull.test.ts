/**
 * The grazing fix's arithmetic (plan R10, R10.T16.c): the ground's depth slope in closed form
 * against the exact depth over the plane in `f64`, and the terrain's push and a ground mark's pull
 * in metres at 10 m, 1 km and 300 km out, grazing 2° and 60°, at 1080p across 60°.
 */

import { describe, expect, it } from "vitest";

import { scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { groundDepthSlopePerPx, groundMarkDepth, TERRAIN_DEPTH_FRACTION } from "./groundPull";

const VIEWPORT = { widthPx: 1920, heightPx: 1080 } as const;
const CAMERA = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 } as const;
/** The focal length, px: the view's 960 px half-width over tan 30°. */
const FOCAL_PX = 960 / Math.tan(Math.PI / 6);
/** The overlay's slope term at a stroke scale of 1, px (`occluderSlopePxAt(1)`). */
const OCCLUDER_SLOPE_PX = 3;
const RAD = Math.PI / 180;

/**
 * The view axes' camera looks along −z, its screen up +y. The ground is the plane through the
 * point `rangeM` ahead, its normal tilted `grazingRad` from the view axis so that the view's
 * central ray meets it at that angle; the body's centre lies far beneath it along its normal.
 */
function groundAt(
  rangeM: number,
  grazingRad: number,
): { readonly point: Vec3; readonly normal: Vec3; readonly centre: Vec3 } {
  const point = vec3(0, 0, -rangeM);
  const normal = vec3(0, Math.cos(grazingRad), Math.sin(grazingRad));
  return { point, normal, centre: sub(point, scale(normal, 1e9)) };
}

/** The exact reversed depth over the plane at pixel (x, y) from the view's centre, in `f64`. */
function planeDepthAt(point: Vec3, normal: Vec3, xPx: number, yPx: number): number {
  const s = 1 / Math.tan(CAMERA.fovXRad / 2);
  const aspect = VIEWPORT.widthPx / VIEWPORT.heightPx;
  const ray = vec3(
    (2 * xPx) / VIEWPORT.widthPx / s,
    (2 * yPx) / VIEWPORT.heightPx / (s * aspect),
    -1,
  );
  const t =
    (normal.x * point.x + normal.y * point.y + normal.z * point.z) /
    (normal.x * ray.x + normal.y * ray.y + normal.z * ray.z);
  // The ray's view-axis component is 1, so t is the distance along the view axis.
  return NEAR_PLANE_M / t;
}

/** A length in reversed depth at the range `rangeM` along the view axis, as metres: Δd z² ÷ n. */
function metresOf(depthDelta: number, rangeM: number): number {
  return (depthDelta * rangeM * rangeM) / NEAR_PLANE_M;
}

const CASES = [10, 1_000, 300_000].flatMap((rangeM) =>
  [2, 60].map((grazingDeg) => ({ rangeM, grazingDeg })),
);

describe("the ground's depth slope and the terrain's push (R10.T16.c's grazing fix)", () => {
  it.each(CASES)(
    "gives the plane's depth slope at $rangeM m, grazing $grazingDeg°, as the exact depth does",
    ({ rangeM, grazingDeg }) => {
      const { point, normal, centre } = groundAt(rangeM, grazingDeg * RAD);
      const dx = planeDepthAt(point, normal, 0.5, 0) - planeDepthAt(point, normal, -0.5, 0);
      const dy = planeDepthAt(point, normal, 0, 0.5) - planeDepthAt(point, normal, 0, -0.5);
      expect(
        groundDepthSlopePerPx(point, centre, CAMERA, VIEWPORT) / Math.hypot(dx, dy),
      ).toBeCloseTo(1, 9);
    },
  );

  it.each(CASES)(
    "pulls a ground mark at $rangeM m, grazing $grazingDeg°, by the old push, 1.8e-3 cot θ of the range",
    ({ rangeM, grazingDeg }) => {
      const { point, centre } = groundAt(rangeM, grazingDeg * RAD);
      const slope = groundDepthSlopePerPx(point, centre, CAMERA, VIEWPORT);
      const pulledM = metresOf(OCCLUDER_SLOPE_PX * slope, rangeM);
      const expectedM = ((OCCLUDER_SLOPE_PX / FOCAL_PX) * rangeM) / Math.tan(grazingDeg * RAD);
      expect(pulledM / expectedM).toBeCloseTo(1, 9);
    },
  );

  it("pushes the terrain by 2^-16 of its depth alone: 0.15 mm at 10 m, 1.5 cm at 1 km, 4.6 m at 300 km", () => {
    const pushedM = [10, 1_000, 300_000].map((rangeM) => {
      const depth = NEAR_PLANE_M / rangeM;
      // The range at which the pushed depth stands, less the ground's.
      return NEAR_PLANE_M / (depth * (1 - TERRAIN_DEPTH_FRACTION)) - rangeM;
    });
    expect(pushedM.map((m) => m.toPrecision(3))).toEqual(["0.000153", "0.0153", "4.58"]);
  });

  it("hides a mark 1 km behind grazing ground 10 km under the camera, which the slope term showed", () => {
    const grazingRad = 2 * RAD;
    const rangeM = 10_000 / Math.sin(grazingRad);
    const { point, normal, centre } = groundAt(rangeM, grazingRad);
    const ground = planeDepthAt(point, normal, 0, 0);
    const mark = NEAR_PLANE_M / (rangeM + 1_000);
    const slope = groundDepthSlopePerPx(point, centre, CAMERA, VIEWPORT);
    const oldPush = ground * (1 - TERRAIN_DEPTH_FRACTION) - OCCLUDER_SLOPE_PX * slope;
    const newPush = ground * (1 - TERRAIN_DEPTH_FRACTION);
    // Reversed-Z: a fragment stands in front where its depth is greater (greater-equal passes).
    expect([
      Math.round(metresOf(OCCLUDER_SLOPE_PX * slope, rangeM)),
      mark >= oldPush,
      mark >= newPush,
    ]).toEqual([14_804, true, false]);
  });

  it.each([2, 10, 60])(
    "keeps a stroke on the ground at grazing %s° in front of the ground beneath its half-width, 2.5 px",
    (grazingDeg) => {
      const rangeM = 1_000 / Math.sin(grazingDeg * RAD);
      const { point, normal, centre } = groundAt(rangeM, grazingDeg * RAD);
      const depth = planeDepthAt(point, normal, 0, 0);
      const slope = groundDepthSlopePerPx(point, centre, CAMERA, VIEWPORT);
      const pulled = groundMarkDepth(depth, slope, OCCLUDER_SLOPE_PX);
      // The nearer ground beneath the stroke's lower half, 2.5 px down the screen, pushed.
      const beneath = planeDepthAt(point, normal, 0, -2.5) * (1 - TERRAIN_DEPTH_FRACTION);
      expect([beneath > depth, pulled > beneath]).toEqual([true, true]);
    },
  );

  it("pulls no mark past the near plane", () => {
    expect(groundMarkDepth(0.999, 1, OCCLUDER_SLOPE_PX)).toBe(1);
  });
});
