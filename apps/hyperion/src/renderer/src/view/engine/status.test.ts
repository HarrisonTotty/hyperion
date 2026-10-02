import { describe, expect, it, onTestFinished, vi } from "vitest";

import type {
  GpuProcessGoneReport,
  GraphicsApi,
  GraphicsLaunchMode,
} from "../../../../preload/api";
import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { type AdapterOutcome, requestAdapterOutcome } from "./platform";
import {
  DEVICE_LOSS_LIMIT,
  feedGraphicsStatus,
  type GraphicsEvent,
  type GraphicsStatus,
  graphicsAnnunciation,
  GraphicsStatusStore,
  initialGraphicsStatus,
  reduceGraphicsStatus,
} from "./status";

async function adapterOutcome(
  info = INTEL_UHD_620_INFO,
  features: ReadonlyArray<GPUFeatureName> = ["subgroups", "timestamp-query"],
): Promise<AdapterOutcome & { readonly kind: "adapter" }> {
  const outcome = await requestAdapterOutcome(new FakeGpu([new FakeAdapter({ info, features })]));
  if (outcome.kind !== "adapter") {
    throw new Error("the fake gave no adapter");
  }
  return outcome;
}

function reduce(status: GraphicsStatus, ...events: ReadonlyArray<GraphicsEvent>): GraphicsStatus {
  return events.reduce(reduceGraphicsStatus, status);
}

function launched(mode: GraphicsLaunchMode = "vulkan", gpuTiming = false): GraphicsStatus {
  return initialGraphicsStatus(mode, gpuTiming);
}

const LOST: GraphicsEvent = { kind: "device-lost", reason: "unknown", message: "gpu gone" };

describe("the graphics condition", () => {
  it("is acquiring until the adapter answers", () => {
    expect(launched().condition).toEqual({ kind: "acquiring" });
  });

  it("is no-webgpu without navigator.gpu", () => {
    const status = reduce(launched(), {
      kind: "adapter-outcome",
      outcome: { kind: "no-webgpu" },
    });
    expect(status.condition).toEqual({ kind: "no-webgpu" });
  });

  it("is no-adapter for a null adapter", () => {
    const status = reduce(launched(), {
      kind: "adapter-outcome",
      outcome: { kind: "no-adapter" },
    });
    expect(status.condition).toEqual({ kind: "no-adapter" });
  });

  it("is nominal on a hardware adapter", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome });
    expect(status.condition).toEqual({
      kind: "nominal",
      summary: outcome.summary,
      styles: { wireframe: true, photorealistic: true },
    });
    expect(status.capabilities).toEqual(outcome.capabilities);
  });

  it("is software-adapter on SwiftShader", async () => {
    const outcome = await adapterOutcome(SWIFTSHADER_INFO);
    const status = reduce(launched(), { kind: "adapter-outcome", outcome });
    expect(status.condition).toEqual({ kind: "software-adapter", summary: outcome.summary });
  });

  it("is safe-mode in a safe launch whatever the adapter", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched("safe"), { kind: "adapter-outcome", outcome });
    expect(status.condition).toEqual({ kind: "safe-mode" });
  });
});

describe("the timer", () => {
  it("is absent without timestamp-query", async () => {
    const outcome = await adapterOutcome(INTEL_UHD_620_INFO, ["subgroups"]);
    const status = reduce(launched("vulkan", true), { kind: "adapter-outcome", outcome });
    expect(status.timer).toBe("absent");
  });

  it("is quantized with timestamp-query and no timing switch", async () => {
    const outcome = await adapterOutcome();
    expect(reduce(launched(), { kind: "adapter-outcome", outcome }).timer).toBe("quantized");
  });

  it("is full only with the timing switch", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched("vulkan", true), { kind: "adapter-outcome", outcome });
    expect(status.timer).toBe("full");
  });
});

describe("the device's capabilities", () => {
  it("replace the adapter's, so that a withheld feature reads as absent", async () => {
    const outcome = await adapterOutcome();
    const capabilities = { ...outcome.capabilities, subgroups: false, timestampQuery: false };
    const status = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      { kind: "device-capabilities", capabilities },
    );
    expect(status.capabilities).toEqual(capabilities);
    expect(status.timer).toBe("absent");
  });

  it("leave a settled condition as it is", async () => {
    const outcome = await adapterOutcome();
    const safe = launched("safe");
    expect(reduce(safe, { kind: "device-capabilities", capabilities: outcome.capabilities })).toBe(
      safe,
    );
  });
});

describe("a refused shader", () => {
  it("is a fault naming the effect by its display name", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      { kind: "shader-refused", effectName: "TEST EFFECT" },
    );
    expect(status.fault).toEqual({ kind: "shader-refused", effectName: "TEST EFFECT" });
    expect(graphicsAnnunciation(status)).toEqual({
      text: "GRAPHICS SHADER REFUSED: TEST EFFECT did not compile, not drawn",
      standing: "fault",
    });
  });

  it("keeps the first refusal when another follows", async () => {
    const outcome = await adapterOutcome();
    const once = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      { kind: "shader-refused", effectName: "TEST EFFECT" },
    );
    expect(reduce(once, { kind: "shader-refused", effectName: "ATMOSPHERE" })).toBe(once);
  });

  it("does not hide a lost device", async () => {
    const outcome = await adapterOutcome();
    const lost = reduce(launched(), { kind: "adapter-outcome", outcome }, LOST);
    expect(reduce(lost, { kind: "shader-refused", effectName: "TEST EFFECT" })).toBe(lost);
  });

  it("is cleared by a restore", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      { kind: "shader-refused", effectName: "TEST EFFECT" },
      { kind: "device-restored", outcome },
    );
    expect(status.fault).toBeNull();
  });

  it("changes nothing in a settled condition", () => {
    const safe = launched("safe");
    expect(reduce(safe, { kind: "shader-refused", effectName: "TEST EFFECT" })).toBe(safe);
  });
});

describe("a refused view", () => {
  const REFUSED: GraphicsEvent = { kind: "view-refused", viewName: "view" };

  it("is a fault in the decided words", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome }, REFUSED);
    expect(status.fault).toEqual({ kind: "view-refused", viewName: "view" });
    expect(graphicsAnnunciation(status)).toEqual({
      text: "GRAPHICS VIEW REFUSED: not re-created after device loss, not drawn, relaunch to retry",
      standing: "fault",
    });
  });

  it("does not hide a lost device or a refused shader", async () => {
    const outcome = await adapterOutcome();
    const lost = reduce(launched(), { kind: "adapter-outcome", outcome }, LOST);
    expect(reduce(lost, REFUSED)).toBe(lost);
    const shader = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      { kind: "shader-refused", effectName: "BODY OCCLUDER" },
    );
    expect(reduce(shader, REFUSED)).toBe(shader);
  });

  it("keeps the first refusal when another view's follows", async () => {
    const outcome = await adapterOutcome();
    const once = reduce(launched(), { kind: "adapter-outcome", outcome }, REFUSED);
    expect(reduce(once, { kind: "view-refused", viewName: "cockpit" })).toBe(once);
  });

  it("is cleared by its own view's release only", async () => {
    const outcome = await adapterOutcome();
    const refused = reduce(launched(), { kind: "adapter-outcome", outcome }, REFUSED);
    expect(reduce(refused, { kind: "view-released", viewName: "cockpit" })).toBe(refused);
    expect(reduce(refused, { kind: "view-released", viewName: "view" }).fault).toBeNull();
    const shader = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      { kind: "shader-refused", effectName: "BODY OCCLUDER" },
    );
    expect(reduce(shader, { kind: "view-released", viewName: "view" })).toBe(shader);
  });

  it("is cleared by a restore, and raised again by a refusal after it", async () => {
    const outcome = await adapterOutcome();
    const refused = reduce(launched(), { kind: "adapter-outcome", outcome }, REFUSED);
    const restored = reduce(refused, LOST, { kind: "device-restored", outcome });
    expect(restored.fault).toBeNull();
    expect(reduce(restored, REFUSED).fault).toEqual({ kind: "view-refused", viewName: "view" });
  });

  it("changes nothing in a settled condition", () => {
    const safe = launched("safe");
    expect(reduce(safe, REFUSED)).toBe(safe);
  });
});

describe("device loss", () => {
  it("sets the fault and counts", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome }, LOST);
    expect(status.fault).toEqual({ kind: "device-lost", reason: "unknown", message: "gpu gone" });
    expect(status.deviceLosses).toBe(1);
  });

  it("clears the fault on a restore and keeps the count", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome }, LOST, {
      kind: "device-restored",
      outcome: await adapterOutcome(),
    });
    expect(status.fault).toBeNull();
    expect(status.deviceLosses).toBe(1);
    expect(status.condition.kind).toBe("nominal");
  });

  it("disables WebGPU at the third loss, and later restores do not leave it", async () => {
    const outcome = await adapterOutcome();
    const restored: GraphicsEvent = { kind: "device-restored", outcome };
    const status = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      LOST,
      restored,
      LOST,
      restored,
      LOST,
      restored,
    );
    expect(DEVICE_LOSS_LIMIT).toBe(3);
    expect(status.condition).toEqual({ kind: "disabled", cause: "device-losses", losses: 3 });
  });

  it("disables WebGPU when a rebuild after one loss finds no adapter", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome }, LOST, {
      kind: "adapter-withdrawn",
    });
    expect(status.condition).toEqual({ kind: "disabled", cause: "adapter-withdrawn", losses: 1 });
  });
});

describe("GPU-process crashes", () => {
  it("set their fault with the count", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      {
        kind: "gpu-process-gone",
        count: 2,
      },
    );
    expect(status.fault).toEqual({ kind: "gpu-process-gone", count: 2 });
    expect(status.gpuProcessCrashes).toBe(2);
  });
});

describe("the annunciation", () => {
  it("is nothing while nominal", async () => {
    const outcome = await adapterOutcome();
    expect(
      graphicsAnnunciation(reduce(launched(), { kind: "adapter-outcome", outcome })),
    ).toBeNull();
  });

  it("states the wait for the adapter in plain text", () => {
    expect(graphicsAnnunciation(launched())).toEqual({
      text: "GRAPHICS ACQUIRING ADAPTER",
      standing: "refused",
    });
  });

  it("states a software adapter", async () => {
    const outcome = await adapterOutcome(SWIFTSHADER_INFO);
    expect(graphicsAnnunciation(reduce(launched(), { kind: "adapter-outcome", outcome }))).toEqual({
      text: "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available",
      standing: "refused",
    });
  });

  it("states no WebGPU and no adapter", () => {
    expect(
      graphicsAnnunciation(
        reduce(launched(), { kind: "adapter-outcome", outcome: { kind: "no-webgpu" } }),
      ),
    ).toEqual({ text: "GRAPHICS NOT AVAILABLE: no WebGPU", standing: "refused" });
    expect(
      graphicsAnnunciation(
        reduce(launched(), { kind: "adapter-outcome", outcome: { kind: "no-adapter" } }),
      ),
    ).toEqual({
      text: "GRAPHICS NO ADAPTER: views not available, relaunch to retry",
      standing: "refused",
    });
  });

  it("reports a lost device and a restarted process as faults", async () => {
    const outcome = await adapterOutcome();
    const nominal = reduce(launched(), { kind: "adapter-outcome", outcome });
    expect(graphicsAnnunciation(reduce(nominal, LOST))).toEqual({
      text: "GRAPHICS DEVICE LOST: re-creating",
      standing: "fault",
    });
    expect(graphicsAnnunciation(reduce(nominal, { kind: "gpu-process-gone", count: 1 }))).toEqual({
      text: "GRAPHICS PROCESS RESTARTED: re-acquiring",
      standing: "fault",
    });
  });

  it("states the safe mode", () => {
    expect(graphicsAnnunciation(launched("safe"))).toEqual({
      text: "GRAPHICS SAFE MODE: views not available, relaunch to retry",
      standing: "refused",
    });
  });

  it("states either cause of the disabled state", async () => {
    const outcome = await adapterOutcome();
    const nominal = reduce(launched(), { kind: "adapter-outcome", outcome });
    expect(graphicsAnnunciation(reduce(nominal, LOST, LOST, LOST))).toEqual({
      text: "GRAPHICS DISABLED: 3 device losses, relaunch to retry",
      standing: "refused",
    });
    expect(graphicsAnnunciation(reduce(nominal, LOST, { kind: "adapter-withdrawn" }))).toEqual({
      text: "GRAPHICS DISABLED: adapter withdrawn, relaunch to retry",
      standing: "refused",
    });
  });
});

describe("the status store", () => {
  it("notifies its subscribers of a change", () => {
    const store = new GraphicsStatusStore(launched());
    const listener = vi.fn<() => void>();
    store.subscribe(listener);
    store.dispatch({ kind: "gpu-process-gone", count: 1 });
    expect(listener).toHaveBeenCalledTimes(1);
    expect(store.getSnapshot().gpuProcessCrashes).toBe(1);
  });

  it("does not notify when an event changes nothing", async () => {
    const store = new GraphicsStatusStore(launched("safe"));
    const listener = vi.fn<() => void>();
    store.subscribe(listener);
    store.dispatch({ kind: "adapter-outcome", outcome: await adapterOutcome() });
    expect(listener).not.toHaveBeenCalled();
  });

  it("stops notifying after its subscriber leaves", () => {
    const store = new GraphicsStatusStore(launched());
    const listener = vi.fn<() => void>();
    const unsubscribe = store.subscribe(listener);
    unsubscribe();
    store.dispatch({ kind: "gpu-process-gone", count: 1 });
    expect(listener).not.toHaveBeenCalled();
  });
});

describe("a restarted GPU process", () => {
  const CRASH: GraphicsEvent = { kind: "gpu-process-gone", count: 1 };

  it("clears on the next granted adapter and keeps its count", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome }, CRASH, {
      kind: "adapter-reacquired",
      outcome: await adapterOutcome(),
    });
    expect(status.fault).toBeNull();
    expect(status.gpuProcessCrashes).toBe(1);
    expect(status.condition.kind).toBe("nominal");
  });

  it("keeps the device's capabilities when the re-acquired adapter answers late", async () => {
    const outcome = await adapterOutcome();
    const deviceCapabilities = { ...outcome.capabilities, timestampQuery: false };
    const status = reduce(
      launched(),
      { kind: "adapter-outcome", outcome },
      CRASH,
      { kind: "device-restored", outcome: await adapterOutcome() },
      { kind: "device-capabilities", capabilities: deviceCapabilities },
      { kind: "adapter-reacquired", outcome: await adapterOutcome() },
    );
    expect(status.capabilities).toEqual(deviceCapabilities);
    expect(status.timer).toBe("absent");
    expect(status.fault).toBeNull();
  });

  it("takes the adapter where none had been granted", async () => {
    const status = reduce(
      launched(),
      { kind: "adapter-outcome", outcome: { kind: "no-adapter" } },
      CRASH,
      { kind: "adapter-reacquired", outcome: await adapterOutcome() },
    );
    expect(status.fault).toBeNull();
    expect(status.condition.kind).toBe("nominal");
  });

  it("takes no adapter where one had been granted as the adapter withdrawn", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome }, CRASH, {
      kind: "adapter-reacquired",
      outcome: { kind: "no-adapter" },
    });
    expect(status.fault).toBeNull();
    expect(status.condition).toEqual({ kind: "disabled", cause: "adapter-withdrawn", losses: 0 });
  });

  it("clears where none had been granted and none is, leaving the condition's remedy", () => {
    const status = reduce(
      launched(),
      { kind: "adapter-outcome", outcome: { kind: "no-adapter" } },
      CRASH,
      { kind: "adapter-reacquired", outcome: { kind: "no-adapter" } },
    );
    expect(status.fault).toBeNull();
    expect(status.gpuProcessCrashes).toBe(1);
    expect(graphicsAnnunciation(status)?.text).toBe(
      "GRAPHICS NO ADAPTER: views not available, relaunch to retry",
    );
  });

  it("does not clear a lost device's fault", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome }, LOST, {
      kind: "adapter-reacquired",
      outcome: await adapterOutcome(),
    });
    expect(status.fault?.kind).toBe("device-lost");
  });
});

describe("a settled condition", () => {
  it("counts a loss in safe mode without a fault", () => {
    const status = reduce(launched("safe"), LOST);
    expect(status.deviceLosses).toBe(1);
    expect(status.fault).toBeNull();
    expect(status.condition).toEqual({ kind: "safe-mode" });
  });

  it("counts a crash once disabled without a fault", async () => {
    const outcome = await adapterOutcome();
    const status = reduce(launched(), { kind: "adapter-outcome", outcome }, LOST, LOST, LOST, {
      kind: "gpu-process-gone",
      count: 4,
    });
    expect(status.gpuProcessCrashes).toBe(4);
    expect(status.fault).toBeNull();
    expect(status.condition.kind).toBe("disabled");
  });
});

describe("the target rounding", () => {
  it("is unknown until probed, then the probe's", () => {
    const rounding = { rgba16float: "toward-zero", rg11b10ufloat: "unknown" } as const;
    expect(launched().targetRounding).toEqual({ rgba16float: "unknown", rg11b10ufloat: "unknown" });
    expect(reduce(launched(), { kind: "target-rounding", rounding }).targetRounding).toEqual(
      rounding,
    );
  });
});

/** A preload's graphics API whose crash reports the test sends. */
function fakeGraphics(launchMode: GraphicsLaunchMode): {
  readonly api: GraphicsApi;
  readonly crash: (report: GpuProcessGoneReport) => void;
  readonly listeners: () => number;
} {
  const listeners = new Set<(report: GpuProcessGoneReport) => void>();
  return {
    api: {
      launchMode,
      gpuTiming: false,
      onGpuProcessGone: (listener) => {
        listeners.add(listener);
        return () => {
          listeners.delete(listener);
        };
      },
    },
    crash: (report) => {
      for (const listener of listeners) {
        listener(report);
      }
    },
    listeners: () => listeners.size,
  };
}

describe("the status feed", () => {
  it("dispatches the first adapter's outcome", async () => {
    const store = new GraphicsStatusStore(launched());
    const gpu = new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] })]);
    const end = feedGraphicsStatus(store, fakeGraphics("vulkan").api, gpu);
    await vi.waitFor(() => {
      expect(store.getSnapshot().condition.kind).toBe("nominal");
    });
    end();
  });

  it("drops an answer that arrives after it ends", async () => {
    const store = new GraphicsStatusStore(launched());
    const gpu = new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] })]);
    const end = feedGraphicsStatus(store, fakeGraphics("vulkan").api, gpu);
    end();
    await Promise.resolve();
    await Promise.resolve();
    expect(store.getSnapshot().condition).toEqual({ kind: "acquiring" });
  });

  it("gives no-adapter when the request fails", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const store = new GraphicsStatusStore(launched());
    const gpu = new FakeGpu([]);
    vi.spyOn(gpu, "requestAdapter").mockRejectedValue(new Error("GPU process gone"));
    const end = feedGraphicsStatus(store, fakeGraphics("vulkan").api, gpu);
    await vi.waitFor(() => {
      expect(store.getSnapshot().condition).toEqual({ kind: "no-adapter" });
    });
    end();
  });

  it("asks for no adapter in safe mode", () => {
    const gpu = new FakeGpu([]);
    feedGraphicsStatus(new GraphicsStatusStore(launched("safe")), fakeGraphics("safe").api, gpu);
    expect(gpu.requests).toEqual([]);
  });

  it("asks for an adapter again after a crash and clears the fault when it is granted", async () => {
    const store = new GraphicsStatusStore(launched());
    const graphics = fakeGraphics("vulkan");
    const gpu = new FakeGpu([
      new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }),
      new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }),
    ]);
    const end = feedGraphicsStatus(store, graphics.api, gpu);
    await vi.waitFor(() => {
      expect(store.getSnapshot().condition.kind).toBe("nominal");
    });
    graphics.crash({ reason: "crashed", count: 1 });
    expect(store.getSnapshot().fault).toEqual({ kind: "gpu-process-gone", count: 1 });
    await vi.waitFor(() => {
      expect(store.getSnapshot().fault).toBeNull();
    });
    expect(gpu.requests).toHaveLength(2);
    expect(store.getSnapshot().gpuProcessCrashes).toBe(1);
    end();
  });

  it("takes no adapter after a crash as the adapter withdrawn", async () => {
    const store = new GraphicsStatusStore(launched());
    const graphics = fakeGraphics("vulkan");
    const gpu = new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }), null]);
    const end = feedGraphicsStatus(store, graphics.api, gpu);
    await vi.waitFor(() => {
      expect(store.getSnapshot().condition.kind).toBe("nominal");
    });
    graphics.crash({ reason: "crashed", count: 1 });
    await vi.waitFor(() => {
      expect(store.getSnapshot().condition).toEqual({
        kind: "disabled",
        cause: "adapter-withdrawn",
        losses: 0,
      });
    });
    end();
  });

  it("asks for no adapter after a crash in safe mode", () => {
    const gpu = new FakeGpu([]);
    const graphics = fakeGraphics("safe");
    const end = feedGraphicsStatus(new GraphicsStatusStore(launched("safe")), graphics.api, gpu);
    onTestFinished(end);
    graphics.crash({ reason: "crashed", count: 1 });
    expect(gpu.requests).toEqual([]);
  });

  it("drops the first answer when a crash comes before it", async () => {
    const store = new GraphicsStatusStore(launched());
    const graphics = fakeGraphics("vulkan");
    const gpu = new FakeGpu([]);
    const grants: Array<(adapter: GPUAdapter | null) => void> = [];
    vi.spyOn(gpu, "requestAdapter").mockImplementation(
      () =>
        new Promise((resolve) => {
          grants.push(resolve);
        }),
    );
    const end = feedGraphicsStatus(store, graphics.api, gpu);
    onTestFinished(end);
    graphics.crash({ reason: "crashed", count: 1 });
    grants[0]?.(new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }));
    await Promise.resolve();
    await Promise.resolve();
    expect(store.getSnapshot().fault).toEqual({ kind: "gpu-process-gone", count: 1 });
    expect(store.getSnapshot().condition).toEqual({ kind: "acquiring" });
    expect(grants).toHaveLength(2);
    grants[1]?.(new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }));
    await vi.waitFor(() => {
      expect(store.getSnapshot().fault).toBeNull();
    });
    expect(store.getSnapshot().condition.kind).toBe("nominal");
  });

  it("passes on crash reports until it ends", () => {
    const store = new GraphicsStatusStore(launched());
    const graphics = fakeGraphics("vulkan");
    const end = feedGraphicsStatus(store, graphics.api, undefined);
    graphics.crash({ reason: "killed", count: 1 });
    expect(store.getSnapshot().gpuProcessCrashes).toBe(1);
    end();
    expect(graphics.listeners()).toBe(0);
  });
});
