import {
  METRES_PER_LIGHT_YEAR,
  type SceneNotificationDto,
  type SceneStateDto,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { FIXTURE_EARTH, FIXTURE_JUPITER } from "../../test/planetaryFixture";
import {
  craftFixture,
  designateFixture,
  earthReSentAsContact,
  sceneClock,
  sceneSequence,
  shipInSystem,
  sliceSceneSystem,
  stateAfterHeartbeat,
  stateAfterLeaving,
  stateInSpace,
} from "../../test/sceneFixture";
import type { SceneModel } from "./model";
import { applySceneNotification, toSceneModel, type SceneUpdate } from "./sceneWire";

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

/** `model` without the tidal radius, which only an arrival carries. */
function withoutTidalRadius(model: SceneModel): SceneModel {
  return model.system === null
    ? model
    : { ...model, system: { ...model.system, tidalRadiusM: null } };
}

describe("toSceneModel", () => {
  it("reads a state in a system, every body with its grant", () => {
    const model = modelOf({
      ...stateInSpace(),
      ship: shipInSystem(3_000),
      system: sliceSceneSystem(),
    });

    expect(model.system?.bodies.bodies.map((body) => body.id)).toEqual([
      FIXTURE_EARTH,
      FIXTURE_JUPITER,
    ]);
    expect(model.system?.grants.get(FIXTURE_EARTH)).toEqual({ level: "full", seen: null });
    expect(model.system?.model.hosts).toHaveLength(1);
    expect(model.system?.tidalRadiusM).toBeNull();
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

describe("applySceneNotification", () => {
  it("equals the state built directly for its end, after every notification", () => {
    const notifications = sceneSequence();

    const afterHeartbeat = applyAll(stateInSpace(), notifications.slice(0, 3));
    const afterLeaving = applyAll(stateInSpace(), notifications);

    expect(withoutTidalRadius(afterHeartbeat)).toEqual(modelOf(stateAfterHeartbeat()));
    expect(afterLeaving).toEqual(modelOf(stateAfterLeaving()));
  });

  it("keeps the arrival's tidal radius until the scene leaves the system", () => {
    const notifications = sceneSequence();

    expect(applyAll(stateInSpace(), notifications.slice(0, 3)).system?.tidalRadiusM).toBe(2.1e16);
    expect(applyAll(stateInSpace(), notifications).system).toBeNull();
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
        arrival: { type: "system", system: withoutEarth, tidal_radius_m: 2.1e16 },
        bodies: [],
      },
      { sequence: 2, clock: sceneClock(3_500), bodies: [{ level: "full", record: earth }] },
    ]);

    expect(withoutTidalRadius(model)).toEqual(
      modelOf({
        sequence: 2,
        clock: sceneClock(3_500),
        ship: shipInSystem(3_400),
        system: full,
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
