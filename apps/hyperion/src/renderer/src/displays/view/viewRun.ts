/**
 * The `VIEW` display's run of a scene: the scene at its time, the camera stepped through it, and
 * what the display shows of both (plan R02, R02.T15 and T17). Pure: the display calls it each frame
 * and on each command.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import {
  type BodyDistanceUnit,
  formatBodyDistance,
  formatNumber,
  formatSignificant,
  formatUniverseTimeDhms,
} from "../../lib/format";
import { norm, sub } from "../../geometry/vec3";
import { flightInput, type ViewKeyAction } from "../../view/camera/keys";
import {
  changeFreeRate,
  freeRateMPerS,
  MAX_FREE_STEP_S,
  stepFreeCamera,
} from "../../view/camera/freeCamera";
import type { CameraFrame, CameraPose } from "../../view/camera/pose";
import {
  advanceEasedMove,
  type CameraPreset,
  type CameraState,
  type CameraTarget,
  type CutDestination,
  type CutOptions,
  cutTo,
  type RenderStyle,
  displayPose,
  followPreset,
  newCameraState,
  nextTarget,
  onSystemChange,
  stepFov,
  targetPosition,
} from "../../view/camera/state";
import type { AppearanceLabel } from "../../view/appearance/fromWire";
import { relativeToCamera } from "../../view/coords/relative";
import { differenceM, type ViewPosition } from "../../view/coords/position";
import {
  controlEv100,
  type ExposureControl,
  exposureLevelReading,
} from "../../view/photometry/exposure";
import {
  cameraSceneOf,
  isLitKind,
  sceneOrigins,
  type ViewBodyKind,
  type ViewScene,
} from "../../view/scene/model";
import { ECLIPSE_SCENE_NAME, eclipseScene } from "../../view/scenes/eclipseScene";
import { FRAME_CHANGE_SCENE_NAME, frameChangeScene } from "../../view/scenes/frameChange";
import type { KeptScene } from "../../view/scenes/kept";
import { otherStyle, styleName, withStyle } from "../../view/photoreal/style";
import type { StyleAvailability } from "../../view/engine/platform";
import { PHASE_SCENE_NAME, phaseScene } from "../../view/scenes/phaseScene";
import { PRECISION_SCENE_NAME, precisionScene } from "../../view/scenes/precision";
import type { TerrainAnnunciation } from "../../view/terrain/annunciation";
import { type LightingState, lightingStatement } from "../../view/lighting/hostLights";
import { closureRateMPerS } from "../../view/wireframe/symbology";

/** A kept scene the `SCENE` selector offers, by the name it shows. */
export interface SceneOption {
  readonly name: string;
  readonly make: () => KeptScene;
}

/**
 * The kept scenes: `PRECISION TEST`, `FRAME CHANGE TEST`, `PHASE TEST` (R07.T8.a) and
 * `ECLIPSE TEST` (R07.T10.c).
 */
export const SCENE_OPTIONS: ReadonlyArray<SceneOption> = [
  { name: PRECISION_SCENE_NAME, make: precisionScene },
  { name: FRAME_CHANGE_SCENE_NAME, make: frameChangeScene },
  { name: PHASE_SCENE_NAME, make: phaseScene },
  { name: ECLIPSE_SCENE_NAME, make: eclipseScene },
];

/** The `SCENE` selector's name for the server's scene of the open universe (R02.T17). */
export const SERVER_SCENE_NAME = "SERVER";

/**
 * Where a run's scene comes from: a kept scene's script, or the server's scene, which each frame
 * brings ({@link FrameInput.serverScene}).
 */
export type RunSource =
  { readonly kind: "kept"; readonly kept: KeptScene } | { readonly kind: "server" };

/** A view's run: its source, a script's time, the scene then and the camera in it. */
export interface ViewRun {
  readonly source: RunSource;
  /** Seconds into a kept scene's script, in [0, its duration); 0 for the server's scene. */
  readonly tS: number;
  readonly scene: ViewScene;
  readonly camera: CameraState;
}

/** A run at the start of `kept`'s script, its camera at the seat (or free, with no own ship). */
export function startRun(kept: KeptScene): ViewRun {
  const scene = kept.sceneAt(0);
  return {
    source: { kind: "kept", kept },
    tS: 0,
    scene,
    camera: newCameraState(cameraSceneOf(scene), "eye"),
  };
}

/**
 * A run of the server's scene from `scene`, its first frame, the camera at the seat of the ship
 * stand-in; each step takes the scene its frame brings.
 */
export function startServerRun(scene: ViewScene): ViewRun {
  return {
    source: { kind: "server" },
    tS: 0,
    scene,
    camera: newCameraState(cameraSceneOf(scene), "eye"),
  };
}

/** What one display frame brings: its duration, the flight keys held and the server's scene. */
export interface FrameInput {
  /**
   * The server's scene at the frame's time (`useScene`'s `frameAt`), for a run of it; `null` for a
   * kept run, and while the server's cannot be drawn, when the run holds its last.
   */
  readonly serverScene: ViewScene | null;
  /** The frame's duration, s. */
  readonly dtS: number;
  /** The flight keys held on the focused canvas (`keys.ts`' binding names). */
  readonly held: ReadonlySet<string>;
  readonly reducedMotion: boolean;
}

/** Whether every frame a camera's pose refers to is in `scene`: its system, body or craft. */
function frameHeld(camera: CameraState, scene: ViewScene): boolean {
  const frame = camera.pose.frame;
  let held: boolean;
  switch (frame.kind) {
    case "galactic":
      held = true;
      break;
    case "system":
      held = frame.system === scene.system;
      break;
    case "body":
      held = scene.bodies.some((body) => body.id === frame.body);
      break;
    case "craft":
      held = scene.craft.some((craft) => craft.id === frame.craft);
      break;
  }
  return held;
}

/**
 * The run one frame on: a kept scene's script advanced in real time and run again from its start
 * at its end, or the server's scene the frame brings; the seat or chase camera following
 * the own ship, the free camera flown by the held keys, and an eased move advanced.
 *
 * @remarks
 * When the server's scene is in another system, told by the system's ID and never by the scene
 * object's identity, or no longer holds the body or craft the camera is held to, the camera is
 * moved as on a jump (`onSystemChange`, Design note 7): a free camera returns to the chase preset,
 * a seat or chase camera stays with the ship. While the server's scene cannot be drawn the run
 * holds its last.
 */
export function stepRun(run: ViewRun, input: FrameInput): ViewRun {
  const dtS = Math.min(Math.max(input.dtS, 0), MAX_FREE_STEP_S);
  let tS = 0;
  let scene: ViewScene;
  let camera = run.camera;
  if (run.source.kind === "kept") {
    tS = (run.tS + dtS) % run.source.kept.durationS;
    scene = run.source.kept.sceneAt(tS);
  } else {
    scene = input.serverScene ?? run.scene;
    if (scene.system !== run.scene.system || !frameHeld(camera, scene)) {
      camera = onSystemChange(camera, cameraSceneOf(scene));
    }
  }
  return { ...run, tS, scene, camera: flyCamera(camera, scene, dtS, input) };
}

/** A camera one frame on in `scene`: following its preset, flown by the held keys, eased. */
function flyCamera(
  camera: CameraState,
  scene: ViewScene,
  dtS: number,
  input: Pick<FrameInput, "held" | "reducedMotion">,
): CameraState {
  const cameraScene = cameraSceneOf(scene);
  const following = followPreset(camera, cameraScene);
  const flown = stepFreeCamera(
    following,
    flightInput(input.held),
    dtS,
    input.reducedMotion,
    cameraScene,
  ).state;
  return advanceEasedMove(flown, dtS);
}

/**
 * An instrument view's run one frame on (plan R07, T19): the scene its primary view drew this
 * frame, at the primary's script time, and the instrument's own camera stepped through it as
 * {@link stepRun} steps one, flown by the keys held on the instrument's canvas.
 *
 * @remarks
 * Where the primary's scene is in another system than the instrument last drew, or no longer
 * holds the body or craft its camera is held to, the camera is moved as on a jump, as a server
 * scene's is.
 *
 * @param input - The instrument's own frame: its time since it last drew (it draws at its own
 *   rate) and its held keys.
 */
export function followRun(
  run: ViewRun,
  primary: ViewRun,
  input: Pick<FrameInput, "dtS" | "held" | "reducedMotion">,
): ViewRun {
  const dtS = Math.min(Math.max(input.dtS, 0), MAX_FREE_STEP_S);
  const { scene } = primary;
  let camera = run.camera;
  if (scene.system !== run.scene.system || !frameHeld(camera, scene)) {
    camera = onSystemChange(camera, cameraSceneOf(scene));
  }
  return {
    source: primary.source,
    tS: primary.tS,
    scene,
    camera: flyCamera(camera, scene, dtS, input),
  };
}

/**
 * An instrument view's run as it opens (plan R07, T19): the primary's scene, a camera of R02's
 * `camera` role, at the `CHASE` preset where the scene has an own ship (beside a primary at its
 * seat) and else as a new camera starts.
 */
export function startInstrumentRun(primary: ViewRun): ViewRun {
  const fresh: ViewRun = {
    source: primary.source,
    tS: primary.tS,
    scene: primary.scene,
    camera: newCameraState(cameraSceneOf(primary.scene), "camera"),
  };
  const chase = commandRun(
    fresh,
    { kind: "preset", preset: "chase" },
    { easedMoves: false, reducedMotion: true },
  );
  return chase.kind === "done" ? chase.run : fresh;
}

/** The styles a view offers before its adapter has answered: the wireframe alone. */
export const WIREFRAME_ONLY: StyleAvailability = { wireframe: true, photorealistic: false };

/**
 * A command's outcome: the run after it, or why it was refused: a preset that needs an own ship, or
 * a rate step outside `FREE` (R07.T19.b).
 */
export type CommandResult =
  | { readonly kind: "done"; readonly run: ViewRun }
  | { readonly kind: "refused"; readonly reason: "no_own_ship" | "not_free" };

function cut(run: ViewRun, to: CutDestination, options: CutOptions): CommandResult {
  const result = cutTo(run.camera, to, cameraSceneOf(run.scene), options);
  return result.kind === "refused"
    ? result
    : { kind: "done", run: { ...run, camera: result.state } };
}

/**
 * The run after a camera command: a key's action or a control's (Design note 18's cuts).
 *
 * @remarks
 * `PAGE UP` and `PAGE DOWN` step the free camera's rate only in `FREE`, as the flight keys move
 * only the free camera, so that the rate never changes unseen (decision-r07-t19-layout, item 1b).
 *
 * @param availability - The styles the adapter offers (R01's `styleAvailability`); a refused
 *   style leaves the camera as it was (R07.T7's `withStyle`).
 */
export function commandRun(
  run: ViewRun,
  action: ViewKeyAction,
  options: CutOptions,
  availability: StyleAvailability = WIREFRAME_ONLY,
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
      result =
        run.camera.preset === "free"
          ? {
              kind: "done",
              run: { ...run, camera: changeFreeRate(run.camera, action.step, cameraScene) },
            }
          : { kind: "refused", reason: "not_free" };
      break;
    case "style": {
      const style = action.style === "toggle" ? otherStyle(run.camera.style) : action.style;
      result = {
        kind: "done",
        run: { ...run, camera: withStyle(run.camera, style, availability) },
      };
      break;
    }
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

/**
 * The star source's reading where the scene's system has no known position (a server scene's
 * system the client was not told of, R02.T17): no range query can be centred, so none is drawn.
 */
export const STARS_WITHOUT_POSITION = "NOT AVAILABLE: the system's position is not known";

/** One line of the label block: its label and its reading. */
export interface LabelLine {
  readonly label: string;
  readonly value: string;
  /** Set on a reading of the server's scene while the scene is stale: muted, with its `S`. */
  readonly stale?: true;
}

/**
 * The free camera's translation rate, the speed its flight keys fly it at, as the view reads it, three significant figures:
 * `RATE 316 m/s`, then in km/s from 1 km/s, `RATE 1.00 km/s` (the guide's "Numbers").
 *
 * @param rateStep - The camera's rate step, {@link freeRateMPerS}'s argument.
 */
export function freeRateReading(rateStep: number): string {
  const rateMPerS = freeRateMPerS(rateStep);
  return rateMPerS < 1000
    ? `RATE ${formatSignificant(rateMPerS)} m/s`
    : `RATE ${formatSignificant(rateMPerS / 1000)} km/s`;
}

/** The exposure's reading: `EV100 -1.0 MAN`. */
export function exposureReading(exposure: ExposureControl): string {
  return `EV100 ${formatNumber(controlEv100(exposure), 1)} ${exposureLevelReading(exposure)}`;
}

/**
 * A camera's preset as the label block reads it: `SEAT`, `CHASE`, or in `FREE` the free camera's
 * rate after a middle dot, `FREE · RATE 1.00 km/s`, so that the rate is on show while the camera
 * panel is folded (decision-r07-t19-layout, item 1b).
 */
export function cameraReading(camera: CameraState): string {
  const name = PRESET_NAMES[camera.preset];
  return camera.preset === "free" ? `${name} · ${freeRateReading(camera.free.rateStep)}` : name;
}

/**
 * The label block's lines (Design note 16): always the frame, the time with its time system, the
 * style, the camera preset ({@link cameraReading}), the field of view and the exposure with its
 * level, and the star source;
 * the scene's name in a kept scene; `POSITIONS AS SEEN FROM SHIP` while the camera is off the hull;
 * `ROTATION: NOT YET MODELLED` while a body's rotation is not modelled.
 *
 * @param stale - Whether the server's scene is stale (`useScene`'s `stale`): its time, held where
 *   the scene went stale, then reads as the guide's stale value.
 */
export function labelLines(
  run: ViewRun,
  exposure: ExposureControl,
  stale = false,
): ReadonlyArray<LabelLine> {
  const { scene, camera } = run;
  const time = `UT ${formatUniverseTimeDhms(scene.time)}`;
  const lines: LabelLine[] = [
    { label: "FRAME", value: frameName(camera.pose.frame, scene) },
    stale ? { label: "TIME", value: time, stale: true } : { label: "TIME", value: time },
    { label: "STYLE", value: styleName(camera.style) },
    { label: "CAMERA", value: cameraReading(camera) },
    { label: "FOV", value: `${String(camera.fovDeg)}°` },
    { label: "EXPOSURE", value: exposureReading(exposure) },
    {
      label: "STARS",
      value: scene.barycentre === null ? STARS_WITHOUT_POSITION : STAR_SOURCE,
    },
  ];
  if (scene.provenance.kind === "kept") {
    lines.push({ label: "SCENE", value: scene.provenance.name });
  }
  return lines;
}

/** The statement of a view whose camera is off the hull: positions are as the ship sees them. */
export const POSITIONS_FROM_SHIP = "POSITIONS AS SEEN FROM SHIP";

/**
 * The statement the `PRIMARY` view's block carries while the `EASED CAMERA MOVES` setting is on
 * and applied, not under reduced motion (decision-r07-t19-layout, item 1b): display-wide, so on
 * the primary's block alone, and stated only while it is on, as `DECORATION ON` is.
 */
export const EASED_MOVES_STATEMENT = "EASED CAMERA MOVES";

/**
 * The steady statements under the label block's lines, each while its condition holds.
 *
 * @param terrain - The view's debounced terrain annunciation (plan R05, T9), or `null` while
 *   neither condition holds or the view draws no terrain.
 */
export function labelStatements(
  run: ViewRun,
  terrain: TerrainAnnunciation | null = null,
): ReadonlyArray<string> {
  const statements: string[] = [];
  if (run.camera.preset !== "seat" && run.scene.ownShip !== null) {
    statements.push(POSITIONS_FROM_SHIP);
  }
  if (run.scene.bodies.some((body) => body.rotation === null)) {
    statements.push("ROTATION: NOT YET MODELLED");
  }
  if (terrain !== null) {
    statements.push(terrain);
  }
  return statements;
}

/** The statement while the photorealistic style is chosen and its image is not yet drawn. */
export const PHOTOREAL_PREPARING = "PHOTOREALISTIC: PREPARING";

/**
 * The note while the photorealistic frame is drawn and its scene has a craft, the own ship
 * included (R07.T16.e; decision-r07-t16a, item 3): no craft's light is computed, so each is drawn
 * as its cased hull outline on a `--surface-0` silhouette, never read as its own.
 */
export const CRAFT_PHOTOMETRY_STATEMENT = "CRAFT PHOTOMETRY: NOT YET MODELLED";

/**
 * {@link CRAFT_PHOTOMETRY_STATEMENT} composed with `BODY PHOTOMETRY: NOT YET MODELLED` into one
 * note, in that note's place, while both hold, as the guide composes its pairs of notes.
 */
export const BODY_AND_CRAFT_PHOTOMETRY_STATEMENT = "BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED";

/** The body note that {@link BODY_AND_CRAFT_PHOTOMETRY_STATEMENT} takes the place of. */
const BODY_PHOTOMETRY_LABEL: AppearanceLabel = "BODY PHOTOMETRY: NOT YET MODELLED";

/**
 * The photorealistic style's statements under the label block (R07.T8.a), none in the wireframe:
 * {@link PHOTOREAL_PREPARING} while the view still draws its wireframe in its place (its pipelines
 * compiling); then, while the scene has a body to light, the lighting line while no star lights it
 * (decision-r07-t8a, item 1) and each label the drawn bodies carry (`BODY PHOTOMETRY: NOT YET
 * MODELLED` for the provisional photometry, Design note 5); and, while the scene has a craft,
 * {@link CRAFT_PHOTOMETRY_STATEMENT}, composed with the body note into
 * {@link BODY_AND_CRAFT_PHOTOMETRY_STATEMENT} where both hold (R07.T16.e).
 *
 * @remarks
 * Scene-level, as the bodies' labels are (`litLabelsOf`): a craft's note stands with the scene's
 * craft, whether or not one is in the picture, and with no lit body as well.
 *
 * @param drawn - The style the view's last frame was drawn in.
 * @param labels - The appearance labels of the bodies the photorealistic frame lights.
 */
export function photorealStatements(
  run: ViewRun,
  lighting: LightingState,
  drawn: RenderStyle,
  labels: ReadonlyArray<AppearanceLabel>,
): ReadonlyArray<string> {
  if (run.camera.style !== "photorealistic") {
    return [];
  }
  if (drawn !== "photorealistic") {
    return [PHOTOREAL_PREPARING];
  }
  const hasCraft = run.scene.craft.length > 0;
  if (!run.scene.bodies.some((body) => isLitKind(body.kind))) {
    return hasCraft ? [CRAFT_PHOTOMETRY_STATEMENT] : [];
  }
  const line = lightingStatement(lighting);
  const bodyNotes: ReadonlyArray<string> = [...new Set(labels)];
  let notes = bodyNotes;
  if (hasCraft) {
    notes = bodyNotes.includes(BODY_PHOTOMETRY_LABEL)
      ? bodyNotes.map((note) =>
          note === BODY_PHOTOMETRY_LABEL ? BODY_AND_CRAFT_PHOTOMETRY_STATEMENT : note,
        )
      : [...bodyNotes, CRAFT_PHOTOMETRY_STATEMENT];
  }
  return [...(line === null ? [] : [line]), ...notes];
}

/**
 * Each body kind's name in the list's `KIND` column, as the guide's nomenclature names it (its
 * `KIND` row): `PLANET`, `DWARF PLANET`, `MOON`, `UNRESOLVED CONTACT`, and `STAR`.
 */
export const BODY_KIND_NAMES: Readonly<Record<ViewBodyKind, string>> = {
  star: "STAR",
  planet: "PLANET",
  dwarf_planet: "DWARF PLANET",
  moon: "MOON",
  unresolved: "UNRESOLVED CONTACT",
};

/** A row of the view's list: a target with its range. */
export interface MarkRow {
  /** The row's key: the target's kind and ID. */
  readonly key: string;
  readonly target: CameraTarget;
  /** Its designation, `TEST HULL` for a craft drawn by the test hull. */
  readonly name: string;
  /** What it is: one of {@link BODY_KIND_NAMES}, or `CRAFT`. */
  readonly kind: string;
  /** Its range, with its unit: `384 Mm`. */
  readonly range: string;
  /** The unit the range is in, which the next update keeps within its hysteresis. */
  readonly unit: BodyDistanceUnit;
  /** Whether the range is from the camera, there being no own ship (Design note 17). */
  readonly fromCamera: boolean;
  /**
   * A craft's closure rate on the own ship: `known`, with its sign, `+3.40 m/s`; `unknown`
   * where its or the own ship's velocity is not known; `none` for a body, or with no own ship.
   */
  readonly closure: ClosureReading;
}

/**
 * A list row's closure rate: `none` for a body or with no own ship to close on, `unknown` where its
 * or the own ship's velocity is not known (shown as {@link MISSING_READING}), else `known`.
 */
export type ClosureReading =
  | { readonly kind: "none" }
  | { readonly kind: "unknown" }
  | { readonly kind: "known"; readonly text: string };

/** A reading the scene does not have: an em dash, shown in `--text-muted` (the guide's "Missing"). */
export const MISSING_READING = "—";

/** A closure rate with its sign, since direction matters (the guide's "Numbers"): `+3.40 m/s`. */
function closureText(closureMPerS: number): string {
  const sign = closureMPerS < 0 ? "-" : "+";
  return `${sign}${formatSignificant(Math.abs(closureMPerS))} m/s`;
}

/**
 * A row's range as the canvas labels and the list's accessible names read it, `FROM CAMERA` with
 * no own ship; the list's rows show the bare range under its `RANGE FROM CAMERA` head
 * (R07.T19.b).
 */
export function rangeText(row: MarkRow): string {
  return row.fromCamera ? `${row.range} FROM CAMERA` : row.range;
}

/** Whether a scene's ranges are from the camera: it has no own ship to measure from. */
export function rangesFromCamera(scene: ViewScene): boolean {
  return !scene.craft.some((craft) => craft.id === scene.ownShip);
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
  const fromCamera = rangesFromCamera(scene);
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
    let closure: ClosureReading = { kind: "none" };
    if (craft !== undefined && own !== undefined) {
      const closureMPerS = closureRateMPerS(
        differenceM(craft.pose.position, own.pose.position, origins),
        own.velocityMPerS === null || craft.velocityMPerS === null
          ? null
          : sub(craft.velocityMPerS, own.velocityMPerS),
      );
      closure =
        closureMPerS === null
          ? { kind: "unknown" }
          : { kind: "known", text: closureText(closureMPerS) };
    }
    return {
      key,
      target,
      name:
        body?.designation ??
        (craft === undefined ? "" : `${craft.designation} · ${craft.hull.name}`),
      kind: body === undefined ? "CRAFT" : BODY_KIND_NAMES[body.kind],
      range: `${distance.value} ${distance.unit}`,
      unit: distance.unit,
      fromCamera,
      closure,
    };
  });
}
