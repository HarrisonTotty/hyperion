import type { DestructionCauseDto, SurfaceStateDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { destructionCauseLabel, surfaceStateLabel } from "./bodyWords";

describe("surfaceStateLabel", () => {
  // Every surface state the wire carries, the two no rule produces yet included.
  it.each<readonly [SurfaceStateDto, string]>([
    ["magma_ocean", "MAGMA OCEAN"],
    ["airless", "AIRLESS"],
    ["runaway_greenhouse", "RUNAWAY GREENHOUSE"],
    ["temperate", "TEMPERATE"],
    ["snowball", "SNOWBALL"],
    ["steam_envelope", "SUPERCRITICAL H2O"],
    ["tenuous", "TENUOUS ATMOSPHERE"],
  ])("names %s as %s", (state, word) => {
    expect(surfaceStateLabel(state)).toBe(word);
  });
});

describe("destructionCauseLabel", () => {
  it("names every cause the wire carries", () => {
    const causes: [DestructionCauseDto, string][] = [
      ["dispersed", "DISPERSED"],
      ["engulfed", "ENGULFED"],
      ["tidally_disrupted", "TIDALLY DISRUPTED"],
      ["collided", "COLLIDED"],
    ];
    for (const [cause, word] of causes) {
      expect(destructionCauseLabel(cause)).toBe(word);
    }
  });
});
