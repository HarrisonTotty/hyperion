/**
 * The case writer's rules and refusals (plan R08, R08.T12.b.1; `decision-r08-t12b-scope.md`,
 * rulings 4, 6 and 10), on small variations of the cases' own terms: the branches the four
 * committed cases do not reach.
 */

import { describe, expect, it } from "vitest";

import type { MediumTerm, PhaseTable } from "../view/atmosphere/medium";
import { EARTH_CASE, MARS_CASE } from "../view/atmosphere/reference/cases";
import {
  absorberCurveJson,
  caseJson,
  densityJson,
  tabulatedPhaseJson,
  termJson,
  traceCommand,
} from "./atmosphereCases";
import { fourAbsorberMedium } from "./atmosphereFixtures";
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

  it("refuses a curve of several pressure rows until R08.T12.b.2", () => {
    if (OZONE_CURVE === undefined) {
      throw new Error("Earth's case has its ozone's curve");
    }
    const rows = { ...OZONE_CURVE, logPressures: new Float64Array([9, 10]) };
    expect(() => absorberCurveJson(rows)).toThrow(/2 pressure rows/u);
  });

  it("refuses a pair absorber until R08.T12.b.2", () => {
    const pair = fourAbsorberMedium().medium.terms.find((t) => t.absorber?.kind === "pair");
    if (pair === undefined) {
      throw new Error("the four-absorber medium has its pair");
    }
    expect(() => termJson(pair, OZONE_CURVE, undefined)).toThrow(/pair absorber/u);
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
