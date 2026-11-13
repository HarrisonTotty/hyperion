import { describe, expect, it } from "vitest";

import { toChartFeatures } from "../../lib/galaxy/featureWire";
import type { ChartFeatures } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { localFrameAt } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import {
  aFeaturesInRange,
  type FeaturesInRangeSpec,
  ASSOCIATION,
  GALACTIC_CENTRE,
  GOLDEN_CENTRE_LY,
  GOLDEN_RADIUS_LY,
  OPEN_CLUSTER,
  STAR_FORMING_REGION,
} from "../../test/featureFixtures";
import { aSystemsInRange } from "../../test/galaxyFixtures";
import {
  FEATURE_SIZE_CLASS,
  featureDataFault,
  featureMarkId,
  selectionMarkId,
  selectionOfMark,
  toScene,
} from "./chartModel";

const FRAME = localFrameAt(vec3(...GOLDEN_CENTRE_LY));

/** The golden's chart: a system 3 ly out at the golden's centre and radius. */
const CHART = toChartResult(
  aSystemsInRange({
    centreLy: GOLDEN_CENTRE_LY,
    radiusLy: GOLDEN_RADIUS_LY,
    systems: [{ relLy: [3, 0, 0], layer: "a" }],
  }),
);

function featuresOf(spec: FeaturesInRangeSpec = {}): ChartFeatures {
  return toChartFeatures(aFeaturesInRange(spec));
}

function sceneWith(features: ChartFeatures, selectedId: string | null = null) {
  return toScene(CHART, {
    frame: FRAME,
    driveRangeLy: 50,
    selectedId,
    destinationId: null,
    features: features.features,
  });
}

describe("toScene with features (plan 09, P09.T41.a)", () => {
  it("draws each feature as the Greek cross at its centre, at the largest size, labelled by kind", () => {
    const features = featuresOf({ features: [OPEN_CLUSTER, STAR_FORMING_REGION, ASSOCIATION] });

    const scene = sceneWith(features);

    expect(
      scene.features?.map((mark) => [mark.id, mark.shape, mark.sizeClass, mark.status, mark.label]),
    ).toEqual([
      [`feature:${OPEN_CLUSTER.id}`, "greek-cross", FEATURE_SIZE_CLASS, "plain", "OC GPF F207"],
      [`feature:${STAR_FORMING_REGION.id}`, "greek-cross", 4, "plain", "SFR GPG F336"],
      [`feature:${ASSOCIATION.id}`, "greek-cross", 4, "plain", "OB GPG F530"],
    ]);
    expect(scene.features?.[0]?.position).toEqual(features.features[0]?.relLy);
    expect(scene.features?.[0]?.radius).toBe(OPEN_CLUSTER.extent_ly);
  });

  it("labels the centre by its designation alone, its kind's own name", () => {
    const scene = sceneWith(featuresOf({ features: [GALACTIC_CENTRE] }));

    expect(scene.features?.map((mark) => mark.label)).toEqual(["CENTRE"]);
  });

  it("ranks every feature's label above every system's, the heavier first", () => {
    const scene = sceneWith(featuresOf({ features: [OPEN_CLUSTER, ASSOCIATION] }));
    const [cluster, association] = scene.features ?? [];
    const heaviestSystem = Math.max(...scene.points.map((mark) => mark.labelPriority));

    expect(cluster?.labelPriority ?? 0).toBeGreaterThan(Math.max(heaviestSystem, 150));
    expect(association?.labelPriority ?? 0).toBeGreaterThan(cluster?.labelPriority ?? 0);
  });

  it("draws a nursery's superbubble as a shell at its true radius about the feature", () => {
    const features = featuresOf({ features: [OPEN_CLUSTER, STAR_FORMING_REGION] });

    const scene = sceneWith(features);

    expect(scene.shells).toEqual([
      {
        id: `bubble:${OPEN_CLUSTER.id}`,
        centre: features.features[0]?.relLy,
        radius: OPEN_CLUSTER.bubble_radius_ly,
        label: "SUPERBUBBLE GPF F207",
      },
    ]);
  });

  it("selects a feature's mark by its own ID, apart from any system's", () => {
    const scene = sceneWith(
      featuresOf({ features: [OPEN_CLUSTER] }),
      featureMarkId(OPEN_CLUSTER.id),
    );

    expect(scene.selectedId).toBe(`feature:${OPEN_CLUSTER.id}`);
    expect(scene.points.map((mark) => mark.id)).not.toContain(scene.selectedId);
  });

  it("adds no features or shells to a chart without them", () => {
    const scene = toScene(CHART, {
      frame: FRAME,
      driveRangeLy: 50,
      selectedId: null,
      destinationId: null,
    });

    expect(scene.features).toBeUndefined();
    expect(scene.shells).toBeUndefined();
  });
});

describe("a chart's selection by mark", () => {
  it("reads a feature's mark as that feature, and any other mark as its system", () => {
    expect(selectionOfMark(featureMarkId(OPEN_CLUSTER.id))).toEqual({
      kind: "feature",
      id: OPEN_CLUSTER.id,
    });
    expect(selectionOfMark(OPEN_CLUSTER.id)).toEqual({ kind: "system", id: OPEN_CLUSTER.id });
  });

  it("gives each selection back its own mark's ID", () => {
    expect(selectionMarkId({ kind: "feature", id: OPEN_CLUSTER.id })).toBe(
      featureMarkId(OPEN_CLUSTER.id),
    );
    expect(selectionMarkId({ kind: "system", id: OPEN_CLUSTER.id })).toBe(OPEN_CLUSTER.id);
  });
});

describe("featureDataFault", () => {
  it("finds nothing wrong with the golden's answer", () => {
    expect(featureDataFault(featuresOf())).toBeNull();
  });

  it.each([
    ["an extent", { extent_ly: Number.NaN }],
    ["a mass", { mass_msun: -1 }],
    ["an expected count", { expected_members: Number.POSITIVE_INFINITY }],
    ["an age", { age_myr: Number.NaN }],
    ["a bubble", { bubble_radius_ly: -5 }],
  ])("reports a feature with %s it cannot draw or read", (_, change) => {
    const features = featuresOf({ features: [{ ...OPEN_CLUSTER, ...change }] });

    expect(featureDataFault(features)).toBe("feature unusable");
  });

  it("reports a radius it cannot draw", () => {
    expect(featureDataFault(featuresOf({ radiusLy: 0 }))).toBe("query radius unusable");
  });
});
