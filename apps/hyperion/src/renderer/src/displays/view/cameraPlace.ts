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
   * Whether the camera is held to a craft, the own ship at its seat or chasing it: its place is
   * then the craft's, which the server's scene gives, and goes stale with it.
   */
  readonly heldToCraft: boolean;
}

/** The suffix of a direction whose azimuth runs from +x, where `COREWARD` is not defined. */
export const FROM_PLUS_X = "FROM +X";

const ORIGIN = galacticPositionFromLy([0, 0, 0]);
const FORWARD = vec3(0, 0, -1);

/**
 * A direction in the guide's form for a direction from the ship (its "Numbers, units and time"):
 * an azimuth from `COREWARD` through `SPINWARD`, `000°` to `359°`, and a signed elevation, positive
 * `NORTH`, `047° +12°`. Without a local frame, or on the galactic axis, the azimuth runs from +x the
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
  const pose = rebase(drawn, readingFrame(drawn.frame, scene), sceneOrigins(scene)).pose;
  const pointingAlong = rotate(pose.orientation, FORWARD);
  if (pose.frame.kind === "galactic") {
    const [x, y, z] = galacticDeltaLy(ORIGIN, pose.frame.origin);
    const atLy = add(vec3(x, y, z), lyOf(pose.positionM));
    return {
      position: galacticReading(atLy),
      pointing: directionReading(pointingAlong, localFrameAt(atLy)),
      unit: null,
      heldToCraft,
    };
  }
  let local: LocalFrame | null = null;
  if (scene.barycentre !== null) {
    const [x, y, z] = galacticDeltaLy(ORIGIN, scene.barycentre);
    local = localFrameAt(vec3(x, y, z));
  }
  const rangeM = norm(pose.positionM);
  const range = formatBodyDistance(rangeM / 1000, previous);
  const direction = rangeM > 0 ? directionReading(pose.positionM, local) : MISSING_READING;
  return {
    position: `${range.value} ${range.unit} ${direction}`,
    pointing: directionReading(pointingAlong, local),
    unit: range.unit,
    heldToCraft,
  };
}

/**
 * Whether a camera's place reads as stale: it is held to a craft, whose place the server's scene
 * gives, while that scene is stale (the guide's "Data states"). A free camera's place is the
 * client's own and never goes stale.
 *
 * @param sceneStale - Whether the server's scene is stale (`useScene`'s `stale`).
 */
export function placeStale(place: CameraPlace, sceneStale: boolean): boolean {
  return sceneStale && place.heldToCraft;
}

/**
 * The label block's lines for a camera's place, `POSITION` and `POINTING`, muted with their `S`
 * while {@link placeStale}.
 */
export function placeLines(place: CameraPlace, sceneStale: boolean): ReadonlyArray<LabelLine> {
  const stale = placeStale(place, sceneStale);
  return [
    stale
      ? { label: "POSITION", value: place.position, stale: true }
      : { label: "POSITION", value: place.position },
    stale
      ? { label: "POINTING", value: place.pointing, stale: true }
      : { label: "POINTING", value: place.pointing },
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
