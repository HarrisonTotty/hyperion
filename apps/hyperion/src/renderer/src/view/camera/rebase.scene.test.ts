import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { add, cross, norm, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import { expressIn, type ViewPosition } from "../coords/position";
import { type CameraOrigins, relativeToCamera } from "../coords/relative";
import { TEST_HULL } from "../scene/hull";
import { cameraSceneOf, sceneOrigins } from "../scene/model";
import { KEPT_SYSTEM } from "../scenes/kept";
import {
  FRAME_CHANGE_DURATION_S,
  FRAME_CHANGE_LANDER,
  FRAME_CHANGE_LANDER_AT,
  FRAME_CHANGE_LANDMARK,
  FRAME_CHANGE_MOON,
  FRAME_CHANGE_MOON_HILL_M,
  FRAME_CHANGE_MOON_M,
  FRAME_CHANGE_PLANET,
  FRAME_CHANGE_PLANET_HILL_M,
  FRAME_CHANGE_PLANET_M,
  FRAME_CHANGE_SHIP,
  FRAME_CHANGE_SPIN_RAD_PER_S,
  frameChangeCameraAt,
  frameChangeRotationAt,
  frameChangeScene,
  frameChangeShipAt,
} from "../scenes/frameChange";
import { BODY_FRAME_ENTRY, cameraFrameCandidate, selectCameraFrame } from "./frames";
import type { CameraFrame, CameraPose } from "./pose";
import { DEFAULT_FOV_DEG, project, type Viewport } from "./projection";
import { lookAlong } from "./quaternion";
import { rebase, sameCameraFrame } from "./rebase";
import { CHASE_OFFSET_HULL_LENGTHS, sceneFrameFor } from "./state";

const KEPT = frameChangeScene();
const STEP_S = 0.25;
const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const FOV_X_RAD = (DEFAULT_FOV_DEG * Math.PI) / 180;
const SYSTEM_FRAME: CameraFrame = { kind: "system", system: KEPT_SYSTEM };

/** Each Hill sphere of the scene: its centre and radius, m. */
const SPHERES: Readonly<Record<string, { readonly centreM: Vec3; readonly hillM: number }>> = {
  [FRAME_CHANGE_PLANET]: { centreM: FRAME_CHANGE_PLANET_M, hillM: FRAME_CHANGE_PLANET_HILL_M },
  [FRAME_CHANGE_MOON]: { centreM: FRAME_CHANGE_MOON_M, hillM: FRAME_CHANGE_MOON_HILL_M },
};

/**
 * What a frame change did: when, into which frame, the landmark's jump in its vector (m) and pixel
 * (px), and where the change fell on the sphere's boundary: the ratio of distance to Hill radius,
 * and how far that ratio moved over the step.
 */
interface ChangeRecord {
  readonly tS: number;
  readonly to: BodyIdHex | null;
  readonly jumpM: number;
  readonly jumpPx: number;
  readonly ratio: number;
  readonly stepRatio: number;
}

/** The landmark as a camera sees it: its vector from the camera and its pixel. */
interface Seen {
  readonly v: Vec3;
  readonly xPx: number;
  readonly yPx: number;
}

/** The script's times, every {@link STEP_S}. */
const TIMES: readonly number[] = Array.from(
  { length: Math.floor(FRAME_CHANGE_DURATION_S / STEP_S) + 1 },
  (_, i) => i * STEP_S,
);

/** The body a camera frame is, or `null` for the system frame. */
function bodyOf(frame: CameraFrame): BodyIdHex | null {
  return frame.kind === "body" ? frame.body : null;
}

/** The landmark from `pose`, which looks at the planet. */
function landmarkSeen(pose: CameraPose, origins: CameraOrigins): Seen {
  const v = relativeToCamera(FRAME_CHANGE_LANDMARK, pose, origins);
  const p = project(v, { orientation: pose.orientation, fovXRad: FOV_X_RAD }, VIEWPORT);
  return { v, xPx: p.xPx, yPx: p.yPx };
}

/**
 * A change's record. The ratio is the entered sphere's on an entry and the left sphere's on an
 * exit, with the step's motion in that ratio, from the positions at `tS` and one step before.
 */
function record(
  tS: number,
  from: CameraFrame,
  to: CameraFrame,
  before: Seen,
  after: Seen,
  path: (t: number) => Vec3,
): ChangeRecord {
  const entering = to.kind === "body" && (from.kind !== "body" || to.body === FRAME_CHANGE_MOON);
  const body = entering ? bodyOf(to) : bodyOf(from);
  const sphere = body === null ? undefined : SPHERES[body];
  if (sphere === undefined) {
    throw new Error(`no Hill sphere for the change into ${String(bodyOf(to))}`);
  }
  const ratioAt = (t: number): number => norm(sub(path(t), sphere.centreM)) / sphere.hillM;
  return {
    tS,
    to: bodyOf(to),
    jumpM: norm(sub(before.v, after.v)),
    jumpPx: Math.hypot(before.xPx - after.xPx, before.yPx - after.yPx),
    ratio: ratioAt(tS),
    stepRatio: Math.abs(ratioAt(tS) - ratioAt(tS - STEP_S)),
  };
}

/**
 * The own ship's frame changes along its path, with a chase camera held about it looking at the
 * planet. A seat or chase camera is held in the ship's `craft` frame (Design note 22), so it is the
 * ship's position that changes frame: at each change it is re-expressed in its new frame, as the
 * flight model will hold it, and the landmark is seen from the same camera in the old frame and
 * the new.
 */
function shipChanges(): ChangeRecord[] {
  let shipFrame = SYSTEM_FRAME;
  const changes: ChangeRecord[] = [];
  for (const tS of TIMES) {
    const scene = KEPT.sceneAt(tS);
    const base = sceneOrigins(scene);
    const shipM = frameChangeShipAt(tS);
    const candidates = cameraSceneOf(scene).frameBodies.map((body) =>
      cameraFrameCandidate(
        body.id,
        body.parent,
        norm(sub(shipM, base.bodyCentreM(body.id))),
        body.hillRadiusM,
      ),
    );
    const selected = selectCameraFrame(candidates, bodyOf(shipFrame));
    const next: CameraFrame = selected === null ? SYSTEM_FRAME : { kind: "body", body: selected };
    if (!sameCameraFrame(next, shipFrame)) {
      const inSystem: ViewPosition = { kind: "system", system: KEPT_SYSTEM, m: shipM };
      const shipIn = (frame: CameraFrame): CameraOrigins => ({
        ...base,
        craftPosition: () =>
          expressIn(
            inSystem,
            frame.kind === "body" ? frame : { kind: "system", system: KEPT_SYSTEM },
            base,
          ),
      });
      const offsetM = scale(CHASE_OFFSET_HULL_LENGTHS, TEST_HULL.lengthM);
      const chase: CameraPose = {
        frame: { kind: "craft", craft: FRAME_CHANGE_SHIP },
        positionM: offsetM,
        orientation: lookAlong(
          sub(base.bodyCentreM(FRAME_CHANGE_PLANET), add(shipM, offsetM)),
          vec3(0, 0, 1),
        ),
      };
      changes.push(
        record(
          tS,
          shipFrame,
          next,
          landmarkSeen(chase, shipIn(shipFrame)),
          landmarkSeen(chase, shipIn(next)),
          frameChangeShipAt,
        ),
      );
      shipFrame = next;
    }
  }
  return changes;
}

/**
 * The scripted free camera's own frame changes: it moves in whatever frame it is held in, its frame
 * is re-selected each step, and the landmark is seen before and after each rebase.
 */
function freeCameraChanges(): ChangeRecord[] {
  let pose = KEPT.cameraAt(0);
  const changes: ChangeRecord[] = [];
  for (const tS of TIMES) {
    const scene = cameraSceneOf(KEPT.sceneAt(tS));
    pose = rebase(KEPT.cameraAt(tS), pose.frame, scene.origins).pose;
    const next = sceneFrameFor(pose, scene);
    const rebased = rebase(pose, next, scene.origins);
    if (rebased.change !== null) {
      changes.push(
        record(
          tS,
          pose.frame,
          next,
          landmarkSeen(pose, scene.origins),
          landmarkSeen(rebased.pose, scene.origins),
          frameChangeCameraAt,
        ),
      );
    }
    pose = rebased.pose;
  }
  return changes;
}

const SHIP = shipChanges();
const FREE = freeCameraChanges();
const EXPECTED_FRAMES = [FRAME_CHANGE_PLANET, FRAME_CHANGE_MOON, FRAME_CHANGE_PLANET, null];

/** How far each change fell from its threshold: 0.9 on entry, 1 on exit, as a count of steps. */
function stepsFromThreshold(changes: readonly ChangeRecord[]): number[] {
  return changes.map((c, i) =>
    i < 2 ? (BODY_FRAME_ENTRY - c.ratio) / c.stepRatio : (c.ratio - 1) / c.stepRatio,
  );
}

describe.each([
  ["the own ship", SHIP],
  ["the free camera", FREE],
])("rebasing as %s's frame changes along the frame-change scene", (_, changes) => {
  it("goes through the planet's and the moon's frames and out", () => {
    expect(changes.map((c) => c.to)).toEqual(EXPECTED_FRAMES);
  });

  it("enters at a ratio of at most 0.9 and leaves above 1, each within a step (the hysteresis)", () => {
    const steps = stepsFromThreshold(changes);
    expect(steps.filter((s) => !(s >= 0 && s < 1))).toEqual([]);
  });

  it("keeps the landmark's vector continuous to 1 mm across each change", () => {
    expect(changes.filter((c) => !(c.jumpM < 1e-3))).toEqual([]);
  });

  it("keeps the landmark's pixel continuous to 1 px across each change", () => {
    expect(changes.filter((c) => !(c.jumpPx < 1))).toEqual([]);
  });
});

describe("the free camera in the frame-change scene", () => {
  it("changes frame at other moments from the own ship", () => {
    const ship = new Set(SHIP.map((c) => c.tS));
    expect(FREE.filter((c) => ship.has(c.tS))).toEqual([]);
  });
});

describe("a grounded craft in the frame-change scene", () => {
  /** The lander's position in the planet's non-rotating frame at `tS`, m. */
  function inBodyFrame(tS: number): Vec3 {
    const lander = expressIn(
      FRAME_CHANGE_LANDER_AT,
      { kind: "body", body: FRAME_CHANGE_PLANET },
      sceneOrigins(KEPT.sceneAt(tS)),
    );
    if (lander.kind !== "body") {
      throw new Error("a position expressed in a body frame must be a body position");
    }
    return lander.m;
  }

  it("keeps its body-fixed position throughout the script", () => {
    const held = TIMES.map((tS) => {
      const lander = KEPT.sceneAt(tS).craft.find((c) => c.id === FRAME_CHANGE_LANDER);
      if (lander === undefined) {
        throw new Error(`the scene has no lander at ${String(tS)} s`);
      }
      return lander.pose.position;
    });
    expect(held.filter((p) => p !== FRAME_CHANGE_LANDER_AT)).toEqual([]);
  });

  it("moves in the body frame at ω × r within 10⁻⁶", () => {
    const tS = 100;
    const dtS = 0.5;
    const velocity = scale(sub(inBodyFrame(tS + dtS), inBodyFrame(tS - dtS)), 1 / (2 * dtS));
    const [r0, r1, r2] = frameChangeRotationAt(tS).rows;
    const pole = vec3(r0.z, r1.z, r2.z);
    const expected = cross(scale(pole, FRAME_CHANGE_SPIN_RAD_PER_S), inBodyFrame(tS));
    expect(norm(sub(velocity, expected)) / norm(expected)).toBeLessThan(1e-6);
  });
});
