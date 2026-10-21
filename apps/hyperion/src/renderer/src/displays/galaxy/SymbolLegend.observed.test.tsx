import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { aCensus } from "../../test/galaxyFixtures";
import { layerBands, OBSERVER_LABEL } from "./chartModel";
import { APPARENT_LEGEND, SymbolLegend } from "./SymbolLegend";

function legend(): HTMLElement {
  return screen.getByRole("group", { name: "Chart legend" });
}

describe("SymbolLegend in observed mode (plan 12, P12.T11)", () => {
  it("names the observer's mark and an apparent position's tick and joiner in words", () => {
    render(<SymbolLegend bands={layerBands(aCensus().layers)} observed />);

    expect(within(legend()).getByText(OBSERVER_LABEL)).toBeInTheDocument();
    expect(within(legend()).getByText(APPARENT_LEGEND)).toBeInTheDocument();
    expect(APPARENT_LEGEND).toBe("TICK APPARENT POSITION, DASHED TO PRESENT");
  });

  it("names neither in now, which draws neither", () => {
    render(<SymbolLegend bands={layerBands(aCensus().layers)} />);

    expect(within(legend()).queryByText(OBSERVER_LABEL)).toBeNull();
    expect(within(legend()).queryByText(APPARENT_LEGEND)).toBeNull();
  });
});
