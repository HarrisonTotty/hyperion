/**
 * The case writer's rules and refusals (plan R08, R08.T12.b.1 and b.2; `decision-r08-t12b-scope.md`,
 * rulings 4, 6 and 10), on small variations of the cases' own terms and on a line absorber's medium:
 * the branches the committed cases do not reach.
 */

import { describe, expect, it } from "vitest";

import type { MediumTerm, PhaseTable } from "../view/atmosphere/medium";
import { EARTH_CASE, MARS_CASE, type ReferenceCase } from "../view/atmosphere/reference/cases";
import {
  EARTH_SPECTRAL_DAY_CASE,
  MARS_SPECTRAL_LOW_CASE,
  type SpectralCaseTerm,
  type SpectralReferenceCase,
} from "../view/atmosphere/reference/spectralCases";
import {
  absorberCurveJson,
  caseJson,
  densityJson,
  roundedTermJson,
  SPECTRAL_CASE_SIGNIFICANT_FIGURES,
  spectralCaseJson,
  tabulatedPhaseJson,
  type TermJson,
  termJson,
  traceCommand,
} from "./atmosphereCases";
import { fourAbsorberMedium, lineAbsorberMedium } from "./atmosphereFixtures";
import { linearInUMoments } from "./mieMatrices";

/** A case's term by name, or a thrown error naming it. */
function termOf(terms: ReadonlyArray<MediumTerm>, name: string): MediumTerm {
  const term = terms.find((t) => t.name === name);
  if (term === undefined) {
    throw new Error(`the case has its ${name} term`);
  }
  return term;
}

const OZONE = termOf(EARTH_CASE.medium.terms, "absorber:O3");
const DUST = termOf(MARS_CASE.medium.terms, "dust");
const [OZONE_CURVE] = EARTH_CASE.curves;

/** Mars's dust table, as its term carries it. */
function dustTable(): PhaseTable {
  if (DUST.phase.kind !== "tabulated") {
    throw new Error("Mars's dust has a tabulated phase");
  }
  return DUST.phase.table;
}

const DUST_ASYMMETRY = MARS_CASE.phaseAsymmetry["dust"];

describe("the case writer's densities", () => {
  it("writes a tent's three heights", () => {
    expect(densityJson({ kind: "tent", bottomM: 10, peakM: 25, topM: 50 })).toEqual({
      kind: "tent",
      bottomM: 10,
      peakM: 25,
      topM: 50,
    });
  });

  it("drops a tabulated profile's layer edges, which the format refuses", () => {
    const profile = {
      kind: "tabulated",
      altitudesM: new Float64Array([0, 1000]),
      relative: new Float64Array([1, 0.5]),
      edgesM: [500],
    } as const;
    expect(densityJson(profile)).toEqual({
      kind: "tabulated",
      altitudesM: [0, 1000],
      relative: [1, 0.5],
    });
  });
});

describe("the case writer's phases", () => {
  it("normalises a table's entries over the sphere read linearly in u", () => {
    const json = tabulatedPhaseJson(dustTable(), [0.7, 0.7, 0.7]);
    const integrals =
      json.kind === "tabulated" ? json.a1.map((row) => linearInUMoments(json.u, row).integral) : [];
    expect(integrals).toEqual([
      expect.closeTo(1, 12),
      expect.closeTo(1, 12),
      expect.closeTo(1, 12),
    ]);
  });

  it("gives a table the asymmetry it is handed, not the table's own", () => {
    const json = tabulatedPhaseJson(dustTable(), [0.1, 0.2, 0.3]);
    expect(json.kind === "tabulated" ? json.asymmetry : undefined).toEqual([0.1, 0.2, 0.3]);
  });

  it("keeps a table's matrix, and does not mark the term depolarising", () => {
    const json = termJson(DUST, undefined, DUST_ASYMMETRY);
    expect([
      json.depolarising,
      json.phase.kind === "tabulated" && json.phase.matrix !== undefined,
    ]).toEqual([undefined, true]);
  });

  it("marks a table without a matrix depolarising", () => {
    const bare: MediumTerm = {
      ...DUST,
      phase: { kind: "tabulated", table: { ...dustTable(), matrix: undefined } },
    };
    expect(termJson(bare, undefined, DUST_ASYMMETRY).depolarising).toBe(true);
  });

  it("refuses a table whose source's asymmetry is not given", () => {
    expect(() => termJson(DUST, undefined, undefined)).toThrow(/asymmetry/u);
  });
});

describe("the case writer's absorbers", () => {
  it("writes a column absorber's density and its curve's one row", () => {
    const json = termJson(OZONE, OZONE_CURVE, undefined);
    expect([json.absorber?.kind, json.absorber?.curves.length, json.phase.kind]).toEqual([
      "column",
      1,
      "none",
    ]);
  });

  it("writes a curve of one row with no rows, as before R08.T12.b.2", () => {
    if (OZONE_CURVE === undefined) {
      throw new Error("Earth's case has its ozone's curve");
    }
    expect(Object.keys(absorberCurveJson(OZONE_CURVE))).toEqual(["logColumns", "opticalDepth"]);
  });

  it("writes a curve of several rows with their ln p, its read's top and its depths row-major", () => {
    const { curves } = lineAbsorberMedium();
    const line = curves[0]?.find((c) => c.logPressures.length > 1);
    if (line === undefined) {
      throw new Error("the line medium has a curve with pressure rows");
    }
    const json = absorberCurveJson(line);
    expect([
      json.logPressures,
      json.readTopLogPressure,
      json.opticalDepth.map((row) => row.length),
    ]).toEqual([
      Array.from(line.logPressures),
      line.readTopLogPressure,
      [0, 1, 2].map(() => line.logPressures.length * line.logColumns.length),
    ]);
  });

  it("writes a pair absorber's pair density with its curve", () => {
    const { medium, curves } = fourAbsorberMedium();
    const terms = medium.terms.filter((t) => t.absorber !== undefined);
    const k = terms.findIndex((t) => t.absorber?.kind === "pair");
    const pair = terms[k];
    if (pair?.absorber?.kind !== "pair") {
      throw new Error("the four-absorber medium has its pair");
    }
    const json = termJson(pair, curves[0]?.[k], undefined);
    expect(json.absorber).toEqual({
      kind: "pair",
      pairDensityPerM6: pair.absorber.pairDensityPerM6,
      curves: [expect.objectContaining({ logColumns: expect.any(Array) })],
    });
  });

  it("refuses an absorber given a curve of the other kind", () => {
    const pair = fourAbsorberMedium().medium.terms.find((t) => t.absorber?.kind === "pair");
    if (pair === undefined) {
      throw new Error("the four-absorber medium has its pair");
    }
    expect(() => termJson(pair, OZONE_CURVE, undefined)).toThrow(/pair absorber .* column curve/u);
  });

  it("refuses a column absorber given no curve", () => {
    expect(() => termJson(OZONE, undefined, undefined)).toThrow(/no curve/u);
  });

  it("refuses a case whose curves are not one an absorber", () => {
    expect(() => caseJson({ ...EARTH_CASE, curves: [] }, undefined)).toThrow(
      /0 curves for 1 absorbers/u,
    );
  });
});

describe("the case writer's line media (R08.T12.b.2)", () => {
  const { medium, curves } = lineAbsorberMedium();
  const lineCase: ReferenceCase = { ...EARTH_CASE, medium, curves: curves[0] ?? [] };

  it("writes the medium's pressure profile beside its terms", () => {
    const json = caseJson(lineCase, undefined);
    expect([
      Object.keys(json).indexOf("pressureBar") === Object.keys(json).indexOf("terms") - 1,
      json.pressureBar,
    ]).toEqual([
      true,
      medium.pressureBar === undefined ? undefined : densityJson(medium.pressureBar),
    ]);
  });

  it("writes no pressure profile for a medium without one", () => {
    expect(caseJson(EARTH_CASE, undefined).pressureBar).toBeUndefined();
  });

  it("writes its pressure profile and its curves' pressure rows to the case's figures", () => {
    const json = caseJson({ ...lineCase, significantFigures: 3 }, undefined);
    const profile = json.pressureBar;
    const rows = json.terms.flatMap((t) => t.absorber?.curves ?? []);
    const numbers = [
      ...(profile?.kind === "tabulated" ? [...profile.altitudesM, ...profile.relative] : []),
      ...rows.flatMap((c) => [
        ...(c.logPressures ?? []),
        ...(c.readTopLogPressure === undefined ? [] : [c.readTopLogPressure]),
        ...c.opticalDepth.flat(),
      ]),
    ];
    expect([
      profile?.kind,
      rows.some((c) => (c.logPressures?.length ?? 0) > 1),
      numbers.filter((v) => Number(v.toPrecision(3)) !== v),
    ]).toEqual(["tabulated", true, []]);
  });
});

describe("the case writer's figures (R08.T12.b.2)", () => {
  it("writes a case's terms to its figures", () => {
    const json = caseJson({ ...MARS_CASE, significantFigures: 3 }, undefined);
    const dust = json.terms.find((t) => t.name === "dust");
    expect(dust?.scattering.filter((v) => Number(v.toPrecision(3)) !== v)).toEqual([]);
  });

  it("writes a case's geometries in full whatever its figures", () => {
    expect(caseJson({ ...MARS_CASE, significantFigures: 3 }, undefined).geometries).toEqual(
      caseJson(MARS_CASE, undefined).geometries,
    );
  });

  it("keeps each matrix element no larger than a₁ when rounded", () => {
    const rounded = roundedTermJson(termJson(DUST, undefined, DUST_ASYMMETRY), 2);
    const phase = rounded.phase;
    if (phase.kind !== "tabulated" || phase.matrix === undefined) {
      throw new Error("Mars's dust keeps its table's matrix");
    }
    const { a1, matrix } = phase;
    const past = (["a2", "a3", "a4", "b1", "b2"] as const).flatMap((element) =>
      matrix[element].flatMap((row, c) =>
        row.filter((value, j) => Math.abs(value) > (a1[c]?.[j] ?? Number.NaN)),
      ),
    );
    expect(past).toEqual([]);
  });

  it("writes every term field it rounds in the order the full term has it", () => {
    const term = termJson(DUST, undefined, DUST_ASYMMETRY);
    expect(Object.keys(roundedTermJson(term, 9))).toEqual(Object.keys(term));
  });
});

describe("the case writer's geometries", () => {
  const [first, second] = MARS_CASE.geometries;
  if (first === undefined || second === undefined) {
    throw new Error("Mars's case has geometries");
  }

  it("writes a geometry's own cone where it gives one, and none where it does not", () => {
    const json = caseJson(
      { ...MARS_CASE, geometries: [{ ...first, detectorHalfAngleDeg: 1 }, second] },
      undefined,
    );
    expect(json.geometries.map((g) => g.detectorHalfAngleDeg)).toEqual([1, undefined]);
  });

  it("writes a geometry's own samples where the counts give one, and none where they do not", () => {
    const json = caseJson(MARS_CASE, { geometries: { [first.name]: 8192 } });
    expect(json.geometries.slice(0, 2).map((g) => g.samples)).toEqual([8192, undefined]);
  });
});

describe("the trace command", () => {
  it("traces at the default count the committed references record", () => {
    expect(traceCommand("mars")).toMatch(/ --samples 4096 /u);
  });
});

/** `spectral` with its term `name` changed by `change`. */
function withTerm(
  spectral: SpectralReferenceCase,
  name: string,
  change: (term: SpectralCaseTerm) => SpectralCaseTerm,
): SpectralReferenceCase {
  return { ...spectral, terms: spectral.terms.map((t) => (t.name === name ? change(t) : t)) };
}

/** A spectral case's dust as the writer writes it. */
function writtenDust(spectral: SpectralReferenceCase): TermJson {
  const dust = spectralCaseJson(spectral, undefined).terms.find((t) => t.name === "dust");
  if (dust === undefined) {
    throw new Error("the dust is written");
  }
  return dust;
}

describe("the spectral case writer (R08.T17)", () => {
  it("normalises each wavelength's table over the sphere read linearly in u, by its own integral", () => {
    // One wavelength's table doubled: its row must still integrate to 1, the others unmoved.
    const doubled = withTerm(MARS_SPECTRAL_LOW_CASE, "dust", (t) =>
      t.phase.kind === "tabulated"
        ? {
            ...t,
            phase: {
              ...t.phase,
              tables: t.phase.tables.map((table, w) =>
                w === 5
                  ? {
                      ...table,
                      values: [
                        table.values[0].map((v) => 2 * v),
                        table.values[1].map((v) => 2 * v),
                        table.values[2].map((v) => 2 * v),
                      ],
                    }
                  : table,
              ),
            },
          }
        : t,
    );
    const plain = writtenDust(MARS_SPECTRAL_LOW_CASE);
    const json = writtenDust(doubled);
    if (json.phase.kind !== "tabulated" || plain.phase.kind !== "tabulated") {
      throw new Error("the dust is written tabulated");
    }
    const u = Float64Array.from(json.phase.u);
    expect(json.phase.a1).toHaveLength(15);
    for (const row of json.phase.a1) {
      expect(linearInUMoments(u, Float64Array.from(row)).integral).toBeCloseTo(1, 5);
    }
    // Doubling is exact in binary, so the doubled row over its doubled integral is the plain row.
    expect(json.phase.a1[5]).toEqual(plain.phase.a1[5]);
    expect(json.depolarising).toBeUndefined();
  });

  it("drops the matrix and marks the term depolarising where one wavelength's table has none", () => {
    const spectral = withTerm(MARS_SPECTRAL_LOW_CASE, "dust", (t) =>
      t.phase.kind === "tabulated"
        ? {
            ...t,
            phase: {
              ...t.phase,
              tables: t.phase.tables.map((table, w) =>
                w === 7 ? { ...table, matrix: undefined } : table,
              ),
            },
          }
        : t,
    );
    const dust = spectralCaseJson(spectral, undefined).terms.find((t) => t.name === "dust");
    expect(dust?.phase.kind === "tabulated" ? dust.phase.matrix : "none").toBeUndefined();
    expect(dust?.depolarising).toBe(true);
  });

  it("refuses a tabulated phase whose wavelengths' tables are on different entries", () => {
    const spectral = withTerm(MARS_SPECTRAL_LOW_CASE, "dust", (t) =>
      t.phase.kind === "tabulated"
        ? {
            ...t,
            phase: {
              ...t.phase,
              tables: t.phase.tables.map((table, w) =>
                w === 3 ? { ...table, u: table.u.map((v, i) => (i === 1 ? v * 1.01 : v)) } : table,
              ),
            },
          }
        : t,
    );
    expect(() => spectralCaseJson(spectral, undefined)).toThrow(RangeError);
  });

  it("writes an absorber's 15 bins' curves as one curve of 15 wavelengths", () => {
    const ozone = spectralCaseJson(EARTH_SPECTRAL_DAY_CASE, undefined).terms.find(
      (t) => t.name === "absorber:O3",
    );
    expect(ozone?.absorber?.curves).toHaveLength(1);
    expect(ozone?.absorber?.curves[0]?.opticalDepth).toHaveLength(15);
    expect(ozone?.absorber?.kind).toBe("column");
  });

  it("writes its terms to its figures and its geometries in full, with their own counts", () => {
    const [g] = EARTH_SPECTRAL_DAY_CASE.geometries;
    if (g === undefined) {
      throw new Error("Earth by day has geometries");
    }
    const json = spectralCaseJson(EARTH_SPECTRAL_DAY_CASE, { geometries: { [g.name]: 8192 } });
    const rayleigh = json.terms.find((t) => t.name === "rayleigh");
    for (const v of rayleigh?.scattering ?? []) {
      expect(Number(v.toPrecision(SPECTRAL_CASE_SIGNIFICANT_FIGURES))).toBe(v);
    }
    expect(json.geometries[0]?.view).toEqual(g.view);
    expect(json.geometries[0]?.observer).toEqual(g.observer);
    expect(json.geometries[0]?.samples).toBe(8192);
    expect(json.geometries[1]?.samples).toBeUndefined();
    expect(json.wavelengthsNm).toHaveLength(15);
  });
});
