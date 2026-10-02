import { describe, expect, it } from "vitest";

import { add, dot, norm, normalise, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import {
  aCameraPose,
  aCameraScene,
  anOwnShip,
  FIXTURE_MOON,
  FIXTURE_MOON_CENTRE_M,
  FIXTURE_PLANET,
  FIXTURE_SHIP,
  FIXTURE_SYSTEM,
} from "../../test/viewFixtures";
import { relativeToCamera } from "../coords/relative";
import { type FreeCameraInput, stepFreeCamera } from "./freeCamera";
import type { CameraPose } from "./pose";
import { quaternionFromAxisAngle, rotate } from "./quaternion";
import { rebase } from "./rebase";
import {
  advanceEasedMove,
  type CameraState,
  CHASE_OFFSET_HULL_LENGTHS,
  cutTo,
  type CutOptions,
  displayPose,
  EASED_MOVE_S,
  followPreset,
  newCameraState,
  nextTarget,
  offeredPresets,
  rebaseState,
  sceneFrameFor,
  onSystemChange,
  stepFov,
  targetPosition,
  viewId,
} from "./state";

const CUT: CutOptions = { easedMoves: false, reducedMotion: false };
const EASED: CutOptions = { easedMoves: true, reducedMotion: false };

/** The camera's forward direction (its −z) in its frame's axes. */
function forwardOf(pose: CameraPose): Vec3 {
  return rotate(pose.orientation, vec3(0, 0, -1));
}

function cut(state: CameraState, to: Parameters<typeof cutTo>[1], options = CUT): CameraState {
  const result = cutTo(state, to, aCameraScene(), options);
  if (result.kind !== "cut") {
    throw new Error(`expected a cut, got ${result.reason}`);
  }
  return result.state;
}

/** The distance between two vectors. */
function apart(a: Vec3, b: Vec3): number {
  return norm(sub(a, b));
}

describe("camera presets", () => {
  it("puts the seat at the own ship's eye point, in the ship's craft frame, looking forward", () => {
    const attitude = quaternionFromAxisAngle(vec3(0, 0, 1), 0.3);
    const scene = aCameraScene({ ownShip: anOwnShip({ attitude }) });
    const state = newCameraState(scene, "eye");
    expect(state.preset).toBe("seat");
    expect(state.pose.frame).toEqual({ kind: "craft", craft: FIXTURE_SHIP });
    expect(apart(state.pose.positionM, rotate(attitude, vec3(0, 1.5, -6)))).toBeLessThan(1e-12);
    expect(apart(forwardOf(state.pose), rotate(attitude, vec3(0, 0, -1)))).toBeLessThan(1e-12);
  });

  it("turns the seat aft and towards the target", () => {
    const scene = aCameraScene();
    const seat = newCameraState(scene, "eye");
    const aft = cut(seat, { kind: "look", look: "aft" });
    expect(apart(forwardOf(aft.pose), vec3(0, 0, 1))).toBeLessThan(1e-12);
    const atMoon = cut(seat, { kind: "target", target: { kind: "body", body: FIXTURE_MOON } });
    expect(atMoon.look).toBe("target");
    const toMoon = relativeToCamera(
      targetPosition({ kind: "body", body: FIXTURE_MOON }, scene.origins),
      atMoon.pose,
      scene.origins,
    );
    expect(dot(forwardOf(atMoon.pose), normalise(toMoon))).toBeCloseTo(1, 12);
  });

  it("holds the chase camera astern and above the ship, looking at it", () => {
    const state = cut(newCameraState(aCameraScene(), "camera"), {
      kind: "preset",
      preset: "chase",
    });
    expect(state.pose.frame).toEqual({ kind: "craft", craft: FIXTURE_SHIP });
    expect(apart(state.pose.positionM, scale(CHASE_OFFSET_HULL_LENGTHS, 20))).toBeLessThan(1e-9);
    expect(dot(forwardOf(state.pose), normalise(scale(state.pose.positionM, -1)))).toBeCloseTo(
      1,
      12,
    );
  });

  it("detaches the free camera where it stands, in the frame the body rule selects", () => {
    const scene = aCameraScene();
    const seat = newCameraState(scene, "camera");
    const free = cut(seat, { kind: "preset", preset: "free" });
    // The ship is 2 × 10⁷ m from the planet, well inside its Hill sphere and outside the moon's.
    expect(free.pose.frame).toEqual({ kind: "body", body: FIXTURE_PLANET });
    expect(apart(free.pose.positionM, vec3(2e7, 1.5, -6))).toBeLessThan(1e-6);
    expect(free.pose.orientation).toEqual(seat.pose.orientation);
  });

  it("starts a view with an own ship in seat", () => {
    expect(newCameraState(aCameraScene(), "eye").preset).toBe("seat");
  });

  it("starts a view without an own ship in free at the default pose", () => {
    const scene = aCameraScene({ ownShip: null });
    const state = newCameraState(scene, "camera");
    expect({ preset: state.preset, pose: state.pose }).toEqual({
      preset: "free",
      pose: scene.defaultPose,
    });
  });

  it("offers only free without an own ship", () => {
    expect([
      offeredPresets(aCameraScene({ ownShip: null })),
      offeredPresets(aCameraScene()),
    ]).toEqual([["free"], ["seat", "chase", "free"]]);
  });

  it("refuses seat and chase with no own ship", () => {
    const scene = aCameraScene({ ownShip: null });
    const state = newCameraState(scene, "camera");
    const results = (["seat", "chase"] as const).map((preset) =>
      cutTo(state, { kind: "preset", preset }, scene, CUT),
    );
    expect(results).toEqual([
      { kind: "refused", reason: "no_own_ship" },
      { kind: "refused", reason: "no_own_ship" },
    ]);
  });

  it("holds a free camera given a craft as its target in that craft's frame", () => {
    const scene = aCameraScene();
    const free = cut(newCameraState(scene, "camera"), { kind: "preset", preset: "free" });
    const orbiting = cut(free, { kind: "target", target: { kind: "craft", craft: FIXTURE_SHIP } });
    expect({
      frame: orbiting.pose.frame,
      atSeat: apart(orbiting.pose.positionM, vec3(0, 1.5, -6)) < 1e-6,
    }).toEqual({ frame: { kind: "craft", craft: FIXTURE_SHIP }, atSeat: true });
  });
});

describe("cuts and eased moves", () => {
  it("changes the pose in one step on a cut", () => {
    const seat = newCameraState(aCameraScene(), "camera");
    const chase = cut(seat, { kind: "preset", preset: "chase" });
    expect(displayPose(chase, aCameraScene().origins)).toBe(chase.pose);
  });

  it("eases a move over 0.4 s under the setting", () => {
    const scene = aCameraScene();
    const seat = newCameraState(scene, "camera");
    const moving = cut(seat, { kind: "preset", preset: "chase" }, EASED);
    const start = displayPose(moving, scene.origins).positionM;
    const between = displayPose(
      advanceEasedMove(moving, EASED_MOVE_S / 2),
      scene.origins,
    ).positionM;
    const almost = advanceEasedMove(moving, EASED_MOVE_S * 0.99);
    const done = advanceEasedMove(moving, EASED_MOVE_S);
    expect({
      startsOnScreen: apart(start, seat.pose.positionM) < 1e-9,
      halfwayBetween:
        apart(between, seat.pose.positionM) > 1 && apart(between, moving.pose.positionM) > 1,
      runningAt99: almost.move !== null,
      doneAt04: done.move === null && displayPose(done, scene.origins) === moving.pose,
    }).toEqual({ startsOnScreen: true, halfwayBetween: true, runningAt99: true, doneAt04: true });
  });

  it("reports the frame change a cut makes, and none where the frame stays", () => {
    const scene = aCameraScene();
    const free = cut(newCameraState(scene, "camera"), { kind: "preset", preset: "free" });
    const toChase = cutTo(free, { kind: "preset", preset: "chase" }, scene, CUT);
    const toSeat = cutTo(
      newCameraState(scene, "camera"),
      { kind: "preset", preset: "chase" },
      scene,
      CUT,
    );
    expect([
      toChase.kind === "cut" ? toChase.change : "refused",
      toSeat.kind === "cut" ? toSeat.change : "refused",
    ]).toEqual([
      { from: { kind: "body", body: FIXTURE_PLANET }, to: { kind: "craft", craft: FIXTURE_SHIP } },
      null,
    ]);
  });

  it("makes an eased move a cut under reduced motion", () => {
    const seat = newCameraState(aCameraScene(), "camera");
    const chase = cut(
      seat,
      { kind: "preset", preset: "chase" },
      { easedMoves: true, reducedMotion: true },
    );
    expect(chase.move).toBeNull();
  });

  it("keeps an eased move's blend continuous across a frame change", () => {
    const scene = aCameraScene();
    const seat = newCameraState(scene, "camera");
    const moving = advanceEasedMove(cut(seat, { kind: "preset", preset: "chase" }, EASED), 0.1);
    const onScreen = displayPose(moving, scene.origins);
    const { pose, change } = rebase(
      moving.pose,
      { kind: "body", body: FIXTURE_PLANET },
      scene.origins,
    );
    const rebased = rebaseState(moving, pose, change, scene.origins);
    const after = rebase(displayPose(rebased, scene.origins), onScreen.frame, scene.origins).pose;
    expect(apart(after.positionM, onScreen.positionM)).toBeLessThan(1e-6);
  });

  it("recomputes the seat from the ship's attitude each frame", () => {
    const seat = newCameraState(aCameraScene(), "eye");
    const attitude = quaternionFromAxisAngle(vec3(0, 1, 0), 1);
    const turned = followPreset(seat, aCameraScene({ ownShip: anOwnShip({ attitude }) }));
    expect(apart(forwardOf(turned.pose), rotate(attitude, vec3(0, 0, -1)))).toBeLessThan(1e-12);
  });

  it("keeps the chase camera astern of a turned ship", () => {
    const chase = cut(newCameraState(aCameraScene(), "camera"), {
      kind: "preset",
      preset: "chase",
    });
    const attitude = quaternionFromAxisAngle(vec3(0, 1, 0), 1);
    const turned = followPreset(chase, aCameraScene({ ownShip: anOwnShip({ attitude }) }));
    const astern = rotate(attitude, scale(CHASE_OFFSET_HULL_LENGTHS, 20));
    expect(apart(turned.pose.positionM, astern)).toBeLessThan(1e-9);
  });
});

describe("frame selection", () => {
  // The scene draws the moon at FIXTURE_MOON_CENTRE_M, where the ship sees it; were its geometric
  // centre 10⁸ m away, the camera's selection would not consult it, since a camera scene holds only
  // drawn centres (Design note 6, as amended 2026-10-02; `fromServer.test.ts` shifts one).
  const scene = aCameraScene({ ownShip: null });
  const besideMoonM = vec3(0, 1e7, 0);

  it("puts a camera beside a drawn moon in its frame, whatever frame holds its pose", () => {
    // 10⁷ m from the drawn moon, inside its 5.8 × 10⁷ m Hill sphere.
    const inSystem = aCameraPose({ positionM: add(FIXTURE_MOON_CENTRE_M, besideMoonM) });
    const inMoon = aCameraPose({
      frame: { kind: "body", body: FIXTURE_MOON },
      positionM: besideMoonM,
    });
    expect([sceneFrameFor(inSystem, scene), sceneFrameFor(inMoon, scene)]).toEqual([
      { kind: "body", body: FIXTURE_MOON },
      { kind: "body", body: FIXTURE_MOON },
    ]);
  });

  it("keeps a camera at rest beside a drawn moon in its frame, never leaving and re-entering", () => {
    const start = aCameraPose({ positionM: add(FIXTURE_MOON_CENTRE_M, besideMoonM) });
    const frame = sceneFrameFor(start, scene);
    const inFrame = rebase(start, frame, scene.origins).pose;
    let state = newCameraState(aCameraScene({ ownShip: null, defaultPose: inFrame }), "camera");
    // Turning in place re-selects the frame at every step while the camera stays where it is.
    const look: FreeCameraInput = { translate: vec3(0, 0, 0), rotate: vec3(0, 1, 0) };
    const drawnAt = (pose: CameraPose): Vec3 =>
      rebase(pose, { kind: "system", system: FIXTURE_SYSTEM }, scene.origins).pose.positionM;
    const frames: CameraPose["frame"][] = [];
    let farthestM = 0;
    for (let step = 0; step < 100; step += 1) {
      state = stepFreeCamera(state, look, 1 / 60, false, scene).state;
      frames.push(state.pose.frame);
      farthestM = Math.max(farthestM, norm(sub(drawnAt(state.pose), drawnAt(start))));
    }
    expect(state.preset).toBe("free");
    expect(new Set(frames.map((each) => JSON.stringify(each)))).toEqual(
      new Set([JSON.stringify({ kind: "body", body: FIXTURE_MOON })]),
    );
    expect(farthestM).toBeLessThan(1e-3);
  });
});

describe("a change of system", () => {
  it("returns a free camera to chase about the own ship", () => {
    const scene = aCameraScene();
    const free = cut(newCameraState(scene, "camera"), { kind: "preset", preset: "free" });
    expect(onSystemChange(free, scene).preset).toBe("chase");
  });

  it("returns a free camera to the default pose where there is no own ship", () => {
    const scene = aCameraScene();
    const free = cut(newCameraState(scene, "camera"), { kind: "preset", preset: "free" });
    const alone = aCameraScene({ ownShip: null, defaultPose: aCameraPose() });
    expect(onSystemChange(free, alone).pose).toEqual(alone.defaultPose);
  });

  it("leaves a seat camera in the seat", () => {
    const scene = aCameraScene();
    const seat = newCameraState(scene, "eye");
    expect(onSystemChange(seat, scene).pose).toEqual(seat.pose);
  });
});

describe("targets", () => {
  it("steps through the scene's targets, wrapping at the ends", () => {
    const scene = aCameraScene();
    const state = newCameraState(scene, "camera");
    const atMoon = cut(state, { kind: "target", target: { kind: "body", body: FIXTURE_MOON } });
    expect([
      nextTarget(state, scene, 1),
      nextTarget(state, scene, -1),
      nextTarget(atMoon, scene, 1),
    ]).toEqual([
      { kind: "body", body: FIXTURE_PLANET },
      { kind: "body", body: FIXTURE_MOON },
      { kind: "body", body: FIXTURE_PLANET },
    ]);
  });

  it("has no next target in a scene without targets", () => {
    const scene = aCameraScene({ targets: [] });
    expect(nextTarget(newCameraState(scene, "camera"), scene, 1)).toBeNull();
  });

  it("turns a free camera to look at a new target in place", () => {
    const scene = aCameraScene({ ownShip: null });
    const free = newCameraState(scene, "camera");
    const result = cutTo(
      free,
      { kind: "target", target: { kind: "body", body: FIXTURE_MOON } },
      scene,
      CUT,
    );
    if (result.kind !== "cut") {
      throw new Error("expected a cut");
    }
    const { pose } = result.state;
    const toMoon = relativeToCamera(
      targetPosition({ kind: "body", body: FIXTURE_MOON }, scene.origins),
      pose,
      scene.origins,
    );
    expect({
      frame: pose.frame,
      position: pose.positionM,
      facing: dot(forwardOf(pose), normalise(toMoon)) > 1 - 1e-12,
    }).toEqual({
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      position: free.pose.positionM,
      facing: true,
    });
  });
});

describe("the field of view", () => {
  it("steps along its steps and holds at the ends", () => {
    expect([
      stepFov(60, -1),
      stepFov(60, 1),
      stepFov(10, -1),
      stepFov(120, 1),
      stepFov(50, 1),
    ]).toEqual([45, 90, 10, 120, 60]);
  });
});

describe("viewId", () => {
  it("builds an identity from a name", () => {
    expect(viewId("main")).toBe("main");
  });

  it("refuses an empty name", () => {
    expect(() => viewId("")).toThrow(RangeError);
  });
});
