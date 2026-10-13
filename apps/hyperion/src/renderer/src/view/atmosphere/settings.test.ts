import { describe, expect, it } from "vitest";

import { QUALITY_SETTINGS, SETTINGS } from "../quality/qualitySetting";
import { AERIAL_PERSPECTIVE_SCOPE } from "./settings";

describe("AERIAL_PERSPECTIVE_SCOPE", () => {
  it("is each setting's atmosphere scope, projected and not held twice", () => {
    for (const setting of QUALITY_SETTINGS) {
      expect(AERIAL_PERSPECTIVE_SCOPE[setting]).toBe(
        SETTINGS[setting].atmosphere.aerialPerspectiveScope,
      );
    }
  });

  it("applies aerial perspective to everything opaque on high and to the terrain alone on low", () => {
    expect(AERIAL_PERSPECTIVE_SCOPE).toEqual({ high: "scene", low: "terrain" });
  });
});
