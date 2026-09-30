import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION, lookAlong } from "../camera/quaternion";
import { behindLimb, sphereInFrustum } from "./cull";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const EARTH_RADIUS_M = 6.371e6;

describe("sphereInFrustum", () => {
  it("keeps a sphere ahead and drops one behind or wholly off to a side", () => {
    expect([
      sphereInFrustum(vec3(0, 0, -1e9), 1e6, CAMERA, VIEWPORT),
      sphereInFrustum(vec3(0, 0, 1e9), 1e6, CAMERA, VIEWPORT),
      sphereInFrustum(vec3(1e9, 0, -1e8), 1e6, CAMERA, VIEWPORT),
    ]).toEqual([true, false, false]);
  });

  it("keeps a mark larger than the frustum", () => {
    // A sphere beside the camera, reaching far across the view.
    expect(sphereInFrustum(vec3(2e9, 0, 0), 1.9e9, CAMERA, VIEWPORT)).toBe(true);
  });

  it("keeps a body whose Hill sphere the camera is inside, though its centre is behind", () => {
    expect(sphereInFrustum(vec3(0, 0, 1e8), 1.5e9, CAMERA, VIEWPORT)).toBe(true);
  });

  it("keeps a body whose bounding sphere the camera is inside", () => {
    expect(sphereInFrustum(vec3(0, -6.3e6, 0), EARTH_RADIUS_M, CAMERA, VIEWPORT)).toBe(true);
  });
});

describe("behindLimb at grazing altitude", () => {
  // A camera 400 km above the surface, the body's centre straight below it.
  const altitudeM = 4e5;
  const centreM = vec3(0, -(EARTH_RADIUS_M + altitudeM), 0);
  const horizonDistanceM = Math.sqrt((EARTH_RADIUS_M + altitudeM) ** 2 - EARTH_RADIUS_M ** 2);
  const dip = Math.acos(EARTH_RADIUS_M / (EARTH_RADIUS_M + altitudeM));
  /** A point beyond the horizon along the grazing line of sight, raised or lowered by `offsetM`. */
  const beyond = (offsetM: number): ReturnType<typeof vec3> =>
    vec3(2 * horizonDistanceM * Math.cos(dip), -2 * horizonDistanceM * Math.sin(dip) + offsetM, 0);

  it("hides a mark just behind the limb", () => {
    expect(behindLimb(beyond(-1_000), centreM, EARTH_RADIUS_M)).toBe(true);
  });

  it("keeps a mark just above it", () => {
    expect(behindLimb(beyond(1_000), centreM, EARTH_RADIUS_M)).toBe(false);
  });

  it("keeps a mark in front of the limb", () => {
    expect(behindLimb(vec3(1e5, -1e5, 0), centreM, EARTH_RADIUS_M)).toBe(false);
  });

  it("keeps a mark on the near surface, such as a landing site", () => {
    // The sub-camera point and points around it on the surface.
    const onSurface = [0, 0.01, 0.1, 0.3].map((angle) =>
      vec3(
        EARTH_RADIUS_M * Math.sin(angle) * Math.cos(0.5),
        centreM.y + EARTH_RADIUS_M * Math.cos(angle),
        EARTH_RADIUS_M * Math.sin(angle) * Math.sin(0.5),
      ),
    );
    expect(onSurface.map((p) => behindLimb(p, centreM, EARTH_RADIUS_M))).toEqual([
      false,
      false,
      false,
      false,
    ]);
  });

  it("keeps a mark above the horizon in the frustum of a camera looking at it", () => {
    const camera: ProjectionCamera = {
      orientation: lookAlong(vec3(Math.cos(dip), -Math.sin(dip), 0), vec3(0, 1, 0)),
      fovXRad: Math.PI / 3,
    };
    expect(sphereInFrustum(beyond(1_000), 1, camera, VIEWPORT)).toBe(true);
  });
});
