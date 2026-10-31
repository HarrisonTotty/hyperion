import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { toBlackHole, toChartFeatures, toFeatureDetail } from "../../lib/galaxy/featureWire";
import type { BlackHoleModel, ChartFeature, FeatureDetailModel } from "../../lib/galaxy/model";
import {
  aBlackHoleState,
  aCentreDetail,
  aCloudDetail,
  aClusterDetail,
  aFeaturesInRange,
  ASSOCIATION,
  GALACTIC_CENTRE,
  MOLECULAR_CLOUD,
  OPEN_CLUSTER,
} from "../../test/featureFixtures";
import type { FeatureSummary } from "@hyperion/protocol";
import { FeatureReadout } from "./FeatureReadout";
import { type ChartDistanceUnit, distanceUnitFor } from "./chartModel";

/** One of the golden's features as the chart holds it, from the golden's centre. */
function aFeature(
  summary: FeatureSummary,
  centreLy: readonly [number, number, number] = [0, 26_000, 0],
): ChartFeature {
  const [feature] = toChartFeatures(aFeaturesInRange({ centreLy, features: [summary] })).features;
  if (feature === undefined) {
    throw new Error("the fixture built no feature");
  }
  return feature;
}

interface RenderOptions {
  readonly detail?: FeatureDetailModel | null;
  readonly blackHole?: BlackHoleModel | null;
  readonly stale?: boolean;
  readonly detailStale?: boolean;
  readonly distanceUnit?: ChartDistanceUnit;
}

function renderReadout(
  feature: ChartFeature,
  {
    detail = null,
    blackHole = null,
    stale = false,
    detailStale = false,
    distanceUnit = distanceUnitFor(50),
  }: RenderOptions = {},
) {
  render(
    <FeatureReadout
      feature={feature}
      timeYr={0}
      distanceUnit={distanceUnit}
      stale={stale}
      detail={detail}
      detailStale={detailStale}
      blackHole={blackHole}
      blackHoleStale={false}
    />,
  );
  return screen.getByRole("status", { name: "Selected feature" });
}

/** The value beside the `nth` reading of a label, or `undefined` where there is none. */
function valueOf(readout: HTMLElement, label: string, nth = 0): string | null | undefined {
  const term = within(readout).queryAllByText(label, { selector: "dt" })[nth];
  return term === undefined ? undefined : (term.nextElementSibling?.textContent ?? null);
}

describe("FeatureReadout (plan 09, P09.T41.c)", () => {
  it("reads the golden's open cluster: designation, kind, distance, extent, age, mass, members and emission", () => {
    const cluster = aFeature(OPEN_CLUSTER);
    const readout = renderReadout(cluster);

    expect(valueOf(readout, "DESIG")).toBe("GPF F207");
    expect(valueOf(readout, "ID")).toBe("E859E06780000000");
    expect(valueOf(readout, "KIND")).toBe("OPEN CLUSTER");
    expect(valueOf(readout, "DIST")).toBe(`${cluster.distanceLy.toFixed(2)} ly`);
    expect(valueOf(readout, "EXTENT")).toBe("26.9 ly");
    expect(valueOf(readout, "AGE")).toBe("8.41 Myr· AT UT +0.00 yr");
    expect(valueOf(readout, "MASS")).toBe("268 M");
    // The model's expected count, Estimated: never a count of rows.
    expect(valueOf(readout, "MEMBERS")).toBe("~459");
    expect(valueOf(readout, "EMISSION")).toBe("NONE");
    expect(valueOf(readout, "SUPERBUBBLE")).toBe("379 ly");
  });

  it("reads the zero members of a feature with no cluster model Missing, and its emission class by the guide's name", () => {
    const readout = renderReadout(aFeature(ASSOCIATION));

    expect(valueOf(readout, "KIND")).toBe("OB ASSOCIATION");
    // The zero the server sends for a feature it gives no cluster model, not none (P09.T40.b).
    expect(valueOf(readout, "MEMBERS")).toBe("—");
    expect(valueOf(readout, "EMISSION")).toBe("H II");
  });

  it("reads a molecular cloud's age, which it has none of, as the em dash", () => {
    const readout = renderReadout(aFeature(MOLECULAR_CLOUD));

    expect(valueOf(readout, "AGE")).toBe("—");
    expect(valueOf(readout, "EMISSION")).toBe("DARK CLOUD");
    expect(valueOf(readout, "SUPERBUBBLE")).toBe("NONE");
  });

  it("adds no structure until the detail answers", () => {
    renderReadout(aFeature(OPEN_CLUSTER));

    expect(screen.queryByRole("heading", { name: "STRUCTURE" })).not.toBeInTheDocument();
  });

  it("reads a cluster's structure once the detail answers", () => {
    const cluster = aFeature(OPEN_CLUSTER);
    const readout = renderReadout(cluster, { detail: toFeatureDetail(aClusterDetail()) });

    expect(within(readout).getByRole("heading", { name: "STRUCTURE" })).toBeInTheDocument();
    expect(valueOf(readout, "CORE RADIUS")).toBe("0.916 ly");
    expect(valueOf(readout, "HALF-MASS RADIUS")).toBe("2.11 ly");
    expect(valueOf(readout, "TIDAL RADIUS")).toBe("26.9 ly");
    expect(valueOf(readout, "ESCAPE SPEED")).toBe("2.06 km/s");
    expect(valueOf(readout, "RELAXATION TIME")).toBe("3.89 Myr");
    expect(valueOf(readout, "CORE COLLAPSED")).toBe("NO");
    expect(valueOf(readout, "BLACK HOLES")).toBe("NONE");
    expect(valueOf(readout, "MEMBER CLASSES")).toBe("2");
  });

  it("reads a cloud's core and central density", () => {
    const readout = renderReadout(aFeature(MOLECULAR_CLOUD), {
      detail: toFeatureDetail(aCloudDetail()),
    });

    expect(valueOf(readout, "CORE RADIUS")).toBe("15.8 ly");
    expect(valueOf(readout, "CENTRAL DENSITY")).toBe("1.07E3 /cm³");
  });

  it("reads no structure answered for another feature", () => {
    const readout = renderReadout(aFeature(ASSOCIATION), {
      detail: toFeatureDetail(aClusterDetail()),
    });

    expect(within(readout).queryByRole("heading", { name: "STRUCTURE" })).not.toBeInTheDocument();
  });

  it("reads the centre's radii and its black hole's mass, luminosity and disruptions", () => {
    const readout = renderReadout(aFeature(GALACTIC_CENTRE, [0, 0, 0]), {
      detail: toFeatureDetail(aCentreDetail()),
      blackHole: toBlackHole(aBlackHoleState()),
    });

    expect(valueOf(readout, "KIND")).toBe("GALACTIC CENTRE");
    expect(valueOf(readout, "AGE")).toBe("—");
    expect(valueOf(readout, "MEMBERS")).toBe("~9.14E7");
    expect(valueOf(readout, "INFLUENCE RADIUS")).toBe("111 ly");
    expect(valueOf(readout, "LOSS-CONE RADIUS")).toBe("8.52E-5 ly");
    expect(within(readout).getByRole("heading", { name: "BLACK HOLE" })).toBeInTheDocument();
    expect(valueOf(readout, "MASS", 1)).toBe("8.42E7 M");
    expect(valueOf(readout, "X-RAY LUMINOSITY")).toBe("1.69E28 W");
    expect(valueOf(readout, "DISRUPTIONS")).toBe("2");
    expect(valueOf(readout, "LAST DISRUPTION")).toBe("UT -43,495.65 yr");
  });

  it("reads the centre's distance in AU on a chart of its fine steps (plan 09, P09.T42)", () => {
    // 0.0042 ly is 265.61 AU.
    const readout = renderReadout(
      { ...aFeature(GALACTIC_CENTRE, [0, 0, 0]), distanceLy: 0.0042 },
      { distanceUnit: distanceUnitFor(0.005) },
    );

    expect(valueOf(readout, "DIST")).toBe("265.6 AU");
    // A feature's radius is a size, to three figures in light-years, not a distance on the chart.
    expect(valueOf(readout, "EXTENT")).toMatch(/ ly$/u);
  });

  it("reads the black hole's luminosity Missing from a server that does not send it, its mass as before", () => {
    const { luminosity: _luminosity, ...unlit } = aBlackHoleState();
    const readout = renderReadout(aFeature(GALACTIC_CENTRE, [0, 0, 0]), {
      detail: toFeatureDetail(aCentreDetail()),
      blackHole: toBlackHole(unlit),
    });

    expect(within(readout).getByRole("heading", { name: "BLACK HOLE" })).toBeInTheDocument();
    expect(valueOf(readout, "MASS", 1)).toBe("8.42E7 M");
    expect(valueOf(readout, "X-RAY LUMINOSITY")).toBe("—");
    expect(valueOf(readout, "DISRUPTIONS")).toBe("2");
  });

  it("mutes a stale structure and trails its S, the summary's values staying live", () => {
    const readout = renderReadout(aFeature(OPEN_CLUSTER), {
      detail: toFeatureDetail(aClusterDetail()),
      detailStale: true,
    });

    expect(valueOf(readout, "CORE RADIUS")).toMatch(/^0\.916 ly S/u);
    expect(valueOf(readout, "EXTENT")).toBe("26.9 ly");
  });
});
