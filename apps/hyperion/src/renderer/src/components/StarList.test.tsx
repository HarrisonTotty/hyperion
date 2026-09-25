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
});
