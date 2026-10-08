import { describe, expect, it } from "vitest";

import { dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { aCameraScene, anOwnShip } from "../../test/viewFixtures";
import { turnFreeCamera } from "./freeCamera";
import { hasLookOffset, NO_LOOK_OFFSET, NO_TURN, type ViewTurn } from "./look";
import type { CameraPose } from "./pose";
import { quaternionFromAxisAngle, rotate } from "./quaternion";
import {
  type CameraState,
  CHASE_OFFSET_HULL_LENGTHS,
  cutTo,
  followPreset,
  newCameraState,
  turnLook,
} from "./state";

const DEG = Math.PI / 180;
const ATTITUDE = quaternionFromAxisAngle(normalise(vec3(0.3, 1, 0.2)), 0.7);
const SCENE = aCameraScene({ ownShip: anOwnShip({ attitude: ATTITUDE }) });
const HULL_UP = rotate(ATTITUDE, vec3(0, 1, 0));

function forwardOf(pose: CameraPose): Vec3 {
  return rotate(pose.orientation, vec3(0, 0, -1));
}

function upOf(pose: CameraPose): Vec3 {
  return rotate(pose.orientation, vec3(0, 1, 0));
}

function rightOf(pose: CameraPose): Vec3 {
  return rotate(pose.orientation, vec3(1, 0, 0));
}

function inPreset(preset: "seat" | "chase" | "free"): CameraState {
  const result = cutTo(newCameraState(SCENE, "camera"), { kind: "preset", preset }, SCENE, {
    easedMoves: false,
    reducedMotion: false,
  });
  if (result.kind !== "cut") {
    throw new Error("the preset was refused");
  }
  return result.state;
}

/** A camera turned by `turn` and followed, as a frame does it. */
function turned(state: CameraState, turn: ViewTurn): CameraState {
  return followPreset(turnLook(state, turn, SCENE), SCENE);
}

describe("a SEAT camera's look offset", () => {
  it("turns the seat where it stands, left for a positive yaw and up for a positive pitch", () => {
    const seat = inPreset("seat");
    const left = turned(seat, { yawRad: 30 * DEG, pitchRad: 0 });
    const up = turned(seat, { yawRad: 0, pitchRad: 20 * DEG });
    const hullLeft = rotate(ATTITUDE, vec3(-1, 0, 0));
    expect(left.pose.positionM).toEqual(seat.pose.positionM);
    expect(dot(forwardOf(left.pose), hullLeft)).toBeCloseTo(Math.sin(30 * DEG), 12);
    expect(dot(forwardOf(up.pose), HULL_UP)).toBeCloseTo(Math.sin(20 * DEG), 12);
  });

  it("never rolls the camera from the hull's up, however it is turned", () => {
    let seat = inPreset("seat");
    for (const turn of [
      { yawRad: 40 * DEG, pitchRad: 25 * DEG },
      { yawRad: -70 * DEG, pitchRad: 30 * DEG },
      { yawRad: 15 * DEG, pitchRad: -80 * DEG },
    ]) {
      seat = turned(seat, turn);
      expect(dot(rightOf(seat.pose), HULL_UP)).toBeCloseTo(0, 12);
      expect(dot(upOf(seat.pose), HULL_UP)).toBeGreaterThanOrEqual(-1e-12);
    }
  });

  it("holds the elevation where the line of sight would pass the vertical", () => {
    const seat = turned(inPreset("seat"), { yawRad: 0, pitchRad: 200 * DEG });
    expect(seat.offset.elevationRad).toBeCloseTo(90 * DEG, 12);
    expect(dot(forwardOf(seat.pose), HULL_UP)).toBeCloseTo(1, 12);
    const down = turned(seat, { yawRad: 0, pitchRad: -400 * DEG });
    expect(down.offset.elevationRad).toBeCloseTo(-90 * DEG, 12);
  });

  it("wraps the azimuth", () => {
    const seat = turned(inPreset("seat"), { yawRad: 270 * DEG, pitchRad: 0 });
    expect(seat.offset.azimuthRad).toBeCloseTo(-90 * DEG, 12);
  });

  it("is cleared by every cut, the preset's key pressed again among them", () => {
    const seat = turned(inPreset("seat"), { yawRad: 30 * DEG, pitchRad: 10 * DEG });
    const again = cutTo(seat, { kind: "preset", preset: "seat" }, SCENE, {
      easedMoves: false,
      reducedMotion: false,
    });
    const target = cutTo(seat, { kind: "target", target: SCENE.targets[0] ?? null }, SCENE, {
      easedMoves: false,
      reducedMotion: false,
    });
    expect([
      hasLookOffset(seat.offset),
      again.kind === "cut" && again.state.offset,
      target.kind === "cut" && target.state.offset,
    ]).toEqual([true, NO_LOOK_OFFSET, NO_LOOK_OFFSET]);
    expect(again.kind === "cut" ? forwardOf(again.state.pose) : null).toEqual(
      forwardOf(inPreset("seat").pose),
    );
  });

  it("is kept as the ship turns, the camera turning with the hull", () => {
    const seat = turned(inPreset("seat"), { yawRad: 30 * DEG, pitchRad: 0 });
    const roll = quaternionFromAxisAngle(vec3(0, 0, 1), 0.4);
    const rolled = aCameraScene({ ownShip: anOwnShip({ attitude: roll }) });
    const followed = followPreset(seat, rolled);
    expect(followed.offset).toEqual(seat.offset);
    expect(dot(rightOf(followed.pose), rotate(roll, vec3(0, 1, 0)))).toBeCloseTo(0, 12);
  });

  it("leaves a free camera and no turn alone", () => {
    const free = inPreset("free");
    expect(turnLook(free, { yawRad: 0.5, pitchRad: 0.5 }, SCENE)).toBe(free);
    const seat = inPreset("seat");
    expect(turnLook(seat, NO_TURN, SCENE)).toBe(seat);
  });
});

describe("a CHASE camera's orbit", () => {
  const distanceM = norm(scale(CHASE_OFFSET_HULL_LENGTHS, anOwnShip().lengthM));

  it("swings about the own ship at the chase distance, keeping it at the centre", () => {
    let chase = inPreset("chase");
    for (const turn of [
      { yawRad: 60 * DEG, pitchRad: 0 },
      { yawRad: 0, pitchRad: -40 * DEG },
      { yawRad: -150 * DEG, pitchRad: 70 * DEG },
    ]) {
      chase = turned(chase, turn);
      // The ship is at its craft frame's origin.
      const toShip = sub(vec3(0, 0, 0), chase.pose.positionM);
      expect(norm(chase.pose.positionM)).toBeCloseTo(distanceM, 9);
      expect(dot(normalise(toShip), forwardOf(chase.pose))).toBeCloseTo(1, 12);
      expect(dot(rightOf(chase.pose), HULL_UP)).toBeCloseTo(0, 12);
    }
  });

  it("turns the line of sight as the seat's does, so that the far scene follows the drag", () => {
    const chase = inPreset("chase");
    const left = turned(chase, { yawRad: 20 * DEG, pitchRad: 0 });
    // The line of sight turns left about the hull's up; the camera swings to starboard.
    const swung = dot(forwardOf(left.pose), rotate(ATTITUDE, vec3(-1, 0, 0)));
    expect(swung).toBeGreaterThan(0);
    expect(dot(left.pose.positionM, rotate(ATTITUDE, vec3(1, 0, 0)))).toBeGreaterThan(0);
  });

  it("holds the camera at the vertical over the ship, right way up", () => {
    const over = turned(inPreset("chase"), { yawRad: 0, pitchRad: -170 * DEG });
    expect(dot(forwardOf(over.pose), HULL_UP)).toBeCloseTo(-1, 12);
    expect(dot(normalise(over.pose.positionM), HULL_UP)).toBeCloseTo(1, 12);
    expect(dot(upOf(over.pose), HULL_UP)).toBeGreaterThanOrEqual(-1e-12);
  });

  it("holds the offset at 90° up from the chase view's own line of sight, under the ship", () => {
    const chaseElevationRad = Math.atan2(CHASE_OFFSET_HULL_LENGTHS.y, CHASE_OFFSET_HULL_LENGTHS.z);
    const under = turned(inPreset("chase"), { yawRad: 0, pitchRad: 170 * DEG });
    expect(under.offset.elevationRad).toBeCloseTo(90 * DEG, 12);
    expect(dot(forwardOf(under.pose), HULL_UP)).toBeCloseTo(Math.cos(chaseElevationRad), 12);
    expect(dot(upOf(under.pose), HULL_UP)).toBeGreaterThanOrEqual(-1e-12);
  });

  it("returns to the chase pose on 2, exactly", () => {
    const chase = turned(inPreset("chase"), { yawRad: 1, pitchRad: 0.3 });
    const back = cutTo(chase, { kind: "preset", preset: "chase" }, SCENE, {
      easedMoves: false,
      reducedMotion: false,
    });
    expect(back.kind === "cut" ? back.state.pose : null).toEqual(inPreset("chase").pose);
  });
});

describe("a FREE camera's drag", () => {
  it("turns the camera itself about its own axes, with no roll, and moves nothing", () => {
    const free = inPreset("free");
    const yawed = turnFreeCamera(free, { yawRad: 30 * DEG, pitchRad: 0 });
    const pitched = turnFreeCamera(free, { yawRad: 0, pitchRad: 30 * DEG });
    expect(yawed.pose.positionM).toEqual(free.pose.positionM);
    expect(dot(forwardOf(yawed.pose), rightOf(free.pose))).toBeCloseTo(-Math.sin(30 * DEG), 12);
    expect(dot(forwardOf(pitched.pose), upOf(free.pose))).toBeCloseTo(Math.sin(30 * DEG), 12);
    expect(dot(rightOf(yawed.pose), upOf(free.pose))).toBeCloseTo(0, 12);
    expect(dot(rightOf(pitched.pose), rightOf(free.pose))).toBeCloseTo(1, 12);
  });

  it("leaves a SEAT camera, and its offset, to turnLook", () => {
    const seat = inPreset("seat");
    expect(turnFreeCamera(seat, { yawRad: 1, pitchRad: 1 })).toBe(seat);
  });
});
