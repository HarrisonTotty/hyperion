import { describe, expect, it } from "vitest";

import { FakeAdapter, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import type { AllocationEvent } from "../memory";
import { summariseAdapter } from "../platform";
import { Float32BlendUnavailable, type PointSplatSpec } from "../types";
import {
  assertSplatInputs,
  assertSplatSupported,
  pointSplatPipelineDescriptor,
  SPLAT_BLEND,
  splatTargetSpec,
} from "./pointSplat";
import { ResourceRegistry } from "./resources";

const SPEC: PointSplatSpec = {
  name: "sky",
  vertexWgsl: "vertex",
  fragmentWgsl: "fragment",
  format: "rgba32float",
  blend: "additive",
};

const MODULE: GPUShaderModule = {
  label: "m",
  getCompilationInfo: () => Promise.reject(new Error("unused")),
};

function capabilities(
  features: ReadonlyArray<GPUFeatureName>,
): ReturnType<typeof summariseAdapter>["capabilities"] {
  return summariseAdapter(new FakeAdapter({ info: SWIFTSHADER_INFO, features })).capabilities;
}

describe("the point splat", () => {
  it("draws a point list into rgba32float, adding every channel", () => {
    const descriptor = pointSplatPipelineDescriptor(SPEC, MODULE, MODULE);
    expect(descriptor.primitive).toEqual({ topology: "point-list" });
    expect(descriptor.fragment?.targets).toEqual([{ format: "rgba32float", blend: SPLAT_BLEND }]);
    expect(SPLAT_BLEND).toEqual({
      color: { srcFactor: "one", dstFactor: "one", operation: "add" },
      alpha: { srcFactor: "one", dstFactor: "one", operation: "add" },
    });
  });

  it("is refused without float32-blendable", () => {
    expect(() => {
      assertSplatSupported(capabilities(["float32-filterable"]));
    }).toThrow(Float32BlendUnavailable);
  });

  it("is made with float32-blendable", () => {
    expect(() => {
      assertSplatSupported(capabilities(["float32-blendable"]));
    }).not.toThrow();
  });

  it("bakes into a target whose bytes are raised as an allocation", async () => {
    const device = await new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] }).requestDevice();
    const events: AllocationEvent[] = [];
    const resources = new ResourceRegistry(device, (event) => events.push(event));
    const spec = splatTargetSpec("sky face", 64, "other");
    resources.createTexture(spec);
    expect(spec.usage & TEXTURE_USAGE.RENDER_ATTACHMENT).not.toBe(0);
    expect(events).toEqual([
      { kind: "created", name: "sky face", bytes: 64 * 64 * 16, category: "other" },
    ]);
  });
});

describe("a splat's inputs", () => {
  const target = splatTargetSpec("face", 4, "other");
  const points = {
    name: "points",
    bytes: 32,
    usage: BUFFER_USAGE.STORAGE,
    category: "other",
  } as const;

  it("are accepted as a bake target and a storage buffer", () => {
    expect(() => {
      assertSplatInputs(SPEC, target, points);
    }).not.toThrow();
  });

  it("refuse a target of another format", () => {
    expect(() => {
      assertSplatInputs(SPEC, { ...target, format: "rgba16float" }, points);
    }).toThrow(/2D rgba32float, not face/u);
  });

  it("refuse a target of several layers", () => {
    expect(() => {
      assertSplatInputs(SPEC, { ...target, size: [4, 4, 2] }, points);
    }).toThrow(/one layer/u);
  });

  it("refuse a target without RENDER_ATTACHMENT", () => {
    expect(() => {
      assertSplatInputs(SPEC, { ...target, usage: TEXTURE_USAGE.TEXTURE_BINDING }, points);
    }).toThrow(/RENDER_ATTACHMENT/u);
  });

  it("refuse points without STORAGE", () => {
    expect(() => {
      assertSplatInputs(SPEC, target, { ...points, usage: BUFFER_USAGE.VERTEX });
    }).toThrow(/STORAGE/u);
  });
});
