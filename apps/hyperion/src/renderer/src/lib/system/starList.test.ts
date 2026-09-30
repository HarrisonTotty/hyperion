import { describe, expect, it } from "vitest";

import { aSingleStarSummary, aSystemSummary, aTripleSummary } from "../../test/systemFixtures";
import type { SystemModel } from "./model";
import { starListRows } from "./starList";
import { toSystemModel } from "./wire";

function modelOf(summary = aSystemSummary()): SystemModel {
  const result = toSystemModel(summary, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(`the fixture should convert, but: ${result.fault}`);
  }
  return result.model;
}

describe("starListRows", () => {
  it("gives a single star the letter A and no orbit", () => {
    const rows = starListRows(modelOf(aSingleStarSummary()));

    expect(rows.stars.map(({ letter, host }) => [letter, host.bodyIndex])).toEqual([["A", 0]]);
    expect(rows.orbits).toEqual([]);
  });

  it("letters a triple's stars in hierarchy order and names each orbit by the stars it joins", () => {
    const rows = starListRows(modelOf(aTripleSummary()));

    expect(rows.stars.map(({ letter, host }) => [letter, host.bodyIndex])).toEqual([
      ["A", 0],
      ["B", 1],
      ["C", 2],
    ]);
    expect(rows.orbits.map(({ id, label }) => [id, label])).toEqual([
      ["pair:0", "AB–C"],
      ["pair:1", "A–B"],
    ]);
    expect(rows.orbits[1]?.orbit.semiMajorAxisM).toBe(1.5e10);
  });

  it("lists a system not yet formed as no star and no orbit", () => {
    const rows = starListRows({ ...modelOf(), hosts: [], hierarchy: [], formed: false });

    expect(rows).toEqual({ stars: [], orbits: [] });
  });
});
