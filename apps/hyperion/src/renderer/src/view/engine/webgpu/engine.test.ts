import { describe, expect, it, vi } from "vitest";

import { FakeAdapter, type FakeDevice, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import type { KernelPair } from "../kernels";
import type { AllocationEvent, TextureSpec } from "../memory";
import { GraphicsStatusStore, initialGraphicsStatus } from "../status";
import { type PassTimes, PresentationOnlyReadback, type TextureHandle } from "../types";
import { WebGpuRenderEngine } from "./engine";

/** An engine over a fake device with `features`. */
async function engineOn(features: ReadonlyArray<GPUFeatureName> = []): Promise<{
  readonly engine: WebGpuRenderEngine;
  readonly gpu: FakeDevice;
  readonly status: GraphicsStatusStore;
}> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features });
  await adapter.requestDevice({ requiredFeatures: [...features] });
  const gpu = adapter.devices.at(0);
  if (gpu === undefined) {
    throw new Error("the fake adapter made no device");
  }
  const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  return { engine: new WebGpuRenderEngine(gpu, status), gpu, status };
}

/** A storage texture of `mips` levels the CPU can read back. */
function storageTexture(name: string, mips = 1): TextureSpec {
  return {
    name,
    size: [4, 4],
    dimension: "2d",
    format: "rgba16float",
    mips,
    usage: TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.COPY_SRC | TEXTURE_USAGE.COPY_DST,
    category: "other",
  };
}

/** A kernel that writes `out`, a storage texture. */
const SPLATTER: KernelPair = {
  name: "splatter",
  reference: "@group(0) @binding(0) var out : texture_storage_2d<rgba16float, write>;",
  subgroup: null,
  readback: "presentation-only",
};

/** A bit-exact kernel that only reads `source`, a storage texture, into `total`. */
const SUMMER: KernelPair = {
  name: "summer",
  reference: [
    "@group(0) @binding(0) var source : texture_storage_2d<rgba16float, read>;",
    "@group(0) @binding(1) var<storage, read_write> total : array<u32>;",
  ].join("\n"),
  subgroup: null,
  readback: "bit-exact",
};

/** A presentation-only kernel that only reads `source`, a storage texture, into `out`. */
const BLURRER: KernelPair = {
  name: "blurrer",
  reference: [
    "@group(0) @binding(0) var source : texture_storage_2d<rgba16float, read>;",
    "@group(0) @binding(1) var out : texture_storage_2d<rgba16float, write>;",
  ].join("\n"),
  subgroup: null,
  readback: "presentation-only",
};

const NO_BINDINGS = { uniforms: {}, buffers: {}, sampled: {} };

/** Writes `texture` with the presentation-only splatter. */
function splat(engine: WebGpuRenderEngine, texture: TextureHandle): void {
  engine.dispatch(
    engine.createCompute(SPLATTER),
    { ...NO_BINDINGS, storage: { out: { texture, level: 0 } } },
    [1, 1, 1],
  );
}

describe("the readback guard", () => {
  it("keeps a presentation-only texture refused after a bit-exact kernel only reads it", async () => {
    const { engine } = await engineOn();
    const image = engine.createTexture(storageTexture("image"));
    splat(engine, image);
    const total = engine.createBuffer({
      name: "total",
      bytes: 16,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
      category: "other",
    });
    engine.dispatch(
      engine.createCompute(SUMMER),
      {
        ...NO_BINDINGS,
        buffers: { total },
        storage: { source: { texture: image, level: 0 } },
      },
      [1, 1, 1],
    );
    await expect(engine.readTexture(image)).rejects.toBeInstanceOf(PresentationOnlyReadback);
    await expect(engine.readBuffer(total)).resolves.toBeInstanceOf(ArrayBuffer);
  });

  it("leaves a texture readable after a presentation-only kernel only reads it", async () => {
    const { engine } = await engineOn();
    const source = engine.createTexture(storageTexture("source"));
    const out = engine.createTexture(storageTexture("out"));
    engine.dispatch(
      engine.createCompute(BLURRER),
      {
        ...NO_BINDINGS,
        storage: { source: { texture: source, level: 0 }, out: { texture: out, level: 0 } },
      },
      [1, 1, 1],
    );
    await expect(engine.readTexture(source)).resolves.toBeInstanceOf(ArrayBuffer);
    await expect(engine.readTexture(out)).rejects.toBeInstanceOf(PresentationOnlyReadback);
  });

  it("keeps a presentation-only texture refused after a CPU write of part of it", async () => {
    const { engine } = await engineOn();
    const image = engine.createTexture(storageTexture("image"));
    splat(engine, image);
    engine.writeTexture(image, [0, 0, 0], [1, 1, 1], new Uint16Array(4));
    await expect(engine.readTexture(image)).rejects.toBeInstanceOf(PresentationOnlyReadback);
    engine.writeTexture(image, [0, 0, 0], [4, 4, 1], new Uint16Array(64));
    await expect(engine.readTexture(image)).resolves.toBeInstanceOf(ArrayBuffer);
  });

  it("keeps a presentation-only texture refused after a kernel writes one level of several", async () => {
    const { engine } = await engineOn();
    const image = engine.createTexture(storageTexture("image", 3));
    splat(engine, image);
    engine.dispatch(
      engine.createCompute({ ...SPLATTER, name: "levels", readback: "bit-exact" }),
      { ...NO_BINDINGS, storage: { out: { texture: image, level: 1 } } },
      [1, 1, 1],
    );
    await expect(engine.readTexture(image)).rejects.toBeInstanceOf(PresentationOnlyReadback);
  });

  it("keeps a presentation-only buffer refused after a CPU write of part of it", async () => {
    const { engine } = await engineOn();
    const counts = engine.createBuffer({
      name: "counts",
      bytes: 64,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    engine.dispatch(
      engine.createCompute({
        name: "histogram",
        reference: "@group(0) @binding(0) var<storage, read_write> counts : array<u32>;",
        subgroup: null,
        readback: "presentation-only",
      }),
      { ...NO_BINDINGS, storage: {}, buffers: { counts } },
      [1, 1, 1],
    );
    engine.writeBuffer(counts, 0, new Uint32Array(1));
    await expect(engine.readBuffer(counts)).rejects.toBeInstanceOf(PresentationOnlyReadback);
    engine.writeBuffer(counts, 0, new Uint32Array(16));
    await expect(engine.readBuffer(counts)).resolves.toBeInstanceOf(ArrayBuffer);
  });

  it("keeps a presentation-only cube refused after the CPU writes one of its levels", async () => {
    const { engine } = await engineOn();
    const cube = engine.createPackedCube(4, 3, "other");
    const packed = engine.createBuffer({
      name: "packed",
      bytes: 256 * 4 * 6,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
      category: "other",
    });
    engine.dispatch(
      engine.createCompute({
        name: "packer",
        reference: "@group(0) @binding(0) var<storage, read_write> packed : array<u32>;",
        subgroup: null,
        readback: "presentation-only",
      }),
      { ...NO_BINDINGS, storage: {}, buffers: { packed } },
      [1, 1, 1],
    );
    engine.writePackedCubeLevelFromBuffer(cube, 0, packed);
    engine.writePackedCubeLevel(cube, 1, new Uint32Array(2 * 2 * 6));
    await expect(engine.readTexture(cube)).rejects.toBeInstanceOf(PresentationOnlyReadback);
  });
});

describe("the engine's pass times", () => {
  it("count the query set and the timer's buffers as allocations", async () => {
    const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: ["timestamp-query"] });
    const device = await adapter.requestDevice({ requiredFeatures: ["timestamp-query"] });
    const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const events: AllocationEvent[] = [];
    const engine = new WebGpuRenderEngine(device, status);
    engine.onAllocation((event) => events.push(event));
    const image = engine.createTexture(storageTexture("image"));
    splat(engine, image);
    renderEmptyFrame(engine, "frame");
    engine.dispose();
    expect(events.filter(({ name }) => name.startsWith("pass times"))).toEqual([
      { kind: "created", name: "pass times", bytes: 1024, category: "other" },
      { kind: "created", name: "pass times resolved 1", bytes: 1024, category: "other" },
      { kind: "created", name: "pass times readback 1", bytes: 1024, category: "other" },
      { kind: "destroyed", name: "pass times", bytes: 1024, category: "other" },
      { kind: "destroyed", name: "pass times resolved 1", bytes: 1024, category: "other" },
      { kind: "destroyed", name: "pass times readback 1", bytes: 1024, category: "other" },
    ]);
  });

  it("leave out a dispatch whose encoding threw", async () => {
    const { engine } = await engineOn(["timestamp-query"]);
    const times = vi.fn<(times: PassTimes) => void>();
    engine.onPassTimes(times);
    const image = engine.createTexture(storageTexture("image"));
    const kernel = engine.createCompute(SPLATTER);
    expect(() => {
      engine.dispatch(
        kernel,
        {
          ...NO_BINDINGS,
          storage: { out: { texture: image, level: 0 }, ghost: { texture: image, level: 0 } },
        },
        [1, 1, 1],
        "broken",
      );
    }).toThrow(/declares no binding ghost/u);
    engine.dispatch(
      kernel,
      { ...NO_BINDINGS, storage: { out: { texture: image, level: 0 } } },
      [1, 1, 1],
      "bake",
    );
    renderEmptyFrame(engine, "frame");
    await vi.waitFor(() => {
      expect(times).toHaveBeenCalledOnce();
    });
    expect(times.mock.calls[0]?.[0].passes.map(({ label }) => label)).toEqual(["bake", "frame"]);
  });

  it("resolve a long bake's dispatches before they fill the query set", async () => {
    const { engine } = await engineOn(["timestamp-query"]);
    const times = vi.fn<(times: PassTimes) => void>();
    engine.onPassTimes(times);
    const image = engine.createTexture(storageTexture("image"));
    const kernel = engine.createCompute(SPLATTER);
    for (let pass = 0; pass < 100; pass += 1) {
      engine.dispatch(
        kernel,
        { ...NO_BINDINGS, storage: { out: { texture: image, level: 0 } } },
        [1, 1, 1],
        `bake ${pass}`,
      );
    }
    renderEmptyFrame(engine, "frame");
    await vi.waitFor(() => {
      expect(times).toHaveBeenCalledTimes(4);
    });
    const labels = times.mock.calls.flatMap(([frame]) => frame.passes.map(({ label }) => label));
    expect(labels).toHaveLength(101);
    expect(labels.at(-1)).toBe("frame");
  });
});

/** Renders a frame with no draws into a small target, which resolves the pending pass times. */
function renderEmptyFrame(engine: WebGpuRenderEngine, label: string): void {
  engine
    .createRenderTarget({
      name: label,
      size: { widthPx: 4, heightPx: 4 },
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    })
    .render({
      label,
      viewRotation: new Float32Array(16),
      projection: new Float32Array(16),
      draws: [],
      postProcesses: [],
    });
}
