import { describe, expect, it } from "vitest";

import { cross, dot, vec3, type Vec3 } from "../../geometry/vec3";
import {
  DEFAULT_FOV_DEG,
  FOV_STEPS_DEG,
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  pixelSolidAngle,
  project,
  type ProjectionCamera,
  toViewAxes,
  viewRotation,
  viewRotation4,
  type Viewport,
} from "./projection";
import { IDENTITY_QUATERNION, quaternion, quaternionFromAxisAngle, rotate } from "./quaternion";

const AU_M = 1.495_978_707e11;
const FULL_HD: Viewport = { widthPx: 1920, heightPx: 1080 };
const SIXTY = (60 * Math.PI) / 180;
const LOOKING_DOWN_MINUS_Z: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: SIXTY };

/** The column-major matrix `m` applied to (x, y, z, w), each product and sum rounded to `f32`. */
function applyF32(m: Float32Array, v: readonly [number, number, number, number]): number[] {
  const out: number[] = [];
  for (let row = 0; row < 4; row += 1) {
    let sum = 0;
    for (let col = 0; col < 4; col += 1) {
      sum = Math.fround(
        sum + Math.fround((m[col * 4 + row] ?? Number.NaN) * (v[col] ?? Number.NaN)),
      );
    }
    out.push(sum);
  }
  return out;
}

/** Pixel x of a clip-space position on {@link FULL_HD}. */
function pixelX(clip: readonly number[]): number {
  return (((clip[0] ?? Number.NaN) / (clip[3] ?? Number.NaN) + 1) / 2) * FULL_HD.widthPx;
}

describe("perspectiveReversedInfinite", () => {
  it("is Design note 4's matrix at 60° and 16:9, by hand values", () => {
    // s = 1 ÷ tan 30° = √3 and s·a = 16√3 ÷ 9, column-major.
    const m = [...perspectiveReversedInfinite(SIXTY, 16 / 9, NEAR_PLANE_M)];
    const expected = [1.7320508, 0, 0, 0, 0, 3.0792014, 0, 0, 0, 0, 0, -1, 0, 0, 0.1, 0];
    expect(m.map((x, i) => Math.abs(x - (expected[i] ?? Number.NaN)))).toEqual(
      expected.map(() => expect.closeTo(0, 6)),
    );
  });

  it("puts the near plane at depth 1 and a point 1 au out at 6.7e-13", () => {
    const m = perspectiveReversedInfinite(SIXTY, 16 / 9, NEAR_PLANE_M);
    const depthAt = (distanceM: number): number => {
      const clip = applyF32(m, [0, 0, -distanceM, 1]);
      return (clip[2] ?? Number.NaN) / (clip[3] ?? Number.NaN);
    };
    expect(depthAt(NEAR_PLANE_M)).toBeCloseTo(1, 6);
    expect(Math.abs(depthAt(AU_M) / (NEAR_PLANE_M / AU_M) - 1)).toBeLessThanOrEqual(1e-6);
    expect(NEAR_PLANE_M / AU_M).toBeCloseTo(6.7e-13, 14);
  });

  it("refuses a field of view outside (0, π)", () => {
    expect(() => perspectiveReversedInfinite(Math.PI, 1, NEAR_PLANE_M)).toThrow(RangeError);
  });
});

/** The pixel a camera-frame point reaches through `view` then `projection`, all in `f32`. */
function pixelThroughF32(
  view: Float32Array,
  projection: Float32Array,
  p: readonly number[],
): number {
  const eye = applyF32(view, [p[0] ?? Number.NaN, p[1] ?? Number.NaN, p[2] ?? Number.NaN, 1]);
  const clip = applyF32(projection, [
    eye[0] ?? Number.NaN,
    eye[1] ?? Number.NaN,
    eye[2] ?? Number.NaN,
    eye[3] ?? Number.NaN,
  ]);
  return pixelX(clip);
}

describe("the rotation-only view", () => {
  // A turned camera 1 au from its frame's origin; the point 1 km ahead of it, 50 m right, 20 m up.
  const turned = quaternionFromAxisAngle(vec3(0.2, 1, 0.1), 0.7);
  const camera: ProjectionCamera = { orientation: turned, fovXRad: SIXTY };
  const cameraM: Vec3 = vec3(AU_M, 0, 0);
  const offsetM = rotate(turned, vec3(50, 20, -1_000));
  const pointM: Vec3 = vec3(cameraM.x + offsetM.x, cameraM.y + offsetM.y, cameraM.z + offsetM.z);
  const projectionM = perspectiveReversedInfinite(SIXTY, 16 / 9, NEAR_PLANE_M);
  const exact = project(offsetM, camera, FULL_HD).xPx;

  it("keeps a point 1 km from a camera 1 au out within 0.1 px", () => {
    const narrowed = [
      Math.fround(pointM.x - cameraM.x),
      Math.fround(pointM.y - cameraM.y),
      Math.fround(pointM.z - cameraM.z),
    ];
    expect(
      Math.abs(pixelThroughF32(viewRotation4(turned), projectionM, narrowed) - exact),
    ).toBeLessThanOrEqual(0.1);
  });

  it("errs by more than a pixel with the camera's translation composed into an f32 matrix", () => {
    // The naive view matrix: the rotation with a translation column −Rᵀ·camera, and the point in
    // world coordinates, both in f32, where an f32 at 1 au is spaced at 16 km.
    const naive = new Float32Array(viewRotation4(turned));
    const shift = toViewAxes(cameraM, turned);
    naive[12] = -shift.x;
    naive[13] = -shift.y;
    naive[14] = -shift.z;
    const world = [Math.fround(pointM.x), Math.fround(pointM.y), Math.fround(pointM.z)];
    expect(Math.abs(pixelThroughF32(naive, projectionM, world) - exact)).toBeGreaterThan(1);
  });

  it("gives the GPU the transpose of the orientation, column-major", () => {
    const axes = [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, -1)];
    const m = viewRotation4(turned);
    for (const axis of axes) {
      const inFrame = rotate(turned, axis);
      const [x, y, z] = applyF32(m, [inFrame.x, inFrame.y, inFrame.z, 0]);
      expect(
        [x, y, z].map((c, k) =>
          Math.abs((c ?? Number.NaN) - ([axis.x, axis.y, axis.z][k] ?? Number.NaN)),
        ),
      ).toEqual([0, 0, 0].map(() => expect.closeTo(0, 6)));
    }
    expect([m[12], m[13], m[14], m[15]]).toEqual([0, 0, 0, 1]);
  });

  it("has a right-handed basis", () => {
    const m = viewRotation(quaternion(0.3, -0.5, 0.7, 0.1));
    const column = (j: number): Vec3 =>
      vec3(m[j * 3] ?? Number.NaN, m[j * 3 + 1] ?? Number.NaN, m[j * 3 + 2] ?? Number.NaN);
    expect(dot(cross(column(0), column(1)), column(2))).toBeCloseTo(1, 6);
  });
});

describe("project", () => {
  const tanX = Math.tan(SIXTY / 2);

  it("puts the right edge of the field of view at the right edge of the view", () => {
    expect(project(vec3(tanX, 0, -1), LOOKING_DOWN_MINUS_Z, FULL_HD).xPx).toBeCloseTo(1920, 9);
    expect(project(vec3(-tanX, 0, -1), LOOKING_DOWN_MINUS_Z, FULL_HD).xPx).toBeCloseTo(0, 9);
  });

  it("puts up at the top, with square pixels", () => {
    const top = project(vec3(0, tanX * (1080 / 1920), -1), LOOKING_DOWN_MINUS_Z, FULL_HD);
    expect(top.yPx).toBeCloseTo(0, 9);
    expect(top.depth).toBeCloseTo(NEAR_PLANE_M, 12);
  });

  it("says a point behind the camera is not in front", () => {
    expect(project(vec3(0, 0, 5), LOOKING_DOWN_MINUS_Z, FULL_HD).inFront).toBe(false);
    expect(project(vec3(0, 0, -5), LOOKING_DOWN_MINUS_Z, FULL_HD).inFront).toBe(true);
  });
});

describe("pixelSolidAngle", () => {
  it("gives a 1920 px, 60° view's centre pixel 3.62e-7 sr", () => {
    const omega = pixelSolidAngle(vec3(0, 0, -1), LOOKING_DOWN_MINUS_Z, FULL_HD);
    expect(Math.abs(omega / 3.62e-7 - 1)).toBeLessThanOrEqual(0.005);
  });

  it("gives a 16:9 corner pixel, 33.5° off the axis, 0.580 of the centre's", () => {
    const tanX = Math.tan(SIXTY / 2);
    const corner = vec3(tanX, tanX * (9 / 16), -1);
    const centre = pixelSolidAngle(vec3(0, 0, -1), LOOKING_DOWN_MINUS_Z, FULL_HD);
    const ratio = pixelSolidAngle(corner, LOOKING_DOWN_MINUS_Z, FULL_HD) / centre;
    // cos³ 33.5° = 0.5794, which the plan states as 0.580.
    expect(Math.abs(ratio / 0.58 - 1)).toBeLessThanOrEqual(0.002);
  });
});

describe("the field of view steps", () => {
  it("default to 60° among 10° to 120°", () => {
    expect(FOV_STEPS_DEG).toContain(DEFAULT_FOV_DEG);
    expect([FOV_STEPS_DEG[0], FOV_STEPS_DEG.at(-1)]).toEqual([10, 120]);
  });
});
