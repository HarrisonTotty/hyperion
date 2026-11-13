import { describe, expect, it } from "vitest";

import { FakeAdapter, type FakeDevice, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import type { TextureSpec } from "../memory";
import type { BufferHandle } from "../types";
import { kernelBindings, type KernelRecord } from "./compute";
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

/** {@link KERNEL} with the bindings `wgsl` declares. */
function declaring(wgsl: string): KernelRecord {
  return { ...KERNEL, bindings: kernelBindings(wgsl) };
}

/** A sampled and storage `rgba16float` texture, 4 × 4, of `layers` layers and `mips` levels. */
function layered(
  name: string,
  layers: number,
  dimension: "2d" | "3d" = "2d",
  mips = 1,
): TextureSpec {
  return {
    name,
    size: [4, 4, layers],
    dimension,
    format: "rgba16float",
    mips,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.STORAGE_BINDING,
    category: "other",
  };
}

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

describe("a kernel's texture, viewed at the dimension its kernel declares (R08.T0)", () => {
  it("is a one-layer array where a single-layer 2D texture is sampled as texture_2d_array", async () => {
    const { gpu, resources } = await setUp();
    const table = resources.createTexture(layered("table", 1));
    resolveKernelResources(
      resources,
      declaring("@group(0) @binding(0) var table : texture_2d_array<f32>;"),
      { ...NONE, sampled: { table } },
      new Map(),
    );
    expect(gpu.textures.at(0)?.views).toStrictEqual([{ label: "table", dimension: "2d-array" }]);
  });

  it("is a one-layer array at a level where a single-layer 2D texture is texture_storage_2d_array", async () => {
    const { gpu, resources } = await setUp();
    const table = resources.createTexture(layered("table", 1, "2d", 3));
    resolveKernelResources(
      resources,
      declaring("@group(0) @binding(0) var table : texture_storage_2d_array<rgba16float, write>;"),
      { ...NONE, storage: { table: { texture: table, level: 1 } } },
      new Map(),
    );
    expect(gpu.textures.at(0)?.views).toStrictEqual([
      { label: "table level 1", dimension: "2d-array", baseMipLevel: 1, mipLevelCount: 1 },
    ]);
  });

  it("refuses a three-layer texture where texture_2d is declared, naming the kernel and the binding", async () => {
    const { gpu, resources } = await setUp();
    const layers = resources.createTexture(layered("layers", 3));
    const kernel = declaring("@group(0) @binding(0) var tiles : texture_2d<f32>;");
    expect(() =>
      resolveKernelResources(resources, kernel, { ...NONE, sampled: { tiles: layers } }, new Map()),
    ).toThrow(/^kernel bake declares tiles as 2d, but layers is 2d-array$/u);
    expect(gpu.textures.at(0)?.views).toEqual([]);
  });

  it("refuses before any uniform is written or any view made", async () => {
    const { gpu, resources } = await setUp();
    const flat = resources.createTexture(layered("flat", 1));
    const layers = resources.createTexture(layered("layers", 3));
    const kernel = declaring(
      [
        "@group(0) @binding(0) var<uniform> scale : vec4f;",
        "@group(0) @binding(1) var flat : texture_2d<f32>;",
        "@group(0) @binding(2) var out : texture_storage_2d<rgba16float, write>;",
      ].join("\n"),
    );
    const uniforms = new Map<string, BufferHandle>();
    expect(() =>
      resolveKernelResources(
        resources,
        kernel,
        {
          ...NONE,
          uniforms: { scale: new Float32Array(4) },
          sampled: { flat },
          storage: { out: { texture: layers, level: 0 } },
        },
        uniforms,
      ),
    ).toThrow(/kernel bake declares out as 2d, but layers is 2d-array/u);
    expect(uniforms.size).toBe(0);
    expect(gpu.queue.writes).toEqual([]);
    expect(gpu.textures.flatMap(({ views }) => views)).toEqual([]);
  });

  it("refuses a cube where a 2D storage texture is declared", async () => {
    const { resources } = await setUp();
    const cube = resources.createTexture(packedCubeSpec(8, 1, "other", "sky"));
    const kernel = declaring(
      "@group(0) @binding(0) var faces : texture_storage_2d<rgba16float, write>;",
    );
    expect(() =>
      resolveKernelResources(
        resources,
        kernel,
        { ...NONE, storage: { faces: { texture: cube, level: 0 } } },
        new Map(),
      ),
    ).toThrow(/kernel bake declares faces as 2d, but sky is cube/u);
  });

  it("keeps a cube's storage a six-layer 2d-array where texture_storage_2d_array is declared", async () => {
    const { gpu, resources } = await setUp();
    const cube = resources.createTexture(packedCubeSpec(8, 3, "other", "sky"));
    resolveKernelResources(
      resources,
      declaring("@group(0) @binding(0) var faces : texture_storage_2d_array<rgba16float, write>;"),
      { ...NONE, storage: { faces: { texture: cube, level: 2 } } },
      new Map(),
    );
    // No layer range: the view holds every one of the cube's six layers.
    expect(gpu.textures.at(0)?.views).toStrictEqual([
      { label: "sky level 2", dimension: "2d-array", baseMipLevel: 2, mipLevelCount: 1 },
    ]);
    expect(gpu.textures.at(0)?.depthOrArrayLayers).toBe(6);
  });

  it("views a 3D texture as 3d, sampled and as storage", async () => {
    const { gpu, resources } = await setUp();
    const volume = resources.createTexture(layered("volume", 4, "3d"));
    resolveKernelResources(
      resources,
      declaring(
        [
          "@group(0) @binding(0) var source : texture_3d<f32>;",
          "@group(0) @binding(1) var target : texture_storage_3d<rgba16float, write>;",
        ].join("\n"),
      ),
      { ...NONE, sampled: { source: volume }, storage: { target: { texture: volume, level: 0 } } },
      new Map(),
    );
    expect(gpu.textures.at(0)?.views.map((view) => view?.dimension)).toEqual(["3d", "3d"]);
  });

  it("is left at the texture's own dimension under a name declared as a buffer", async () => {
    const { gpu, resources } = await setUp();
    const layers = resources.createTexture(layered("layers", 3));
    resolveKernelResources(
      resources,
      declaring("@group(0) @binding(0) var<uniform> scale : vec4f;"),
      { ...NONE, sampled: { scale: layers } },
      new Map(),
    );
    expect(gpu.textures.at(0)?.views).toStrictEqual([{ label: "layers", dimension: "2d-array" }]);
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
