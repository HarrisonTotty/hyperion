import type { BodyIdHex, SystemIdHex } from "@hyperion/protocol";

import { add, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { easeOut } from "../../spatial/transition";
import { expressIn, type ViewPosition } from "../coords/position";
import { type CameraOrigins, frameOrigin, relativeToCamera } from "../coords/relative";
import { cameraFrameCandidate, selectCameraFrame } from "./frames";
import type { CameraFrame, CameraPose, CraftId, Quaternion } from "./pose";
import { DEFAULT_FOV_DEG, FOV_STEPS_DEG } from "./projection";
import { lookAlong, multiply, quaternionFromAxisAngle, rotate, slerp } from "./quaternion";
import { type FrameChange, rebase, sameCameraFrame } from "./rebase";

/**
 * Where a local view's camera is held: at the own ship's seat, chasing it, or detached (plan R02,
 * R02.T9.a; brainstorm, "The free camera").
 *
 * @remarks
 * The union is open: R07.T25's Phase C adds `slaved`, a station's view following the main
 * screen's camera.
 */
export type CameraPreset = "seat" | "chase" | "free";

/**
 * How a view draws: R02's `wireframe`, or R07's `photorealistic` (R07.T7). A style chooses passes
 * and strokes and owns no scene, camera or projection (brainstorm, "Two styles of one renderer").
 */
export type RenderStyle = "wireframe" | "photorealistic";

/**
 * What a view stands for: `eye` for the single-player cockpit view, whose star limits are a human
 * eye's, and `camera` for every other view (R06's star limits read it).
 */
export type ViewRole = "eye" | "camera";

/**
 * A view's identity within the client, which `CameraReporter` (R03) and R07 key on. Build one with
 * {@link viewId}.
 */
export type ViewId = string & { readonly __viewId: true };

/**
 * Builds a {@link ViewId} from its name.
 *
 * @throws RangeError if the name is empty.
 */
export function viewId(name: string): ViewId {
  if (!isViewId(name)) {
    throw new RangeError("a view's identity must not be empty");
  }
  return name;
}

/** Whether a name may be a {@link ViewId}: any string but the empty one. */
function isViewId(name: string): name is ViewId {
  return name.length > 0;
}

/** What a camera can be pointed at: a body or a craft of the scene. */
export type CameraTarget =
  | { readonly kind: "body"; readonly body: BodyIdHex }
  | { readonly kind: "craft"; readonly craft: CraftId };

/** Which way the seat camera looks: along the hull, back along it, or at the target. */
export type SeatLook = "forward" | "aft" | "target";

/**
 * The scene's own ship as the camera needs it.
 *
 * @remarks
 * Hull axes are the camera's convention: +x to starboard, +y dorsal and −z forward, so that a seat
 * looking forward has the ship's attitude as its orientation. Until sessions give a real ship, it
 * is a kept scene's test craft or R03's ship stand-in (`scene_ship`).
 */
export interface OwnShip {
  /** The craft the ship is in the scene. */
  readonly craft: CraftId;
  /** The rotation from hull axes to the ship's frame axes (the galactic axes). */
  readonly attitude: Quaternion;
  /** The pilot's eye point, m in hull axes from the craft's position. */
  readonly eyePointM: Vec3;
  /** The hull's length, m, positive: the chase camera's offset scales with it. */
  readonly lengthM: number;
}

/** A body the camera's frame selection considers: a planet, dwarf planet or moon with its Hill radius. */
export interface FrameBody {
  /** The body. */
  readonly id: BodyIdHex;
  /** The body it orbits, or `null`. */
  readonly parent: BodyIdHex | null;
  /** Its Hill radius at pericentre, m, positive. */
  readonly hillRadiusM: number;
}

/**
 * What the camera needs of the scene it looks at: its frames, its targets and its own ship.
 *
 * @remarks
 * R02.T11's `ViewScene` and R02.T17's server scene both supply it.
 */
export interface CameraScene {
  /** The scene's system, or `null` where the scene itself is galactic. */
  readonly system: SystemIdHex | null;
  /** The system's tidal radius, m, which bounds a free camera (Design note 7); `null` if galactic. */
  readonly tidalRadiusM: number | null;
  /** Where every frame's origin and every craft is at the frame time, as the scene draws them. */
  readonly origins: CameraOrigins;
  /** The bodies whose frames the camera may enter. */
  readonly frameBodies: ReadonlyArray<FrameBody>;
  /** The targets, in the order the next and previous target keys step through them. */
  readonly targets: ReadonlyArray<CameraTarget>;
  /** The own ship, or `null` where there is none. */
  readonly ownShip: OwnShip | null;
  /** The pose a camera with no own ship starts at. */
  readonly defaultPose: CameraPose;
}

/**
 * The free camera's motion state, integrated at display rate (R02.T9.b).
 *
 * @remarks
 * Rates are along the camera's own axes: translation in m/s; rotation in rad/s about x (pitch),
 * y (yaw) and z (roll).
 */
export interface FreeFlight {
  /** The current translation rate, m/s in camera axes. */
  readonly velocityMPerS: Vec3;
  /** The current rotation rate, rad/s about the camera's axes. */
  readonly angularRateRadPerS: Vec3;
  /** The commanded rate's step on its logarithmic scale: the rate is 10^(step ÷ 2) m/s. */
  readonly rateStep: number;
  /** Time not yet integrated, s, less than one tick. */
  readonly pendingS: number;
}

/** An eased move in progress: where it started, in the destination's frame, and how long ago. */
export interface EasedMove {
  /** The pose the move started from, in the frame of the state's pose. */
  readonly from: CameraPose;
  /** Time since the move started, s. */
  readonly elapsedS: number;
}

/**
 * A local view's camera: its preset, target, style and role, and its pose as frame plus offset
 * (brainstorm, "The free camera": one shape in both deployments).
 */
export interface CameraState {
  /** The preset the camera is in. */
  readonly preset: CameraPreset;
  /** The selected target, or `null`. */
  readonly target: CameraTarget | null;
  /** Which way the seat looks. */
  readonly look: SeatLook;
  /** How the view draws. */
  readonly style: RenderStyle;
  /** What the view stands for. */
  readonly role: ViewRole;
  /** The camera's pose: where a move in progress is heading. */
  readonly pose: CameraPose;
  /** The horizontal field of view, degrees, one of `FOV_STEPS_DEG`. */
  readonly fovDeg: number;
  /** The free camera's motion. */
  readonly free: FreeFlight;
  /** An eased move in progress, or `null`. */
  readonly move: EasedMove | null;
}

/**
 * How long an eased camera move takes, s: 0.4 (brainstorm, "The free camera").
 *
 * @remarks
 * Under the `EASED CAMERA MOVES` setting only, off by default, and never under
 * `prefers-reduced-motion`; otherwise every preset change and slew is a cut (Design note 18).
 */
export const EASED_MOVE_S = 0.4;

/**
 * The chase camera's offset from the own ship, in hull lengths along hull axes: three lengths
 * astern and half a length above, looking at the ship.
 *
 * @remarks
 * A choice of this plan, not a figure from a source: far enough to show the whole hull at the
 * default 60° field of view.
 */
export const CHASE_OFFSET_HULL_LENGTHS: Vec3 = vec3(0, 0.5, 3);

/**
 * The free camera's commanded rate step a new camera starts at: 6, 10³ m/s.
 *
 * @remarks
 * A choice of this plan: a craft's length in hundredths of a second, a planet's radius in about
 * two hours; the rate keys take it to either end in a dozen steps or so.
 */
export const DEFAULT_FREE_RATE_STEP = 6;

const HULL_UP = vec3(0, 1, 0);
const AFT_TURN = quaternionFromAxisAngle(HULL_UP, Math.PI);

/** A still free camera at the default rate. */
const STILL: FreeFlight = {
  velocityMPerS: vec3(0, 0, 0),
  angularRateRadPerS: vec3(0, 0, 0),
  rateStep: DEFAULT_FREE_RATE_STEP,
  pendingS: 0,
};

/** The presets a scene offers: all three with an own ship, only `free` without one. */
export function offeredPresets(scene: CameraScene): readonly CameraPreset[] {
  return scene.ownShip === null ? ["free"] : ["seat", "chase", "free"];
}

/** Where a target is. */
export function targetPosition(target: CameraTarget, origins: CameraOrigins): ViewPosition {
  return target.kind === "body"
    ? { kind: "body", body: target.body, m: vec3(0, 0, 0) }
    : origins.craftPosition(target.craft);
}

function sameTarget(a: CameraTarget | null, b: CameraTarget | null): boolean {
  if (a === null || b === null) {
    return a === b;
  }
  if (a.kind === "body") {
    return b.kind === "body" && a.body === b.body;
  }
  return b.kind === "craft" && a.craft === b.craft;
}

/** The seat's pose: at the eye point, in the ship's `craft` frame (Design note 22). */
function seatPose(
  ship: OwnShip,
  look: SeatLook,
  target: CameraTarget | null,
  scene: CameraScene,
): CameraPose {
  const frame: CameraFrame = { kind: "craft", craft: ship.craft };
  const positionM = rotate(ship.attitude, ship.eyePointM);
  const forward: CameraPose = { frame, positionM, orientation: ship.attitude };
  if (look === "aft") {
    return { ...forward, orientation: multiply(ship.attitude, AFT_TURN) };
  }
  if (look === "forward" || target === null) {
    return forward;
  }
  if (target.kind === "craft" && target.craft === ship.craft) {
    return forward;
  }
  const toTarget = relativeToCamera(targetPosition(target, scene.origins), forward, scene.origins);
  if (!(norm(toTarget) > 0)) {
    return forward;
  }
  return { ...forward, orientation: lookAlong(toTarget, rotate(ship.attitude, HULL_UP)) };
}

/** The chase pose: astern and above the ship, looking at it, in its `craft` frame. */
function chasePose(ship: OwnShip): CameraPose {
  const offsetM = rotate(ship.attitude, scale(CHASE_OFFSET_HULL_LENGTHS, ship.lengthM));
  return {
    frame: { kind: "craft", craft: ship.craft },
    positionM: offsetM,
    orientation: lookAlong(scale(offsetM, -1), rotate(ship.attitude, HULL_UP)),
  };
}

/**
 * The body frame the rule's hysteresis starts from: the camera's own, or for a camera held about a
 * craft the body frame the craft's position is given in, so that a camera detaching from a ship in
 * a sphere's band keeps the ship's frame.
 */
function currentBodyFrame(frame: CameraFrame, scene: CameraScene): BodyIdHex | null {
  if (frame.kind === "body") {
    return frame.body;
  }
  if (frame.kind === "craft") {
    const craft = scene.origins.craftPosition(frame.craft);
    return craft.kind === "body" || craft.kind === "body_fixed" ? craft.body : null;
  }
  return null;
}

/**
 * The frame a detached camera at `pose` belongs in: the scene's system or the body frame the rule
 * selects there (Design note 6), or the galactic frame where the scene is galactic.
 *
 * @remarks
 * The camera and the candidates' centres are both where the view draws them (`origins`). A free
 * camera lives in the drawn scene, so its frame is the body it is drawn beside, and it holds still
 * there whatever frame its pose is held in. The sim's geometric rule over present positions governs
 * the ship's local body (`sceneAt`) and the flight model; the camera runs the same rule over the
 * drawn centres (Design note 6, as amended 2026-10-02).
 */
export function sceneFrameFor(pose: CameraPose, scene: CameraScene): CameraFrame {
  const { origins } = scene;
  if (scene.system === null) {
    const origin = expressIn(frameOrigin(pose.frame, origins), { kind: "galactic" }, origins);
    if (origin.kind !== "galactic") {
      throw new Error("a position expressed in the galactic frame must be galactic");
    }
    return { kind: "galactic", origin: origin.position };
  }
  const system = scene.system;
  const inSystem = rebase(pose, { kind: "system", system }, origins).pose.positionM;
  const current = currentBodyFrame(pose.frame, scene);
  const candidates = scene.frameBodies.map((body) =>
    cameraFrameCandidate(
      body.id,
      body.parent,
      norm(sub(inSystem, origins.bodyCentreM(body.id))),
      body.hillRadiusM,
    ),
  );
  const selected = selectCameraFrame(candidates, current);
  return selected === null ? { kind: "system", system } : { kind: "body", body: selected };
}

/**
 * The pose a preset puts the camera at, or `null` where the preset needs an own ship and the scene
 * has none.
 *
 * @remarks
 * `seat` is at the own ship's eye point and `chase` astern of it, both in the ship's `craft` frame
 * (Design note 22). `free` detaches from `from` where it stands: into the target craft's frame when
 * the target is a craft (it then orbits that craft), otherwise into the scene's own frames.
 */
export function presetPose(
  preset: CameraPreset,
  look: SeatLook,
  target: CameraTarget | null,
  scene: CameraScene,
  from: CameraPose,
): CameraPose | null {
  const ship = scene.ownShip;
  let pose: CameraPose | null;
  switch (preset) {
    case "seat":
      pose = ship === null ? null : seatPose(ship, look, target, scene);
      break;
    case "chase":
      pose = ship === null ? null : chasePose(ship);
      break;
    case "free": {
      const frame: CameraFrame =
        target?.kind === "craft"
          ? { kind: "craft", craft: target.craft }
          : sceneFrameFor(from, scene);
      pose = rebase(from, frame, scene.origins).pose;
      break;
    }
  }
  return pose;
}

/**
 * A new view's camera: `seat` where the scene has an own ship, `free` at the scene's default pose
 * where it has none (both brainstorms: "The free camera", "The view outside").
 */
export function newCameraState(scene: CameraScene, role: ViewRole): CameraState {
  const seat = presetPose("seat", "forward", null, scene, scene.defaultPose);
  return {
    preset: seat === null ? "free" : "seat",
    target: null,
    look: "forward",
    style: "wireframe",
    role,
    pose: seat ?? scene.defaultPose,
    fovDeg: DEFAULT_FOV_DEG,
    free: STILL,
    move: null,
  };
}

/** What a cut goes to: a preset, a target to look at, or the seat's look. */
export type CutDestination =
  | { readonly kind: "preset"; readonly preset: CameraPreset }
  | { readonly kind: "target"; readonly target: CameraTarget | null }
  | { readonly kind: "look"; readonly look: SeatLook };

/** The settings a cut honours. */
export interface CutOptions {
  /** The `EASED CAMERA MOVES` setting. */
  readonly easedMoves: boolean;
  /** Whether the operator has asked for reduced motion, which makes every move a cut. */
  readonly reducedMotion: boolean;
}

/**
 * A cut's outcome: the new state, with the {@link FrameChange} it made where the pose is now in
 * another frame than the one on screen (every cached camera-relative quantity is then dropped, as
 * on a step's change), or why it was refused.
 */
export type CutResult =
  | { readonly kind: "cut"; readonly state: CameraState; readonly change: FrameChange | null }
  | { readonly kind: "refused"; readonly reason: "no_own_ship" };

/**
 * The camera after a preset change, a slew to a target or a change of the seat's look.
 *
 * @remarks
 * Every such change is a cut, the pose replaced in one step (Design note 18), except under the
 * `EASED CAMERA MOVES` setting without reduced motion, when the state carries a 0.4 s eased move
 * from the pose on screen, which {@link displayPose} blends and {@link advanceEasedMove} runs out.
 * A free camera slewed to a target turns in place to look at it; one given a craft as its target
 * is held in that craft's frame, where it orbits it (Design note 22). The free camera's motion
 * stops at a cut.
 */
export function cutTo(
  state: CameraState,
  to: CutDestination,
  scene: CameraScene,
  options: CutOptions,
): CutResult {
  let preset = state.preset;
  let target = state.target;
  let look = state.look;
  switch (to.kind) {
    case "preset":
      preset = to.preset;
      break;
    case "target":
      target = to.target;
      if (preset === "seat" && target !== null) {
        look = "target";
      }
      break;
    case "look":
      look = to.look;
      break;
  }
  const onScreen = displayPose(state, scene.origins);
  let pose = presetPose(preset, look, target, scene, onScreen);
  if (pose === null) {
    return { kind: "refused", reason: "no_own_ship" };
  }
  if (
    preset === "free" &&
    to.kind === "target" &&
    target !== null &&
    !sameTarget(target, state.target)
  ) {
    const toTarget = relativeToCamera(targetPosition(target, scene.origins), pose, scene.origins);
    if (norm(toTarget) > 0) {
      pose = { ...pose, orientation: lookAlong(toTarget, rotate(pose.orientation, HULL_UP)) };
    }
  }
  const eased = options.easedMoves && !options.reducedMotion;
  return {
    kind: "cut",
    change: sameCameraFrame(onScreen.frame, pose.frame)
      ? null
      : { from: onScreen.frame, to: pose.frame },
    state: {
      ...state,
      preset,
      target,
      look,
      pose,
      free: { ...STILL, rateStep: state.free.rateStep },
      move: eased ? { from: rebase(onScreen, pose.frame, scene.origins).pose, elapsedS: 0 } : null,
    },
  };
}

/**
 * The camera with its seat or chase pose recomputed from the own ship's attitude, as each frame
 * needs; a free camera is returned as it was.
 */
export function followPreset(state: CameraState, scene: CameraScene): CameraState {
  if (state.preset === "free") {
    return state;
  }
  const pose = presetPose(state.preset, state.look, state.target, scene, state.pose);
  return pose === null ? state : { ...state, pose };
}

/** The camera with its eased move advanced by `dtS` seconds, and dropped once it has run out. */
export function advanceEasedMove(state: CameraState, dtS: number): CameraState {
  if (state.move === null) {
    return state;
  }
  const elapsedS = state.move.elapsedS + dtS;
  return { ...state, move: elapsedS >= EASED_MOVE_S ? null : { ...state.move, elapsedS } };
}

/**
 * The pose to draw from: the state's pose, or the eased blend towards it while a move is in
 * progress (positions linear, orientation along the shorter arc, `easeOut` in time).
 */
export function displayPose(state: CameraState, origins: CameraOrigins): CameraPose {
  const { move, pose } = state;
  if (move === null) {
    return pose;
  }
  const from = rebase(move.from, pose.frame, origins).pose;
  const t = easeOut(move.elapsedS / EASED_MOVE_S);
  return {
    frame: pose.frame,
    positionM: add(from.positionM, scale(sub(pose.positionM, from.positionM), t)),
    orientation: slerp(from.orientation, pose.orientation, t),
  };
}

/**
 * The field of view one step narrower (`step` −1) or wider (+1) on `FOV_STEPS_DEG`, held at the
 * ends; a value off the steps moves to the nearest step in that direction.
 */
export function stepFov(fovDeg: number, step: -1 | 1): number {
  if (step > 0) {
    return FOV_STEPS_DEG.find((stepDeg) => stepDeg > fovDeg) ?? fovDeg;
  }
  return FOV_STEPS_DEG.findLast((stepDeg) => stepDeg < fovDeg) ?? fovDeg;
}

/**
 * The next (`step` +1) or previous (−1) of the scene's targets after the selected one, wrapping at
 * the ends; the first or the last where none is selected, and `null` where the scene has none.
 */
export function nextTarget(
  state: CameraState,
  scene: CameraScene,
  step: -1 | 1,
): CameraTarget | null {
  const { targets } = scene;
  if (targets.length === 0) {
    return null;
  }
  const index = targets.findIndex((target) => sameTarget(target, state.target));
  const next =
    index < 0
      ? step > 0
        ? 0
        : targets.length - 1
      : (index + step + targets.length) % targets.length;
  return targets[next] ?? null;
}

/**
 * The camera after the scene's system changes (a jump): a free camera returns to `chase` about the
 * own ship, or to the scene's default pose where there is none (Design note 7; brainstorm, "The
 * free camera"); a seat or chase camera, held in the ship's `craft` frame, stays where it is.
 */
export function onSystemChange(state: CameraState, scene: CameraScene): CameraState {
  const ship = scene.ownShip;
  if (state.preset !== "free" && ship !== null) {
    return followPreset({ ...state, move: null }, scene);
  }
  return {
    ...state,
    preset: ship === null ? "free" : "chase",
    target: null,
    look: "forward",
    pose: ship === null ? scene.defaultPose : chasePose(ship),
    free: { ...STILL, rateStep: state.free.rateStep },
    move: null,
  };
}

/** The state with its pose (and any move's start) re-expressed after a frame change. */
export function rebaseState(
  state: CameraState,
  pose: CameraPose,
  change: FrameChange | null,
  origins: CameraOrigins,
): CameraState {
  if (change === null) {
    return { ...state, pose };
  }
  return {
    ...state,
    pose,
    move:
      state.move === null
        ? null
        : { ...state.move, from: rebase(state.move.from, change.to, origins).pose },
  };
}
