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

  it("refuses a record of another system", () => {
    expect(toBodyDetail(earthDetail(), "0200080020000001", DESIGNATION)).toEqual({
      kind: "fault",
      fault: `body ${FIXTURE_EARTH} of another system`,
    });
  });
});
