import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { SystemModel } from "../lib/system/model";
import { toSystemModel } from "../lib/system/wire";
import {
  aBinaryHierarchy,
  aBlackHole,
  aPairOrbit,
  aSingleStarSummary,
  aSunlikeStar,
  aSystemSummary,
  aTripleSummary,
  aWhiteDwarf,
} from "../test/systemFixtures";
import { StarList } from "./StarList";

function modelOf(summary = aSystemSummary()): SystemModel {
  const result = toSystemModel(summary, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(`the fixture should convert, but: ${result.fault}`);
  }
  return result.model;
}

/** Each body row of a table, as the text of its cells. */
function rowsOf(table: HTMLElement): ReadonlyArray<ReadonlyArray<string>> {
  return within(table)
    .getAllByRole("row")
    .slice(1)
    .map((row) => [...row.querySelectorAll("th, td")].map((cell) => cell.textContent));
}

describe("StarList", () => {
  it("lists a single star under A, with its class, mass and state, and no orbit table", () => {
    render(<StarList model={modelOf(aSingleStarSummary())} stale={false} />);

    const stars = screen.getByRole("table", { name: "STARS" });
    expect(rowsOf(stars)).toEqual([["A", "G2V", "1.00", "DWARF"]]);
    expect(
      within(stars).getByRole("columnheader", { name: "MASS solar masses" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "ORBITS" })).not.toBeInTheDocument();
  });

  it("lists a triple's three stars and its two orbits with their units", () => {
    render(<StarList model={modelOf(aTripleSummary())} stale={false} />);

    expect(rowsOf(screen.getByRole("table", { name: "STARS" })).map((row) => row[0])).toEqual([
      "A",
      "B",
      "C",
    ]);
    expect(rowsOf(screen.getByRole("table", { name: "ORBITS" }))).toEqual([
      ["AB–C", "80.8 yr", "23.5 AU", "0.5000"],
      ["A–B", "8.91 d", "0.100 AU", "0.0000"],
    ]);
  });

  it("groups a wide orbit's digits and reads a remnant's mass, or the em dash where none is left", () => {
    const wide = aSystemSummary({
      stars: [aBlackHole(), aSunlikeStar({ body_index: 1, kind: "no_remnant", mass_msun: 0 })],
      hierarchy: aBinaryHierarchy(aPairOrbit({ semi_major_axis_m: 1.5e15, period_s: 3.2e12 })),
    });
    render(<StarList model={modelOf(wide)} stale={false} />);

    expect(rowsOf(screen.getByRole("table", { name: "STARS" }))).toEqual([
      ["A", "BH", "12.5", "BLACK HOLE"],
      ["B", "G2V", "—", "NO REMNANT"],
    ]);
    expect(rowsOf(screen.getByRole("table", { name: "ORBITS" }))[0]?.slice(1, 3)).toEqual([
      "1.01E5 yr",
      "10,000 AU",
    ]);
  });

  it("says once that binary classes are not yet modelled while the server computes none", () => {
    render(<StarList model={modelOf(aTripleSummary())} stale={false} />);

    expect(screen.getByText("BINARY CLASSES: NOT YET MODELLED")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "BINARY CLASSES" })).not.toBeInTheDocument();
  });

  it("names an innermost pair's shared class once, under the pair", () => {
    const cataclysmic = aSystemSummary({
      stars: [
        aWhiteDwarf({
          binary_class: { type: "cataclysmic_variable", kind: "dwarf_nova" },
        }),
        aSunlikeStar({
          body_index: 1,
          binary_class: { type: "cataclysmic_variable", kind: "dwarf_nova" },
        }),
      ],
    });
    render(<StarList model={modelOf(cataclysmic)} stale={false} />);

    const classes = screen.getByRole("table", { name: "BINARY CLASSES" });
    expect(within(classes).getByRole("columnheader", { name: "STARS" })).toBeInTheDocument();
    expect(rowsOf(classes)).toEqual([["A–B", "DWARF NOVA"]]);
    expect(screen.queryByText("BINARY CLASSES: NOT YET MODELLED")).not.toBeInTheDocument();
  });

  it("reads a hot subdwarf against its pair, whose STATE says which star it is", () => {
    const subdwarf = aSystemSummary({
      stars: [
        aSunlikeStar({ binary_class: { type: "hot_subdwarf" } }),
        aSunlikeStar({
          body_index: 1,
          kind: "hot_subdwarf",
          binary_class: { type: "hot_subdwarf" },
        }),
      ],
    });
    render(<StarList model={modelOf(subdwarf)} stale={false} />);

    expect(rowsOf(screen.getByRole("table", { name: "BINARY CLASSES" }))).toEqual([
      ["A–B", "HOT SUBDWARF"],
    ]);
  });

  it("reads a merger's product under its own letter", () => {
    const single = aSingleStarSummary();
    const stars = single.stars.map((star) => ({
      ...star,
      binary_class: { type: "r_coronae_borealis" as const },
    }));
    render(<StarList model={modelOf({ ...single, stars })} stale={false} />);

    expect(rowsOf(screen.getByRole("table", { name: "BINARY CLASSES" }))).toEqual([
      ["A", "R CORONAE BOREALIS STAR"],
    ]);
  });

  it("keeps a hyphenated word whole, so the class wraps only at its spaces", () => {
    const xray = aSystemSummary({
      stars: [
        aBlackHole({ binary_class: { type: "low_mass_xray_binary", kind: "persistent" } }),
        aSunlikeStar({
          body_index: 1,
          binary_class: { type: "low_mass_xray_binary", kind: "persistent" },
        }),
      ],
    });
    render(<StarList model={modelOf(xray)} stale={false} />);

    // Each hyphenated word is one element of its own, which the stylesheet keeps from breaking.
    const cell = within(screen.getByRole("table", { name: "BINARY CLASSES" })).getByRole("cell");
    expect(cell).toHaveTextContent("LOW-MASS X-RAY BINARY");
    expect(within(cell).getByText("LOW-MASS")).toBeInTheDocument();
    expect(within(cell).getByText("X-RAY")).toBeInTheDocument();
  });

  it("reads NONE when the server classed no star in a binary class", () => {
    const single = aSingleStarSummary();
    const stars = single.stars.map((star) => ({ ...star, binary_class: null }));
    render(<StarList model={modelOf({ ...single, stars })} stale={false} />);

    expect(rowsOf(screen.getByRole("table", { name: "BINARY CLASSES" }))).toEqual([["NONE"]]);
  });

  it("reads the em dash for a star whose class is not computed beside one whose class is", () => {
    const mixed = aSystemSummary({
      stars: [
        aWhiteDwarf({ binary_class: { type: "high_mass_xray_binary", kind: "be_x" } }),
        aSunlikeStar({ body_index: 1 }),
      ],
    });
    render(<StarList model={modelOf(mixed)} stale={false} />);

    expect(rowsOf(screen.getByRole("table", { name: "BINARY CLASSES" }))).toEqual([
      ["A", "Be X-RAY BINARY"],
      ["B", "—"],
    ]);
  });
});
