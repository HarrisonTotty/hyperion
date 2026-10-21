import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  FakeAdapter,
  FakeCommandBuffer,
  type FakeDevice,
  FakeGpu,
  FakeTextureView,
  INTEL_UHD_620_INFO,
} from "../../../test/fakeGpu";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import type { KernelPair } from "../kernels";
import type { AllocationEvent, TextureSpec } from "../memory";
import { GraphicsStatusStore, initialGraphicsStatus } from "../status";
import {
  type DepthLayerPass,
  DepthSelfSample,
  type DrawItem,
  Float32BlendUnavailable,
  type MaterialHandle,
  type MeshHandle,
  type PassTimes,
  PresentationOnlyReadback,
  type RenderTarget,
  type TextureHandle,
  type WgslMaterialSpec,
} from "../types";
import { pipelineOutputs, samplerKey, WebGpuRenderEngine } from "./engine";

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

  it("refuse a render target's colour, which the target releases, and leave the engine disposable", async () => {
    const { engine } = await engineOn();
    const target = engine.createRenderTarget({
      name: "hdr",
      size: { widthPx: 4, heightPx: 4 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    expect(() => {
      engine.releaseTexture(target.colour);
    }).toThrow(/belongs to a render target/u);
    expect(() => {
      engine.dispose();
    }).not.toThrow();
  });

  it("name each packed cube as its caller asks", async () => {
    const { engine } = await engineOn();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    engine.createPackedCube(2, 1, "other", "sky cube: cockpit");
    engine.createPackedCube(2, 1, "other", "sky cube: main");
    expect(events.map((event) => event.name)).toEqual(["sky cube: cockpit", "sky cube: main"]);
  });
});

describe("createPointSplatAsync (R06.T13.h)", () => {
  const SPLAT = {
    name: "broken splat",
    vertexWgsl: "@vertex fn main() -> @builtin(position) vec4f { return oops; }",
    fragmentWgsl: "@fragment fn main() -> @location(0) vec4f { return vec4f(1.0); }",
    format: "rgba32float",
    blend: "additive",
  } as const;

  it("rejects naming the splat and the compiler's messages when its WGSL does not compile", async () => {
    const { engine, gpu } = await engineOn(["float32-blendable"]);
    gpu.shaderErrors = (code) => (code.includes("oops") ? ["unresolved value 'oops'"] : []);
    gpu.createRenderPipelineAsync = () => Promise.reject(new Error("pipeline creation failed"));
    await expect(engine.createPointSplatAsync(SPLAT)).rejects.toThrow(
      "splat broken splat did not compile:\n1:1 unresolved value 'oops'",
    );
  });

  it("resolves a splat whose pipeline is made", async () => {
    const { engine } = await engineOn(["float32-blendable"]);
    const made = await engine.createPointSplatAsync({ ...SPLAT, name: "good splat" });
    expect(typeof made.draw).toBe("function");
  });

  it("rejects with Float32BlendUnavailable without the feature", async () => {
    const { engine } = await engineOn();
    await expect(engine.createPointSplatAsync(SPLAT)).rejects.toBeInstanceOf(
      Float32BlendUnavailable,
    );
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

  it("number each resolve as it is submitted, as its times report it (R07.T19)", async () => {
    const { engine } = await engineOn(["timestamp-query"]);
    const times = vi.fn<(times: PassTimes) => void>();
    engine.onPassTimes(times);
    const before = engine.passTimesFrame;
    renderEmptyFrame(engine, "first");
    const afterFirst = engine.passTimesFrame;
    renderEmptyFrame(engine, "second");
    await vi.waitFor(() => {
      expect(times).toHaveBeenCalledTimes(2);
    });
    expect([
      before,
      afterFirst,
      engine.passTimesFrame,
      times.mock.calls.map(([t]) => t.frame),
    ]).toEqual([0, 1, 2, [1, 2]]);
  });

  it("stay at frame 0 without timestamp-query (R07.T19)", async () => {
    const { engine } = await engineOn();
    renderEmptyFrame(engine, "frame");
    expect(engine.passTimesFrame).toBe(0);
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

/** A material with one sampled texture, `tiles`, whose pipelines for `meshes` are made. */
function tiledMaterial(
  engine: WebGpuRenderEngine,
  name: string,
  meshes: ReadonlyArray<MeshHandle>,
): Promise<MaterialHandle> {
  return engine.createMaterialAsync(
    {
      name,
      displayName: name.toUpperCase(),
      vertexWgsl: "",
      fragmentWgsl: "",
      uniforms: [],
      samplers: [],
      textures: [{ name: "tiles", binding: 0 }],
      cullMode: "none",
      depthWrite: false,
      colourWrites: true,
      blend: "none",
    },
    ["rgba16float"],
    meshes,
  );
}

/** One frame drawing `mesh` with each of `materials`, its texture `tiles`, into `target`. */
function drawEach(
  target: RenderTarget,
  mesh: MeshHandle,
  materials: ReadonlyArray<MaterialHandle>,
  tiles: TextureHandle,
): void {
  target.render({
    label: "releases",
    viewRotation: new Float32Array(16),
    projection: new Float32Array(16),
    draws: materials.map((material) => ({
      mesh,
      material,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: { tiles },
    })),
    postProcesses: [],
  });
}

/** A triangle with indices: a position buffer and an index buffer. */
function indexedTriangle(engine: WebGpuRenderEngine): MeshHandle {
  return engine.createMesh({
    name: "triangle",
    positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
    indices: new Uint32Array([0, 1, 2]),
    topology: "triangle-list",
    attributes: {},
  });
}

/** A small target to draw into. */
function outTarget(engine: WebGpuRenderEngine): RenderTarget {
  return engine.createRenderTarget({
    name: "out",
    size: { widthPx: 4, heightPx: 4 },
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
}

describe("mesh and material releases (R10.T16.b)", () => {
  beforeEach(() => {
    vi.stubGlobal("navigator", { gpu: new FakeGpu([]) });
  });

  it("destroy a mesh's buffers at once, one destroyed event each with the bytes it was made with", async () => {
    const { engine, gpu } = await engineOn();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    const mesh = indexedTriangle(engine);
    const created = events.flatMap((event) =>
      event.kind === "created" ? [[event.name, event.bytes, event.category]] : [],
    );
    engine.releaseMesh(mesh);
    const destroyed = events.flatMap((event) =>
      event.kind === "destroyed" ? [[event.name, event.bytes, event.category]] : [],
    );
    expect(created.map(([name]) => name)).toEqual(["triangle:position", "triangle:indices"]);
    expect(destroyed).toEqual(created);
    expect(
      gpu.buffers.filter((b) => b.label.startsWith("triangle:")).map((b) => b.destroyed),
    ).toEqual([true, true]);
  });

  it("refuse a released mesh in a draw, in createMaterialAsync's meshes and in a second release", async () => {
    const { engine } = await engineOn();
    const mesh = indexedTriangle(engine);
    const tiles = engine.createTexture(sampled("tiles", 1));
    const material = await tiledMaterial(engine, "flat", [mesh]);
    engine.releaseMesh(mesh);
    expect(() => {
      drawEach(outTarget(engine), mesh, [material], tiles);
    }).toThrow(/mesh triangle was released/u);
    await expect(tiledMaterial(engine, "later", [mesh])).rejects.toThrow(
      /mesh triangle was released/u,
    );
    expect(() => {
      engine.releaseMesh(mesh);
    }).toThrow(/mesh triangle was released/u);
  });

  it("refuse a released material in a draw and in a second release", async () => {
    const { engine } = await engineOn();
    const mesh = indexedTriangle(engine);
    const tiles = engine.createTexture(sampled("tiles", 1));
    const material = await tiledMaterial(engine, "flat", [mesh]);
    engine.releaseMaterial(material);
    expect(() => {
      drawEach(outTarget(engine), mesh, [material], tiles);
    }).toThrow(/material flat was released/u);
    expect(() => {
      engine.releaseMaterial(material);
    }).toThrow(/material flat was released/u);
  });

  it("refuse the release of a mesh or material another engine made", async () => {
    const { engine } = await engineOn();
    const other = (await engineOn()).engine;
    const mesh = indexedTriangle(other);
    expect(() => {
      engine.releaseMesh(mesh);
    }).toThrow(/mesh triangle was not made by this engine/u);
    const material = await tiledMaterial(other, "flat", []);
    expect(() => {
      engine.releaseMaterial(material);
    }).toThrow(/material flat was not made by this engine/u);
  });

  it("leave no cached bind group of a released material: the draw path's cache is cleared", async () => {
    const { engine, gpu } = await engineOn();
    const mesh = indexedTriangle(engine);
    const tiles = engine.createTexture(sampled("tiles", 1));
    const released = await tiledMaterial(engine, "released", [mesh]);
    const kept = await tiledMaterial(engine, "kept", [mesh]);
    const target = outTarget(engine);
    drawEach(target, mesh, [released, kept], tiles);
    const made = gpu.bindGroups.length;
    // Cached: the same frame again makes no bind group.
    drawEach(target, mesh, [kept], tiles);
    expect(gpu.bindGroups).toHaveLength(made);
    engine.releaseMaterial(released);
    // Every cached group is gone, the released material's among them: the kept one's is made again.
    drawEach(target, mesh, [kept], tiles);
    expect(gpu.bindGroups.length).toBeGreaterThan(made);
  });
});

/** A material of `createMaterial` reading one sampled texture, `tiles`. */
const SYNC_TILED: WgslMaterialSpec = {
  name: "prepared",
  displayName: "PREPARED",
  vertexWgsl: "",
  fragmentWgsl: "",
  uniforms: [],
  samplers: [],
  textures: [{ name: "tiles", binding: 0 }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** Every pipeline a frame's passes set, in the order the queue took them. */
function pipelinesSet(gpu: FakeDevice): unknown[] {
  return gpu.queue.submitted.flatMap((buffer) =>
    buffer instanceof FakeCommandBuffer
      ? buffer.commands.filter(({ op }) => op === "setPipeline").map(({ args }) => args[0])
      : [],
  );
}

describe("prepareMaterial (R10.T14.d)", () => {
  beforeEach(() => {
    vi.stubGlobal("navigator", { gpu: new FakeGpu([]) });
  });

  it("makes a synchronous material's pipelines asynchronously, its first draw making none", async () => {
    const { engine, gpu } = await engineOn();
    const asked: string[] = [];
    const make = gpu.createRenderPipelineAsync.bind(gpu);
    gpu.createRenderPipelineAsync = (descriptor) => {
      asked.push(descriptor.label ?? "");
      return make(descriptor);
    };
    const mesh = indexedTriangle(engine);
    const material = engine.createMaterial(SYNC_TILED);
    await engine.prepareMaterial(material, ["rgba16float"], [mesh]);
    const made = gpu.renderPipelines.length;
    drawEach(outTarget(engine), mesh, [material], engine.createTexture(sampled("tiles", 1)));
    expect([asked, gpu.renderPipelines.length - made]).toEqual([["prepared", "prepared"], 0]);
  });

  it("lets a draw before the pipelines are made make its own, and keeps it once they are", async () => {
    const { engine, gpu } = await engineOn();
    const landings: Array<() => void> = [];
    gpu.createRenderPipelineAsync = (descriptor) =>
      new Promise((resolve) => {
        landings.push(() => {
          resolve(gpu.createRenderPipeline(descriptor));
        });
      });
    const mesh = indexedTriangle(engine);
    const material = engine.createMaterial(SYNC_TILED);
    const tiles = engine.createTexture(sampled("tiles", 1));
    const target = outTarget(engine);
    // The draw's own output, its preparation asked for once the compile check has answered.
    const prepared = engine.prepareMaterial(
      material,
      [{ colour: "rgba16float", depth: false }],
      [mesh],
    );
    await vi.waitFor(() => {
      expect(landings).toHaveLength(1);
    });
    drawEach(target, mesh, [material], tiles);
    for (const land of landings) {
      land();
    }
    await prepared;
    drawEach(target, mesh, [material], tiles);
    const [first, second] = pipelinesSet(gpu);
    expect([first !== undefined, second]).toEqual([true, first]);
  });

  it("leaves none of a prepared material's draws out, before or after its pipelines are made", async () => {
    const { engine, gpu } = await engineOn();
    const landings: Array<() => void> = [];
    gpu.createRenderPipelineAsync = (descriptor) =>
      new Promise((resolve) => {
        landings.push(() => {
          resolve(gpu.createRenderPipeline(descriptor));
        });
      });
    const mesh = indexedTriangle(engine);
    const material = engine.createMaterial(SYNC_TILED);
    const tiles = engine.createTexture(sampled("tiles", 1));
    const target = outTarget(engine);
    const prepared = engine.prepareMaterial(material, ["rgba16float"], [mesh]);
    drawEach(target, mesh, [material], tiles);
    // The preparation asks once the material's compile check has answered, for the one pipeline
    // the draw did not make: the target's with depth.
    await vi.waitFor(() => {
      expect(landings).toHaveLength(1);
    });
    for (const land of landings) {
      land();
    }
    await prepared;
    drawEach(target, mesh, [material], tiles);
    const draws = gpu.queue.submitted.flatMap((buffer) =>
      buffer instanceof FakeCommandBuffer
        ? buffer.commands.filter(({ op }) => op === "drawIndexed")
        : [],
    );
    expect(draws).toHaveLength(2);
  });

  it("prepares only the outputs asked for, a colour format with depth alone", async () => {
    const { engine, gpu } = await engineOn();
    const depths: boolean[] = [];
    const make = gpu.createRenderPipelineAsync.bind(gpu);
    gpu.createRenderPipelineAsync = (descriptor) => {
      depths.push(descriptor.depthStencil !== undefined);
      return make(descriptor);
    };
    const material = engine.createMaterial(SYNC_TILED);
    await engine.prepareMaterial(
      material,
      [{ colour: "rgba16float", depth: true }],
      [indexedTriangle(engine)],
    );
    expect(depths).toEqual([true]);
  });

  it("prepares nothing of a material whose shaders failed, which its check reports once", async () => {
    const { engine, gpu } = await engineOn();
    gpu.shaderErrors = (code) => (code.includes("oops") ? ["unresolved value 'oops'"] : []);
    const asked: string[] = [];
    const make = gpu.createRenderPipelineAsync.bind(gpu);
    gpu.createRenderPipelineAsync = (descriptor) => {
      asked.push(descriptor.label ?? "");
      return make(descriptor);
    };
    const broken = engine.createMaterial({ ...SYNC_TILED, name: "broken", vertexWgsl: "oops" });
    await engine.prepareMaterial(broken, ["rgba16float"], [indexedTriangle(engine)]);
    expect(asked).toEqual([]);
  });

  it("rejects for a released material", async () => {
    const { engine } = await engineOn();
    const mesh = indexedTriangle(engine);
    const material = engine.createMaterial(SYNC_TILED);
    engine.releaseMaterial(material);
    await expect(engine.prepareMaterial(material, ["rgba16float"], [mesh])).rejects.toThrow(
      /material prepared was released/u,
    );
  });
});

describe("a material's pipelines by its mesh's layout (R10.T14.d)", () => {
  beforeEach(() => {
    vi.stubGlobal("navigator", { gpu: new FakeGpu([]) });
  });

  /** A material compiled for a mesh of the triangle's layout, that mesh released after. */
  async function compiledOnAnother(engine: WebGpuRenderEngine): Promise<MaterialHandle> {
    const compiledFor = indexedTriangle(engine);
    const material = await tiledMaterial(engine, "shared", [compiledFor]);
    engine.releaseMesh(compiledFor);
    return material;
  }

  it("draw any mesh of the layout they were compiled for, making no pipeline", async () => {
    const { engine, gpu } = await engineOn();
    const material = await compiledOnAnother(engine);
    const tiles = engine.createTexture(sampled("tiles", 1));
    const made = gpu.renderPipelines.length;
    drawEach(outTarget(engine), indexedTriangle(engine), [material], tiles);
    expect(gpu.renderPipelines).toHaveLength(made);
  });

  it("make one for a mesh of another layout", async () => {
    const { engine, gpu } = await engineOn();
    const material = await compiledOnAnother(engine);
    const tiles = engine.createTexture(sampled("tiles", 1));
    const made = gpu.renderPipelines.length;
    const coloured = engine.createMesh({
      name: "coloured",
      positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
      indices: null,
      topology: "triangle-list",
      attributes: { colour: { data: new Float32Array(9), size: 3 } },
    });
    drawEach(outTarget(engine), coloured, [material], tiles);
    expect(gpu.renderPipelines).toHaveLength(made + 1);
  });
});

/** A depth-only caster: no colour, depth written, both sides (R10.T9.b). */
const CASTER: WgslMaterialSpec = {
  name: "caster",
  displayName: "CASTER",
  vertexWgsl: "",
  fragmentWgsl: "",
  uniforms: [],
  samplers: [],
  cullMode: "none",
  depthWrite: true,
  colourWrites: false,
  blend: "none",
};

/** A 64² set of four layers under `shadows`. */
const LAYERS = { name: "cascades", sizePx: 64, layers: 4, category: "shadows" } as const;

const IDENTITY_4 = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

/** A pass of `draws` into `layer`, its projection's depth translation `depth` (clip z there). */
function layerPass(layer: number, draws: ReadonlyArray<DrawItem>, depth = 0.5): DepthLayerPass {
  const projection = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, depth, 1]);
  return { label: `view:shadow ${layer}`, layer, viewRotation: IDENTITY_4, projection, draws };
}

/** A draw of `mesh` with `material`, binding `textures`. */
function drawItem(
  mesh: MeshHandle,
  material: MaterialHandle,
  textures: Readonly<Record<string, TextureHandle>> = {},
): DrawItem {
  return { mesh, material, offsetFromCameraM: new Float32Array(3), uniforms: {}, textures };
}

/** The command buffers the queue took whose encoder is labelled `label`. */
function submittedAs(gpu: FakeDevice, label: string): FakeCommandBuffer[] {
  return gpu.queue.submitted.filter(
    (buffer): buffer is FakeCommandBuffer =>
      buffer instanceof FakeCommandBuffer && buffer.label === label,
  );
}

/** Whether a recorded command's argument is a render pass's descriptor. */
function isRenderPassDescriptor(value: unknown): value is GPURenderPassDescriptor {
  return typeof value === "object" && value !== null && "colorAttachments" in value;
}

/** The descriptor of each render pass a command buffer began. */
function renderPasses(buffer: FakeCommandBuffer): GPURenderPassDescriptor[] {
  return buffer.commands
    .filter(({ op }) => op === "beginRenderPass")
    .map(({ args }) => args[0])
    .filter(isRenderPassDescriptor);
}

/** The texture, dimension and layers of a depth attachment's view. */
function layerOf(
  attachment: GPURenderPassDepthStencilAttachment | undefined,
): ReadonlyArray<unknown> | null {
  const view = attachment?.view;
  if (!(view instanceof FakeTextureView)) {
    return null;
  }
  const { dimension, baseArrayLayer, arrayLayerCount } = view.descriptor ?? {};
  return [view.texture.label, dimension, baseArrayLayer, arrayLayerCount];
}

describe("depth-layer targets (R10.T9.b)", () => {
  beforeEach(() => {
    vi.stubGlobal("navigator", { gpu: new FakeGpu([]) });
  });

  it("tally the layers' bytes under their category, and release them at disposal", async () => {
    const { engine, gpu } = await engineOn();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    const target = engine.createDepthLayers(LAYERS);
    const texture = gpu.textures.find((t) => t.label === "cascades");
    expect(texture?.descriptor).toMatchObject({
      size: { width: 64, height: 64, depthOrArrayLayers: 4 },
      dimension: "2d",
      format: "depth32float",
      mipLevelCount: 1,
      usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING,
    });
    target.dispose();
    expect(events).toEqual([
      { kind: "created", name: "cascades", bytes: 4 * 64 * 64 * 4, category: "shadows" },
      { kind: "destroyed", name: "cascades", bytes: 4 * 64 * 64 * 4, category: "shadows" },
    ]);
    expect(texture?.destroyed).toBe(true);
  });

  it("encode each pass in a submission of its own, onto its layer, cleared to 0 and stored", async () => {
    const { engine, gpu } = await engineOn();
    const mesh = indexedTriangle(engine);
    const caster = engine.createMaterial(CASTER);
    const target = engine.createDepthLayers(LAYERS);
    target.render("view", [layerPass(2, [drawItem(mesh, caster)]), layerPass(0, [])]);
    const buffers = [
      ...submittedAs(gpu, "view view:shadow 2"),
      ...submittedAs(gpu, "view view:shadow 0"),
    ];
    const passes = buffers.flatMap(renderPasses);
    const depths = passes.map(({ depthStencilAttachment }) => depthStencilAttachment);
    expect({
      submissions: buffers.length,
      labels: passes.map(({ label }) => label),
      colour: passes.map(({ colorAttachments }) => Array.from(colorAttachments)),
      ops: depths.map((d) => [d?.depthLoadOp, d?.depthStoreOp, d?.depthClearValue]),
      layers: depths.map(layerOf),
      draws: buffers.map((b) => b.commands.filter(({ op }) => op === "drawIndexed").length),
    }).toEqual({
      submissions: 2,
      labels: ["view:shadow 2", "view:shadow 0"],
      colour: [[], []],
      ops: [
        ["clear", "store", 0],
        ["clear", "store", 0],
      ],
      layers: [
        ["cascades", "2d", 2, 1],
        ["cascades", "2d", 0, 1],
      ],
      draws: [1, 0],
    });
  });

  it("write each pass's own Frame before its submission: its matrices and the layers' size", async () => {
    const { engine, gpu } = await engineOn();
    const writes = vi.spyOn(gpu.queue, "writeBuffer");
    const target = engine.createDepthLayers(LAYERS);
    target.render("view", [layerPass(1, [], 0.25), layerPass(3, [], 0.75)]);
    const frames = writes.mock.calls
      .filter(([buffer]) => buffer.label === "frame uniforms")
      .map(([, , data, offset = 0, size = 0]) =>
        data instanceof ArrayBuffer
          ? new Float32Array(data, offset, size / 4)
          : new Float32Array(0),
      );
    // The projection's depth translation, then the viewport's width and height.
    expect(frames.map((frame) => [frame[16 + 14], frame[32], frame[33]])).toEqual([
      [0.25, 64, 64],
      [0.75, 64, 64],
    ]);
  });

  it("draw through each material's depth-only pipeline: no colour target, the material's depth state", async () => {
    const { engine, gpu } = await engineOn();
    const mesh = indexedTriangle(engine);
    const caster = await engine.createMaterialAsync(CASTER, ["depth-only"], [mesh]);
    const made = gpu.renderPipelines.filter((p) => p.label === "caster");
    engine.createDepthLayers(LAYERS).render("view", [layerPass(0, [drawItem(mesh, caster)])]);
    expect({
      made: made.map((p) => [
        Array.from(p.descriptor.fragment?.targets ?? []),
        p.descriptor.depthStencil?.depthCompare,
        p.descriptor.depthStencil?.depthWriteEnabled,
      ]),
      afterDraw: gpu.renderPipelines.filter((p) => p.label === "caster").length,
      drawnWith: pipelinesSet(gpu).includes(made[0]),
    }).toEqual({ made: [[[], "greater-equal", true]], afterDraw: 1, drawnWith: true });
  });

  it("make a material's pipelines for depth-only ahead, with depth and no colour", () => {
    expect(pipelineOutputs("depth-only", "bgra8unorm")).toEqual([["depth-only", true]]);
  });

  it("refuse a layer outside the target, or named twice, submitting nothing", async () => {
    const { engine, gpu } = await engineOn();
    const target = engine.createDepthLayers(LAYERS);
    const before = gpu.queue.submitted.length;
    expect(() => {
      target.render("view", [layerPass(0, []), layerPass(4, [])]);
    }).toThrow(/layer 4 of cascades, which has layers 0 to 3/u);
    expect(() => {
      target.render("view", [layerPass(1, []), layerPass(1, [])]);
    }).toThrow(/layer 1 of cascades again/u);
    expect(gpu.queue.submitted).toHaveLength(before);
  });

  it("refuse a draw that samples the layers it writes, submitting nothing", async () => {
    const { engine, gpu } = await engineOn();
    const mesh = indexedTriangle(engine);
    const caster = engine.createMaterial(CASTER);
    const target = engine.createDepthLayers(LAYERS);
    const before = gpu.queue.submitted.length;
    expect(() => {
      target.render("view", [layerPass(0, [drawItem(mesh, caster, { cascades: target.depth })])]);
    }).toThrow(DepthSelfSample);
    expect(gpu.queue.submitted).toHaveLength(before);
  });

  it("refuse the release of the layers' texture, which the target releases", async () => {
    const { engine } = await engineOn();
    const target = engine.createDepthLayers(LAYERS);
    expect(() => {
      engine.releaseTexture(target.depth);
    }).toThrow(/belongs to a depth-layer target/u);
  });

  it("refuse layers that do not fit the device, naming the target", async () => {
    const { engine } = await engineOn();
    expect(() => engine.createDepthLayers({ ...LAYERS, sizePx: 8193 })).toThrow(
      /depth layers cascades of 8193 px a side/u,
    );
  });

  it("draw nothing once disposed, or once their engine is", async () => {
    const { engine, gpu } = await engineOn();
    const disposed = engine.createDepthLayers(LAYERS);
    disposed.dispose();
    const kept = engine.createDepthLayers({ ...LAYERS, name: "kept" });
    engine.dispose();
    const before = gpu.queue.submitted.length;
    disposed.render("view", [layerPass(0, [])]);
    kept.render("view", [layerPass(0, [])]);
    expect({
      submitted: gpu.queue.submitted.length - before,
      keptDestroyed: gpu.textures.find((t) => t.label === "kept")?.destroyed,
    }).toEqual({ submitted: 0, keptDestroyed: true });
  });

  it("bind the layers where a material declares a depth 2d-array, through a comparison sampler", async () => {
    const { engine, gpu } = await engineOn();
    const mesh = indexedTriangle(engine);
    const receiver = engine.createMaterial({
      ...SYNC_TILED,
      name: "receiver",
      samplers: [
        {
          name: "cascadesSampler",
          filter: "linear",
          address: "clamp-to-edge",
          binding: 1,
          compare: "greater-equal",
        },
      ],
      textures: [{ name: "cascades", binding: 0, sampleType: "depth", viewDimension: "2d-array" }],
    });
    const target = engine.createDepthLayers(LAYERS);
    outTarget(engine).render({
      label: "receiver",
      viewRotation: new Float32Array(16),
      projection: new Float32Array(16),
      draws: [drawItem(mesh, receiver, { cascades: target.depth })],
      postProcesses: [],
    });
    expect({
      views: gpu.textures.find((t) => t.label === "cascades")?.views,
      layout: gpu.bindGroupLayouts.find((l) => l.label === "material receiver")?.descriptor.entries,
      samplers: gpu.samplers.map(({ descriptor }) => descriptor.compare),
    }).toEqual({
      views: [
        { label: "cascades layer 0", dimension: "2d", baseArrayLayer: 0, arrayLayerCount: 1 },
        { label: "cascades layer 1", dimension: "2d", baseArrayLayer: 1, arrayLayerCount: 1 },
        { label: "cascades layer 2", dimension: "2d", baseArrayLayer: 2, arrayLayerCount: 1 },
        { label: "cascades layer 3", dimension: "2d", baseArrayLayer: 3, arrayLayerCount: 1 },
        { dimension: "2d-array", aspect: "depth-only" },
      ],
      layout: [
        { binding: 0, visibility: 3, texture: { sampleType: "depth", viewDimension: "2d-array" } },
        { binding: 1, visibility: 3, sampler: { type: "comparison" } },
      ],
      samplers: ["greater-equal"],
    });
  });

  it("take no resolve of their own: their passes ride the next render's", async () => {
    const { engine } = await engineOn(["timestamp-query"]);
    const times = vi.fn<(times: PassTimes) => void>();
    engine.onPassTimes(times);
    engine.createDepthLayers(LAYERS).render("view", [layerPass(0, []), layerPass(1, [])]);
    const afterShadows = engine.passTimesFrame;
    renderEmptyFrame(engine, "view:bodies");
    await vi.waitFor(() => {
      expect(times).toHaveBeenCalledOnce();
    });
    expect({
      frames: [afterShadows, engine.passTimesFrame],
      labels: times.mock.calls[0]?.[0].passes.map(({ label }) => label),
    }).toEqual({ frames: [0, 1], labels: ["view:shadow 0", "view:shadow 1", "view:bodies"] });
  });
});

describe("unclippedDepth (R10.T9.b)", () => {
  beforeEach(() => {
    vi.stubGlobal("navigator", { gpu: new FakeGpu([]) });
  });

  const UNCLIPPED: WgslMaterialSpec = { ...CASTER, name: "unclipped", unclippedDepth: true };

  it("is refused without depth-clip-control, by createMaterial and createMaterialAsync alike", async () => {
    const { engine, gpu } = await engineOn();
    expect(() => engine.createMaterial(UNCLIPPED)).toThrow(
      /material unclipped asks for unclippedDepth, which needs depth-clip-control/u,
    );
    await expect(engine.createMaterialAsync(UNCLIPPED, ["depth-only"])).rejects.toThrow(
      /material unclipped .* depth-clip-control/u,
    );
    expect(gpu.shaderModules.filter((m) => m.label === "unclipped")).toEqual([]);
  });

  it("takes primitive.unclippedDepth with the feature", async () => {
    const { engine, gpu } = await engineOn(["depth-clip-control"]);
    await engine.createMaterialAsync(UNCLIPPED, ["depth-only"], [indexedTriangle(engine)]);
    expect(
      gpu.renderPipelines
        .filter((p) => p.label === "unclipped")
        .map((p) => p.descriptor.primitive?.unclippedDepth),
    ).toEqual([true]);
  });
});

describe("the sampler cache (R10.T9.b)", () => {
  it("keys a sampler by its filter, its address mode and its comparison", () => {
    const plain = { filter: "linear", address: "clamp-to-edge" } as const;
    expect([samplerKey(plain), samplerKey({ ...plain, compare: "greater-equal" })]).toEqual([
      "linear:clamp-to-edge:none",
      "linear:clamp-to-edge:greater-equal",
    ]);
  });
});
