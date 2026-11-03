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
      "overlayDepth=scene-depth,strides=measured,finClusters=true,wireClusters=false,hullClusters=true,figureClusters=false,casterStrideCap=4,clusterPath=true,casterDepthCull=false",
    );
    expect([override, geometryText(override), geometryText({})]).toEqual([
      {
        overlayDepth: "scene-depth",
        strides: "measured",
        finClusters: true,
        wireClusters: false,
        hullClusters: true,
        figureClusters: false,
        casterStrideCap: 4,
        clusterPath: true,
        casterDepthCull: false,
      },
      "clusterPath=true,casterDepthCull=false,casterStrideCap=4,overlayDepth=scene-depth,figureClusters=false,hullClusters=true,wireClusters=false,finClusters=true,strides=measured",
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
