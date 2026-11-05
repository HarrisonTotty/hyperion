import { describe, expect, it } from "vitest";

import { dot, norm, normalise, vec3 } from "../../geometry/vec3";
import { rotation3FromRows } from "../coords/rotation";
import {
  coRotated,
  orbitRadiusM,
  ORBIT_CEILING_RADII,
  ORBIT_FLOOR_M,
  ORBIT_START_RADII,
  ORBIT_ZOOM_FACTOR,
  orbitAltitudeM,
  orbitDragged,
  orbitLooked,
  orbitStart,
  orbitTiltRad,
  orbitZoomed,
} from "./orbit";
import type { CameraPose } from "./pose";
import { lookAlong, rotate } from "./quaternion";

/** An Earth's equatorial radius, m. */
const RADIUS_M = 6.378e6;

const FRAME: CameraPose["frame"] = { kind: "body", body: "0200080020000000.0300" };

/** A camera on +x at `distanceM`, looking at the centre, galactic north up. */
function onX(distanceM: number): CameraPose {
  return {
    frame: FRAME,
    positionM: vec3(distanceM, 0, 0),
    orientation: lookAlong(vec3(-1, 0, 0), vec3(0, 0, 1)),
  };
}

/** The angle of a camera's place from +x about the centre, rad. */
function angle(pose: CameraPose): number {
  return Math.acos(dot(normalise(pose.positionM), vec3(1, 0, 0)));
}

/** A spin of `angleRad` about +z, from body-fixed to body axes. */
function quarter(angleRad: number) {
  return rotation3FromRows([
    vec3(Math.cos(angleRad), -Math.sin(angleRad), 0),
    vec3(Math.sin(angleRad), Math.cos(angleRad), 0),
    vec3(0, 0, 1),
  ]);
}

function forwardOf(pose: CameraPose) {
  return rotate(pose.orientation, vec3(0, 0, -1));
}

describe("the ORBIT camera", () => {
  it("starts where it stands, looking at the centre, or out at three radii from inside", () => {
    const start = orbitStart(onX(2 * RADIUS_M), FRAME, RADIUS_M);
    expect(norm(start.positionM)).toBeCloseTo(2 * RADIUS_M, 3);
    expect(dot(forwardOf(start), vec3(-1, 0, 0))).toBeCloseTo(1, 12);
    const inside = orbitStart(onX(0.5 * RADIUS_M), FRAME, RADIUS_M);
    expect(norm(inside.positionM)).toBeCloseTo(ORBIT_START_RADII * RADIUS_M, 3);
  });

  it("zooms the altitude by its factor a step, between its floor and its ceiling", () => {
    const pose = onX(RADIUS_M + 1e6);
    const nearer = orbitZoomed(pose, 1, RADIUS_M);
    expect(orbitAltitudeM(nearer, RADIUS_M)).toBeCloseTo(1e6 / ORBIT_ZOOM_FACTOR, 3);
    const floor = orbitZoomed(pose, 200, RADIUS_M);
    expect(orbitAltitudeM(floor, RADIUS_M)).toBeCloseTo(ORBIT_FLOOR_M, 6);
    const ceiling = orbitZoomed(pose, -200, RADIUS_M);
    expect(norm(ceiling.positionM)).toBeCloseTo(ORBIT_CEILING_RADII * RADIUS_M, 0);
    // The look is kept.
    expect(nearer.orientation).toEqual(pose.orientation);
  });

  it("swings round the body far out and pans over the ground close in, its look with it", () => {
    const turn = { yawRad: 0.1, pitchRad: 0 };
    const far = orbitDragged(onX(100 * RADIUS_M), turn, RADIUS_M);
    const near = orbitDragged(onX(RADIUS_M + 1000), turn, RADIUS_M);
    expect(angle(far)).toBeCloseTo(0.1 * 0.99, 3);
    expect(angle(near)).toBeCloseTo((0.1 * 1000) / (RADIUS_M + 1000), 9);
    // Still looking at the centre, at its distance.
    expect(dot(forwardOf(far), normalise(far.positionM))).toBeCloseTo(-1, 12);
    expect(norm(near.positionM)).toBeCloseTo(RADIUS_M + 1000, 3);
  });

  it("tilts towards the horizon and no further, and turns its heading about the vertical", () => {
    const pose = onX(RADIUS_M + 1000);
    expect(orbitTiltRad(pose)).toBeCloseTo(0, 9);
    const tilted = orbitLooked(pose, 0.5, 0);
    expect(orbitTiltRad(tilted)).toBeCloseTo(0.5, 9);
    const horizon = orbitLooked(pose, 3, 0);
    expect(orbitTiltRad(horizon)).toBeCloseTo(Math.PI / 2, 9);
    const turned = orbitLooked(tilted, 0, 1);
    expect(orbitTiltRad(turned)).toBeCloseTo(0.5, 9);
    expect(turned.positionM).toEqual(pose.positionM);
  });

  it("measures its altitude above an oblate datum, the polar radius over the pole", () => {
    const datum = { equatorialRadiusM: 6.3781e6, polarRadiusM: 6.3568e6, pole: vec3(0, 0, 1) };
    expect([
      orbitRadiusM(datum, vec3(0, 0, 2e7)),
      orbitRadiusM(datum, vec3(2e7, 0, 0)),
      orbitRadiusM({ ...datum, pole: null }, vec3(0, 0, 2e7)),
    ]).toEqual([
      expect.closeTo(6.3568e6, 3),
      expect.closeTo(6.3781e6, 3),
      expect.closeTo(6.3781e6, 3),
    ]);
  });

  it("turns with its body's spin, so that it hovers over one place of the ground", () => {
    const pose = onX(RADIUS_M + 1000);
    const turned = coRotated(pose, quarter(0), quarter(Math.PI / 2));
    expect(turned.positionM.x).toBeCloseTo(0, 3);
    expect(turned.positionM.y).toBeCloseTo(RADIUS_M + 1000, 3);
    expect(dot(forwardOf(turned), vec3(0, -1, 0))).toBeCloseTo(1, 12);
  });
});
