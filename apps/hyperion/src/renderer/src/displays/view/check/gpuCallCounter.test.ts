import { describe, expect, it } from "vitest";

import { FakeAdapter, FakeGpu, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import { wrapGpu } from "../../../view/spike/pipelineShim";
import { AFTER_LAST_PASS, bufferWriteBytes, GpuCallCounter } from "./gpuCallCounter";

/** A counted device on a fake adapter, through the seam the views probe's engine source uses. */
async function countedDevice(): Promise<{
  readonly counter: GpuCallCounter;
  readonly device: GPUDevice;
}> {
  const counter = new GpuCallCounter();
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
    .map(
      ({ label, call, count, bytes }) =>
        `${label}: ${call} × ${String(count)}${bytes > 0 ? ` (${String(bytes)} B)` : ""}`,
    );
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
    expect(counter.take()).toEqual([]);
    device.createCommandEncoder({ label: "after" });
    expect(lines(counter)).toEqual(["after: device.createCommandEncoder × 1"]);
    expect(counter.take()).toEqual([]);
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
});
