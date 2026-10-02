import { METRES_PER_LIGHT_YEAR } from "@hyperion/protocol";

import { add, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { clampToTidalRadius } from "./frames";
import type { CameraPose } from "./pose";
import { multiply, quaternionFromAxisAngle, rotate } from "./quaternion";
import { type FrameChange, rebase } from "./rebase";
import { type CameraScene, type CameraState, rebaseState, sceneFrameFor } from "./state";

/**
 * The operator's commands to a free camera at one moment, each component in [−1, 1] along the
 * camera's own axes.
 *
 * @remarks
 * `translate`: +x to the right, +y up, −z forward. `rotate`: about +x (pitch up), +y (yaw left) and
 * +z (roll left), right-handed.
 */
export interface FreeCameraInput {
  /** The commanded translation, a fraction of the commanded rate per axis. */
  readonly translate: Vec3;
  /** The commanded rotation, a fraction of {@link FREE_ROTATION_RATE_DEG_PER_S} per axis. */
  readonly rotate: Vec3;
}

/** No command: the camera coasts to a stop (or stops at once under reduced motion). */
export const NO_FREE_INPUT: FreeCameraInput = { translate: vec3(0, 0, 0), rotate: vec3(0, 0, 0) };

/**
 * The free camera's integration step, s: 1/1440.
 *
 * @remarks
 * Display frames are integrated as whole ticks, the remainder carried to the next frame, so that
 * one input sequence gives one path at any display rate: 1/1440 s divides both the 60 Hz and the
 * 144 Hz frame (24 and 10 ticks), and at any other rate the pose lags by less than a tick.
 */
export const FREE_CAMERA_TICK_S = 1 / 1440;

/**
 * The longest frame integrated, s: 0.25, so that a view resumed after a stall does not leap. A
 * choice of this plan: four frames at 15 fps, far beyond any frame the view should take.
 */
export const MAX_FREE_STEP_S = 0.25;

/**
 * The time constant of the acceleration ramp and damping, s: 0.25.
 *
 * @remarks
 * A choice of this plan. The rate approaches the command as 1 − e^(−t ÷ τ), which is the
 * non-physical smoothing that reduced motion removes (Design note 18).
 */
export const FREE_CAMERA_SMOOTHING_S = 0.25;

/**
 * The free camera's full rotation rate, degrees a second, about each axis: 45.
 *
 * @remarks
 * A choice of this plan: a quarter-turn in two seconds, slow enough to track a mark by key and fast
 * enough to look behind in four.
 */
export const FREE_ROTATION_RATE_DEG_PER_S = 45;

/**
 * The fraction of the commanded rate below which a coasting camera, with no command, is at rest.
 *
 * @remarks
 * Exponential damping never reaches zero; without this a camera left to coast would move by
 * ever-smaller amounts, and be re-clamped and re-selected, on every frame.
 */
export const FREE_REST_FRACTION = 1e-6;

/** The fraction of the system's diameter the fastest rate crosses in a second: a tenth. */
export const FREE_RATE_SYSTEM_FRACTION = 0.1;

/**
 * The fastest rate in a galactic scene, which has no tidal radius, m/s: a tenth of a light-year a
 * second. A choice of this plan.
 */
export const FREE_RATE_GALACTIC_MAX_M_PER_S = METRES_PER_LIGHT_YEAR / 10;

/**
 * The commanded translation rate at a step, m/s: 10^(step ÷ 2), two steps a decade.
 *
 * @remarks
 * From 1 m/s at step 0 to a tenth of the system across in a second at the top (R02.T9.b: one
 * control spans a metre a second to a tenth of the system across in seconds).
 */
export function freeRateMPerS(step: number): number {
  return 10 ** (step / 2);
}

/** The highest rate step a scene allows: a tenth of the system across in a second. */
export function maxFreeRateStep(scene: CameraScene): number {
  const maxMPerS =
    scene.tidalRadiusM === null
      ? FREE_RATE_GALACTIC_MAX_M_PER_S
      : FREE_RATE_SYSTEM_FRACTION * 2 * scene.tidalRadiusM;
  return Math.max(0, Math.floor(2 * Math.log10(maxMPerS)));
}

/** The camera with its commanded rate moved `steps` steps up or down, held within the scene's range. */
export function changeFreeRate(state: CameraState, steps: number, scene: CameraScene): CameraState {
  const rateStep = Math.min(maxFreeRateStep(scene), Math.max(0, state.free.rateStep + steps));
  return { ...state, free: { ...state.free, rateStep } };
}

/** One free camera tick's result. */
export interface FreeStep {
  /** The camera after the step. */
  readonly state: CameraState;
  /** The frame change the step made, or `null`; every cached camera-relative quantity is dropped. */
  readonly change: FrameChange | null;
}

/** `current` moved towards `commanded`: at once under reduced motion, else by the ramp's factor. */
function approach(current: Vec3, commanded: Vec3, factor: number): Vec3 {
  return add(current, scale(sub(commanded, current), factor));
}

/**
 * The camera held within the system's tidal radius: re-expressed in the system frame and pulled
 * back where it is outside (Design note 7), in its own frame otherwise.
 */
function clampPose(pose: CameraPose, scene: CameraScene): CameraPose {
  if (scene.system === null || scene.tidalRadiusM === null) {
    return pose;
  }
  const inSystem = rebase(pose, { kind: "system", system: scene.system }, scene.origins).pose;
  const clampedM = clampToTidalRadius(inSystem.positionM, scene.tidalRadiusM);
  if (clampedM === inSystem.positionM) {
    return pose;
  }
  return rebase({ ...inSystem, positionM: clampedM }, pose.frame, scene.origins).pose;
}

/**
 * Integrates a free camera over one display frame (plan R02, R02.T9.b).
 *
 * @remarks
 * Translation is commanded at the camera's rate on its logarithmic scale and rotation at
 * {@link FREE_ROTATION_RATE_DEG_PER_S}, both along the camera's axes, in whole ticks of
 * {@link FREE_CAMERA_TICK_S}. Without reduced motion the rates ramp towards the command and damp
 * to rest when it stops; under reduced motion they are the command from the first tick and zero on
 * the frame the input stops (Design note 18). After the frame the camera is held within the
 * system's tidal radius and its frame re-selected by the body-frame rule, rebased in one operation
 * when it changes; a camera orbiting a target craft stays in that craft's frame. A camera not in
 * `free` is returned unchanged.
 *
 * @param dtS - The frame's duration, s, not negative; at most {@link MAX_FREE_STEP_S} is integrated.
 */
export function stepFreeCamera(
  state: CameraState,
  input: FreeCameraInput,
  dtS: number,
  reducedMotion: boolean,
  scene: CameraScene,
): FreeStep {
  if (state.preset !== "free") {
    return { state, change: null };
  }
  const totalS = state.free.pendingS + Math.min(Math.max(dtS, 0), MAX_FREE_STEP_S);
  // The small allowance keeps a frame of exactly 24 or 10 ticks from losing one to rounding.
  const ticks = Math.floor(totalS / FREE_CAMERA_TICK_S + 1e-6);
  const pendingS = Math.max(0, totalS - ticks * FREE_CAMERA_TICK_S);
  const rateMPerS = freeRateMPerS(state.free.rateStep);
  const commandedV = scale(input.translate, rateMPerS);
  const rotationRateRadPerS = (FREE_ROTATION_RATE_DEG_PER_S * Math.PI) / 180;
  const commandedW = scale(input.rotate, rotationRateRadPerS);
  const factor = reducedMotion ? 1 : 1 - Math.exp(-FREE_CAMERA_TICK_S / FREE_CAMERA_SMOOTHING_S);
  let velocity = state.free.velocityMPerS;
  let angular = state.free.angularRateRadPerS;
  let { positionM, orientation } = state.pose;
  for (let tick = 0; tick < ticks; tick += 1) {
    velocity = approach(velocity, commandedV, factor);
    angular = approach(angular, commandedW, factor);
    const angleRad = norm(angular) * FREE_CAMERA_TICK_S;
    if (angleRad > 0) {
      orientation = multiply(orientation, quaternionFromAxisAngle(angular, angleRad));
    }
    if (norm(velocity) > 0) {
      positionM = add(positionM, scale(rotate(orientation, velocity), FREE_CAMERA_TICK_S));
    }
  }
  if (norm(commandedV) === 0 && norm(velocity) < FREE_REST_FRACTION * rateMPerS) {
    velocity = vec3(0, 0, 0);
  }
  if (norm(commandedW) === 0 && norm(angular) < FREE_REST_FRACTION * rotationRateRadPerS) {
    angular = vec3(0, 0, 0);
  }
  if (positionM === state.pose.positionM && orientation === state.pose.orientation) {
    // At rest: nothing moved, so there is nothing to clamp and no frame to re-select.
    const free = { ...state.free, velocityMPerS: velocity, angularRateRadPerS: angular, pendingS };
    return { state: { ...state, free }, change: null };
  }
  const moved = clampPose({ frame: state.pose.frame, positionM, orientation }, scene);
  const frame = moved.frame.kind === "craft" ? moved.frame : sceneFrameFor(moved, scene);
  const { pose, change } = rebase(moved, frame, scene.origins);
  const stepped: CameraState = {
    ...state,
    free: { ...state.free, velocityMPerS: velocity, angularRateRadPerS: angular, pendingS },
  };
  return { state: rebaseState(stepped, pose, change, scene.origins), change };
}
