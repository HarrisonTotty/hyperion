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
