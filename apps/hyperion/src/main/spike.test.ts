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
  SPIKE_TRACE_CATEGORIES,
  SpikeTrace,
  type SpikeTracing,
  spikeTraceConfig,
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

/** A `contentTracing` that records its calls. */
function fakeTracing(): SpikeTracing & {
  readonly startRecording: Mock<SpikeTracing["startRecording"]>;
  readonly stopRecording: Mock<SpikeTracing["stopRecording"]>;
} {
  return {
    startRecording: vi.fn<SpikeTracing["startRecording"]>(() => Promise.resolve()),
    stopRecording: vi.fn<SpikeTracing["stopRecording"]>((path) => Promise.resolve(path ?? "")),
  };
}

describe("the spike's trace", () => {
  it("records Design note 18's categories, and only those", () => {
    const config = spikeTraceConfig();
    expect(config.included_categories).toEqual([...SPIKE_TRACE_CATEGORIES]);
    expect(config.excluded_categories).toEqual(["*"]);
    expect(config.recording_mode).toBe("record-until-full");
    for (const category of [
      "devtools.timeline",
      "disabled-by-default-v8.gc",
      "disabled-by-default-v8.cpu_profiler",
      "blink.user_timing",
      "gpu",
    ]) {
      expect(SPIKE_TRACE_CATEGORIES).toContain(category);
    }
  });

  it("starts once and stops into the path given", async () => {
    const tracing = fakeTracing();
    const trace = new SpikeTrace(tracing);
    await trace.start();
    expect(trace.recording).toBe(true);
    await expect(trace.start()).rejects.toThrow("already recording");
    await expect(trace.stop("/runs/descent.trace.json")).resolves.toBe("/runs/descent.trace.json");
    expect(tracing.startRecording).toHaveBeenCalledOnce();
    expect(tracing.startRecording).toHaveBeenCalledWith(spikeTraceConfig());
    expect(trace.recording).toBe(false);
  });

  it("refuses a stop with nothing recording", async () => {
    const trace = new SpikeTrace(fakeTracing());
    await expect(trace.stop("/runs/descent.trace.json")).rejects.toThrow("not recording");
  });

  it("can start again after a start that failed", async () => {
    const tracing = fakeTracing();
    tracing.startRecording.mockRejectedValueOnce(new Error("tracing service gone"));
    const trace = new SpikeTrace(tracing);
    await expect(trace.start()).rejects.toThrow("did not start");
    expect(trace.recording).toBe(false);
    await trace.start();
    expect(trace.recording).toBe(true);
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
      rendererMemory: vi.fn<(bytes: number) => void>(),
      writeResults: vi.fn<SpikeHandlerDeps<FakeEvent>["writeResults"]>(() =>
        Promise.resolve({ json: "a.json", markdown: "a.md" }),
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
    await call(SPIKE_CHANNELS.memory, OWN, 4096);
    await expect(call(SPIKE_CHANNELS.writeResults, OWN, smallReport())).resolves.toEqual({
      json: "a.json",
      markdown: "a.md",
    });
    await call(SPIKE_CHANNELS.stopTrace, OWN);
    await call(SPIKE_CHANNELS.end, OWN, { status: "fail", reason: "no bake" });
    expect(deps.startMeasuring).toHaveBeenCalledOnce();
    expect(deps.stopMeasuring).toHaveBeenCalledOnce();
    expect(deps.rendererMemory).toHaveBeenCalledWith(4096);
    expect(deps.writeResults).toHaveBeenCalledWith(smallReport());
    expect(deps.end).toHaveBeenCalledWith(1, "no bake");
  });
});
