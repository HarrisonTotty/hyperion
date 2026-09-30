import { describe, expect, it } from "vitest";

import { FakeAdapter, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import { requiredFeatures } from "../platform";
import { babylonEngineOptions } from "./options";

describe("the engine's options", () => {
  it("ask the device for requiredFeatures' answer", () => {
    const adapter = new FakeAdapter({
      info: SWIFTSHADER_INFO,
      features: ["subgroups", "timestamp-query", "float32-blendable"],
    });
    const overrides = { withholdSubgroups: true, withholdShaderF16: false };
    const options = babylonEngineOptions(requiredFeatures(adapter, overrides));
    expect(options.deviceDescriptor?.requiredFeatures).toEqual(
      requiredFeatures(adapter, overrides),
    );
    expect(options.enableAllFeatures).toBe(false);
  });

  it("fix what Design note 11 names", () => {
    expect(babylonEngineOptions([])).toMatchObject({
      useLargeWorldRendering: false,
      stencil: false,
      antialias: false,
      doNotHandleContextLost: true,
      powerPreference: "high-performance",
    });
  });
});
