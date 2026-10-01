/**
 * The `VIEW` display's run of a scene: the scene at its time, the camera stepped through it, and
 * what the display shows of both (plan R02, R02.T15). Pure: the display calls it each frame and on
 * each command.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import {
  type BodyDistanceUnit,
  formatBodyDistance,
  formatNumber,
  formatUniverseTimeDhms,
} from "../../lib/format";
import { norm } from "../../geometry/vec3";
import { flightInput, type ViewKeyAction } from "../../view/camera/keys";
import { changeFreeRate, MAX_FREE_STEP_S, stepFreeCamera } from "../../view/camera/freeCamera";
import type { CameraFrame, CameraPose } from "../../view/camera/pose";
import {
  advanceEasedMove,
  type CameraPreset,
  type CameraState,
  type CameraTarget,
  type CutDestination,
  type CutOptions,
  cutTo,
  displayPose,
  followPreset,
  newCameraState,
  nextTarget,
  stepFov,
  targetPosition,
} from "../../view/camera/state";
import { relativeToCamera } from "../../view/coords/relative";
import { differenceM, type ViewPosition } from "../../view/coords/position";
import {
  controlEv100,
  type ExposureControl,
  exposureLevelReading,
} from "../../view/photometry/exposure";
import { cameraSceneOf, sceneOrigins, type ViewScene } from "../../view/scene/model";
import { FRAME_CHANGE_SCENE_NAME, frameChangeScene } from "../../view/scenes/frameChange";
import type { KeptScene } from "../../view/scenes/kept";
import { PRECISION_SCENE_NAME, precisionScene } from "../../view/scenes/precision";

/** A scene the `SCENE` selector offers, by the name it shows. */
export interface SceneOption {
  readonly name: string;
  readonly make: () => KeptScene;
}

/** The kept scenes, until R02.T17 adds the server's: `PRECISION TEST` and `FRAME CHANGE TEST`. */
export const SCENE_OPTIONS: ReadonlyArray<SceneOption> = [
  { name: PRECISION_SCENE_NAME, make: precisionScene },
  { name: FRAME_CHANGE_SCENE_NAME, make: frameChangeScene },
];

/** A view's run: its kept scene, the script's time, the scene then and the camera in it. */
export interface ViewRun {
  readonly kept: KeptScene;
  /** Seconds into the script, in [0, its duration). */
  readonly tS: number;
  readonly scene: ViewScene;
  readonly camera: CameraState;
}

/** A run at the start of `kept`'s script, its camera at the seat (or free, with no own ship). */
export function startRun(kept: KeptScene): ViewRun {
  const scene = kept.sceneAt(0);
  return { kept, tS: 0, scene, camera: newCameraState(cameraSceneOf(scene), "eye") };
}

/** What one display frame brings: its duration and the flight keys held. */
export interface FrameInput {
  /** The frame's duration, s. */
  readonly dtS: number;
  /** The flight keys held on the focused canvas (`keys.ts`' binding names). */
  readonly held: ReadonlySet<string>;
  readonly reducedMotion: boolean;
}

/**
 * The run one frame on: the script advanced in real time and run again from its start at its end,
 * the seat or chase camera following the own ship, the free camera flown by the held keys, and an
 * eased move advanced.
 */
export function stepRun(run: ViewRun, input: FrameInput): ViewRun {
  const dtS = Math.min(Math.max(input.dtS, 0), MAX_FREE_STEP_S);
  const tS = (run.tS + dtS) % run.kept.durationS;
  const scene = run.kept.sceneAt(tS);
  const cameraScene = cameraSceneOf(scene);
  const following = followPreset(run.camera, cameraScene);
  const flown = stepFreeCamera(
    following,
    flightInput(input.held),
    dtS,
    input.reducedMotion,
    cameraScene,
  ).state;
  return { ...run, tS, scene, camera: advanceEasedMove(flown, dtS) };
}

/** A command's outcome: the run after it, or why it was refused. */
export type CommandResult =
  | { readonly kind: "done"; readonly run: ViewRun }
  | { readonly kind: "refused"; readonly reason: "no_own_ship" };

function cut(run: ViewRun, to: CutDestination, options: CutOptions): CommandResult {
  const result = cutTo(run.camera, to, cameraSceneOf(run.scene), options);
  return result.kind === "refused"
    ? result
    : { kind: "done", run: { ...run, camera: result.state } };
}

/** The run after a camera command: a key's action or a control's (Design note 18's cuts). */
export function commandRun(
  run: ViewRun,
  action: ViewKeyAction,
  options: CutOptions,
): CommandResult {
  const cameraScene = cameraSceneOf(run.scene);
  let result: CommandResult;
  switch (action.kind) {
    case "preset":
      result = cut(run, { kind: "preset", preset: action.preset }, options);
      break;
    case "target":
      result = cut(
        run,
        { kind: "target", target: nextTarget(run.camera, cameraScene, action.step) },
        options,
      );
      break;
    case "fov":
      result = {
        kind: "done",
        run: { ...run, camera: { ...run.camera, fovDeg: stepFov(run.camera.fovDeg, action.step) } },
      };
      break;
    case "rate":
      result = {
        kind: "done",
        run: { ...run, camera: changeFreeRate(run.camera, action.step, cameraScene) },
      };
      break;
  }
  return result;
}

/** The pose the run draws from: the camera's, blended through an eased move. */
export function runPose(run: ViewRun): CameraPose {
  return displayPose(run.camera, sceneOrigins(run.scene));
}

/** The preset's name as the view shows it. */
export const PRESET_NAMES: Readonly<Record<CameraPreset, string>> = {
  seat: "SEAT",
  chase: "CHASE",
  free: "FREE",
};

/** A body's frame's name: `BODY <designation>`. */
function bodyFrameName(body: BodyIdHex, scene: ViewScene): string {
  return `BODY ${scene.bodies.find((b) => b.id === body)?.designation ?? body}`;
}

/** The name of the frame a position is in; a body-fixed position is in its body's frame. */
function positionFrameName(position: ViewPosition, scene: ViewScene): string {
  let name: string;
  switch (position.kind) {
    case "galactic":
      name = "GALACTIC";
      break;
    case "system":
      name = "SYSTEM BARYCENTRIC";
      break;
    case "body":
    case "body_fixed":
      name = bodyFrameName(position.body, scene);
      break;
  }
  return name;
}

/** The frame's name on the label block: `SYSTEM BARYCENTRIC`, `BODY <designation>`, `GALACTIC`. */
export function frameName(frame: CameraFrame, scene: ViewScene): string {
  let name: string;
  switch (frame.kind) {
    case "galactic":
      name = "GALACTIC";
      break;
    case "system":
      name = "SYSTEM BARYCENTRIC";
      break;
    case "body":
      name = bodyFrameName(frame.body, scene);
      break;
    case "craft": {
      // A camera held to a craft is in the frame the craft's position is in.
      const position = scene.craft.find((c) => c.id === frame.craft)?.pose.position;
      name = position === undefined ? "SYSTEM BARYCENTRIC" : positionFrameName(position, scene);
      break;
    }
  }
  return name;
}

/** The star source's reading until R06's sky (Design note 16). */
export const STAR_SOURCE = "RANGE QUERY · VOLUME-LIMITED · NO EXTINCTION";

/** One line of the label block: its label and its reading. */
export interface LabelLine {
  readonly label: string;
  readonly value: string;
}

/** The exposure's reading: `EV100 -1.0 MAN`. */
export function exposureReading(exposure: ExposureControl): string {
  return `EV100 ${formatNumber(controlEv100(exposure), 1)} ${exposureLevelReading(exposure)}`;
}

/**
 * The label block's lines (Design note 16): always the frame, the time with its time system, the
 * style, the camera preset, the field of view and the exposure with its level, and the star source;
 * the scene's name in a kept scene; `POSITIONS AS SEEN FROM SHIP` while the camera is off the hull;
 * `ROTATION NOT YET MODELLED` while a body's rotation is not modelled.
 */
export function labelLines(run: ViewRun, exposure: ExposureControl): ReadonlyArray<LabelLine> {
  const { scene, camera } = run;
  const lines: LabelLine[] = [
    { label: "FRAME", value: frameName(camera.pose.frame, scene) },
    { label: "TIME", value: `UT ${formatUniverseTimeDhms(scene.time)}` },
    { label: "STYLE", value: "WIREFRAME" },
    { label: "CAMERA", value: PRESET_NAMES[camera.preset] },
    { label: "FOV", value: `${String(camera.fovDeg)}°` },
    { label: "EXPOSURE", value: exposureReading(exposure) },
    { label: "STARS", value: STAR_SOURCE },
  ];
  if (scene.provenance.kind === "kept") {
    lines.push({ label: "SCENE", value: scene.provenance.name });
  }
  return lines;
}

/** The steady statements under the label block's lines, each while its condition holds. */
export function labelStatements(run: ViewRun): ReadonlyArray<string> {
  const statements: string[] = [];
  if (run.camera.preset !== "seat" && run.scene.ownShip !== null) {
    statements.push("POSITIONS AS SEEN FROM SHIP");
  }
  if (run.scene.bodies.some((body) => body.rotation === null)) {
    statements.push("ROTATION NOT YET MODELLED");
  }
  return statements;
}

/** A row of the view's list: a target with its range. */
export interface MarkRow {
  /** The row's key: the target's kind and ID. */
  readonly key: string;
  readonly target: CameraTarget;
  /** Its designation, `TEST HULL` for a craft drawn by the test hull. */
  readonly name: string;
  /** What it is: `PLANET`, `MOON`, `STAR` or `CRAFT`. */
  readonly kind: string;
  /** Its range, with its unit: `384 Mm`. */
  readonly range: string;
  /** The unit the range is in, which the next update keeps within its hysteresis. */
  readonly unit: BodyDistanceUnit;
  /** Whether the range is from the camera, there being no own ship (Design note 17). */
  readonly fromCamera: boolean;
}

/** A target's key, stable from frame to frame. */
export function targetKey(target: CameraTarget): string {
  return target.kind === "body" ? `body:${target.body}` : `craft:${target.craft}`;
}

/**
 * The list's rows (Design note 17): the bodies, then the craft but the own ship, each with its
 * range from the own ship where there is one, else from the camera, its unit switching with
 * hysteresis from the one it was last shown in.
 */
export function markRows(
  run: ViewRun,
  previousUnits: ReadonlyMap<string, BodyDistanceUnit> = new Map(),
): ReadonlyArray<MarkRow> {
  const { scene } = run;
  const origins = sceneOrigins(scene);
  const pose = runPose(run);
  const own = scene.craft.find((c) => c.id === scene.ownShip);
  return cameraSceneOf(scene).targets.map((target) => {
    const position = targetPosition(target, origins);
    const rangeM =
      own === undefined
        ? norm(relativeToCamera(position, pose, origins))
        : norm(differenceM(position, own.pose.position, origins));
    const body =
      target.kind === "body" ? scene.bodies.find((b) => b.id === target.body) : undefined;
    const craft =
      target.kind === "craft" ? scene.craft.find((c) => c.id === target.craft) : undefined;
    const key = targetKey(target);
    const distance = formatBodyDistance(rangeM / 1000, previousUnits.get(key) ?? null);
    return {
      key,
      target,
      name:
        body?.designation ??
        (craft === undefined ? "" : `${craft.designation} · ${craft.hull.name}`),
      kind: body === undefined ? "CRAFT" : body.kind.toUpperCase(),
      range: `${distance.value} ${distance.unit}`,
      unit: distance.unit,
      fromCamera: own === undefined,
    };
  });
}
