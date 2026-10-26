import { describe, expect, it } from "vitest";

import {
  ATMOSPHERE_ANSWER_UNREADABLE,
  ATMOSPHERE_LABELS,
  ATMOSPHERE_NOT_AVAILABLE,
  ATMOSPHERE_STATEMENTS,
  atmosphereDataInvalid,
  type AtmosphereLabel,
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

  it("spell the fault as the guide's ATMOSPHERE NOT AVAILABLE row does", () => {
    expect(ATMOSPHERE_NOT_AVAILABLE).toBe(
      "ATMOSPHERE NOT AVAILABLE: computation failed, reopen VIEW to retry",
    );
  });

  it("say the server's reason for a refused record, then the remedy", () => {
    expect(atmosphereRejected("unknown body")).toBe(
      "ATMOSPHERE REJECTED: unknown body, reopen VIEW to retry",
    );
  });

  it("leave the remedy alone when the server gives no reason", () => {
    expect(atmosphereRejected("  ")).toBe("ATMOSPHERE REJECTED: reopen VIEW to retry");
  });

  it("say the check an unreadable record failed, then the remedy", () => {
    expect(atmosphereDataInvalid(ATMOSPHERE_ANSWER_UNREADABLE)).toBe(
      "ATMOSPHERE DATA INVALID: answer unreadable, reopen VIEW to retry",
    );
  });

  it("keep the fault out of the steady statements", () => {
    expect(Object.values(ATMOSPHERE_STATEMENTS)).not.toContain(ATMOSPHERE_NOT_AVAILABLE);
  });

  it("list every label once", () => {
    expect(new Set(ATMOSPHERE_LABELS).size).toBe(ATMOSPHERE_LABELS.length);
    expect(ATMOSPHERE_LABELS.toSorted()).toEqual(Object.keys(ATMOSPHERE_STATEMENTS).toSorted());
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
