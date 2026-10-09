/**
 * Where a view's camera is and where it looks, as the view reads them (plan R07, T19.f): its
 * `POSITION` in the frame its `FRAME` line names and its `POINTING`, each a direction in the
 * guide's form for a direction from the ship.
 *
 * @remarks
 * The readings' form is here ({@link cameraPlace}); where they stand is `placeLines` (the
 * `PRIMARY` view's label block) and `CameraReadings` (the camera panel), so that either can move
 * without the other.
 */

import { galacticDeltaLy, galacticPositionFromLy, METRES_PER_LIGHT_YEAR } from "@hyperion/protocol";
import { useState } from "react";

import {
  AXIS_TOLERANCE_LY,
  cylindrical,
  type LocalFrame,
  localFrameAt,
} from "../../geometry/frame";
import { add, dot, norm, scale, type Vec3, vec3 } from "../../geometry/vec3";
import {
  type BodyDistanceUnit,
  formatBearingDeg,
  formatBodyDistance,
  formatLengthLy,
  formatSigned,
  formatSignedDeg,
} from "../../lib/format";
import type { CameraFrame } from "../../view/camera/pose";
import { rotate } from "../../view/camera/quaternion";
import { rebase } from "../../view/camera/rebase";
import { sceneOrigins, type ViewScene } from "../../view/scene/model";
import { type LabelLine, MISSING_READING, runPose, type ViewRun } from "./viewRun";

/** A view camera's place as its readings show it. */
export interface CameraPlace {
  /**
   * `POSITION`'s reading: the range from the centre of the frame the `FRAME` line names, with its
   * unit, and the direction from that centre, `26.4 Mm 047° +12°`; in the `GALACTIC` frame its
   * `RADIUS`, `ANGLE` and `HEIGHT`.
   */
  readonly position: string;
  /** `POINTING`'s reading: the line of sight's direction, `210° -05°`. */
  readonly pointing: string;
  /** The range's unit, which the next reading keeps within its hysteresis; `null` in `GALACTIC`. */
  readonly unit: BodyDistanceUnit | null;
  /**
   * Whether the camera is held to a craft, the own ship at its seat, chasing it or flown free from
   * either: its position is then the craft's, which the server's scene gives, and goes stale with
   * it.
   */
  readonly heldToCraft: boolean;
  /**
   * Whether its line of sight follows the hull, in `SEAT` and `CHASE`, so that its pointing goes
   * stale with the scene too; a free camera's orientation is the console's own.
   */
  readonly followsHull: boolean;
}

/** The suffix of a direction whose azimuth runs from +x, where `COREWARD` is not defined. */
export const FROM_PLUS_X = "FROM +X";

/**
 * The distance from the galactic axis within which a direction's azimuth runs from +x, ly: one,
 * as the guide's "Numbers, units and time" gives it (decision-r07-t19f-position, item 3).
 */
export const FROM_PLUS_X_WITHIN_LY = 1;

/**
 * The directions at a galactic place as a direction's reading takes them, or `null` within
 * {@link FROM_PLUS_X_WITHIN_LY} of the axis, where its azimuth runs from +x.
 */
function readingFrameAt(atLy: Vec3): LocalFrame | null {
  return Math.hypot(atLy.x, atLy.y) > FROM_PLUS_X_WITHIN_LY ? localFrameAt(atLy) : null;
}

const ORIGIN = galacticPositionFromLy([0, 0, 0]);
const FORWARD = vec3(0, 0, -1);

/**
 * A direction in the guide's form for a direction from the ship (its "Numbers, units and time"):
 * an azimuth from `COREWARD` through `SPINWARD`, `000°` to `359°`, and a signed elevation, positive
 * `NORTH`, `047° +12°`. Without a local frame (`null` within a light-year of the galactic axis, or
 * where the place is not known), or on the galactic axis, the azimuth runs from +x the
 * same way round (from +x through −y, clockwise seen from the north, as `COREWARD` through
 * `SPINWARD` runs) and the reading says so, `047° +12° FROM +X`. A direction that reads `+90°` or
 * `-90°` has no azimuth, which is the missing value's em dash, `— -90°`, with no reference to state.
 *
 * @param direction - Along the galactic axes, any length but zero.
 * @param local - The directions at the place, or `null` where the place is not known.
 */
export function directionReading(direction: Vec3, local: LocalFrame | null): string {
  const fromPlusX = local === null || local.onAxis;
  const across = fromPlusX ? direction.x : dot(direction, local.coreward);
  const along = fromPlusX ? -direction.y : dot(direction, local.spinward);
  const elevationDeg = (Math.atan2(direction.z, Math.hypot(across, along)) * 180) / Math.PI;
  const elevation = formatSignedDeg(elevationDeg);
  if (Math.round(Math.abs(elevationDeg)) >= 90) {
    return `${MISSING_READING} ${elevation}`;
  }
  const azimuth = formatBearingDeg((Math.atan2(along, across) * 180) / Math.PI);
  return fromPlusX ? `${azimuth} ${elevation} ${FROM_PLUS_X}` : `${azimuth} ${elevation}`;
}

/**
 * The frame a camera's position is read in: the frame its `FRAME` line names (`frameName`), so a
 * camera held to a craft is read in the frame the craft's position is in.
 */
function readingFrame(frame: CameraFrame, scene: ViewScene): CameraFrame {
  if (frame.kind !== "craft") {
    return frame;
  }
  const position = scene.craft.find((craft) => craft.id === frame.craft)?.pose.position;
  let read: CameraFrame;
  switch (position?.kind) {
    case undefined:
      read = { kind: "system", system: scene.system };
      break;
    case "system":
      read = { kind: "system", system: position.system };
      break;
    case "galactic":
      read = { kind: "galactic", origin: position.position };
      break;
    case "body":
    case "body_fixed":
      read = { kind: "body", body: position.body };
      break;
  }
  return read;
}

/** Light-years in a metre-valued vector. */
function lyOf(m: Vec3): Vec3 {
  return scale(m, 1 / METRES_PER_LIGHT_YEAR);
}

/**
 * The `GALACTIC` frame's own coordinates of a point, as the system readout reads them: `RADIUS`
 * and `HEIGHT` in `ly` to one decimal, and `ANGLE` to one decimal, the missing value's em dash on
 * the galactic axis, where it has none.
 */
function galacticReading(atLy: Vec3): string {
  const { radiusLy, angleDeg, heightLy } = cylindrical(atLy);
  const angle = radiusLy > AXIS_TOLERANCE_LY ? formatBearingDeg(angleDeg, 1) : MISSING_READING;
  const radius = `RADIUS ${formatLengthLy(radiusLy, 1)} ly`;
  return `${radius} · ANGLE ${angle} · HEIGHT ${formatSigned(heightLy, 1)} ly`;
}

/**
 * A view camera's place (plan R07, T19.f): where its run draws it from and where it looks.
 *
 * @remarks
 * The directions are taken at the system's barycentre, in whose neighbourhood `COREWARD` does not
 * turn, or at the camera itself in the `GALACTIC` frame. A server scene whose system's place the
 * client was not told of has no barycentre, so its directions run `FROM +X`. A camera at its
 * frame's very centre has no direction there, which reads as the missing value's em dash.
 *
 * @param previous - The unit the range was last shown in, held within its hysteresis
 *   (`formatBodyDistance`); `null` for the first showing.
 */
export function cameraPlace(run: ViewRun, previous: BodyDistanceUnit | null): CameraPlace {
  const { scene } = run;
  const drawn = runPose(run);
  const heldToCraft = drawn.frame.kind === "craft";
  const followsHull = run.camera.preset !== "free";
  const pose = rebase(drawn, readingFrame(drawn.frame, scene), sceneOrigins(scene)).pose;
  const pointingAlong = rotate(pose.orientation, FORWARD);
  if (pose.frame.kind === "galactic") {
    const [x, y, z] = galacticDeltaLy(ORIGIN, pose.frame.origin);
    const atLy = add(vec3(x, y, z), lyOf(pose.positionM));
    return {
      position: galacticReading(atLy),
      pointing: directionReading(pointingAlong, readingFrameAt(atLy)),
      unit: null,
      heldToCraft,
      followsHull,
    };
  }
  let local: LocalFrame | null = null;
  if (scene.barycentre !== null) {
    const [x, y, z] = galacticDeltaLy(ORIGIN, scene.barycentre);
    local = readingFrameAt(vec3(x, y, z));
  }
  const rangeM = norm(pose.positionM);
  const range = formatBodyDistance(rangeM / 1000, previous);
  const direction = rangeM > 0 ? directionReading(pose.positionM, local) : MISSING_READING;
  return {
    position: `${range.value} ${range.unit} ${direction}`,
    pointing: directionReading(pointingAlong, local),
    unit: range.unit,
    heldToCraft,
    followsHull,
  };
}

/** Which of a camera's place's readings read as stale. */
export interface PlaceStaleness {
  readonly position: boolean;
  readonly pointing: boolean;
}

/**
 * Which of a camera's place's readings read as stale while the server's scene is (the guide's
 * "Data states"; decision-r07-t19f-position, item 3): `POSITION` while the camera is held to a
 * craft, whose place the scene gives, and `POINTING` in `SEAT` and `CHASE` alone, whose line of
 * sight follows the hull. A free camera's orientation, and its position where it is held to no
 * craft, are the console's own.
 *
 * @param sceneStale - Whether the server's scene is stale (`useScene`'s `stale`).
 */
export function placeStale(place: CameraPlace, sceneStale: boolean): PlaceStaleness {
  return {
    position: sceneStale && place.heldToCraft,
    pointing: sceneStale && place.followsHull,
  };
}

/** A label line, muted with its `S` where `stale`. */
function line(label: string, value: string, stale: boolean): LabelLine {
  return stale ? { label, value, stale: true } : { label, value };
}

/**
 * The label block's lines for a camera's place, `POSITION` and `POINTING`, each muted with its `S`
 * while {@link placeStale} says so.
 */
export function placeLines(place: CameraPlace, sceneStale: boolean): ReadonlyArray<LabelLine> {
  const stale = placeStale(place, sceneStale);
  return [
    line("POSITION", place.position, stale.position),
    line("POINTING", place.pointing, stale.pointing),
  ];
}

/** A run and the place read from it, kept for the next reading's hysteresis. */
interface ShownPlace {
  readonly run: ViewRun;
  readonly place: CameraPlace;
}

/**
 * A view camera's place as its readouts show it, read again as each published run arrives, its
 * range's unit held from the last reading within its hysteresis (plan R07, T19.f).
 *
 * @remarks
 * Adjusted during render as the list's ranges are (`markRows`), so that the readings change with
 * the readouts, at 4 Hz.
 */
export function useCameraPlace(run: ViewRun): CameraPlace {
  const [shown, setShown] = useState<ShownPlace>(() => ({ run, place: cameraPlace(run, null) }));
  if (shown.run === run) {
    return shown.place;
  }
  const place = cameraPlace(run, shown.place.unit);
  setShown({ run, place });
  return place;
}
