import { describe, expect, it } from "vitest";

import fixture from "../fixtures/planetary.json" with { type: "json" };
import type { BodyKindDto } from "./generated/BodyKindDto";
import type { BodyStateDto } from "./generated/BodyStateDto";
import type { BodySummaryDto } from "./generated/BodySummaryDto";
import type { ClientMessage } from "./generated/ClientMessage";
import type { ResponseBody } from "./generated/ResponseBody";
import type { SectionDto } from "./generated/SectionDto";
import { formatBodyId, parseBodyId } from "./hex";
import { decodeServerMessage, encodeClientMessage } from "./index";
import type { RequestKind, ResponseFor } from "./requests";

/** The name of a message of the fixture, whose wire forms `hyperion-protocol`'s tests pin. */
type FixtureName = Exclude<keyof typeof fixture, "description">;

/** The astronomical unit, in metres (IAU 2012 Resolution B2). */
const METRES_PER_AU = 149_597_870_700;

const SYSTEM = "0200080020000000";
const UNIVERSE = "000000000000002a";
const EPOCH = { seconds: 0, nanos: 0 };

/** Whether `body` answers a request of kind `kind`. */
function isResponseOf<K extends RequestKind>(body: ResponseBody, kind: K): body is ResponseFor<K> {
  return body.kind === kind;
}

/** The fixture's message `name`, decoded as the client decodes a frame, as a response of `kind`. */
function response<K extends RequestKind>(name: FixtureName, kind: K): ResponseFor<K> {
  const message = decodeServerMessage(JSON.stringify(fixture[name]));
  if (message.type !== "response" || !isResponseOf(message.body, kind)) {
    throw new Error(`the fixture's ${name} is not a ${kind} response`);
  }
  return message.body;
}

/** The value of a section that must be `ok`. */
function okValue<T>(section: SectionDto<T>): T {
  if (section.state !== "ok") {
    throw new Error(`expected an ok section, got ${section.state}`);
  }
  return section.value;
}

/** A body's index within its system. */
function indexOf(body: BodySummaryDto): number {
  return parseBodyId(body.id).bodyIndex;
}

/** What a body is, in words, as a list names it. */
function kindInWords(kind: BodyKindDto): string {
  let words: string;
  switch (kind.type) {
    case "planet":
    case "ring":
    case "unresolved":
      words = kind.type;
      break;
    case "moon":
      words = `${kind.origin.replace("_", " ")} moon`;
      break;
    case "belt":
      words = `${kind.belt_kind} belt`;
      break;
    case "dwarf_planet":
    case "cometary_halo":
    case "protoplanetary_disc":
    case "debris_disc":
      words = kind.type.replace("_", " ");
      break;
  }
  return words;
}

/** A body's state, in words, as a list names it. */
function stateInWords(state: BodyStateDto): string {
  let words: string;
  switch (state.type) {
    case "not_yet_formed":
      words = "not yet formed";
      break;
    case "present":
    case "unbound":
      words = state.type;
      break;
    case "destroyed":
      words = `destroyed: ${state.cause}`;
      break;
  }
  return words;
}

describe("system_bodies", () => {
  it("decodes the slice's answer: hosts, zone, plane and planets", () => {
    const system = response("system_bodies_response", "system_bodies");

    expect(system.granted).toBe("full");
    expect(system.hosts.stars.map((star) => star.class)).toEqual(["G2V"]);
    expect(system.hosts.hierarchy.nodes).toEqual([{ type: "star", body_index: 0, mass_msun: 1.0 }]);
    const [zone] = system.zones;
    if (zone === undefined) {
      throw new Error("the slice's system lost its zone");
    }
    expect(zone.host).toEqual({ type: "star", body_index: 0 });
    expect(zone.inner_m).toBeNull();
    expect(zone.snow_line_m).toBeCloseTo(2.259 * METRES_PER_AU, -8);
    expect(zone.architecture).toBe("solar_like");
    expect(zone.habitable_zone?.moist_greenhouse_m).toBe(1.48e11);
    expect(system.system_plane).toEqual(zone.plane);
    expect([system.belts.state, system.halo.state]).toEqual(["not_modelled", "not_modelled"]);
    expect(system.bodies.map(indexOf)).toEqual([0x0300, 0x0500]);
    expect(system.bodies.map((body) => okValue(body.bulk).class)).toEqual(["rocky", "gas_giant"]);
    for (const body of system.bodies) {
      expect(parseBodyId(body.id).system).toBe(system.hosts.system);
      expect([body.label.state, body.moons.state, body.rings.state]).toEqual([
        "not_modelled",
        "not_modelled",
        "not_modelled",
      ]);
    }
  });

  it("carries each planet's whole element set and its mass in SI units", () => {
    const [earth] = response("system_bodies_response", "system_bodies").bodies;
    if (earth === undefined) {
      throw new Error("the slice's system lost its Earth");
    }
    const { parent, orbit, valid_until: validUntil } = okValue(earth.orbit);

    expect(parent).toEqual({ type: "star", body_index: 0 });
    expect(earth.parent).toEqual(parent);
    expect(orbit.semi_major_axis_m).toBe(METRES_PER_AU);
    expect(orbit.eccentricity).toBe(0.0167);
    expect(orbit.mu_m3_s2).toBe(1.327128386e20);
    expect(validUntil).toBeNull();
    expect(okValue(earth.mass_kg)).toBe(5.97216786779e24);
    expect(okValue(earth.bulk).radius_m).toBe(6_371_000);
    expect(earth.position_m).toEqual([39_081_101_553.4, -105_520_721_023, 95_232_836_817.8]);
  });

  it("rebuilds a populated system's tree from each body's parent", () => {
    const system = response("system_bodies_populated", "system_bodies");
    const children = new Map<string, number[]>();
    for (const body of system.bodies) {
      if (body.parent?.type === "body") {
        children.set(body.parent.id, [...(children.get(body.parent.id) ?? []), indexOf(body)]);
      }
    }

    expect(children).toEqual(
      new Map([
        [formatBodyId({ system: SYSTEM, bodyIndex: 0x0100 }), [0x0101, 0x0180]],
        [formatBodyId({ system: SYSTEM, bodyIndex: 0xe000 }), [0xe001]],
      ]),
    );
    const planet = system.bodies.find((body) => indexOf(body) === 0x0100);
    if (planet === undefined) {
      throw new Error("the populated system lost its first planet");
    }
    expect(okValue(planet.moons).map((id) => parseBodyId(id).bodyIndex)).toEqual([0x0101]);
    expect(okValue(planet.rings).map((id) => parseBodyId(id).bodyIndex)).toEqual([0x0180]);
    expect(okValue(system.belts).map((id) => parseBodyId(id).bodyIndex)).toEqual([0xe000]);
    expect(okValue(system.halo)).toBe(formatBodyId({ system: SYSTEM, bodyIndex: 0xe100 }));
  });

  it("names every kind and state a populated system holds", () => {
    const { bodies } = response("system_bodies_populated", "system_bodies");

    expect(bodies.map((body) => [kindInWords(body.kind), stateInWords(body.state)])).toEqual([
      ["planet", "present"],
      ["giant impact moon", "present"],
      ["ring", "present"],
      ["planet", "destroyed: engulfed"],
      ["planet", "not yet formed"],
      ["planet", "unbound"],
      ["asteroid belt", "present"],
      ["dwarf planet", "present"],
      ["cometary halo", "present"],
      ["protoplanetary disc", "destroyed: dispersed"],
      ["debris disc", "present"],
    ]);
    const unplaced = bodies.filter((body) => body.position_m === null).map(indexOf);
    expect(unplaced).toEqual([0x0180, 0x0200, 0x0300, 0x0400, 0xe000, 0xe100, 0xe200, 0xe300]);
  });
});

describe("body_detail", () => {
  it("decodes a whole record, with the surface and hooks not yet modelled", () => {
    const detail = response("body_detail_response", "body_detail");

    expect(detail.universe).toBe(UNIVERSE);
    expect(detail.granted).toBe("full");
    expect(parseBodyId(detail.record.id)).toEqual({ system: SYSTEM, bodyIndex: 0x0300 });
    expect(detail.record.kind).toEqual({ type: "planet" });
    expect(okValue(detail.record.bulk).mass_fractions).toEqual({
      iron: 0.323,
      rock: 0.677,
      water: 0,
      envelope: 0,
    });
    expect([detail.record.surface.state, detail.record.hooks.state]).toEqual([
      "not_modelled",
      "not_modelled",
    ]);
  });

  it("holds the same body in the list and in its whole record", () => {
    const [listed] = response("system_bodies_response", "system_bodies").bodies;
    const { record } = response("body_detail_response", "body_detail");
    // A record has every field a list entry has, so the list's code reads it unchanged.
    const asListed: BodySummaryDto = record;

    expect(listed).toEqual({
      id: asListed.id,
      kind: asListed.kind,
      label: asListed.label,
      parent: asListed.parent,
      state: asListed.state,
      position_m: asListed.position_m,
      mass_kg: asListed.mass_kg,
      orbit: asListed.orbit,
      moons: asListed.moons,
      rings: asListed.rings,
      bulk: asListed.bulk,
    });
  });

  it("withholds the bulk of a mass-and-orbit record, and carries no value of it", () => {
    const detail = response("body_detail_mass_and_orbit", "body_detail");
    const text = JSON.stringify(detail);

    expect(detail.granted).toBe("mass_and_orbit");
    expect(detail.record.bulk).toEqual({ state: "not_resolved" });
    expect(okValue(detail.record.mass_kg)).toBeGreaterThan(0);
    for (const key of ["radius_m", "equilibrium_temperature_k", "mass_fractions"]) {
      expect(text).not.toContain(key);
    }
  });

  it("keeps a contact's ID, parent, state and position and withholds everything else", () => {
    const { granted, record } = response("body_detail_contact", "body_detail");
    const sections = [
      record.label,
      record.mass_kg,
      record.orbit,
      record.moons,
      record.rings,
      record.bulk,
      record.surface,
      record.hooks,
    ];

    expect(granted).toBe("contact");
    expect(record.kind).toEqual({ type: "unresolved" });
    expect(new Set(sections.map((section) => section.state))).toEqual(new Set(["not_resolved"]));
    expect(record.parent).toEqual({ type: "star", body_index: 0 });
    expect(record.state).toEqual({ type: "present" });
    expect(record.position_m).toHaveLength(3);
  });

  it("decodes the refusal of a body its system does not have", () => {
    const message = decodeServerMessage(JSON.stringify(fixture.unknown_body));
    if (message.type !== "request_error") {
      throw new Error("the fixture's unknown_body is not a request error");
    }

    expect(message.error.code).toBe("unknown_body");
    expect(message.error.field).toBe("body");
  });
});

describe("body_events", () => {
  it("decodes an answer, which holds no event before its generator lands", () => {
    const events = response("body_events_response", "body_events");

    expect([events.universe, events.system]).toEqual([UNIVERSE, SYSTEM]);
    expect(events.to.seconds - events.from.seconds).toBe(3_155_760_000);
    expect(events.events).toEqual([]);
  });
});

describe("planetary requests", () => {
  it.each<[FixtureName, ClientMessage]>([
    [
      "system_bodies_request",
      {
        type: "request",
        id: 21,
        body: {
          kind: "system_bodies",
          universe: UNIVERSE,
          system: SYSTEM,
          time: EPOCH,
          detail: "full",
        },
      },
    ],
    [
      "body_detail_request",
      {
        type: "request",
        id: 23,
        body: {
          kind: "body_detail",
          universe: UNIVERSE,
          body: formatBodyId({ system: SYSTEM, bodyIndex: 0x0300 }),
          time: EPOCH,
          detail: "full",
        },
      },
    ],
    [
      "body_events_request",
      {
        type: "request",
        id: 27,
        body: {
          kind: "body_events",
          universe: UNIVERSE,
          system: SYSTEM,
          from: { seconds: -1_577_880_000, nanos: 0 },
          to: { seconds: 1_577_880_000, nanos: 0 },
        },
      },
    ],
  ])("writes %s as the server reads it", (name, message) => {
    expect(JSON.parse(encodeClientMessage(message))).toEqual(fixture[name]);
  });
});
