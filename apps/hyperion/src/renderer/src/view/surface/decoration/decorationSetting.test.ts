import { describe, expect, it } from "vitest";

import { QUALITY_SETTINGS, SETTINGS } from "../../quality/qualitySetting";
import {
  DECORATION_ON,
  decorationAllowed,
  decorationStatement,
  type R11Ladder,
} from "./decorationSetting";

// One entry per field, so that a ladder entry added to `R11Ladder` and missed here fails the
// typecheck.
const LADDER_FIELDS = {
  decoration: true,
  scatterDensity: true,
  clouds: true,
  cloudShadows: true,
  oceanComponents: true,
  oceanRefraction: true,
  whitecaps: true,
  rings: true,
  ringShadowOnBody: true,
} satisfies Record<keyof R11Ladder, true>;

/** A setting's ladder entries, read from its `ViewSettings` by name. */
function ladderOf(settings: R11Ladder): R11Ladder {
  return {
    decoration: settings.decoration,
    scatterDensity: settings.scatterDensity,
    clouds: settings.clouds,
    cloudShadows: settings.cloudShadows,
    oceanComponents: settings.oceanComponents,
    oceanRefraction: settings.oceanRefraction,
    whitecaps: settings.whitecaps,
    rings: settings.rings,
    ringShadowOnBody: settings.ringShadowOnBody,
  };
}

describe("R11's ladder entries (Design note 18)", () => {
  it("gives both settings every entry as a field of their ViewSettings", () => {
    for (const setting of QUALITY_SETTINGS) {
      for (const field of Object.keys(LADDER_FIELDS)) {
        expect(SETTINGS[setting]).toHaveProperty(field);
      }
    }
  });

  it("gives the low setting Design note 18's values, with no decoration of any kind", () => {
    expect(ladderOf(SETTINGS.low)).toEqual({
      decoration: false,
      scatterDensity: 0,
      clouds: "layer",
      cloudShadows: false,
      oceanComponents: 8,
      oceanRefraction: false,
      whitecaps: false,
      rings: "annulus",
      ringShadowOnBody: false,
    });
  });

  it("gives the high setting Design note 18's values", () => {
    expect(ladderOf(SETTINGS.high)).toEqual({
      decoration: true,
      scatterDensity: 1,
      clouds: "volume",
      cloudShadows: true,
      oceanComponents: 32,
      oceanRefraction: true,
      whitecaps: true,
      rings: "particles",
      ringShadowOnBody: true,
    });
  });
});

describe("whether a frame may draw decoration", () => {
  it("allows it in the photorealistic style on the high setting", () => {
    expect(decorationAllowed(SETTINGS.high, { style: "photorealistic", floorStepped: false })).toBe(
      true,
    );
  });

  it("never allows it on the low setting", () => {
    expect(decorationAllowed(SETTINGS.low, { style: "photorealistic", floorStepped: false })).toBe(
      false,
    );
  });

  it("never allows it in the wireframe", () => {
    expect(decorationAllowed(SETTINGS.high, { style: "wireframe", floorStepped: false })).toBe(
      false,
    );
  });

  it("turns it off while the frame-rate floor has stepped the view down", () => {
    expect(decorationAllowed(SETTINGS.high, { style: "photorealistic", floorStepped: true })).toBe(
      false,
    );
  });
});

describe("where DECORATION ON stands", () => {
  it("is the guide's nomenclature", () => {
    expect(DECORATION_ON).toBe("DECORATION ON");
  });

  it("follows what was drawn, not the setting: nothing drawn states nothing", () => {
    expect(decorationStatement(false, true)).toBeNull();
    expect(decorationStatement(false, false)).toBeNull();
  });

  it("stands on the survey line while it stands as a reading", () => {
    expect(decorationStatement(true, true)).toBe("survey-line");
  });

  it("stands among the label block's statements where no survey line can state it", () => {
    expect(decorationStatement(true, false)).toBe("statement");
  });
});
