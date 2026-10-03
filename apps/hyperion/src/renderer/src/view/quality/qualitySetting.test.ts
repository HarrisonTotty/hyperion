import { describe, expect, it } from "vitest";

import {
  QUALITY_SETTINGS,
  SETTINGS,
  TERRAIN_SETTINGS,
  type TerrainSettings,
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

  it("makes TERRAIN_SETTINGS the terrain field of SETTINGS", () => {
    for (const setting of QUALITY_SETTINGS) {
      expect(TERRAIN_SETTINGS[setting]).toBe(SETTINGS[setting].terrain);
    }
  });
});
