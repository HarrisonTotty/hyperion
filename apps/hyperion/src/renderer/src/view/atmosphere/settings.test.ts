import { describe, expect, it } from "vitest";

import { MAX_BODY_LIGHTS } from "../lighting/hostLights";
import { QUALITY_SETTINGS, SETTINGS } from "../quality/qualitySetting";
import {
  AERIAL_PERSPECTIVE_SCOPE,
  ATMOSPHERE_QUALITY_LIMITS,
  ATMOSPHERE_VIEW_SETTINGS,
  MARCH_BAND,
  MAX_SKY_LIGHT_SLOTS,
  MAX_SKY_SOURCES,
  SKY_SUN_CAP,
} from "./settings";

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

describe("MARCH_BAND (R08.T8)", () => {
  it("is each setting's march band, projected and not held twice", () => {
    for (const setting of QUALITY_SETTINGS) {
      expect(MARCH_BAND[setting]).toBe(SETTINGS[setting].atmosphere.marchBand);
    }
  });

  it("hands the frame to the march between 0.3 and 0.5 of the top on high and 0.2 and 0.3 on low", () => {
    expect(MARCH_BAND).toEqual({
      high: { lowerShareOfTop: 0.3, upperShareOfTop: 0.5 },
      low: { lowerShareOfTop: 0.2, upperShareOfTop: 0.3 },
    });
  });
});

describe("the atmosphere's view settings (R08.T7)", () => {
  it("reads R07's star count, never a literal of its own", () => {
    for (const setting of QUALITY_SETTINGS) {
      expect(SKY_SUN_CAP[setting]).toBe(MAX_BODY_LIGHTS);
    }
  });

  it("is SETTINGS' own field, its planetshine count R07's", () => {
    for (const setting of QUALITY_SETTINGS) {
      expect(SETTINGS[setting].atmosphereView).toBe(ATMOSPHERE_VIEW_SETTINGS[setting]);
      expect(SETTINGS[setting].atmosphereView.skyPlanetshineSources).toBe(
        SETTINGS[setting].photoreal.planetshineSources,
      );
      expect(ATMOSPHERE_QUALITY_LIMITS[setting]).toBe(
        SETTINGS[setting].atmosphereView.qualityLimits,
      );
    }
  });

  it("sizes the kernels for four stars and two planetshine sources, past every setting's bound", () => {
    expect([MAX_SKY_SOURCES, MAX_SKY_LIGHT_SLOTS]).toEqual([6, 4]);
    for (const setting of QUALITY_SETTINGS) {
      const view = ATMOSPHERE_VIEW_SETTINGS[setting];
      expect(view.skySunCap).toBeLessThanOrEqual(MAX_SKY_LIGHT_SLOTS);
      expect(view.skySunCap + view.skyPlanetshineSources).toBeLessThanOrEqual(MAX_SKY_SOURCES);
    }
  });

  it("states Design note 7's limits on every setting", () => {
    for (const setting of QUALITY_SETTINGS) {
      const limits = ATMOSPHERE_QUALITY_LIMITS[setting].join(" ");
      expect(limits).toMatch(/third star/u);
      expect(limits).toMatch(/twilight limit/u);
      expect(limits).toMatch(/Planetshine/u);
    }
  });

  it("states low's one planetshine source as a limit of its own", () => {
    expect(ATMOSPHERE_QUALITY_LIMITS.low.length).toBe(ATMOSPHERE_QUALITY_LIMITS.high.length + 1);
  });
});
