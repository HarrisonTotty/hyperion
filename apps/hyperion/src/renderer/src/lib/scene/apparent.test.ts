import {
  type BodyIdHex,
  formatBodyId,
  type SystemBodiesDto,
  type UniverseTime,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { type Vec3 } from "../../geometry/vec3";
import {
  type GoldenReading,
  type GoldenSource,
  goldenReadings,
  goldenSystemBodies,
} from "../../test/inSystemGolden";
import {
  apparentPosition,
  placedTrack,
  type SceneObserver,
  sceneAt,
  type SeenSource,
  shipObserver,
  systemPlacements,
} from "./apparent";
import { SPEED_OF_LIGHT_M_PER_S } from "./lightTime";
import type { SceneModel, SceneSystem } from "./model";
import { toSceneModel } from "./sceneWire";

/**
 * The largest discrepancy measured against the golden, as a fraction of Σ = |x_B(t − τ)| + |x_o(t)|
 * (R03.T13, 2026-09-30): 2.0 × 10⁻¹³, from two moons whose orbits evolve tidally, which the
 * simulation evaluates at the emitted time and the wire states at the record's; every other vector
 * agrees within 7 × 10⁻¹⁵ Σ.
 */
const MEASURED_RATIO = 2.0e-13;

/** Design note 7's ceiling: a measurement above it is a bug, not a bound to widen. */
const CEILING_RATIO = 1e-12;

/**
 * The pinned tolerance as a fraction of Σ: ten times the measurement, held to the ceiling, which
 * the plan's Verification says the client is never looser than.
 */
const PINNED_RATIO = Math.min(10 * MEASURED_RATIO, CEILING_RATIO);

function norm(v: Vec3): number {
  return Math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
}

function minus(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z };
}

function plus(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z };
}

function times(v: Vec3, k: number): Vec3 {
  return { x: v.x * k, y: v.y * k, z: v.z * k };
}

/** The angle between two vectors, rad, by the cross product, which keeps small angles. */
function angle(a: Vec3, b: Vec3): number {
  const cross = {
    x: a.y * b.z - a.z * b.y,
    y: a.z * b.x - a.x * b.z,
    z: a.x * b.y - a.y * b.x,
  };
  return Math.atan2(norm(cross), a.x * b.x + a.y * b.y + a.z * b.z);
}

function nanosBetween(a: UniverseTime, b: UniverseTime): number {
  return (a.seconds - b.seconds) * 1e9 + (a.nanos - b.nanos);
}

function later(time: UniverseTime, nanos: number): UniverseTime {
  const total = time.nanos + nanos;
  const carry = Math.floor(total / 1e9);
  return { seconds: time.seconds + carry, nanos: total - carry * 1e9 };
}

/** A body the scene grants only `contact`, placed by the server at `at`. */
interface Contact {
  readonly id: BodyIdHex;
  readonly at: Vec3;
}

/** A scene in `bodies`' system, every body granted the level the answer holds but `contact`. */
function sceneOf(bodies: SystemBodiesDto, contact: Contact | null = null): SceneModel {
  const result = toSceneModel(
    {
      sequence: 0,
      clock: { time: bodies.hosts.time, time_rate: 0, state: "paused" },
      ship: {
        position: { frame: "system", system: bodies.hosts.system, offset_m: [0, 0, 0] },
        velocity_m_s: [0, 0, 0],
        time: bodies.hosts.time,
      },
      system: {
        system: bodies,
        grants: bodies.bodies.map((body) =>
          contact?.id === body.id
            ? {
                body: body.id,
                level: "contact",
                seen: {
                  apparent_m: [contact.at.x, contact.at.y, contact.at.z],
                  emitted: bodies.hosts.time,
                },
              }
            : { body: body.id, level: bodies.granted },
        ),
      },
      craft: [],
    },
    () => "GOLDEN",
  );
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return result.model;
}

function systemOf(model: SceneModel): SceneSystem {
  if (model.system === null) {
    throw new Error("a golden scene has a system");
  }
  return model.system;
}

function reading(
  system: GoldenReading["system"],
  when: GoldenReading["when"],
  who: string,
): GoldenReading {
  const found = goldenReadings().find(
    (each) => each.system === system && each.when === when && each.who === who,
  );
  if (found === undefined) {
    throw new Error(`the golden has no ${system} ${when} ${who}`);
  }
  return found;
}

function observerOf(golden: GoldenReading): SceneObserver {
  return { positionM: golden.observerM, velocityMPerS: golden.observerVelocityMPerS };
}

function seenOrThrow(result: ReturnType<typeof apparentPosition>): SeenSource {
  if (result.kind !== "seen") {
    throw new Error(`not seen: ${result.kind}`);
  }
  return result;
}

/** One golden source the wire also lists, with the scene it is placed in. */
interface Compared {
  readonly golden: GoldenReading;
  readonly source: GoldenSource;
  readonly id: BodyIdHex;
  readonly system: SceneSystem;
}

/**
 * Every golden source seen whose body or star the client places, for every reading: the planets,
 * moons and stars; a belt's members are not on the wire yet and a ring has no single position.
 */
function compared(): Compared[] {
  const all: Compared[] = [];
  for (const golden of goldenReadings()) {
    const system = systemOf(sceneOf(goldenSystemBodies(golden.system, golden.when)));
    const placements = systemPlacements(system);
    for (const source of golden.sources) {
      const id = formatBodyId({ system: system.model.system, bodyIndex: source.bodyIndex });
      if (source.seen !== null && placements.placements.has(id)) {
        all.push({ golden, source, id, system });
      }
    }
  }
  return all;
}

/** Design note 7's bound on |A_client − A_sim|, m: 1 mm + 10⁻¹⁴ Σ + (|v_B| + γ |v_o|) |Δemitted|. */
function noteSevenBound(
  sigma: number,
  sourceSpeed: number,
  observerSpeed: number,
  emittedApartNs: number,
): number {
  const beta = observerSpeed / SPEED_OF_LIGHT_M_PER_S;
  const gamma = 1 / Math.sqrt(1 - beta * beta);
  return (
    1e-3 + 1e-14 * sigma + (sourceSpeed + gamma * observerSpeed) * Math.abs(emittedApartNs) * 1e-9
  );
}

/** Design note 7's allowance on the emitted times, ns: 1 ns + ⌈(1 mm + 10⁻¹⁴ Σ) ÷ c⌉. */
function emittedAllowanceNs(sigma: number): number {
  return 1 + Math.ceil(((1e-3 + 1e-14 * sigma) / SPEED_OF_LIGHT_M_PER_S) * 1e9);
}

describe("apparentPosition against the simulation's golden vectors", () => {
  const sources = compared();

  it("compares every planet, moon and star the wire carries", () => {
    expect(sources.length).toBeGreaterThanOrEqual(140);
    expect(sources.filter(({ source }) => source.kind === "star").length).toBeGreaterThanOrEqual(
      20,
    );
  });

  it("places every vector within the pinned bound, and measures under the ceiling", () => {
    const outside: string[] = [];
    let worstRatio = 0;
    for (const { golden, source, id, system } of sources) {
      const track = placedTrack(systemPlacements(system), id);
      const seen = seenOrThrow(apparentPosition(track, observerOf(golden), golden.time, null));
      const sim = source.seen;
      if (sim === null) {
        throw new Error("only seen sources are compared");
      }
      const sigma = norm(sim.geometricM) + norm(golden.observerM);
      const apartM = norm(minus(seen.apparentM, sim.apparentM));
      const bound = noteSevenBound(
        sigma,
        source.velocityNowMPerS === null ? 0 : norm(source.velocityNowMPerS),
        norm(golden.observerVelocityMPerS),
        nanosBetween(seen.emitted, sim.emitted),
      );
      if (!(apartM <= Math.max(bound, PINNED_RATIO * sigma))) {
        outside.push(`${golden.system} ${golden.when} ${golden.who} ${id}: ${apartM} m`);
      }
      worstRatio = Math.max(worstRatio, apartM / sigma);
    }
    expect(outside).toEqual([]);
    expect(worstRatio).toBeLessThanOrEqual(CEILING_RATIO);
  });

  it("gives every emitted time within the allowance of the golden's", () => {
    const outside: string[] = [];
    for (const { golden, source, id, system } of sources) {
      const track = placedTrack(systemPlacements(system), id);
      const seen = seenOrThrow(apparentPosition(track, observerOf(golden), golden.time, null));
      const sim = source.seen;
      if (sim === null) {
        throw new Error("only seen sources are compared");
      }
      const sigma = norm(sim.geometricM) + norm(golden.observerM);
      const apartNs = Math.abs(nanosBetween(seen.emitted, sim.emitted));
      if (!(apartNs <= emittedAllowanceNs(sigma))) {
        outside.push(`${golden.system} ${golden.when} ${golden.who} ${id}: ${apartNs} ns`);
      }
    }
    expect(outside).toEqual([]);
  });

  it("gives every Hill radius within 10⁻¹² of the golden's", () => {
    const outside: string[] = [];
    let checked = 0;
    for (const golden of goldenReadings()) {
      const system = systemOf(sceneOf(goldenSystemBodies(golden.system, golden.when)));
      const onWire = new Set(system.bodies.bodies.map((body) => body.id));
      for (const source of golden.sources) {
        const id = formatBodyId({ system: system.model.system, bodyIndex: source.bodyIndex });
        if (source.hillRadiusM === null || !onWire.has(id)) {
          continue;
        }
        const hill = system.hillRadiiM.get(id);
        if (
          hill === undefined ||
          !(Math.abs(hill - source.hillRadiusM) <= 1e-12 * source.hillRadiusM)
        ) {
          outside.push(`${id}: ${String(hill)} m against ${source.hillRadiusM} m`);
        }
        checked += 1;
      }
    }
    expect(outside).toEqual([]);
    expect(checked).toBeGreaterThan(50);
  });

  it("agrees between a warm start from a frame 16 ms before and a cold start", () => {
    const outside: string[] = [];
    for (const { golden, id, system } of sources) {
      const track = placedTrack(systemPlacements(system), id);
      const observer = observerOf(golden);
      const before = seenOrThrow(apparentPosition(track, observer, golden.time, null));
      const at = later(golden.time, 16_000_000);
      const cold = seenOrThrow(apparentPosition(track, observer, at, null));
      const warm = seenOrThrow(apparentPosition(track, observer, at, before.lightTime));
      const sigma = norm(cold.geometricThenM) + norm(observer.positionM);
      const apartNs = nanosBetween(warm.emitted, cold.emitted);
      // 10⁵ m/s bounds every source's speed in these systems.
      const apartM = norm(minus(warm.apparentM, cold.apparentM));
      if (
        !(Math.abs(apartNs) <= emittedAllowanceNs(sigma)) ||
        !(apartM <= noteSevenBound(sigma, 1e5, norm(observer.velocityMPerS), apartNs))
      ) {
        outside.push(`${id}: ${apartNs} ns, ${apartM} m`);
      }
    }
    expect(outside).toEqual([]);
  });
});

describe("apparentPosition", () => {
  const golden = reading("solar_like", "epoch", "low_orbit");
  const system = systemOf(sceneOf(goldenSystemBodies("solar_like", "epoch")));
  const placements = systemPlacements(system);
  const idOf = (suffix: string): BodyIdHex => `${system.model.system}.${suffix}`;

  it("shows a far body and its moons shifted together, as their light times allow", () => {
    const outside: string[] = [];
    const giant = placedTrack(placements, idOf("0500"));
    const observer = observerOf(golden);
    const giantSeen = seenOrThrow(apparentPosition(giant, observer, golden.time, null));
    const giantNow = giant.positionAt(golden.time);
    const giantAhead = giant.positionAt(later(golden.time, 1e9));
    let moons = 0;
    for (const suffix of ["0501", "0502", "0503", "0504", "0505", "0506", "0507"]) {
      const moon = placedTrack(placements, idOf(suffix));
      const moonSeen = seenOrThrow(apparentPosition(moon, observer, golden.time, null));
      const moonNow = moon.positionAt(golden.time);
      const moonAhead = moon.positionAt(later(golden.time, 1e9));
      if (giantNow === null || giantAhead === null || moonNow === null || moonAhead === null) {
        throw new Error("the giant and its moons are present");
      }
      // The moon's speed about the giant, m/s, by a difference over a second.
      const relativeSpeed = norm(minus(minus(moonAhead, moonNow), minus(giantAhead, giantNow)));
      const tauS = moonSeen.lightTime.seconds + moonSeen.lightTime.nanos * 1e-9;
      const seenApart = minus(moonSeen.apparentM, giantSeen.apparentM);
      const truly = minus(moonNow, giantNow);
      // The two light times differ by the moon's offset over c, across which the giant moves and
      // the observer's aberration shifts.
      const tausApartS = Math.abs(nanosBetween(moonSeen.emitted, giantSeen.emitted)) * 1e-9;
      const giantSpeed = norm(minus(giantAhead, giantNow));
      const sideways = (giantSpeed + norm(observer.velocityMPerS)) * tausApartS;
      const offM = norm(minus(seenApart, truly));
      if (!(offM <= 1.01 * relativeSpeed * tauS + sideways + 1)) {
        outside.push(`${suffix}: ${offM} m`);
      }
      moons += 1;
    }
    expect(outside).toEqual([]);
    expect(moons).toBe(7);
  });

  it("leaves a source out when it is not present at its emitted time", () => {
    const planet = placedTrack(placements, idOf("0300"));
    const appearing = {
      positionAt: (time: UniverseTime): Vec3 | null =>
        nanosBetween(time, golden.time) > -1e9 ? planet.positionAt(time) : null,
    };

    expect(apparentPosition(appearing, observerOf(golden), golden.time, null).kind).toBe(
      "not_present_then",
    );
  });

  it("shows a body moving with the observer near its present direction", () => {
    const planet = placedTrack(placements, idOf("0300"));
    const now = planet.positionAt(golden.time);
    const ahead = planet.positionAt(later(golden.time, 1e9));
    const behind = planet.positionAt(later(golden.time, -1e9));
    if (now === null || ahead === null || behind === null) {
      throw new Error("the planet is present");
    }
    const velocity = times(minus(ahead, behind), 0.5);
    const acceleration = norm(plus(minus(ahead, now), minus(behind, now)));
    const observer = {
      positionM: plus(now, { x: 1e9, y: -4e8, z: 2e8 }),
      velocityMPerS: velocity,
    };

    const seen = seenOrThrow(apparentPosition(planet, observer, golden.time, null));

    const beta = norm(velocity) / SPEED_OF_LIGHT_M_PER_S;
    const tauS = seen.lightTime.seconds + seen.lightTime.nanos * 1e-9;
    expect(
      angle(minus(seen.apparentM, observer.positionM), minus(now, observer.positionM)),
    ).toBeLessThanOrEqual(
      (beta * beta) / 4 + (acceleration * tauS) / (2 * SPEED_OF_LIGHT_M_PER_S) + 1e-14,
    );
  });
});

describe("sceneAt", () => {
  const golden = reading("solar_like", "epoch", "low_orbit");
  const bodies = goldenSystemBodies("solar_like", "epoch");
  const model = sceneOf(bodies);
  const system = systemOf(model);
  const idOf = (suffix: string): BodyIdHex => `${system.model.system}.${suffix}`;

  it("names the planet the ship orbits low as its local body", () => {
    const frame = sceneAt(model, observerOf(golden), golden.time, null);

    expect(frame?.localBody).toBe(idOf("0100"));
    expect(frame?.stars.map((star) => star.id)).toEqual([idOf("0000")]);
    expect(frame?.bodies.find((body) => body.id === idOf("0100"))?.hillRadiusM).toBe(
      system.hillRadiiM.get(idOf("0100")),
    );
  });

  it("keeps whichever body it had for a ship at 0.95 of a moon's sphere", () => {
    const moon = idOf("0501");
    const hill = system.hillRadiiM.get(moon);
    const moonNow = placedTrack(systemPlacements(system), moon).positionAt(golden.time);
    if (hill === undefined || moonNow === null) {
      throw new Error("the moon is present with a Hill radius");
    }
    const ship = {
      positionM: plus(moonNow, { x: 0.95 * hill, y: 0, z: 0 }),
      velocityMPerS: { x: 0, y: 0, z: 0 },
    };

    const fresh = sceneAt(model, ship, golden.time, null);
    if (fresh === null) {
      throw new Error("the scene has a system");
    }
    const kept = sceneAt(model, ship, golden.time, { ...fresh, localBody: moon });

    expect(fresh.localBody).toBe(idOf("0500"));
    expect(kept?.localBody).toBe(moon);
  });

  it("leaves out a body its record says is gone", () => {
    const destroyed = sceneOf({
      ...bodies,
      bodies: bodies.bodies.map((body) =>
        body.id === idOf("0300")
          ? {
              ...body,
              state: { type: "destroyed", cause: "engulfed", at: { seconds: -10, nanos: 0 } },
            }
          : body,
      ),
    });

    const frame = sceneAt(destroyed, observerOf(golden), golden.time, null);

    expect(frame?.bodies.some((body) => body.id === idOf("0300"))).toBe(false);
    expect(frame?.bodies.some((body) => body.id === idOf("0200"))).toBe(true);
  });

  it("places a contact at the server's seen position with no geometry or Hill radius", () => {
    const at = { x: 1.2e11, y: -3e10, z: 4e9 };
    const contact = sceneOf(bodies, { id: idOf("0300"), at });

    const body = sceneAt(contact, observerOf(golden), golden.time, null)?.bodies.find(
      (each) => each.id === idOf("0300"),
    );

    expect(body).toMatchObject({
      apparentM: at,
      geometricM: null,
      seen: true,
      hillRadiusM: null,
      level: "contact",
    });
  });

  it("carries a body-frame ship with its body", () => {
    const planet = idOf("0100");
    const shipped: SceneModel = {
      ...model,
      ship: {
        position: { kind: "body", body: planet, offsetM: { x: 7e6, y: 0, z: 0 } },
        velocityMPerS: { x: 0, y: 10, z: 0 },
        time: golden.time,
      },
    };
    const at = later(golden.time, 60e9);

    const observer = shipObserver(shipped, at);
    const planetThen = placedTrack(systemPlacements(system), planet).positionAt(at);
    if (observer === null || planetThen === null) {
      throw new Error("the ship and its planet are placed");
    }

    expect(
      norm(minus(observer.positionM, plus(planetThen, { x: 7e6, y: 600, z: 0 }))),
    ).toBeLessThan(1e-3);
  });
});
