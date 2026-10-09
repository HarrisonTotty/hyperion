import { type BodyGrantDto, type CameraReportDto, type SceneStateDto } from "@hyperion/protocol";
import { afterEach, describe, expect, it, vi } from "vitest";

import { stepRun, startServerRun, type ViewRun } from "../../displays/view/viewRun";
import { bodyPositionM, layoutBodies } from "../../displays/system/bodyMap";
import { add, dot, norm, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import type { SceneFrame } from "../../lib/scene/apparent";
import { CAMERA_REPORT_INTERVAL_MS, CameraReporter } from "../../lib/scene/cameraReports";
import { spanSeconds } from "../../lib/scene/lightTime";
import { layoutHierarchy } from "../../lib/system/hierarchy";
import {
  FIXTURE_EARTH,
  FIXTURE_JUPITER,
  FIXTURE_SYSTEM,
  populatedBodies,
} from "../../test/planetaryFixture";
import {
  SCENE_CHARTED_PLACE,
  SCENE_DESIGNATION,
  SCENE_TIDAL_RADIUS_M,
  sceneClock,
  sceneModelOf,
  shipFrameOf,
  shipInSystem,
  sliceSceneSystem,
  stateAfterHeartbeat,
  stateInSpace,
  viewBodyOf,
  viewSceneOf,
} from "../../test/sceneFixture";
import type { CameraPose } from "../camera/pose";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { sceneFrameFor, viewId } from "../camera/state";
import { galacticTranslated } from "../coords/position";
import { rotateToBody } from "../coords/rotation";
import { bodyFixedAxesAt } from "../../lib/system/rotation";
import type { SystemBody } from "../../lib/system/model";
import { heldAppearanceOf, PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import { TEST_HULL } from "./hull";
import {
  cameraKinematics,
  reportBondRatioFindings,
  SERVER_OWN_SHIP,
  serverSceneAtFrame,
  serverSceneAtPush,
  serverSceneGap,
} from "./fromServer";
import { cameraSceneOf, type ViewScene } from "./model";

afterEach(() => {
  vi.useRealTimers();
});

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
    const model = sceneModelOf(statePopulated());
    const time = { seconds: 86_400 * 200, nanos: 250_000_000 };
    const frame = shipFrameOf(model, time);
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
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    const scene = viewSceneOf(model, frame);
    const seen = (id: string) => frame.bodies.find((each) => each.id === id);
    const earth = seen(FIXTURE_EARTH);
    const jupiter = seen(FIXTURE_JUPITER);
    expect(frame.localBody).toBe(FIXTURE_EARTH);
    expect([
      earth?.kind === "placed" ? earth.geometricM : null,
      jupiter?.apparentM,
      jupiter?.kind === "placed" && norm(sub(jupiter.apparentM, jupiter.geometricM)) > 1e3,
    ]).toEqual([
      viewBodyOf(scene, FIXTURE_EARTH).centreM,
      viewBodyOf(scene, FIXTURE_JUPITER).centreM,
      true,
    ]);
  });

  it("lights the ship's local body from where it was when the light seen left it, though drawn at the present", () => {
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    const earth = frame.bodies.find((each) => each.id === FIXTURE_EARTH);
    if (earth?.kind !== "placed") {
      throw new Error("the fixture's frame places its Earth");
    }
    expect(frame.localBody).toBe(FIXTURE_EARTH);
    expect(viewBodyOf(viewSceneOf(model, frame), FIXTURE_EARTH).retarded).toEqual({
      centreM: earth.emittedM,
      velocityMPerS: earth.emittedVelocityMPerS,
      lightTimeS: spanSeconds(earth.lightTime),
    });
  });

  it("lights every other body from where it was when the light seen left it, not its apparent place", () => {
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    const jupiter = frame.bodies.find((each) => each.id === FIXTURE_JUPITER);
    if (jupiter?.kind !== "placed") {
      throw new Error("the fixture's frame places its Jupiter");
    }
    expect(viewBodyOf(viewSceneOf(model, frame), FIXTURE_JUPITER).retarded).toEqual({
      centreM: jupiter.emittedM,
      velocityMPerS: jupiter.emittedVelocityMPerS,
      lightTimeS: spanSeconds(jupiter.lightTime),
    });
    expect(norm(sub(jupiter.emittedM, jupiter.apparentM))).toBeGreaterThan(1e3);
  });

  it("lights a star from where it was when the light seen left it", () => {
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    const [star] = frame.stars;
    if (star === undefined) {
      throw new Error("the fixture's frame sees its star");
    }
    expect(viewBodyOf(viewSceneOf(model, frame), star.id).retarded).toEqual({
      centreM: star.emittedM,
      velocityMPerS: star.emittedVelocityMPerS,
      lightTimeS: spanSeconds(star.lightTime),
    });
  });

  it("gives a contact, placed by the server's seen position, no retarded centre", () => {
    const model = sceneModelOf(stateAfterHeartbeat());
    const scene = viewSceneOf(model, shipFrameOf(model));
    expect(viewBodyOf(scene, FIXTURE_EARTH).retarded).toBeNull();
    expect(viewBodyOf(scene, FIXTURE_JUPITER).retarded).not.toBeNull();
  });

  it("gives the camera the drawn centres, which its frame selection measures to", () => {
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    const scene = viewSceneOf(model, frame);
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
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    const scene = viewSceneOf(model, frame);
    const jupiter = viewBodyOf(scene, FIXTURE_JUPITER);
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
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
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
    const scene = viewSceneOf(model, shifted);
    const pose: CameraPose = {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: add(viewBodyOf(scene, FIXTURE_JUPITER).centreM, vec3(2e8, 0, 0)),
      orientation: IDENTITY_QUATERNION,
    };
    expect(sceneFrameFor(pose, cameraSceneOf(scene))).toEqual({
      kind: "body",
      body: FIXTURE_JUPITER,
    });
  });

  it("never makes a body with no Hill radius the camera's frame", () => {
    // The Earth re-sent at `contact`, placed by the server's seen position, has none.
    const model = sceneModelOf(stateAfterHeartbeat());
    const scene = viewSceneOf(model, shipFrameOf(model));
    const earth = viewBodyOf(scene, FIXTURE_EARTH);
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
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    const scene = viewSceneOf(model, frame);
    const own = scene.craft.find((craft) => craft.id === scene.ownShip);
    expect([scene.ownShip, own?.hull, own?.pose.position, scene.provenance]).toEqual([
      SERVER_OWN_SHIP,
      TEST_HULL,
      { kind: "system", system: FIXTURE_SYSTEM, m: frame.observer.positionM },
      { kind: "server" },
    ]);
  });

  it("states the scene's tidal radius, time and running rate", () => {
    const model = sceneModelOf(stateNearEarth());
    const scene = viewSceneOf(model, shipFrameOf(model));
    expect([scene.tidalRadiusM, scene.time, scene.timeRate]).toEqual([
      SCENE_TIDAL_RADIUS_M,
      model.clock.time,
      1_000,
    ]);
  });

  it("names the bodies from the place's designation, the system's ID where none is known", () => {
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    const unknown = viewSceneOf(model, frame, {
      kind: "unknown",
      system: FIXTURE_SYSTEM,
      designation: FIXTURE_SYSTEM,
    });
    expect([
      viewBodyOf(viewSceneOf(model, frame), FIXTURE_EARTH).designation,
      viewBodyOf(unknown, FIXTURE_EARTH).designation,
      unknown.barycentre,
    ]).toEqual([`${SCENE_DESIGNATION} /768`, `${FIXTURE_SYSTEM} /768`, null]);
  });

  it("puts the barycentre where the scene's place has drifted to at the frame's time", () => {
    const model = sceneModelOf(stateNearEarth(3_000));
    const place = model.system?.place ?? null;
    if (place?.kind !== "stated") {
      throw new Error("the fixture's scene states its place");
    }
    // The place holds at 3,400 s, the frame is at 3,000 s: 400 s before, at the place's velocity.
    const scene = viewSceneOf(model, shipFrameOf(model), place);
    expect(scene.barycentre).toEqual(
      galacticTranslated(place.barycentre, scale(place.velocityMPerS, -400)),
    );
    expect(scene.barycentre).not.toEqual(place.barycentre);
    // A place known only from the chart has no drift: its barycentre is drawn as it is.
    expect(viewSceneOf(model, shipFrameOf(model)).barycentre).toEqual(
      SCENE_CHARTED_PLACE.barycentre,
    );
  });

  it("draws a planet's ring about it, a moon's orbit about its planet and a planet's about its star", () => {
    const model = sceneModelOf(statePopulated());
    const scene = viewSceneOf(model, shipFrameOf(model));
    const planet = `${FIXTURE_SYSTEM}.0100`;
    const moon = `${FIXTURE_SYSTEM}.0101`;
    const star = `${FIXTURE_SYSTEM}.0000`;
    expect([
      scene.rings.map((ring) => ring.body),
      scene.orbits.find((orbit) => orbit.body === moon)?.parent,
      scene.orbits.find((orbit) => orbit.body === planet)?.parent,
      viewBodyOf(scene, moon).kind,
      viewBodyOf(scene, star).kind,
    ]).toEqual([[planet], planet, star, "moon", "star"]);
  });

  it("gives a dwarf planet of a belt the belt's star as its parent, and leaves the belt out", () => {
    const model = sceneModelOf(statePopulated());
    const scene = viewSceneOf(model, shipFrameOf(model));
    expect([
      viewBodyOf(scene, `${FIXTURE_SYSTEM}.e001`).parent,
      scene.bodies.some((body) => body.id === `${FIXTURE_SYSTEM}.e000`),
    ]).toEqual([`${FIXTURE_SYSTEM}.0000`, false]);
  });

  it("keeps a dwarf planet apart from a planet", () => {
    const model = sceneModelOf(statePopulated());
    const scene = viewSceneOf(model, shipFrameOf(model));
    expect(viewBodyOf(scene, `${FIXTURE_SYSTEM}.e001`).kind).toBe("dwarf_planet");
  });

  it("cannot be drawn with the ship in no system", () => {
    const model = sceneModelOf(stateInSpace());
    expect([serverSceneGap(model), serverSceneAtPush(model, SCENE_CHARTED_PLACE)]).toEqual([
      "no_system",
      null,
    ]);
  });

  it("cannot be drawn when the system's tidal radius was not sent", () => {
    const model = sceneModelOf(stateNearEarth());
    if (model.system === null) {
      throw new Error("the fixture's model holds no system");
    }
    const unsent = { ...model, system: { ...model.system, tidalRadiusM: null } };
    expect([serverSceneGap(model), serverSceneGap(unsent)]).toEqual([null, "no_tidal_radius"]);
  });

  it("is drawn at frameAt(nowMs) each frame", () => {
    const model = sceneModelOf(stateNearEarth());
    const first = viewSceneOf(model, shipFrameOf(model));
    const asked: number[] = [];
    const times = [1_000, 1_016, 1_033];
    const source = {
      model,
      place: SCENE_CHARTED_PLACE,
      frameAt: (nowMs: number) => {
        asked.push(nowMs);
        return shipFrameOf(model, { seconds: 3_000 + nowMs, nanos: 0 });
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
    const model = sceneModelOf(stateNearEarth());
    const scene = viewSceneOf(model, shipFrameOf(model));
    const free = freeAt(startServerRun(scene), {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: vec3(1e11, 0, 0),
      orientation: IDENTITY_QUATERNION,
    });
    const next = viewSceneOf(model, shipFrameOf(model, { seconds: 3_001, nanos: 0 }));
    const kept = stepRun(free, { ...STILL, serverScene: next });
    const held = stepRun(kept, { ...STILL, serverScene: null });
    expect([kept.camera.preset, kept.scene, held.scene]).toEqual(["free", next, next]);
  });

  it("returns a free camera to chase when the scene is of another system", () => {
    const model = sceneModelOf(stateNearEarth());
    const scene = viewSceneOf(model, shipFrameOf(model));
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
    const model = sceneModelOf(stateNearEarth());
    const scene = viewSceneOf(model, shipFrameOf(model));
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
    const model = sceneModelOf(stateNearEarth());
    const populated = sceneModelOf(statePopulated());
    const frame = shipFrameOf(model);
    const star = frame.stars[0];
    if (star === undefined) {
      throw new Error("the fixture's scene sees a star");
    }
    const foreign: SceneFrame = { ...frame, stars: [{ ...star, id: "ffffffffffffffff.0000" }] };
    expect([
      serverSceneAtFrame(
        { model: populated, place: SCENE_CHARTED_PLACE, frameAt: () => frame },
        0,
      ) === null,
      serverSceneAtFrame({ model, place: SCENE_CHARTED_PLACE, frameAt: () => foreign }, 0),
      serverSceneAtFrame({ model, place: SCENE_CHARTED_PLACE, frameAt: () => null }, 0),
    ]).toEqual([false, null, null]);
  });
});

/** The ship 1 au out in the slice's system, where its Earth is not the local body. */
function stateInSystem(seconds = 3_000): SceneStateDto {
  return { ...stateNearEarth(seconds), ship: shipInSystem(seconds) };
}

/** A record of the scene's system, by ID. */
function recordOf(model: ReturnType<typeof sceneModelOf>, id: string): SystemBody {
  const record = model.system?.bodies.bodies.find((body) => body.id === id);
  if (record === undefined) {
    throw new Error(`the fixture's scene holds no record ${id}`);
  }
  return record;
}

/** The record's body-fixed axes at `time`, from its rotation section. */
function axesAt(record: SystemBody, time: Parameters<typeof bodyFixedAxesAt>[1]) {
  if (record.rotation.state !== "ok") {
    throw new Error(`the fixture's ${record.id} has a rotation section`);
  }
  return bodyFixedAxesAt(record.rotation.value, time);
}

/** The body-fixed axes a view body's rotation holds, each in the body frame, or `null`. */
function heldAxes(scene: ViewScene, id: string) {
  const rotation = viewBodyOf(scene, id).rotation;
  return rotation === null
    ? null
    : {
        meridian: rotateToBody(rotation, vec3(1, 0, 0)),
        east: rotateToBody(rotation, vec3(0, 1, 0)),
        pole: rotateToBody(rotation, vec3(0, 0, 1)),
      };
}

describe("a server body's rotation and shading from plan 14's sections (R07.T2.b)", () => {
  it("turns the ship's local body by its rotation law at the frame's time, drawn at the present", () => {
    const model = sceneModelOf(stateNearEarth());
    const frame = shipFrameOf(model);
    expect(frame.localBody).toBe(FIXTURE_EARTH);
    expect(heldAxes(viewSceneOf(model, frame), FIXTURE_EARTH)).toEqual(
      axesAt(recordOf(model, FIXTURE_EARTH), frame.time),
    );
  });

  it("turns every other body by its law when the light seen left it, where it is drawn", () => {
    const model = sceneModelOf(stateInSystem());
    const frame = shipFrameOf(model);
    const earth = frame.bodies.find((each) => each.id === FIXTURE_EARTH);
    if (earth?.kind !== "placed") {
      throw new Error("the fixture's frame places its Earth");
    }
    const record = recordOf(model, FIXTURE_EARTH);
    const held = heldAxes(viewSceneOf(model, frame), FIXTURE_EARTH);
    expect(frame.localBody).not.toBe(FIXTURE_EARTH);
    expect(held).toEqual(axesAt(record, earth.emitted));
  });

  it("turns it the light time's spin short of the present", () => {
    const model = sceneModelOf(stateInSystem());
    const frame = shipFrameOf(model);
    const earth = frame.bodies.find((each) => each.id === FIXTURE_EARTH);
    const record = recordOf(model, FIXTURE_EARTH);
    if (earth?.kind !== "placed" || record.rotation.state !== "ok") {
      throw new Error("the fixture's frame places its Earth, which turns");
    }
    const held = heldAxes(viewSceneOf(model, frame), FIXTURE_EARTH)?.meridian ?? vec3(0, 0, 0);
    const turnedRad = Math.acos(dot(held, axesAt(record, frame.time).meridian));
    const spunRad = record.rotation.value.initialRateRadPerS * spanSeconds(earth.lightTime);
    expect(spunRad).toBeGreaterThan(0.01);
    expect(turnedRad).toBeCloseTo(spunRad, 9);
  });

  it("holds the pole along the body-fixed z axis", () => {
    const model = sceneModelOf(stateNearEarth());
    const pole = heldAxes(viewSceneOf(model, shipFrameOf(model)), FIXTURE_EARTH)?.pole;
    const record = recordOf(model, FIXTURE_EARTH);
    expect(pole).toEqual(record.rotation.state === "ok" ? record.rotation.value.pole : null);
  });

  it("gives a body without a rotation section, and a star, no rotation", () => {
    const model = sceneModelOf(stateNearEarth());
    const scene = viewSceneOf(model, shipFrameOf(model));
    const star = scene.bodies.find((body) => body.kind === "star");
    expect([viewBodyOf(scene, FIXTURE_JUPITER).rotation, star?.rotation]).toEqual([null, null]);
  });

  it("shades a body with its sections' appearance, one object a record across frames", () => {
    const model = sceneModelOf(stateNearEarth());
    const first = viewSceneOf(model, shipFrameOf(model));
    const later = viewSceneOf(model, shipFrameOf(model, { seconds: 3_100, nanos: 0 }));
    const appearance = viewBodyOf(first, FIXTURE_EARTH).appearance;
    expect(appearance).toBe(heldAppearanceOf(recordOf(model, FIXTURE_EARTH)));
    expect(viewBodyOf(later, FIXTURE_EARTH).appearance).toBe(appearance);
    expect([appearance?.photometry.provenance, appearance?.labels]).toEqual(["modelled", []]);
  });

  it("draws a body with a figure at its equatorial radius, one without at its mean radius", () => {
    const model = sceneModelOf(stateNearEarth());
    const scene = viewSceneOf(model, shipFrameOf(model));
    const jupiter = recordOf(model, FIXTURE_JUPITER);
    expect([
      viewBodyOf(scene, FIXTURE_EARTH).radiusM,
      viewBodyOf(scene, FIXTURE_JUPITER).radiusM,
    ]).toEqual([6_378_137, jupiter.bulk.state === "ok" ? jupiter.bulk.value.radiusM : Number.NaN]);
  });

  it("shades a body without a photometric section with the provisional photometry, labelled", () => {
    const model = sceneModelOf(stateNearEarth());
    const appearance = viewBodyOf(
      viewSceneOf(model, shipFrameOf(model)),
      FIXTURE_JUPITER,
    ).appearance;
    expect([appearance?.photometry, appearance?.labels]).toEqual([
      PROVISIONAL_PHOTOMETRY,
      ["BODY PHOTOMETRY: NOT YET MODELLED"],
    ]);
  });

  it("reports a body whose stated ratio departs from its law's, once across frames", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const state = stateNearEarth();
    const system = state.system;
    if (system === null) {
      throw new Error("the fixture's state holds a system");
    }
    const earth = system.system.bodies.find((body) => body.id === FIXTURE_EARTH);
    if (earth?.photometry?.state !== "ok") {
      throw new Error("the fixture's Earth has a photometric section");
    }
    // A ratio three times the Earth's, which no law of its section reaches.
    const misstated = {
      ...earth,
      photometry: { state: "ok" as const, value: { ...earth.photometry.value, bond_ratio: 3 } },
    };
    const bodies = system.system.bodies.map((body) =>
      body.id === FIXTURE_EARTH ? misstated : body,
    );
    const model = sceneModelOf({
      ...state,
      system: { ...system, system: { ...system.system, bodies } },
    });

    reportBondRatioFindings(viewSceneOf(model, shipFrameOf(model)));
    reportBondRatioFindings(viewSceneOf(model, shipFrameOf(model, { seconds: 3_100, nanos: 0 })));

    expect(warn).toHaveBeenCalledOnce();
    expect(warn.mock.calls[0]?.[0]).toMatch(/^body H7K 4C0RFZ D-7 \/768: .*plan 14's owner$/);
  });

  it("reports nothing for bodies whose ratios agree with their laws", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const model = sceneModelOf(stateNearEarth());

    reportBondRatioFindings(viewSceneOf(model, shipFrameOf(model)));

    expect(warn).not.toHaveBeenCalled();
  });

  it("gives a star no appearance", () => {
    const model = sceneModelOf(stateNearEarth());
    const star = viewSceneOf(model, shipFrameOf(model)).bodies.find((body) => body.kind === "star");
    expect(star?.appearance).toBeNull();
  });
});

describe("the view's camera reports", () => {
  it("carry the pose at once on each frame change and at R03's rate otherwise", () => {
    vi.useFakeTimers();
    const model = sceneModelOf(stateNearEarth());
    const scene = viewSceneOf(model, shipFrameOf(model));
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
    const model = sceneModelOf(stateNearEarth());
    const scene = viewSceneOf(model, shipFrameOf(model));
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

  it("state a camera held to a craft in a turning body's fixed frame in that body's frame", () => {
    const model = sceneModelOf(stateNearEarth());
    const drawn = viewSceneOf(model, shipFrameOf(model));
    const rotation = viewBodyOf(drawn, FIXTURE_EARTH).rotation;
    if (rotation === null) {
      throw new Error("the fixture's Earth turns");
    }
    const fixedM = vec3(1e7, 0, 0);
    const own = drawn.craft.find((craft) => craft.id === SERVER_OWN_SHIP);
    if (own === undefined) {
      throw new Error("the fixture's scene has its own ship");
    }
    const scene: ViewScene = {
      ...drawn,
      craft: [
        {
          ...own,
          pose: { ...own.pose, position: { kind: "body_fixed", body: FIXTURE_EARTH, m: fixedM } },
        },
        ...drawn.craft.filter((craft) => craft.id !== SERVER_OWN_SHIP),
      ],
    };
    const report = cameraKinematics(
      {
        frame: { kind: "craft", craft: SERVER_OWN_SHIP },
        positionM: vec3(0, 10, 60),
        orientation: IDENTITY_QUATERNION,
      },
      scene,
    );
    expect(report.position).toEqual({
      kind: "body",
      body: FIXTURE_EARTH,
      offsetM: add(rotateToBody(rotation, fixedM), vec3(0, 10, 60)),
    });
  });
});
