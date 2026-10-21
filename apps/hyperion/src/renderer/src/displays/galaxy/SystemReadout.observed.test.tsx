import { universeTimeFromYears, type UniverseTime } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { localFrameAt } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import type { ChartSystem, QueryMode } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import type { SystemModel } from "../../lib/system/model";
import { toSystemModel } from "../../lib/system/wire";
import { anObservation, aSystemsInRange, type RelativeSystemSpec } from "../../test/galaxyFixtures";
import { aRedSupergiant, aSingleStarSummary } from "../../test/systemFixtures";
import { AS_OBSERVED, CURVATURE_BOUND_LABEL, EMITTED_LABEL, SystemReadout } from "./SystemReadout";

const CENTRE = [26_000, 0, 0] as const;
const FRAME = localFrameAt(vec3(CENTRE[0], CENTRE[1], CENTRE[2]));
const OBSERVED: QueryMode = { kind: "observed", observerLy: [26_000, 0, 0] };
const CHART_TIME_YR = 12.5;

/** One system of layer E 3 ly coreward and 4 ly spinward-ish of the centre, as the adapter builds it. */
function aSystem(spec: Partial<RelativeSystemSpec> = {}, mode: QueryMode = OBSERVED): ChartSystem {
  const [system] = toChartResult(
    aSystemsInRange({
      centreLy: CENTRE,
      timeYr: CHART_TIME_YR,
      systems: [{ relLy: [-3, 4, 0], layer: "e", ageMyr: 0.15, ...spec }],
    }),
    mode,
  ).systems;
  if (system === undefined) {
    throw new Error("the fixture built no system");
  }
  return system;
}

/** A system seen 50,000 years back, its stated bound 1E-5 ly and 1.5E-3″. */
function anObservedSystem(spec: Partial<RelativeSystemSpec> = {}): ChartSystem {
  return aSystem({
    observed: {
      lightAgeYr: 50_000,
      apparentRelLy: [-3.01, 4.02, 0.005],
      boundLy: 1e-5,
      boundArcsec: 1.5e-3,
    },
    ...spec,
  });
}

function renderReadout(
  system: ChartSystem,
  stars: SystemModel | null = null,
  starsAt: UniverseTime | null = null,
) {
  render(
    <SystemReadout
      system={system}
      frame={FRAME}
      driveRangeLy={50}
      timeYr={CHART_TIME_YR}
      distanceDecimals={2}
      stars={stars}
      starsStale={false}
      starsAt={starsAt}
      extinction={null}
    />,
  );
  return screen.getByRole("status", { name: "Selected system" });
}

/** The `nth` value read beside a label in the readout. */
function valueOf(readout: HTMLElement, label: string, nth = 0): string | null {
  const term = within(readout).getAllByText(label, { exact: true })[nth];
  const value = term?.nextElementSibling ?? null;
  if (value === null) {
    throw new Error(`${label} has no value`);
  }
  return value.textContent;
}

describe("SystemReadout as observed (plan 12, P12.T11)", () => {
  it("reads AS OBSERVED, with the light's age and when it left, in universe time", () => {
    const readout = renderReadout(anObservedSystem());

    expect(within(readout).getByText("AS OBSERVED")).toBeInTheDocument();
    expect(valueOf(readout, "LIGHT AGE")).toBe("50,000.00 yr");
    expect(EMITTED_LABEL).toBe("EMITTED UT");
    expect(valueOf(readout, EMITTED_LABEL)).toBe("-49,987.50 yr");
  });

  it("keeps AS OBSERVED on show above the scrolling readings, and says it with the selection", () => {
    const readout = renderReadout(anObservedSystem());
    const readings = screen.getByRole("region", { name: "Readings" });

    const shown = screen.getAllByText(AS_OBSERVED).filter((line) => !readings.contains(line));
    expect(shown).toHaveLength(1);
    expect(within(readout).getByText(AS_OBSERVED)).toHaveClass("visually-hidden");
  });

  it("gives the age when the light left, at the time it left, as an Observed value", () => {
    const readout = renderReadout(anObservedSystem());

    // 0.15 Myr now, less 50,000 years: 0.100 Myr, with no `~`.
    expect(valueOf(readout, "AGE")).toBe("0.100 MyrAT UT -49,987.50 yr");
  });

  it("reads NOT YET FORMED for a system younger than its light", () => {
    const readout = renderReadout(anObservedSystem({ ageMyr: 0.01 }));

    expect(valueOf(readout, "AGE")).toBe("NOT YET FORMEDAT UT -49,987.50 yr");
  });

  it("states the present position as Estimated, with the stated bound after it", () => {
    const readout = renderReadout(anObservedSystem());

    expect(valueOf(readout, "DIST")).toBe("~5.00 ly");
    expect(valueOf(readout, "NORTH")).toBe("~+0.00 ly");
    expect(valueOf(readout, "COREWARD")).toBe("~+3.00 ly");
    expect(valueOf(readout, "RADIUS", 0)).toMatch(/^~25,997\.0 ly$/u);
    expect(valueOf(readout, "HEIGHT")).toBe("~+0.0 ly");
    expect(valueOf(readout, "ANGLE")).toMatch(/^~\d{3}\.\d°$/u);
    // A bound, not a `±`: a reading of its own, in the distance ladder and in arcseconds.
    expect(valueOf(readout, CURVATURE_BOUND_LABEL)).toBe("0.632 AU · 0.00150″");
    expect(readout).not.toHaveTextContent("±");
  });

  it("reads a bound of zero, an orbit followed in full, as 0", () => {
    const readout = renderReadout(
      anObservedSystem({
        observed: {
          lightAgeYr: 50_000,
          apparentRelLy: [-3, 4, 0],
          boundLy: 0,
          boundArcsec: 0,
        },
      }),
    );

    expect(valueOf(readout, CURVATURE_BOUND_LABEL)).toBe("0 ly · 0″");
  });

  it("shows no velocity, which is present state the crew cannot know", () => {
    const readout = renderReadout(anObservedSystem());

    expect(valueOf(readout, "VEL")).toBe("—");
    expect(valueOf(readout, "COREWARD", 1)).toBe("—");
    expect(valueOf(readout, "SPINWARD", 1)).toBe("—");
    expect(valueOf(readout, "NORTH", 1)).toBe("—");
    expect(within(readout).queryByText("km/s")).toBeNull();
  });

  it("reads a row without its observation as in now, whatever was asked", () => {
    const readout = renderReadout(aSystem());

    expect(within(readout).queryByText("AS OBSERVED")).toBeNull();
    expect(within(readout).queryByText("LIGHT AGE")).toBeNull();
    expect(within(readout).queryByText(CURVATURE_BOUND_LABEL)).toBeNull();
    expect(valueOf(readout, "DIST")).toBe("5.00 ly");
    expect(valueOf(readout, "AGE")).toBe("0.150 MyrAT UT +12.50 yr");
    expect(valueOf(readout, "VEL")).toMatch(/^\d+\.\d km\/s$/u);
  });

  it("reads the stars as the summary asked in observed mode gives them, a death counted from the light's leaving", () => {
    const emitted = universeTimeFromYears(CHART_TIME_YR - 50_000);
    // Dead 49,000 years before the chart's time, so 1,000 years after its light left.
    const summary = aSingleStarSummary(
      aRedSupergiant({ death_time: universeTimeFromYears(CHART_TIME_YR - 49_000) }),
    );
    const model = toSystemModel(
      {
        ...summary,
        time: universeTimeFromYears(CHART_TIME_YR),
        age_myr: 0.1,
        observed: anObservation({
          apparentLy: [25_997, 4, 0],
          lightAgeYr: 50_000,
          timeYr: CHART_TIME_YR,
        }),
      },
      "H7K 4C0RFZ E-1",
    );
    if (model.kind !== "ok") {
      throw new Error(`the summary is unusable: ${model.fault}`);
    }

    const readout = renderReadout(anObservedSystem(), model.model, emitted);

    expect(within(readout).getByRole("heading", { name: "STAR A" })).toBeInTheDocument();
    expect(valueOf(readout, "KIND")).toBe("SUPERGIANT");
    expect(valueOf(readout, "DIES IN")).toBe("1000 yr");
    expect(within(readout).queryByText("DIED")).toBeNull();
  });
});
