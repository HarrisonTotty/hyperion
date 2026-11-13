import { describe, expect, it } from "vitest";

import { chooseMeter, DEFAULT_METER_CHOICE, shownMeter } from "./meterChoice";

describe("the meter VIEW meters with (ux-calls-view, item 3b)", () => {
  it("opens a scene that brings its own meter on it, over the operator's choice elsewhere", () => {
    const chosen = chooseMeter(DEFAULT_METER_CHOICE, "lit", "PHASE TEST", undefined);
    expect(shownMeter(chosen, "BODY INSPECTION 1", "incident")).toBe("incident");
  });

  it("keeps a meter chosen in that opening through choices elsewhere, and opens the next on its own", () => {
    const inInspection = chooseMeter(
      DEFAULT_METER_CHOICE,
      "average",
      "BODY INSPECTION 1",
      "incident",
    );
    const thenElsewhere = chooseMeter(inInspection, "dark", "PHASE TEST", undefined);
    expect([
      shownMeter(thenElsewhere, "BODY INSPECTION 1", "incident"),
      shownMeter(thenElsewhere, "BODY INSPECTION 2", "incident"),
      shownMeter(thenElsewhere, "PHASE TEST", undefined),
    ]).toEqual(["average", "incident", "dark"]);
  });

  it("returns to the operator's own choice in a scene that brings none, the inspection's left behind", () => {
    const inInspection = chooseMeter(DEFAULT_METER_CHOICE, "lit", "BODY INSPECTION 1", "incident");
    expect(shownMeter(inInspection, "SERVER", undefined)).toBe("average");
  });
});
