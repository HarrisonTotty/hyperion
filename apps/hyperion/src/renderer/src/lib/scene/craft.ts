/**
 * A craft's path ahead (rendering plan R03, Design note 4): the flight computer's predicted path
 * where the craft has one, else a straight line, which R11's `couldTouch` sweeps.
 */
import {
  METRES_PER_LIGHT_YEAR,
  type GalacticPosition,
  type UniverseTime,
} from "@hyperion/protocol";

import { add, scale, type Vec3 } from "../../geometry/vec3";
import type { SceneCraft, SceneKinematics, ScenePosition } from "./model";

/** A pose on a craft's path: a position in a frame, its velocity and its time. */
export type CraftPose = SceneKinematics;

const NANOS_PER_SECOND = 1_000_000_000;

/** `time` plus `seconds`, a whole number of nanoseconds from the epoch, as `UniverseTime` keeps. */
function later(time: UniverseTime, seconds: number): UniverseTime {
  const whole = Math.floor(seconds);
  const nanos = time.nanos + Math.round((seconds - whole) * NANOS_PER_SECOND);
  const carry = Math.floor(nanos / NANOS_PER_SECOND);
  return { seconds: time.seconds + whole + carry, nanos: nanos - carry * NANOS_PER_SECOND };
}

/** One axis of a galactic position moved by `deltaM`, its offset kept in `[0, 1 ly)`. */
function moveAxis(cellLy: number, offsetM: number, deltaM: number): readonly [number, number] {
  const movedM = offsetM + deltaM;
  const cells = Math.floor(movedM / METRES_PER_LIGHT_YEAR);
  return [cellLy + cells, movedM - cells * METRES_PER_LIGHT_YEAR];
}

function moveGalactic(position: GalacticPosition, deltaM: Vec3): GalacticPosition {
  const [xCell, xOffset] = moveAxis(position.cell_ly[0], position.offset_m[0], deltaM.x);
  const [yCell, yOffset] = moveAxis(position.cell_ly[1], position.offset_m[1], deltaM.y);
  const [zCell, zOffset] = moveAxis(position.cell_ly[2], position.offset_m[2], deltaM.z);
  return { cell_ly: [xCell, yCell, zCell], offset_m: [xOffset, yOffset, zOffset] };
}

/** `pose` carried `seconds` ahead at its own velocity, in its own frame. */
function coast(pose: CraftPose, seconds: number): CraftPose {
  const deltaM = scale(pose.velocityMPerS, seconds);
  let position: ScenePosition;
  switch (pose.position.kind) {
    case "galactic":
      position = {
        kind: "galactic",
        position: moveGalactic(pose.position.position, deltaM),
      };
      break;
    case "system":
    case "body":
      position = { ...pose.position, offsetM: add(pose.position.offsetM, deltaM) };
      break;
  }
  return { position, velocityMPerS: pose.velocityMPerS, time: later(pose.time, seconds) };
}

/**
 * The craft's path for `untilS` seconds of scene time from its pose's time.
 *
 * @remarks
 * A craft with a planned path, which the sessions plan and the flight model fill, gets it
 * unchanged. Without one the pose is extrapolated in a straight line at its velocity in its frame,
 * exact for nothing but the only honest guess without the flight model; R11 widens its sweep for
 * that case. The line is its two ends, the pose and the pose `untilS` later.
 *
 * @param untilS - How far ahead, s of scene time, finite and not negative.
 * @throws RangeError for an `untilS` that is negative or not finite.
 */
export function predictedPath(craft: SceneCraft, untilS: number): ReadonlyArray<CraftPose> {
  if (!(Number.isFinite(untilS) && untilS >= 0)) {
    throw new RangeError(`a path ${String(untilS)} s ahead cannot be predicted`);
  }
  if (craft.plannedPath !== null) {
    return craft.plannedPath;
  }
  return [craft.state, coast(craft.state, untilS)];
}
