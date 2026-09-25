import { type Population, universeTimeFromYears } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  aRangeRequest,
  aStellarBrief,
  aSystemsInRange,
  UNIVERSE_ID,
} from "../../test/galaxyFixtures";
import { CHART_SYSTEM_LIMIT } from "./model";
import { layerIndex, populationLabel, toChartResult, toRangeRequest } from "./wire";

describe("toChartResult", () => {
  it("sorts systems nearest first and gives each its distance and offset", () => {
    const result = toChartResult(
      aSystemsInRange({
        systems: [
          { relLy: [0, 0, 30], layer: "a" },
          { relLy: [3, 4, 0], layer: "c" },
          { relLy: [-10, 0, 0], layer: "b" },
        ],
      }),
    );

    expect(result.systems.map((system) => system.distanceLy)).toEqual([5, 10, 30]);
    const nearest = result.systems[0];
    expect(nearest?.layer).toBe("c");
    expect(nearest?.relLy.x).toBeCloseTo(3, 9);
    expect(nearest?.relLy.y).toBeCloseTo(4, 9);
    expect(nearest?.relLy.z).toBeCloseTo(0, 9);
    expect(nearest?.positionLy.x).toBeCloseTo(26_003, 9);
  });

  it("orders systems at equal distance by ID, whatever the wire order", () => {
    const response = aSystemsInRange({
      // Offsets along one axis each, so that the three distances are exactly equal.
      systems: [
        { relLy: [0, 5, 0], layer: "a" },
        { relLy: [0, 0, -5], layer: "a" },
        { relLy: [0, -5, 0], layer: "a" },
      ],
      centreLy: [0, 0, 0],
    });
    const reversed = { ...response, systems: response.systems.toReversed() };

    const ids = ["0000000000000001", "0000000000000002", "0000000000000003"];
    expect(toChartResult(response).systems.map((system) => system.id)).toEqual(ids);
    expect(toChartResult(reversed).systems.map((system) => system.id)).toEqual(ids);
  });

  it("keeps a hundredth of a light-year exact 60,000 ly from the centre of the galaxy", () => {
    const result = toChartResult(
      aSystemsInRange({ centreLy: [60_000, 0, 0], systems: [{ relLy: [0.01, 0, 0], layer: "a" }] }),
    );

    expect(Math.abs((result.systems[0]?.distanceLy ?? Number.NaN) - 0.01)).toBeLessThan(1e-9);
  });

  it("takes centre, radius and time from the answer", () => {
    const result = toChartResult(
      aSystemsInRange({ centreLy: [-26_000.25, 12, -3.5], radiusLy: 20, timeYr: 12.5 }),
    );

    expect(result.centreLy[0]).toBeCloseTo(-26_000.25, 9);
    expect(result.centreLy[1]).toBeCloseTo(12, 9);
    expect(result.centreLy[2]).toBeCloseTo(-3.5, 9);
    expect(result.radiusLy).toBe(20);
    expect(result.timeYr).toBeCloseTo(12.5, 12);
  });

  it("reads the census as complete above the lightest included layer", () => {
    const result = toChartResult(
      aSystemsInRange({ minLayer: "b", systems: [{ relLy: [1, 0, 0], layer: "b" }] }),
    );

    expect(result.census).toEqual({ kind: "complete", aboveMsun: 0.5 });
    expect(result.layers.map((layer) => layer.status)).toEqual([
      "below_mass_floor",
      "included",
      "included",
      "included",
      "included",
    ]);
  });

  it("reads a census with no complete layer as nothing fits", () => {
    // Every layer over the limit: the fixture's census then has no lower edge of completeness.
    const response = aSystemsInRange({ overLimit: ["a", "b", "c", "d", "e"] });

    expect(toChartResult(response).census).toEqual({ kind: "nothing_fits" });
  });
});

describe("toChartResult's stellar briefs", () => {
  it("carries each row's brief into its system", () => {
    const result = toChartResult(
      aSystemsInRange({
        systems: [{ relLy: [1, 0, 0], layer: "c", stellar: aStellarBrief("c", "white_dwarf") }],
      }),
    );

    expect(result.systems[0]?.star).toEqual({
      kind: "white_dwarf",
      spectralClass: "DA4.2",
      logLuminosityLsun: -2.5,
      teffK: 12_000,
      starCount: 1,
    });
  });

  it("keeps a black hole's missing luminosity and temperature missing", () => {
    const result = toChartResult(
      aSystemsInRange({
        systems: [{ relLy: [1, 0, 0], layer: "e", stellar: aStellarBrief("e", "black_hole") }],
      }),
    );

    expect(result.systems[0]?.star?.logLuminosityLsun).toBeNull();
    expect(result.systems[0]?.star?.teffK).toBeNull();
  });

  it("reads a row without a brief as a system not yet formed", () => {
    const result = toChartResult(
      aSystemsInRange({ systems: [{ relLy: [1, 0, 0], layer: "a", stellar: null }] }),
    );

    expect(result.systems[0]?.star).toBeNull();
  });
});

describe("toRangeRequest", () => {
  it("asks for every row's stellar brief", () => {
    const request = toRangeRequest(UNIVERSE_ID, [26_000, 0, 0], 20, 0, "a");

    expect(request.include_stellar).toBe(true);
  });

  it("carries the centre exactly, flooring a negative coordinate into the cell below", () => {
    const request = toRangeRequest(UNIVERSE_ID, [-0.25, 26_000.5, 0], 50, 12.5, "b");

    expect(request.centre.cell_ly).toEqual([-1, 26_000, 0]);
    expect(request.centre.offset_m[0]).toBeCloseTo(0.75 * 9_460_730_472_580_800, -3);
    expect(request.centre.offset_m[1]).toBeCloseTo(0.5 * 9_460_730_472_580_800, -3);
  });

  it("names the universe, radius, time and layer, and carries the chart limit", () => {
    const request = toRangeRequest(UNIVERSE_ID, [26_000, 0, 0], 20, 12.5, "c");

    expect(request).toEqual(
      aRangeRequest({
        universe: UNIVERSE_ID,
        centreLy: [26_000, 0, 0],
        radiusLy: 20,
        timeYr: 12.5,
        minLayer: "c",
        limit: CHART_SYSTEM_LIMIT,
      }),
    );
    expect(request.time).toEqual(universeTimeFromYears(12.5));
    expect(CHART_SYSTEM_LIMIT).toBe(4_000);
  });
});

describe("populationLabel", () => {
  it.each<[Population, string]>([
    ["young_thin_disc", "YOUNG THIN DISC"],
    ["old_thin_disc", "OLD THIN DISC"],
    ["thick_disc", "THICK DISC"],
    ["bulge", "BULGE"],
    ["long_bar", "LONG BAR"],
    ["nuclear_disc", "NUCLEAR DISC"],
    ["halo", "HALO"],
  ])("names %s %s", (population, label) => {
    expect(populationLabel(population)).toBe(label);
  });
});

describe("layerIndex", () => {
  it("counts the layers from the lightest", () => {
    expect((["a", "b", "c", "d", "e"] as const).map(layerIndex)).toEqual([0, 1, 2, 3, 4]);
  });
});
