import { universeTimeFromYears, type UniverseTime } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { localFrameAt } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import { type ChartSystem, CLOCK_WINDOW_YR, type QueryMode } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { EMITTED_WIDTH_CH } from "../../lib/galaxy/observation";
import type { SystemModel } from "../../lib/system/model";
import { toSystemModel } from "../../lib/system/wire";
import { anObservation, aSystemsInRange, type RelativeSystemSpec } from "../../test/galaxyFixtures";
import { stylesheetRule } from "../../test/stylesheet";
import { aRedSupergiant, aSingleStarSummary } from "../../test/systemFixtures";
import { TIME_WIDTH_CH } from "./chartReadings";
import { AS_OBSERVED, CURVATURE_BOUND_LABEL, EMITTED_LABEL, SystemReadout } from "./SystemReadout";

const CENTRE = [26_000, 0, 0] as const;
const FRAME = localFrameAt(vec3(CENTRE[0], CENTRE[1], CENTRE[2]));
const OBSERVED: QueryMode = { kind: "observed", observerLy: [26_000, 0, 0] };
const NOW: QueryMode = { kind: "now" };
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
  timeYr = CHART_TIME_YR,
  stale = false,
) {
  render(
    <SystemReadout
      system={system}
      frame={FRAME}
      driveRangeLy={50}
      timeYr={timeYr}
      distanceDecimals={2}
      stars={stars}
      stale={stale}
      starsStale={false}
      starsAt={starsAt}
      extinction={null}
    />,
  );
  return screen.getByRole("status", { name: "Selected system" });
}

/**
 * A system seen 229,012.5 years back from the chart's +12.50, so emitted at -229,000.00, an
 * 11-character time, and 22.93 Myr less its light's age, 22.7 Myr, old then.
 */
function anOldLightSystem(): ChartSystem {
  return anObservedSystem({
    ageMyr: 22.929_012_5,
    observed: {
      lightAgeYr: 229_012.5,
      apparentRelLy: [-3.01, 4.02, 0.005],
      boundLy: 1e-5,
      boundArcsec: 1.5e-3,
    },
  });
}

/** The `AGE` reading's value element, its `dd`. */
function ageOf(readout: HTMLElement): Element {
  const value = within(readout).getByText("AGE", { exact: true }).nextElementSibling;
  if (value === null) {
    throw new Error("AGE has no value");
  }
  return value;
}

/** The fixed slot the age's time stands in, which there must be. */
function timeSlotOf(age: Element): HTMLElement {
  const time = age.querySelector(".system-readout__value--fixed");
  if (!(time instanceof HTMLElement)) {
    throw new Error("the age's time stands in no fixed slot");
  }
  return time;
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
    expect(valueOf(readout, "AGE")).toBe("0.100 Myr· AT UT -49,987.50 yr");
  });

  it("reads NOT YET FORMED for a system younger than its light", () => {
    const readout = renderReadout(anObservedSystem({ ageMyr: 0.01 }));

    expect(valueOf(readout, "AGE")).toBe("NOT YET FORMED· AT UT -49,987.50 yr");
  });

  it("takes the readout's row for the age as observed", () => {
    const age = ageOf(renderReadout(anOldLightSystem()));

    expect(age.textContent).toBe("22.7 Myr· AT UT -229,000.00 yr");
    expect(age).toHaveClass("system-readout__wide");
  });

  it("stands the age's emitted time in a slot that holds it", () => {
    const time = ageOf(renderReadout(anOldLightSystem())).querySelector(
      ".system-readout__value--fixed",
    );
    if (!(time instanceof HTMLElement)) {
      throw new Error("the age's time stands in no fixed slot");
    }

    expect(time.style.minWidth).toBe(`${String(EMITTED_WIDTH_CH)}ch`);
    expect(time.textContent.length).toBeLessThanOrEqual(EMITTED_WIDTH_CH);
  });

  it.each([
    ["as observed", anOldLightSystem, "UT -229,000.00 yr"],
    ["in now", () => aSystem(), "UT +12.50 yr"],
  ] as const)(
    "keeps the age's time whole with its system's label and its unit %s",
    (_, system, whole) => {
      expect(
        ageOf(renderReadout(system())).querySelector(".system-readout__whole")?.textContent,
      ).toBe(whole);
    },
  );

  it("keeps a time whole with its label and unit, in the stylesheet jsdom does not apply", () => {
    expect(stylesheetRule(".system-readout__whole")).toMatch(/white-space: nowrap;/u);
  });

  it("sets the age's time as a value, in --text and untracked, so that its slot in ch holds it", () => {
    const rule = stylesheetRule(".system-readout__beside .system-readout__value");

    expect(rule).toMatch(/color: var\(--text\);/u);
    expect(rule).toMatch(/letter-spacing: 0;/u);
  });

  it("mutes the age's time with the rest of a stale readout", () => {
    expect(
      stylesheetRule(
        ".systems-panel--stale .system-readout__beside .system-readout__value,\n" +
          ".stale .system-readout__beside .system-readout__value",
      ),
    ).toMatch(/color: var\(--text-muted\);/u);
  });

  it("takes the readout's row for the age in now, its time in the chart time's slot", () => {
    const age = ageOf(renderReadout(aSystem({}, NOW)));

    expect([age.textContent, age.classList.contains("system-readout__wide")]).toEqual([
      "0.150 Myr· AT UT +12.50 yr",
      true,
    ]);
    expect(timeSlotOf(age).style.minWidth).toBe(`${String(TIME_WIDTH_CH)}ch`);
  });

  it.each([
    [-CLOCK_WINDOW_YR, "-1000.00"],
    [CLOCK_WINDOW_YR, "+1000.00"],
  ] as const)(
    "holds the chart time at the clock window's edge, %s yr, in its slot in now",
    (timeYr, text) => {
      const time = timeSlotOf(ageOf(renderReadout(aSystem({}, NOW), null, null, timeYr)));

      expect([time.textContent, time.textContent.length <= TIME_WIDTH_CH]).toEqual([text, true]);
    },
  );

  it("takes the readout's row for NOT YET FORMED as observed", () => {
    expect(ageOf(renderReadout(anObservedSystem({ ageMyr: 0.01 })))).toHaveClass(
      "system-readout__wide",
    );
  });

  it("stands the time of a row without its observation, asked observed, in now's slot", () => {
    const time = timeSlotOf(ageOf(renderReadout(aSystem())));

    expect([time.textContent, time.style.minWidth]).toEqual([
      "+12.50",
      `${String(TIME_WIDTH_CH)}ch`,
    ]);
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
    expect(valueOf(readout, "AGE")).toBe("0.150 Myr· AT UT +12.50 yr");
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

describe("SystemReadout as observed, stale (plan 05, P05.T14)", () => {
  it("gives AGE its two marks in order, the age's after its unit and the emitted time's after yr", () => {
    const age = ageOf(renderReadout(anOldLightSystem(), null, null, CHART_TIME_YR, true));

    expect(age.textContent).toBe("22.7 Myr Sstale· AT UT -229,000.00 yr Sstale");
    expect(age.querySelector(".system-readout__whole")).toHaveTextContent(
      /^UT -229,000\.00 yr S\s*stale$/u,
    );
  });

  it("marks the emitted time and not NOT YET FORMED, a status in the age's place", () => {
    const age = ageOf(
      renderReadout(anObservedSystem({ ageMyr: 0.01 }), null, null, CHART_TIME_YR, true),
    );

    expect(age.textContent).toBe("NOT YET FORMED· AT UT -49,987.50 yr Sstale");
    // The status is muted with its stale answer all the same, though it takes no mark.
    expect(age.querySelector(".stale")?.textContent).toMatch(/^NOT YET FORMED·/u);
  });

  it("keeps an empty slot after every value of the observed rows while live", () => {
    const readout = renderReadout(anObservedSystem());

    // The light's age, the emitted time, the curvature bound and the velocity's em dashes among
    // them: a reading that forgot its flag, its slot and its mark with it, fails here.
    const values = readout.querySelectorAll("dd");
    expect(values.length).toBeGreaterThan(0);
    for (const value of values) {
      const slots = value.querySelectorAll(".stale-slot");
      expect([value.previousElementSibling?.textContent, slots.length > 0]).toEqual([
        value.previousElementSibling?.textContent,
        true,
      ]);
      for (const slot of slots) {
        expect(slot).toBeEmptyDOMElement();
      }
    }
  });

  it("keeps both slots of the age empty while live, so that the mark moves nothing", () => {
    const slots = ageOf(renderReadout(anOldLightSystem())).querySelectorAll(".stale-slot");

    expect(slots).toHaveLength(2);
    for (const slot of slots) {
      expect(slot).toBeEmptyDOMElement();
    }
  });

  it("marks each part of the curvature bound, and the light's age and when it left", () => {
    const readout = renderReadout(anObservedSystem(), null, null, CHART_TIME_YR, true);

    expect(valueOf(readout, CURVATURE_BOUND_LABEL)).toBe("0.632 AU Sstale · 0.00150″ Sstale");
    expect(valueOf(readout, "LIGHT AGE")).toBe("50,000.00 yr Sstale");
    expect(valueOf(readout, EMITTED_LABEL)).toBe("-49,987.50 yr Sstale");
    expect(valueOf(readout, "DIST")).toBe("~5.00 ly Sstale");
  });

  it("puts no S after AS OBSERVED, a state, nor after the velocity's em dashes", () => {
    const readout = renderReadout(anObservedSystem(), null, null, CHART_TIME_YR, true);

    expect(within(readout).getByText(AS_OBSERVED)).toHaveTextContent(/^AS OBSERVED$/u);
    expect(
      screen
        .getAllByText(AS_OBSERVED)
        .map((line) => line.textContent)
        .every((text) => text === AS_OBSERVED),
    ).toBe(true);
    expect(valueOf(readout, "VEL")).toBe("—");
  });
});
