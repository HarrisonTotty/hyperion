import { describe, expect, it } from "vitest";

import { norm, sub, vec3 } from "../../geometry/vec3";
import {
  aCameraPose,
  aCameraScene,
  FIXTURE_PLANET,
  FIXTURE_PLANET_CENTRE_M,
  FIXTURE_SHIP,
  FIXTURE_SYSTEM,
  FIXTURE_TIDAL_RADIUS_M,
} from "../../test/viewFixtures";
import type { ViewPosition } from "../coords/position";
import { relativeToCamera } from "../coords/relative";
import {
  changeFreeRate,
  type FreeCameraInput,
  freeRateMPerS,
  maxFreeRateStep,
  NO_FREE_INPUT,
  stepFreeCamera,
} from "./freeCamera";
import { rebase } from "./rebase";
import { type CameraState, cutTo, newCameraState } from "./state";

/** A free camera, no own ship, at `positionM` in the system frame. */
function freeAt(positionM = vec3(0, 1e8, 0)): {
  state: CameraState;
  scene: ReturnType<typeof aCameraScene>;
} {
  const scene = aCameraScene({ ownShip: null, defaultPose: aCameraPose({ positionM }) });
  return { state: newCameraState(scene, "camera"), scene };
}

/** The input at time `tS` of a fixed two-second sequence: forward and yaw, then roll and climb. */
function scripted(tS: number): FreeCameraInput {
  if (tS < 1) {
    return { translate: vec3(0, 0, -1), rotate: vec3(0, 0.5, 0) };
  }
  if (tS < 1.5) {
    return NO_FREE_INPUT;
  }
  return { translate: vec3(0.3, 1, 0), rotate: vec3(0.2, 0, 1) };
}

function fly(hz: number, seconds: number): CameraState {
  let { state, scene } = freeAt();
  const frames = Math.round(seconds * hz);
  for (let frame = 0; frame < frames; frame += 1) {
    state = stepFreeCamera(state, scripted(frame / hz), 1 / hz, false, scene).state;
  }
  return state;
}

describe("stepFreeCamera", () => {
  it("gives one pose for one input sequence at 60 Hz and at 144 Hz", () => {
    const at60 = fly(60, 2.5);
    const at144 = fly(144, 2.5);
    const { scene } = freeAt();
    const start = aCameraPose();
    const travelled = norm(
      sub(rebase(at60.pose, start.frame, scene.origins).pose.positionM, start.positionM),
    );
    expect(travelled).toBeGreaterThan(100);
    const apart = rebase(at144.pose, at60.pose.frame, scene.origins).pose.positionM;
    expect(norm(sub(apart, at60.pose.positionM))).toBeLessThan(1e-6 * travelled);
    const q = at60.pose.orientation;
    const r = at144.pose.orientation;
    expect(Math.abs(q.w * r.w + q.x * r.x + q.y * r.y + q.z * r.z)).toBeGreaterThan(1 - 1e-12);
  });

  it("under reduced motion moves at the commanded rate from the first frame and stops with the input", () => {
    const { state, scene } = freeAt();
    const forward: FreeCameraInput = { translate: vec3(0, 0, -1), rotate: vec3(0, 0, 0) };
    const first = stepFreeCamera(state, forward, 1 / 60, true, scene).state;
    const rate = freeRateMPerS(state.free.rateStep);
    expect(state.pose.positionM.z - first.pose.positionM.z).toBeCloseTo(rate / 60, 6);
    const stopped = stepFreeCamera(first, NO_FREE_INPUT, 1 / 60, true, scene).state;
    expect(stopped.pose.positionM).toEqual(first.pose.positionM);
  });

  it("ramps and damps the motion without reduced motion", () => {
    const { state, scene } = freeAt();
    const forward: FreeCameraInput = { translate: vec3(0, 0, -1), rotate: vec3(0, 0, 0) };
    const first = stepFreeCamera(state, forward, 1 / 60, false, scene).state;
    const rate = freeRateMPerS(state.free.rateStep);
    const moved = state.pose.positionM.z - first.pose.positionM.z;
    expect(moved).toBeGreaterThan(0);
    expect(moved).toBeLessThan((0.1 * rate) / 60);
    const coasting = stepFreeCamera(first, NO_FREE_INPUT, 1 / 60, false, scene).state;
    expect(coasting.pose.positionM.z).toBeLessThan(first.pose.positionM.z);
  });

  it("holds the camera within the system's tidal radius", () => {
    let { state, scene } = freeAt(vec3(0, 0, -0.95 * FIXTURE_TIDAL_RADIUS_M));
    state = changeFreeRate(state, 100, scene);
    for (let frame = 0; frame < 120; frame += 1) {
      state = stepFreeCamera(
        state,
        { translate: vec3(0, 0, -1), rotate: vec3(0, 0, 0) },
        1 / 60,
        true,
        scene,
      ).state;
    }
    const inSystem = rebase(
      state.pose,
      { kind: "system", system: FIXTURE_SYSTEM },
      scene.origins,
    ).pose;
    expect(norm(inSystem.positionM)).toBeLessThanOrEqual(FIXTURE_TIDAL_RADIUS_M * (1 + 1e-12));
    expect(norm(inSystem.positionM)).toBeGreaterThan(0.999 * FIXTURE_TIDAL_RADIUS_M);
  });

  it("re-selects its frame as it enters a body's Hill sphere, keeping the view of a landmark", () => {
    // 1.4 × 10⁹ m from the planet: inside its Hill sphere (1.5 × 10⁹) but outside the entry band.
    const startM = vec3(FIXTURE_PLANET_CENTRE_M.x - 1.4e9, 0, 0);
    let { state, scene } = freeAt(startM);
    expect(state.pose.frame.kind).toBe("system");
    const landmark: ViewPosition = { kind: "body", body: FIXTURE_PLANET, m: vec3(-6.371e6, 0, 0) };
    // Face +x, towards the planet, and fly at 10⁸ m/s.
    state = {
      ...state,
      pose: { ...state.pose, orientation: { w: Math.SQRT1_2, x: 0, y: -Math.SQRT1_2, z: 0 } },
    };
    state = changeFreeRate(state, 16 - state.free.rateStep, scene);
    const travelM = freeRateMPerS(state.free.rateStep) / 60;
    // How far the landmark's vector strays from the frame's own travel, on each frame change.
    const strays: number[] = [];
    for (let frame = 0; frame < 60; frame += 1) {
      const before = relativeToCamera(landmark, state.pose, scene.origins);
      const step = stepFreeCamera(
        state,
        { translate: vec3(0, 0, -1), rotate: vec3(0, 0, 0) },
        1 / 60,
        true,
        scene,
      );
      const after = relativeToCamera(landmark, step.state.pose, scene.origins);
      if (step.change !== null) {
        strays.push(Math.abs(norm(sub(after, before)) - travelM));
      }
      state = step.state;
    }
    expect(strays).toHaveLength(1);
    expect(Math.max(...strays)).toBeLessThan(1e-3);
    expect(state.pose.frame).toEqual({ kind: "body", body: FIXTURE_PLANET });
  });
});

describe("stepFreeCamera's other cases", () => {
  const RIGHT: FreeCameraInput = { translate: vec3(1, 0, 0), rotate: vec3(0, 0, 0) };

  it("leaves a camera in a preset other than free unchanged", () => {
    const scene = aCameraScene();
    const seat = newCameraState(scene, "eye");
    expect(stepFreeCamera(seat, RIGHT, 1 / 60, false, scene).state).toBe(seat);
  });

  it("keeps a camera orbiting a target craft in that craft's frame as it flies", () => {
    const scene = aCameraScene();
    const orbiting = cutTo(
      newCameraState(scene, "camera"),
      { kind: "target", target: { kind: "craft", craft: FIXTURE_SHIP } },
      scene,
      { easedMoves: false, reducedMotion: false },
    );
    if (orbiting.kind !== "cut") {
      throw new Error("expected a cut");
    }
    const free = cutTo(orbiting.state, { kind: "preset", preset: "free" }, scene, {
      easedMoves: false,
      reducedMotion: false,
    });
    if (free.kind !== "cut") {
      throw new Error("expected a cut");
    }
    const flown = stepFreeCamera(free.state, RIGHT, 1, true, scene).state;
    expect(flown.pose.frame).toEqual({ kind: "craft", craft: FIXTURE_SHIP });
  });

  it("stops rotating on the frame the input stops under reduced motion", () => {
    const { state, scene } = freeAt();
    const yaw: FreeCameraInput = { translate: vec3(0, 0, 0), rotate: vec3(0, 1, 0) };
    const turned = stepFreeCamera(state, yaw, 1 / 60, true, scene).state;
    const stopped = stepFreeCamera(turned, NO_FREE_INPUT, 1 / 60, true, scene).state;
    expect(stopped.pose.orientation).toEqual(turned.pose.orientation);
  });

  it("comes to rest after coasting", () => {
    const { state, scene } = freeAt();
    let coasting = stepFreeCamera(state, RIGHT, 0.25, false, scene).state;
    for (let frame = 0; frame < 600; frame += 1) {
      coasting = stepFreeCamera(coasting, NO_FREE_INPUT, 1 / 60, false, scene).state;
    }
    const rested = stepFreeCamera(coasting, NO_FREE_INPUT, 1 / 60, false, scene).state;
    expect(rested.pose).toBe(coasting.pose);
  });
});

describe("the commanded rate", () => {
  it("steps on a logarithmic scale from 1 m/s to a tenth of the system across in a second", () => {
    const { state, scene } = freeAt();
    expect(freeRateMPerS(0)).toBe(1);
    expect(freeRateMPerS(2)).toBe(10);
    expect(changeFreeRate(state, -100, scene).free.rateStep).toBe(0);
    const top = changeFreeRate(state, 100, scene).free.rateStep;
    expect(top).toBe(maxFreeRateStep(scene));
    expect(freeRateMPerS(top)).toBeLessThanOrEqual(0.2 * FIXTURE_TIDAL_RADIUS_M);
    expect(freeRateMPerS(top + 1)).toBeGreaterThan(0.2 * FIXTURE_TIDAL_RADIUS_M);
  });
});
