import type {
  AbsorberDto,
  AerosolInventoryDto,
  AerosolModeDto,
  BodyEnvelopeDto,
  BodyFigureDto,
  BodyPhotometryDto,
  BodyRecordDto,
  BodyRotationDto,
  BodyStateDto,
  BodySummaryDto,
  BodySurfaceDto,
  CondensateDeckDto,
  OrbitDriftDto,
  ResponseFor,
  SectionDto,
  SubsurfaceOceanDto,
  SurfaceCondensateDto,
  SurfaceModifiersDto,
  UniverseTime,
  VerticalStructureDto,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  earthContact,
  earthDetail,
  earthDetailOfAnOlderServer,
  earthMassAndOrbit,
  earthUnderASupercriticalEnvelope,
  FIXTURE_EARTH,
  FIXTURE_RING_NORMAL,
  FIXTURE_SYSTEM,
  jupiterDetail,
  lavaWorldDetail,
  populatedBodies,
  populatedBodiesWith,
  sliceBodies,
  sliceBodiesWith,
  tiltedPole,
  titanDetail,
  withoutRingNormal,
  withRingNormal,
} from "../../test/planetaryFixture";
import { GOLDEN_SYSTEMS, GOLDEN_TIMES, goldenSystemBodies } from "../../test/inSystemGolden";
import { vec3 } from "../../geometry/vec3";
import {
  bodyDetailSeed,
  bodyRecordOrbit,
  bodyRecordSeasons,
  EARTH_MASS_KG,
  toBodiesRequest,
  toBodyDetail,
  toBodyDetailRequest,
  toSystemBodiesModel,
} from "./bodiesWire";
import type { AerosolInventory, AerosolMode, Modelled, Section, SurfaceCondensate } from "./model";

const DESIGNATION = "H7K 4C0RFZ D-7";

function bodiesOf(response = sliceBodies()) {
  const result = toSystemBodiesModel(response, DESIGNATION);
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return result;
}

function faultOf(response = sliceBodies()): string {
  const result = toSystemBodiesModel(response, DESIGNATION);
  return result.kind === "fault" ? result.fault : "no fault";
}

/** The plane's normal of the populated answer's ring as `response` reads, if it reads a ring. */
function ringNormalOf(response: ResponseFor<"system_bodies">) {
  const ring = bodiesOf(response).bodies.bodies.find((body) => body.bodyIndex === 0x0180);
  return ring?.population.state === "ok" && ring.population.value.kind === "ring"
    ? ring.population.value.normal
    : undefined;
}

/** The slice's answer with its first body in `state`, and so with no orbit. */
function withFirstState(state: BodyStateDto) {
  const first = sliceBodies().bodies[0]?.id;
  if (first === undefined) {
    throw new Error("the slice's answer has a body");
  }
  return sliceBodiesWith((body) =>
    body.id === first ? { ...body, state, orbit: { state: "not_applicable" } } : body,
  );
}

/** The slice's answer with every orbit drifting as `drift` says. */
function withDrift(drift: OrbitDriftDto) {
  return sliceBodiesWith((body) =>
    body.orbit.state === "ok"
      ? { ...body, orbit: { state: "ok", value: { ...body.orbit.value, drift } } }
      : body,
  );
}

/** A drift of close_binary's receding moon's size, from the epoch. */
const DRIFT: OrbitDriftDto = {
  reference: { seconds: 0, nanos: 0 },
  semi_major_axis_rate_m_per_s: 2.59e-9,
  eccentricity_rate_per_s: 0,
  mean_motion_rate_rad_per_s2: -1.27e-22,
};

/** The slice's answer with every orbit's `valid_until` set to `validUntil`. */
function withValidUntil(validUntil: UniverseTime) {
  return sliceBodiesWith((body) =>
    body.orbit.state === "ok"
      ? { ...body, orbit: { state: "ok", value: { ...body.orbit.value, valid_until: validUntil } } }
      : body,
  );
}

/** A record's hooks section, `ok`, carrying `detailSeed`. */
function withHooks(detailSeed: SectionDto<string>): Pick<BodyRecordDto, "hooks"> {
  return { hooks: { state: "ok", value: { detail_seed: detailSeed } } };
}

/** The fixture's Earth with a hooks section that is `ok` and carries `detailSeed`, read. */
function earthWithDetailSeed(detailSeed: SectionDto<string>) {
  const detail = earthDetail();
  return toBodyDetail(
    {
      ...detail,
      record: { ...detail.record, hooks: { state: "ok", value: { detail_seed: detailSeed } } },
    },
    FIXTURE_SYSTEM,
    DESIGNATION,
  );
}

/** The slice's answer with its giant's effective temperature set to `effectiveK`. */
function withEffective(effectiveK: number) {
  return sliceBodiesWith((body) =>
    body.bulk.state === "ok" && body.bulk.value.effective_temperature_k !== null
      ? {
          ...body,
          bulk: { state: "ok", value: { ...body.bulk.value, effective_temperature_k: effectiveK } },
        }
      : body,
  );
}

describe("toBodiesRequest and toBodyDetailRequest", () => {
  it("ask for every level of detail at the time given", () => {
    const time = { seconds: 12, nanos: 5 };

    expect(toBodiesRequest("000000000000002a", FIXTURE_SYSTEM, time)).toEqual({
      kind: "system_bodies",
      universe: "000000000000002a",
      system: FIXTURE_SYSTEM,
      time,
      detail: "full",
    });
    expect(toBodyDetailRequest("000000000000002a", FIXTURE_EARTH, time)).toEqual({
      kind: "body_detail",
      universe: "000000000000002a",
      body: FIXTURE_EARTH,
      time,
      detail: "full",
    });
  });
});

describe("toSystemBodiesModel", () => {
  it("reads the slice's answer: its star, its zone, its plane and two planets", () => {
    const { model, bodies } = bodiesOf();

    expect(model.hosts.map((host) => host.designation)).toEqual([`${DESIGNATION} /0`]);
    expect(bodies.granted).toBe("full");
    expect(bodies.systemPlane).toEqual({ inclinationRad: 1, ascendingNodeRad: 2.5 });
    expect(bodies.zones.map((zone) => zone.architecture)).toEqual(["solar_like"]);
    expect(bodies.bodies.map((body) => [body.designation, body.kind.kind])).toEqual([
      [`${DESIGNATION} /768`, "planet"],
      [`${DESIGNATION} /1280`, "planet"],
    ]);
    expect(bodies.belts.state).toBe("not_modelled");
    expect(bodies.halo.state).toBe("not_modelled");
  });

  it("holds masses in Earth masses by the simulation's Earth mass", () => {
    const earth = bodiesOf().bodies.bodies[0];

    expect(EARTH_MASS_KG).toBe(3.986_004e14 / 6.674_3e-11);
    expect(earth?.massMearth).toEqual({
      state: "ok",
      value: 5.972_167_867_791_379e24 / EARTH_MASS_KG,
    });
    expect(earth?.massMearth.state === "ok" ? earth.massMearth.value : 0).toBeCloseTo(1, 9);
  });

  it("keeps every section's tag as the server set it", () => {
    const earth = bodiesOf().bodies.bodies[0];

    expect(earth?.label).toEqual({ state: "not_modelled" });
    expect(earth?.moons).toEqual({ state: "not_modelled" });
    expect(earth?.bulk.state).toBe("ok");
  });

  it("reads every kind, state and section state of the populated answer", () => {
    const { bodies } = bodiesOf(populatedBodies());

    expect(bodies.bodies.map((body) => [body.kind.kind, body.state.kind])).toEqual([
      ["planet", "present"],
      ["moon", "present"],
      ["ring", "present"],
      ["planet", "destroyed"],
      ["planet", "not_yet_formed"],
      ["planet", "unbound"],
      ["belt", "present"],
      ["dwarf_planet", "present"],
      ["cometary_halo", "present"],
      ["protoplanetary_disc", "destroyed"],
      ["debris_disc", "present"],
    ]);
    expect(bodies.belts).toEqual({ state: "ok", value: [`${FIXTURE_SYSTEM}.e000`] });
    expect(bodies.halo).toEqual({ state: "ok", value: `${FIXTURE_SYSTEM}.e100` });
  });

  it("reads a ring's, a belt's and the halo's populations in metres, each section as tagged", () => {
    const { bodies } = bodiesOf(populatedBodies());
    const population = (index: number) => {
      const section = bodies.bodies.find((body) => body.bodyIndex === index)?.population;
      return section?.state === "ok" ? section.value : section;
    };

    expect(population(0x0180)).toMatchObject({
      kind: "ring",
      ringKind: "massive",
      material: "porous_ice",
      innerEdgeM: 66_000_000,
      outerEdgeM: 136_800_000,
      gaps: [{ moon: `${FIXTURE_SYSTEM}.0101`, resonance: [2, 1], radiusM: 117_000_000 }],
    });
    expect(population(0xe000)).toMatchObject({
      kind: "belt",
      host: { kind: "star", bodyIndex: 0 },
      site: "inside_giant",
      main: { innerEdgeM: 308_900_000_000, outerEdgeM: 490_500_000_000 },
      scattered: null,
      members: { state: "ok", value: [`${FIXTURE_SYSTEM}.e001`] },
    });
    expect(population(0xe100)).toMatchObject({
      kind: "cometary_halo",
      host: { kind: "barycentre" },
      comets: 750_000_000_000,
      cometRatePerS: 3.45e-7,
    });
    expect(population(0x0100)).toEqual({ state: "not_applicable" });
  });

  it("refuses a population whose edges are reversed", () => {
    const reversed = populatedBodiesWith((body) =>
      body.population.state === "ok" && body.population.value.type === "ring"
        ? {
            ...body,
            population: { state: "ok", value: { ...body.population.value, inner_edge_m: 2e8 } },
          }
        : body,
    );

    expect(faultOf(reversed)).toBe("ring edges unusable");
  });

  it("reads a ring's plane's normal", () => {
    expect(ringNormalOf(populatedBodies())).toEqual(FIXTURE_RING_NORMAL);
    expect(ringNormalOf(populatedBodiesWith(withRingNormal([0, -0.6, 0.8])))).toEqual(
      vec3(0, -0.6, 0.8),
    );
  });

  it("reads a ring's plane's normal as none where an older server leaves it out", () => {
    expect(ringNormalOf(populatedBodiesWith(withoutRingNormal))).toBeNull();
  });

  it("holds the fixture's ring normal as A b's pole at the slice Earth's obliquity", () => {
    const { pole } = tiltedPole(populatedBodies(), `${FIXTURE_SYSTEM}.0100`, 0.409);
    const error = Math.max(
      Math.abs(pole.x - FIXTURE_RING_NORMAL.x),
      Math.abs(pole.y - FIXTURE_RING_NORMAL.y),
      Math.abs(pole.z - FIXTURE_RING_NORMAL.z),
    );

    expect(error).toBeLessThanOrEqual(1e-12);
  });

  it("refuses a ring normal that is not of unit length", () => {
    expect(faultOf(populatedBodiesWith(withRingNormal([0, -0.6, 0.9])))).toBe(
      "ring normal unusable",
    );
    expect(faultOf(populatedBodiesWith(withRingNormal([0, Number.NaN, 1])))).toBe(
      "ring normal unusable",
    );
  });

  it("refuses a belt about a star the system does not have", () => {
    const stray = populatedBodiesWith((body) =>
      body.population.state === "ok" && body.population.value.type === "belt"
        ? {
            ...body,
            population: {
              state: "ok",
              value: { ...body.population.value, host: { type: "star", body_index: 3 } },
            },
          }
        : body,
    );

    expect(faultOf(stray)).toBe("body 57344 host unknown");
  });

  it("reads an evolving orbit's drift with its rates", () => {
    const earth = bodiesOf(withDrift(DRIFT)).bodies.bodies[0];

    expect(earth?.orbit).toMatchObject({
      state: "ok",
      value: {
        orbit: {
          drift: {
            reference: { seconds: 0, nanos: 0 },
            semiMajorAxisRateMPerS: 2.59e-9,
            eccentricityRatePerS: 0,
            meanMotionRateRadPerS2: -1.27e-22,
          },
        },
      },
    });
    expect(bodiesOf().bodies.bodies[0]?.orbit).not.toHaveProperty("value.orbit.drift");
  });

  it("refuses a drift with a rate that is not finite or a malformed reference", () => {
    expect(faultOf(withDrift({ ...DRIFT, mean_motion_rate_rad_per_s2: Number.NaN }))).toBe(
      "orbit drift unusable",
    );
    expect(faultOf(withDrift({ ...DRIFT, reference: { seconds: 0.5, nanos: 0 } }))).toBe(
      "orbit drift unusable",
    );
  });

  it("refuses an orbit the client cannot propagate", () => {
    const response = sliceBodiesWith((body) =>
      body.orbit.state === "ok"
        ? {
            ...body,
            orbit: {
              state: "ok",
              value: { ...body.orbit.value, orbit: { ...body.orbit.value.orbit, eccentricity: 1 } },
            },
          }
        : body,
    );

    expect(faultOf(response)).toBe("orbit unusable");
  });

  it("reads a giant's effective temperature beside its equilibrium one", () => {
    const jupiter = bodiesOf().bodies.bodies[1];

    expect(jupiter?.bulk).toMatchObject({
      state: "ok",
      value: { equilibriumTemperatureK: 111.6, effectiveTemperatureK: 128.9 },
    });
  });

  it("refuses an effective temperature of 0 K or below the equilibrium one", () => {
    // Internal heat only adds to what the light gives: T_eff^4 = T_eq^4 + L_int / (4 pi R^2 sigma).
    expect(faultOf(withEffective(0))).toBe("bulk values unusable");
    expect(faultOf(withEffective(100))).toBe("bulk values unusable");
    expect(faultOf(withEffective(111.6))).toBe("no fault");
  });

  it("refuses a body of another system", () => {
    expect(
      faultOf(sliceBodiesWith((body) => ({ ...body, id: `0200080020000001${body.id.slice(16)}` }))),
    ).toBe("body 0200080020000001.0300 of another system");
  });

  it("refuses a body that orbits a star the system does not have", () => {
    expect(
      faultOf(sliceBodiesWith((body) => ({ ...body, parent: { type: "star", body_index: 3 } }))),
    ).toBe("body 768 host unknown");
  });

  it("refuses bodies that orbit each other in a circle", () => {
    const [earth, jupiter] = sliceBodies().bodies;
    const response = sliceBodiesWith((body) => ({
      ...body,
      parent: { type: "body", id: body.id === earth?.id ? (jupiter?.id ?? "") : (earth?.id ?? "") },
    }));

    expect(faultOf(response)).toBe("body 768 parents circular");
  });

  it("refuses a list out of index order", () => {
    const slice = sliceBodies();

    expect(faultOf({ ...slice, bodies: slice.bodies.toReversed() })).toBe("body list malformed");
  });
});

describe("toBodyDetail", () => {
  it("reads the Earth's whole record, its hooks not modelled", () => {
    const result = toBodyDetail(earthDetail(), FIXTURE_SYSTEM, DESIGNATION);
    if (result.kind !== "ok") {
      throw new Error(result.fault);
    }

    expect(result.detail.granted).toBe("full");
    expect(result.detail.record.designation).toBe(`${DESIGNATION} /768`);
    expect(result.detail.record.surface.state).toBe("ok");
    expect(result.detail.record.envelope).toEqual({ state: "not_applicable" });
    expect(result.detail.record.hooks).toEqual({ state: "not_modelled" });
  });

  it("keeps what a lower detail level withholds as not resolved", () => {
    const massAndOrbit = toBodyDetail(earthMassAndOrbit(), FIXTURE_SYSTEM, DESIGNATION);
    const contact = toBodyDetail(earthContact(), FIXTURE_SYSTEM, DESIGNATION);
    if (massAndOrbit.kind !== "ok" || contact.kind !== "ok") {
      throw new Error("the fixture's records are usable");
    }

    expect(massAndOrbit.detail.record.bulk).toEqual({ state: "not_resolved" });
    expect(massAndOrbit.detail.record.orbit.state).toBe("ok");
    expect(contact.detail.record.kind).toEqual({ kind: "unresolved" });
    expect(contact.detail.record.orbit).toEqual({ state: "not_resolved" });
  });

  it("reads a hooks section with its detail seed", () => {
    const result = earthWithDetailSeed({ state: "ok", value: "0123456789abcdef" });

    expect(result.kind === "ok" ? result.detail.record.hooks : result.fault).toEqual({
      state: "ok",
      value: { detailSeed: { state: "ok", value: "0123456789abcdef" } },
    });
  });

  it("reads a hooks section whose detail seed is not modelled", () => {
    const result = earthWithDetailSeed({ state: "not_modelled" });

    expect(result.kind === "ok" ? result.detail.record.hooks : result.fault).toEqual({
      state: "ok",
      value: { detailSeed: { state: "not_modelled" } },
    });
  });

  it("refuses a detail seed that is not 16 lower-case hexadecimal digits", () => {
    expect(earthWithDetailSeed({ state: "ok", value: "0123456789ABCDEF" })).toEqual({
      kind: "fault",
      fault: "detail seed malformed",
    });
  });

  it("gives a record's detail seed alone, for the terrain session (R10.T16.a)", () => {
    expect(bodyDetailSeed(withHooks({ state: "ok", value: "0123456789abcdef" }))).toBe(
      "0123456789abcdef",
    );
  });

  it("gives no detail seed where the hooks or the seed are not ok, or the seed is malformed", () => {
    expect([
      bodyDetailSeed(withHooks({ state: "not_modelled" })),
      bodyDetailSeed(withHooks({ state: "ok", value: "0123456789ABCDEF" })),
      bodyDetailSeed({ hooks: { state: "not_applicable" } }),
      bodyDetailSeed(earthDetail().record),
    ]).toEqual([null, null, null, null]);
  });

  it("gives a record's own orbit alone, for the terrain readout's season (R10.T12)", () => {
    const { orbit } = earthDetail().record;
    if (orbit.state !== "ok") {
      throw new Error("the fixture's Earth has an orbit");
    }
    const { orbit: elements } = orbit.value;
    expect(bodyRecordOrbit(earthDetail().record)).toEqual({
      semiMajorAxisM: elements.semi_major_axis_m,
      eccentricity: elements.eccentricity,
      inclinationRad: elements.inclination_rad,
      ascendingNodeRad: elements.ascending_node_rad,
      argumentOfPeriapsisRad: elements.argument_of_periapsis_rad,
      meanAnomalyAtEpochRad: elements.mean_anomaly_at_epoch_rad,
      periodS: elements.period_s,
    });
  });

  it("gives no orbit where the section is not ok or the orbit cannot be propagated", () => {
    const { orbit } = earthDetail().record;
    if (orbit.state !== "ok") {
      throw new Error("the fixture's Earth has an orbit");
    }
    const open = {
      state: "ok",
      value: { ...orbit.value, orbit: { ...orbit.value.orbit, eccentricity: 0.99995 } },
    } as const;
    expect([
      bodyRecordOrbit({ orbit: { state: "not_applicable" } }),
      bodyRecordOrbit({ orbit: open }),
    ]).toEqual([null, null]);
  });

  it("takes a moon's seasons from its planet, and every other body's from its own orbit", () => {
    const { record } = earthDetail();
    const planet = `${FIXTURE_SYSTEM}.0100`;
    const moonOf = (parent: (typeof record)["parent"]) => ({
      ...record,
      kind: { type: "moon", origin: "captured" } as const,
      parent,
    });
    expect([
      bodyRecordSeasons(moonOf({ type: "body", id: planet })),
      bodyRecordSeasons({ ...record, parent: { type: "body", id: planet } }),
      bodyRecordSeasons(moonOf({ type: "star", body_index: 0 })),
      bodyRecordSeasons({ ...record, orbit: { state: "not_applicable" } }),
    ]).toEqual([
      { kind: "planet", planet },
      { kind: "own", orbit: bodyRecordOrbit(record) },
      { kind: "own", orbit: bodyRecordOrbit(record) },
      { kind: "none" },
    ]);
  });

  it("refuses a record of another system", () => {
    expect(toBodyDetail(earthDetail(), "0200080020000001", DESIGNATION)).toEqual({
      kind: "fault",
      fault: `body ${FIXTURE_EARTH} of another system`,
    });
  });
});

/** The fixture's Earth's surface, which the shared fixture carries `ok`. */
function earthSurface(): BodySurfaceDto {
  const surface = earthDetail().record.surface;
  if (surface.state !== "ok") {
    throw new Error("the fixture's Earth carries its surface");
  }
  return surface.value;
}

/** The fixture's Jupiter's envelope, which the shared fixture carries `ok`. */
function jupiterEnvelope(): BodyEnvelopeDto {
  const envelope = jupiterDetail().record.envelope;
  if (envelope?.state !== "ok") {
    throw new Error("the fixture's Jupiter carries its envelope");
  }
  return envelope.value;
}

/** The fixture's Earth with its surface `surface`, read. */
function earthWithSurface(surface: BodySurfaceDto) {
  const detail = earthDetail();
  return toBodyDetail(
    { ...detail, record: { ...detail.record, surface: { state: "ok", value: surface } } },
    FIXTURE_SYSTEM,
    DESIGNATION,
  );
}

/** The fixture's Jupiter with its envelope `envelope`, read. */
function jupiterWithEnvelope(envelope: BodyEnvelopeDto) {
  const detail = jupiterDetail();
  return toBodyDetail(
    { ...detail, record: { ...detail.record, envelope: { state: "ok", value: envelope } } },
    FIXTURE_SYSTEM,
    DESIGNATION,
  );
}

/** The value of a read record's section that must be `ok`. */
function okSection<T>(section: Section<T>): T {
  if (section.state !== "ok") {
    throw new Error(`expected an ok section, got ${section.state}`);
  }
  return section.value;
}

/** The record a `body_detail` answer reads as, which must be usable. */
function recordOf(result: ReturnType<typeof toBodyDetail>) {
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return result.detail.record;
}

/** A water cloud's droplets, illustrative, as a mode of an aerosol inventory, every member modelled. */
const DROPLETS: AerosolModeDto = {
  material: "H2O",
  phase: "liquid",
  deck: "H2O",
  shape: "sphere",
  size_distribution: { type: "gamma", effective_radius_m: 1e-5, effective_variance: 0.1 },
  column_mass_kg_m2: 0.05,
  tau_550: 7.5,
  tau_basis: "sphere",
  base_pa: 85_000,
  top_pa: 60_000,
  f_sed: 1,
  coverage: 0.67,
  temperature_k: 281.5,
};

/** The water deck {@link DROPLETS} belong to, illustrative. */
const WATER_DECK: CondensateDeckDto = {
  condensate: "H2O",
  phase: "liquid",
  base_pa: 85_000,
  base_temperature_k: 281.5,
  top_pa: 60_000,
  vapour_mole_fraction_below: 0.0131,
  vapour_mole_fraction_above: 3e-6,
  f_sed: 1,
  extrapolated: false,
};

/**
 * A Titan-like haze of fractal aggregates, illustrative: its τ(550) a rule's, its column mass and
 * temperature not modelled.
 */
const HAZE: AerosolModeDto = {
  material: "tholin",
  phase: "solid",
  deck: null,
  shape: "aggregate",
  size_distribution: {
    type: "aggregate",
    monomer_radius_m: 5e-8,
    monomer_count: 3_000,
    fractal_dimension: 2,
    fractal_prefactor: 1.6666666666666667,
  },
  tau_550: 0.4,
  tau_basis: "rule",
  base_pa: 20_000,
  top_pa: 10,
  f_sed: 0,
  coverage: 1,
  not_modelled: { type: "no_mass_extinction" },
};

const { not_modelled: _hazeReason, ...HAZE_WITHOUT_REASON } = HAZE;
const { tau_basis: _dropletsBasis, ...DROPLETS_WITHOUT_BASIS } = DROPLETS;
const { tau_550: _dropletsDepth, ...DROPLETS_WITHOUT_DEPTH } = DROPLETS;
const { tau_550: _depth, tau_basis: _basis, ...DROPLETS_WITHOUT_OPTICS } = DROPLETS;
const { size_distribution: _dropletsSize, ...DROPLETS_WITHOUT_OPTICS_OR_SIZE } =
  DROPLETS_WITHOUT_OPTICS;

/** A surface's inventory of the modes `modes` over the water deck, with no absorber. */
function inventoryOf(modes: ReadonlyArray<AerosolModeDto>): AerosolInventoryDto {
  return { modes: [...modes], decks: [WATER_DECK], absorbers: [], not_formed: [] };
}

/** The read inventory of the Earth whose surface's inventory is `inventory`, which must be usable. */
function inventoryRead(inventory: AerosolInventoryDto): AerosolInventory {
  const { aerosols } = okSection(
    recordOf(earthWithSurface({ ...earthSurface(), aerosols: inventory })).surface,
  );
  if (aerosols.kind !== "value") {
    throw new Error(`expected an inventory, got ${aerosols.kind}`);
  }
  return aerosols.value;
}

/** The mode `mode` reads as, the one mode of an inventory over the water deck. */
function modeRead(mode: AerosolModeDto): AerosolMode {
  const [read] = inventoryRead(inventoryOf([mode])).modes;
  if (read === undefined) {
    throw new Error("the inventory reads its mode");
  }
  return read;
}

/** Earth's ozone on a profile of three nodes, illustrative. */
const OZONE: AbsorberDto = {
  species: "O3",
  column_m2: 8.06e22,
  base_pa: 30_000,
  top_pa: 30,
  profile: [
    { pressure_pa: 30_000, relative_mixing_ratio: 0.05 },
    { pressure_pa: 1_000, relative_mixing_ratio: 1 },
    { pressure_pa: 30, relative_mixing_ratio: 0.2 },
  ],
};

/** Europa's ocean as P14.T17.b's flag gives it, illustrative. */
const EUROPA_OCEAN: SubsurfaceOceanDto = {
  shell_thickness_m: 12_800,
  thickness_m: 124_200,
  floor: "rock",
  liquid: "H2O",
  solute_mass_fraction: 0,
  base_temperature_k: 273.15,
};

/** An Earth's vertical structure, illustrative: its moist α and two nodes. */
const EARTH_STRUCTURE: VerticalStructureDto = {
  alpha: 0.6,
  skin_temperature_k: 214.5,
  adiabat: [
    { pressure_pa: 101_325, temperature_k: 287.9 },
    { pressure_pa: 14_000, temperature_k: 214.5 },
  ],
};

/** An Earth's ocean as a surface condensate, illustrative: its mass and its 2.6 km layer. */
const OCEAN: SurfaceCondensateDto = {
  substance: "H2O",
  phase: "liquid",
  reservoir_kg_m2: 2_652_000,
  reservoir_gel_m: 2_600,
  area_fraction: 0.71,
  seasonal: false,
};

/** A seasonal cap of carbon dioxide ice, illustrative: 320 kg m⁻², a 0.2 m layer. */
const DRY_ICE: SurfaceCondensateDto = {
  substance: "CO2",
  phase: "solid",
  reservoir_kg_m2: 320,
  reservoir_gel_m: 0.2,
  area_fraction: 0.02,
  seasonal: true,
};

/** A seasonal argon frost, illustrative, with no layer: the registry has not read its density. */
const ARGON_FROST: SurfaceCondensateDto = {
  substance: "Ar",
  phase: "solid",
  reservoir_kg_m2: 15,
  area_fraction: 0.01,
  seasonal: true,
};

/** The fault the Earth reads as with its surface changed by `change`. */
function surfaceFault(change: Partial<BodySurfaceDto>): string {
  const result = earthWithSurface({ ...earthSurface(), ...change });
  return result.kind === "fault" ? result.fault : "no fault";
}

/** A surface's condensates, which must be on the record. */
function condensatesIn(
  condensates: Modelled<ReadonlyArray<SurfaceCondensate>>,
): ReadonlyArray<SurfaceCondensate> {
  if (condensates.kind !== "value") {
    throw new Error(`expected condensates, got ${condensates.kind}`);
  }
  return condensates.value;
}

/** The condensates the Earth reads as with its surface's list `condensates`, which must be usable. */
function condensatesOf(
  condensates: ReadonlyArray<SurfaceCondensateDto>,
): ReadonlyArray<SurfaceCondensate> {
  return condensatesIn(
    okSection(
      recordOf(earthWithSurface({ ...earthSurface(), condensates: [...condensates] })).surface,
    ).condensates,
  );
}

/** The fault the Jupiter reads as with its envelope changed by `change`. */
function envelopeFault(change: Partial<BodyEnvelopeDto>): string {
  const result = jupiterWithEnvelope({ ...jupiterEnvelope(), ...change });
  return result.kind === "fault" ? result.fault : "no fault";
}

describe("the surface section and the gas envelope (P14.T35.e)", () => {
  it("reads the Earth's surface in the display's units, every substance by its key", () => {
    const surface = okSection(
      recordOf(toBodyDetail(earthDetail(), FIXTURE_SYSTEM, DESIGNATION)).surface,
    );

    expect(surface.state).toBe("temperate");
    expect(surface.material).toBe("rock");
    expect(surface.meanSurfaceTemperatureK).toBe(287.9);
    expect(surface.surfacePressurePa).toBe(101_325);
    expect(surface.gases.map((gas) => gas.species)).toEqual(["N2", "H2O", "Ar", "CO2"]);
    expect(surface.gases[0]?.moleFraction).toBe(0.9805);
    expect(surface.climateRegime).toEqual({
      thermal: "efficient",
      forcing: "seasonal",
      condensable: "H2O",
      model: "energy_balance",
    });
    expect(surface.surfaceLiquids).toEqual([{ substance: "H2O", areaFraction: 0.71 }]);
    expect(surface.tectonicRegime).toBe("mobile_lid");
    expect(surface.craters.screening).toEqual({ kind: "atmosphere", columnMassKgM2: 10_318 });
    expect(surface.wetEpoch?.liquid).toBe("H2O");
  });

  it("reads every member of a later task left out as not modelled, never as none", () => {
    const surface = okSection(
      recordOf(toBodyDetail(earthDetail(), FIXTURE_SYSTEM, DESIGNATION)).surface,
    );
    const later = [
      surface.subsurfaceOcean,
      surface.verticalStructure,
      surface.aerosols,
      surface.modifiers,
      surface.biosphere,
      surface.silicateVapour,
    ];

    expect(later).toEqual(Array.from({ length: later.length }, () => ({ kind: "not_modelled" })));
  });

  it("reads the Earth's crust as the server sends it, its melt area 0 (P14.T54.a)", () => {
    const surface = okSection(
      recordOf(toBodyDetail(earthDetail(), FIXTURE_SYSTEM, DESIGNATION)).surface,
    );

    expect(surface.crust).toEqual({
      kind: "value",
      value: {
        primary: "ultramafic",
        secondary: "basalt",
        tertiary: "granite",
        bulk: null,
        lid: "secondary",
        provinces: "basalt",
        redox: "oxidised",
        mantleRedoxIw: 3.55,
        meltAreaFraction: 0,
      },
    });
  });

  it("reads a locked lava world's crust, whose dayside pool holds melt (P14.T54.a)", () => {
    const surface = okSection(
      recordOf(toBodyDetail(lavaWorldDetail(), FIXTURE_SYSTEM, DESIGNATION)).surface,
    );
    if (surface.crust.kind !== "value") {
      throw new Error(`expected the lava world's crust, got ${surface.crust.kind}`);
    }

    expect(surface.state).toBe("airless");
    expect(surface.crust.value.meltAreaFraction).toBeGreaterThan(0);
    expect(surface.silicateVapour).toEqual({ kind: "not_modelled" });
  });

  it("reads a Titan's lakes as its one condensate, a ch4_c2h6_n2 liquid, and its surface liquid (P14.T54.a)", () => {
    const surface = okSection(
      recordOf(toBodyDetail(titanDetail(), FIXTURE_SYSTEM, DESIGNATION)).surface,
    );
    const lakes = condensatesIn(surface.condensates);

    expect(lakes.map(({ substance, phase }) => [substance, phase])).toEqual([
      ["ch4_c2h6_n2", "liquid"],
    ]);
    expect(lakes[0]?.areaFraction).toBe(0.01);
    expect(surface.surfaceLiquids).toEqual([{ substance: "ch4_c2h6_n2", areaFraction: 0.01 }]);
  });

  it("reads a Titan's crust as an H2O lid whose melt area is 0 (P14.T54.a)", () => {
    const surface = okSection(
      recordOf(toBodyDetail(titanDetail(), FIXTURE_SYSTEM, DESIGNATION)).surface,
    );
    if (surface.crust.kind !== "value") {
      throw new Error(`expected the Titan's crust, got ${surface.crust.kind}`);
    }

    expect(surface.crust.value.primary).toBe("H2O");
    expect(surface.crust.value.lid).toBe("primary");
    expect(surface.crust.value.meltAreaFraction).toBe(0);
  });

  it("reads the Earth's condensates in the display's units, in the server's order (P14.T54.a)", () => {
    const surface = okSection(
      recordOf(toBodyDetail(earthDetail(), FIXTURE_SYSTEM, DESIGNATION)).surface,
    );

    expect(surface.condensates).toEqual({
      kind: "value",
      value: [
        {
          substance: "H2O",
          phase: "liquid",
          reservoirKgM2: 2_652_000,
          reservoirGelM: { kind: "value", value: 2_600 },
          areaFraction: 0.71,
          trappedAreaFraction: { kind: "not_modelled" },
          seasonal: false,
        },
        {
          substance: "H2O",
          phase: "solid",
          reservoirKgM2: 53_100,
          reservoirGelM: { kind: "value", value: 57.9 },
          areaFraction: 0.03,
          trappedAreaFraction: { kind: "value", value: 0 },
          seasonal: false,
        },
      ],
    });
  });

  it("reads a later member's null as none", () => {
    const none = okSection(
      recordOf(
        earthWithSurface({
          ...earthSurface(),
          subsurface_ocean: null,
          aerosols: null,
          silicate_vapour: null,
        }),
      ).surface,
    );

    expect([none.subsurfaceOcean, none.aerosols, none.silicateVapour]).toEqual([
      { kind: "none" },
      { kind: "none" },
      { kind: "none" },
    ]);
  });

  it("reads a newer server's silicate vapour as a value, never as a fault", () => {
    const result = earthWithSurface({
      ...earthSurface(),
      // @ts-expect-error -- SilicateVapourDto has no members until P14.T55.b, and a newer server's value must still read.
      silicate_vapour: { vapour_pressure_pa: 10 },
    });

    expect(result.kind).toBe("ok");
    expect(okSection(recordOf(result).surface).silicateVapour).toEqual({
      kind: "value",
      value: { vapour_pressure_pa: 10 },
    });
  });

  it("reads a later member's value in the display's units", () => {
    const valued = okSection(
      recordOf(
        earthWithSurface({
          ...earthSurface(),
          subsurface_ocean: EUROPA_OCEAN,
          vertical_structure: EARTH_STRUCTURE,
          aerosols: {
            modes: [DROPLETS, HAZE],
            decks: [WATER_DECK],
            absorbers: [OZONE],
            not_formed: [{ condensate: "N2", reason: { type: "bulk_gas" } }],
          },
          modifiers: {
            maturity: 0.8,
            weathering_rate: 1,
            organic_cover: 0.1,
            resurfacing_m_per_yr: 0.01,
            volcanic_deposits: [{ substance: "SO2", area_fraction: 0.27 }],
            weathering_deposits: [],
          },
        }),
      ).surface,
    );

    expect(valued.subsurfaceOcean).toEqual({
      kind: "value",
      value: {
        shellThicknessM: 12_800,
        thicknessM: 124_200,
        floor: "rock",
        liquid: "H2O",
        soluteMassFraction: 0,
        baseTemperatureK: 273.15,
      },
    });
    expect(valued.verticalStructure).toEqual({
      kind: "value",
      value: {
        alpha: 0.6,
        skinTemperatureK: 214.5,
        adiabat: [
          { pressurePa: 101_325, temperatureK: 287.9 },
          { pressurePa: 14_000, temperatureK: 214.5 },
        ],
      },
    });
    const aerosols = valued.aerosols.kind === "value" ? valued.aerosols.value : null;
    expect(
      aerosols?.modes.map((mode) =>
        mode.sizeDistribution.kind === "value" ? mode.sizeDistribution.value.kind : null,
      ),
    ).toEqual(["gamma", "aggregate"]);
    expect(aerosols?.absorbers[0]?.columnPerM2).toBe(8.06e22);
    expect(aerosols?.notFormed).toEqual([{ condensate: "N2", reason: { kind: "bulk_gas" } }]);
    expect(valued.modifiers).toEqual({
      kind: "value",
      value: {
        maturity: 0.8,
        weatheringRate: 1,
        organicCover: 0.1,
        resurfacingMPerYr: 0.01,
        volcanicDeposits: [{ substance: "SO2", areaFraction: 0.27 }],
        weatheringDeposits: [],
      },
    });
  });

  it("reads a Jupiter's envelope, its surface not applicable", () => {
    const record = recordOf(toBodyDetail(jupiterDetail(), FIXTURE_SYSTEM, DESIGNATION));
    const envelope = okSection(record.envelope);

    expect(record.surface).toEqual({ state: "not_applicable" });
    expect(envelope.composition.map((gas) => gas.species)).toEqual(["H2", "He", "CH4"]);
    expect(envelope.adiabat[1]).toEqual({ pressurePa: 100_000, temperatureK: 166.1 });
    expect(envelope.decks[0]?.deck).toBe("NH3");
    expect(envelope.profile.redistributionFactor).toBe(0.25);
  });

  it("reads an envelope's supercritical condensable by its key", () => {
    const envelope = okSection(
      recordOf(jupiterWithEnvelope({ ...jupiterEnvelope(), supercritical: "H2O" })).envelope,
    );

    expect(envelope.supercritical).toBe("H2O");
  });

  it("reads an H2/He envelope's supercritical condensable as null", () => {
    const envelope = okSection(
      recordOf(toBodyDetail(jupiterDetail(), FIXTURE_SYSTEM, DESIGNATION)).envelope,
    );

    expect(envelope.supercritical).toBeNull();
  });

  it("reads a solid planet under a supercritical envelope, its surface not applicable and its envelope not modelled", () => {
    // P14.T53.b part 2: from generator version 24 a rocky body may send the pair a giant sends.
    const record = recordOf(
      toBodyDetail(earthUnderASupercriticalEnvelope(), FIXTURE_SYSTEM, DESIGNATION),
    );

    expect([record.surface, record.envelope]).toEqual([
      { state: "not_applicable" },
      { state: "not_modelled" },
    ]);
  });

  it("reads an older server's record, which has no envelope, as its envelope not modelled", () => {
    const record = recordOf(
      toBodyDetail(earthDetailOfAnOlderServer(), FIXTURE_SYSTEM, DESIGNATION),
    );

    expect("envelope" in earthDetailOfAnOlderServer().record).toBe(false);
    expect([record.surface, record.envelope]).toEqual([
      { state: "not_modelled" },
      { state: "not_modelled" },
    ]);
  });

  it("reads the withheld envelope of a mass-and-orbit record as not resolved", () => {
    const record = recordOf(toBodyDetail(earthMassAndOrbit(), FIXTURE_SYSTEM, DESIGNATION));

    expect([record.surface, record.envelope]).toEqual([
      { state: "not_resolved" },
      { state: "not_resolved" },
    ]);
  });

  it("passes a record with a key it does not know, and a surface with a member it does not know", () => {
    // Decision-p14-t35e-wire §1.5 item 3: what a newer server adds is ignored, never refused.
    const detail = earthDetail();
    const surface = { ...earthSurface(), lightning_rate_per_s: 44 };
    const record = {
      ...detail.record,
      albedo_map: { state: "not_modelled" },
      surface: { state: "ok" as const, value: surface },
    };
    const newer: BodyRecordDto = record;

    expect(toBodyDetail({ ...detail, record: newer }, FIXTURE_SYSTEM, DESIGNATION).kind).toBe("ok");
  });

  it("passes a surface and an envelope whose every substance is a key it does not know", () => {
    const unknown = "Xe";
    const surface = earthSurface();
    const envelope = jupiterEnvelope();
    const surfaceResult = earthWithSurface({
      ...surface,
      gases: surface.gases.map((gas) => ({ species: unknown, mole_fraction: gas.mole_fraction })),
      climate_regime: { ...surface.climate_regime, condensable: unknown },
      surface_liquids: surface.surface_liquids.map((area) => ({
        substance: unknown,
        area_fraction: area.area_fraction,
      })),
      surface_ices: surface.surface_ices.map((area) => ({
        substance: unknown,
        area_fraction: area.area_fraction,
      })),
      wet_epoch: surface.wet_epoch === null ? null : { ...surface.wet_epoch, liquid: unknown },
      subsurface_ocean: { ...EUROPA_OCEAN, liquid: unknown },
      aerosols: {
        modes: [{ ...DROPLETS, material: unknown, deck: unknown }],
        decks: [{ ...WATER_DECK, condensate: unknown }],
        absorbers: [
          { species: unknown, column_m2: 1e19, base_pa: 5_000, top_pa: 1_000, profile: null },
        ],
        not_formed: [
          { condensate: unknown, reason: { type: "displaced", by: unknown } },
          { condensate: unknown, reason: { type: "bulk_gas" } },
        ],
      },
      condensates: [
        {
          substance: unknown,
          phase: "solid",
          reservoir_kg_m2: 320,
          reservoir_gel_m: 0.2,
          area_fraction: 0.02,
          seasonal: true,
        },
      ],
      crust: {
        primary: unknown,
        secondary: unknown,
        tertiary: null,
        bulk: null,
        lid: "secondary",
        provinces: unknown,
        redox: "intermediate",
        mantle_redox_iw: -1.5,
        melt_area_fraction: 0,
      },
      biosphere: { fluxes: [{ species: unknown, flux_earth: 1 }] },
    });
    const [ammonia] = envelope.decks;
    if (ammonia === undefined) {
      throw new Error("the fixture's Jupiter has its ammonia deck");
    }
    const envelopeResult = jupiterWithEnvelope({
      ...envelope,
      supercritical: unknown,
      composition: envelope.composition.map((gas) => ({
        species: unknown,
        mole_fraction: gas.mole_fraction,
      })),
      decks: [{ ...ammonia, material: unknown, deck: unknown }],
    });

    expect([surfaceResult.kind, envelopeResult.kind]).toEqual(["ok", "ok"]);
    expect(okSection(recordOf(surfaceResult).surface).gases[0]?.species).toBe(unknown);
  });

  it("refuses an aerosol mode that is a vapour or a supercritical fluid", () => {
    for (const phase of ["vapour", "supercritical"] as const) {
      const mode = { ...DROPLETS, phase };

      expect(
        earthWithSurface({
          ...earthSurface(),
          aerosols: inventoryOf([mode]),
        }),
      ).toEqual({ kind: "fault", fault: "aerosol phase unusable" });
      expect(jupiterWithEnvelope({ ...jupiterEnvelope(), decks: [mode] })).toEqual({
        kind: "fault",
        fault: "aerosol phase unusable",
      });
    }
  });

  it("refuses surface gases that do not sum to 1", () => {
    const short = earthSurface().gases.map((gas) => ({
      species: gas.species,
      mole_fraction: gas.mole_fraction / 2,
    }));

    expect(surfaceFault({ gases: short })).toBe("surface gases unusable");
  });

  it("refuses surface gases that are not largest first", () => {
    expect(surfaceFault({ gases: earthSurface().gases.toReversed() })).toBe(
      "surface gases unusable",
    );
  });

  it("refuses an envelope's composition that does not sum to 1", () => {
    expect(envelopeFault({ composition: [{ species: "H2", mole_fraction: 0.5 }] })).toBe(
      "envelope composition unusable",
    );
  });

  it.each<[string, Partial<BodySurfaceDto>]>([
    ["a mean temperature of 0 K", { mean_surface_temperature_k: 0 }],
    ["a negative day–night contrast", { day_night_contrast_k: -1 }],
    ["a negative pressure", { surface_pressure_pa: -1 }],
    ["a negative stellar flux", { stellar_flux_w_m2: -1 }],
    ["a mean molar mass of 0", { mean_molar_mass_g_per_mol: 0 }],
  ])("refuses a surface with %s", (_, change) => {
    expect(surfaceFault(change)).toBe("surface conditions unusable");
  });

  it.each<[string, Partial<BodySurfaceDto>]>([
    ["a negative σ_h", { sigma_h_m: -1 }],
    ["a negative heat flow", { heat_flow_w_m2: -0.01 }],
    ["a negative volcanism level", { volcanism_level: -1 }],
    ["a negative outgassing", { outgassing_earth: -1 }],
    ["a negative surface age", { surface_age_s: -1 }],
  ])("refuses global figures with %s", (_, change) => {
    expect(surfaceFault(change)).toBe("global figures unusable");
  });

  it("refuses a subsurface ocean of no depth", () => {
    expect(surfaceFault({ subsurface_ocean: { ...EUROPA_OCEAN, thickness_m: 0 } })).toBe(
      "subsurface ocean unusable",
    );
  });

  it("refuses a crater contract with a target of no density", () => {
    expect(surfaceFault({ craters: { ...earthSurface().craters, target_density_kg_m3: 0 } })).toBe(
      "crater contract unusable",
    );
  });

  it("refuses a crater screening by a negative column mass", () => {
    expect(
      surfaceFault({
        craters: {
          ...earthSurface().craters,
          screening: { type: "atmosphere", column_mass_kg_m2: -1 },
        },
      }),
    ).toBe("crater contract unusable");
  });

  it("refuses a wet epoch that ends before it starts", () => {
    const epoch = earthSurface().wet_epoch;
    if (epoch === null) {
      throw new Error("the fixture's Earth has its wet epoch");
    }

    expect(surfaceFault({ wet_epoch: { ...epoch, end_ago_s: epoch.start_ago_s * 2 } })).toBe(
      "wet epoch unusable",
    );
  });

  it("refuses a vertical structure whose adiabat rises in pressure", () => {
    expect(
      surfaceFault({
        vertical_structure: { ...EARTH_STRUCTURE, adiabat: EARTH_STRUCTURE.adiabat.toReversed() },
      }),
    ).toBe("vertical structure unusable");
  });

  it("refuses an aggregate above the model's fractal dimension of 2.5", () => {
    const haze: AerosolModeDto = {
      ...HAZE,
      size_distribution: {
        type: "aggregate",
        monomer_radius_m: 5e-8,
        monomer_count: 3_000,
        fractal_dimension: 2.8,
        fractal_prefactor: 1.6666666666666667,
      },
    };

    expect(surfaceFault({ aerosols: inventoryOf([haze]) })).toBe("aerosol mode unusable");
  });

  it("refuses an aerosol mode whose base lies above its top", () => {
    expect(
      surfaceFault({
        aerosols: inventoryOf([{ ...DROPLETS, base_pa: 50_000 }]),
      }),
    ).toBe("aerosol mode unusable");
  });

  it("refuses an absorber whose profile's pressures do not descend", () => {
    const profile = OZONE.profile?.toReversed() ?? null;

    expect(
      surfaceFault({
        aerosols: { modes: [], decks: [], absorbers: [{ ...OZONE, profile }], not_formed: [] },
      }),
    ).toBe("absorber unusable");
  });

  it.each([
    [
      "a vapour below that is not a fraction",
      { vapour_mole_fraction_below: 1.5 },
      "condensate deck unusable",
    ],
    [
      "a vapour above that is not a fraction",
      { vapour_mole_fraction_above: 2 },
      "condensate deck unusable",
    ],
    ["a base above its top", { base_pa: 50_000 }, "condensate deck unusable"],
    ["a base of no temperature", { base_temperature_k: 0 }, "condensate deck unusable"],
    ["a negative f_sed", { f_sed: -1 }, "condensate deck unusable"],
    ["the vapour phase", { phase: "vapour" }, "aerosol phase unusable"],
    ["the supercritical phase", { phase: "supercritical" }, "aerosol phase unusable"],
  ] satisfies ReadonlyArray<[string, Partial<CondensateDeckDto>, string]>)(
    "refuses a condensate deck with %s",
    (_, change, fault) => {
      expect(surfaceFault({ aerosols: inventoryOf([]) })).toBe("no fault");
      expect(
        surfaceFault({
          aerosols: { ...inventoryOf([]), decks: [{ ...WATER_DECK, ...change }] },
        }),
      ).toBe(fault);
    },
  );

  it("refuses an inventory with two decks of one condensate (R08's S3)", () => {
    expect(
      surfaceFault({
        aerosols: { ...inventoryOf([]), decks: [WATER_DECK, { ...WATER_DECK, top_pa: 70_000 }] },
      }),
    ).toBe("condensate deck unusable");
  });

  it("reads a modelled aerosol mode's coverage, temperature and optical depth with its basis (P14.T54.b.b)", () => {
    expect(modeRead(DROPLETS)).toEqual({
      material: "H2O",
      phase: "liquid",
      deck: "H2O",
      shape: "sphere",
      sizeDistribution: {
        kind: "value",
        value: { kind: "gamma", effectiveRadiusM: 1e-5, effectiveVariance: 0.1 },
      },
      columnMassKgM2: { kind: "value", value: 0.05 },
      opticalDepth: { kind: "value", value: { tau550: 7.5, basis: "sphere" } },
      basePa: 85_000,
      topPa: 60_000,
      fSed: 1,
      coverage: 0.67,
      temperatureK: { kind: "value", value: 281.5 },
      notModelled: null,
    });
  });

  it("reads an aggregate's mode with its rule's optical depth, and no column mass or temperature", () => {
    const haze = modeRead(HAZE);

    expect(haze.notModelled).toEqual({ kind: "no_mass_extinction" });
    expect(haze.opticalDepth).toEqual({ kind: "value", value: { tau550: 0.4, basis: "rule" } });
    expect([haze.columnMassKgM2, haze.temperatureK]).toEqual([
      { kind: "not_modelled" },
      { kind: "not_modelled" },
    ]);
  });

  it("reads an inventory's condensate deck with its phase, base, top, vapours, f_sed and extrapolation", () => {
    expect(inventoryRead(inventoryOf([DROPLETS])).decks).toEqual([
      {
        condensate: "H2O",
        phase: "liquid",
        basePa: 85_000,
        baseTemperatureK: 281.5,
        topPa: 60_000,
        vapourMoleFractionBelow: 0.0131,
        vapourMoleFractionAbove: 3e-6,
        fSed: 1,
        extrapolated: false,
      },
    ]);
  });

  it("reads a condensable displaced by another, with the condensate that displaces it", () => {
    const inventory = inventoryRead({
      ...inventoryOf([]),
      not_formed: [{ condensate: "SiO2", reason: { type: "displaced", by: "MgSiO3" } }],
    });

    expect(inventory.notFormed).toEqual([
      { condensate: "SiO2", reason: { kind: "displaced", by: "MgSiO3" } },
    ]);
  });

  it("reads a condensable past its critical point as not formed, labelled supercritical (P14.T52.f)", () => {
    const inventory = inventoryRead({
      ...inventoryOf([]),
      not_formed: [
        { condensate: "CH4", reason: { type: "bulk_gas" } },
        { condensate: "N2", reason: { type: "supercritical" } },
      ],
    });

    expect(inventory.notFormed).toEqual([
      { condensate: "CH4", reason: { kind: "bulk_gas" } },
      { condensate: "N2", reason: { kind: "supercritical" } },
    ]);
  });

  it("reads a deck's mode whose size is not modelled for want of a grain density, with its phase and temperature", () => {
    expect(
      modeRead({
        ...DROPLETS_WITHOUT_OPTICS_OR_SIZE,
        not_modelled: { type: "no_grain_density", phase: "liquid", temperature_k: 281.5 },
      }),
    ).toEqual({
      material: "H2O",
      phase: "liquid",
      deck: "H2O",
      shape: "sphere",
      sizeDistribution: { kind: "not_modelled" },
      columnMassKgM2: { kind: "value", value: 0.05 },
      opticalDepth: { kind: "not_modelled" },
      basePa: 85_000,
      topPa: 60_000,
      fSed: 1,
      coverage: 0.67,
      temperatureK: { kind: "value", value: 281.5 },
      notModelled: { kind: "no_grain_density", phase: "liquid", temperatureK: 281.5 },
    });
  });

  it("reads an aerosol mode's member sent as null as not modelled, as the server reads it", () => {
    const mode = modeRead({
      ...DROPLETS_WITHOUT_OPTICS,
      not_modelled: { type: "optics_not_modelled" },
      // @ts-expect-error -- the binding leaves an absent member out, but the server reads null as absent, and an older or hand-written frame may send it.
      column_mass_kg_m2: null,
    });

    expect(mode.columnMassKgM2).toEqual({ kind: "not_modelled" });
  });

  it("refuses a grain density's reason in the vapour phase", () => {
    expect(
      surfaceFault({
        aerosols: inventoryOf([
          {
            ...DROPLETS_WITHOUT_OPTICS,
            not_modelled: { type: "no_grain_density", phase: "vapour", temperature_k: 281.5 },
          },
        ]),
      }),
    ).toBe("aerosol phase unusable");
  });

  it.each([
    ["no reason for its absent column mass", HAZE_WITHOUT_REASON],
    [
      "a reason with every member present",
      { ...DROPLETS, not_modelled: { type: "no_condensate" } },
    ],
    ["an optical depth with no basis", DROPLETS_WITHOUT_BASIS],
    [
      "a basis with no optical depth",
      { ...DROPLETS_WITHOUT_DEPTH, not_modelled: { type: "optics_not_modelled" } },
    ],
    ["an aggregate's reason beside a temperature", { ...HAZE, temperature_k: 150 }],
    [
      "a size's reason beside a size",
      { ...DROPLETS_WITHOUT_OPTICS, not_modelled: { type: "no_size_distribution" } },
    ],
    ["a coverage beyond the sky", { ...DROPLETS, coverage: 1.5 }],
    ["a temperature of 0 K", { ...DROPLETS, temperature_k: 0 }],
    [
      "a grain density's reason of no temperature",
      {
        ...DROPLETS_WITHOUT_OPTICS,
        not_modelled: { type: "no_grain_density", phase: "liquid", temperature_k: 0 },
      },
    ],
    [
      "a grain density's reason beside an optical depth",
      {
        ...DROPLETS,
        not_modelled: { type: "no_grain_density", phase: "liquid", temperature_k: 281.5 },
      },
    ],
    [
      "the optics' reason with no size",
      {
        ...DROPLETS_WITHOUT_OPTICS_OR_SIZE,
        not_modelled: { type: "optics_not_modelled" },
      },
    ],
  ] satisfies ReadonlyArray<[string, AerosolModeDto]>)(
    "refuses an aerosol mode with %s (P14.T54.b.b)",
    (_, mode) => {
      expect(surfaceFault({ aerosols: inventoryOf([mode]) })).toBe("aerosol mode unusable");
    },
  );

  it("refuses an aerosol mode whose deck the inventory does not list", () => {
    expect(surfaceFault({ aerosols: inventoryOf([{ ...DROPLETS, deck: "NH3" }]) })).toBe(
      "aerosol mode unusable",
    );
    expect(surfaceFault({ aerosols: inventoryOf([DROPLETS, HAZE]) })).toBe("no fault");
  });

  it("refuses a surface condensate that is a vapour", () => {
    expect(surfaceFault({ condensates: [{ ...DRY_ICE, phase: "vapour" }] })).toBe(
      "condensate unusable",
    );
  });

  it("reads a condensate's layer where the server sends one (P14.T54.a)", () => {
    const [ocean] = condensatesOf([OCEAN]);

    expect(ocean?.reservoirGelM).toEqual({ kind: "value", value: 2_600 });
  });

  it("reads a condensate's absent layer as not modelled, never as none (P14.T54.a)", () => {
    const [frost] = condensatesOf([ARGON_FROST]);

    expect(frost?.reservoirGelM).toEqual({ kind: "not_modelled" });
  });

  it("reads a condensate's layer sent as null as not modelled, as the server reads it", () => {
    const [frost] = condensatesOf([
      {
        ...ARGON_FROST,
        // @ts-expect-error -- the binding leaves an absent layer out, but the server reads null as absent, and an older or hand-written frame may send it.
        reservoir_gel_m: null,
      },
    ]);

    expect(frost?.reservoirGelM).toEqual({ kind: "not_modelled" });
  });

  it("reads a condensate's trapped area where the server sends one (P14.T54.b)", () => {
    const [ice] = condensatesOf([{ ...DRY_ICE, trapped_area_fraction: 0.005 }]);

    expect(ice?.trappedAreaFraction).toEqual({ kind: "value", value: 0.005 });
  });

  it("reads a condensate's absent trapped area as not modelled, never as none", () => {
    const [ice] = condensatesOf([DRY_ICE]);

    expect(ice?.trappedAreaFraction).toEqual({ kind: "not_modelled" });
  });

  it("reads a condensate's trapped area sent as null as not modelled, as the server reads it", () => {
    const [ice] = condensatesOf([
      {
        ...DRY_ICE,
        // @ts-expect-error -- the binding leaves an absent trapped area out, but the server reads null as absent, and an older or hand-written frame may send it.
        trapped_area_fraction: null,
      },
    ]);

    expect(ice?.trappedAreaFraction).toEqual({ kind: "not_modelled" });
  });

  it.each<[string, SurfaceCondensateDto]>([
    ["whose mass is negative", { ...DRY_ICE, reservoir_kg_m2: -320 }],
    ["whose layer is negative", { ...DRY_ICE, reservoir_gel_m: -0.2 }],
    ["whose trapped area is larger than its area", { ...DRY_ICE, trapped_area_fraction: 0.03 }],
    ["whose trapped area is negative", { ...DRY_ICE, trapped_area_fraction: -0.005 }],
    ["whose trapped area is not finite", { ...DRY_ICE, trapped_area_fraction: Number.NaN }],
  ])("refuses a surface condensate %s", (_, condensate) => {
    expect(surfaceFault({ condensates: [condensate] })).toBe("condensate unusable");
  });

  it.each<[string, Partial<SurfaceModifiersDto>]>([
    ["whose maturity is not a fraction", { maturity: 1.5 }],
    ["whose weathering rate is negative", { weathering_rate: -1 }],
    [
      "whose deposits cover more than the surface",
      {
        volcanic_deposits: [
          { substance: "SO2", area_fraction: 0.6 },
          { substance: "S8", area_fraction: 0.6 },
        ],
      },
    ],
    [
      "whose deposit's share is negative, which only the deposit's own check catches",
      { volcanic_deposits: [{ substance: "SO2", area_fraction: -0.2 }] },
    ],
    [
      "whose weathering deposit's share is negative",
      { weathering_deposits: [{ substance: "ferric", area_fraction: -0.2 }] },
    ],
    [
      "whose volcanic and weathering deposits together cover more than the surface",
      {
        volcanic_deposits: [{ substance: "SO2", area_fraction: 0.8 }],
        weathering_deposits: [{ substance: "ferric", area_fraction: 0.3 }],
      },
    ],
  ])("refuses surface modifiers %s", (_, change) => {
    const modifiers: SurfaceModifiersDto = {
      maturity: 0.8,
      weathering_rate: 1,
      organic_cover: 0,
      resurfacing_m_per_yr: 0,
      volcanic_deposits: [],
      weathering_deposits: [],
      ...change,
    };
    expect(surfaceFault({ modifiers })).toBe("surface modifiers unusable");
  });

  it("accepts surface modifiers whose deposits sum to 1 within the tolerance", () => {
    const modifiers: SurfaceModifiersDto = {
      maturity: 0.8,
      weathering_rate: 1,
      organic_cover: 0,
      resurfacing_m_per_yr: 0,
      volcanic_deposits: [
        { substance: "SO2", area_fraction: 0.4 },
        { substance: "S8", area_fraction: 0.6 + 5e-10 },
      ],
      weathering_deposits: [],
    };
    const surface = okSection(recordOf(earthWithSurface({ ...earthSurface(), modifiers })).surface);

    expect(surface.modifiers).toEqual({
      kind: "value",
      value: {
        maturity: 0.8,
        weatheringRate: 1,
        organicCover: 0,
        resurfacingMPerYr: 0,
        volcanicDeposits: [
          { substance: "SO2", areaFraction: 0.4 },
          { substance: "S8", areaFraction: 0.6 + 5e-10 },
        ],
        weatheringDeposits: [],
      },
    });
  });

  it("reads an oxidised surface's ferric deposit (plan 14's P14.T51.j)", () => {
    const modifiers: SurfaceModifiersDto = {
      maturity: 0,
      weathering_rate: 2.2e-248,
      organic_cover: 0,
      resurfacing_m_per_yr: 0,
      volcanic_deposits: [],
      weathering_deposits: [{ substance: "ferric", area_fraction: 0.3069 }],
    };
    const surface = okSection(recordOf(earthWithSurface({ ...earthSurface(), modifiers })).surface);

    expect(surface.modifiers).toEqual({
      kind: "value",
      value: {
        maturity: 0,
        weatheringRate: 2.2e-248,
        organicCover: 0,
        resurfacingMPerYr: 0,
        volcanicDeposits: [],
        weatheringDeposits: [{ substance: "ferric", areaFraction: 0.3069 }],
      },
    });
  });

  it("refuses a biosphere flux that is not finite", () => {
    expect(
      surfaceFault({ biosphere: { fluxes: [{ species: "O2", flux_earth: Number.NaN }] } }),
    ).toBe("biosphere unusable");
  });

  it.each<[string, Partial<BodyEnvelopeDto>]>([
    ["a negative internal temperature", { internal_temperature_k: -1 }],
    ["a join at no pressure", { join_pressure_pa: 0 }],
    ["an adiabat starting above its join", { join_pressure_pa: 1e6 }],
    ["a mean molar mass of 0", { mean_molar_mass_g_per_mol: 0 }],
    ["a negative haze", { haze_tau_550: -1 }],
  ])("refuses an envelope with %s", (_, change) => {
    expect(envelopeFault(change)).toBe("envelope unusable");
  });

  it("reads an airless body's empty gases with no mixture figures", () => {
    const airless = okSection(
      recordOf(
        earthWithSurface({
          ...earthSurface(),
          state: "airless",
          surface_pressure_pa: 0,
          gases: [],
          mean_molar_mass_g_per_mol: null,
          heat_capacity_over_r: null,
        }),
      ).surface,
    );

    expect([airless.gases, airless.meanMolarMassGPerMol, airless.heatCapacityOverR]).toEqual([
      [],
      null,
      null,
    ]);
  });

  it("reads a tenuous body's air: its pressure, its gases and their mixture figures", () => {
    // P14.T53.c's `tenuous` state's air members, as the server sends them from 24 (P14.T54.b.h):
    // a Pluto-like 1 Pa of nitrogen with a trace of methane, on the fixture's other members.
    const tenuous = okSection(
      recordOf(
        earthWithSurface({
          ...earthSurface(),
          state: "tenuous",
          surface_pressure_pa: 1,
          gases: [
            { species: "N2", mole_fraction: 0.995 },
            { species: "CH4", mole_fraction: 0.005 },
          ],
          mean_molar_mass_g_per_mol: 27.95,
          heat_capacity_over_r: 3.5,
        }),
      ).surface,
    );

    expect(tenuous.state).toBe("tenuous");
    expect(tenuous.gases).toEqual([
      { species: "N2", moleFraction: 0.995 },
      { species: "CH4", moleFraction: 0.005 },
    ]);
    expect([
      tenuous.surfacePressurePa,
      tenuous.meanMolarMassGPerMol,
      tenuous.heatCapacityOverR,
    ]).toEqual([1, 27.95, 3.5]);
  });

  it("refuses a cloud fraction above 1", () => {
    expect(surfaceFault({ cloud_fraction: 1.2 })).toBe("surface cover unusable");
  });

  it("refuses liquids that do not sum to the ocean fraction", () => {
    expect(surfaceFault({ ocean_fraction: 0.5 })).toBe("surface cover unusable");
  });

  it("refuses a crust whose lid names an absent crust", () => {
    expect(
      surfaceFault({
        crust: {
          primary: null,
          secondary: "basalt",
          tertiary: null,
          bulk: null,
          lid: "primary",
          provinces: "basalt",
          redox: "intermediate",
          mantle_redox_iw: -1.5,
          melt_area_fraction: 0,
        },
      }),
    ).toBe("crust unusable");
  });

  it("refuses an envelope whose adiabat does not rise in pressure from its join", () => {
    expect(envelopeFault({ adiabat: jupiterEnvelope().adiabat.toReversed() })).toBe(
      "envelope unusable",
    );
  });
});

/** A golden system's `system_bodies` answer as the server sent it, with its kind. */
function goldenAnswer(
  system: (typeof GOLDEN_SYSTEMS)[number],
  when: (typeof GOLDEN_TIMES)[number],
) {
  return { kind: "system_bodies" as const, ...goldenSystemBodies(system, when) };
}

describe("body-state times beyond 2^53 s (P14.T35.d)", () => {
  // About -6.3 Gyr, which a JavaScript number holds to 32 s.
  const LONG_AGO_S = -199_097_968_544_446_944;

  it("reads every frame of the RM1 fixture's three golden systems", () => {
    for (const system of GOLDEN_SYSTEMS) {
      for (const when of GOLDEN_TIMES) {
        expect(faultOf(goldenAnswer(system, when)), `${system} ${when}`).toBe("no fault");
      }
    }
  });

  it("keeps the wide binary's moon unbound 2.1 Gyr before the epoch", () => {
    const { bodies } = bodiesOf(goldenAnswer("wide_binary", "epoch"));
    const moon = bodies.bodies.find((body) => body.id === "41ffecae00000004.0401");

    expect(moon?.state).toEqual({
      kind: "unbound",
      at: { seconds: -67_192_088_818_403_264, nanos: 0 },
    });
  });

  it("reads an unbinding or a destruction at any whole number of seconds", () => {
    const at = { seconds: LONG_AGO_S, nanos: 0 };

    expect(faultOf(withFirstState({ type: "unbound", at }))).toBe("no fault");
    expect(faultOf(withFirstState({ type: "destroyed", cause: "engulfed", at }))).toBe("no fault");
  });

  it("refuses a state time that is not whole seconds and nanoseconds", () => {
    expect(faultOf(withFirstState({ type: "unbound", at: { seconds: 0.5, nanos: 0 } }))).toBe(
      "state time unusable",
    );
    expect(faultOf(withFirstState({ type: "unbound", at: { seconds: 0, nanos: 1e9 } }))).toBe(
      "state time unusable",
    );
  });

  it("still refuses an orbit's valid_until beyond 2^53 s, which the client computes with", () => {
    expect(faultOf(withValidUntil({ seconds: -LONG_AGO_S, nanos: 0 }))).toBe("orbit time unusable");
  });
});

/** The slice's answer with its Earth's summary changed by `change`. */
function withEarth(change: (earth: BodySummaryDto) => BodySummaryDto) {
  return sliceBodiesWith((body) => (body.id === FIXTURE_EARTH ? change(body) : body));
}

/** The value of an `ok` section of the slice's Earth, as the fixture gives it. */
function earthValue<K extends "rotation" | "figure" | "photometry">(
  key: K,
): Extract<NonNullable<BodySummaryDto[K]>, { state: "ok" }>["value"] {
  const section = sliceBodies().bodies.find((body) => body.id === FIXTURE_EARTH)?.[key];
  if (section?.state !== "ok") {
    throw new Error(`the fixture's Earth has an ok ${key} section`);
  }
  return section.value;
}

/** The slice's answer with its Earth's rotation law changed by `change`. */
function withRotation(change: Partial<BodyRotationDto>) {
  return withEarth((earth) => ({
    ...earth,
    rotation: { state: "ok", value: { ...earthValue("rotation"), ...change } },
  }));
}

/** The slice's answer with its Earth's figure changed by `change`. */
function withFigure(change: Partial<BodyFigureDto>) {
  return withEarth((earth) => ({
    ...earth,
    figure: { state: "ok", value: { ...earthValue("figure"), ...change } },
  }));
}

/** The slice's answer with its Earth's photometry changed by `change`. */
function withPhotometry(change: Partial<BodyPhotometryDto>) {
  return withEarth((earth) => ({
    ...earth,
    photometry: { state: "ok", value: { ...earthValue("photometry"), ...change } },
  }));
}

function earthOf(response = sliceBodies()) {
  const earth = bodiesOf(response).bodies.bodies.find((body) => body.id === FIXTURE_EARTH);
  if (earth === undefined) {
    throw new Error("the slice's answer lists its Earth");
  }
  return earth;
}

describe("the rotation, figure and photometry sections (P14.T46.f, P14.T47.d; R07.T2.b)", () => {
  it("reads the Earth's rotation law in the client's units", () => {
    expect(earthOf().rotation).toEqual({
      state: "ok",
      value: {
        pole: { x: 0, y: -0.39769200800981236, z: 0.9175189735177814 },
        equatorNode: { x: 1, y: 0, z: 0 },
        equatorQuarter: { x: 0, y: 0.9175189735177814, z: 0.39769200800981236 },
        obliquityRad: 0.409,
        initialRateRadPerS: 7.292115e-5,
        lockedRateRadPerS: 1.9909866e-7,
        ageAtEpochS: 1.44e17,
        lock: { kind: "never" },
        resonance: "synchronous",
        clockPeriodS: earthValue("rotation").clock_period_s,
        clockMeanAnomalyAtEpochRad: earthValue("rotation").clock_mean_anomaly_at_epoch_rad,
        subPrimaryAngleRad: earthValue("rotation").sub_primary_angle_rad,
        phaseAtEpochRad: earthValue("rotation").phase_at_epoch_rad,
        capturePhaseRad: earthValue("rotation").capture_phase_rad,
      },
    });
  });

  it("reads the Earth's figure, its spheroid about its pole", () => {
    expect(earthOf().figure).toEqual({
      state: "ok",
      value: {
        equatorialRadiusM: 6_378_137,
        polarRadiusM: 6_356_752.314245179,
        flattening: 0.0033528106647474805,
        pole: { x: 0, y: -0.39769200800981236, z: 0.9175189735177814 },
        momentOfInertiaFactor: 0.33,
        law: "rotational",
        datum: "solid_surface",
      },
    });
  });

  it("reads the Earth's photometry per band, with its stated ratio", () => {
    expect(earthOf().photometry).toEqual({
      state: "ok",
      value: {
        geometricAlbedo: { b: 0.263, v: 0.215, r: 0.21 },
        phaseTemplate: "earth",
        phaseExponent: { b: 1, v: 1, r: 1 },
        lunarLambertShare: 0,
        bondAlbedo: 0.294,
        bondRatio: 0.95914,
        provisional: false,
      },
    });
  });

  it("reads the sections a record carries, and what a lower level withholds", () => {
    const full = toBodyDetail(earthDetail(), FIXTURE_SYSTEM, DESIGNATION);
    const massAndOrbit = toBodyDetail(earthMassAndOrbit(), FIXTURE_SYSTEM, DESIGNATION);
    if (full.kind !== "ok" || massAndOrbit.kind !== "ok") {
      throw new Error("the fixture's records are usable");
    }

    expect(
      [full, massAndOrbit].map(({ detail: { record } }) => [
        record.rotation.state,
        record.figure.state,
        record.photometry.state,
      ]),
    ).toEqual([
      ["ok", "ok", "ok"],
      ["not_resolved", "not_resolved", "not_resolved"],
    ]);
  });

  it("reads a summary without the fields, an older server's, as not modelled", () => {
    const jupiter = bodiesOf().bodies.bodies.find((body) => body.id !== FIXTURE_EARTH);

    expect([jupiter?.rotation, jupiter?.figure, jupiter?.photometry]).toEqual([
      { state: "not_modelled" },
      { state: "not_modelled" },
      { state: "not_modelled" },
    ]);
  });

  it.each(["not_resolved", "not_modelled", "not_applicable"] as const)(
    "keeps each section tagged %s as the server tagged it",
    (state) => {
      const earth = earthOf(
        withEarth((body) => ({
          ...body,
          rotation: { state },
          figure: { state },
          photometry: { state },
        })),
      );

      expect([earth.rotation, earth.figure, earth.photometry]).toEqual([
        { state },
        { state },
        { state },
      ]);
    },
  );

  it("reads a lock far outside the clock window, which it only compares", () => {
    const lockedLongAgo = withRotation({
      locking_age_s: 1.5e17,
      locks_at: { seconds: -59_362_396_298_040_352, nanos: 0 },
    });

    expect(faultOf(lockedLongAgo)).toBe("no fault");
  });

  const badAxes: ReadonlyArray<readonly [string, Partial<BodyRotationDto>]> = [
    ["a node off the equator", { equator_node: [1, 1e-12, 0] }],
    ["a left-handed triad", { equator_quarter: [0, -0.9175189735177814, -0.39769200800981236] }],
    ["a pole of 0", { pole: [0, 0, 0] }],
  ];
  it.each(badAxes)("refuses %s", (_name, change) => {
    expect(faultOf(withRotation(change))).toBe("rotation axes unusable");
  });

  const badLaws: ReadonlyArray<readonly [string, Partial<BodyRotationDto>]> = [
    ["a clock period of 0", { clock_period_s: 0 }],
    ["a spin rate below 0", { initial_rate_rad_s: -7.292115e-5 }],
    ["an obliquity past π", { obliquity_rad: 4 }],
    ["a phase that is not finite", { phase_at_epoch_rad: Number.NaN }],
  ];
  it.each(badLaws)("refuses a rotation law with %s", (_name, change) => {
    expect(faultOf(withRotation(change))).toBe("rotation law unusable");
  });

  it("reads a body that never locks, one that locks beyond the clock, and one within it", () => {
    const at = { seconds: 946_728_000, nanos: 0 };
    const lockOf = (change: Partial<BodyRotationDto>) => {
      const rotation = earthOf(withRotation(change)).rotation;
      return rotation.state === "ok" ? rotation.value.lock : null;
    };

    expect([
      lockOf({}),
      lockOf({ locking_age_s: 2e17 }),
      lockOf({ locking_age_s: 1.44e17 + 946_728_000, locks_at: at }),
    ]).toEqual([
      { kind: "never" },
      { kind: "outside_clock", lockingAgeS: 2e17 },
      { kind: "in_clock", lockingAgeS: 1.44e17 + 946_728_000, locksAt: at },
    ]);
  });

  it("refuses a locking age of 0", () => {
    expect(faultOf(withRotation({ locking_age_s: 0 }))).toBe("rotation law unusable");
  });

  it("refuses a lock in the window without a locking age", () => {
    expect(faultOf(withRotation({ locks_at: { seconds: 1e9, nanos: 0 } }))).toBe(
      "rotation lock unusable",
    );
  });

  const badFigures: ReadonlyArray<readonly [string, Partial<BodyFigureDto>]> = [
    ["a polar radius above the equatorial", { polar_radius_m: 6_400_000 }],
    ["a radius of 0", { equatorial_radius_m: 0 }],
    ["a pole that is not unit", { pole: [0, 0, 2] }],
    ["a moment of inertia factor above a uniform sphere's", { moment_of_inertia_factor: 0.5 }],
  ];
  it.each(badFigures)("refuses a figure with %s", (_name, change) => {
    expect(faultOf(withFigure(change))).toBe("figure unusable");
  });

  const badPhotometry: ReadonlyArray<readonly [string, Partial<BodyPhotometryDto>]> = [
    ["a geometric albedo of 0", { geometric_albedo: { b: 0.263, v: 0, r: 0.21 } }],
    ["an exponent below 0", { phase_exponent: { b: 1, v: -1, r: 1 } }],
    ["a Lommel–Seeliger share above 1", { lunar_lambert_share: 1.5 }],
    ["a Bond albedo of 1", { bond_albedo: 1 }],
    ["a stated ratio that is not finite", { bond_ratio: Number.POSITIVE_INFINITY }],
  ];
  it.each(badPhotometry)("refuses a photometry with %s", (_name, change) => {
    expect(faultOf(withPhotometry(change))).toBe("photometry unusable");
  });
});
