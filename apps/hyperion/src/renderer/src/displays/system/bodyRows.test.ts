import type { SystemSummaryDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { layoutHierarchy } from "../../lib/system/hierarchy";
import { toSystemModel } from "../../lib/system/wire";
import { aSystemSummary, aTripleSummary } from "../../test/systemFixtures";
import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { FIXTURE_SYSTEM, populatedBodies, sliceBodies } from "../../test/planetaryFixture";
import { hostRows, systemRows } from "./bodyRows";

function built(summary: SystemSummaryDto = aSystemSummary()) {
  const result = toSystemModel(summary, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return {
    model: result.model,
    layout: layoutHierarchy(result.model.hierarchy, result.model.hosts),
  };
}

describe("hostRows", () => {
  it("lists the hosts by body index with their kind in words and a companion's SMA", () => {
    const { model, layout } = built();

    expect(
      hostRows(model.hosts, layout).map((row) => [
        row.designation,
        row.kind,
        row.level,
        row.semiMajorAxis,
      ]),
    ).toEqual([
      ["H7K 4C0RFZ D-7 /0", "WHITE DWARF", 1, null],
      ["H7K 4C0RFZ D-7 /1", "DWARF", 1, { value: "23.5", unit: "AU" }],
    ]);
  });

  it("gives a triple's close companion the close orbit's SMA and the far one the wide orbit's", () => {
    const { model, layout } = built(aTripleSummary());

    // The close pair is 1.5E10 m apart, 15.0 Gm, just past formatBodyDistance's 0.1 AU edge.
    expect(hostRows(model.hosts, layout).map((row) => row.semiMajorAxis)).toEqual([
      null,
      { value: "0.100", unit: "AU" },
      { value: "23.5", unit: "AU" },
    ]);
  });
});

function rowsOf(response = sliceBodies()) {
  const result = toSystemBodiesModel(response, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  const { model, bodies } = result;
  return systemRows(
    FIXTURE_SYSTEM,
    model.hosts,
    layoutHierarchy(model.hierarchy, model.hosts),
    bodies.bodies,
  );
}

describe("systemRows", () => {
  it("lists the planets under their host by semi-major axis", () => {
    expect(
      rowsOf().map((row) => [row.designation, row.level, row.kind, row.semiMajorAxis, row.state]),
    ).toEqual([
      ["H7K 4C0RFZ D-7 /0", 1, "DWARF", null, null],
      ["H7K 4C0RFZ D-7 /768", 2, "PLANET", { value: "1.00", unit: "AU" }, null],
      ["H7K 4C0RFZ D-7 /1280", 2, "PLANET", { value: "5.20", unit: "AU" }, null],
    ]);
  });

  it("keeps every body in the tree, moons under their planet, and reads a gone body's state", () => {
    expect(
      rowsOf(populatedBodies()).map((row) => [row.id.slice(17), row.level, row.state]),
    ).toEqual([
      ["0000", 1, null],
      ["0100", 2, null],
      ["0101", 3, null],
      ["0180", 3, null],
      ["0200", 2, "DESTROYED"],
      ["0300", 2, "NOT YET FORMED"],
      ["0400", 2, "UNBOUND"],
      ["e000", 2, null],
      ["e001", 3, null],
      ["e200", 2, "DESTROYED"],
      ["e300", 2, null],
      ["e100", 1, null],
    ]);
  });
});
