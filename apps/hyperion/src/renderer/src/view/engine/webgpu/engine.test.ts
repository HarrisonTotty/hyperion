import { beforeEach, describe, expect, it, vi } from "vitest";

import { FakeAdapter, type FakeDevice, FakeGpu, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
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

describe("releases (R06.T13.h)", () => {
  it("raise one destroyed event and refuse the handle afterwards", async () => {
    const { engine } = await engineOn();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    const cube = engine.createPackedCube(4, 3, "other", "sky cube: main");
    engine.releaseTexture(cube);
    expect(events.at(-1)).toEqual({
      kind: "destroyed",
      name: "sky cube: main",
      bytes: events.at(0)?.bytes,
      category: "other",
    });
    expect(() => {
      engine.writePackedCubeLevel(cube, 0, new Uint32Array(4 * 4 * 6));
    }).toThrow(/was released/u);
  });

  it("release a buffer once, and refuse a second release", async () => {
    const { engine } = await engineOn();
    const buffer = engine.createBuffer({
      name: "staging",
      bytes: 256,
      usage: BUFFER_USAGE.COPY_SRC,
      category: "other",
    });
    engine.releaseBuffer(buffer);
    expect(() => {
      engine.releaseBuffer(buffer);
    }).toThrow(/staging was released/u);
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

/** A 4 × 4 sampled `rg16float` texture of `layers` layers. */
function sampled(name: string, layers: number): TextureSpec {
  return {
    name,
    size: { width: 4, height: 4, depthOrArrayLayers: layers },
    dimension: "2d",
    format: "rg16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  };
}

/** Draws `texture` once through a material that declares its binding as `viewDimension`. */
async function drawWith(
  engine: WebGpuRenderEngine,
  texture: TextureHandle,
  viewDimension: "2d" | "2d-array",
): Promise<void> {
  const mesh = engine.createMesh({
    name: "triangle",
    positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  });
  const material = await engine.createMaterialAsync(
    {
      name: `layers-${viewDimension}`,
      displayName: "LAYERS",
      vertexWgsl: "",
      fragmentWgsl: "",
      uniforms: [],
      samplers: [],
      textures: [{ name: "tiles", binding: 0, viewDimension }],
      cullMode: "none",
      depthWrite: false,
      colourWrites: true,
      blend: "none",
    },
    ["rgba16float"],
    [mesh],
  );
  engine
    .createRenderTarget({
      name: "out",
      size: { widthPx: 4, heightPx: 4 },
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    })
    .render({
      label: "out",
      viewRotation: new Float32Array(16),
      projection: new Float32Array(16),
      draws: [
        {
          mesh,
          material,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {},
          textures: { tiles: texture },
        },
      ],
      postProcesses: [],
    });
}

describe("a texture bound where a 2d-array is declared", () => {
  beforeEach(() => {
    // The engine asks `navigator.gpu` for the canvas's format when it makes a material's pipelines.
    vi.stubGlobal("navigator", { gpu: new FakeGpu([]) });
  });

  it("views a single-layer 2D texture as a one-layer array", async () => {
    const { engine, gpu } = await engineOn();
    await drawWith(engine, engine.createTexture(sampled("atlas", 1)), "2d-array");
    expect(gpu.textures.find((t) => t.label === "atlas")?.views).toContainEqual({
      dimension: "2d-array",
    });
  });

  it("gives the same texture a view of its own where a 2d is declared", async () => {
    const { engine, gpu } = await engineOn();
    const atlas = engine.createTexture(sampled("atlas", 1));
    await drawWith(engine, atlas, "2d-array");
    await drawWith(engine, atlas, "2d");
    const views = gpu.textures.find((t) => t.label === "atlas")?.views ?? [];
    expect(views).toContainEqual({ dimension: "2d-array" });
    expect(views).toContainEqual({ dimension: "2d" });
  });

  it("still refuses a layered texture where a 2d is declared", async () => {
    const { engine } = await engineOn();
    await expect(
      drawWith(engine, engine.createTexture(sampled("layers", 2)), "2d"),
    ).rejects.toThrow(/declares tiles as 2d, but layers is 2d-array/u);
  });
});
