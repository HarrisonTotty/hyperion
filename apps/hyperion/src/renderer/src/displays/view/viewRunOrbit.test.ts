/** `VIEW`'s `ORBIT` camera in a run (plan R15, T4): opened, zoomed, flown and held to the ground. */
import { describe, expect, it } from "vitest";

import { norm, sub as sub3, type Vec3 } from "../../geometry/vec3";
import { sliceEarthInspection } from "../../test/inspectionFixture";
import { FIXTURE_EARTH } from "../../test/planetaryFixture";
import { orbitAltitudeM } from "../../view/camera/orbit";
import { offeredPresets } from "../../view/camera/state";
import { rotateToBodyFixed } from "../../view/coords/rotation";
import { cameraSceneOf, sceneOrigins } from "../../view/scene/model";
import { inspectionScene } from "../../view/scenes/inspectionScene";
import type { KeptScene } from "../../view/scenes/kept";
import { PRECISION_PLANET, precisionScene } from "../../view/scenes/precision";
import { commandRun, framedRun, startRun, stepRun, type ViewRun } from "./viewRun";

const CUT = { easedMoves: false, reducedMotion: true } as const;

/** `kept` offering ORBIT about `body` and opening in it, as the viewer's scene does. */
function orbiting(kept: KeptScene, body: string): KeptScene {
  return {
    ...kept,
    startPreset: "orbit",
    sceneAt: (tS) => ({ ...kept.sceneAt(tS), orbitHome: body }),
  };
}

/** The precision scene orbiting its planet. */
function orbitingPrecision(): KeptScene {
  return orbiting(precisionScene(), PRECISION_PLANET);
}

/** One still frame `dtS` on, no key held. */
function after(run: ViewRun, dtS: number, held: ReadonlySet<string> = new Set()): ViewRun {
  return stepRun(run, { serverScene: null, dtS, held, reducedMotion: true });
}

describe("the ORBIT camera in a run", () => {
  it("is offered only by a scene that names its body", () => {
    expect([
      offeredPresets(cameraSceneOf(startRun(precisionScene()).scene)).includes("orbit"),
      offeredPresets(cameraSceneOf(startRun(orbitingPrecision()).scene)).includes("orbit"),
    ]).toEqual([false, true]);
  });

  it("refuses its key in a scene that names no body", () => {
    expect(
      commandRun(startRun(precisionScene()), { kind: "preset", preset: "orbit" }, CUT),
    ).toEqual({ kind: "refused", reason: "no_body" });
  });

  it("refuses the wheel outside ORBIT", () => {
    expect(commandRun(startRun(precisionScene()), { kind: "zoom", steps: 1 }, CUT)).toEqual({
      kind: "refused",
      reason: "not_orbit",
    });
  });

  it("opens a scene that asks for it in ORBIT about its body, and zooms it by the wheel", () => {
    const run = startRun(orbitingPrecision());
    expect(run.camera.preset).toBe("orbit");
    expect(run.camera.pose.frame).toEqual({ kind: "body", body: PRECISION_PLANET });
    const body = cameraSceneOf(run.scene).orbitBodies.find((each) => each.id === PRECISION_PLANET);
    if (body === undefined) {
      throw new Error("the orbit's body is not among the scene's");
    }
    const before = orbitAltitudeM(run.camera.pose, body.radiusM);
    const zoomed = commandRun(run, { kind: "zoom", steps: 2 }, CUT);
    if (zoomed.kind !== "done") {
      throw new Error("the wheel was refused in ORBIT");
    }
    expect(orbitAltitudeM(zoomed.run.camera.pose, body.radiusM)).toBeCloseTo(before / 1.25 ** 2, 0);
  });

  it("zooms in under W", () => {
    const run = startRun(orbitingPrecision());
    const nearer = after(run, 0.25, new Set(["w"]));
    expect(norm(nearer.camera.pose.positionM)).toBeLessThan(norm(run.camera.pose.positionM));
  });

  it("swings round under the arrows, keeping its distance", () => {
    const run = startRun(orbitingPrecision());
    const swung = after(run, 0.25, new Set(["ArrowRight"]));
    expect([
      norm(swung.camera.pose.positionM) / norm(run.camera.pose.positionM),
      swung.camera.pose.positionM === run.camera.pose.positionM,
    ]).toEqual([expect.closeTo(1, 9), false]);
  });
});

/** The camera's place in the slice's Earth's body-fixed axes. */
function onGround(run: ViewRun): Vec3 {
  const rotation = sceneOrigins(run.scene).bodyFixedRotation(FIXTURE_EARTH);
  if (rotation === null) {
    throw new Error("the slice's Earth has its rotation");
  }
  return rotateToBodyFixed(rotation, run.camera.pose.positionM);
}

describe("the ORBIT camera about an inspected body", () => {
  it("takes the inspection's framed pose where it is about the framed body", () => {
    const kept = orbiting(inspectionScene(sliceEarthInspection()), FIXTURE_EARTH);
    const run = startRun(kept);
    const framed = kept.framedPoseFor?.(1.26);
    expect(framed?.frame).toEqual({ kind: "body", body: FIXTURE_EARTH });
    expect(framedRun(run, 1.26).camera.pose).toEqual(framed);
  });

  it("keeps its own pose where it is about another body than the framed one", () => {
    const kept = inspectionScene(sliceEarthInspection());
    const other = cameraSceneOf(kept.sceneAt(0)).orbitBodies.find(
      (body) => body.id !== FIXTURE_EARTH,
    );
    if (other === undefined) {
      throw new Error("the slice holds a body besides its Earth");
    }
    const run = startRun(orbiting(kept, other.id));
    expect(run.camera.pose.frame).toEqual({ kind: "body", body: other.id });
    expect(framedRun(run, 1.26)).toBe(run);
  });

  it("turns with its body's spin, so that it hovers over one place of the ground", () => {
    const run = startRun(orbiting(inspectionScene(sliceEarthInspection()), FIXTURE_EARTH));
    // One frame, at most `MAX_FREE_STEP_S` of the script's time.
    const later = after(run, 0.1);
    const [before, now] = [onGround(run), onGround(later)];
    expect([now.x - before.x, now.y - before.y, now.z - before.z].map(Math.abs)).toEqual([
      expect.closeTo(0, 0),
      expect.closeTo(0, 0),
      expect.closeTo(0, 0),
    ]);
    // The ground turned under the galactic axes, about 0.5 km at the frame's distance in 0.1 s: the
    // camera moved with it.
    expect(norm(sub3(later.camera.pose.positionM, run.camera.pose.positionM))).toBeGreaterThan(100);
  });
});
