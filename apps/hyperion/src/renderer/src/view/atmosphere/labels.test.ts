import { describe, expect, it } from "vitest";

import {
  ATMOSPHERE_LABELS,
  ATMOSPHERE_STATEMENTS,
  type AtmosphereLabel,
  atmosphereStatements,
} from "./labels";

describe("the atmosphere labels", () => {
  it("spell the six statements as the guide's rows do", () => {
    expect(ATMOSPHERE_STATEMENTS).toEqual({
      atmosphereNotResolved: "ATMOSPHERE: NOT RESOLVED",
      atmosphereNotYetModelled: "ATMOSPHERE: NOT YET MODELLED",
      aerosolsNotYetModelled: "AEROSOLS AND ABSORBERS: NOT YET MODELLED",
      atmospherePending: "ATMOSPHERE: PENDING",
      atmosphereComputing: "ATMOSPHERE: COMPUTING",
      atmosphereApproximate: "ATMOSPHERE: APPROXIMATE",
    });
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
      "aerosolsNotYetModelled",
      "atmosphereNotYetModelled",
    ];
    expect(atmosphereStatements(labels)).toEqual([
      "ATMOSPHERE: NOT YET MODELLED",
      "AEROSOLS AND ABSORBERS: NOT YET MODELLED",
      "ATMOSPHERE: NOT RESOLVED",
      "ATMOSPHERE: PENDING",
      "ATMOSPHERE: COMPUTING",
      "ATMOSPHERE: APPROXIMATE",
    ]);
  });
});
