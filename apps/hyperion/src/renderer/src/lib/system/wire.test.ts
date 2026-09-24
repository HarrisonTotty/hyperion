import { describe, expect, it } from "vitest";

import {
  aBinaryHierarchy,
  aNotYetBornSummary,
  aPairOrbit,
  aSunlikeStar,
  aSystemSummary,
  aWhiteDwarf,
  PIN_SYSTEM,
} from "../../test/systemFixtures";
import type { SystemModel } from "./model";
import { toSummaryRequest, toSystemModel } from "./wire";

const DESIGNATION = "H7K 4C0RFZ D-7";

function modelOf(summary = aSystemSummary()): SystemModel {
  const result = toSystemModel(summary, DESIGNATION);
  if (result.kind !== "ok") {
    throw new Error(`the fixture should convert, but: ${result.fault}`);
  }
  return result.model;
}

describe("toSystemModel", () => {
  it("names each star by its system's ID and its body index", () => {
    expect(modelOf().hosts.map((host) => host.id)).toEqual([
      `${PIN_SYSTEM}.0000`,
      `${PIN_SYSTEM}.0001`,
    ]);
  });

  it("gives each star its system's designation and its body index", () => {
    expect(modelOf().hosts.map((host) => host.designation)).toEqual([
      "H7K 4C0RFZ D-7 /0",
      "H7K 4C0RFZ D-7 /1",
    ]);
  });

  it("carries the pinned white dwarf's values and its cooling age", () => {
    const [dwarf] = modelOf().hosts;

    expect(dwarf).toMatchObject({
      kind: "white_dwarf",
      phase: "carbon_oxygen_white_dwarf",
      spectralClass: "DA9.2",
      initialMassMsun: 2.5,
      massMsun: 0.687_5,
      luminosityLsun: 0.000_107_5,
      radiusRsun: 0.011_5,
      teffK: 5_480,
      remnant: {
        kind: "white_dwarf",
        coolingAgeMyr: 3_950.5,
        natalKick: { kind: "not_modelled" },
      },
    });
  });

  it("reads a value the wire leaves out as not modelled, a null one as none", () => {
    const summary = aSystemSummary({
      stars: [aSunlikeStar({ rotation_period_d: 25.375, variability: null })],
      hierarchy: { nodes: [{ type: "star", body_index: 0, mass_msun: 1 }] },
    });
    const [star] = modelOf(summary).hosts;

    expect(star?.rotationPeriodD).toEqual({ kind: "value", value: 25.375 });
    expect(star?.variability).toEqual({ kind: "none" });
    expect(star?.activityLogLxLbol).toEqual({ kind: "not_modelled" });
  });

  it("takes the pair's orbit as the elements the client propagates", () => {
    const [pair] = modelOf().hierarchy;

    expect(pair).toEqual({
      kind: "pair",
      inner: 1,
      outer: 2,
      orbit: {
        semiMajorAxisM: 3_515_625_000_000,
        eccentricity: 0.5,
        inclinationRad: 1.5,
        ascendingNodeRad: 3.5,
        argumentOfPeriapsisRad: 4.25,
        meanAnomalyAtEpochRad: 0.125,
        periodS: 1_921_736_528.404_686,
      },
    });
  });

  it("reads a system not yet born as not formed, with no stars", () => {
    const model = modelOf(aNotYetBornSummary());

    expect(model.formed).toBe(false);
    expect(model.hosts).toEqual([]);
  });

  it.each([
    [
      "a pair whose member is itself",
      aSystemSummary({
        hierarchy: {
          nodes: [
            { type: "pair", inner: 0, outer: 2, orbit: aPairOrbit() },
            { type: "star", body_index: 0, mass_msun: 2.5 },
            { type: "star", body_index: 1, mass_msun: 1 },
          ],
        },
      }),
      "hierarchy malformed",
    ],
    [
      "a star listed but not in the hierarchy",
      aSystemSummary({ hierarchy: { nodes: [{ type: "star", body_index: 0, mass_msun: 2.5 }] } }),
      "hierarchy malformed",
    ],
    [
      "an orbit the client cannot propagate",
      aSystemSummary({ hierarchy: aBinaryHierarchy(aPairOrbit({ eccentricity: 0.999_95 })) }),
      "orbit unusable",
    ],
    [
      "stars out of body-index order",
      aSystemSummary({ stars: [aSunlikeStar({ body_index: 1 }), aWhiteDwarf()] }),
      "star list malformed",
    ],
    [
      "a formed system with no stars",
      aSystemSummary({ stars: [], hierarchy: { nodes: [] } }),
      "star list empty",
    ],
    [
      "a mass that is not a number",
      aSystemSummary({
        stars: [aWhiteDwarf({ mass_msun: Number.NaN }), aSunlikeStar({ body_index: 1 })],
      }),
      "star 0 values unusable",
    ],
  ])("reports %s as a fault in words", (_case, summary, fault) => {
    expect(toSystemModel(summary, DESIGNATION)).toEqual({ kind: "fault", fault });
  });
});

describe("toSummaryRequest", () => {
  it("asks for the system at the time given", () => {
    expect(toSummaryRequest("0123456789abcdef", PIN_SYSTEM, { seconds: 7, nanos: 5 })).toEqual({
      kind: "system_summary",
      universe: "0123456789abcdef",
      system: PIN_SYSTEM,
      time: { seconds: 7, nanos: 5 },
    });
  });
});
