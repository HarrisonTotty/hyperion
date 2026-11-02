import { describe, expect, it } from "vitest";

import {
  geometryFromArgv,
  geometrySwitch,
  geometryText,
  parseGeometryOverride,
} from "./geometryLaunch";

describe("the geometry override's hand-off (plan R14, Design note 17)", () => {
  it("reads each field's values and writes them back in the fields' order", () => {
    const override = parseGeometryOverride(
      "overlayDepth=scene-depth,casterStrideCap=4,clusterPath=true,casterDepthCull=false",
    );
    expect([override, geometryText(override), geometryText({})]).toEqual([
      {
        overlayDepth: "scene-depth",
        casterStrideCap: 4,
        clusterPath: true,
        casterDepthCull: false,
      },
      "clusterPath=true,casterDepthCull=false,casterStrideCap=4,overlayDepth=scene-depth",
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
