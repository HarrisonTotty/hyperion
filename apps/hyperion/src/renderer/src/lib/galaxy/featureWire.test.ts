import { describe, expect, it } from "vitest";

import {
  aFeaturesInRange,
  ASSOCIATION,
  GALACTIC_CENTRE,
  GOLDEN_CENTRE_LY,
  OPEN_CLUSTER,
  STAR_FORMING_REGION,
} from "../../test/featureFixtures";
import { UNIVERSE_ID } from "../../test/galaxyFixtures";
import { CHART_FEATURE_KINDS, toChartFeatures, toFeaturesRequest } from "./featureWire";

/** A metre in light-years: the light-year is c times a Julian year, 9,460,730,472,580,800 m exactly. */
const LY_PER_M = 1 / 9_460_730_472_580_800;

describe("toChartFeatures", () => {
  it("reads the golden's features nearest first, each offset from the answer's own centre", () => {
    const chart = toChartFeatures(aFeaturesInRange());

    expect(chart.features.map((feature) => feature.designation)).toEqual([
      "GPF F207",
      "GPG F336",
      "GPG F530",
    ]);
    const cluster = chart.features[0];
    expect(cluster?.relLy.x).toBeCloseTo(106 + 6_814_297_182_197_701 * LY_PER_M, 6);
    expect(cluster?.relLy.y).toBeCloseTo(115 + 1_813_335_634_579_622.8 * LY_PER_M, 6);
    expect(cluster?.positionLy.y).toBeCloseTo(26_115 + 1_813_335_634_579_622.8 * LY_PER_M, 6);
    expect(cluster?.distanceLy).toBeCloseTo(
      Math.hypot(cluster?.relLy.x ?? 0, cluster?.relLy.y ?? 0, cluster?.relLy.z ?? 0),
      9,
    );
  });

  it("keeps each feature's values as the server sent them, in the client's names", () => {
    const [cluster] = toChartFeatures(aFeaturesInRange({ features: [OPEN_CLUSTER] })).features;

    expect(cluster).toMatchObject({
      id: "e859e06780000000",
      designation: "GPF F207",
      kind: "open_cluster",
      extentLy: OPEN_CLUSTER.extent_ly,
      ageMyr: OPEN_CLUSTER.age_myr,
      massMsun: OPEN_CLUSTER.mass_msun,
      expectedMembers: OPEN_CLUSTER.expected_members,
      emission: null,
      bubbleRadiusLy: OPEN_CLUSTER.bubble_radius_ly,
    });
  });

  it("keeps a cloud's and the centre's missing age, and a region's emission", () => {
    const chart = toChartFeatures(
      aFeaturesInRange({ centreLy: [0, 0, 0], features: [GALACTIC_CENTRE, STAR_FORMING_REGION] }),
    );

    expect(
      chart.features.map((feature) => [feature.kind, feature.ageMyr, feature.emission]),
    ).toEqual([
      ["galactic_centre", null, null],
      ["star_forming_region", STAR_FORMING_REGION.age_myr, "reflection_nebula"],
    ]);
  });

  it("orders features at one distance by ID, whatever the wire's order", () => {
    const twin = { ...ASSOCIATION, id: "e85a010900000001" };

    const forwards = toChartFeatures(aFeaturesInRange({ features: [twin, ASSOCIATION] }));
    const backwards = toChartFeatures(aFeaturesInRange({ features: [ASSOCIATION, twin] }));

    expect(forwards.features.map((feature) => feature.id)).toEqual([
      "e85a010900000000",
      "e85a010900000001",
    ]);
    expect(backwards.features).toEqual(forwards.features);
  });

  it("echoes the answer's centre, radius, time and census", () => {
    const response = aFeaturesInRange({ timeYr: 1000, radiusLy: 50 });

    const chart = toChartFeatures(response);

    expect(chart.centreLy).toEqual(GOLDEN_CENTRE_LY);
    expect(chart.radiusLy).toBe(50);
    expect(chart.timeYr).toBe(1000);
    expect(chart.census).toBe(response.census);
  });
});

describe("toFeaturesRequest", () => {
  it("asks for every kind at the chart's centre, radius and time", () => {
    expect(toFeaturesRequest(UNIVERSE_ID, [0, 26_000, -0.5], 50, 12.5)).toEqual({
      kind: "features_in_range",
      universe: UNIVERSE_ID,
      centre: { cell_ly: [0, 26_000, -1], offset_m: [0, 0, 0.5 / LY_PER_M] },
      radius_ly: 50,
      time: { seconds: 394_470_000, nanos: 0 },
      kinds: [...CHART_FEATURE_KINDS],
    });
    expect(CHART_FEATURE_KINDS).toHaveLength(8);
  });
});
