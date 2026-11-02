/**
 * The medium of a body from its record (plan R08, R08.T10.a): the labels by section, the key, the
 * gravity by figure law, the column on the record's vertical structure (R08.T3.e), the seams the
 * cache and the tables take, and the types held to theirs.
 * The generated Earth's medium with its line tables is `assembleEarth.test.ts`'s; aerosols are
 * `assembleAerosols.test.ts`'s; the router at the assembly is `assembleThick.test.ts`'s.
 */
import type { CrustCompositionDto, SystemIdHex } from "@hyperion/protocol";
import { describe, expect, expectTypeOf, it } from "vitest";

import type { BodySurface } from "../../lib/system/model";
import { spinRateAt } from "../../lib/system/rotation";
import {
  detailOf,
  EMPTY_INVENTORY,
  recordInput,
  SUN_LIGHT,
  withFigureLaw,
  withInventory,
  withRecord,
  withSurface,
  type DetailAnswer,
} from "../../test/atmosphereRecords";
import {
  earthContact,
  earthDetail,
  earthDetailOfAnOlderServer,
  earthMassAndOrbit,
  jupiterDetail,
  lavaWorldDetail,
  TEMPERATE_WORLD_SYSTEM,
  temperateWorldDetail,
} from "../../test/planetaryFixture";
import { type GoldenStructure, goldenStructure } from "../../test/verticalStructureGolden";
import { MAX_BODY_LIGHTS } from "../lighting/hostLights";
import {
  type AssembledBody,
  type AssembledMedium,
  assembleMedium,
  ATMOSPHERE_OPTICS_VERSION,
  atmosphereKey,
  type CurveSlots,
  type DrawnMedium,
  LIGHT_SLOT_LIMIT,
  lineSpeciesOf,
  MEDIUM_TERM_LIMIT,
  mediumTableRadiusM,
  THICK_REASON,
  withCurves,
} from "./assemble";
import { type AtmosphereColumn, hydrostaticColumn, type TemperatureProfile } from "./column";
import { type SpheroidFigure, tableRadiusM } from "./hillaire";
import { type DensityProfile, densityAt } from "./medium";
import type { GasFractions } from "./rayleigh";
import { type AbsorberCurveSlots, type AtmosphereBody, MAX_TERMS } from "./tables";

/** The assembly of a fixture answer of `system`'s, the slice's unless given, lit by the Sun. */
function assembled(answer: DetailAnswer, system?: SystemIdHex): AssembledMedium {
  return assembleMedium(recordInput(detailOf(answer, system)));
}

/** An answer's crust with its melt area set. */
function withMeltArea(answer: DetailAnswer, meltAreaFraction: number): DetailAnswer {
  return withSurface(answer, (surface) => {
    const crust: CrustCompositionDto | null | undefined = surface.crust;
    if (crust === undefined || crust === null) {
      throw new Error("the fixture's surface has no crust");
    }
    return { ...surface, crust: { ...crust, melt_area_fraction: meltAreaFraction } };
  });
}

/** The generated Earth's key, lit by the Sun. */
function earthKey(): string {
  return atmosphereKey(recordInput(detailOf(earthDetail())));
}

/**
 * The generated Earth on a figure law at 20 times its spin, past its break-up rate
 * √(GM ÷ a³) = 1.24 × 10⁻³ rad s⁻¹.
 */
function fastEarth(law: "rotational" | "capped"): DetailAnswer {
  return withRecord(withFigureLaw(earthDetail(), law), (record) => {
    const rotation = record.rotation;
    if (rotation?.state !== "ok") {
      throw new Error("the fixture Earth has no rotation");
    }
    const rate = 20 * rotation.value.initial_rate_rad_s;
    return {
      ...record,
      rotation: { state: "ok", value: { ...rotation.value, initial_rate_rad_s: rate } },
    };
  });
}

/** Jupiter's answer with its envelope section as given, or left out. */
function jupiterWithEnvelope(envelope: "absent" | "not_modelled"): DetailAnswer {
  return withRecord(jupiterDetail(), (record) => {
    const { envelope: _envelope, ...rest } = record;
    return envelope === "absent" ? rest : { ...rest, envelope: { state: "not_modelled" } };
  });
}

describe("a body's labels by the section its air comes from", () => {
  it("draws no air and says NOT RESOLVED where the surface section is withheld", () => {
    for (const answer of [earthMassAndOrbit(), earthContact()]) {
      const result = assembled(answer);
      expect([result.medium, result.labels]).toEqual([undefined, ["atmosphereNotResolved"]]);
    }
  });

  it("draws no air and says NOT YET MODELLED where the surface section is not modelled", () => {
    const result = assembled(earthDetailOfAnOlderServer());
    expect([result.medium, result.labels]).toEqual([undefined, ["atmosphereNotYetModelled"]]);
  });

  it("says NOT YET MODELLED for a giant whose record has no envelope", () => {
    const result = assembled(jupiterWithEnvelope("absent"));
    expect([result.medium, result.labels]).toEqual([undefined, ["atmosphereNotYetModelled"]]);
  });

  it("says NOT YET MODELLED for a sub-Neptune whose envelope is not modelled, as a giant's", () => {
    const result = assembled(jupiterWithEnvelope("not_modelled"));
    expect([result.medium, result.labels]).toEqual([undefined, ["atmosphereNotYetModelled"]]);
  });

  it("draws no envelope sent ok and keeps NOT YET MODELLED until envelopes are drawn", () => {
    const result = assembled(jupiterDetail());
    expect([result.medium, result.labels]).toEqual([undefined, ["atmosphereNotYetModelled"]]);
  });

  it("gives a generated airless body the exosphere's note alone", () => {
    const result = assembled(withMeltArea(lavaWorldDetail(), 0));
    expect([result.medium, result.labels]).toEqual([undefined, ["exosphereNotYetModelled"]]);
  });

  it("gives a kept scene's body set airless no medium and no label", () => {
    const result = assembleMedium({ kind: "kept", atmosphere: { kind: "airless" } });
    expect([result.medium, result.labels]).toEqual([undefined, []]);
  });

  it("gives a kept scene's body with no atmosphere set NOT YET MODELLED", () => {
    const result = assembleMedium({ kind: "kept", atmosphere: { kind: "unset" } });
    expect([result.medium, result.labels]).toEqual([undefined, ["atmosphereNotYetModelled"]]);
  });

  it("adds SILICATE VAPOUR beside the exosphere's note where an airless body holds a pool of melt", () => {
    const result = assembled(lavaWorldDetail());
    expect([result.medium, result.labels]).toEqual([
      undefined,
      ["exosphereNotYetModelled", "silicateVapourNotYetModelled"],
    ]);
  });

  it("draws an air over melt", () => {
    expect(assembled(withMeltArea(earthDetail(), 0.1)).medium).toBeDefined();
  });

  it("gives an air over melt SILICATE VAPOUR beside its other notes", () => {
    expect(assembled(withMeltArea(earthDetail(), 0.1)).labels).toEqual([
      "aerosolsNotYetModelled",
      "silicateVapourNotYetModelled",
      "atmosphereApproximate",
    ]);
  });

  it("gives the drawn medium its column's temperature, a temperate world's under the thermal cut (R08.T18.b)", () => {
    const result = assembled(earthDetail());
    if (result.medium === undefined) {
      throw new Error("Earth's air is drawn");
    }
    const temperature = result.drawn.temperature;
    const surface = detailOf(earthDetail()).record.surface;
    if (temperature === undefined || surface.state !== "ok") {
      throw new Error("the drawn medium holds its column's temperature");
    }
    expect(new Set(temperature.temperaturesK)).toEqual(
      new Set([surface.value.meanSurfaceTemperatureK]),
    );
    expect(temperature.heightsM[0]).toBe(0);
    expect(result.regime.selfLuminous).toBeUndefined();
  });

  it("makes a world of 2,600 K selfLuminous, its air's own light above the cut (R08.T18.b)", () => {
    const hot = withSurface(earthDetail(), (surface) => ({
      ...surface,
      mean_surface_temperature_k: 2_600,
    }));
    const result = assembled(hot);
    expect(result.medium === undefined ? undefined : result.regime.selfLuminous).toBe(true);
  });

  it("draws no air for a melt with no volatile air, and says SILICATE VAPOUR alone", () => {
    const melt = withSurface(earthDetail(), (surface) => ({
      ...surface,
      state: "magma_ocean",
      surface_pressure_pa: 0,
      gases: [],
      mean_molar_mass_g_per_mol: null,
      heat_capacity_over_r: null,
    }));
    const result = assembled(melt);
    expect([result.medium, result.labels]).toEqual([undefined, ["silicateVapourNotYetModelled"]]);
  });

  it("gives an absent inventory no aerosol term", () => {
    expect(
      assembled(earthDetail()).medium?.terms.some((term) => term.name.startsWith("aerosol")),
    ).toBe(false);
  });

  it("gives an absent inventory AEROSOLS AND ABSORBERS: NOT YET MODELLED", () => {
    expect(assembled(earthDetail()).labels).toContain("aerosolsNotYetModelled");
  });

  it("gives an inventory computed empty no AEROSOLS note", () => {
    const result = assembled(withInventory(earthDetail(), EMPTY_INVENTORY));
    expect(result.labels.includes("aerosolsNotYetModelled")).toBe(false);
  });

  it("carries no AEROSOLS note under NOT RESOLVED or NOT YET MODELLED", () => {
    for (const answer of [earthMassAndOrbit(), earthDetailOfAnOlderServer(), jupiterDetail()]) {
      expect(assembled(answer).labels.includes("aerosolsNotYetModelled")).toBe(false);
    }
  });
});

/**
 * The generated Earth on N₂ 97%, SO₂ 1%, atomic S 1%, Ar 0.9%, H₂O 0.07% and CO₂ 0.03%: SO₂'s
 * Rayleigh scattering is estimated (`rayleigh.ts`'s `ESTIMATED_RAYLEIGH`), and S has no Rayleigh
 * row in the registry.
 */
function withEstimatedAndUnscatteringGases(): DetailAnswer {
  return withSurface(earthDetail(), (surface) => ({
    ...surface,
    gases: [
      { species: "N2", mole_fraction: 0.97 },
      { species: "SO2", mole_fraction: 0.01 },
      { species: "S", mole_fraction: 0.01 },
      { species: "Ar", mole_fraction: 0.009 },
      { species: "H2O", mole_fraction: 0.0007 },
      { species: "CO2", mole_fraction: 0.0003 },
    ],
  }));
}

describe("the gases' reasons", () => {
  it("say a gas's scattering is estimated", () => {
    expect(assembled(withEstimatedAndUnscatteringGases()).reasons).toContain(
      "SO2: its scattering estimated, not measured",
    );
  });

  it("say a gas the registry gives no Rayleigh row scatters nothing drawn", () => {
    expect(assembled(withEstimatedAndUnscatteringGases()).reasons).toContain(
      "S: no scattering data on file, its scattering not drawn",
    );
  });

  it("say a line species drawn without its tables absorbs nothing drawn", () => {
    expect(assembled(earthDetail()).reasons).toContain(
      "H2O: no absorption data on file, its absorption not drawn",
    );
  });

  it("give e− and the ions, whose optics other terms draw, no scattering reason", () => {
    const electrons = withSurface(earthDetail(), (surface) => ({
      ...surface,
      gases: [
        { species: "N2", mole_fraction: 0.98 },
        { species: "H2O", mole_fraction: 0.0102 },
        { species: "Ar", mole_fraction: 0.009 },
        { species: "e-", mole_fraction: 0.0005 },
        { species: "Na+", mole_fraction: 0.0003 },
      ],
    }));
    // The multiple-scattering reason is the air's, past the router's boundary (R08.T13.d), not a gas's.
    expect(
      assembled(electrons).reasons.filter(
        (reason) => reason.includes("scattering") && reason !== THICK_REASON,
      ),
    ).toEqual([]);
  });
});

describe("the key the cache holds a medium under", () => {
  it("is the same for the same inputs", () => {
    expect(earthKey()).toBe(earthKey());
  });

  it("changes with the inventory", () => {
    expect(
      atmosphereKey(recordInput(detailOf(withInventory(earthDetail(), EMPTY_INVENTORY)))),
    ).not.toBe(atmosphereKey(recordInput(detailOf(earthDetail()))));
  });

  it("changes with the lights", () => {
    expect(atmosphereKey(recordInput(detailOf(earthDetail()), []))).not.toBe(earthKey());
  });

  it("changes with the ground", () => {
    expect(
      atmosphereKey(recordInput(detailOf(earthDetail()), [SUN_LIGHT], [0.31, 0.3, 0.3])),
    ).not.toBe(earthKey());
  });

  it("stays at a later time with the same spin rate", () => {
    const detail = detailOf(earthDetail());
    const later = { ...detail, time: { seconds: detail.time.seconds + 86_400, nanos: 0 } };
    expect(atmosphereKey(recordInput(later))).toBe(atmosphereKey(recordInput(detail)));
  });

  it("names the body, the optics' version and a 64-bit digest", () => {
    expect(atmosphereKey(recordInput(detailOf(earthDetail())))).toMatch(
      new RegExp(`^0200080020000000\\.0300/v${ATMOSPHERE_OPTICS_VERSION}/[0-9a-f]{16}$`, "u"),
    );
  });

  it("keys on the optics' version 6, raised last by a mode's τ taken over the whole sky", () => {
    expect(ATMOSPHERE_OPTICS_VERSION).toBe(6);
  });

  it.each(["unset", "airless"] as const)(
    "is kept/%s/v<version> for a kept scene's body so set",
    (kind) => {
      expect(atmosphereKey({ kind: "kept", atmosphere: { kind } })).toBe(
        `kept/${kind}/v${ATMOSPHERE_OPTICS_VERSION}`,
      );
    },
  );
});

/**
 * The generated Earth's answer under a structure of the sim's golden: its surface's p_s and T_s the
 * structure's, and, unless `structure` is `"not_modelled"`, the structure on the wire.
 */
function withGoldenAir(structure: GoldenStructure, sent: "sent" | "not_modelled"): DetailAnswer {
  return withSurface(earthDetail(), (surface) => ({
    ...surface,
    surface_pressure_pa: structure.surfacePa,
    mean_surface_temperature_k: structure.surfaceK,
    ...(sent === "sent"
      ? {
          vertical_structure: {
            alpha: structure.alpha,
            skin_temperature_k: structure.skinK,
            adiabat: structure.nodes.map(({ pressurePa, temperatureK }) => ({
              pressure_pa: pressurePa,
              temperature_k: temperatureK,
            })),
          },
        }
      : {}),
  }));
}

/** An assembly's drawn medium and its molecular term's density, which the fixture's air has. */
function drawnOf(
  result: AssembledMedium,
): DrawnMedium & { readonly molecularDensity: DensityProfile } {
  const molecularDensity = result.medium?.terms[0]?.density;
  if (result.drawn === undefined || molecularDensity === undefined) {
    throw new Error("the fixture's air draws a medium");
  }
  return { ...result.drawn, molecularDensity };
}

/**
 * The column an assembly's surface should take, at the drawn body's g_ref and R_ref, of an answer
 * of `system`'s, the slice's unless given.
 */
function expectedColumn(
  answer: DetailAnswer,
  temperature: TemperatureProfile,
  system?: SystemIdHex,
): AtmosphereColumn {
  const surface = detailOf(answer, system).record.surface;
  if (surface.state !== "ok" || surface.value.meanMolarMassGPerMol === null) {
    throw new Error("the fixture's surface has air");
  }
  const drawn = drawnOf(assembled(answer, system));
  return hydrostaticColumn({
    surfacePa: surface.value.surfacePressurePa,
    temperature,
    meanMolarMassGPerMol: surface.value.meanMolarMassGPerMol,
    referenceGravityMS2: drawn.body.gravity.referenceGravityMS2,
    referenceRadiusM: drawn.bottomRadiusM,
  });
}

describe("a body's column on its published vertical structure (R08.T3.e)", () => {
  const earth = goldenStructure("earth");
  const nodes = { kind: "nodes", nodes: earth.nodes, skinK: earth.skinK } as const;

  it("lays the column, its thermal temperature and the molecular term on the structure's nodes", () => {
    const answer = withGoldenAir(earth, "sent");
    const drawn = drawnOf(assembled(answer));
    const column = expectedColumn(answer, nodes);
    expect([drawn.temperature, drawn.molecularDensity]).toEqual([
      { heightsM: column.altitudesM, temperaturesK: column.temperaturesK },
      column.density,
    ]);
    expect([column.temperaturesK[0], column.temperaturesK.at(-1)]).toEqual([
      earth.surfaceK,
      earth.skinK,
    ]);
  });

  it("lays the generated temperate world's column on its own structure's nodes, from its p_s and T_s", () => {
    // The fixture's generated message (R08.T19's fifth row), whose structure is the record's own.
    const answer = temperateWorldDetail();
    const { surface } = detailOf(answer, TEMPERATE_WORLD_SYSTEM).record;
    if (surface.state !== "ok" || surface.value.verticalStructure.kind !== "value") {
      throw new Error("the generated surface carries its vertical structure");
    }
    const { adiabat, skinTemperatureK } = surface.value.verticalStructure.value;
    const drawn = drawnOf(assembled(answer, TEMPERATE_WORLD_SYSTEM));
    const column = expectedColumn(
      answer,
      { kind: "nodes", nodes: adiabat, skinK: skinTemperatureK },
      TEMPERATE_WORLD_SYSTEM,
    );
    expect([drawn.temperature, drawn.molecularDensity]).toEqual([
      { heightsM: column.altitudesM, temperaturesK: column.temperaturesK },
      column.density,
    ]);
    expect([column.pressuresPa[0], column.temperaturesK[0], column.temperaturesK.at(-1)]).toEqual([
      surface.value.surfacePressurePa,
      surface.value.meanSurfaceTemperatureK,
      skinTemperatureK,
    ]);
  });

  it("draws the golden Earth's air 1.8 times thinner at 30 km and 4.0 at 50 km than the seam", () => {
    // The seam's 290 K against the skin's 216 K above the tropopause near 13 km.
    const seam = drawnOf(assembled(withGoldenAir(earth, "not_modelled")));
    const drawn = drawnOf(assembled(withGoldenAir(earth, "sent")));
    const ratioAt = (heightM: number): number =>
      densityAt(seam.molecularDensity, heightM) / densityAt(drawn.molecularDensity, heightM);
    expect([ratioAt(30e3), ratioAt(50e3)].map((r) => Math.round(r * 10) / 10)).toEqual([1.8, 4]);
  });

  it("takes the absorbers' temperatures from the structure's colder column", () => {
    const [seam] = drawnOf(assembled(withGoldenAir(earth, "not_modelled"))).absorbers;
    const [drawn] = drawnOf(assembled(withGoldenAir(earth, "sent"))).absorbers;
    expect([seam?.name, drawn?.name]).toEqual(["pair:H2O+H2O,H2O+air", "pair:H2O+H2O,H2O+air"]);
    expect(Math.abs((seam?.temperatureK ?? Number.NaN) / earth.surfaceK - 1)).toBeLessThan(1e-12);
    expect(drawn?.temperatureK).toBeLessThan(earth.surfaceK - 1);
    expect(drawn?.temperatureK).toBeGreaterThan(earth.skinK);
  });

  it("keeps the isothermal seam at T_s, bit for bit, where the structure is not modelled", () => {
    const answer = earthDetail();
    const drawn = drawnOf(assembled(answer));
    const column = expectedColumn(answer, { kind: "isothermal", temperatureK: 287.9 });
    expect([drawn.temperature, drawn.molecularDensity]).toEqual([
      { heightsM: column.altitudesM, temperaturesK: column.temperaturesK },
      column.density,
    ]);
    expect(column.temperaturesK.every((t) => t === 287.9)).toBe(true);
  });

  it.each<[string, DetailAnswer, RegExp]>([
    [
      "a structure whose first node is not the section's p_s",
      withSurface(withGoldenAir(earth, "sent"), (surface) => ({
        ...surface,
        surface_pressure_pa: 101_325,
      })),
      /starts at its surface's/u,
    ],
    [
      "a structure whose first node is not the section's T_s",
      withSurface(withGoldenAir(earth, "sent"), (surface) => ({
        ...surface,
        mean_surface_temperature_k: 287.9,
      })),
      /starts at its surface's/u,
    ],
    [
      "a structure whose last node is not at its skin's temperature",
      withSurface(withGoldenAir(earth, "sent"), (surface) => {
        const structure = surface.vertical_structure;
        if (structure === undefined || structure === null) {
          throw new Error("the answer sends its structure");
        }
        return {
          ...surface,
          vertical_structure: { ...structure, skin_temperature_k: earth.skinK - 1 },
        };
      }),
      /ends at its skin's/u,
    ],
    [
      "a structure that is none under an air",
      withSurface(earthDetail(), (surface) => ({ ...surface, vertical_structure: null })),
      /is not none/u,
    ],
  ])("refuses %s with a RangeError", (_, answer, message) => {
    expect(() => assembled(answer)).toThrow(RangeError);
    expect(() => assembled(answer)).toThrow(message);
  });
});

describe("a body's gravity by its figure law (Design note 17)", () => {
  const tidal = detailOf(withFigureLaw(earthDetail(), "rotational_and_tidal"));
  const tidalOmega = (): number => {
    const rotation = tidal.record.rotation;
    if (rotation.state !== "ok") {
      throw new Error("the tidal Earth has no rotation");
    }
    return spinRateAt(rotation.value, tidal.time);
  };
  const tidalBody = (): AssembledBody => {
    const { drawn } = assembleMedium(recordInput(tidal));
    if (drawn === undefined) {
      throw new Error("the tidal Earth draws no medium");
    }
    return drawn.body;
  };

  it("builds a rotational_and_tidal figure at √2.5 ω", () => {
    const spin = tidalBody().gravity.spheroid?.angularVelocityRadS ?? Number.NaN;
    expect(Math.abs(spin / (Math.sqrt(2.5) * tidalOmega()) - 1)).toBeLessThan(1e-12);
  });

  it("adds ω²R, R = ∛(a²c), to a rotational_and_tidal figure's reference gravity", () => {
    const body = tidalBody();
    const { equatorialRadiusM: a, polarRadiusM: c } = body.figure;
    const omega = tidalOmega();
    expect(
      Math.abs(body.gravity.gravityOffsetMS2 / (omega * omega * Math.cbrt(a * a * c)) - 1),
    ).toBeLessThan(1e-12);
  });

  it("refuses a rotational figure spun past break-up, where γ_e is not above 0", () => {
    expect(() => assembled(fastEarth("rotational"))).toThrow(RangeError);
  });

  it("builds a capped figure at the same spin, past break-up", () => {
    expect(assembled(fastEarth("capped")).medium).toBeDefined();
  });

  it("lays its tables on (2a + c) ÷ 3, hillaire.ts's R_ref", () => {
    const { drawn } = assembled(earthDetail());
    const figure: SpheroidFigure = drawn?.body.figure ?? { equatorialRadiusM: 0, polarRadiusM: 0 };
    expect([drawn?.bottomRadiusM, mediumTableRadiusM(figure)]).toEqual([
      tableRadiusM(figure),
      tableRadiusM(figure),
    ]);
  });
});

describe("the assembly's seams", () => {
  it("passes the record's gases to the molecular term unconverted", () => {
    expectTypeOf<BodySurface["gases"]>().toExtend<GasFractions>();
  });

  it("gives the tables' body and curve slots their types", () => {
    expectTypeOf<AssembledBody>().toExtend<AtmosphereBody>();
    expectTypeOf<CurveSlots>().toEqualTypeOf<AbsorberCurveSlots>();
  });

  it("holds the tables' term limit and R07's light slots", () => {
    expect([MEDIUM_TERM_LIMIT, LIGHT_SLOT_LIMIT]).toEqual([MAX_TERMS, MAX_BODY_LIGHTS]);
  });

  it("lists the line species the worker loads: those the record lists with committed tables", () => {
    // N₂'s hot table among them, though N₂ is transparent below its 1,800 K (R08.T4.f).
    expect(lineSpeciesOf(recordInput(detailOf(earthDetail())))).toEqual(["CO2", "H2O", "N2"]);
  });

  it("lists none for a body that draws no air", () => {
    expect(lineSpeciesOf(recordInput(detailOf(jupiterDetail())))).toEqual([]);
  });

  it("gives a result that survives the structured clone a worker's message takes", () => {
    const result = assembled(earthDetail());
    expect(structuredClone(result)).toEqual(result);
  });

  it("refuses curves that are not one slot a light", () => {
    const result = assembled(earthDetail());
    expect(() => withCurves(result, [[], []])).toThrow(RangeError);
  });

  it("refuses more lights than the medium's slots", () => {
    const detail = detailOf(earthDetail());
    expect(() => assembleMedium(recordInput(detail, [SUN_LIGHT, SUN_LIGHT, SUN_LIGHT]))).toThrow(
      RangeError,
    );
  });

  it("clamps the ground's albedo to [0, 1]", () => {
    const result = assembleMedium(
      recordInput(detailOf(earthDetail()), [SUN_LIGHT], [1.2, 0.5, -0.1]),
    );
    expect(result.medium?.groundAlbedo).toEqual([1, 0.5, 0]);
  });
});
