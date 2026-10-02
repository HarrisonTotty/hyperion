/**
 * What the engine encodes, read from a recording `FakeDevice`: the fixed bind-group layouts, a
 * frame's draws and post-processes, mip generation, compile errors and the device request.
 */

import { describe, expect, it, vi } from "vitest";

import {
  FakeAdapter,
  type FakeCommand,
  type FakeDevice,
  FakeGpu,
  FakeTexture,
  FakeTextureView,
  INTEL_UHD_620_INFO,
} from "../../../test/fakeGpu";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import { requestAdapterOutcome } from "../platform";
import { GraphicsStatusStore, initialGraphicsStatus } from "../status";
import type { DrawItem, FrameSubmission, MaterialHandle, MeshHandle } from "../types";
import { createWebGpuEngine, WebGpuRenderEngine } from "./engine";
import { MipGenerator } from "./mipmaps";

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

/**
 * A material whose sources declare no `Draw` struct, so any uniforms pass the check, shown on the
 * console as `TEST <name>`.
 */
function material(engine: WebGpuRenderEngine, name: string, code = "flat"): MaterialHandle {
  return engine.createMaterial({
    name,
    displayName: `TEST ${name.toUpperCase()}`,
    vertexWgsl: code,
    fragmentWgsl: code,
    uniforms: [{ name: "tint", type: "vec4f" }],
    samplers: [],
    cullMode: "none",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
  });
}

function triangle(engine: WebGpuRenderEngine): MeshHandle {
  return engine.createMesh({
    name: "triangle",
    positions: new Float32Array(9),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  });
}

function draw(mesh: MeshHandle, handle: MaterialHandle, extra: Partial<DrawItem> = {}): DrawItem {
  return {
    mesh,
    material: handle,
    offsetFromCameraM: new Float32Array(3),
    uniforms: { tint: new Float32Array([1, 0, 0, 1]) },
    textures: {},
    ...extra,
  };
}

function frame(
  label: string,
  draws: ReadonlyArray<DrawItem>,
  postProcesses: FrameSubmission["postProcesses"] = [],
): FrameSubmission {
  return {
    label,
    viewRotation: new Float32Array(16),
    projection: new Float32Array(16),
    draws,
    postProcesses,
  };
}

/** The commands of the frame encoded under `label`. */
function commandsOf(gpu: FakeDevice, label: string): ReadonlyArray<FakeCommand> {
  const encoder = gpu.encoders.find((candidate) => candidate.label === label);
  if (encoder === undefined) {
    throw new Error(`no encoder ${label}`);
  }
  return encoder.commands;
}

/** The texture a pass's first colour attachment, or a bound view, is a view of. */
function textureLabelOf(view: unknown): string {
  return view instanceof FakeTextureView ? view.texture.label : "not a fake view";
}

/** The colour attachments' textures of each render pass in `commands`, in order. */
function attachments(commands: ReadonlyArray<FakeCommand>): string[] {
  return commands
    .filter(({ op }) => op === "beginRenderPass")
    .map(({ args: [descriptor] }) => {
      const colour: unknown = Reflect.get(Object(descriptor), "colorAttachments");
      const first: unknown = Array.isArray(colour) ? colour[0] : undefined;
      return textureLabelOf(Reflect.get(Object(first), "view"));
    });
}

describe("the fixed bind-group layouts", () => {
  it("are the frame's uniform at group 0 and the draw's at a dynamic offset at group 1", async () => {
    const { engine, gpu } = await engineOn();
    material(engine, "flat");
    expect(
      gpu.bindGroupLayouts.slice(0, 2).map(({ label, descriptor }) => [label, descriptor]),
    ).toEqual([
      [
        "frame",
        { label: "frame", entries: [{ binding: 0, visibility: 3, buffer: { type: "uniform" } }] },
      ],
      [
        "draw",
        {
          label: "draw",
          entries: [
            { binding: 0, visibility: 3, buffer: { type: "uniform", hasDynamicOffset: true } },
          ],
        },
      ],
    ]);
    const [frameLayout, drawLayout] = gpu.bindGroupLayouts;
    const layout = gpu.pipelineLayouts.at(-1)?.descriptor;
    expect(layout?.label).toBe("flat");
    expect([...(layout?.bindGroupLayouts ?? [])].slice(0, 2)).toEqual([frameLayout, drawLayout]);
    expect(layout?.bindGroupLayouts).toHaveLength(3);
  });
});

describe("a frame's encoding", () => {
  it("draws in submission order, indirect draws in place, each at its own dynamic offset", async () => {
    const { engine, gpu } = await engineOn();
    const mesh = triangle(engine);
    const flat = material(engine, "flat");
    const args = engine.createBuffer({
      name: "args",
      bytes: 16,
      usage: BUFFER_USAGE.INDIRECT | BUFFER_USAGE.STORAGE,
      category: "other",
    });
    const target = engine.createRenderTarget({
      name: "scene",
      size: { widthPx: 8, heightPx: 8 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    target.render(
      frame("ordered", [
        draw(mesh, flat, { instanceCount: 2 }),
        draw(mesh, flat, { indirect: { buffer: args, offsetBytes: 0 } }),
        draw(mesh, flat),
      ]),
    );
    const commands = commandsOf(gpu, "ordered");
    expect(
      commands
        .filter(({ op }) => op.startsWith("draw"))
        .map(({ op, args: drawArgs }) => ({
          op,
          numbers: drawArgs.filter((a) => typeof a === "number"),
        })),
    ).toEqual([
      { op: "draw", numbers: [3, 2] },
      { op: "drawIndirect", numbers: [0] },
      { op: "draw", numbers: [3, 1] },
    ]);
    const groups = commands.filter(({ op }) => op === "setBindGroup");
    expect(groups.filter(({ args: [index] }) => index === 0)).toHaveLength(1);
    expect(
      groups.filter(({ args: [index] }) => index === 1).map(({ args: [, , offsets] }) => offsets),
    ).toEqual([[0], [256], [512]]);
    expect(commands.at(0)?.op).toBe("beginRenderPass");
    expect(commands.at(-1)?.op).toBe("end");
  });

  it("chains post-processes through the intermediates, the last writing the output", async () => {
    const { engine, gpu } = await engineOn();
    const mesh = triangle(engine);
    const flat = material(engine, "flat");
    const first = engine.createPostProcess({
      name: "first",
      displayName: "TEST FIRST",
      fragmentWgsl: "a",
      uniforms: [],
    });
    const second = engine.createPostProcess({
      name: "second",
      displayName: "TEST SECOND",
      fragmentWgsl: "b",
      uniforms: [],
    });
    const target = engine.createRenderTarget({
      name: "scene",
      size: { widthPx: 8, heightPx: 8 },
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    });
    target.render(
      frame(
        "chained",
        [draw(mesh, flat)],
        [
          { postProcess: first, uniforms: {} },
          { postProcess: second, uniforms: {} },
        ],
      ),
    );
    const commands = commandsOf(gpu, "chained");
    expect(attachments(commands)).toEqual([
      "scene:post-process 0",
      "scene:post-process 1",
      "scene:colour",
    ]);
    // Each post-process samples, as hdr-colour, the intermediate the pass before it wrote.
    const sampled = commands
      .filter(({ op, args: [index] }) => op === "setBindGroup" && index === 2)
      .slice(1)
      .map(({ args: [, group] }) => {
        const descriptor = gpu.bindGroups.find((made) => made === group)?.descriptor;
        const entry = [...(descriptor?.entries ?? [])].find(({ binding }) => binding === 0);
        return textureLabelOf(entry?.resource);
      });
    expect(sampled).toEqual(["scene:post-process 0", "scene:post-process 1"]);
  });

  it("leaves out the draws of a material whose shaders failed to compile, and reports it", async () => {
    const { engine, gpu, status } = await engineOn();
    gpu.shaderErrors = (code) => (code.includes("oops") ? ["unresolved value 'oops'"] : []);
    const logged = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const mesh = triangle(engine);
    const broken = material(engine, "broken", "return oops;");
    await vi.waitFor(() => {
      expect(status.getSnapshot().fault).toEqual({
        kind: "shader-refused",
        effectName: "TEST BROKEN",
      });
    });
    const target = engine.createRenderTarget({
      name: "scene",
      size: { widthPx: 8, heightPx: 8 },
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    });
    target.render(frame("refused", [draw(mesh, broken), draw(mesh, material(engine, "flat"))]));
    expect(commandsOf(gpu, "refused").filter(({ op }) => op === "draw")).toHaveLength(1);
    // The screen has the display name; the log keeps the code name beside it.
    expect(logged).toHaveBeenCalledWith(
      "material broken (TEST BROKEN) failed to compile; its draws are left out:\n1:1 unresolved value 'oops'",
    );
  });
});

describe("mip generation", () => {
  it("draws each level from the one above, timing the chain as one pass", async () => {
    const { gpu } = await engineOn();
    const texture = new FakeTexture({
      label: "bloom",
      size: [8, 8],
      format: "rgba16float",
      mipLevelCount: 3,
      usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING,
    });
    const encoder = gpu.createCommandEncoder({ label: "mips" });
    const querySet = gpu.createQuerySet({ type: "timestamp", count: 2 });
    new MipGenerator(gpu).encode(encoder, texture, 3, {
      querySet,
      beginningOfPassWriteIndex: 0,
      endOfPassWriteIndex: 1,
    });
    const passes = commandsOf(gpu, "mips")
      .filter(({ op }) => op === "beginRenderPass")
      .map(({ args: [descriptor] }) => descriptor);
    expect(passes).toHaveLength(2);
    expect(passes.map((pass) => Reflect.get(Object(pass), "timestampWrites"))).toEqual([
      { querySet, beginningOfPassWriteIndex: 0 },
      { querySet, endOfPassWriteIndex: 1 },
    ]);
    // Level n is drawn into from level n - 1, one level a view.
    const levels = texture.views.map((view) => [view?.baseMipLevel, view?.mipLevelCount]);
    expect(levels).toEqual([
      [0, 1],
      [1, 1],
      [1, 1],
      [2, 1],
    ]);
    expect(gpu.renderPipelines.map(({ descriptor }) => descriptor.layout)).toEqual(["auto"]);
  });
});

describe("the engine's creation", () => {
  it("requests the wanted features the adapter has, less those withheld", async () => {
    const adapter = new FakeAdapter({
      info: INTEL_UHD_620_INFO,
      features: ["subgroups", "timestamp-query", "float32-blendable"],
    });
    const outcome = await requestAdapterOutcome(new FakeGpu([adapter]));
    if (outcome.kind !== "adapter") {
      throw new Error("the fake gave no adapter");
    }
    const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const engine = await createWebGpuEngine(outcome, status, {
      withholdSubgroups: true,
      withholdShaderF16: false,
    });
    expect(adapter.devices.at(0)?.requiredFeatures).toEqual([
      "timestamp-query",
      "float32-blendable",
    ]);
    expect(engine.capabilities.subgroups).toBe(false);
    expect(status.getSnapshot().capabilities).toEqual(engine.capabilities);
    engine.dispose();
  });
});
