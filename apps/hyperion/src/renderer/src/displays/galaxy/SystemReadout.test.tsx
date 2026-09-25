import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ChartSystem } from "../../lib/galaxy/model";
import type { SystemModel } from "../../lib/system/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { localFrameAt } from "../../spatial/frame";
import { vec3 } from "../../spatial/vec3";
import { aSystemsInRange } from "../../test/galaxyFixtures";
import { SystemReadout } from "./SystemReadout";

const CENTRE = [26_000, 0, 0] as const;
const FRAME = localFrameAt(vec3(CENTRE[0], CENTRE[1], CENTRE[2]));

/** One system at an offset from the chart centre, as the adapter builds it. */
function aSystem(
  relLy: readonly [number, number, number],
  overrides: Partial<Parameters<typeof aSystemsInRange>[0]> = {},
): ChartSystem {
  const [system] = toChartResult(
    aSystemsInRange({ centreLy: CENTRE, systems: [{ relLy, layer: "c" }], ...overrides }),
  ).systems;
  if (system === undefined) {
    throw new Error("the fixture built no system");
  }
  return system;
}

function renderReadout(
  system: ChartSystem | null,
  { timeYr = 0, driveRangeLy = 50 } = {},
  stars: SystemModel | null = null,
) {
  render(
    <SystemReadout
      system={system}
      frame={FRAME}
      driveRangeLy={driveRangeLy}
      timeYr={timeYr}
      distanceDecimals={2}
      stars={stars}
      starsStale={false}
    />,
  );
  return screen.getByRole("status", { name: "Selected system" });
}

/**
 * The value beside a label in the readout: the `nth` reading of that name, since the velocity's
 * `COREWARD`, `SPINWARD` and `NORTH` (the second of each) follow the offsets' (the first).
 */
function valueOf(readout: HTMLElement, label: string, nth = 0): string | null {
  const term = within(readout).getAllByText(label)[nth];
  const value = term?.nextElementSibling ?? null;
  if (value === null) {
    throw new Error(`${label} has no value`);
  }
  return value.textContent;
}

describe("SystemReadout", () => {
  it("is read as a whole when the selection changes", () => {
    const readout = renderReadout(aSystem([1, 0, 0]));

    // Atomic, so that a new selection is read as one reading.
    expect(readout).toHaveAttribute("aria-atomic", "true");
  });

  it("holds its list in an element whose content may be a list, not in an output", () => {
    const readout = renderReadout(aSystem([1, 0, 0]));

    // A rule of HTML's content models, so held as markup: an `output` holds phrasing content only
    // and cannot hold the readout's `dl` (the orchestrator's ruling 14).
    expect(readout.tagName).toBe("DIV");
  });

  it("names the system and gives its ID in upper case", () => {
    const readout = renderReadout(aSystem([1, 0, 0]));

    expect(valueOf(readout, "DESIG")).toBe("H7K 4C0RFZ C-1");
    expect(valueOf(readout, "ID")).toBe("0000000000000001");
  });

  it("gives the signed offset north of the reference plane, negative below it", () => {
    const readout = renderReadout(aSystem([0, 0, -3.2]));

    expect(valueOf(readout, "NORTH")).toBe("-3.20 ly");
  });

  it("gives the offsets along the named directions at the centre", () => {
    // Coreward is −x and spinward +y at a centre on the +x axis.
    const readout = renderReadout(aSystem([-4, 2, 0]));

    expect(valueOf(readout, "COREWARD")).toBe("+4.00 ly");
    expect(valueOf(readout, "SPINWARD")).toBe("+2.00 ly");
  });

  it("gives the system's own place in the galactic frame", () => {
    const readout = renderReadout(aSystem([4, 0, 12]));

    expect(valueOf(readout, "RADIUS")).toBe("26,004.0 ly");
    expect(valueOf(readout, "ANGLE")).toBe("000.0°");
    expect(valueOf(readout, "HEIGHT")).toBe("+12.0 ly");
  });

  it("groups the galactic radius, angle and height under the heading GALACTIC", () => {
    const readout = renderReadout(aSystem([4, 0, 12]));

    // The guide's § Voice: "A readout groups the three under the heading `GALACTIC`."
    const heading = within(readout).getByRole("heading", { name: "GALACTIC" });
    const group = heading.nextElementSibling;
    expect(group?.tagName).toBe("DL");
    const terms = [...(group?.querySelectorAll("dt") ?? [])].map((term) => term.textContent);
    expect(terms).toEqual(["RADIUS", "ANGLE", "HEIGHT"]);
  });

  it("says in words whether the system is within the drive range", () => {
    const readout = renderReadout(aSystem([60, 0, 0], { radiusLy: 80 }), { driveRangeLy: 50 });

    expect(valueOf(readout, "DRIVE RANGE")).toBe("OUT OF RANGE");
  });

  it("gives the initial mass with the drawn solar mass", () => {
    const readout = renderReadout(aSystem([1, 0, 0]));

    expect(valueOf(readout, "INIT MASS")).toBe("1.63 M");
    expect(within(readout).getByRole("img", { name: "solar masses" })).toBeInTheDocument();
  });

  it("reads an age of 4,600 Myr in gigayears, at the chart time", () => {
    const readout = renderReadout(aSystem([1, 0, 0]), { timeYr: 12.5 });

    expect(valueOf(readout, "AGE")).toBe("4.60 GyrAT UT +12.50 yr");
  });

  it("names the population in upper case", () => {
    const readout = renderReadout(aSystem([1, 0, 0]));

    expect(valueOf(readout, "POPULATION")).toBe("OLD THIN DISC");
  });

  it("reads an em dash for every value with nothing selected", () => {
    const readout = renderReadout(null);

    for (const label of [
      "DESIG",
      "ID",
      "DIST",
      "DRIVE RANGE",
      "NORTH",
      "HEIGHT",
      "INIT MASS",
      "AGE",
    ]) {
      expect(valueOf(readout, label)).toBe("—");
    }
  });

  it("gives the velocity, its speed and its components at the system's own position", () => {
    const readout = renderReadout(
      aSystem([1, 0, 0], {
        systems: [{ relLy: [1, 0, 0], layer: "c", velocityKmS: [3.5, 231.25, -7] }],
      }),
    );

    // At (26,001, 0, 0) spinward is +y and coreward −x: a disc star turning with the galaxy,
    // drifting a little outward and south (plan 08, P08.T7.b).
    expect(valueOf(readout, "VEL")).toBe("231.4 km/s");
    expect(valueOf(readout, "COREWARD", 1)).toBe("-3.5 km/s");
    expect(valueOf(readout, "SPINWARD", 1)).toBe("+231.3 km/s");
    expect(valueOf(readout, "NORTH", 1)).toBe("-7.0 km/s");
    // The offsets keep their own rows.
    expect(valueOf(readout, "COREWARD")).toBe("-1.00 ly");
  });

  it("reads every velocity value as the em dash with nothing selected", () => {
    const readout = renderReadout(null);

    expect(valueOf(readout, "VEL")).toBe("—");
    expect(valueOf(readout, "SPINWARD", 1)).toBe("—");
  });
});
