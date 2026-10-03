import { describe, expect, it } from "vitest";

import { FakeAdapter, FakeDevice, FakeGpu, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { PipelineTally, shimPipelines, wrapGpu } from "./pipelineShim";

function adapter(): FakeAdapter {
  return new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
}

const SHADER = { label: "s", code: "" };

/** A clock the test moves by hand. */
function clock(): { now: number; read: () => number } {
  const state = { now: 0, read: (): number => state.now };
  return state;
}

describe("the wrapped GPU", () => {
  it("passes every device its adapters give through the wrapper, and returns the same device", async () => {
    const seen: GPUDevice[] = [];
    const gpu = wrapGpu(new FakeGpu([adapter()]), (device) => {
      seen.push(device);
      return device;
    });
    const wrapped = await gpu.requestAdapter({ powerPreference: "high-performance" });
    const device = await wrapped?.requestDevice();
    expect(device).toBeInstanceOf(FakeDevice);
    expect(seen).toEqual([device]);
  });

  it("forwards the adapter request's options and the canvas format", async () => {
    const fake = new FakeGpu([adapter()]);
    const gpu = wrapGpu(fake, (device) => device);
    await gpu.requestAdapter({ powerPreference: "high-performance" });
    expect(fake.requests).toEqual([{ powerPreference: "high-performance" }]);
    expect(gpu.getPreferredCanvasFormat()).toBe("bgra8unorm");
  });

  it("gives null where the GPU has no adapter", async () => {
    const gpu = wrapGpu(new FakeGpu([]), (device) => device);
    await expect(gpu.requestAdapter()).resolves.toBeNull();
  });
});

describe("the pipeline shim", () => {
  async function shimmed(): Promise<{
    device: FakeDevice;
    tally: PipelineTally;
    time: ReturnType<typeof clock>;
  }> {
    const time = clock();
    const tally = new PipelineTally(time.read);
    const device = new FakeDevice(INTEL_UHD_620_INFO, [], adapter().limits);
    shimPipelines(device, tally);
    return Promise.resolve({ device, tally, time });
  }

  it("passes each creation through unchanged", async () => {
    const { device } = await shimmed();
    const module = device.createShaderModule(SHADER);
    const descriptor: GPUComputePipelineDescriptor = {
      label: "k",
      layout: "auto",
      compute: { module, entryPoint: "main" },
    };
    const pipeline = device.createComputePipeline(descriptor);
    expect(device.computePipelines).toEqual([pipeline]);
    expect(device.computePipelines[0]?.descriptor).toBe(descriptor);
    const fromAsync = await device.createComputePipelineAsync(descriptor);
    expect(device.computePipelines).toEqual([pipeline, fromAsync]);
  });

  it("counts a creation after the warm-up as late, with its label and kind", async () => {
    const { device, tally, time } = await shimmed();
    const module = device.createShaderModule(SHADER);
    const render: GPURenderPipelineDescriptor = {
      label: "terrain",
      layout: "auto",
      vertex: { module, entryPoint: "vertexMain" },
    };
    device.createRenderPipeline(render);
    tally.endWarmup();
    time.now = 12_500;
    await device.createRenderPipelineAsync({ ...render, label: "atmosphere.sky" });
    device.createComputePipeline({ label: "normals", layout: "auto", compute: { module } });
    expect(tally.creations).toHaveLength(3);
    expect(tally.late()).toEqual([
      { label: "atmosphere.sky", kind: "render", async: true, atMs: 12_500, late: true },
      { label: "normals", kind: "compute", async: false, atMs: 12_500, late: true },
    ]);
  });
});
