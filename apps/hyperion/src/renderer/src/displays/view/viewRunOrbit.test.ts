/** `VIEW`'s `ORBIT` camera in a run (plan R15, T4): opened, zoomed, flown and held to the ground. */
import { describe, expect, it } from "vitest";

import { dot, norm, normalise, sub as sub3, type Vec3 } from "../../geometry/vec3";
import { sliceEarthInspection } from "../../test/inspectionFixture";
import { FIXTURE_EARTH } from "../../test/planetaryFixture";
import { ORBIT_FLOOR_M, orbitAltitudeM, orbitRadiusM } from "../../view/camera/orbit";
import { offeredPresets } from "../../view/camera/state";
import { rotateToBodyFixed } from "../../view/coords/rotation";
import { cameraSceneOf, sceneOrigins } from "../../view/scene/model";
import { inspectionScene } from "../../view/scenes/inspectionScene";
import type { KeptScene } from "../../view/scenes/kept";
import { PRECISION_PLANET, precisionScene } from "../../view/scenes/precision";
import { heldGroundUnder } from "../../view/terrain/grounded";
import { orbitGroundOver } from "./orbitGround";
import {
  commandRun,
  type FrameInput,
  framedRun,
  type OrbitGround,
  startRun,
  stepRun,
  type ViewRun,
} from "./viewRun";

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
    const radiusM = orbitRadiusM(body, run.camera.pose.positionM);
    const before = orbitAltitudeM(run.camera.pose, radiusM);
    const zoomed = commandRun(run, { kind: "zoom", steps: 2 }, CUT);
    if (zoomed.kind !== "done") {
      throw new Error("the wheel was refused in ORBIT");
    }
    expect(orbitAltitudeM(zoomed.run.camera.pose, radiusM)).toBeCloseTo(before / 1.25 ** 2, 0);
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

/** The slice's Earth as the camera holds it: its spheroid, polar radius under equatorial. */
function earthDatum(run: ViewRun) {
  const datum = cameraSceneOf(run.scene).orbitBodies.find((body) => body.id === FIXTURE_EARTH);
  if (datum === undefined) {
    throw new Error("the slice's Earth is not among the orbit's bodies");
  }
  return datum;
}

describe("the ORBIT camera about an oblate body", () => {
  it("holds the Earth's spheroid, its polar radius under its equatorial, about its pole", () => {
    const datum = earthDatum(
      startRun(orbiting(inspectionScene(sliceEarthInspection()), FIXTURE_EARTH)),
    );
    expect([datum.polarRadiusM < datum.equatorialRadiusM, datum.pole !== null]).toEqual([
      true,
      true,
    ]);
  });

  it("comes down to its floor above the spheroid along its line, off the equator too", () => {
    const run = startRun(orbiting(inspectionScene(sliceEarthInspection()), FIXTURE_EARTH));
    // Up the arrows to well off the equator, where the datum stands kilometres under the equator's.
    const north = after(run, 0.25, new Set(["ArrowUp"]));
    const datum = earthDatum(north);
    const off = datum.equatorialRadiusM - orbitRadiusM(datum, north.camera.pose.positionM);
    expect(off).toBeGreaterThan(1_000);
    const floor = commandRun(north, { kind: "zoom", steps: 400 }, CUT);
    if (floor.kind !== "done") {
      throw new Error("the wheel was refused in ORBIT");
    }
    const pose = floor.run.camera.pose;
    expect(orbitAltitudeM(pose, orbitRadiusM(datum, pose.positionM))).toBeCloseTo(ORBIT_FLOOR_M, 3);
  });
});

/** The slice's Earth's ORBIT run, opened about it. */
function earthRun(): ViewRun {
  return startRun(orbiting(inspectionScene(sliceEarthInspection()), FIXTURE_EARTH));
}

/**
 * Ground drawn `groundM` above the slice's Earth's datum everywhere, as a terrain pass's resident
 * heights give it (`heldGroundUnder` over a stand-in for `TerrainPass.heights`).
 */
function groundOf(run: ViewRun, groundM: number): OrbitGround {
  return orbitGroundOver(FIXTURE_EARTH, earthDatum(run), { groundHeightAtM: () => groundM }, null);
}

/** The camera's height above ground drawn `groundM` above the datum, m, along its normal. */
function aboveGround(run: ViewRun, groundM: number): number {
  return heldGroundUnder(earthDatum(run), { groundHeightAtM: () => groundM })(onGround(run))
    .heightM;
}

/** One frame of 0.1 s on, with `input`'s keys, drag and ground. */
function frame(run: ViewRun, input: Partial<FrameInput> = {}): ViewRun {
  return stepRun(run, {
    serverScene: null,
    dtS: 0.1,
    held: new Set(),
    reducedMotion: true,
    ...input,
  });
}

/** `run` zoomed by the wheel to its floor over what it last drew. */
function zoomedDown(run: ViewRun): ViewRun {
  const zoomed = commandRun(run, { kind: "zoom", steps: 400 }, CUT);
  if (zoomed.kind !== "done") {
    throw new Error("the wheel was refused in ORBIT");
  }
  return zoomed.run;
}

describe("the ORBIT camera over drawn ground (plan R15, T4)", () => {
  /** Ground 2 km above the datum, a plateau's. */
  const HIGH_M = 2_000;

  it("stops the wheel's zoom 10 m above ground drawn 2 km above the datum", () => {
    const run = earthRun();
    const ground = groundOf(run, HIGH_M);
    const down = zoomedDown(frame(run, { orbitGround: ground }));
    expect(aboveGround(down, HIGH_M)).toBeCloseTo(ORBIT_FLOOR_M, 1);
  });

  it("stops the zoom under W 10 m above that ground", () => {
    const run = zoomedDown(earthRun());
    const ground = groundOf(run, HIGH_M);
    const flown = frame(run, { held: new Set(["w"]), orbitGround: ground });
    expect(aboveGround(flown, HIGH_M)).toBeCloseTo(ORBIT_FLOOR_M, 1);
  });

  it("holds a drag over that ground 10 m above it", () => {
    const run = zoomedDown(earthRun());
    const ground = groundOf(run, HIGH_M);
    const dragged = frame(run, { turn: { yawRad: 0.2, pitchRad: 0.1 }, orbitGround: ground });
    expect(aboveGround(dragged, HIGH_M)).toBeCloseTo(ORBIT_FLOOR_M, 1);
  });

  it("lifts a still camera that ground drawn since stands over to 10 m above it", () => {
    const run = frame(zoomedDown(earthRun()));
    const ground = groundOf(run, HIGH_M);
    const lifted = frame(run, { orbitGround: ground });
    expect(aboveGround(lifted, HIGH_M)).toBeCloseTo(ORBIT_FLOOR_M, 1);
  });

  it("holds its floor above the datum where no ground is drawn", () => {
    const run = zoomedDown(earthRun());
    const datum = earthDatum(run);
    const pose = frame(run).camera.pose;
    expect(orbitAltitudeM(pose, orbitRadiusM(datum, pose.positionM))).toBeCloseTo(ORBIT_FLOOR_M, 1);
  });

  it("reads the ground in the body's fixed axes, under the camera alone", () => {
    const run = zoomedDown(earthRun());
    const under = normalise(onGround(run));
    // A plateau only about the camera's own body-fixed direction: read in other axes, it is missed.
    const plateau = orbitGroundOver(
      FIXTURE_EARTH,
      earthDatum(run),
      { groundHeightAtM: (d) => (dot(normalise(d), under) > 0.999_99 ? HIGH_M : 0) },
      null,
    );
    const lifted = frame(run, { orbitGround: plateau });
    expect(aboveGround(lifted, HIGH_M)).toBeCloseTo(ORBIT_FLOOR_M, 1);
  });

  it("holds its floor above the datum over another body's ground", () => {
    const run = zoomedDown(earthRun());
    const other = { ...groundOf(run, HIGH_M), body: "0000000000000000.0000" };
    const pose = frame(run, { orbitGround: other }).camera.pose;
    const datum = earthDatum(run);
    expect(orbitAltitudeM(pose, orbitRadiusM(datum, pose.positionM))).toBeCloseTo(ORBIT_FLOOR_M, 1);
  });

  it("stands 10 m above a sea's surface over a seabed below it", () => {
    const run = zoomedDown(earthRun());
    const sea = orbitGroundOver(
      FIXTURE_EARTH,
      earthDatum(run),
      { groundHeightAtM: () => -4_000 },
      136,
    );
    const pose = frame(run, { orbitGround: sea }).camera.pose;
    const datum = earthDatum(run);
    expect(orbitAltitudeM(pose, orbitRadiusM(datum, pose.positionM))).toBeCloseTo(
      136 + ORBIT_FLOOR_M,
      1,
    );
  });

  it("stands 10 m above dry ground below the datum", () => {
    const run = zoomedDown(earthRun());
    const basin = groundOf(run, -3_000);
    const pose = zoomedDown(frame(run, { orbitGround: basin })).camera.pose;
    expect(aboveGround({ ...run, camera: { ...run.camera, pose } }, -3_000)).toBeCloseTo(
      ORBIT_FLOOR_M,
      1,
    );
  });

  it("leaves a camera outside ORBIT as it is, whatever ground is drawn", () => {
    const run = startRun(precisionScene());
    const ground = groundOf(earthRun(), HIGH_M);
    expect(frame(run, { orbitGround: ground }).camera).toEqual(frame(run).camera);
  });
});
