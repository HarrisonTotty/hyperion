import { describe, expect, it } from "vitest";

import {
  ATMOSPHERE_ANSWER_UNREADABLE,
  ATMOSPHERE_LABELS,
  ATMOSPHERE_STATEMENTS,
  atmosphereDataInvalid,
  type AtmosphereLabel,
  atmosphereNotAvailable,
  atmosphereProblemLines,
  atmosphereRejected,
  atmosphereStatements,
} from "./labels";

describe("the atmosphere labels", () => {
  it("spell the eight statements as the guide's rows do", () => {
    expect(ATMOSPHERE_STATEMENTS).toEqual({
      atmosphereNotResolved: "ATMOSPHERE: NOT RESOLVED",
      atmosphereNotYetModelled: "ATMOSPHERE: NOT YET MODELLED",
      aerosolsNotYetModelled: "AEROSOLS AND ABSORBERS: NOT YET MODELLED",
      exosphereNotYetModelled: "EXOSPHERE: NOT YET MODELLED",
      silicateVapourNotYetModelled: "SILICATE VAPOUR: NOT YET MODELLED",
      atmospherePending: "ATMOSPHERE: PENDING",
      atmosphereComputing: "ATMOSPHERE: COMPUTING",
      atmosphereApproximate: "ATMOSPHERE: APPROXIMATE",
    });
  });

  it("spell the fault as the guide's ATMOSPHERE NOT AVAILABLE row does, naming the body", () => {
    expect(atmosphereNotAvailable(["TEST PLANET"])).toBe(
      "ATMOSPHERE NOT AVAILABLE: computation failed for TEST PLANET, reopen VIEW to retry",
    );
  });

  it("name the bodies before the server's reason for a refused record", () => {
    expect(atmosphereRejected(["TEST MOON"], "unknown body")).toBe(
      "ATMOSPHERE REJECTED: for TEST MOON, unknown body, reopen VIEW to retry",
    );
  });

  it("leave the remedy alone when the server gives no reason", () => {
    expect(atmosphereRejected(["TEST MOON"], "  ")).toBe(
      "ATMOSPHERE REJECTED: for TEST MOON, reopen VIEW to retry",
    );
  });

  it("name the body after the check its unreadable record failed", () => {
    expect(atmosphereDataInvalid(["TEST MOON"], ATMOSPHERE_ANSWER_UNREADABLE)).toBe(
      "ATMOSPHERE DATA INVALID: answer unreadable for TEST MOON, reopen VIEW to retry",
    );
  });

  it("join three designations as the guide joins names in one line", () => {
    expect(atmosphereNotAvailable(["TEST PLANET", "TEST MOON", "TEST GIANT"])).toBe(
      "ATMOSPHERE NOT AVAILABLE: computation failed for TEST PLANET, TEST MOON and TEST GIANT, " +
        "reopen VIEW to retry",
    );
  });

  it("refuse a fault that names no body", () => {
    expect(() => atmosphereNotAvailable([])).toThrow(RangeError);
  });

  it("list every label once", () => {
    expect(new Set(ATMOSPHERE_LABELS).size).toBe(ATMOSPHERE_LABELS.length);
    expect(ATMOSPHERE_LABELS.toSorted()).toEqual(Object.keys(ATMOSPHERE_STATEMENTS).toSorted());
  });
});

describe("atmosphereProblemLines", () => {
  it("gives two bodies failing alike one line naming both, in designation order", () => {
    const lines = atmosphereProblemLines([
      { designation: "Kepler-9 /10", problem: { kind: "failed" } },
      { designation: "Kepler-9 /2", problem: { kind: "failed" } },
    ]);
    expect(lines.fault).toEqual([
      "ATMOSPHERE NOT AVAILABLE: computation failed for Kepler-9 /2 and Kepler-9 /10, " +
        "reopen VIEW to retry",
    ]);
  });

  it("gives each distinct reason a line of its own, coloured by whether it was a refusal", () => {
    const lines = atmosphereProblemLines([
      {
        designation: "TEST PLANET",
        problem: { kind: "rejected", code: "internal", message: "store offline", refusal: false },
      },
      {
        designation: "TEST MOON",
        problem: { kind: "rejected", code: "unknown_body", message: "unknown body", refusal: true },
      },
    ]);
    expect(lines.plate).toEqual([
      {
        statement: "ATMOSPHERE REJECTED: for TEST MOON, unknown body, reopen VIEW to retry",
        refusal: true,
      },
      {
        statement: "ATMOSPHERE REJECTED: for TEST PLANET, store offline, reopen VIEW to retry",
        refusal: false,
      },
    ]);
  });

  it("puts the failed computations' line before the unreadable records' on the fault line", () => {
    const lines = atmosphereProblemLines([
      { designation: "A", problem: { kind: "invalid", check: ATMOSPHERE_ANSWER_UNREADABLE } },
      { designation: "B", problem: { kind: "failed" } },
    ]);
    expect(lines.fault).toEqual([
      "ATMOSPHERE NOT AVAILABLE: computation failed for B, reopen VIEW to retry",
      "ATMOSPHERE DATA INVALID: answer unreadable for A, reopen VIEW to retry",
    ]);
  });

  it("states nothing for a view with no problems", () => {
    expect(atmosphereProblemLines([])).toEqual({ plate: [], fault: [] });
  });
});

describe("atmosphereStatements", () => {
  it("states nothing for bodies that carry no label", () => {
    expect(atmosphereStatements([])).toEqual([]);
  });

  it("states a label two bodies carry once", () => {
    expect(atmosphereStatements(["atmosphereNotYetModelled", "atmosphereNotYetModelled"])).toEqual([
      "ATMOSPHERE: NOT YET MODELLED",
    ]);
  });

  it("puts NOT RESOLVED after the NOT YET MODELLED notes and the annunciations last", () => {
    const labels: ReadonlyArray<AtmosphereLabel> = [
      "atmosphereApproximate",
      "atmospherePending",
      "atmosphereNotResolved",
      "atmosphereComputing",
      "silicateVapourNotYetModelled",
      "exosphereNotYetModelled",
      "aerosolsNotYetModelled",
      "atmosphereNotYetModelled",
    ];
    expect(atmosphereStatements(labels)).toEqual([
      "ATMOSPHERE: NOT YET MODELLED",
      "AEROSOLS AND ABSORBERS: NOT YET MODELLED",
      "EXOSPHERE: NOT YET MODELLED",
      "SILICATE VAPOUR: NOT YET MODELLED",
      "ATMOSPHERE: NOT RESOLVED",
      "ATMOSPHERE: PENDING",
      "ATMOSPHERE: COMPUTING",
      "ATMOSPHERE: APPROXIMATE",
    ]);
  });
});
