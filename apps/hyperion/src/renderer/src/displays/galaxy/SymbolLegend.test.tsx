import type { ObjectKindDto } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { starSymbol } from "../../lib/galaxy/starSymbols";
import { aCensus } from "../../test/galaxyFixtures";
import { layerBands } from "./chartModel";
import { SymbolLegend } from "./SymbolLegend";

/** Every kind the wire names, so that the legend is checked against the whole symbol set. */
const EVERY_KIND: ReadonlyArray<ObjectKindDto> = [
  "protostar",
  "pre_main_sequence",
  "dwarf",
  "subgiant",
  "giant",
  "supergiant",
  "wolf_rayet",
  "hot_subdwarf",
  "white_dwarf",
  "neutron_star",
  "black_hole",
  "no_remnant",
  "substellar",
];

function legend(): HTMLElement {
  return screen.getByRole("group", { name: "Chart legend" });
}

/** The words beside the symbol named `name`, which say what it stands for. */
function meaningOf(name: string): string {
  return within(legend()).getByRole("img", { name }).parentElement?.textContent ?? "";
}

describe("SymbolLegend", () => {
  it("names every shape of the star symbol set, with what it stands for", () => {
    render(<SymbolLegend bands={layerBands(aCensus().layers)} />);

    expect(meaningOf("Circle")).toBe(
      "PROTOSTAR, PRE-MAIN-SEQUENCE STAR, DWARF, SUBGIANT, HOT SUBDWARF, BROWN DWARF",
    );
    expect(meaningOf("Ringed circle")).toBe("GIANT, SUPERGIANT, WOLF-RAYET STAR");
    expect(meaningOf("Diamond")).toBe("WHITE DWARF");
    expect(meaningOf("Triangle")).toBe("NEUTRON STAR");
    expect(meaningOf("Square")).toBe("BLACK HOLE");
  });

  it("names every shape a star is drawn with, and no other", () => {
    render(<SymbolLegend bands={null} />);

    const drawn = new Set(EVERY_KIND.map(starSymbol).filter((shape) => shape !== null));
    const named = within(legend())
      .getAllByRole("img")
      .map((symbol) => symbol.getAttribute("aria-label"));
    expect(named).toHaveLength(drawn.size);
  });

  it("says that a star that left no remnant is listed and not drawn", () => {
    render(<SymbolLegend bands={null} />);

    expect(within(legend()).getByText("NO REMNANT, NOT YET FORMED: LIST ONLY")).toBeInTheDocument();
  });

  it("draws the ringed circle's disc filled inside an open ring", () => {
    render(<SymbolLegend bands={null} />);

    const paths = within(legend())
      .getByRole("img", { name: "Ringed circle" })
      .querySelectorAll("path");
    expect([...paths].map((path) => path.getAttribute("class"))).toEqual([
      null,
      "symbol-legend__filled",
    ]);
  });

  it("says a ringed circle is drawn at least at the size of the third band", () => {
    render(<SymbolLegend bands={layerBands(aCensus().layers)} />);

    expect(within(legend()).getByText(/^RINGED CIRCLE SIZE AT LEAST 0\.75/u)).toBeInTheDocument();
  });
});
