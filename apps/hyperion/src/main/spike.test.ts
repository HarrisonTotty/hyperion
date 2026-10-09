import { describe, expect, it, type Mock, vi } from "vitest";

import {
  applyGraphicsSwitches,
  type ChromiumSwitch,
  type GraphicsLaunchMode,
  graphicsSwitches,
} from "./graphics/switches";
import {
  DAWN_SAFETY_OFF_DISABLED,
  DAWN_SAFETY_OFF_ENABLED,
  type DawnSafety,
  launchSwitches,
  registerSpikeHandlers,
  SPIKE_CHANNELS,
  type SpikeHandlerDeps,
  SPIKE_GPU_CATEGORY,
  SPIKE_PROFILED_TRACE_BUFFER_KB,
  SPIKE_PROFILER_CATEGORY,
  SPIKE_TRACE_BUFFER_KB,
  SPIKE_TRACE_CATEGORIES,
  SPIKE_TRACE_FORMAT,
  SpikeTrace,
  type SpikeTracing,
  spikeTraceConfig,
  type TraceWindowStop,
  traceSettingsOf,
} from "./spike";
import { smallReport } from "./fixtures/spikeReport";

const PLATFORMS: ReadonlyArray<NodeJS.Platform> = ["linux", "win32", "darwin"];
const MODES: ReadonlyArray<GraphicsLaunchMode> = ["default", "vulkan", "safe"];
const SAFETY: ReadonlyArray<DawnSafety> = ["on", "off"];

const VULKAN_SET: ReadonlyArray<ChromiumSwitch> = [
  { name: "use-angle", value: "vulkan" },
  { name: "enable-features", value: "Vulkan,VulkanFromANGLE,DefaultANGLEVulkan" },
  { name: "enable-dawn-features", value: "enable_subgroups_intel_gen9" },
];

/** A command line that keeps the last copy of each switch, as Chromium's does. */
class FakeCommandLine {
  readonly appended: Array<{ readonly name: string; readonly value: string | undefined }> = [];
  readonly #values = new Map<string, string>();

  appendSwitch(name: string, value?: string): void {
    this.appended.push({ name, value });
    this.#values.set(name, value ?? "");
  }

  getSwitchValue(name: string): string {
    return this.#values.get(name) ?? "";
  }
}

const TIMING: ChromiumSwitch = { name: "disable-dawn-features", value: "timestamp_quantization" };
const SAFETY_OFF_ENABLE: ChromiumSwitch = {
  name: "enable-dawn-features",
  value: DAWN_SAFETY_OFF_ENABLED.join(","),
};
const SAFETY_OFF_DISABLE: ChromiumSwitch = {
  name: "disable-dawn-features",
  value: DAWN_SAFETY_OFF_DISABLED.join(","),
};
const VULKAN_SAFETY_OFF: ReadonlyArray<ChromiumSwitch> = [
  { name: "use-angle", value: "vulkan" },
  { name: "enable-features", value: "Vulkan,VulkanFromANGLE,DefaultANGLEVulkan" },
  {
    name: "enable-dawn-features",
    value: ["enable_subgroups_intel_gen9", ...DAWN_SAFETY_OFF_ENABLED].join(","),
  },
  {
    name: "disable-dawn-features",
    value: ["timestamp_quantization", ...DAWN_SAFETY_OFF_DISABLED].join(","),
  },
];

/** The documented switch set of a spike run, whatever `--hyperion-gpu-timing` says. */
function documentedSpikeSet(
  platform: NodeJS.Platform,
  mode: GraphicsLaunchMode,
  dawnSafety: DawnSafety,
): ReadonlyArray<ChromiumSwitch> {
  const forced = platform === "linux" && mode === "vulkan";
  if (mode === "safe") {
    return [];
  }
  if (dawnSafety === "on") {
    return forced ? [...VULKAN_SET, TIMING] : [];
  }
  return forced ? VULKAN_SAFETY_OFF : [SAFETY_OFF_ENABLE, SAFETY_OFF_DISABLE];
}

describe("the spike's measurement switches", () => {
  it("are exactly the documented set for every platform, mode, safety and timing request", () => {
    for (const platform of [...PLATFORMS, "freebsd" as const]) {
      for (const mode of MODES) {
        for (const dawnSafety of SAFETY) {
          for (const gpuTiming of [false, true]) {
            expect(launchSwitches({ platform, mode, gpuTiming }, { dawnSafety })).toEqual(
              documentedSpikeSet(platform, mode, dawnSafety),
            );
          }
        }
      }
    }
  });

  it("leave an ordinary launch with exactly R01's switches", () => {
    for (const platform of PLATFORMS) {
      for (const mode of MODES) {
        for (const gpuTiming of [false, true]) {
          const options = { platform, mode, gpuTiming };
          expect(launchSwitches(options, undefined)).toEqual(graphicsSwitches(options));
        }
      }
    }
  });

  it("turn R01's GPU timing on, and nothing else, with the safety checks on", () => {
    expect(
      launchSwitches({ platform: "linux", mode: "vulkan", gpuTiming: false }, { dawnSafety: "on" }),
    ).toEqual([...VULKAN_SET, { name: "disable-dawn-features", value: "timestamp_quantization" }]);
  });

  it("merge the safety toggles into one switch of each list with timing on", () => {
    const switches = launchSwitches(
      { platform: "linux", mode: "vulkan", gpuTiming: false },
      { dawnSafety: "off" },
    );
    expect(switches).toEqual([
      { name: "use-angle", value: "vulkan" },
      { name: "enable-features", value: "Vulkan,VulkanFromANGLE,DefaultANGLEVulkan" },
      {
        name: "enable-dawn-features",
        value: [
          "enable_subgroups_intel_gen9",
          "disable_robustness",
          "skip_validation",
          "disable_workgroup_init",
          "disable_lazy_clear_for_mapped_at_creation_buffer",
          "disable_polyfills_on_integer_div_and_mod",
        ].join(","),
      },
      {
        name: "disable-dawn-features",
        value: "timestamp_quantization,lazy_clear_resource_on_first_use",
      },
    ]);
  });

  it("reach the command line as one value a list switch", () => {
    const commandLine = new FakeCommandLine();
    applyGraphicsSwitches(
      commandLine,
      launchSwitches(
        { platform: "linux", mode: "vulkan", gpuTiming: false },
        { dawnSafety: "off" },
      ),
    );
    const disables = commandLine.appended.filter(({ name }) => name === "disable-dawn-features");
    expect(disables).toEqual([
      {
        name: "disable-dawn-features",
        value: "timestamp_quantization,lazy_clear_resource_on_first_use",
      },
    ]);
    expect(commandLine.appended.filter(({ name }) => name === "enable-dawn-features")).toHaveLength(
      1,
    );
    expect(commandLine.getSwitchValue("enable-dawn-features").split(",")).toEqual([
      "enable_subgroups_intel_gen9",
      ...DAWN_SAFETY_OFF_ENABLED,
    ]);
  });

  it("put the safety toggles on a default-mode launch, which has no R01 switches", () => {
    expect(
      launchSwitches(
        { platform: "win32", mode: "default", gpuTiming: false },
        { dawnSafety: "off" },
      ),
    ).toEqual([
      { name: "enable-dawn-features", value: DAWN_SAFETY_OFF_ENABLED.join(",") },
      { name: "disable-dawn-features", value: DAWN_SAFETY_OFF_DISABLED.join(",") },
    ]);
  });

  it("add nothing in safe mode, where nothing draws with WebGPU", () => {
    for (const platform of PLATFORMS) {
      for (const dawnSafety of SAFETY) {
        expect(launchSwitches({ platform, mode: "safe", gpuTiming: true }, { dawnSafety })).toEqual(
          [],
        );
      }
    }
  });

  it("never contain the unsafe WebGPU flag or the adapter override", () => {
    for (const platform of PLATFORMS) {
      for (const mode of MODES) {
        for (const dawnSafety of SAFETY) {
          const names = launchSwitches({ platform, mode, gpuTiming: false }, { dawnSafety }).map(
            ({ name }) => name,
          );
          expect(names).not.toContain("enable-unsafe-webgpu");
          expect(names).not.toContain("use-webgpu-adapter");
        }
      }
    }
  });
});

/** A transport that records its calls in order; each stop tells a fifth of the buffer used. */
function fakeTracing(): SpikeTracing & {
  readonly start: Mock<SpikeTracing["start"]>;
  readonly stop: Mock<SpikeTracing["stop"]>;
  readonly close: Mock<SpikeTracing["close"]>;
  readonly calls: string[];
} {
  const calls: string[] = [];
  return {
    calls,
    start: vi.fn<SpikeTracing["start"]>(() => {
      calls.push("start");
      return Promise.resolve();
    }),
    stop: vi.fn<SpikeTracing["stop"]>((path) => {
      calls.push(`stop ${path}`);
      return Promise.resolve(FIFTH);
    }),
    close: vi.fn<SpikeTracing["close"]>(() => {
      calls.push("close");
    }),
  };
}

/** A stop's outcome: a fifth of the buffer used, no data lost. */
const FIFTH: TraceWindowStop = { lostData: false, bufferPercent: 20 };

/** A cycle's `stopped` that keeps what it is told. */
function keep(): {
  readonly stops: TraceWindowStop[];
  readonly stopped: (stop: TraceWindowStop) => void;
} {
  const stops: TraceWindowStop[] = [];
  return {
    stops,
    stopped: (stop) => {
      stops.push(stop);
    },
  };
}

describe("the spike's trace", () => {
  it("records a timed run's five categories, without gpu or the CPU profiler, and only those", () => {
    const config = spikeTraceConfig();
    expect(config.included_categories).toEqual([
      "devtools.timeline",
      "disabled-by-default-devtools.timeline",
      "disabled-by-default-devtools.timeline.frame",
      "disabled-by-default-v8.gc",
      "blink.user_timing",
    ]);
    expect(config.included_categories).toEqual([...SPIKE_TRACE_CATEGORIES]);
    expect(config.excluded_categories).toEqual(["*"]);
  });

  it("records a timed run's windows in a 768 MiB until-full buffer", () => {
    const config = spikeTraceConfig();
    expect(config.trace_buffer_size_in_kb).toBe(786_432);
    expect(config.recording_mode).toBe("record-until-full");
    expect(SPIKE_TRACE_BUFFER_KB).toBe(786_432);
  });

  it("adds gpu and the CPU profiler in a profiled run, with a 1.5 GiB until-full buffer", () => {
    const profiled = spikeTraceConfig({ profiled: true });
    expect(profiled.included_categories).toEqual([
      ...SPIKE_TRACE_CATEGORIES,
      "gpu",
      "disabled-by-default-v8.cpu_profiler",
    ]);
    expect([SPIKE_GPU_CATEGORY, SPIKE_PROFILER_CATEGORY]).toEqual([
      "gpu",
      "disabled-by-default-v8.cpu_profiler",
    ]);
    expect(profiled.trace_buffer_size_in_kb).toBe(1_572_864);
    expect(profiled.recording_mode).toBe("record-until-full");
    expect(SPIKE_PROFILED_TRACE_BUFFER_KB).toBe(1_572_864);
  });

  it("calls a run profiled when the CPU profiler's category is recorded", () => {
    expect(traceSettingsOf(spikeTraceConfig({ profiled: true }), "perfetto-proto")).toEqual({
      format: "perfetto-proto",
      profiled: true,
      categories: [...SPIKE_TRACE_CATEGORIES, SPIKE_GPU_CATEGORY, SPIKE_PROFILER_CATEGORY],
      recordingMode: "record-until-full",
      bufferKb: 1_572_864,
    });
    expect(traceSettingsOf({ included_categories: ["gpu"] }, "json")).toEqual({
      format: "json",
      profiled: false,
      categories: ["gpu"],
      recordingMode: "record-until-full",
      bufferKb: 0,
    });
  });

  it("exposes the settings it records every window with, as a protobuf stream", async () => {
    const tracing = fakeTracing();
    const trace = new SpikeTrace(tracing, { profiled: true });
    expect(SPIKE_TRACE_FORMAT).toBe("perfetto-proto");
    expect(trace.settings).toEqual(
      traceSettingsOf(spikeTraceConfig({ profiled: true }), "perfetto-proto"),
    );
    expect(new SpikeTrace(tracing).settings.profiled).toBe(false);
    expect(new SpikeTrace(tracing).settings.format).toBe("perfetto-proto");
    await trace.start();
    expect(tracing.start).toHaveBeenCalledWith(spikeTraceConfig({ profiled: true }));
  });

  it("starts once and stops into the path given, with what the stop told", async () => {
    const tracing = fakeTracing();
    const trace = new SpikeTrace(tracing);
    await trace.start();
    expect(trace.recording).toBe(true);
    await expect(trace.start()).rejects.toThrow("already recording");
    await expect(trace.stop("/runs/spike-trace-0.pftrace")).resolves.toEqual(FIFTH);
    expect(tracing.start).toHaveBeenCalledOnce();
    expect(tracing.start).toHaveBeenCalledWith(spikeTraceConfig());
    expect(tracing.stop).toHaveBeenCalledWith("/runs/spike-trace-0.pftrace");
    expect(trace.recording).toBe(false);
  });

  it("cycles by stopping to its path before it starts, and still refuses a start", async () => {
    const tracing = fakeTracing();
    const trace = new SpikeTrace(tracing);
    const { stops, stopped } = keep();
    await trace.start();
    await expect(trace.cycle("/p/spike-trace-0.pftrace", stopped)).resolves.toBeUndefined();
    expect(tracing.calls).toEqual(["start", "stop /p/spike-trace-0.pftrace", "start"]);
    expect(stops).toEqual([FIFTH]);
    expect(trace.recording).toBe(true);
    await expect(trace.start()).rejects.toThrow("already recording");
    await expect(trace.stop("/p/spike-trace-1.pftrace")).resolves.toEqual(FIFTH);
  });

  it("refuses any call while a cycle is in flight", async () => {
    const tracing = fakeTracing();
    const held: { release: (() => void) | null } = { release: null };
    const trace = new SpikeTrace(tracing);
    await trace.start();
    tracing.stop.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          held.release = () => {
            resolve(FIFTH);
          };
        }),
    );
    const cycling = trace.cycle("/p/a.pftrace", keep().stopped);
    expect(trace.state).toBe("busy");
    expect(trace.recording).toBe(false);
    await expect(trace.start()).rejects.toThrow("busy");
    await expect(trace.stop("/p/b.pftrace")).rejects.toThrow("busy");
    await expect(trace.cycle("/p/b.pftrace", keep().stopped)).rejects.toThrow("busy");
    held.release?.();
    await expect(cycling).resolves.toBeUndefined();
  });

  it("refuses a stop or a cycle with nothing recording", async () => {
    const trace = new SpikeTrace(fakeTracing());
    await expect(trace.stop("/runs/a.pftrace")).rejects.toThrow("not recording");
    await expect(trace.cycle("/runs/a.pftrace", keep().stopped)).rejects.toThrow("not recording");
  });

  it("ends stopped when a cycle's stop fails, telling no stop", async () => {
    const tracing = fakeTracing();
    const trace = new SpikeTrace(tracing);
    const { stops, stopped } = keep();
    await trace.start();
    tracing.stop.mockRejectedValueOnce(new Error("service gone"));
    await expect(trace.cycle("/p/a.pftrace", stopped)).rejects.toThrow(
      "the spike's trace did not stop: service gone",
    );
    expect(stops).toEqual([]);
    expect(trace.recording).toBe(false);
  });

  it("ends stopped when a cycle's start fails, its stop told first", async () => {
    const tracing = fakeTracing();
    const trace = new SpikeTrace(tracing);
    const { stops, stopped } = keep();
    await trace.start();
    tracing.start.mockRejectedValueOnce(new Error("service gone"));
    await expect(trace.cycle("/p/b.pftrace", stopped)).rejects.toThrow(
      "the spike's trace did not start: service gone",
    );
    expect(stops).toEqual([FIFTH]);
    expect(trace.recording).toBe(false);
  });

  it("ends stopped when a cycle's stopped callback throws", async () => {
    const tracing = fakeTracing();
    const trace = new SpikeTrace(tracing);
    await trace.start();
    await expect(
      trace.cycle("/p/a.pftrace", () => {
        throw new Error("not kept");
      }),
    ).rejects.toThrow("not kept");
    expect(trace.state).toBe("idle");
  });

  it("ends stopped when its stop fails, with the transport's error as the cause", async () => {
    const tracing = fakeTracing();
    const trace = new SpikeTrace(tracing);
    await trace.start();
    const gone = new Error("service gone");
    tracing.stop.mockRejectedValueOnce(gone);
    await expect(trace.stop("/p/a.pftrace")).rejects.toMatchObject({
      message: "the spike's trace did not stop: service gone",
      cause: gone,
    });
    expect(trace.state).toBe("idle");
  });

  it("can start again after a start that failed", async () => {
    const tracing = fakeTracing();
    tracing.start.mockRejectedValueOnce(new Error("tracing service gone"));
    const trace = new SpikeTrace(tracing);
    await expect(trace.start()).rejects.toThrow("did not start: tracing service gone");
    expect(trace.recording).toBe(false);
    await trace.start();
    expect(trace.recording).toBe(true);
  });

  it("closes its transport when asked", () => {
    const tracing = fakeTracing();
    new SpikeTrace(tracing).close();
    expect(tracing.calls).toEqual(["close"]);
  });
});

describe("the spike's IPC handlers", () => {
  /** A fake IPC event: whether it comes from the spike's page. */
  interface FakeEvent {
    readonly own: boolean;
  }
  type Listener = (event: FakeEvent, ...args: unknown[]) => Promise<unknown>;
  const OWN: FakeEvent = { own: true };
  const FOREIGN: FakeEvent = { own: false };

  function setUp(): {
    readonly call: (channel: string, event: FakeEvent, ...args: unknown[]) => Promise<unknown>;
    readonly deps: SpikeHandlerDeps<FakeEvent>;
    readonly channels: ReadonlyArray<string>;
  } {
    const listeners = new Map<string, Listener>();
    const deps: SpikeHandlerDeps<FakeEvent> = {
      handle: (channel, listener) => {
        listeners.set(channel, listener);
      },
      isSender: (event) => event.own,
      startMeasuring: vi.fn<() => Promise<void>>(() => Promise.resolve()),
      stopMeasuring: vi.fn<() => Promise<void>>(() => Promise.resolve()),
      cycleTrace: vi.fn<() => Promise<void>>(() => Promise.resolve()),
      rendererMemory: vi.fn<(bytes: number) => void>(),
      writeResults: vi.fn<SpikeHandlerDeps<FakeEvent>["writeResults"]>(() =>
        Promise.resolve({ kind: "written", paths: { json: "a.json", markdown: "a.md" } }),
      ),
      writeCapture: vi.fn<SpikeHandlerDeps<FakeEvent>["writeCapture"]>(() =>
        Promise.resolve("/capture"),
      ),
      end: vi.fn<(code: number, reason: string | null) => void>(),
    };
    registerSpikeHandlers(deps);
    return {
      deps,
      channels: [...listeners.keys()],
      call: (channel, event, ...args) => {
        const listener = listeners.get(channel);
        if (listener === undefined) {
          throw new Error(`no handler on ${channel}`);
        }
        return listener(event, ...args);
      },
    };
  }

  it("registers one handler a channel", () => {
    const { channels } = setUp();
    expect(channels.toSorted()).toEqual(Object.values(SPIKE_CHANNELS).toSorted());
  });

  it.each(Object.values(SPIKE_CHANNELS))("refuses %s from a foreign sender", async (channel) => {
    const { call, deps } = setUp();
    await expect(call(channel, FOREIGN, smallReport())).rejects.toThrow(/not the spike window/);
    expect(deps.startMeasuring).not.toHaveBeenCalled();
    expect(deps.cycleTrace).not.toHaveBeenCalled();
    expect(deps.writeResults).not.toHaveBeenCalled();
    expect(deps.end).not.toHaveBeenCalled();
  });

  it("refuses arguments that do not check", async () => {
    const { call, deps } = setUp();
    await expect(call(SPIKE_CHANNELS.memory, OWN, -1)).rejects.toThrow(/byte count/);
    await expect(call(SPIKE_CHANNELS.writeResults, OWN, { frames: 1 })).rejects.toThrow(/report/);
    await expect(call(SPIKE_CHANNELS.writeCapture, OWN, { json: 1 })).rejects.toThrow(/capture/);
    await expect(call(SPIKE_CHANNELS.end, OWN, { status: "fail" })).rejects.toThrow(/end/);
    expect(deps.rendererMemory).not.toHaveBeenCalled();
    expect(deps.writeResults).not.toHaveBeenCalled();
    expect(deps.end).not.toHaveBeenCalled();
  });

  it("does each operation for the spike's own page", async () => {
    const { call, deps } = setUp();
    await call(SPIKE_CHANNELS.startTrace, OWN);
    await call(SPIKE_CHANNELS.cycleTrace, OWN);
    await call(SPIKE_CHANNELS.memory, OWN, 4096);
    await expect(call(SPIKE_CHANNELS.writeResults, OWN, smallReport())).resolves.toEqual({
      kind: "written",
      paths: { json: "a.json", markdown: "a.md" },
    });
    await call(SPIKE_CHANNELS.stopTrace, OWN);
    await call(SPIKE_CHANNELS.end, OWN, { status: "fail", reason: "no bake" });
    expect(deps.startMeasuring).toHaveBeenCalledOnce();
    expect(deps.cycleTrace).toHaveBeenCalledOnce();
    expect(deps.stopMeasuring).toHaveBeenCalledOnce();
    expect(deps.rendererMemory).toHaveBeenCalledWith(4096);
    expect(deps.writeResults).toHaveBeenCalledWith(smallReport());
    expect(deps.end).toHaveBeenCalledWith(1, "no bake");
  });
});
