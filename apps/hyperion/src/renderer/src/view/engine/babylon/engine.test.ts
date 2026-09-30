import { describe, expect, it } from "vitest";

import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import { initialiseOnAdapter, type InitialisableEngine, postProcessSamplers } from "./engine";

const SWIFTSHADER_FEATURES: ReadonlyArray<GPUFeatureName> = [
  "subgroups",
  "timestamp-query",
  "float32-filterable",
  "float32-blendable",
];

/** An engine whose `initAsync` asks the entry point for an adapter, as Babylon's does. */
class FakeEngine implements InitialisableEngine {
  readonly enabledExtensions: ReadonlyArray<string>;
  /** The adapter `initAsync` was answered with. */
  handed: GPUAdapter | null = null;
  readonly #gpu: GPU;
  readonly #failure: Error | null;

  constructor(gpu: GPU, enabled: ReadonlyArray<string>, failure: Error | null = null) {
    this.#gpu = gpu;
    this.enabledExtensions = enabled;
    this.#failure = failure;
  }

  async initAsync(): Promise<void> {
    this.handed = await this.#gpu.requestAdapter();
    if (this.#failure !== null) {
      throw this.#failure;
    }
  }
}

describe("the engine's creation", () => {
  it("hands initAsync the vetted adapter and restores the entry point after", async () => {
    const gpu = new FakeGpu([]);
    const vetted = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
    const engine = new FakeEngine(gpu, []);
    await initialiseOnAdapter(engine, gpu, vetted, []);
    expect(engine.handed).toBe(vetted);
    expect(Object.hasOwn(gpu, "requestAdapter")).toBe(false);
    expect(await gpu.requestAdapter()).toBeNull();
  });

  it("restores the entry point when initAsync fails", async () => {
    const gpu = new FakeGpu([]);
    const vetted = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
    const failure = new Error("no device");
    await expect(
      initialiseOnAdapter(new FakeEngine(gpu, [], failure), gpu, vetted, []),
    ).rejects.toBe(failure);
    expect(Object.hasOwn(gpu, "requestAdapter")).toBe(false);
  });

  it("reports a feature asked for and not enabled", async () => {
    const gpu = new FakeGpu([]);
    const vetted = new FakeAdapter({ info: SWIFTSHADER_INFO, features: SWIFTSHADER_FEATURES });
    const engine = new FakeEngine(gpu, ["subgroups", "timestamp-query"]);
    expect(
      await initialiseOnAdapter(engine, gpu, vetted, [
        "subgroups",
        "timestamp-query",
        "float32-blendable",
      ]),
    ).toEqual(["float32-blendable"]);
  });
});

describe("a post-process's inputs", () => {
  it("bind the depth by its name and leave the colour to Babylon", () => {
    const spec = { name: "aerial", fragmentWgsl: "", uniforms: [] };
    expect(postProcessSamplers({ ...spec, inputs: ["depth", "hdr-colour"] })).toEqual([
      "depthTexture",
    ]);
    expect(postProcessSamplers(spec)).toEqual([]);
  });
});
