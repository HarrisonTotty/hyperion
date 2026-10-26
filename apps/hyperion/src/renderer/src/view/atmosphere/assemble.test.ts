/**
 * The medium of a body from its record (plan R08, R08.T10.a): the labels by section, the key, the
 * gravity by figure law, the seams the cache and the tables take, and the types held to theirs.
 * The generated Earth's medium with its line tables is `assembleEarth.test.ts`'s; aerosols are
 * `assembleAerosols.test.ts`'s; the thick line is `assembleThick.test.ts`'s.
 */
import type { CrustCompositionDto } from "@hyperion/protocol";
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
} from "../../test/planetaryFixture";
import { MAX_BODY_LIGHTS } from "../lighting/hostLights";
import {
  type AssembledBody,
  type AssembledMedium,
  assembleMedium,
  atmosphereKey,
  type CurveSlots,
  LIGHT_SLOT_LIMIT,
  lineSpeciesOf,
  MEDIUM_TERM_LIMIT,
  mediumTableRadiusM,
  withCurves,
} from "./assemble";
import { type SpheroidFigure, tableRadiusM } from "./hillaire";
import type { GasFractions } from "./rayleigh";
import { type AbsorberCurveSlots, type AtmosphereBody, MAX_TERMS } from "./tables";

/** The assembly of a fixture answer, lit by the Sun. */
function assembled(answer: DetailAnswer): AssembledMedium {
  return assembleMedium(recordInput(detailOf(answer)));
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

  it("draws an air over melt with SILICATE VAPOUR beside its other notes", () => {
    const result = assembled(withMeltArea(earthDetail(), 0.1));
    expect([
      result.medium !== undefined,
      result.labels.includes("silicateVapourNotYetModelled"),
    ]).toEqual([true, true]);
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

  it("gives an absent inventory no aerosol term and AEROSOLS AND ABSORBERS: NOT YET MODELLED", () => {
    const result = assembled(earthDetail());
    expect([
      result.medium?.terms.some((term) => term.name.startsWith("aerosol")),
      result.labels.includes("aerosolsNotYetModelled"),
    ]).toEqual([false, true]);
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

describe("the key the cache holds a medium under", () => {
  it("is the same for the same inputs", () => {
    expect(earthKey()).toBe(earthKey());
  });

  it("changes with the inventory", () => {
    expect(
      atmosphereKey(recordInput(detailOf(withInventory(earthDetail(), EMPTY_INVENTORY)))),
    ).not.toBe(atmosphereKey(recordInput(detailOf(earthDetail()))));
  });

  it("changes with the lights and with the ground", () => {
    const detail = detailOf(earthDetail());
    const base = atmosphereKey(recordInput(detail));
    expect([
      atmosphereKey(recordInput(detail, [])) !== base,
      atmosphereKey(recordInput(detail, [SUN_LIGHT], [0.31, 0.3, 0.3])) !== base,
    ]).toEqual([true, true]);
  });

  it("stays at a later time with the same spin rate", () => {
    const detail = detailOf(earthDetail());
    const later = { ...detail, time: { seconds: detail.time.seconds + 86_400, nanos: 0 } };
    expect(atmosphereKey(recordInput(later))).toBe(atmosphereKey(recordInput(detail)));
  });

  it("names the body, the optics' version and a 64-bit digest", () => {
    expect(atmosphereKey(recordInput(detailOf(earthDetail())))).toMatch(
      /^0200080020000000\.0300\/v1\/[0-9a-f]{16}$/u,
    );
  });

  it("is a kept body's statement for a kept scene's body", () => {
    expect([
      atmosphereKey({ kind: "kept", atmosphere: { kind: "unset" } }),
      atmosphereKey({ kind: "kept", atmosphere: { kind: "airless" } }),
    ]).toEqual(["kept/unset/v1", "kept/airless/v1"]);
  });
});

describe("a body's gravity by its figure law (Design note 17)", () => {
  it("builds a rotational_and_tidal figure at √2.5 ω, with ω²R in its reference gravity", () => {
    const detail = detailOf(withFigureLaw(earthDetail(), "rotational_and_tidal"));
    const { drawn } = assembleMedium(recordInput(detail));
    const rotation = detail.record.rotation;
    if (drawn === undefined || rotation.state !== "ok") {
      throw new Error("the tidal Earth draws no medium, or has no rotation");
    }
    const omega = spinRateAt(rotation.value, detail.time);
    const { equatorialRadiusM: a, polarRadiusM: c } = drawn.body.figure;
    expect(
      Math.abs(
        (drawn.body.gravity.spheroid?.angularVelocityRadS ?? Number.NaN) /
          (Math.sqrt(2.5) * omega) -
          1,
      ),
    ).toBeLessThan(1e-12);
    expect(
      Math.abs(drawn.body.gravity.gravityOffsetMS2 / (omega * omega * Math.cbrt(a * a * c)) - 1),
    ).toBeLessThan(1e-12);
  });

  it("builds a capped figure at a spin past break-up, where the rotational law cannot", () => {
    expect(() => assembled(fastEarth("rotational"))).toThrow(RangeError);
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
    expect(lineSpeciesOf(recordInput(detailOf(earthDetail())))).toEqual(["CO2", "H2O"]);
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
