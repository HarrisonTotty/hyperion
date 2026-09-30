import { describe, expect, it } from "vitest";

import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import {
  type AdapterOutcome,
  requestAdapterOutcome,
  requiredFeatures,
  styleAvailability,
  summariseAdapter,
  WANTED_FEATURES,
} from "./platform";

/** The UHD 620's features under the forced switches (probes of 2026-09-29). */
const INTEL_FEATURES: ReadonlyArray<GPUFeatureName> = [
  "subgroups",
  "shader-f16",
  "timestamp-query",
  "float32-filterable",
  "rg11b10ufloat-renderable",
  "depth-clip-control",
  "texture-compression-bc",
];

/** SwiftShader's, which lack `shader-f16` (R01 Design note 17). */
const SWIFTSHADER_FEATURES: ReadonlyArray<GPUFeatureName> = [
  "subgroups",
  "timestamp-query",
  "float32-filterable",
  "float32-blendable",
];

async function outcomeOf(adapter: FakeAdapter): Promise<AdapterOutcome> {
  return requestAdapterOutcome(new FakeGpu([adapter]));
}

describe("adapter acquisition", () => {
  it("gives no-webgpu without navigator.gpu", async () => {
    expect(await requestAdapterOutcome(undefined)).toEqual({ kind: "no-webgpu" });
  });

  it("gives no-adapter for a null adapter", async () => {
    expect(await requestAdapterOutcome(new FakeGpu([null]))).toEqual({ kind: "no-adapter" });
  });

  it("asks for a high-performance adapter", async () => {
    const gpu = new FakeGpu([null]);
    await requestAdapterOutcome(gpu);
    expect(gpu.requests).toEqual([{ powerPreference: "high-performance" }]);
  });

  it("offers both styles on the Intel part", async () => {
    const outcome = await outcomeOf(
      new FakeAdapter({ info: INTEL_UHD_620_INFO, features: INTEL_FEATURES }),
    );
    expect(outcome.kind === "adapter" ? outcome.styles : undefined).toEqual({
      wireframe: true,
      photorealistic: true,
    });
    expect(outcome.kind === "adapter" ? outcome.summary : undefined).toEqual({
      vendor: "intel",
      architecture: "gen-9",
      description: "",
      fallback: false,
    });
  });

  it("offers the wireframe only on SwiftShader", async () => {
    const outcome = await outcomeOf(
      new FakeAdapter({ info: SWIFTSHADER_INFO, features: SWIFTSHADER_FEATURES }),
    );
    expect(outcome.kind === "adapter" ? outcome.styles : undefined).toEqual({
      wireframe: true,
      photorealistic: false,
    });
  });

  it("offers the wireframe only on either software signal alone", () => {
    const flagged = new FakeAdapter({
      info: { ...INTEL_UHD_620_INFO, isFallbackAdapter: true },
      features: [],
    });
    const named = new FakeAdapter({
      info: { ...INTEL_UHD_620_INFO, architecture: "swiftshader" },
      features: [],
    });
    for (const adapter of [flagged, named]) {
      expect(styleAvailability(summariseAdapter(adapter).summary)).toEqual({
        wireframe: true,
        photorealistic: false,
      });
    }
  });
});

describe("the capability summary", () => {
  it("reads each feature and the 2D texture limit", () => {
    const { capabilities } = summariseAdapter(
      new FakeAdapter({
        info: INTEL_UHD_620_INFO,
        features: INTEL_FEATURES,
        maxTextureDimension2D: 16_384,
      }),
    );
    expect(capabilities).toEqual({
      subgroups: true,
      shaderF16: true,
      timestampQuery: true,
      float32Filterable: true,
      float32Blendable: false,
      rg11b10Renderable: true,
      depthClipControl: true,
      maxTextureDimension2D: 16_384,
      subgroupMinSize: 8,
    });
  });

  it("reads SwiftShader's features", () => {
    const { capabilities } = summariseAdapter(
      new FakeAdapter({ info: SWIFTSHADER_INFO, features: SWIFTSHADER_FEATURES }),
    );
    expect(capabilities.shaderF16).toBe(false);
    expect(capabilities.float32Blendable).toBe(true);
    expect(capabilities.subgroupMinSize).toBe(4);
  });

  it("gives no subgroup size without subgroups", () => {
    const { capabilities } = summariseAdapter(
      new FakeAdapter({ info: INTEL_UHD_620_INFO, features: ["shader-f16"] }),
    );
    expect(capabilities.subgroups).toBe(false);
    expect(capabilities.subgroupMinSize).toBeNull();
  });
});

describe("the required features", () => {
  const intel = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: INTEL_FEATURES });

  it("are the wanted features the adapter has", () => {
    expect(requiredFeatures(intel, undefined)).toEqual(
      WANTED_FEATURES.filter((feature) => INTEL_FEATURES.includes(feature)),
    );
    expect(requiredFeatures(intel, undefined)).not.toContain("texture-compression-bc");
  });

  it("leave out what the overrides withhold", () => {
    const required = requiredFeatures(intel, {
      withholdSubgroups: true,
      withholdShaderF16: true,
    });
    expect(required).not.toContain("subgroups");
    expect(required).not.toContain("shader-f16");
    expect(required).toContain("timestamp-query");
  });

  it("leave out float32-blendable when withheld", () => {
    const swiftShader = new FakeAdapter({ info: SWIFTSHADER_INFO, features: SWIFTSHADER_FEATURES });
    const required = requiredFeatures(swiftShader, {
      withholdSubgroups: false,
      withholdShaderF16: false,
      withholdFloat32Blendable: true,
    });
    expect(required).toEqual(["subgroups", "timestamp-query", "float32-filterable"]);
  });
});

describe("the fake adapter", () => {
  it("is consumed by its first requestDevice", async () => {
    const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: INTEL_FEATURES });
    await adapter.requestDevice();
    const second = await adapter.requestDevice();
    expect((await second.lost).message).toBe("the adapter was already consumed");
  });
});
