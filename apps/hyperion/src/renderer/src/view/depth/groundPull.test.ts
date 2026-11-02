/**
 * The grazing fix's arithmetic (plan R10, R10.T16.c): the ground's depth gradient in closed form
 * against the exact depth over the plane in `f64`, the terrain's push in metres at 10 m, 1 km and
 * 300 km out, and a ground mark's pull, each fragment to the plane at its own pixel and a pixel of
 * its slope besides, over flat ground, a ridge and a dip, at 1080p across 60°.
 */

import { describe, expect, it } from "vitest";

import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import {
  GROUND_PULL_PX,
  groundDepthGradientPerPx,
  groundMarkDepth,
  TERRAIN_DEPTH_FRACTION,
  type ViewPx,
} from "./groundPull";

const VIEWPORT = { widthPx: 1920, heightPx: 1080 } as const;
const CAMERA = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 } as const;
/**
 * The survey edge's cased stroke's reach from its centre line at the views' stroke scale of 2, px:
 * half its 2 px, its casing of 2 px, and the half pixel of its coverage.
 */
const CASED_REACH_PX = 3.5;
/** The overlay's slope term at a stroke scale of 1, px (`occluderSlopePxAt(1)`): the old push's. */
const OCCLUDER_SLOPE_PX = 3;
/** The views' slope term at their stroke scale of 2, px (`occluderSlopePxAt(2)`): the old pull's. */
const VIEW_SLOPE_PX = 5;
const RAD = Math.PI / 180;

/**
 * The view axes' camera looks along −z, its screen up +y. The ground is the plane through the
 * point `rangeM` ahead, its normal tilted `grazingRad` from the view axis so that the view's
 * central ray meets it at that angle, and rolled `rollRad` about it; the body's centre lies far
 * beneath it along its normal.
 */
function groundAt(
  rangeM: number,
  grazingRad: number,
  rollRad = 0,
): { readonly point: Vec3; readonly normal: Vec3; readonly centre: Vec3 } {
  const point = vec3(0, 0, -rangeM);
  const normal = vec3(
    Math.sin(rollRad) * Math.cos(grazingRad),
    Math.cos(rollRad) * Math.cos(grazingRad),
    Math.sin(grazingRad),
  );
  return { point, normal, centre: sub(point, scale(normal, 1e9)) };
}

/** The ray through pixel (x, y) from the view's centre, y down, its view-axis component 1. */
function rayAt(xPx: number, yPx: number): Vec3 {
  const s = 1 / Math.tan(CAMERA.fovXRad / 2);
  const aspect = VIEWPORT.widthPx / VIEWPORT.heightPx;
  return vec3((2 * xPx) / VIEWPORT.widthPx / s, (-2 * yPx) / VIEWPORT.heightPx / (s * aspect), -1);
}

/** The exact reversed depth over the plane at pixel (x, y) from the view's centre, in `f64`. */
function planeDepthAt(point: Vec3, normal: Vec3, xPx: number, yPx: number): number {
  // The ray's view-axis component is 1, so t is the distance along the view axis.
  const t = dot(normal, point) / dot(normal, rayAt(xPx, yPx));
  return NEAR_PLANE_M / t;
}

/** A length in reversed depth at the range `rangeM` along the view axis, as metres: Δd z² ÷ n. */
function metresOf(depthDelta: number, rangeM: number): number {
  return (depthDelta * rangeM * rangeM) / NEAR_PLANE_M;
}

/** The pixel offset `px` along the unit vector `unit`. */
function along(unit: ViewPx, px: number): ViewPx {
  return { xPx: unit.xPx * px, yPx: unit.yPx * px };
}

/** The screen's down, the nearer ground's side of a stroke across the view. */
const DOWN: ViewPx = { xPx: 0, yPx: 1 };

/** Offsets across the cased stroke on the ground, px, its farther side first: −3.5 to 3.5. */
const ACROSS_PX = [-3.5, -2.5, -1, -0.5, 0, 0.5, 1, 2.5, 3.5];

const CASES = [10, 1_000, 300_000].flatMap((rangeM) =>
  [2, 60].flatMap((grazingDeg) => [0, 35].map((rollDeg) => ({ rangeM, grazingDeg, rollDeg }))),
);

describe("the ground's depth gradient and the terrain's push (R10.T16.c's grazing fix)", () => {
  it.each(CASES)(
    "gives the plane's depth gradient at $rangeM m, grazing $grazingDeg°, rolled $rollDeg°, as the exact depth does",
    ({ rangeM, grazingDeg, rollDeg }) => {
      const { point, normal, centre } = groundAt(rangeM, grazingDeg * RAD, rollDeg * RAD);
      const dx = planeDepthAt(point, normal, 0.5, 0) - planeDepthAt(point, normal, -0.5, 0);
      const dy = planeDepthAt(point, normal, 0, 0.5) - planeDepthAt(point, normal, 0, -0.5);
      const gradient = groundDepthGradientPerPx(point, centre, CAMERA, VIEWPORT);
      const size = Math.hypot(dx, dy);
      expect([(gradient.perXPx - dx) / size, (gradient.perYPx - dy) / size].map(Math.abs)).toEqual([
        expect.closeTo(0, 9),
        expect.closeTo(0, 9),
      ]);
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
    const g = groundDepthGradientPerPx(point, centre, CAMERA, VIEWPORT);
    const slope = Math.hypot(g.perXPx, g.perYPx);
    const oldPush = ground * (1 - TERRAIN_DEPTH_FRACTION) - OCCLUDER_SLOPE_PX * slope;
    const newPush = ground * (1 - TERRAIN_DEPTH_FRACTION);
    // Reversed-Z: a fragment stands in front where its depth is greater (greater-equal passes).
    expect([
      Math.round(metresOf(OCCLUDER_SLOPE_PX * slope, rangeM)),
      mark >= oldPush,
      mark >= newPush,
    ]).toEqual([14_804, true, false]);
  });
});

describe("a ground mark's pull, each fragment to the datum's plane at its pixel (R10.T16.c)", () => {
  it.each(CASES)(
    "pulls a fragment of the nearer half at $rangeM m, grazing $grazingDeg°, rolled $rollDeg°, to the plane's exact depth at its pixel, beside its pixel of slope",
    ({ rangeM, grazingDeg, rollDeg }) => {
      const { point, normal, centre } = groundAt(rangeM, grazingDeg * RAD, rollDeg * RAD);
      const gradient = groundDepthGradientPerPx(point, centre, CAMERA, VIEWPORT);
      // The stroke runs along the plane's level line on the view, its normal down the gradient.
      const size = Math.hypot(gradient.perXPx, gradient.perYPx);
      const nearer = { xPx: gradient.perXPx / size, yPx: gradient.perYPx / size };
      const depth = planeDepthAt(point, normal, 0, 0);
      const errors = [0.5, 1, 2.5, CASED_REACH_PX].map((px) => {
        const at = along(nearer, px);
        const exact = planeDepthAt(point, normal, at.xPx, at.yPx);
        const pulled = groundMarkDepth(depth, gradient, at) - GROUND_PULL_PX * size;
        return Math.abs(pulled / exact - 1);
      });
      expect(errors.every((error) => error < 1e-9)).toBe(true);
    },
  );

  it("pulls the centre line and the farther half by a pixel of the plane's slope alone", () => {
    const { point, normal, centre } = groundAt(1_000 / Math.sin(10 * RAD), 10 * RAD);
    const gradient = groundDepthGradientPerPx(point, centre, CAMERA, VIEWPORT);
    const depth = planeDepthAt(point, normal, 0, 0);
    const pulled = depth + GROUND_PULL_PX * Math.hypot(gradient.perXPx, gradient.perYPx);
    expect(
      [0, -0.5, -1, -CASED_REACH_PX].map((px) => groundMarkDepth(depth, gradient, along(DOWN, px))),
    ).toEqual([pulled, pulled, pulled, pulled]);
  });

  it.each([2, 10, 60])(
    "keeps a stroke on the ground at grazing %s° whole: each fragment in front of the ground beneath it",
    (grazingDeg) => {
      const rangeM = 1_000 / Math.sin(grazingDeg * RAD);
      const { point, normal, centre } = groundAt(rangeM, grazingDeg * RAD);
      const gradient = groundDepthGradientPerPx(point, centre, CAMERA, VIEWPORT);
      const depth = planeDepthAt(point, normal, 0, 0);
      const inFront = ACROSS_PX.map((px) => {
        // The ground beneath the fragment, pushed as the terrain's depth is.
        const beneath = planeDepthAt(point, normal, 0, px) * (1 - TERRAIN_DEPTH_FRACTION);
        return groundMarkDepth(depth, gradient, along(DOWN, px)) >= beneath;
      });
      expect(inFront).toEqual(ACROSS_PX.map(() => true));
    },
  );

  it("hides every fragment of an edge 287 km out from 10 km up at 2° behind a ridge 21.7 km nearer, which the old pull showed", () => {
    // The ridge's face, across the stroke's pixels, stands 21.7 km in front of the edge: beyond the
    // 20.6 km of the pull of its cased nearer half's edge, its pixel of slope and 3.5 px across,
    // and within the old pull's 22.7 km, 5 px of the slope at the views' stroke scale.
    const grazingRad = 2 * RAD;
    const rangeM = 10_000 / Math.sin(grazingRad);
    const { point, normal, centre } = groundAt(rangeM, grazingRad);
    const gradient = groundDepthGradientPerPx(point, centre, CAMERA, VIEWPORT);
    const depth = planeDepthAt(point, normal, 0, 0);
    const ridge = (NEAR_PLANE_M / (rangeM - 21_700)) * (1 - TERRAIN_DEPTH_FRACTION);
    const oldPull = VIEW_SLOPE_PX * Math.hypot(gradient.perXPx, gradient.perYPx);
    expect([
      ACROSS_PX.filter((px) => groundMarkDepth(depth, gradient, along(DOWN, px)) >= ridge),
      ACROSS_PX.filter(() => Math.min(depth + oldPull, 1) >= ridge),
    ]).toEqual([[], ACROSS_PX]);
  });

  describe("over a dip, from 2 km up at 17°, as TERRAIN TEST's 248 s look meets its edge across a valley", () => {
    const grazingRad = 17 * RAD;
    const rangeM = 2_000 / Math.sin(grazingRad);
    const datum = groundAt(rangeM, grazingRad);
    const gradient = groundDepthGradientPerPx(datum.point, datum.centre, CAMERA, VIEWPORT);
    const depth = planeDepthAt(datum.point, datum.normal, 0, 0);
    const pulled = ACROSS_PX.map((px) => groundMarkDepth(depth, gradient, along(DOWN, px)));

    it("keeps the stroke whole on the valley's floor, tilted 10° towards the camera against the datum", () => {
      const floor = groundAt(rangeM, grazingRad + 10 * RAD);
      const onFloor = ACROSS_PX.map(
        (px) => planeDepthAt(floor.point, floor.normal, 0, px) * (1 - TERRAIN_DEPTH_FRACTION),
      );
      expect(pulled.every((d, k) => d >= (onFloor[k] ?? 1))).toBe(true);
    });

    it("lets the valley's nearer rim, 300 m in front and 10 m above its line of sight, hide it", () => {
      const sight = normalise(datum.point);
      const crest = add(scale(sight, rangeM - 300), scale(datum.normal, 10));
      const rim = (NEAR_PLANE_M / -crest.z) * (1 - TERRAIN_DEPTH_FRACTION);
      expect(pulled.filter((d) => d >= rim)).toEqual([]);
    });
  });

  it("pulls no mark past the near plane", () => {
    expect(groundMarkDepth(0.999, { perXPx: 0, perYPx: 1 }, { xPx: 0, yPx: 1 })).toBe(1);
  });
});
