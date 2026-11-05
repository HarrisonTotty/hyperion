import { describe, expect, expectTypeOf, it } from "vitest";

import type { GeometryOverride } from "../../../../preload/api";
import { STROKE_PX } from "../wireframe/drawList";
import {
  GEOMETRY_BEFORE_R14,
  geometryOf,
  type GeometrySettings,
  QUALITY_SETTINGS,
  SETTINGS,
  TERRAIN_SETTINGS,
  type TerrainSettings,
  terrainSettingsFor,
  WIREFRAME_TERRAIN_SLOTS,
} from "./qualitySetting";

// One entry per field, so that a field added to `TerrainSettings` and missed here fails the typecheck.
const TERRAIN_FIELDS = {
  tauPx: true,
  renderHeightPx: true,
  normals: true,
  vertexPath: true,
  cacheBytes: true,
  handoverDetail: true,
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
      handoverDetail: "whole",
    });
  });

  it("gives the high setting a 1 px tolerance at 1080 rows at most with doubled normals", () => {
    expect(SETTINGS.high.terrain).toMatchObject({
      tauPx: 1,
      renderHeightPx: 1080,
      normals: "double",
      vertexPath: "baked-offsets",
    });
    expect(SETTINGS.high.terrain.cacheBytes).toBeGreaterThan(SETTINGS.low.terrain.cacheBytes);
  });

  it("fades the terrain's sub-cell shading in below the hand-over on the high setting alone (R10.T15.b.2)", () => {
    expect([SETTINGS.high.terrain.handoverDetail, SETTINGS.low.terrain.handoverDetail]).toEqual([
      "faded",
      "whole",
    ]);
  });

  it("gives the high setting's cache 400,000,000 B, held by R10.T14.b until R10.T4.b", () => {
    expect(SETTINGS.high.terrain.cacheBytes).toBe(400_000_000);
  });

  it("gives the wireframe's cache 520 slots on either setting, held by R10.T14.b until R10.T4.b", () => {
    expect(WIREFRAME_TERRAIN_SLOTS).toBe(520);
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

  it("gives the low setting Design note 18's photorealistic forms (R07.T17)", () => {
    expect(SETTINGS.low.photoreal).toEqual({
      histogramStride: 2,
      bloom: { firstLevel: 1, levels: 5 },
      discAnnuli: 3,
      planetshineSources: 1,
      floorLimbSamples: 2,
    });
  });

  it("gives the high setting the full photorealistic forms (R07.T17)", () => {
    expect(SETTINGS.high.photoreal).toEqual({
      histogramStride: 1,
      bloom: { firstLevel: 0, levels: 7 },
      discAnnuli: 4,
      planetshineSources: 2,
      floorLimbSamples: 2,
    });
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

describe("the geometry's settings (plan R14)", () => {
  it("draws through the cluster path on both settings, its casters culled by their receivers' depths and at their texel, the overlay's depth its own on the cluster path, its far cascades every fourth frame, a mesh page pool of 128 MiB high and 32 MiB low, its edge patches on the path, its camera strides measured, their acceptance passed, occlusion off until its acceptance (R14.T3.c, T3.d), the shading forward (R14.T2), the triangle budget on both (R14.T3.e) and the stand-ins on their own grids strided (R14.T3.c's stand-ins), and keeps the way back", () => {
    for (const setting of QUALITY_SETTINGS) {
      const {
        pageBytes: _pageBytes,
        meshTolerancePx: _meshTolerancePx,
        ...levers
      } = SETTINGS[setting].geometry;
      expect(levers).toEqual({
        clusterPath: true,
        casterDepthCull: true,
        casterStrideCap: 8,
        overlayDepth: "cluster",
        litClusters: true,
        farCascadeRate: 4,
        figureClusters: true,
        hullClusters: true,
        wireClusters: true,
        finClusters: true,
        strides: "measured",
        occlusion: "off",
        shading: "forward",
        terrainBudget: "triangles",
        standInStrides: "own-grids",
      });
    }
    expect([SETTINGS.high.geometry.pageBytes, SETTINGS.low.geometry.pageBytes]).toEqual([
      134_217_728, 33_554_432,
    ]);
    expect(GEOMETRY_BEFORE_R14).toEqual({
      clusterPath: false,
      casterDepthCull: false,
      casterStrideCap: 1,
      overlayDepth: "redraw",
      litClusters: false,
      farCascadeRate: 1,
      figureClusters: false,
      hullClusters: false,
      wireClusters: false,
      pageBytes: 0,
      meshTolerancePx: { camera: 0, wireframeOccluder: 0 },
      finClusters: false,
      strides: "off",
      occlusion: "off",
      shading: "forward",
      terrainBudget: "patches",
      standInStrides: "off",
    });
  });

  it("cuts meshes at 1 px high and 2 px low in the camera, and at half the thin stroke as the wireframe's occluders (plan R14, R14.T5.f)", () => {
    expect([SETTINGS.high.geometry.meshTolerancePx, SETTINGS.low.geometry.meshTolerancePx]).toEqual(
      [
        { camera: 1, wireframeOccluder: STROKE_PX.thin / 2 },
        { camera: 2, wireframeOccluder: STROKE_PX.thin / 2 },
      ],
    );
  });

  it("takes a launch's override over the setting's, field by field, the override's fields the settings' own (Design note 17)", () => {
    expectTypeOf<GeometryOverride>().toEqualTypeOf<Partial<GeometrySettings>>();
    expect([
      geometryOf("low", { clusterPath: false }),
      geometryOf("high", { casterStrideCap: 1, overlayDepth: "redraw" }),
      geometryOf("high", {}),
      geometryOf("high", { farCascadeRate: 1 }),
      geometryOf("high", { pageBytes: 2 ** 20 }),
      geometryOf("low", { finClusters: false }),
      geometryOf("high", { strides: "measured" }),
      geometryOf("low", { terrainBudget: "patches" }),
      geometryOf("high", { standInStrides: "off" }),
    ]).toEqual([
      { ...SETTINGS.low.geometry, clusterPath: false },
      { ...SETTINGS.high.geometry, casterStrideCap: 1, overlayDepth: "redraw" },
      SETTINGS.high.geometry,
      { ...SETTINGS.high.geometry, farCascadeRate: 1 },
      { ...SETTINGS.high.geometry, pageBytes: 2 ** 20 },
      { ...SETTINGS.low.geometry, finClusters: false },
      { ...SETTINGS.high.geometry, strides: "measured" },
      { ...SETTINGS.low.geometry, terrainBudget: "patches" },
      { ...SETTINGS.high.geometry, standInStrides: "off" },
    ]);
  });
});
