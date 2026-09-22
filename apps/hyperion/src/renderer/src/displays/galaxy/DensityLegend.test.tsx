import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { buildRamp, parseHexColour, rampColour } from "../../lib/galaxy/ramp";
import { DensityLegend } from "./DensityLegend";

// The guide's --surface-0 and --text; literals are allowed in tests only.
const RAMP = buildRamp(parseHexColour("#05080d"), parseHexColour("#c8d6e5"));

function renderLegend(floorLog10PerLy2: number, ceilingLog10PerLy2: number): void {
  render(
    <DensityLegend
      ramp={RAMP}
      floorLog10PerLy2={floorLog10PerLy2}
      ceilingLog10PerLy2={ceilingLog10PerLy2}
    />,
  );
}

describe("DensityLegend", () => {
  it("labels every whole decade from floor to ceiling in E notation", () => {
    renderLegend(-4, 2.3);

    for (const tick of ["1E-4", "1E-3", "1E-2", "1E-1", "1E0", "1E1", "1E2"]) {
      expect(screen.getByText(tick)).toBeInTheDocument();
    }
    expect(screen.queryByText("1E3")).not.toBeInTheDocument();
    expect(screen.queryByText("1E-5")).not.toBeInTheDocument();
  });

  it("puts each decade's label where it falls on the bar", () => {
    renderLegend(-4, 2);

    expect(screen.getByText("1E-4")).toHaveStyle({ left: "0%" });
    expect(screen.getByText("1E-1")).toHaveStyle({ left: "50%" });
    expect(screen.getByText("1E2")).toHaveStyle({ left: "100%" });
  });

  it("states the title, the unit, the log scale and the floor", () => {
    renderLegend(-4, 2.3);

    expect(screen.getByText("COLUMN DENSITY")).toBeInTheDocument();
    expect(screen.getByText("SYSTEMS/ly²")).toBeInTheDocument();
    expect(screen.getByText("LOG SCALE")).toBeInTheDocument();
    expect(screen.getByText("FLOOR 1.00E-4: AT OR BELOW SHOWN AS BACKGROUND")).toBeInTheDocument();
  });

  it("states a floor between decades as it is", () => {
    renderLegend(-6.3, 0.7);

    expect(screen.getByText("FLOOR 5.01E-7: AT OR BELOW SHOWN AS BACKGROUND")).toBeInTheDocument();
    expect(screen.getByText("1E-6")).toBeInTheDocument();
    expect(screen.getByText("1E0")).toBeInTheDocument();
  });

  it("names the bar with the range and the unit in words", () => {
    renderLegend(-4, 2.3);

    const bar = screen.getByRole("img", { name: /systems per square light-year/ });
    expect(bar).toHaveAccessibleName(
      "Column density legend, log scale from ten to the power -4.0 to ten to the power 2.3 " +
        "systems per square light-year",
    );
  });

  it("draws the bar in 32 steps of the picture's own ramp", () => {
    renderLegend(-4, 2.3);

    const steps = screen.getByRole("img").querySelectorAll("rect");
    expect(steps).toHaveLength(32);
    expect(steps[0]).toHaveAttribute("fill", rampColour(RAMP, 5));
    expect(steps[31]).toHaveAttribute("fill", rampColour(RAMP, 251));
  });

  it("labels every second decade where the bar is too narrow for a label at each", () => {
    // Seven decades across 246 px leave 35 px a decade; `1E-6` and a gap take 44 px at 16 px a rem.
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
      DOMRect.fromRect({ width: 246, height: 20 }),
    );
    renderLegend(-6, 1);

    for (const tick of ["1E-6", "1E-4", "1E-2", "1E0"]) {
      expect(screen.getByText(tick)).toBeInTheDocument();
    }
    for (const tick of ["1E-5", "1E-3", "1E-1", "1E1"]) {
      expect(screen.queryByText(tick)).not.toBeInTheDocument();
    }
    // Every decade keeps its tick; the labelled ones are longer.
    const ticks = Array.from(screen.getByRole("img").querySelectorAll("line"));
    expect(ticks.map((tick) => tick.getAttribute("y2"))).toEqual([
      "16",
      "14",
      "16",
      "14",
      "16",
      "14",
      "16",
      "14",
    ]);
  });

  it("labels every decade where the bar has room for them", () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
      DOMRect.fromRect({ width: 400, height: 20 }),
    );
    renderLegend(-6, 1);

    for (const tick of ["1E-6", "1E-5", "1E-4", "1E-3", "1E-2", "1E-1", "1E0", "1E1"]) {
      expect(screen.getByText(tick)).toBeInTheDocument();
    }
  });

  it("says so when the map holds no systems", () => {
    renderLegend(0, 0);

    expect(screen.getByText("NO SYSTEMS IN MAP")).toBeInTheDocument();
    expect(screen.queryByText("1E0")).not.toBeInTheDocument();
    expect(screen.getByRole("img")).toHaveAccessibleName(
      "Column density legend, log scale: no systems in the map",
    );
  });
});
