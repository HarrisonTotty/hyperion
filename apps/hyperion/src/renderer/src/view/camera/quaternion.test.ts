import { describe, expect, it } from "vitest";

import { cross, dot, norm, sub, vec3, type Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "./pose";
import {
  lookAlong,
  quaternionFromAxisAngle,
  quaternionFromRows,
  rotate,
  rotationRows,
  slerp,
} from "./quaternion";

/** 1 − |a · b|: zero when two unit quaternions are the same rotation (q and −q alike). */
function rotationGap(a: Quaternion, b: Quaternion): number {
  return 1 - Math.abs(a.w * b.w + a.x * b.x + a.y * b.y + a.z * b.z);
}

/** Where `q` turns the camera's forward axis. */
function turnOf(q: Quaternion): Vec3 {
  return rotate(q, vec3(0, 0, -1));
}

describe("quaternionFromRows", () => {
  it("recovers the rotation of every branch of Shepperd's method", () => {
    const cases = [
      quaternionFromAxisAngle(vec3(1, 2, 3), 0.7),
      quaternionFromAxisAngle(vec3(1, 0, 0), Math.PI),
      quaternionFromAxisAngle(vec3(0, 1, 0), Math.PI),
      quaternionFromAxisAngle(vec3(0, 0, 1), Math.PI),
      quaternionFromAxisAngle(vec3(1, 0.1, -0.2), 3),
    ];
    const gaps = cases.map((q) => rotationGap(quaternionFromRows(rotationRows(q)), q));
    expect(Math.max(...gaps)).toBeLessThan(1e-14);
  });
});

describe("lookAlong", () => {
  it("points the camera's −z along the direction with its +y towards up", () => {
    const forward = vec3(1, 2, -0.5);
    const up = vec3(0, 0, 1);
    const q = lookAlong(forward, up);
    const f = rotate(q, vec3(0, 0, -1));
    expect(
      norm(sub(f, vec3(1 / norm(forward), 2 / norm(forward), -0.5 / norm(forward)))),
    ).toBeLessThan(1e-14);
    const y = rotate(q, vec3(0, 1, 0));
    expect(dot(y, up)).toBeGreaterThan(0);
    expect(Math.abs(dot(cross(f, up), y))).toBeLessThan(1e-14);
  });

  it("finds an up axis when the hint is parallel to the direction", () => {
    const q = lookAlong(vec3(0, 0, 2), vec3(0, 0, 1));
    expect(norm(sub(rotate(q, vec3(0, 0, -1)), vec3(0, 0, 1)))).toBeLessThan(1e-14);
  });

  it("refuses a direction with no length", () => {
    expect(() => lookAlong(vec3(0, 0, 0), vec3(0, 1, 0))).toThrow(RangeError);
  });
});

describe("slerp", () => {
  const a = quaternionFromAxisAngle(vec3(0, 1, 0), 0);
  const b = quaternionFromAxisAngle(vec3(0, 1, 0), 1.2);

  it("runs from a at 0 to b at 1 through the midpoint at a half", () => {
    expect(rotationGap(slerp(a, b, 0), a)).toBeLessThan(1e-15);
    expect(rotationGap(slerp(a, b, 1), b)).toBeLessThan(1e-15);
    const half = quaternionFromAxisAngle(vec3(0, 1, 0), 0.6);
    expect(norm(sub(turnOf(slerp(a, b, 0.5)), turnOf(half)))).toBeLessThan(1e-14);
  });

  it("takes the shorter arc when b is given as its negative", () => {
    const negative = { w: -b.w, x: -b.x, y: -b.y, z: -b.z };
    const half = quaternionFromAxisAngle(vec3(0, 1, 0), 0.6);
    expect(norm(sub(turnOf(slerp(a, negative, 0.5)), turnOf(half)))).toBeLessThan(1e-14);
  });

  it("interpolates linearly between rotations too close for the arc formula", () => {
    const near = quaternionFromAxisAngle(vec3(0, 1, 0), 1e-9);
    expect(
      rotationGap(slerp(a, near, 0.5), quaternionFromAxisAngle(vec3(0, 1, 0), 5e-10)),
    ).toBeLessThan(1e-15);
  });
});
