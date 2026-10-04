import { describe, expect, it, vi } from "vitest";

import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import {
  FAKE_ENGINE_MEMORY,
  type FakeEngineScript,
  fakeEngineModule,
  type FakeRenderEngine,
} from "../../test/fakeRenderEngine";
import { loadRenderEngine } from "./loadEngine";
import type { AllocationEvent } from "./memory";
import { type AdapterOutcome, requestAdapterOutcome } from "./platform";
import { DEVICE_LOSS_LIMIT, GraphicsStatusStore, initialGraphicsStatus } from "./status";
import { EngineUnavailable } from "./resilientEngine";
import type { RenderEngine } from "./types";

const FEATURES: ReadonlyArray<GPUFeatureName> = ["subgroups", "timestamp-query"];

function adapter(): FakeAdapter {
  return new FakeAdapter({ info: INTEL_UHD_620_INFO, features: FEATURES });
}

interface Loaded {
  readonly engine: RenderEngine;
  readonly status: GraphicsStatusStore;
  readonly module: ReturnType<typeof fakeEngineModule>;
  readonly gpu: GPU & { readonly requests: ReadonlyArray<unknown> };
}

/** An entry point whose adapters the test hands over when it chooses. */
class HeldGpu extends FakeGpu {
  readonly #held: Array<(adapter: FakeAdapter | null) => void> = [];

  /** How many requests wait. */
  get waiting(): number {
    return this.#held.length;
  }

  override requestAdapter(options?: GPURequestAdapterOptions): Promise<GPUAdapter | null> {
    if (this.requests.length === 0) {
      return super.requestAdapter(options);
    }
    this.requests.push(options);
    return new Promise((resolve) => {
      this.#held.push(resolve);
    });
  }

  /** Answers the oldest waiting request. */
  answer(given: FakeAdapter | null): void {
    this.#held.shift()?.(given);
  }
}

/** Loads an engine on the first of `adapters`; each rebuild takes the next. */
async function load(
  adapters: ReadonlyArray<FakeAdapter | null>,
  script: FakeEngineScript = {},
  gpu: FakeGpu = new FakeGpu(adapters),
): Promise<Loaded & { readonly gpu: FakeGpu }> {
  const outcome: AdapterOutcome = await requestAdapterOutcome(gpu);
  if (outcome.kind !== "adapter") {
    throw new Error("the fake gave no first adapter");
  }
  const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  status.dispatch({ kind: "adapter-outcome", outcome });
  const module = fakeEngineModule(script);
  const engine = await loadRenderEngine(outcome, status, {
    importEngine: () => Promise.resolve(module),
    gpu,
  });
  return { engine, status, module, gpu };
}

function nth(module: ReturnType<typeof fakeEngineModule>, index: number): FakeRenderEngine {
  const engine = module.engines.at(index);
  if (engine === undefined) {
    throw new Error(`no engine ${index} was made`);
  }
  return engine;
}

describe("a device loss", () => {
  it("re-creates the engine on a fresh adapter, never the consumed one", async () => {
    const first = adapter();
    const second = adapter();
    const { module, gpu, status } = await load([first, second]);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(2);
    });
    expect(module.adapters).toEqual([first, second]);
    expect(gpu.requests).toHaveLength(2);
    expect(nth(module, 0).disposed).toBe(true);
    expect(status.getSnapshot().deviceLosses).toBe(1);
    expect(status.getSnapshot().fault).toBeNull();
  });

  it("reports the device's capabilities after the restore", async () => {
    const { module, status } = await load([adapter(), adapter()]);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(2);
    });
    expect(status.getSnapshot().capabilities).toEqual(nth(module, 1).capabilities);
  });

  it("reports the current engine's timing frame, from 0 again after the restore (R07.T19)", async () => {
    const { engine, module } = await load([adapter(), adapter()]);
    nth(module, 0).passTimesFrame = 7;
    const before = engine.passTimesFrame;
    const restored = vi.fn<() => void>();
    engine.onRestored(restored);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(restored).toHaveBeenCalledOnce();
    });
    expect([before, engine.passTimesFrame]).toEqual([7, 0]);
  });

  it("tells the caller once the engine is restored", async () => {
    const { engine, module } = await load([adapter(), adapter()]);
    const restored = vi.fn<() => void>();
    engine.onRestored(restored);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(restored).toHaveBeenCalledOnce();
    });
  });

  it("disables WebGPU when the retry is given no adapter", async () => {
    const { module, status } = await load([adapter(), null]);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(status.getSnapshot().condition).toEqual({
        kind: "disabled",
        cause: "adapter-withdrawn",
        losses: 1,
      });
    });
    expect(module.engines).toHaveLength(1);
  });

  it("stops re-creating at the session's limit", async () => {
    const adapters = Array.from({ length: DEVICE_LOSS_LIMIT + 1 }, adapter);
    const { module, status } = await load(adapters);
    for (let loss = 0; loss < DEVICE_LOSS_LIMIT; loss += 1) {
      // Each loss waits for the rebuild the previous one started, in order.
      // oxlint-disable-next-line no-await-in-loop
      await vi.waitFor(() => {
        expect(module.engines).toHaveLength(loss + 1);
      });
      nth(module, loss).loseDevice();
    }
    await vi.waitFor(() => {
      expect(status.getSnapshot().condition).toMatchObject({ cause: "device-losses" });
    });
    expect(module.engines).toHaveLength(DEVICE_LOSS_LIMIT);
  });

  it("re-creates each view on its canvas at its size", async () => {
    const { engine, module } = await load([adapter(), adapter()]);
    const canvas = document.createElement("canvas");
    const view = engine.createView(canvas, "cockpit");
    view.resize({ widthPx: 640, heightPx: 360 });
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(nth(module, 1).views).toHaveLength(1);
    });
    const rebuilt = nth(module, 1).views.at(0);
    expect(rebuilt?.canvas).toBe(canvas);
    expect(rebuilt?.name).toBe("cockpit");
    expect(rebuilt?.sizes).toEqual([{ widthPx: 640, heightPx: 360 }]);
    expect(nth(module, 0).views.at(0)?.disposed).toBe(true);
  });

  it("is not acted on after the engine is disposed", async () => {
    const { engine, module, gpu } = await load([adapter(), adapter()]);
    const lost = nth(module, 0);
    engine.dispose();
    lost.loseDevice();
    await Promise.resolve();
    expect(gpu.requests).toHaveLength(1);
  });

  it("counts a failed creation as another loss and tries the next adapter", async () => {
    const { module, status, gpu } = await load([adapter(), adapter(), adapter()], {
      failing: new Set([1]),
    });
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(2);
    });
    expect(gpu.requests).toHaveLength(3);
    expect(status.getSnapshot().deviceLosses).toBe(2);
    expect(status.getSnapshot().fault).toBeNull();
  });

  it("re-creates an engine whose device was lost before it was handed over", async () => {
    const { module, status } = await load([adapter(), adapter(), adapter()], {
      lostAtBirth: new Set([1]),
    });
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(3);
    });
    expect(status.getSnapshot().deviceLosses).toBe(2);
    expect(status.getSnapshot().condition.kind).toBe("nominal");
  });

  it("makes nothing when disposed while the fresh adapter is awaited", async () => {
    const gpu = new HeldGpu([adapter()]);
    const { engine, module, status } = await load([], {}, gpu);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(gpu.waiting).toBe(1);
    });
    engine.dispose();
    gpu.answer(adapter());
    await Promise.resolve();
    await Promise.resolve();
    expect(module.engines).toHaveLength(1);
    expect(status.getSnapshot().fault).toMatchObject({ kind: "device-lost" });
  });

  it("draws a view made during the outage once the engine is restored", async () => {
    const gpu = new HeldGpu([adapter()]);
    const { engine, module } = await load([], {}, gpu);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(gpu.waiting).toBe(1);
    });
    const view = engine.createView(document.createElement("canvas"), "late");
    await expect(view.readBack()).rejects.toBeInstanceOf(EngineUnavailable);
    gpu.answer(adapter());
    await vi.waitFor(() => {
      expect(nth(module, 1).views.map(({ name }) => name)).toEqual(["late"]);
    });
  });

  it("keeps the restored engine when an onRestored listener throws", async () => {
    const { engine, module, status, gpu } = await load([adapter(), adapter(), adapter()]);
    const told = vi.fn<() => void>();
    engine.onRestored(() => {
      throw new Error("the caller's material no longer matches");
    });
    engine.onRestored(told);
    const logged = vi.spyOn(console, "error").mockImplementation(() => undefined);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(told).toHaveBeenCalledOnce();
    });
    await Promise.resolve();
    expect(module.engines).toHaveLength(2);
    expect(nth(module, 1).disposed).toBe(false);
    expect(gpu.requests).toHaveLength(2);
    expect(status.getSnapshot().deviceLosses).toBe(1);
    expect(status.getSnapshot().condition.kind).toBe("nominal");
    expect(logged).toHaveBeenCalledWith("an onRestored listener failed:", expect.any(Error));
  });

  it("keeps the restored engine when a view's canvas gives no context", async () => {
    const { engine, module, status } = await load([adapter(), adapter(), adapter()], {
      viewless: new Set([1]),
    });
    const view = engine.createView(document.createElement("canvas"), "cockpit");
    const logged = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const dispatched = vi.spyOn(status, "dispatch");
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(logged).toHaveBeenCalledWith(
        "view cockpit could not be re-created after a device loss:",
        expect.any(Error),
      );
    });
    await Promise.resolve();
    expect(module.engines).toHaveLength(2);
    expect(nth(module, 1).disposed).toBe(false);
    expect(status.getSnapshot().condition.kind).toBe("nominal");
    expect(status.getSnapshot().deviceLosses).toBe(1);
    const kinds = dispatched.mock.calls.map(([event]) => event.kind);
    expect(kinds.slice(kinds.indexOf("device-restored"))).toEqual([
      "device-restored",
      "device-capabilities",
      "view-refused",
    ]);
    expect(status.getSnapshot().fault).toEqual({ kind: "view-refused", viewName: "cockpit" });
    view.dispose();
    expect(dispatched).toHaveBeenLastCalledWith({ kind: "view-released", viewName: "cockpit" });
    expect(status.getSnapshot().fault).toBeNull();
  });

  it("releases nothing for a refused view that a later restore re-created", async () => {
    const { engine, module, status } = await load([adapter(), adapter(), adapter()], {
      viewless: new Set([1]),
    });
    const view = engine.createView(document.createElement("canvas"), "cockpit");
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(status.getSnapshot().fault).toEqual({ kind: "view-refused", viewName: "cockpit" });
    });
    nth(module, 1).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(3);
    });
    await Promise.resolve();
    expect(status.getSnapshot().fault).toBeNull();
    const dispatched = vi.spyOn(status, "dispatch");
    view.dispose();
    expect(dispatched).not.toHaveBeenCalled();
  });

  it("releases no fault when a view that was re-created is disposed", async () => {
    const { engine, module, status } = await load([adapter(), adapter()]);
    const view = engine.createView(document.createElement("canvas"), "cockpit");
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(2);
    });
    const dispatched = vi.spyOn(status, "dispatch");
    view.dispose();
    expect(dispatched).not.toHaveBeenCalled();
  });

  it("forwards the lost engine's memory releases to the allocation listeners", async () => {
    const { engine, module } = await load([adapter(), adapter()]);
    const events = vi.fn<(event: AllocationEvent) => void>();
    engine.onAllocation(events);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(2);
    });
    expect(events).toHaveBeenCalledWith({ kind: "destroyed", ...FAKE_ENGINE_MEMORY });
  });

  it("forwards a release to the engine that made the handle (R06.T13.h)", async () => {
    const { engine, module } = await load([adapter()]);
    const buffer = engine.createBuffer({ name: "b", bytes: 4, usage: 0x40, category: "other" });
    engine.releaseBuffer(buffer);
    expect(nth(module, 0).released).toEqual([buffer]);
  });

  it("drops the release of a handle a lost engine made (R06.T13.h)", async () => {
    const { engine, module } = await load([adapter(), adapter()]);
    const buffer = engine.createBuffer({ name: "b", bytes: 4, usage: 0x40, category: "other" });
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(2);
    });
    engine.releaseBuffer(buffer);
    expect(nth(module, 0).released).toEqual([]);
    expect(nth(module, 1).released).toEqual([]);
  });

  it("refuses the release of a handle it did not make (R06.T13.h)", async () => {
    const { engine } = await load([adapter()]);
    const stranger = { kind: "buffer", name: "stranger", bytes: 4 } as const;
    expect(() => {
      engine.releaseBuffer(stranger);
    }).toThrow(/stranger was not made through this engine/u);
  });

  it("forwards the release of a handle made after a restore to the new engine (R06.T13.h)", async () => {
    const { engine, module } = await load([adapter(), adapter()]);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(module.engines).toHaveLength(2);
    });
    const buffer = engine.createBuffer({ name: "b", bytes: 4, usage: 0x40, category: "other" });
    engine.releaseBuffer(buffer);
    expect(nth(module, 1).released).toEqual([buffer]);
  });

  it("drops writes and refuses creations while there is no device", async () => {
    const gpu = new HeldGpu([adapter()]);
    const { engine, module } = await load([], {}, gpu);
    nth(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(gpu.waiting).toBe(1);
    });
    const buffer = { kind: "buffer", name: "b", bytes: 4 } as const;
    expect(() => {
      engine.writeBuffer(buffer, 0, new Float32Array(1));
    }).not.toThrow();
    expect(() =>
      engine.createBuffer({ name: "b", bytes: 4, usage: 0x40, category: "other" }),
    ).toThrow(EngineUnavailable);
  });
});
