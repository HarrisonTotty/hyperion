import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ChartSystem } from "../../lib/galaxy/model";
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

function renderReadout(system: ChartSystem | null, { timeYr = 0, driveRangeLy = 50 } = {}) {
  render(
    <SystemReadout
      system={system}
      frame={FRAME}
      driveRangeLy={driveRangeLy}
      timeYr={timeYr}
      distanceDecimals={2}
    />,
  );
  return screen.getByRole("status", { name: "Selected system" });
}

/** The value beside a label in the readout. */
function valueOf(readout: HTMLElement, label: string): string | null {
  const term = within(readout).getByText(label);
  const value = term.nextElementSibling;
  if (value === null) {
    throw new Error(`${label} has no value`);
  }
  return value.textContent;
}

describe("SystemReadout", () => {
  it("is a status region named for the selection, read as a whole", () => {
    const readout = renderReadout(aSystem([1, 0, 0]));

    // Not an `output`, whose content model is phrasing content and cannot hold the readout's `dl`
    // (the orchestrator's ruling 14); atomic, so a new selection is read as one reading.
    expect(readout.tagName.toLowerCase()).toBe("div");
    expect(readout).toHaveAttribute("aria-atomic", "true");
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
});
