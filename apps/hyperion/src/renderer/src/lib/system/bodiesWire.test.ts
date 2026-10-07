import type {
  BodyFigureDto,
  BodyPhotometryDto,
  BodyRotationDto,
  BodyStateDto,
  BodySummaryDto,
  OrbitDriftDto,
  SectionDto,
  UniverseTime,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  earthContact,
  earthDetail,
  earthMassAndOrbit,
  FIXTURE_EARTH,
  FIXTURE_SYSTEM,
  populatedBodies,
  populatedBodiesWith,
  sliceBodies,
  sliceBodiesWith,
} from "../../test/planetaryFixture";
import { GOLDEN_SYSTEMS, GOLDEN_TIMES, goldenSystemBodies } from "../../test/inSystemGolden";
import {
  EARTH_MASS_KG,
  toBodiesRequest,
  toBodyDetail,
  toBodyDetailRequest,
  toSystemBodiesModel,
} from "./bodiesWire";

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
  it("reads the Earth's whole record, its surface and hooks not modelled", () => {
    const result = toBodyDetail(earthDetail(), FIXTURE_SYSTEM, DESIGNATION);
    if (result.kind !== "ok") {
      throw new Error(result.fault);
    }

    expect(result.detail.granted).toBe("full");
    expect(result.detail.record.designation).toBe(`${DESIGNATION} /768`);
    expect(result.detail.record.surface).toEqual({ state: "not_modelled" });
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

  it("refuses a record of another system", () => {
    expect(toBodyDetail(earthDetail(), "0200080020000001", DESIGNATION)).toEqual({
      kind: "fault",
      fault: `body ${FIXTURE_EARTH} of another system`,
    });
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
