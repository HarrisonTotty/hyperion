/**
 * Where a perf scene's camera script stands in the served scene (plan R12, R12.T4.c; Design note
 * 8): the camera frame its positions are measured from and its axes along the galactic axes, so
 * that a `scripts.ts` pose becomes the pose `VIEW`'s free scripted camera is set at
 * (`displays/view/viewPerf.ts`).
 *
 * @remarks
 * `ringed-giant-approach`'s script is in the giant's body-fixed frame, z its spin axis with the
 * rings in its equator. The scene gives the ring plane's normal, not the spin axis, so the script's
 * frame stands as the ring's (a lean, 2026-11-18): z the normal of the giant's widest ring on the
 * host star's side, x the direction to the star projected into the ring plane, y completing a right-handed
 * set, all fixed for a repetition at the arrival's first frame and about the giant's centre in its
 * non-rotating body frame, which follows the giant along its orbit. The camera therefore starts
 * 20° above the sunlit face of the rings and ends inside them on the line toward the star, so that
 * the giant and its rings are lit throughout. `nuclear-disc-sky`'s sweep is in its system's frame,
 * the galactic axes about the barycentre, which a camera's `system` frame is.
 *
 * Every vector is made by `+`, `−`, `×` and `Math.sqrt`, which IEEE 754 rounds correctly, so that
 * a placement and its poses are the same to the bit from the same scene, as the scripts are.
 */

import type { BodyIdHex, SystemIdHex } from "@hyperion/protocol";

import { type Vec3, vec3 } from "../../geometry/vec3";
import type { CameraFrame, CameraPose } from "../camera/pose";
import { multiply, quaternionFromRows } from "../camera/quaternion";
import type { ViewScene } from "../scene/model";
import { approachableRing, type PerfPose, type RingedGiant } from "./scripts";

/** A script's frame in a scene: the camera frame of its origin and its axes, galactic, unit. */
export interface ScriptPlacement {
  readonly frame: CameraFrame;
  /** The script frame's x, y and z axes along the galactic axes, right-handed. */
  readonly axes: readonly [Vec3, Vec3, Vec3];
}

/** The galactic axes: a script in a system's frame. */
const GALACTIC_AXES: readonly [Vec3, Vec3, Vec3] = [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)];

/** `a · b`. */
function dotOf(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

/** `a × b`, right-handed. */
function crossOf(a: Vec3, b: Vec3): Vec3 {
  return vec3(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x);
}

/** `v` over its length by `Math.sqrt`, or `null` where it has none. */
function unitOf(v: Vec3): Vec3 | null {
  const length = Math.sqrt(dotOf(v, v));
  return length > 0 && Number.isFinite(length)
    ? vec3(v.x / length, v.y / length, v.z / length)
    : null;
}

/** `v` less its part along the unit `n`. */
function inPlane(v: Vec3, n: Vec3): Vec3 {
  const along = dotOf(v, n);
  return vec3(v.x - along * n.x, v.y - along * n.y, v.z - along * n.z);
}

/** A script's pose in the scene: its position and orientation along the galactic axes. */
export function placedPose(pose: PerfPose, placement: ScriptPlacement): CameraPose {
  const [x, y, z] = placement.axes;
  const p = pose.positionM;
  // The axes as a rotation from the script's frame to the galactic one: its columns.
  const toGalactic = quaternionFromRows([
    vec3(x.x, y.x, z.x),
    vec3(x.y, y.y, z.y),
    vec3(x.z, y.z, z.z),
  ]);
  return {
    frame: placement.frame,
    positionM: vec3(
      x.x * p.x + y.x * p.y + z.x * p.z,
      x.y * p.x + y.y * p.y + z.y * p.z,
      x.z * p.x + y.z * p.y + z.z * p.z,
    ),
    orientation: multiply(toGalactic, pose.orientation),
  };
}

/** The sweep's placement about a system's barycentre: its frame and the galactic axes. */
export function barycentrePlacement(system: SystemIdHex): ScriptPlacement {
  return { frame: { kind: "system", system }, axes: GALACTIC_AXES };
}

/**
 * A ringed giant as a served scene holds it, the script's annulus and its frame there; or why the
 * approach cannot be flown in that scene.
 */
export type PlacedGiant =
  | { readonly kind: "placed"; readonly giant: RingedGiant; readonly placement: ScriptPlacement }
  | {
      readonly kind: "gap";
      /** Why, in the record's words, with the giant's ID. */
      readonly reason: string;
      /** Why, as the run's fault line states it, with its remedy. */
      readonly shown: string;
    };

/** The remedy of a pin whose scene the served universe does not hold (R12.T5.b's re-pin). */
const REPIN = "re-pin the scene";

/**
 * The approach's giant in `scene`, or why it cannot be flown there: its widest ring's annulus, which
 * must be one the approach can end inside (`approachableRing`), and the script's frame, z that
 * ring's normal and x toward the host star in its plane (the module's remarks).
 *
 * @remarks
 * The host star is the scene's first star, its barycentre where it lists none. A star on the ring's
 * normal, which leaves no direction in the plane, gives x along the galactic x axis in it, or the
 * galactic y axis where that is the normal too.
 */
export function ringedGiantIn(
  scene: Pick<ViewScene, "bodies" | "rings">,
  giant: BodyIdHex,
): PlacedGiant {
  const body = scene.bodies.find(({ id }) => id === giant);
  if (body === undefined) {
    return {
      kind: "gap",
      reason: `the served scene holds no body ${giant}`,
      shown: `the served scene holds no pinned giant, ${REPIN}`,
    };
  }
  let widest: ViewScene["rings"][number] | null = null;
  for (const ring of scene.rings) {
    if (
      ring.body === giant &&
      (widest === null ||
        ring.outerRadiusM - ring.innerRadiusM > widest.outerRadiusM - widest.innerRadiusM)
    ) {
      widest = ring;
    }
  }
  if (widest === null) {
    return {
      kind: "gap",
      reason: `the served scene gives ${giant} no ring`,
      shown: `the served scene gives the pinned giant no ring, ${REPIN}`,
    };
  }
  const annulus: RingedGiant = { ringInnerM: widest.innerRadiusM, ringOuterM: widest.outerRadiusM };
  if (!approachableRing(annulus)) {
    return {
      kind: "gap",
      reason: `${giant}'s widest ring, ${String(annulus.ringInnerM)} m to ${String(annulus.ringOuterM)} m, is no annulus inside 10⁹ m`,
      shown: `the pinned giant's widest ring is too wide for the approach, ${REPIN}`,
    };
  }
  const normal = unitOf(widest.normal);
  if (normal === null) {
    return {
      kind: "gap",
      reason: `the served scene gives ${giant}'s ring no normal`,
      shown: `the served scene gives the pinned giant's ring no plane, ${REPIN}`,
    };
  }
  const host = scene.bodies.find(({ kind }) => kind === "star")?.centreM ?? vec3(0, 0, 0);
  const toHost = vec3(host.x - body.centreM.x, host.y - body.centreM.y, host.z - body.centreM.z);
  // The ring's normal on the star's side, so that the approach starts over the rings' lit face.
  const z = dotOf(toHost, normal) < 0 ? vec3(-normal.x, -normal.y, -normal.z) : normal;
  const x =
    unitOf(inPlane(toHost, z)) ??
    unitOf(inPlane(vec3(1, 0, 0), z)) ??
    unitOf(inPlane(vec3(0, 1, 0), z));
  if (x === null) {
    throw new Error("a unit normal leaves a direction in its plane");
  }
  return {
    kind: "placed",
    giant: annulus,
    placement: { frame: { kind: "body", body: giant }, axes: [x, crossOf(z, x), z] },
  };
}
