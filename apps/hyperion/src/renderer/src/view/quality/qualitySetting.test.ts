import { describe, expect, it } from "vitest";

import {
  QUALITY_SETTINGS,
  SETTINGS,
  TERRAIN_SETTINGS,
  type TerrainSettings,
  terrainSettingsFor,
} from "./qualitySetting";

// One entry per field, so that a field added to `TerrainSettings` and missed here fails the typecheck.
const TERRAIN_FIELDS = {
  tauPx: true,
  renderHeightPx: true,
  normals: true,
  vertexPath: true,
  cacheBytes: true,
} satisfies Record<keyof TerrainSettings, true>;

describe("the quality settings", () => {
  it("lists exactly the high and the low setting", () => {
    expect(QUALITY_SETTINGS).toEqual(["high", "low"]);
    expect(Object.keys(SETTINGS).toSorted()).toEqual(["high", "low"]);
  });

  it("gives both settings every terrain field", () => {
    const fields = Object.keys(TERRAIN_FIELDS).toSorted();
    for (const setting of QUALITY_SETTINGS) {
      expect(Object.keys(SETTINGS[setting].terrain).toSorted()).toEqual(fields);
    }
  });

  it("gives the low setting Design note 26's values", () => {
    expect(SETTINGS.low.terrain).toEqual({
      tauPx: 2,
      renderHeightPx: 720,
      normals: "mesh",
      vertexPath: "face-differences",
      cacheBytes: 64 * 1024 * 1024,
    });
  });

  it("gives the high setting a 1 px tolerance at the canvas's size with doubled normals", () => {
    expect(SETTINGS.high.terrain).toMatchObject({
      tauPx: 1,
      renderHeightPx: null,
      normals: "double",
      vertexPath: "baked-offsets",
    });
    expect(SETTINGS.high.terrain.cacheBytes).toBeGreaterThan(SETTINGS.low.terrain.cacheBytes);
  });

  it("holds the photorealistic view's internal scale in [0.5, 1.0] on both settings", () => {
    for (const setting of QUALITY_SETTINGS) {
      expect(SETTINGS[setting].internalScaleBounds).toEqual([0.5, 1]);
    }
  });

  it("budgets the low setting's photorealistic view at 30 Hz, and only one", () => {
    expect(SETTINGS.low.budget).toEqual({ photorealisticRateHz: 30, photorealisticViews: 1 });
  });

  it("budgets the high setting's photorealistic views at 60 Hz, without a limit", () => {
    expect(SETTINGS.high.budget).toEqual({ photorealisticRateHz: 60, photorealisticViews: null });
  });

  it("makes TERRAIN_SETTINGS the terrain field of SETTINGS", () => {
    for (const setting of QUALITY_SETTINGS) {
      expect(TERRAIN_SETTINGS[setting]).toBe(SETTINGS[setting].terrain);
    }
  });

  it("gives each setting's own terrain for no variant", () => {
    for (const setting of QUALITY_SETTINGS) {
      expect(terrainSettingsFor(setting)).toBe(SETTINGS[setting].terrain);
      expect(terrainSettingsFor(setting, {})).toBe(SETTINGS[setting].terrain);
    }
  });

  it("replaces only the variant's own fields", () => {
    const high = SETTINGS.high.terrain;
    expect(terrainSettingsFor("high", { vertexPath: "face-differences" })).toEqual({
      ...high,
      vertexPath: "face-differences",
    });
    expect(terrainSettingsFor("high", { normals: "mesh" })).toEqual({ ...high, normals: "mesh" });
    expect(terrainSettingsFor("low", { normals: "double" })).toEqual({
      ...SETTINGS.low.terrain,
      normals: "double",
    });
  });

  it("refuses the low setting with the baked-offsets path", () => {
    expect(() => terrainSettingsFor("low", { vertexPath: "baked-offsets" })).toThrow(RangeError);
  });
});
