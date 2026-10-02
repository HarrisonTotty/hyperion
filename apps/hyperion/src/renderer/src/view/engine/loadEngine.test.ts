import { describe, expect, it, vi } from "vitest";

import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { loadRenderEngine } from "./loadEngine";
import { type AdapterOutcome, requestAdapterOutcome } from "./platform";
import { GraphicsStatusStore, initialGraphicsStatus } from "./status";
import type { CreateWebGpuEngine } from "./types";

interface EngineModule {
  readonly createWebGpuEngine: CreateWebGpuEngine;
}

async function vettedAdapter(): Promise<AdapterOutcome & { readonly kind: "adapter" }> {
  const outcome = await requestAdapterOutcome(
    new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: ["subgroups"] })]),
  );
  if (outcome.kind !== "adapter") {
    throw new Error("the fake gave no adapter");
  }
  return outcome;
}

/**
 * What the fake engine factory fails with, so that the test follows the call through the loader
 * without building a whole engine.
 */
const CREATED = new Error("the fake engine was created");

describe("loading the engine", () => {
  it("imports the engine module once per load", async () => {
    const outcome = await vettedAdapter();
    const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const createWebGpuEngine = vi.fn<CreateWebGpuEngine>().mockRejectedValue(CREATED);
    const importEngine = vi
      .fn<() => Promise<EngineModule>>()
      .mockResolvedValue({ createWebGpuEngine });
    const options = { importEngine };

    await expect(loadRenderEngine(outcome, status, options)).rejects.toBe(CREATED);
    expect(importEngine).toHaveBeenCalledOnce();
  });

  it("hands the engine the adapter, the store and the overrides", async () => {
    const outcome = await vettedAdapter();
    const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const overrides = { withholdSubgroups: true, withholdShaderF16: false };
    const createWebGpuEngine = vi.fn<CreateWebGpuEngine>().mockRejectedValue(CREATED);

    await expect(
      loadRenderEngine(outcome, status, {
        overrides,
        importEngine: () => Promise.resolve({ createWebGpuEngine }),
      }),
    ).rejects.toBe(CREATED);

    expect(createWebGpuEngine).toHaveBeenCalledExactlyOnceWith(outcome, status, overrides);
  });

  it("passes no overrides when given none", async () => {
    const outcome = await vettedAdapter();
    const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const createWebGpuEngine = vi.fn<CreateWebGpuEngine>().mockRejectedValue(CREATED);

    await expect(
      loadRenderEngine(outcome, status, {
        importEngine: () => Promise.resolve({ createWebGpuEngine }),
      }),
    ).rejects.toBe(CREATED);

    expect(createWebGpuEngine).toHaveBeenCalledExactlyOnceWith(outcome, status, undefined);
  });
});
