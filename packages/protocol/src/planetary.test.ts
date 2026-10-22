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
    // Ruling 112.7: only the giant radiates at an effective temperature of its own.
    expect(system.bodies.map((body) => okValue(body.bulk).effective_temperature_k)).toEqual([
      null,
      128.9,
    ]);
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
    expect(orbit.mu_m3_s2).toBe(1.327128386004e20);
    expect(validUntil).toBeNull();
    expect(okValue(earth.mass_kg)).toBe(5.972167867791379e24);
    expect(okValue(earth.bulk).radius_m).toBe(6_371_000);
    expect(earth.position_m).toEqual([
      39_081_101_553.427_47, -105_520_721_023.434_39, 95_232_836_817.832_58,
    ]);
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
  it("decodes a whole record, with its surface, no envelope and the hooks not yet modelled", () => {
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
    expect([
      detail.record.surface.state,
      detail.record.envelope,
      detail.record.hooks.state,
    ]).toEqual(["ok", { state: "not_applicable" }, "not_modelled"]);
  });

  it("decodes an Earth's surface, every substance a registry key (P14.T35.e)", () => {
    const surface = okValue(response("body_detail_response", "body_detail").record.surface);
    const sum = surface.gases.reduce((total, gas) => total + gas.mole_fraction, 0);

    expect(surface.state).toBe("temperate");
    expect(surface.gases.map((gas) => gas.species)).toEqual(["N2", "H2O", "Ar", "CO2"]);
    expect(Math.abs(sum - 1)).toBeLessThanOrEqual(1e-9);
    expect(surface.climate_regime.condensable).toBe("H2O");
    expect(surface.craters.screening).toEqual({ type: "atmosphere", column_mass_kg_m2: 10_318 });
    expect(surface.wet_epoch?.liquid).toBe("H2O");
    // The members of later tasks are absent, which reads not modelled, never null.
    for (const member of [
      "subsurface_ocean",
      "vertical_structure",
      "aerosols",
      "modifiers",
      "biosphere",
      "silicate_vapour",
    ]) {
      expect(member in surface).toBe(false);
    }
  });

  it("decodes an Earth's condensates, its ice's trapped area sent and its ocean's absent (P14.T54.a)", () => {
    const surface = okValue(response("body_detail_response", "body_detail").record.surface);

    expect(surface.condensates).toEqual([
      {
        substance: "H2O",
        phase: "liquid",
        reservoir_kg_m2: 2_652_000,
        reservoir_gel_m: 2_600,
        area_fraction: 0.71,
        seasonal: false,
      },
      {
        substance: "H2O",
        phase: "solid",
        reservoir_kg_m2: 53_100,
        reservoir_gel_m: 57.9,
        area_fraction: 0.03,
        trapped_area_fraction: 0,
        seasonal: false,
      },
    ]);
  });

  it("decodes an Earth's crust as the server sends it, its melt area 0 (P14.T54.a)", () => {
    const surface = okValue(response("body_detail_response", "body_detail").record.surface);

    expect(surface.crust).toEqual({
      primary: "ultramafic",
      secondary: "basalt",
      tertiary: "granite",
      bulk: null,
      lid: "secondary",
      provinces: "basalt",
      redox: "oxidised",
      mantle_redox_iw: 3.55,
      melt_area_fraction: 0,
    });
  });

  it("decodes a locked lava world: airless, no volatile air, its crust's dayside pool above 0 (P14.T54.a)", () => {
    const surface = okValue(response("body_detail_lava_world", "body_detail").record.surface);

    expect([surface.state, surface.surface_pressure_pa, surface.gases]).toEqual(["airless", 0, []]);
    expect(surface.crust?.melt_area_fraction).toBeGreaterThan(0);
    expect(surface.crust?.melt_area_fraction).toBeLessThanOrEqual(0.5);
    // Every server before P14.T55.b leaves the melt's own vapour out.
    expect("silicate_vapour" in surface).toBe(false);
  });

  it("reads a melt's own vapour absent as not modelled and null as none (P14.T35.f)", () => {
    const absent = okValue(response("body_detail_response", "body_detail").record.surface);
    const frame = fixture.body_detail_response;
    const { record } = frame.body;
    const computedNone = {
      ...frame,
      body: {
        ...frame.body,
        record: {
          ...record,
          surface: { ...record.surface, value: { ...record.surface.value, silicate_vapour: null } },
        },
      },
    };
    const message = decodeServerMessage(JSON.stringify(computedNone));
    if (message.type !== "response" || !isResponseOf(message.body, "body_detail")) {
      throw new Error("the frame with the vapour computed as none is not a body_detail response");
    }
    const surface = okValue(message.body.record.surface);
    // The binding admits no value until P14.T55.b gives the vapour its members: this line stops
    // compiling if the type does.
    const vapour: null | undefined = surface.silicate_vapour;

    expect(absent.silicate_vapour).toBeUndefined();
    expect("silicate_vapour" in surface).toBe(true);
    expect(surface.silicate_vapour).toBeNull();
    expect(vapour).toBeNull();
  });

  it("decodes a Jupiter's envelope, its surface not applicable (P14.T35.e)", () => {
    const { record } = response("body_detail_jupiter", "body_detail");
    if (record.envelope === undefined) {
      throw new Error("the fixture's Jupiter lost its envelope");
    }
    const envelope = okValue(record.envelope);

    expect(record.surface).toEqual({ state: "not_applicable" });
    expect(okValue(record.bulk).class).toBe("gas_giant");
    expect(envelope.composition.map((gas) => gas.species)).toEqual(["H2", "He", "CH4"]);
    expect(envelope.decks.map((deck) => [deck.material, deck.phase, deck.deck])).toEqual([
      ["NH3", "solid", "NH3"],
    ]);
    expect(envelope.adiabat.map((node) => node.pressure_pa)).toEqual([5e4, 1e5, 1e6, 1e7]);
  });

  it("keeps a record as a server before P14.T35.e wrote it, with no envelope key", () => {
    const { record } = response("body_detail_contact", "body_detail");

    expect("envelope" in record).toBe(false);
    expect(response("body_detail_mass_and_orbit", "body_detail").record.envelope).toEqual({
      state: "not_resolved",
    });
  });

  it("carries a ring's, a belt's and a halo's extent as their population", () => {
    const { bodies } = response("system_bodies_populated", "system_bodies");
    const population = (type: string) =>
      bodies.flatMap((body) => {
        const section = body.population;
        return section.state === "ok" && section.value.type === type ? [section.value] : [];
      });
    const [ring] = population("ring");
    const [belt] = population("belt");
    const [halo] = population("cometary_halo");
    expect(ring?.type === "ring" && ring.inner_edge_m < ring.outer_edge_m).toBe(true);
    expect(belt?.type === "belt" && belt.members.state).toBe("ok");
    expect(halo?.type === "cometary_halo" && halo.comets).toBeGreaterThan(0);
    const planet = bodies.find((body) => body.kind.type === "planet");
    expect(planet?.population).toEqual({ state: "not_applicable" });
  });

  it("carries the ring's plane's normal as a unit vector", () => {
    const { bodies } = response("system_bodies_populated", "system_bodies");
    const ring = bodies.flatMap((body) =>
      body.population.state === "ok" && body.population.value.type === "ring"
        ? [body.population.value]
        : [],
    )[0];
    // As every server from the 22 → 23 batch sends it.
    const normal = ring?.normal;
    if (normal === undefined) {
      throw new Error("the populated answer's ring lost its plane's normal");
    }

    expect(Math.hypot(...normal)).toBeCloseTo(1, 12);
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
      population: asListed.population,
      bulk: asListed.bulk,
      rotation: asListed.rotation,
      figure: asListed.figure,
      photometry: asListed.photometry,
    });
  });

  it("decodes a body's frame, figure and photometry", () => {
    const [earth] = response("system_bodies_response", "system_bodies").bodies;
    if (earth?.rotation === undefined || earth.figure === undefined) {
      throw new Error("the slice's Earth lost its frame or figure");
    }
    if (earth.photometry === undefined) {
      throw new Error("the slice's Earth lost its photometry");
    }
    const rotation = okValue(earth.rotation);
    const figure = okValue(earth.figure);
    const photometry = okValue(earth.photometry);

    expect(rotation.equator_node).toEqual([1, 0, 0]);
    expect(rotation.locking_age_s).toBeNull();
    expect(rotation.locks_at).toBeNull();
    expect(rotation.resonance).toBe("synchronous");
    expect(figure.pole).toEqual(rotation.pole);
    expect(figure.equatorial_radius_m).toBe(6_378_137);
    expect(figure.law).toBe("rotational");
    expect(figure.datum).toBe("solid_surface");
    expect(photometry.phase_template).toBe("earth");
    expect(photometry.geometric_albedo.b).toBe(0.263);
    expect(photometry.provisional).toBe(false);
  });

  it("decodes a body from an older server, which sends no frame, figure or photometry", () => {
    // The slice's giant keeps the form a server before P14.T46.f sent.
    const [, giant] = response("system_bodies_response", "system_bodies").bodies;
    if (giant === undefined) {
      throw new Error("the slice's system lost its giant");
    }

    expect("rotation" in giant || "figure" in giant || "photometry" in giant).toBe(false);
    expect(okValue(giant.bulk).class).toBe("gas_giant");
  });

  it("withholds the bulk of a mass-and-orbit record, and carries no value of it", () => {
    const detail = response("body_detail_mass_and_orbit", "body_detail");
    const text = JSON.stringify(detail);

    expect(detail.granted).toBe("mass_and_orbit");
    expect(detail.record.bulk).toEqual({ state: "not_resolved" });
    expect([detail.record.rotation, detail.record.figure, detail.record.photometry]).toEqual([
      { state: "not_resolved" },
      { state: "not_resolved" },
      { state: "not_resolved" },
    ]);
    expect(okValue(detail.record.mass_kg)).toBeGreaterThan(0);
    for (const key of [
      "radius_m",
      "equilibrium_temperature_k",
      "mass_fractions",
      "pole",
      "obliquity_rad",
      "geometric_albedo",
    ]) {
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
    expect([record.rotation, record.figure, record.photometry]).toEqual([
      { state: "not_resolved" },
      { state: "not_resolved" },
      { state: "not_resolved" },
    ]);
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

/**
 * Titan's surface at the Huygens landing site, K and Pa, with their uncertainties: "At the surface,
 * the temperature was 93.65 ± 0.25 K, and the pressure was 1,467 ± 1 hPa" (Fulchignoni et al. 2005,
 * Nature 438, 785, abstract; the Huygens Atmospheric Structure Instrument).
 */
const HUYGENS_SURFACE_K = 93.65;
const HUYGENS_SURFACE_K_ERROR = 0.25;
const HUYGENS_SURFACE_PA = 146_700;
const HUYGENS_SURFACE_PA_ERROR = 100;

/**
 * Titan's methane mole fraction near the surface, with its uncertainty: "(5.65 ± 0.18) × 10⁻² near
 * the surface (6.7 km to the surface)", against (1.48 ± 0.09) × 10⁻² in the lower stratosphere
 * (Niemann et al. 2010, JGR 115, E12006, abstract; the Huygens Gas Chromatograph Mass
 * Spectrometer).
 */
const NEAR_SURFACE_CH4 = 0.0565;
const NEAR_SURFACE_CH4_ERROR = 0.0018;

/**
 * The share of Titan's surface under liquid, its lakes and seas alone: 0.85–1.07% (Hayes 2016,
 * Annu. Rev. Earth Planet. Sci. 44, 57, as Birch, Hayes and Perron 2025, "Titan's fluvial and
 * lacustrine landscapes", in _Titan After Cassini-Huygens_, Elsevier, 287, cite it; read through
 * decision-p14-t51a-part2's table). Lacustrine terrain, filled and empty, covers about 1.1%.
 */
const LAKES_AREA_FRACTION = { min: 0.0085, max: 0.0107 };

/** Whether `value` lies within `error` of `measured`. */
function isWithin(value: number, measured: number, error: number): boolean {
  return Math.abs(value - measured) <= error;
}

describe("body_detail's Titan, with its lakes (P14.T54.a)", () => {
  const titan = () => response("body_detail_titan", "body_detail").record;

  it("is a regular moon, icy, of a planet of the fixture's system", () => {
    const record = titan();

    expect(parseBodyId(record.id)).toEqual({ system: SYSTEM, bodyIndex: 0x0601 });
    expect(record.kind).toEqual({ type: "moon", origin: "regular" });
    expect(record.parent).toEqual({
      type: "body",
      id: formatBodyId({ system: SYSTEM, bodyIndex: 0x0600 }),
    });
    expect(okValue(record.bulk).class).toBe("icy");
  });

  it("holds the Huygens site's measured surface temperature and pressure", () => {
    const surface = okValue(titan().surface);

    expect(
      isWithin(surface.mean_surface_temperature_k, HUYGENS_SURFACE_K, HUYGENS_SURFACE_K_ERROR),
    ).toBe(true);
    expect(
      isWithin(surface.surface_pressure_pa, HUYGENS_SURFACE_PA, HUYGENS_SURFACE_PA_ERROR),
    ).toBe(true);
  });

  it("holds an air of N2 with the measured near-surface CH4, summing to 1", () => {
    const { gases } = okValue(titan().surface);
    const methane = gases.find((gas) => gas.species === "CH4")?.mole_fraction ?? Number.NaN;
    const sum = gases.reduce((total, gas) => total + gas.mole_fraction, 0);

    expect(gases.map((gas) => gas.species)).toEqual(["N2", "CH4"]);
    expect(isWithin(methane, NEAR_SURFACE_CH4, NEAR_SURFACE_CH4_ERROR)).toBe(true);
    expect(Math.abs(sum - 1)).toBeLessThanOrEqual(1e-9);
  });

  it("holds its lakes as a ch4_c2h6_n2 liquid condensate on the measured share of the surface", () => {
    const surface = okValue(titan().surface);
    const [lakes, ...others] = surface.condensates ?? [];

    expect(others).toEqual([]);
    expect([lakes?.substance, lakes?.phase]).toEqual(["ch4_c2h6_n2", "liquid"]);
    const area = lakes?.area_fraction ?? Number.NaN;
    expect(area >= LAKES_AREA_FRACTION.min && area <= LAKES_AREA_FRACTION.max).toBe(true);
    expect(surface.surface_liquids).toEqual([{ substance: "ch4_c2h6_n2", area_fraction: area }]);
    expect(surface.ocean_fraction).toBe(area);
    expect(surface.climate_regime.condensable).toBe("CH4");
  });

  it("holds an H2O lid whose melt area is 0, its liquid a condensate and not a melt", () => {
    const surface = okValue(titan().surface);

    expect([surface.state, surface.material]).toEqual(["snowball", "ice"]);
    expect(surface.crust?.lid).toBe("primary");
    expect(surface.crust?.primary).toBe("H2O");
    expect(surface.crust?.melt_area_fraction).toBe(0);
  });

  it("holds no melted-rock crust under its ice, as its moment of inertia implies", () => {
    const { crust } = okValue(titan().surface);

    // "The inferred moment of inertia factor is about 0.34, implying incomplete differentiation"
    // (Iess et al. 2010, Science 327, 1367, abstract; Cassini's gravity field): the rock never
    // melted to a crust of its own, so the server's form is a regular icy moon's whose rock did
    // not melt, an `H2O` primary crust and no other.
    expect([crust?.secondary, crust?.tertiary, crust?.bulk]).toEqual([null, null, null]);
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
