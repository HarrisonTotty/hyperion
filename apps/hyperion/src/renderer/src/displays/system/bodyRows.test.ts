import type { SystemSummaryDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { layoutHierarchy } from "../../lib/system/hierarchy";
import { toSystemModel } from "../../lib/system/wire";
import { aSystemSummary, aTripleSummary } from "../../test/systemFixtures";
import { hostRows } from "./bodyRows";

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
