import { describe, expect, it, vi } from "vitest";

import { FakeAdapter, type FakeDevice, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import type { KernelPair } from "../kernels";
import { summariseAdapter } from "../platform";
import type { BufferHandle } from "../types";
import { createKernel, createKernelAsync, type KernelPipelineSpec, prepareKernel } from "./compute";
import { KernelCache } from "./kernelCache";

const DOUBLER: KernelPair = {
  name: "doubler",
  reference: [
    "@group(0) @binding(0) var<uniform> params : vec4u;",
    "@group(0) @binding(1) var<storage, read_write> values : array<u32>;",
  ].join("\n"),
  subgroup: null,
  readback: "bit-exact",
};

/** A cache on a fake device, compiling through the engine's own functions. */
async function cacheOnDevice(): Promise<{
  readonly cache: KernelCache;
  readonly gpu: FakeDevice;
  readonly destroyed: BufferHandle[];
  readonly spec: (pair: KernelPair) => KernelPipelineSpec;
}> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const gpu = adapter.devices.at(0);
  if (gpu === undefined) {
    throw new Error("the fake adapter made no device");
  }
  const { capabilities } = summariseAdapter(adapter);
  const destroyed: BufferHandle[] = [];
  const cache = new KernelCache({
    compile: (pair, spec) => createKernel(gpu, pair, spec),
    compileAsync: (pair, spec) => createKernelAsync(gpu, pair, spec),
    destroyBuffer: (buffer) => {
      destroyed.push(buffer);
    },
  });
  return { cache, gpu, destroyed, spec: (pair) => prepareKernel(pair, capabilities) };
}

/** A uniform buffer's handle, as a dispatch makes one into a record's `uniforms`. */
function uniformBuffer(name: string): BufferHandle {
  return Object.freeze({ kind: "buffer", name, bytes: 16 });
}

describe("the kernel cache (R01.T15)", () => {
  it("makes one pipeline for one name and sources, a new handle over it each call", async () => {
    const { cache, gpu, spec } = await cacheOnDevice();
    const first = cache.create(DOUBLER, spec(DOUBLER));
    const copy = { ...DOUBLER };
    const second = cache.create(copy, spec(copy));
    const third = await cache.createAsync(DOUBLER, spec(DOUBLER));
    expect(gpu.computePipelines).toHaveLength(1);
    expect(new Set([first, second, third]).size).toBe(3);
    expect(cache.kernelOf(second).record).toBe(cache.kernelOf(first).record);
    expect(cache.kernelOf(third).record).toBe(cache.kernelOf(first).record);
    expect(cache.size).toBe(1);
  });

  it("makes a pipeline of its own for a spec that differs only in its entry point, or only in a constant", async () => {
    const { cache, gpu, spec } = await cacheOnDevice();
    const base = spec(DOUBLER);
    const handles = [
      cache.create(DOUBLER, base),
      cache.create(DOUBLER, { ...base, entryPoint: "half" }),
      cache.create(DOUBLER, { ...base, constants: { factor: 2 } }),
      cache.create(DOUBLER, { ...base, constants: { factor: 3 } }),
    ];
    expect(gpu.computePipelines.map((p) => p.descriptor.compute.entryPoint)).toEqual([
      "main",
      "half",
      "main",
      "main",
    ]);
    expect(gpu.computePipelines.map((p) => p.descriptor.compute.constants)).toEqual([
      undefined,
      undefined,
      { factor: 2 },
      { factor: 3 },
    ]);
    expect(new Set(handles.map((handle) => cache.kernelOf(handle).record)).size).toBe(4);
  });

  it("shares one compile among concurrent asynchronous creates of a key", async () => {
    const { cache, gpu, spec } = await cacheOnDevice();
    const compiles = vi.spyOn(gpu, "createComputePipelineAsync");
    const [a, b, c] = await Promise.all([
      cache.createAsync(DOUBLER, spec(DOUBLER)),
      cache.createAsync(DOUBLER, spec(DOUBLER)),
      cache.createAsync({ ...DOUBLER }, spec(DOUBLER)),
    ]);
    expect(compiles).toHaveBeenCalledOnce();
    expect(new Set([a, b, c].map((handle) => cache.kernelOf(handle).record)).size).toBe(1);
    expect(cache.size).toBe(1);
  });

  it("serves a synchronous create from a record compiled asynchronously, and the reverse", async () => {
    const { cache, gpu, spec } = await cacheOnDevice();
    const asynchronous = await cache.createAsync(DOUBLER, spec(DOUBLER));
    const synchronous = cache.create(DOUBLER, spec(DOUBLER));
    const other = { ...DOUBLER, name: "other" };
    const made = cache.create(other, spec(other));
    const served = await cache.createAsync(other, spec(other));
    expect(gpu.computePipelines).toHaveLength(2);
    expect(cache.kernelOf(synchronous).record).toBe(cache.kernelOf(asynchronous).record);
    expect(cache.kernelOf(served).record).toBe(cache.kernelOf(made).record);
  });

  it("rejects every create waiting on a compile that rejects, and keeps nothing of it", async () => {
    const { cache, gpu, spec } = await cacheOnDevice();
    vi.spyOn(gpu, "createComputePipelineAsync").mockRejectedValueOnce(new Error("refused"));
    const settled = await Promise.allSettled([
      cache.createAsync(DOUBLER, spec(DOUBLER)),
      cache.createAsync(DOUBLER, spec(DOUBLER)),
    ]);
    expect(
      settled.map((each) => (each.status === "rejected" ? String(each.reason) : "made")),
    ).toEqual(["Error: refused", "Error: refused"]);
    expect(cache.size).toBe(0);
    await cache.createAsync(DOUBLER, spec(DOUBLER));
    expect([cache.size, gpu.computePipelines.length]).toEqual([1, 1]);
  });

  it("keeps a record and its uniform buffers while any handle holds it, and lets them go at the last release", async () => {
    const { cache, destroyed, spec } = await cacheOnDevice();
    const first = cache.create(DOUBLER, spec(DOUBLER));
    const second = cache.create(DOUBLER, spec(DOUBLER));
    const params = uniformBuffer("doubler:params");
    cache.kernelOf(first).uniforms.set("params", params);
    cache.release(first);
    expect([cache.size, destroyed]).toEqual([1, []]);
    expect(cache.kernelOf(second).uniforms.get("params")).toBe(params);
    cache.release(second);
    expect([cache.size, destroyed]).toEqual([0, [params]]);
  });

  it("returns to holding nothing once N distinct kernels' handles are released", async () => {
    const { cache, destroyed, spec } = await cacheOnDevice();
    const handles = Array.from({ length: 8 }, (_, i) => {
      const pair = { ...DOUBLER, name: `doubler ${i}` };
      const handle = cache.create(pair, spec(pair));
      cache.kernelOf(handle).uniforms.set("params", uniformBuffer(`${pair.name}:params`));
      return handle;
    });
    expect(cache.size).toBe(8);
    for (const handle of handles) {
      cache.release(handle);
    }
    expect([cache.size, destroyed.length]).toEqual([0, 8]);
    for (const handle of handles) {
      expect(() => cache.kernelOf(handle)).toThrow(/was released/u);
    }
  });

  it("compiles a key again after its last release", async () => {
    const { cache, gpu, spec } = await cacheOnDevice();
    cache.release(cache.create(DOUBLER, spec(DOUBLER)));
    cache.create(DOUBLER, spec(DOUBLER));
    expect(gpu.computePipelines).toHaveLength(2);
  });

  it("refuses a released handle and a second release, naming the release, and a handle it did not make", async () => {
    const { cache, spec } = await cacheOnDevice();
    const handle = cache.create(DOUBLER, spec(DOUBLER));
    cache.release(handle);
    expect(() => cache.kernelOf(handle)).toThrow("kernel doubler was released");
    expect(() => {
      cache.release(handle);
    }).toThrow("kernel doubler was released");
    const stranger = Object.freeze({
      kind: "compute",
      name: "stranger",
      path: "reference",
    } as const);
    expect(() => cache.kernelOf(stranger)).toThrow("kernel stranger was not made by this engine");
  });

  it("forgets every record at a clear, a create then compiling again, and still serves and releases a handle made before", async () => {
    const { cache, gpu, destroyed, spec } = await cacheOnDevice();
    const before = cache.create(DOUBLER, spec(DOUBLER));
    const params = uniformBuffer("doubler:params");
    cache.kernelOf(before).uniforms.set("params", params);
    cache.clear();
    expect(cache.size).toBe(0);
    const after = cache.create(DOUBLER, spec(DOUBLER));
    expect(gpu.computePipelines).toHaveLength(2);
    expect(cache.kernelOf(before).record).not.toBe(cache.kernelOf(after).record);
    cache.release(before);
    expect([cache.size, destroyed]).toEqual([1, [params]]);
  });

  it("keeps no record whose compile ends after a clear", async () => {
    const { cache, gpu, spec } = await cacheOnDevice();
    const compiling = cache.createAsync(DOUBLER, spec(DOUBLER));
    cache.clear();
    const handle = await compiling;
    expect(cache.size).toBe(0);
    cache.create(DOUBLER, spec(DOUBLER));
    expect(gpu.computePipelines).toHaveLength(2);
    expect(() => {
      cache.release(handle);
    }).not.toThrow();
  });

  it("leaves a compile begun after a clear for the next create to join, when an older one ends", async () => {
    const { cache, gpu, spec } = await cacheOnDevice();
    // Each compile is held until the test lets it end.
    const held: Array<() => void> = [];
    const compiles = vi.spyOn(gpu, "createComputePipelineAsync").mockImplementation(
      (descriptor) =>
        new Promise((resolve) => {
          held.push(() => {
            resolve(gpu.createComputePipeline(descriptor));
          });
        }),
    );
    const older = cache.createAsync(DOUBLER, spec(DOUBLER));
    cache.clear();
    const newer = cache.createAsync(DOUBLER, spec(DOUBLER));
    held[0]?.();
    await older;
    const joining = cache.createAsync(DOUBLER, spec(DOUBLER));
    for (const end of held.slice(1)) {
      end();
    }
    const [second, third] = await Promise.all([newer, joining]);
    expect(compiles).toHaveBeenCalledTimes(2);
    expect(cache.kernelOf(third).record).toBe(cache.kernelOf(second).record);
  });
});
