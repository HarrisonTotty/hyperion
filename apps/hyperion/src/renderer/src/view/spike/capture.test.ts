import { describe, expect, it } from "vitest";

import {
  FakeAdapter,
  FakeCommandBuffer,
  FakeDevice,
  FakeGpu,
  FakeTexture,
  INTEL_UHD_620_INFO,
} from "../../test/fakeGpu";
import {
  CAPTURE_SCHEMA,
  type CaptureSurface,
  DEVICE_ID,
  GpuCapture,
  parseCapture,
  QUEUE_ID,
  replayCapture,
} from "./capture";
import { PipelineTally, spikeDeviceWrapper, wrapGpu } from "./pipelineShim";

const UNIFORM = 0x40;
const COPY_DST = 0x8;
const COPY_SRC = 0x4;
const RENDER_ATTACHMENT = 0x10;

function adapter(): FakeAdapter {
  return new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
}

/** A device through the spike's seam, with the capture installed. */
async function capturedDevice(capture: GpuCapture | null): Promise<GPUDevice> {
  const tally = new PipelineTally(() => 0);
  const gpu = wrapGpu(new FakeGpu([adapter()]), spikeDeviceWrapper(tally, capture));
  const wrapped = await gpu.requestAdapter();
  if (wrapped === null) {
    throw new Error("the fake GPU gave no adapter");
  }
  return wrapped.requestDevice();
}

/** The scripted run: set-up, an upload before the span, the span's start, one frame. */
async function scriptedRun(capture: GpuCapture, device: GPUDevice): Promise<void> {
  const uniforms = device.createBuffer({ label: "uniforms", size: 16, usage: UNIFORM | COPY_DST });
  device.queue.writeBuffer(uniforms, 0, new Float32Array([9, 9, 9, 9]));
  const target = device.createTexture({
    label: "target",
    size: [64, 32],
    format: "rgba8unorm",
    usage: RENDER_ATTACHMENT,
  });
  const module = device.createShaderModule({ label: "triangle", code: "// wgsl" });
  const pipeline = await device.createRenderPipelineAsync({
    label: "triangle",
    layout: "auto",
    vertex: { module, entryPoint: "vertexMain" },
  });
  await capture.startSpan();
  capture.frame();
  device.queue.writeBuffer(uniforms, 4, new Float32Array([1, 2, 3, 4]), 1, 2);
  const encoder = device.createCommandEncoder({ label: "frame" });
  const pass = encoder.beginRenderPass({
    label: "terrain",
    colorAttachments: [{ view: target.createView(), loadOp: "clear", storeOp: "store" }],
  });
  pass.setPipeline(pipeline);
  pass.draw(3);
  pass.end();
  device.queue.submit([encoder.finish()]);
  capture.endSpan();
}

function noSurface(surface: CaptureSurface): never {
  throw new Error(`the capture names surface ${surface.id}`);
}

describe("the capture", () => {
  it("round-trips a scripted sequence of device calls to the same calls and bytes", async () => {
    const capture = new GpuCapture({ contexts: null, meta: { setting: "low", seed: "7" } });
    const device = await capturedDevice(capture);
    await scriptedRun(capture, device);
    const { file, data } = capture.result();
    const read = parseCapture(JSON.parse(JSON.stringify(file)), data.slice());
    expect(read.file.meta).toEqual({ setting: "low", seed: "7" });

    const replayed = new FakeDevice(INTEL_UHD_620_INFO, [], adapter().limits);
    replayCapture(read, replayed, noSurface);

    expect(replayed.buffers.map(({ label, size }) => [label, size])).toEqual([["uniforms", 16]]);
    expect(replayed.textures.map(({ label }) => label)).toEqual(["target"]);
    expect(replayed.renderPipelines.map(({ label }) => label)).toEqual(["triangle"]);
    // The snapshot's read-backs of the uniforms and the target (the fake maps zeros; rows of
    // 256 bytes), then the span's write of two floats from element 1: 8 bytes at offset 4.
    expect(replayed.queue.writes).toEqual([
      { kind: "buffer", label: "uniforms", offset: 0, bytes: 16 },
      { kind: "texture", label: "target", mipLevel: 0, bytesPerRow: 256, bytes: 256 * 32 },
      { kind: "buffer", label: "uniforms", offset: 4, bytes: 8 },
    ]);
    const [submitted] = replayed.queue.submitted;
    expect(submitted).toBeInstanceOf(FakeCommandBuffer);
    const commands = submitted instanceof FakeCommandBuffer ? submitted.commands : [];
    expect(commands.map(({ op }) => op)).toEqual(["beginRenderPass", "setPipeline", "draw", "end"]);
    expect(replayed.renderPipelines[0]?.descriptor.vertex.module).toBe(replayed.shaderModules[0]);
  });

  it("keeps the bytes the span wrote", async () => {
    const capture = new GpuCapture({ contexts: null, meta: {} });
    await scriptedRun(capture, await capturedDevice(capture));
    const { file, data } = capture.result();
    const write = file.calls.slice(file.spanStart).find(({ op }) => op === "writeBuffer");
    const blob = write?.args[2];
    const index =
      typeof blob === "object" && blob !== null && !Array.isArray(blob) ? blob["$blob"] : undefined;
    const span = typeof index === "number" ? file.blobs[index] : undefined;
    expect(span).toBeDefined();
    const bytes = data.slice(span?.offset ?? 0, (span?.offset ?? 0) + (span?.length ?? 0));
    expect([...new Float32Array(bytes.buffer)]).toEqual([2, 3]);
  });

  it("logs only creations before the span, the snapshot at its start, and every call after", async () => {
    const capture = new GpuCapture({ contexts: null, meta: {} });
    await scriptedRun(capture, await capturedDevice(capture));
    const { file } = capture.result();
    const before = file.calls.slice(0, file.spanStart);
    expect(before.map(({ target, op }) => [target, op])).toEqual([
      [DEVICE_ID, "createBuffer"],
      [DEVICE_ID, "createTexture"],
      [DEVICE_ID, "createShaderModule"],
      [DEVICE_ID, "createRenderPipeline"],
      [QUEUE_ID, "writeBuffer"],
      [QUEUE_ID, "writeTexture"],
    ]);
    expect(file.frames).toEqual([file.spanStart]);
    expect(file.schema).toBe(CAPTURE_SCHEMA);
    expect(file.skipped).toEqual([]);
  });

  it("asks for copy-source usage on what it must read back, and logs the usage as asked", async () => {
    const capture = new GpuCapture({ contexts: null, meta: {} });
    const device = await capturedDevice(capture);
    device.createBuffer({ label: "b", size: 8, usage: UNIFORM });
    const fake = device instanceof FakeDevice ? device : undefined;
    expect(fake?.buffers[0]?.usage).toBe(UNIFORM | COPY_SRC);
    const logged = capture.result().file.calls[0]?.args[0];
    expect(logged).toMatchObject({ usage: UNIFORM });
  });

  it("skips a multisampled texture in the snapshot", async () => {
    const capture = new GpuCapture({ contexts: null, meta: {} });
    const device = await capturedDevice(capture);
    device.createTexture({
      size: [4, 4],
      format: "rgba8unorm",
      sampleCount: 4,
      usage: RENDER_ATTACHMENT,
    });
    await capture.startSpan();
    expect(capture.result().file.skipped).toEqual([{ id: 2, reason: "multisampled" }]);
  });

  it("is absent unless the capture is given", async () => {
    const device = await capturedDevice(null);
    expect(Object.hasOwn(device, "createBuffer")).toBe(false);
    expect(Object.hasOwn(device.queue, "writeBuffer")).toBe(false);
    const captured = await capturedDevice(new GpuCapture({ contexts: null, meta: {} }));
    expect(Object.hasOwn(captured, "createBuffer")).toBe(true);
  });

  it("refuses a capture of another schema", () => {
    expect(() => parseCapture({ schema: "other", version: 1 }, new Uint8Array())).toThrow(
      "not a hyperion.gpu-capture version 1 capture",
    );
  });

  it("starts its span once", async () => {
    const capture = new GpuCapture({ contexts: null, meta: {} });
    await capturedDevice(capture);
    await capture.startSpan();
    await expect(capture.startSpan()).rejects.toThrow("starts once");
  });
});

/** A canvas context's prototype whose textures are fake, and the textures it handed out. */
function fakeContexts(): {
  readonly contexts: { getCurrentTexture(this: object): GPUTexture };
  readonly original: (this: object) => GPUTexture;
} {
  const original = function getCurrentTexture(this: object): GPUTexture {
    return new FakeTexture({
      label: "canvas",
      size: [320, 180],
      format: "bgra8unorm",
      usage: RENDER_ATTACHMENT,
    });
  };
  return { contexts: { getCurrentTexture: original }, original };
}

describe("the capture's canvases", () => {
  it("logs a canvas drawn to in the span as a surface a replay stands in for", async () => {
    const { contexts } = fakeContexts();
    const canvas = {};
    const capture = new GpuCapture({ contexts, meta: {} });
    const device = await capturedDevice(capture);
    // A frame before the span: its canvas texture is of no interest and is not logged.
    contexts.getCurrentTexture.call(canvas).createView();
    await capture.startSpan();
    capture.frame();
    const view = contexts.getCurrentTexture.call(canvas).createView();
    const encoder = device.createCommandEncoder();
    encoder
      .beginRenderPass({ colorAttachments: [{ view, loadOp: "clear", storeOp: "store" }] })
      .end();
    device.queue.submit([encoder.finish()]);
    capture.endSpan();
    const { file, data } = capture.result();
    expect(file.problems).toEqual([]);
    expect(file.surfaces).toEqual([
      { id: expect.any(Number), width: 320, height: 180, format: "bgra8unorm" },
    ]);

    const standIn = new FakeTexture({
      label: "stand-in",
      size: [320, 180],
      format: "bgra8unorm",
      usage: RENDER_ATTACHMENT,
    });
    const replayed = new FakeDevice(INTEL_UHD_620_INFO, [], adapter().limits);
    replayCapture(parseCapture(JSON.parse(JSON.stringify(file)), data), replayed, () => ({
      getCurrentTexture: () => standIn,
    }));
    expect(standIn.views).toHaveLength(1);
  });

  it("restores the canvases' method when disposed, whatever its state", async () => {
    const { contexts, original } = fakeContexts();
    const capture = new GpuCapture({ contexts, meta: {} });
    await capturedDevice(capture);
    await capture.startSpan();
    const installed = (): unknown => Reflect.get(contexts, "getCurrentTexture");
    expect(installed()).not.toBe(original);
    capture.dispose();
    expect(installed()).toBe(original);
  });
});

describe("what the capture cannot keep", () => {
  it("skips a depth texture, which can never be written back", async () => {
    const capture = new GpuCapture({ contexts: null, meta: {} });
    const device = await capturedDevice(capture);
    device.createTexture({ size: [4, 4], format: "depth32float", usage: RENDER_ATTACHMENT });
    await capture.startSpan();
    expect(capture.result().file.skipped).toEqual([
      { id: 2, reason: "a depth or stencil format, never a copy destination" },
    ]);
  });

  it("lists a span call on an encoder made before the span as a problem", async () => {
    const capture = new GpuCapture({ contexts: null, meta: {} });
    const device = await capturedDevice(capture);
    const encoder = device.createCommandEncoder({ label: "straddling" });
    await capture.startSpan();
    encoder.clearBuffer(device.createBuffer({ size: 4, usage: COPY_DST }));
    const { file } = capture.result();
    expect(file.problems).toHaveLength(1);
    expect(file.problems[0]).toMatch(/^clearBuffer on object \d+, whose creation the log lacks$/);
    expect(file.calls.slice(file.spanStart).map(({ op }) => op)).toEqual(["createBuffer"]);
  });

  it("refuses to give its result before the snapshot is read back", async () => {
    const capture = new GpuCapture({ contexts: null, meta: {} });
    const device = await capturedDevice(capture);
    device.createBuffer({ size: 4, usage: COPY_DST });
    const started = capture.startSpan();
    expect(() => capture.result()).toThrow("not read back yet");
    await started;
    expect(() => capture.result()).not.toThrow();
  });
});
