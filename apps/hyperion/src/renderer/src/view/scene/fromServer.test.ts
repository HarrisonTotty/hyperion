import {
  type BodyGrantDto,
  type CameraReportDto,
  galacticPositionFromLy,
  type SceneStateDto,
  type UniverseTime,
} from "@hyperion/protocol";
import { afterEach, describe, expect, it, vi } from "vitest";

import { stepRun, startServerRun, type ViewRun } from "../../displays/view/viewRun";
import { bodyPositionM, layoutBodies } from "../../displays/system/bodyMap";
import { add, norm, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import { type SceneFrame, sceneAt, shipObserver } from "../../lib/scene/apparent";
import { CAMERA_REPORT_INTERVAL_MS, CameraReporter } from "../../lib/scene/cameraReports";
import type { SceneModel, SystemPlace } from "../../lib/scene/model";
import { toSceneModel } from "../../lib/scene/sceneWire";
import { layoutHierarchy } from "../../lib/system/hierarchy";
import {
  FIXTURE_EARTH,
  FIXTURE_JUPITER,
  FIXTURE_SYSTEM,
  populatedBodies,
} from "../../test/planetaryFixture";
import {
  designateFixture,
  SCENE_DESIGNATION,
  SCENE_TIDAL_RADIUS_M,
  sceneClock,
  shipInSystem,
  sliceSceneSystem,
  stateAfterHeartbeat,
  stateInSpace,
} from "../../test/sceneFixture";
import type { CameraPose } from "../camera/pose";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { sceneFrameFor, viewId } from "../camera/state";
import { galacticTranslated } from "../coords/position";
import { TEST_HULL } from "./hull";
import {
  cameraKinematics,
  SERVER_OWN_SHIP,
  serverSceneAtFrame,
  serverSceneAtPush,
  serverSceneGap,
  viewSceneFromServer,
} from "./fromServer";
import { cameraSceneOf, type ViewBody, type ViewScene } from "./model";

afterEach(() => {
  vi.useRealTimers();
});

const PLACE = {
  kind: "charted",
  system: FIXTURE_SYSTEM,
  designation: SCENE_DESIGNATION,
  barycentre: galacticPositionFromLy([8_000, 26_000, 20]),
} as const satisfies SystemPlace;

/** The ship 10,000 km from the slice's Earth, in the Earth's frame, at `seconds`. */
function stateNearEarth(seconds = 3_000): SceneStateDto {
  return {
    sequence: 0,
    clock: sceneClock(seconds),
    ship: {
      position: { frame: "body", body: FIXTURE_EARTH, offset_m: [1e7, 0, 0] },
      velocity_m_s: [0, 7_000, 0],
      time: { seconds, nanos: 0 },
    },
    system: sliceSceneSystem(),
    tidal_radius_m: SCENE_TIDAL_RADIUS_M,
    craft: [],
  };
}

/** The populated answer as a scene's system: every kind, a moon and a ring among them. */
function statePopulated(): SceneStateDto {
  const { kind: _kind, ...system } = populatedBodies();
  return {
    sequence: 0,
    clock: sceneClock(3_000),
    ship: shipInSystem(3_000),
    system: {
      system,
      grants: system.bodies.map((body): BodyGrantDto => ({ body: body.id, level: system.granted })),
    },
    tidal_radius_m: SCENE_TIDAL_RADIUS_M,
    craft: [],
  };
}

function modelOf(state: SceneStateDto): SceneModel {
  const result = toSceneModel(state, designateFixture);
  if (result.kind !== "ok") {
    throw new Error(`the fixture's scene is unusable: ${result.fault}`);
  }
  return result.model;
}

function frameOf(model: SceneModel, time: UniverseTime = model.clock.time): SceneFrame {
  const observer = shipObserver(model, time);
  const frame = observer === null ? null : sceneAt(model, observer, time, null);
  if (frame === null) {
    throw new Error("the fixture's scene has no frame");
  }
  return frame;
}

function sceneOf(model: SceneModel, frame: SceneFrame, place: SystemPlace = PLACE): ViewScene {
  const scene = viewSceneFromServer(model, frame, place);
  if (scene === null) {
    throw new Error("the fixture's scene cannot be drawn");
  }
  return scene;
}

function bodyOf(scene: ViewScene, id: string): ViewBody {
  const body = scene.bodies.find((each) => each.id === id);
  if (body === undefined) {
    throw new Error(`the scene draws no body ${id}`);
  }
  return body;
}

/** A frame with no time passing and no keys held. */
const STILL = { dtS: 0, held: new Set<string>(), reducedMotion: true };

/** The run with its camera made free at `pose`. */
function freeAt(run: ViewRun, pose: CameraPose): ViewRun {
  return { ...run, camera: { ...run.camera, preset: "free", target: null, pose } };
}

function relativeError(a: Vec3, b: Vec3): number {
  return norm(sub(a, b)) / norm(b);
}

describe("the server's scene as the view draws it", () => {
  it("places every body where the SYSTEM display's orbit map puts it at the same time", () => {
    const model = modelOf(statePopulated());
    const time = { seconds: 86_400 * 200, nanos: 250_000_000 };
    const frame = frameOf(model, time);
    const system = model.system;
    if (system === null) {
      throw new Error("the fixture's scene holds a system");
    }
    const layout = layoutBodies(
      system.model.system,
      system.bodies.bodies,
      layoutHierarchy(system.model.hierarchy, system.model.hosts),
    );
    const compared = frame.bodies.flatMap((seen) => {
      const record = system.bodies.bodies.find((body) => body.id === seen.id);
      const orbitMap = record === undefined ? null : bodyPositionM(record, layout, time);
      return seen.kind === "placed" && orbitMap !== null
        ? [relativeError(seen.geometricM, orbitMap)]
        : [];
    });
    expect(compared.length).toBeGreaterThanOrEqual(3);
    expect(Math.max(...compared)).toBeLessThanOrEqual(1e-9);
  });

  it("draws the ship's local body at its geometric position and every other at its apparent one", () => {
    const model = modelOf(stateNearEarth());
    const frame = frameOf(model);
    const scene = sceneOf(model, frame);
    const seen = (id: string) => frame.bodies.find((each) => each.id === id);
    const earth = seen(FIXTURE_EARTH);
    const jupiter = seen(FIXTURE_JUPITER);
    expect(frame.localBody).toBe(FIXTURE_EARTH);
    expect([
      earth?.kind === "placed" ? earth.geometricM : null,
      jupiter?.apparentM,
      jupiter?.kind === "placed" && norm(sub(jupiter.apparentM, jupiter.geometricM)) > 1e3,
    ]).toEqual([
      bodyOf(scene, FIXTURE_EARTH).centreM,
      bodyOf(scene, FIXTURE_JUPITER).centreM,
      true,
    ]);
  });

  it("gives the camera the drawn centres, which its frame selection measures to", () => {
    const model = modelOf(stateNearEarth());
    const frame = frameOf(model);
    const scene = sceneOf(model, frame);
    const seen = (id: string) => frame.bodies.find((each) => each.id === id);
    const earth = seen(FIXTURE_EARTH);
    const { origins } = cameraSceneOf(scene);
    // Jupiter apparent, as drawn; the local body, Earth, geometric, as `sceneAt` chose it.
    expect([origins.bodyCentreM(FIXTURE_JUPITER), origins.bodyCentreM(FIXTURE_EARTH)]).toEqual([
      seen(FIXTURE_JUPITER)?.apparentM,
      earth?.kind === "placed" ? earth.geometricM : null,
    ]);
  });

  it("draws a free camera's own local body, when it is another, at its apparent position", () => {
    const model = modelOf(stateNearEarth());
    const frame = frameOf(model);
    const scene = sceneOf(model, frame);
    const jupiter = bodyOf(scene, FIXTURE_JUPITER);
    const pose: CameraPose = {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: add(jupiter.centreM, vec3(2e8, 0, 0)),
      orientation: IDENTITY_QUATERNION,
    };
    const seen = frame.bodies.find((each) => each.id === FIXTURE_JUPITER);
    expect([sceneFrameFor(pose, cameraSceneOf(scene)), jupiter.centreM]).toEqual([
      { kind: "body", body: FIXTURE_JUPITER },
      seen?.apparentM,
    ]);
  });

  it("puts a camera beside a drawn body in its frame however far it was seen from its geometric centre", () => {
    // Jupiter seen 1.5 Hill radii from where it is: measured to its geometric centre, a camera
    // 2 × 10⁸ m from where it is drawn would be outside its sphere (Design note 6, as amended).
    const model = modelOf(stateNearEarth());
    const frame = frameOf(model);
    const index = frame.bodies.findIndex((each) => each.id === FIXTURE_JUPITER);
    const jupiter = frame.bodies[index];
    if (jupiter?.kind !== "placed" || jupiter.hillRadiusM === null) {
      throw new Error("the fixture's Jupiter is not placed with a Hill radius");
    }
    const apparentM = add(jupiter.geometricM, vec3(0, 1.5 * jupiter.hillRadiusM, 0));
    const shifted: SceneFrame = {
      ...frame,
      bodies: frame.bodies.with(index, { ...jupiter, apparentM }),
    };
    const scene = sceneOf(model, shifted);
    const pose: CameraPose = {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: add(bodyOf(scene, FIXTURE_JUPITER).centreM, vec3(2e8, 0, 0)),
      orientation: IDENTITY_QUATERNION,
    };
    expect(sceneFrameFor(pose, cameraSceneOf(scene))).toEqual({
      kind: "body",
      body: FIXTURE_JUPITER,
    });
  });

  it("never makes a body with no Hill radius the camera's frame", () => {
    // The Earth re-sent at `contact`, placed by the server's seen position, has none.
    const model = modelOf(stateAfterHeartbeat());
    const scene = sceneOf(model, frameOf(model));
    const earth = bodyOf(scene, FIXTURE_EARTH);
    const pose: CameraPose = {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: add(earth.centreM, vec3(1e7, 0, 0)),
      orientation: IDENTITY_QUATERNION,
    };
    expect([
      earth.hillRadiusM,
      cameraSceneOf(scene).frameBodies.some((body) => body.id === FIXTURE_EARTH),
      sceneFrameFor(pose, cameraSceneOf(scene)),
    ]).toEqual([null, false, { kind: "system", system: FIXTURE_SYSTEM }]);
  });

  it("makes the ship stand-in the own ship, at the observer's present position, with the test hull", () => {
    const model = modelOf(stateNearEarth());
    const frame = frameOf(model);
    const scene = sceneOf(model, frame);
    const own = scene.craft.find((craft) => craft.id === scene.ownShip);
    expect([scene.ownShip, own?.hull, own?.pose.position, scene.provenance]).toEqual([
      SERVER_OWN_SHIP,
      TEST_HULL,
      { kind: "system", system: FIXTURE_SYSTEM, m: frame.observer.positionM },
      { kind: "server" },
    ]);
  });

  it("states the scene's tidal radius, time and running rate", () => {
    const model = modelOf(stateNearEarth());
    const scene = sceneOf(model, frameOf(model));
    expect([scene.tidalRadiusM, scene.time, scene.timeRate]).toEqual([
      SCENE_TIDAL_RADIUS_M,
      model.clock.time,
      1_000,
    ]);
  });

  it("names the bodies from the place's designation, the system's ID where none is known", () => {
    const model = modelOf(stateNearEarth());
    const frame = frameOf(model);
    const unknown = sceneOf(model, frame, {
      kind: "unknown",
      system: FIXTURE_SYSTEM,
      designation: FIXTURE_SYSTEM,
    });
    expect([
      bodyOf(sceneOf(model, frame), FIXTURE_EARTH).designation,
      bodyOf(unknown, FIXTURE_EARTH).designation,
      unknown.barycentre,
    ]).toEqual([`${SCENE_DESIGNATION} /768`, `${FIXTURE_SYSTEM} /768`, null]);
  });

  it("puts the barycentre where the scene's place has drifted to at the frame's time", () => {
    const model = modelOf(stateNearEarth(3_000));
    const place = model.system?.place ?? null;
    if (place?.kind !== "stated") {
      throw new Error("the fixture's scene states its place");
    }
    // The place holds at 3,400 s, the frame is at 3,000 s: 400 s before, at the place's velocity.
    const scene = sceneOf(model, frameOf(model), place);
    expect(scene.barycentre).toEqual(
      galacticTranslated(place.barycentre, scale(place.velocityMPerS, -400)),
    );
    expect(scene.barycentre).not.toEqual(place.barycentre);
    // A place known only from the chart has no drift: its barycentre is drawn as it is.
    expect(sceneOf(model, frameOf(model)).barycentre).toEqual(PLACE.barycentre);
  });

  it("draws a planet's ring about it, a moon's orbit about its planet and a planet's about its star", () => {
    const model = modelOf(statePopulated());
    const scene = sceneOf(model, frameOf(model));
    const planet = `${FIXTURE_SYSTEM}.0100`;
    const moon = `${FIXTURE_SYSTEM}.0101`;
    const star = `${FIXTURE_SYSTEM}.0000`;
    expect([
      scene.rings.map((ring) => ring.body),
      scene.orbits.find((orbit) => orbit.body === moon)?.parent,
      scene.orbits.find((orbit) => orbit.body === planet)?.parent,
      bodyOf(scene, moon).kind,
      bodyOf(scene, star).kind,
    ]).toEqual([[planet], planet, star, "moon", "star"]);
  });

  it("gives a dwarf planet of a belt the belt's star as its parent, and leaves the belt out", () => {
    const model = modelOf(statePopulated());
    const scene = sceneOf(model, frameOf(model));
    expect([
      bodyOf(scene, `${FIXTURE_SYSTEM}.e001`).parent,
      scene.bodies.some((body) => body.id === `${FIXTURE_SYSTEM}.e000`),
    ]).toEqual([`${FIXTURE_SYSTEM}.0000`, false]);
  });

  it("keeps a dwarf planet apart from a planet", () => {
    const model = modelOf(statePopulated());
    const scene = sceneOf(model, frameOf(model));
    expect(bodyOf(scene, `${FIXTURE_SYSTEM}.e001`).kind).toBe("dwarf_planet");
  });

  it("cannot be drawn with the ship in no system", () => {
    const model = modelOf(stateInSpace());
    expect([serverSceneGap(model), serverSceneAtPush(model, PLACE)]).toEqual(["no_system", null]);
  });

  it("cannot be drawn when the system's tidal radius was not sent", () => {
    const model = modelOf(stateNearEarth());
    if (model.system === null) {
      throw new Error("the fixture's model holds no system");
    }
    const unsent = { ...model, system: { ...model.system, tidalRadiusM: null } };
    expect([serverSceneGap(model), serverSceneGap(unsent)]).toEqual([null, "no_tidal_radius"]);
  });

  it("is drawn at frameAt(nowMs) each frame", () => {
    const model = modelOf(stateNearEarth());
    const first = sceneOf(model, frameOf(model));
    const asked: number[] = [];
    const times = [1_000, 1_016, 1_033];
    const source = {
      model,
      place: PLACE,
      frameAt: (nowMs: number) => {
        asked.push(nowMs);
        return frameOf(model, { seconds: 3_000 + nowMs, nanos: 0 });
      },
    };
    const run = times.reduce(
      (each, nowMs) =>
        stepRun(each, {
          serverScene: serverSceneAtFrame(source, nowMs),
          dtS: 1 / 60,
          held: new Set(),
          reducedMotion: false,
        }),
      startServerRun(first),
    );
    expect([asked, run.scene.time]).toEqual([times, { seconds: 4_033, nanos: 0 }]);
  });

  it("keeps a free camera through a scene of the same system, and holds the last on null", () => {
    const model = modelOf(stateNearEarth());
    const scene = sceneOf(model, frameOf(model));
    const free = freeAt(startServerRun(scene), {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: vec3(1e11, 0, 0),
      orientation: IDENTITY_QUATERNION,
    });
    const next = sceneOf(model, frameOf(model, { seconds: 3_001, nanos: 0 }));
    const kept = stepRun(free, { ...STILL, serverScene: next });
    const held = stepRun(kept, { ...STILL, serverScene: null });
    expect([kept.camera.preset, kept.scene, held.scene]).toEqual(["free", next, next]);
  });

  it("returns a free camera to chase when the scene is of another system", () => {
    const model = modelOf(stateNearEarth());
    const scene = sceneOf(model, frameOf(model));
    const free = freeAt(startServerRun(scene), {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: vec3(1e11, 0, 0),
      orientation: IDENTITY_QUATERNION,
    });
    const elsewhere: ViewScene = { ...scene, system: "0200080020000001" };
    const moved = stepRun(free, { ...STILL, serverScene: elsewhere });
    expect([moved.camera.preset, moved.camera.pose.frame]).toEqual([
      "chase",
      { kind: "craft", craft: SERVER_OWN_SHIP },
    ]);
  });

  it("returns a free camera to chase when the body it is held to leaves the scene", () => {
    const model = modelOf(stateNearEarth());
    const scene = sceneOf(model, frameOf(model));
    const free = freeAt(startServerRun(scene), {
      frame: { kind: "body", body: FIXTURE_JUPITER },
      positionM: vec3(2e8, 0, 0),
      orientation: IDENTITY_QUATERNION,
    });
    const without: ViewScene = {
      ...scene,
      bodies: scene.bodies.filter((body) => body.id !== FIXTURE_JUPITER),
      orbits: scene.orbits.filter((orbit) => orbit.body !== FIXTURE_JUPITER),
      rings: scene.rings.filter((ring) => ring.body !== FIXTURE_JUPITER),
    };
    expect(stepRun(free, { ...STILL, serverScene: without }).camera.preset).toBe("chase");
  });

  it("holds the last scene while the frame and the model are of different systems", () => {
    const model = modelOf(stateNearEarth());
    const populated = modelOf(statePopulated());
    const frame = frameOf(model);
    const star = frame.stars[0];
    if (star === undefined) {
      throw new Error("the fixture's scene sees a star");
    }
    const foreign: SceneFrame = { ...frame, stars: [{ ...star, id: "ffffffffffffffff.0000" }] };
    expect([
      serverSceneAtFrame({ model: populated, place: PLACE, frameAt: () => frame }, 0) === null,
      serverSceneAtFrame({ model, place: PLACE, frameAt: () => foreign }, 0),
      serverSceneAtFrame({ model, place: PLACE, frameAt: () => null }, 0),
    ]).toEqual([false, null, null]);
  });
});

describe("the view's camera reports", () => {
  it("carry the pose at once on each frame change and at R03's rate otherwise", () => {
    vi.useFakeTimers();
    const model = modelOf(stateNearEarth());
    const scene = sceneOf(model, frameOf(model));
    const sent: CameraReportDto[][] = [];
    const atMs: number[] = [];
    let clockMs = 0;
    const reporter = new CameraReporter();
    reporter.attach((cameras) => {
      sent.push([...cameras]);
      atMs.push(clockMs);
    });
    const view = viewId("view");
    // A second at 60 Hz in the system frame, then a second in the Earth's.
    for (let frame = 0; frame < 120; frame += 1) {
      const pose: CameraPose =
        frame < 60
          ? {
              frame: { kind: "system", system: FIXTURE_SYSTEM },
              positionM: vec3(1.5e11 + frame * 1e3, 0, 0),
              orientation: IDENTITY_QUATERNION,
            }
          : {
              frame: { kind: "body", body: FIXTURE_EARTH },
              positionM: vec3(1e7 + frame * 1e3, 0, 0),
              orientation: IDENTITY_QUATERNION,
            };
      clockMs = frame * 16;
      reporter.report(view, cameraKinematics(pose, scene));
      for (let ms = 1; ms <= 16; ms += 1) {
        clockMs = frame * 16 + ms;
        vi.advanceTimersByTime(1);
      }
    }
    reporter.detach();
    const frames = sent.map((cameras) => cameras[0]?.pose.position.frame);
    const firstInBody = frames.indexOf("body");
    const gaps = atMs.slice(1).map((ms, index) => ms - (atMs[index] ?? 0));
    expect([
      frames[0],
      atMs[firstInBody],
      gaps.every((gapMs, index) => gapMs >= CAMERA_REPORT_INTERVAL_MS || index + 1 === firstInBody),
    ]).toEqual(["system", 60 * 16, true]);
    // Each second of steady frames is reported four times, give or take its ends.
    expect(sent.length).toBeGreaterThanOrEqual(8);
    expect(sent.length).toBeLessThanOrEqual(10);
  });

  it("state a camera held to the own ship in the frame of the ship's position", () => {
    const model = modelOf(stateNearEarth());
    const scene = sceneOf(model, frameOf(model));
    const own = scene.craft.find((craft) => craft.id === scene.ownShip);
    const report = cameraKinematics(
      {
        frame: { kind: "craft", craft: SERVER_OWN_SHIP },
        positionM: vec3(0, 10, 60),
        orientation: IDENTITY_QUATERNION,
      },
      scene,
    );
    const shipM = own?.pose.position.kind === "system" ? own.pose.position.m : vec3(0, 0, 0);
    expect(report.position).toEqual({
      kind: "system",
      system: FIXTURE_SYSTEM,
      offsetM: add(shipM, vec3(0, 10, 60)),
    });
  });
});
