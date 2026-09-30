import { describe, expect, it } from "vitest";

import { readFeatureStatus } from "./featureStatus";

describe("reading the feature status", () => {
  it("reads the vulkan and webgpu entries of a record", () => {
    const status = { gpu_compositing: "enabled", vulkan: "enabled_on", webgpu: "enabled" };
    expect(readFeatureStatus(status)).toEqual({ vulkan: "enabled_on", webgpu: "enabled" });
  });

  it("gives undefined for a missing entry", () => {
    expect(readFeatureStatus({ gpu_compositing: "enabled" })).toEqual({
      vulkan: undefined,
      webgpu: undefined,
    });
  });

  it("gives undefined for an entry that is not text", () => {
    expect(readFeatureStatus({ vulkan: 1, webgpu: null })).toEqual({
      vulkan: undefined,
      webgpu: undefined,
    });
  });

  it("gives undefined for both when the status is not a record", () => {
    for (const status of [undefined, null, "enabled", 3]) {
      expect(readFeatureStatus(status)).toEqual({ vulkan: undefined, webgpu: undefined });
    }
  });
});
