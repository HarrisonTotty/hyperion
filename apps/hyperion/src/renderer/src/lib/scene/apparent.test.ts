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
import { cameraFrameCandidate, selectCameraFrame } from "../../view/camera/frames";
import { SPEED_OF_LIGHT_M_PER_S } from "./lightTime";
import type { SceneModel, SceneSystem } from "./model";
import { toSceneModel } from "./sceneWire";

/**
 * The largest discrepancy measured against the golden (R03.T13, 2026-09-30) for a body whose
 * elements hold, as a fraction of Σ = |x_B(t − τ)| + |x_o(t)|: 5 × 10⁻¹⁵, inside Design note 7's
 * 10⁻¹⁴.
 */
const MEASURED_RATIO = 5e-15;

/**
 * The largest measured for a moon whose orbit evolves tidally while its `valid_until` is unset:
 * 2.0 × 10⁻¹³ Σ, since the simulation evaluates its elements at the emitted time and the wire states
 * them at the record's (a plan 14 matter, recorded in the plan's as-built notes).
 */
const MEASURED_EVOLVING_RATIO = 2.0e-13;

/** Design note 7's ceiling: a measurement above it is a bug, not a bound to widen. */
const CEILING_RATIO = 1e-12;

/** The pinned tolerances as fractions of Σ: ten times each measurement, held to the ceiling. */
const PINNED_RATIO = Math.min(10 * MEASURED_RATIO, CEILING_RATIO);
const PINNED_EVOLVING_RATIO = Math.min(10 * MEASURED_EVOLVING_RATIO, CEILING_RATIO);

/**
 * The bodies of a golden system whose elements differ between the epoch's answer and a century
 * on's while neither states a `valid_until`: the moons whose orbits evolve tidally.
 */
function evolving(system: GoldenReading["system"]): ReadonlySet<BodyIdHex> {
  const orbitsOf = (when: GoldenReading["when"]): Map<BodyIdHex, number> => {
    const orbits = new Map<BodyIdHex, number>();
    for (const body of goldenSystemBodies(system, when).bodies) {
      if (body.orbit.state === "ok" && body.orbit.value.valid_until === null) {
        orbits.set(body.id, body.orbit.value.orbit.semi_major_axis_m);
      }
    }
    return orbits;
  };
  const then = orbitsOf("plus_100_years");
  return new Set(
    [...orbitsOf("epoch")]
      .filter(([id, a]) => then.has(id) && then.get(id) !== a)
      .map(([id]) => id),
  );
}

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
    let worstEvolvingRatio = 0;
    for (const { golden, source, id, system } of sources) {
      const drifting = evolving(golden.system).has(id);
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
      const pinned = drifting ? PINNED_EVOLVING_RATIO : PINNED_RATIO;
      if (!(apartM <= Math.max(bound, pinned * sigma))) {
        outside.push(`${golden.system} ${golden.when} ${golden.who} ${id}: ${apartM} m`);
      }
      if (drifting) {
        worstEvolvingRatio = Math.max(worstEvolvingRatio, apartM / sigma);
      } else {
        worstRatio = Math.max(worstRatio, apartM / sigma);
      }
    }
    expect(outside).toEqual([]);
    expect(Math.max(worstRatio, worstEvolvingRatio)).toBeLessThanOrEqual(CEILING_RATIO);
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
    for (const { golden, source, id, system } of sources) {
      const track = placedTrack(systemPlacements(system), id);
      const observer = observerOf(golden);
      const sourceSpeed = source.velocityNowMPerS === null ? 0 : norm(source.velocityNowMPerS);
      const before = seenOrThrow(apparentPosition(track, observer, golden.time, null));
      const at = later(golden.time, 16_000_000);
      const cold = seenOrThrow(apparentPosition(track, observer, at, null));
      const warm = seenOrThrow(apparentPosition(track, observer, at, before.lightTime));
      const sigma = norm(cold.geometricThenM) + norm(observer.positionM);
      const apartNs = nanosBetween(warm.emitted, cold.emitted);
      const apartM = norm(minus(warm.apparentM, cold.apparentM));
      if (
        !(Math.abs(apartNs) <= emittedAllowanceNs(sigma)) ||
        !(apartM <= noteSevenBound(sigma, sourceSpeed, norm(observer.velocityMPerS), apartNs))
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
      // A 1 s chord understates a moon's speed by a part in 10⁶ at most here; the factor and the
      // metre cover that and the positions' own rounding (about a millimetre at 10 au).
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
  });

  it("names the local body selectCameraFrame names over the scene's planets and moons", () => {
    const observer = observerOf(golden);
    const placements = systemPlacements(system);
    const candidates = system.bodies.bodies.flatMap((body) => {
      const hill = system.hillRadiiM.get(body.id);
      const at = placements.placed.has(body.id)
        ? placedTrack(placements, body.id).positionAt(golden.time)
        : null;
      if (hill === undefined || at === null || body.kind.kind === "ring" || body.parent === null) {
        return [];
      }
      const parent =
        body.parent.kind === "body"
          ? body.parent.id
          : body.parent.kind === "star"
            ? formatBodyId({ system: system.model.system, bodyIndex: body.parent.bodyIndex })
            : null;
      return [cameraFrameCandidate(body.id, parent, norm(minus(observer.positionM, at)), hill)];
    });

    expect(sceneAt(model, observer, golden.time, null)?.localBody).toBe(
      selectCameraFrame(candidates, null),
    );
  });

  it("gives each placed body its Hill radius", () => {
    const body = sceneAt(model, observerOf(golden), golden.time, null)?.bodies.find(
      (each) => each.id === idOf("0100"),
    );

    expect(body?.kind === "placed" ? body.hillRadiusM : null).toBe(
      system.hillRadiiM.get(idOf("0100")),
    );
  });

  it("sees every star of the system", () => {
    const frame = sceneAt(model, observerOf(golden), golden.time, null);

    expect(frame?.stars.map((star) => star.id)).toEqual([idOf("0000")]);
  });

  it("starts a frame 16 ms on warm from the frame before, within the bound of a cold start", () => {
    const observer = observerOf(golden);
    const before = sceneAt(model, observer, golden.time, null);
    const at = later(golden.time, 16_000_000);

    const warm = sceneAt(model, observer, at, before);
    const cold = sceneAt(model, observer, at, null);

    const outside: string[] = [];
    for (const [index, body] of (cold?.bodies ?? []).entries()) {
      const other = warm?.bodies[index];
      if (other === undefined || other.id !== body.id) {
        outside.push(`${body.id} missing`);
        continue;
      }
      const sigma = norm(body.apparentM) + norm(observer.positionM);
      if (!(norm(minus(other.apparentM, body.apparentM)) <= noteSevenBound(sigma, 1e5, 0, 2))) {
        outside.push(body.id);
      }
    }
    expect(outside).toEqual([]);
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

    expect(body).toEqual({
      kind: "contact",
      id: idOf("0300"),
      apparentM: at,
      emitted: bodies.hosts.time,
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

  it("moves a body-frame ship at its body's barycentric velocity plus its own", () => {
    const planet = idOf("0100");
    const shipped: SceneModel = {
      ...model,
      ship: {
        position: { kind: "body", body: planet, offsetM: { x: 7e6, y: 0, z: 0 } },
        velocityMPerS: { x: 0, y: 10, z: 0 },
        time: golden.time,
      },
    };
    const track = placedTrack(systemPlacements(system), planet);
    const ahead = track.positionAt(later(golden.time, 1e9));
    const behind = track.positionAt(later(golden.time, -1e9));
    if (ahead === null || behind === null) {
      throw new Error("the planet is placed");
    }
    const planetVelocity = times(minus(ahead, behind), 0.5);

    const observer = shipObserver(shipped, golden.time);

    expect(
      norm(
        minus(
          observer?.velocityMPerS ?? planetVelocity,
          plus(planetVelocity, { x: 0, y: 10, z: 0 }),
        ),
      ),
    ).toBeLessThan(1e-3);
  });

  it("carries a system-frame ship in a straight line", () => {
    const shipped: SceneModel = {
      ...model,
      ship: {
        position: { kind: "system", system: system.model.system, offsetM: { x: 1e11, y: 0, z: 0 } },
        velocityMPerS: { x: 0, y: 2e4, z: 0 },
        time: golden.time,
      },
    };

    expect(shipObserver(shipped, later(golden.time, 10e9))).toEqual({
      positionM: { x: 1e11, y: 2e5, z: 0 },
      velocityMPerS: { x: 0, y: 2e4, z: 0 },
    });
  });

  it("has no observer for a ship in the galactic frame or another system's", () => {
    const galactic: SceneModel = {
      ...model,
      ship: {
        ...model.ship,
        position: {
          kind: "galactic",
          position: { cell_ly: [0, 0, 0], offset_m: [0, 0, 0] },
        },
      },
    };
    const elsewhere: SceneModel = {
      ...model,
      ship: {
        ...model.ship,
        position: { kind: "system", system: "0000000000000001", offsetM: { x: 0, y: 0, z: 0 } },
      },
    };

    expect([shipObserver(galactic, golden.time), shipObserver(elsewhere, golden.time)]).toEqual([
      null,
      null,
    ]);
  });
});
