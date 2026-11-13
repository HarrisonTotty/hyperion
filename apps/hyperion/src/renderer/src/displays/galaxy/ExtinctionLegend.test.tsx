import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { buildRamp, parseHexColour } from "../../lib/galaxy/ramp";
import { ExtinctionLegend } from "./ExtinctionLegend";

// The guide's --surface-0 and --text; literals are allowed in tests only.
const RAMP = buildRamp(parseHexColour("#05080d"), parseHexColour("#c8d6e5"));

function renderLegend(floorLog10Mag: number, ceilingLog10Mag: number): void {
  render(
    <ExtinctionLegend
      ramp={RAMP}
      floorLog10Mag={floorLog10Mag}
      ceilingLog10Mag={ceilingLog10Mag}
    />,
  );
}

describe("ExtinctionLegend", () => {
  it("labels every whole decade of magnitudes as a plain decimal", () => {
    renderLegend(-2, 1);

    for (const tick of ["0.01", "0.1", "1", "10"]) {
      expect(screen.getByText(tick)).toBeInTheDocument();
    }
  });

  it("names its bar's range in words, to two decimals", () => {
    renderLegend(-2, 1);

    expect(
      screen.getByRole("img", {
        name: "Extinction legend, log scale from 0.01 magnitudes to 10.00 magnitudes",
      }),
    ).toBeInTheDocument();
  });

  it("states the title, the unit, the log scale and the floor", () => {
    renderLegend(-2, 0.32);

    expect(screen.getByText("EXTINCTION A(V)")).toBeInTheDocument();
    expect(screen.getByText("mag")).toBeInTheDocument();
    expect(screen.getByText("LOG SCALE")).toBeInTheDocument();
    expect(screen.getByText("FLOOR 0.01 mag: AT OR BELOW SHOWN AS BACKGROUND")).toBeInTheDocument();
  });

  it("says so when nothing lies above the floor", () => {
    renderLegend(-2, -2);

    expect(screen.getByText("NOTHING ABOVE FLOOR 0.01 mag")).toBeInTheDocument();
    expect(
      screen.getByRole("img", {
        name: "Extinction legend, log scale: no extinction above the floor",
      }),
    ).toBeInTheDocument();
  });
});
