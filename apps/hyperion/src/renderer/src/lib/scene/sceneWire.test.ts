import {
  type KinematicsDto,
  METRES_PER_LIGHT_YEAR,
  type SceneNotificationDto,
  type SceneStateDto,
  type SystemPlaceDto,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { FIXTURE_EARTH, FIXTURE_JUPITER } from "../../test/planetaryFixture";
import {
  craftFixture,
  designateFixture,
  SCENE_TIDAL_RADIUS_M,
  earthReSentAsContact,
  SCENE_DESIGNATION,
  sceneClock,
  scenePlace,
  sceneSequence,
  shipInSpace,
  shipInSystem,
  sliceSceneSystem,
  stateAfterHeartbeat,
  stateAfterLeaving,
  stateInSpace,
} from "../../test/sceneFixture";
import type { SceneModel } from "./model";
import {
  applySceneNotification,
  toKinematicsDto,
  toSceneModel,
  type SceneUpdate,
} from "./sceneWire";

function modelOf(state: SceneStateDto): SceneModel {
  const result = toSceneModel(state, designateFixture);
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return result.model;
}

function applied(update: SceneUpdate): SceneModel {
  if (update.kind !== "ok") {
    throw new Error(`the notification was not applied: ${JSON.stringify(update)}`);
  }
  return update.model;
}

/** The model after each of `notifications` in turn, from `state`. */
function applyAll(
  state: SceneStateDto,
  notifications: ReadonlyArray<SceneNotificationDto>,
): SceneModel {
  return notifications.reduce(
    (model, notification) => applied(applySceneNotification(model, notification, designateFixture)),
    modelOf(state),
  );
}

/** Names every system wrongly, so that a label taken from it shows it was used. */
function designateWrongly(system: string): string {
  return `CHART ${system}`;
}

/** Every star's and body's designation in the model's system, which all extend the system's. */
function labels(model: SceneModel): string[] {
  const system = model.system;
  return system === null
    ? []
    : [...system.model.hosts, ...system.bodies.bodies].map((each) => each.designation);
}

/** Whether every label extends `designation`, and there is at least one. */
function labelledBy(model: SceneModel, designation: string): boolean {
  const all = labels(model);
  return all.length > 0 && all.every((label) => label.startsWith(`${designation} /`));
}

/** The slice's system in a state, with `place` in place of the fixture's (absent for `null`). */
function stateWithPlace(place: SystemPlaceDto | null): SceneStateDto {
  const { place: _place, ...system } = sliceSceneSystem();
  return {
    sequence: 0,
    clock: sceneClock(3_400),
    ship: shipInSystem(3_400),
    system: place === null ? system : { ...system, place },
    tidal_radius_m: SCENE_TIDAL_RADIUS_M,
    craft: [],
  };
}

describe("the system's place (R03.T16)", () => {
  it("is read from a state, and the bodies take its designation, not designate's", () => {
    const result = toSceneModel(stateWithPlace(scenePlace()), designateWrongly);
    if (result.kind !== "ok") {
      throw new Error(result.fault);
    }
    const system = result.model.system;
    expect(system?.place).toEqual({
      system: system?.model.system,
      designation: SCENE_DESIGNATION,
      barycentre: scenePlace().barycentre,
      velocityMPerS: { x: -11_100, y: 232_240.5, z: 7_250 },
      time: { seconds: 3_400, nanos: 0 },
    });
    expect(labelledBy(result.model, SCENE_DESIGNATION)).toBe(true);
  });

  it("is read from an arrival, and kept across a re-sent body", () => {
    const [arrival, resent] = sceneSequence();
    if (arrival === undefined || resent === undefined) {
      throw new Error("the sequence has an arrival and a re-sent body");
    }
    const first = applied(
      applySceneNotification(modelOf(stateInSpace()), arrival, designateWrongly),
    );
    const second = applied(applySceneNotification(first, resent, designateWrongly));
    for (const model of [first, second]) {
      expect(model.system?.place?.designation).toBe(SCENE_DESIGNATION);
      expect(labelledBy(model, SCENE_DESIGNATION)).toBe(true);
    }
  });

  it("falls back to designate, with no barycentre, where the scene states none", () => {
    const result = toSceneModel(stateWithPlace(null), designateWrongly);
    if (result.kind !== "ok") {
      throw new Error(result.fault);
    }
    const system = result.model.system;
    expect(system?.place).toBeNull();
    expect(labelledBy(result.model, designateWrongly(system?.model.system ?? ""))).toBe(true);
  });

  it("refuses a barycentre offset out of [0, 1 ly), a velocity not finite and a malformed time", () => {
    const place = scenePlace();
    const unusable: SystemPlaceDto[] = [
      { ...place, barycentre: { ...place.barycentre, offset_m: [METRES_PER_LIGHT_YEAR, 0, 0] } },
      { ...place, barycentre: { ...place.barycentre, offset_m: [-1, 0, 0] } },
      { ...place, velocity_m_s: [0, Number.NaN, 0] },
      { ...place, velocity_m_s: [Number.POSITIVE_INFINITY, 0, 0] },
      { ...place, time: { seconds: 3_400, nanos: 1_000_000_000 } },
      { ...place, designation: "" },
    ];
    const faults = unusable.map(
      (each) => toSceneModel(stateWithPlace(each), designateFixture).kind,
    );
    expect(faults).toEqual(["fault", "fault", "fault", "fault", "fault", "fault"]);
  });
});

describe("toSceneModel", () => {
  it("reads a state in a system, every body with its grant", () => {
    const model = modelOf({
      ...stateInSpace(),
      ship: shipInSystem(3_000),
      system: sliceSceneSystem(),
      tidal_radius_m: SCENE_TIDAL_RADIUS_M,
    });

    expect(model.system?.bodies.bodies.map((body) => body.id)).toEqual([
      FIXTURE_EARTH,
      FIXTURE_JUPITER,
    ]);
    expect(model.system?.grants.get(FIXTURE_EARTH)).toEqual({ level: "full", seen: null });
    expect(model.system?.model.hosts).toHaveLength(1);
    expect(model.system?.tidalRadiusM).toBe(SCENE_TIDAL_RADIUS_M);
    expect(model.clock).toEqual({
      time: { seconds: 3_000, nanos: 0 },
      rate: 1_000,
      state: "running",
    });
    expect(model.ship.position).toEqual({
      kind: "system",
      system: sliceSceneSystem().system.hosts.system,
      offsetM: { x: 1.5e11, y: 2e9, z: 0 },
    });
  });

  it("reads a state in a system that omits the tidal radius as having none", () => {
    const model = modelOf({
      ...stateInSpace(),
      ship: shipInSystem(3_000),
      system: sliceSceneSystem(),
    });

    expect(model.system?.tidalRadiusM).toBeNull();
  });

  it("refuses a tidal radius that is not a positive length", () => {
    const result = toSceneModel(
      {
        ...stateInSpace(),
        ship: shipInSystem(3_000),
        system: sliceSceneSystem(),
        tidal_radius_m: 0,
      },
      designateFixture,
    );

    expect(result).toEqual({ kind: "fault", fault: "tidal radius unusable" });
  });

  it("reads a state in the galactic frame as having no system", () => {
    const model = modelOf(stateInSpace());

    expect(model.system).toBeNull();
    expect(model.ship.position.kind).toBe("galactic");
  });

  it("refuses grants that do not match the bodies", () => {
    const system = sliceSceneSystem();
    const result = toSceneModel(
      { ...stateInSpace(), system: { ...system, grants: system.grants.slice(1) } },
      designateFixture,
    );

    expect(result).toEqual({ kind: "fault", fault: "grants do not match the bodies" });
  });

  it("reads a craft's attitude from the wire's x, y, z, w", () => {
    const model = modelOf({ ...stateInSpace(), craft: [craftFixture()] });

    expect(model.craft[0]?.attitude).toEqual({ w: 0.5, x: 0.1, y: 0.2, z: 0.3 });
    expect(model.craft[0]?.plannedPath).toBeNull();
  });
});

describe("toKinematicsDto", () => {
  it("writes a pose in each frame back as the wire stated it", () => {
    const inBody: KinematicsDto = {
      position: { frame: "body", body: FIXTURE_EARTH, offset_m: [7e6, -2.5, 0.125] },
      velocity_m_s: [1, 2, 3],
      time: { seconds: 3_000, nanos: 17 },
    };

    for (const ship of [shipInSpace(3_000), shipInSystem(3_000), inBody]) {
      expect(toKinematicsDto(modelOf({ ...stateInSpace(), ship }).ship)).toEqual(ship);
    }
  });
});

describe("applySceneNotification", () => {
  it("equals the state built directly for its end, after every notification", () => {
    const notifications = sceneSequence();

    const afterHeartbeat = applyAll(stateInSpace(), notifications.slice(0, 3));
    const afterLeaving = applyAll(stateInSpace(), notifications);

    expect(afterHeartbeat).toEqual(modelOf(stateAfterHeartbeat()));
    expect(afterLeaving).toEqual(modelOf(stateAfterLeaving()));
  });

  it("keeps the arrival's tidal radius until the scene leaves the system", () => {
    const notifications = sceneSequence();

    expect(applyAll(stateInSpace(), notifications.slice(0, 3)).system?.tidalRadiusM).toBe(
      SCENE_TIDAL_RADIUS_M,
    );
    expect(applyAll(stateInSpace(), notifications).system).toBeNull();
  });

  it("keeps the opening state's tidal radius through pushes, until an arrival states another", () => {
    const [, earthReSent, heartbeat] = sceneSequence();
    if (earthReSent === undefined || heartbeat === undefined) {
      throw new Error("the fixture sequence re-sends a body and beats");
    }
    const inSystem: SceneStateDto = {
      ...stateInSpace(),
      sequence: 1,
      ship: shipInSystem(3_000),
      system: sliceSceneSystem(),
      tidal_radius_m: 3e15,
    };

    const kept = applyAll(inSystem, [earthReSent, heartbeat]);
    const arrived = applied(
      applySceneNotification(
        kept,
        {
          sequence: 4,
          clock: sceneClock(4_700),
          arrival: { type: "system", system: sliceSceneSystem(), tidal_radius_m: 4e15 },
          bodies: [],
        },
        designateFixture,
      ),
    );

    expect(kept.system?.tidalRadiusM).toBe(3e15);
    expect(kept.wire.tidal_radius_m).toBe(3e15);
    expect(arrived.system?.tidalRadiusM).toBe(4e15);
  });

  it("re-sends a body at its new level with its seen position", () => {
    const model = applyAll(stateInSpace(), sceneSequence().slice(0, 2));

    expect(model.system?.grants.get(FIXTURE_EARTH)).toEqual({
      level: "contact",
      seen: {
        apparentM: { x: 1.49e11, y: 1.2e10, z: -3e6 },
        emitted: { seconds: 3_599, nanos: 500_000_000 },
      },
    });
    expect(model.system?.grants.get(FIXTURE_JUPITER)).toEqual({ level: "full", seen: null });
    const earth = model.system?.bodies.bodies.find((body) => body.id === FIXTURE_EARTH);
    expect(earth?.orbit.state).toBe("not_resolved");
  });

  it("lets an arrival replace every body before it", () => {
    const [arrival, resent] = sceneSequence();
    if (arrival === undefined || resent === undefined) {
      throw new Error("the sequence has an arrival and a re-sent body");
    }
    const model = applyAll(stateInSpace(), [arrival, resent, { ...arrival, sequence: 3 }]);

    expect(model.system?.grants.get(FIXTURE_EARTH)).toEqual({ level: "full", seen: null });
  });

  it("inserts a re-sent body absent from the list in index order, with its grant", () => {
    const full = sliceSceneSystem();
    const withoutEarth = {
      ...full,
      system: {
        ...full.system,
        bodies: full.system.bodies.filter((body) => body.id !== FIXTURE_EARTH),
      },
      grants: full.grants.filter((grant) => grant.body !== FIXTURE_EARTH),
    };
    const earth = full.system.bodies.find((body) => body.id === FIXTURE_EARTH);
    if (earth === undefined) {
      throw new Error("the slice lists its Earth");
    }
    const model = applyAll(stateInSpace(), [
      {
        sequence: 1,
        clock: sceneClock(3_400),
        ship: shipInSystem(3_400),
        arrival: { type: "system", system: withoutEarth, tidal_radius_m: SCENE_TIDAL_RADIUS_M },
        bodies: [],
      },
      { sequence: 2, clock: sceneClock(3_500), bodies: [{ level: "full", record: earth }] },
    ]);

    expect(model).toEqual(
      modelOf({
        sequence: 2,
        clock: sceneClock(3_500),
        ship: shipInSystem(3_400),
        system: full,
        tidal_radius_m: SCENE_TIDAL_RADIUS_M,
        craft: [],
      }),
    );
  });

  it("merges bodies re-sent with an arrival into the arriving system", () => {
    const [arrival] = sceneSequence();
    if (arrival === undefined) {
      throw new Error("the sequence has an arrival");
    }
    const model = applyAll(stateInSpace(), [{ ...arrival, bodies: [earthReSentAsContact()] }]);

    expect(model.system?.grants.get(FIXTURE_EARTH)?.level).toBe("contact");
  });

  it("refuses a clock rate that is not 0 or a power of ten to 100,000", () => {
    const result = toSceneModel(
      { ...stateInSpace(), clock: { ...sceneClock(3_000), time_rate: 50 } },
      designateFixture,
    );

    expect(result).toEqual({ kind: "fault", fault: "clock rate unusable" });
  });

  it("refuses a galactic offset of a whole light-year", () => {
    const state = stateInSpace();
    const result = toSceneModel(
      {
        ...state,
        ship: {
          ...state.ship,
          position: {
            frame: "galactic",
            position: { cell_ly: [0, 0, 0], offset_m: [METRES_PER_LIGHT_YEAR, 0, 0] },
          },
        },
      },
      designateFixture,
    );

    expect(result).toEqual({ kind: "fault", fault: "galactic position unusable" });
  });

  it("refuses a gap in the sequence and leaves the model as it was", () => {
    const model = modelOf(stateInSpace());

    const update = applySceneNotification(
      model,
      { sequence: 2, clock: sceneClock(3_100), bodies: [] },
      designateFixture,
    );

    expect(update).toEqual({ kind: "sequence", expected: 1, received: 2 });
  });

  it("refuses a step back in the sequence", () => {
    const model = applyAll(stateInSpace(), sceneSequence().slice(0, 2));

    const update = applySceneNotification(
      model,
      { sequence: 1, clock: sceneClock(3_700), bodies: [] },
      designateFixture,
    );

    expect(update).toEqual({ kind: "sequence", expected: 3, received: 1 });
  });

  it("refuses a body re-sent while the scene has no system", () => {
    const update = applySceneNotification(
      modelOf(stateInSpace()),
      { sequence: 1, clock: sceneClock(3_100), bodies: [earthReSentAsContact()] },
      designateFixture,
    );

    expect(update).toEqual({ kind: "fault", fault: "bodies re-sent outside a system" });
  });

  it("replaces the craft when they are sent and keeps them when not", () => {
    const withCraft = applied(
      applySceneNotification(
        modelOf(stateInSpace()),
        { sequence: 1, clock: sceneClock(3_001), bodies: [], craft: [craftFixture()] },
        designateFixture,
      ),
    );
    const heartbeat = applied(
      applySceneNotification(
        withCraft,
        { sequence: 2, clock: sceneClock(3_002), bodies: [] },
        designateFixture,
      ),
    );

    expect(heartbeat.craft.map((craft) => craft.craft)).toEqual(["ISV-1"]);
    expect(heartbeat.clock.time.seconds).toBe(3_002);
  });
});
