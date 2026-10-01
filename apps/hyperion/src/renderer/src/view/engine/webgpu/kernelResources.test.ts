import { describe, expect, it } from "vitest";

import { FakeAdapter, type FakeDevice, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import { BUFFER_USAGE } from "../gpuFlags";
import type { BufferHandle } from "../types";
import type { KernelRecord } from "./compute";
import { resolveKernelResources, uniformBufferBytes } from "./kernelResources";
import { packedCubeSpec, ResourceRegistry } from "./resources";

async function setUp(): Promise<{
  readonly gpu: FakeDevice;
  readonly resources: ResourceRegistry;
}> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const gpu = adapter.devices.at(0);
  if (gpu === undefined) {
    throw new Error("the fake adapter made no device");
  }
  return { gpu, resources: new ResourceRegistry(gpu, () => undefined) };
}

/** A kernel record whose pipeline the resolution never touches. */
const KERNEL: KernelRecord = {
  pair: { name: "bake", reference: "", subgroup: null, readback: "bit-exact" },
  path: "reference",
  pipeline: {
    label: "bake",
    getBindGroupLayout: () => ({ label: "" }),
  },
  bindings: new Map(),
};

const NONE = { uniforms: {}, buffers: {}, sampled: {}, storage: {} };

describe("a kernel's uniforms", () => {
  it("are sized in 16-byte steps, at least 16", () => {
    expect(uniformBufferBytes(4)).toBe(16);
    expect(uniformBufferBytes(16)).toBe(16);
    expect(uniformBufferBytes(20)).toBe(32);
  });

  it("go in one buffer per name, made once and written each dispatch", async () => {
    const { gpu, resources } = await setUp();
    const uniforms = new Map<string, BufferHandle>();
    const bindings = { ...NONE, uniforms: { scale: new Float32Array(3) } };
    resolveKernelResources(resources, KERNEL, bindings, uniforms);
    resolveKernelResources(resources, KERNEL, bindings, uniforms);
    expect(gpu.buffers.map(({ label, size }) => [label, size])).toEqual([["bake:scale", 16]]);
    expect(gpu.queue.writes).toHaveLength(2);
  });

  it("refuse a value larger than the buffer its first value made", async () => {
    const { resources } = await setUp();
    const uniforms = new Map<string, BufferHandle>();
    resolveKernelResources(
      resources,
      KERNEL,
      { ...NONE, uniforms: { m: new Float32Array(4) } },
      uniforms,
    );
    expect(() =>
      resolveKernelResources(
        resources,
        KERNEL,
        { ...NONE, uniforms: { m: new Float32Array(8) } },
        uniforms,
      ),
    ).toThrow(/uniform m of kernel bake grew from 16 to 32 bytes/u);
  });
});

describe("a kernel's storage texture", () => {
  it("binds a cube's level as a 2D array of its faces", async () => {
    const { gpu, resources } = await setUp();
    const cube = resources.createTexture(packedCubeSpec(8, 3, "other"));
    resolveKernelResources(
      resources,
      KERNEL,
      { ...NONE, storage: { faces: { texture: cube, level: 2 } } },
      new Map(),
    );
    expect(gpu.textures.at(0)?.views.at(0)).toMatchObject({
      dimension: "2d-array",
      baseMipLevel: 2,
      mipLevelCount: 1,
    });
  });
});

describe("a kernel's buffers", () => {
  it("resolve to the registry's GPU buffers", async () => {
    const { gpu, resources } = await setUp();
    const values = resources.createBuffer({
      name: "values",
      bytes: 64,
      usage: BUFFER_USAGE.STORAGE,
      category: "other",
    });
    const resolved = resolveKernelResources(
      resources,
      KERNEL,
      { ...NONE, buffers: { values } },
      new Map(),
    );
    expect(resolved.get("values")).toEqual({ kind: "buffer", buffer: gpu.buffers.at(0) });
  });
});
