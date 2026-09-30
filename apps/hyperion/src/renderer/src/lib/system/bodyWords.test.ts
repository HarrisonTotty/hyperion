import type { DestructionCauseDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { destructionCauseLabel } from "./bodyWords";

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
