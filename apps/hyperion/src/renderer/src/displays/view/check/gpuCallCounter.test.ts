import { describe, expect, it } from "vitest";

import type { InspectionAttachmentSlot, InspectionGpuAttachment } from "../../../../../preload/api";
import { FakeAdapter, FakeGpu, FakeTexture, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import type { CanvasContexts } from "../../../view/spike/capture";
import { wrapGpu } from "../../../view/spike/pipelineShim";
import {
  AFTER_LAST_PASS,
  bufferWriteBytes,
  GpuCallCounter,
  stagedUploadBytes,
} from "./gpuCallCounter";

/**
 * A counted device on a fake adapter, through the seam the views probe's engine source uses, its
 * canvases' textures read from `contexts`.
 */
async function countedDevice(contexts: CanvasContexts | null = null): Promise<{
  readonly counter: GpuCallCounter;
  readonly device: GPUDevice;
}> {
  const counter = new GpuCallCounter(contexts);
  const gpu = wrapGpu(
    new FakeGpu([new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] })]),
    counter.wrapDevice,
  );
  const adapter = await gpu.requestAdapter();
  if (adapter === null) {
    throw new Error("the fake gave no adapter");
  }
  return { counter, device: await adapter.requestDevice() };
}

/** The counts taken, as `label: call × count (bytes)` lines. */
function lines(counter: GpuCallCounter): string[] {
  return counter
    .take()
    .calls.map(
      ({ label, call, count, bytes }) =>
        `${label}: ${call} × ${String(count)}${bytes > 0 ? ` (${String(bytes)} B)` : ""}`,
    );
}

/** A kind of attachment two passes of `bodies` at 640 × 360 px took. */
function bodiesTook(
  attachment: InspectionAttachmentSlot,
  format: GPUTextureFormat,
  loadOp: InspectionGpuAttachment["loadOp"],
  storeOp: InspectionGpuAttachment["storeOp"],
  bytesLoaded: number,
  bytesStored: number,
): InspectionGpuAttachment {
  return {
    label: "bodies",
    attachment,
    view: { format, widthPx: 640, heightPx: 360, sampleCount: 1 },
    loadOp,
    storeOp,
    passes: 2,
    bytesLoaded,
    bytesStored,
  };
}

describe("the frame profile's count of WebGPU calls (plan R14)", () => {
  it("counts a pass's calls under its label, and the writes before it as the pass's", async () => {
    const { counter, device } = await countedDevice();
    const buffer = device.createBuffer({ label: "uniforms", size: 64, usage: 0 });
    counter.begin();
    device.queue.writeBuffer(buffer, 0, new Float32Array(16));
    const encoder = device.createCommandEncoder({ label: "frame" });
    const pass = encoder.beginRenderPass({ label: "bodies", colorAttachments: [] });
    const group = device.createBindGroup({
      layout: device.createBindGroupLayout({ entries: [] }),
      entries: [],
    });
    pass.setBindGroup(0, group);
    pass.setBindGroup(2, group);
    pass.draw(3);
    pass.end();
    device.queue.submit([encoder.finish()]);
    device.queue.writeBuffer(buffer, 0, new Uint8Array(8), 2, 4);
    expect(lines(counter)).toEqual([
      "frame: device.createCommandEncoder × 1",
      "frame: encoder.finish × 1",
      "frame: queue.submit × 1",
      "bodies: queue.writeBuffer × 1 (64 B)",
      "bodies: encoder.beginRenderPass × 1",
      "bodies: device.createBindGroupLayout × 1",
      "bodies: device.createBindGroup × 1",
      "bodies: render pass.setBindGroup × 2",
      "bodies: render pass.draw × 1",
      "bodies: render pass.end × 1",
      `${AFTER_LAST_PASS}: queue.writeBuffer × 1 (4 B)`,
    ]);
  });

  it("counts an unlabelled pass under its encoder, and what the device made as it is used", async () => {
    const { counter, device } = await countedDevice();
    const texture = device.createTexture({ size: [4, 4], format: "rgba8unorm", usage: 0 });
    counter.begin();
    texture.createView();
    const encoder = device.createCommandEncoder({ label: "atmosphere view" });
    const pass = encoder.beginComputePass();
    pass.dispatchWorkgroups(8, 8);
    pass.end();
    expect(lines(counter)).toEqual([
      "atmosphere view: device.createCommandEncoder × 1",
      "atmosphere view: texture.createView × 1",
      "atmosphere view: encoder.beginComputePass × 1",
      "atmosphere view: compute pass.dispatchWorkgroups × 1",
      "atmosphere view: compute pass.end × 1",
    ]);
  });

  it("forgets what it counted at each window's start and after each take", async () => {
    const { counter, device } = await countedDevice();
    device.createCommandEncoder({ label: "before" });
    counter.begin();
    expect(counter.take()).toEqual({ calls: [], attachments: [] });
    device.createCommandEncoder({ label: "after" });
    expect(lines(counter)).toEqual(["after: device.createCommandEncoder × 1"]);
    expect(counter.take()).toEqual({ calls: [], attachments: [] });
  });

  it("counts a pass's attachments and the bytes each load and store moves, a clear and a discard none", async () => {
    const { counter, device } = await countedDevice();
    const texture = (label: string, format: GPUTextureFormat, size: [number, number]): GPUTexture =>
      device.createTexture({ label, size, format, usage: 0 });
    const hdr = texture("hdr", "rgba16float", [640, 360]).createView();
    const ids = texture("ids", "r32uint", [640, 360]).createView();
    // Its second mip, 640 × 360, through a view in its sRGB twin.
    const half = texture("history", "rgba8unorm", [1280, 720]).createView({
      baseMipLevel: 1,
      mipLevelCount: 1,
      format: "rgba8unorm-srgb",
    });
    const depth = texture("depth", "depth24plus-stencil8", [640, 360]).createView();
    counter.begin();
    const encoder = device.createCommandEncoder({ label: "frame" });
    for (let i = 0; i < 2; i += 1) {
      encoder
        .beginRenderPass({
          label: "bodies",
          colorAttachments: [
            { view: hdr, loadOp: "load", storeOp: "store" },
            { view: ids, loadOp: "clear", storeOp: "store" },
            null,
            { view: half, loadOp: "load", storeOp: "discard" },
          ],
          depthStencilAttachment: {
            view: depth,
            depthLoadOp: "load",
            depthStoreOp: "discard",
            stencilLoadOp: "clear",
            stencilStoreOp: "store",
          },
        })
        .end();
    }
    // Two passes of 640 × 360 texels: 8 bytes a texel of rgba16float, 4 of r32uint, rgba8unorm and
    // a depth24plus aspect, 1 of a stencil aspect.
    expect(counter.take().attachments).toEqual([
      bodiesTook("colour 0", "rgba16float", "load", "store", 2 * 1_843_200, 2 * 1_843_200),
      bodiesTook("colour 1", "r32uint", "clear", "store", 0, 2 * 921_600),
      bodiesTook("colour 3", "rgba8unorm-srgb", "load", "discard", 2 * 921_600, 0),
      bodiesTook("depth", "depth24plus-stencil8", "load", "discard", 2 * 921_600, 0),
      bodiesTook("stencil", "depth24plus-stencil8", "clear", "store", 0, 2 * 230_400),
    ]);
  });

  it("reads a canvas's view as its context hands out the texture, without counting the call", async () => {
    const canvas = new FakeTexture({ size: [800, 600], format: "bgra8unorm", usage: 0 });
    const contexts: CanvasContexts = { getCurrentTexture: () => canvas };
    const handOut: unknown = Reflect.get(contexts, "getCurrentTexture");
    const { counter, device } = await countedDevice(contexts);
    counter.begin();
    const encoder = device.createCommandEncoder({ label: "frame" });
    const view = contexts.getCurrentTexture().createView({ format: "bgra8unorm-srgb" });
    encoder
      .beginRenderPass({
        label: "tonemap",
        colorAttachments: [{ view, loadOp: "clear", storeOp: "store" }],
      })
      .end();
    const { calls, attachments } = counter.take();
    counter.release();
    expect([
      calls.map((each) => each.call),
      attachments.map(({ view: seen, bytesLoaded, bytesStored }) => ({
        seen,
        bytesLoaded,
        bytesStored,
      })),
      Reflect.get(contexts, "getCurrentTexture") === handOut,
    ]).toEqual([
      ["device.createCommandEncoder", "encoder.beginRenderPass", "render pass.end"],
      [
        {
          seen: { format: "bgra8unorm-srgb", widthPx: 800, heightPx: 600, sampleCount: 1 },
          bytesLoaded: 0,
          bytesStored: 800 * 600 * 4,
        },
      ],
      true,
    ]);
  });

  it("counts a resolve, a texture attached whole, read-only aspects and a view it never saw made", async () => {
    const { counter, device } = await countedDevice();
    const make = (format: GPUTextureFormat, sampleCount = 1): GPUTexture =>
      device.createTexture({ size: [64, 32], format, sampleCount, usage: 0 });
    const many = make("rgba8unorm", 4).createView();
    const resolved = make("rgba8unorm").createView();
    const whole = make("rgba16float");
    const depth = make("depth24plus-stencil8").createView();
    const unseen = new FakeTexture({
      size: [64, 32],
      format: "depth32float",
      usage: 0,
    }).createView();
    counter.begin();
    const encoder = device.createCommandEncoder({ label: "frame" });
    encoder
      .beginRenderPass({
        label: "lit",
        colorAttachments: [
          { view: many, resolveTarget: resolved, loadOp: "clear", storeOp: "discard" },
          { view: whole, loadOp: "load", storeOp: "store" },
        ],
        depthStencilAttachment: { view: depth, depthReadOnly: true, stencilReadOnly: true },
      })
      .end();
    encoder
      .beginRenderPass({
        label: "overlay",
        colorAttachments: [],
        depthStencilAttachment: { view: unseen, depthLoadOp: "clear", depthStoreOp: "store" },
      })
      .end();
    // 64 × 32 texels: 4 samples of 4 bytes, 4 bytes, 8 bytes, a depth aspect's 4 and a stencil's 1.
    expect(
      counter
        .take()
        .attachments.map((each) => [
          each.label,
          each.attachment,
          each.view?.sampleCount ?? null,
          each.loadOp,
          each.storeOp,
          each.bytesLoaded,
          each.bytesStored,
        ]),
    ).toEqual([
      ["lit", "colour 0", 4, "clear", "discard", 0, 0],
      ["lit", "resolve 0", 1, null, "resolve", 0, 8_192],
      ["lit", "colour 1", 1, "load", "store", 16_384, 16_384],
      ["lit", "depth", 1, "read-only", "read-only", 8_192, 0],
      ["lit", "stencil", 1, "read-only", "read-only", 2_048, 0],
      ["overlay", "depth", null, "clear", "store", 0, null],
    ]);
  });

  it("reads a write's bytes from its elements, its offset and its size", () => {
    expect([
      bufferWriteBytes(new Float32Array(16), undefined, undefined),
      bufferWriteBytes(new Float32Array(16), 4, undefined),
      bufferWriteBytes(new Float32Array(16), 4, 2),
      bufferWriteBytes(new ArrayBuffer(10), 2, undefined),
      bufferWriteBytes(new DataView(new ArrayBuffer(12)), undefined, undefined),
    ]).toEqual([64, 48, 8, 8, 12]);
  });

  it("counts a staging buffer mapped for writing at its creation as its size uploaded (lane perf-page-long-tasks)", () => {
    expect([
      stagedUploadBytes({ size: 4096, usage: 0x2 | 0x4, mappedAtCreation: true }),
      stagedUploadBytes({ size: 4096, usage: 0x4 | 0x8, mappedAtCreation: true }),
      stagedUploadBytes({ size: 4096, usage: 0x2 | 0x4 }),
      stagedUploadBytes(null),
    ]).toEqual([4096, 0, 0, 0]);
  });
});
