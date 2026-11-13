import { describe, expect, it } from "vitest";

import {
  geometryFromArgv,
  geometrySwitch,
  geometryText,
  geometryValueText,
  parseGeometryOverride,
} from "./geometryLaunch";

describe("the geometry override's hand-off (plan R14, Design note 17)", () => {
  it("reads each field's values and writes them back in the fields' order", () => {
    const override = parseGeometryOverride(
      "overlayDepth=scene-depth,shading=visibility,occlusion=hzb,strides=measured,finClusters=true,wireClusters=false,hullClusters=true,figureClusters=false,casterStrideCap=4,clusterPath=true,casterDepthCull=false",
    );
    expect([override, geometryText(override), geometryText({})]).toEqual([
      {
        overlayDepth: "scene-depth",
        strides: "measured",
        occlusion: "hzb",
        shading: "visibility",
        finClusters: true,
        wireClusters: false,
        hullClusters: true,
        figureClusters: false,
        casterStrideCap: 4,
        clusterPath: true,
        casterDepthCull: false,
      },
      "clusterPath=true,casterDepthCull=false,casterStrideCap=4,overlayDepth=scene-depth,figureClusters=false,hullClusters=true,wireClusters=false,finClusters=true,strides=measured,occlusion=hzb,shading=visibility",
      "",
    ]);
  });

  it("reads and writes the far cascades' rate, last in the fields' order (R14.T6.b)", () => {
    const override = parseGeometryOverride("farCascadeRate=1,clusterPath=true");
    expect([
      override,
      geometryText(override),
      ["1", "2", "4"].map((rate) => parseGeometryOverride(`farCascadeRate=${rate}`)),
    ]).toEqual([
      { farCascadeRate: 1, clusterPath: true },
      "clusterPath=true,farCascadeRate=1",
      [{ farCascadeRate: 1 }, { farCascadeRate: 2 }, { farCascadeRate: 4 }],
    ]);
  });

  it("refuses a far cascades' rate other than 1, 2 or 4 (R14.T6.b)", () => {
    for (const text of ["farCascadeRate=3", "farCascadeRate=8", "farCascadeRate=0"]) {
      expect(() => parseGeometryOverride(text)).toThrow("farCascadeRate is 1, 2 or 4");
    }
  });

  it("refuses a shading other than forward or visibility (R14.T2.c)", () => {
    for (const text of ["shading=deferred", "shading=ids", "shading="]) {
      expect(() => parseGeometryOverride(text)).toThrow("shading is forward or visibility");
    }
    expect(parseGeometryOverride("shading=forward")).toEqual({ shading: "forward" });
  });

  it("reads and writes the lit selection's budget, last in the fields' order (R14.T3.e)", () => {
    const override = parseGeometryOverride("terrainBudget=triangles,occlusion=off");
    expect([
      override,
      geometryText(override),
      parseGeometryOverride("terrainBudget=patches"),
    ]).toEqual([
      { terrainBudget: "triangles", occlusion: "off" },
      "occlusion=off,terrainBudget=triangles",
      { terrainBudget: "patches" },
    ]);
  });

  it("reads and writes the overlay's hidden marks, last in the fields' order (R10.T16.c)", () => {
    const override = parseGeometryOverride("hiddenMarks=drawn,terrainBudget=patches");
    expect([override, geometryText(override), parseGeometryOverride("hiddenMarks=culled")]).toEqual(
      [
        { hiddenMarks: "drawn", terrainBudget: "patches" },
        "terrainBudget=patches,hiddenMarks=drawn",
        { hiddenMarks: "culled" },
      ],
    );
  });

  it("refuses hidden marks other than drawn or culled (R10.T16.c)", () => {
    for (const text of ["hiddenMarks=hidden", "hiddenMarks=on", "hiddenMarks="]) {
      expect(() => parseGeometryOverride(text)).toThrow("hiddenMarks is drawn or culled");
    }
  });

  it("refuses a lit selection's budget other than patches or triangles (R14.T3.e)", () => {
    for (const text of ["terrainBudget=frames", "terrainBudget=on", "terrainBudget="]) {
      expect(() => parseGeometryOverride(text)).toThrow("terrainBudget is patches or triangles");
    }
  });

  it("reads and writes the stand-ins' strides, last in the fields' order (R14.T3.c's stand-ins)", () => {
    const override = parseGeometryOverride("standInStrides=off,terrainBudget=patches");
    expect([
      override,
      geometryText(override),
      parseGeometryOverride("standInStrides=own-grids"),
      parseGeometryOverride("standInStrides=all"),
    ]).toEqual([
      { standInStrides: "off", terrainBudget: "patches" },
      "terrainBudget=patches,standInStrides=off",
      { standInStrides: "own-grids" },
      { standInStrides: "all" },
    ]);
  });

  it("refuses stand-ins' strides other than off, own-grids or all (R14.T3.c's stand-ins)", () => {
    for (const text of ["standInStrides=masks", "standInStrides=deep", "standInStrides="]) {
      expect(() => parseGeometryOverride(text)).toThrow("standInStrides is off, own-grids or all");
    }
  });

  it("reads and writes the edge bakes' stand-ins, after the stand-ins' strides in the fields' order (R14.T3.c's edge stand-ins)", () => {
    const override = parseGeometryOverride("edgeStandIns=off,standInStrides=all");
    expect([
      override,
      geometryText(override),
      parseGeometryOverride("edgeStandIns=surveyed"),
      parseGeometryOverride("edgeStandIns=masks"),
    ]).toEqual([
      { edgeStandIns: "off", standInStrides: "all" },
      "standInStrides=all,edgeStandIns=off",
      { edgeStandIns: "surveyed" },
      { edgeStandIns: "masks" },
    ]);
  });

  it("refuses edge stand-ins other than off, surveyed or masks (R14.T3.c's edge stand-ins)", () => {
    for (const text of ["edgeStandIns=all", "edgeStandIns=on", "edgeStandIns="]) {
      expect(() => parseGeometryOverride(text)).toThrow("edgeStandIns is off, surveyed or masks");
    }
  });

  it("reads and writes the disc cells' way, last in the fields' order (plan R07)", () => {
    const override = parseGeometryOverride("discCells=per-cell,standInStrides=off");
    expect([override, geometryText(override), parseGeometryOverride("discCells=packed")]).toEqual([
      { discCells: "per-cell", standInStrides: "off" },
      "standInStrides=off,discCells=per-cell",
      { discCells: "packed" },
    ]);
  });

  it("refuses disc cells other than per-cell or packed (plan R07)", () => {
    for (const text of ["discCells=pooled", "discCells=per-pixel", "discCells="]) {
      expect(() => parseGeometryOverride(text)).toThrow("discCells is per-cell or packed");
    }
  });

  it("reads and writes the figure's shading, last in the fields' order (plan R07)", () => {
    const override = parseGeometryOverride("figureShading=screen,discCells=packed");
    expect([
      override,
      geometryText(override),
      parseGeometryOverride("figureShading=forward"),
    ]).toEqual([
      { figureShading: "screen", discCells: "packed" },
      "discCells=packed,figureShading=screen",
      { figureShading: "forward" },
    ]);
  });

  it("reads and writes the terrain's planetshine, last in the fields' order (perf-bodies-near-ground)", () => {
    const override = parseGeometryOverride("terrainPlanetshine=brightest,figureShading=screen");
    expect([
      override,
      geometryText(override),
      parseGeometryOverride("terrainPlanetshine=record"),
    ]).toEqual([
      { terrainPlanetshine: "brightest", figureShading: "screen" },
      "figureShading=screen,terrainPlanetshine=brightest",
      { terrainPlanetshine: "record" },
    ]);
    for (const text of [
      "terrainPlanetshine=one",
      "terrainPlanetshine=all",
      "terrainPlanetshine=",
    ]) {
      expect(() => parseGeometryOverride(text)).toThrow(
        "terrainPlanetshine is record or brightest",
      );
    }
  });

  it("refuses a figure's shading other than forward or screen (plan R07)", () => {
    for (const text of ["figureShading=visibility", "figureShading=deferred", "figureShading="]) {
      expect(() => parseGeometryOverride(text)).toThrow("figureShading is forward or screen");
    }
  });

  it("refuses an occlusion other than off or hzb (R14.T3.d)", () => {
    for (const text of ["occlusion=none", "occlusion=on", "occlusion="]) {
      expect(() => parseGeometryOverride(text)).toThrow("occlusion is off or hzb");
    }
  });

  it("refuses camera strides other than off or measured (R14.T3.c)", () => {
    for (const text of ["strides=on", "strides=true", "strides="]) {
      expect(() => parseGeometryOverride(text)).toThrow("strides is off or measured");
    }
  });

  it("reads and writes the mesh page pool's bytes, last in the fields' order (R14.T5.b)", () => {
    const override = parseGeometryOverride("pageBytes=33554432,wireClusters=true");
    expect([override, geometryText(override), parseGeometryOverride("pageBytes=0")]).toEqual([
      { pageBytes: 33_554_432, wireClusters: true },
      "wireClusters=true,pageBytes=33554432",
      { pageBytes: 0 },
    ]);
  });

  it.each(["pageBytes=32MiB", "pageBytes=-1", "pageBytes=1.5", "pageBytes="])(
    "refuses %s, the page pool's bytes a whole number (R14.T5.b)",
    (text) => {
      expect(() => parseGeometryOverride(text)).toThrow("pageBytes is a whole number of bytes");
    },
  );

  it("reads and writes the meshes' tolerance as <camera>:<wireframe occluder> px, last in the fields' order (R14.T5.f)", () => {
    const override = parseGeometryOverride("meshTolerancePx=2:0.25,pageBytes=0");
    expect([override, geometryText(override)]).toEqual([
      { meshTolerancePx: { camera: 2, wireframeOccluder: 0.25 }, pageBytes: 0 },
      "pageBytes=0,meshTolerancePx=2:0.25",
    ]);
  });

  it("writes each field's value as the smoke page's query reads it, the meshes' tolerance as <camera>:<occluder> (R14.T5.f)", () => {
    expect([
      geometryValueText({ camera: 2, wireframeOccluder: 0.25 }),
      geometryValueText(33_554_432),
      geometryValueText("scene-depth"),
      geometryValueText(false),
    ]).toEqual(["2:0.25", "33554432", "scene-depth", "false"]);
  });

  it.each([
    "meshTolerancePx=1",
    "meshTolerancePx=1:-1",
    "meshTolerancePx=a:b",
    "meshTolerancePx=1:2:3",
  ])("refuses %s, the meshes' tolerance two numbers of at least 0 (R14.T5.f)", (text) => {
    expect(() => parseGeometryOverride(text)).toThrow(
      "meshTolerancePx is <camera>:<wireframe occluder> px",
    );
  });

  it("carries an override through the renderer's argv, and none where the launch gave none", () => {
    const override = { clusterPath: false } as const;
    expect([
      geometryFromArgv(["--other", geometrySwitch(override)]),
      geometryFromArgv(["--hyperion-quality=high"]),
    ]).toEqual([override, null]);
    expect(() => geometryFromArgv(["--hyperion-geometry=clusterPath=maybe"])).toThrow(
      "does not hold an override",
    );
  });

  it("refuses a field that is not the override's, a value that is not its field's, and a field given twice", () => {
    for (const text of ["tau=1", "clusterPath=1", "casterStrideCap=16", "overlayDepth=", "a=b=c"]) {
      expect(() => parseGeometryOverride(text)).toThrow(Error);
    }
    expect(() => parseGeometryOverride("casterStrideCap=2,casterStrideCap=2")).toThrow(
      "casterStrideCap is given twice",
    );
  });
});
