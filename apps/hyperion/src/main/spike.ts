/**
 * The descent spike's main-process side (plan R05, T14.b, T13.c and T14.e): its measurement
 * switches, its trace in windows and its IPC handlers.
 *
 * @remarks
 * Nothing here runs on an ordinary launch: the switches are added only when the spike flag is given
 * (T13.c), so that a shipped binary never lifts timestamp quantization or turns off Dawn's safety
 * checks by default (the plan's Risks, "Measurement switches in a shipped binary"). The switches are
 * R01's {@link ChromiumSwitch} entries, applied by `applyGraphicsSwitches` before `ready`, which
 * merges each list switch into the value already on the command line.
 */

import type { ContentTracing, IpcMainInvokeEvent, TraceConfig } from "electron";

import type { DescentSpikeReport, SpikeEnd, SpikeResultsPaths } from "../preload/api";

import {
  type ChromiumSwitch,
  type GraphicsLaunchOptions,
  graphicsSwitches,
  LIST_SWITCHES,
  mergeSwitchValue,
} from "./graphics/switches";
import { readDescentSpikeReport, readSpikeCapture, type SpikeCaptureFiles } from "./spikeReport";
import type { TraceSettings } from "./traceWindows";

/** Whether Dawn's safety checks stay on for a spike run (`--dawn-safety`, T13.c). */
export type DawnSafety = "on" | "off";

/** What the spike's measurement switches depend on, beyond the launch's own graphics options. */
export interface SpikeMeasurementOptions {
  readonly dawnSafety: DawnSafety;
}

/**
 * The Dawn toggles a safety-off run enables (R05 Design note 22, step 1): robustness, validation,
 * workgroup-memory initialisation, the lazy clear of buffers mapped at creation and the integer
 * division polyfills.
 */
export const DAWN_SAFETY_OFF_ENABLED: ReadonlyArray<string> = [
  "disable_robustness",
  "skip_validation",
  "disable_workgroup_init",
  "disable_lazy_clear_for_mapped_at_creation_buffer",
  "disable_polyfills_on_integer_div_and_mod",
];

/** The Dawn toggle a safety-off run disables: the lazy clear of every resource on first use. */
export const DAWN_SAFETY_OFF_DISABLED: ReadonlyArray<string> = ["lazy_clear_resource_on_first_use"];

/**
 * `switches` with every list switch given once, its values merged in order of first appearance, at
 * the position of its first copy.
 */
function consolidate(switches: ReadonlyArray<ChromiumSwitch>): ReadonlyArray<ChromiumSwitch> {
  const merged = new Map<string, string>();
  const order: Array<ChromiumSwitch | string> = [];
  for (const entry of switches) {
    if (entry.value === undefined || !LIST_SWITCHES.has(entry.name)) {
      order.push(entry);
      continue;
    }
    const existing = merged.get(entry.name);
    if (existing === undefined) {
      order.push(entry.name);
    }
    merged.set(entry.name, mergeSwitchValue(existing ?? "", entry.value.split(",")));
  }
  return order.map((entry) =>
    typeof entry === "string" ? { name: entry, value: merged.get(entry) ?? "" } : entry,
  );
}

/**
 * The switches for a launch, with the spike's measurement switches when the spike flag is given.
 *
 * @param spike - The spike's options, or `undefined` for an ordinary launch, which gets exactly
 * R01's {@link graphicsSwitches}.
 * @returns For a spike run, R01's switches with `gpuTiming` on, as `--hyperion-gpu-timing` sets it
 * (R01 applies it only in its Linux `vulkan` mode), and, when `dawnSafety` is `off` and the mode
 * draws with WebGPU at all (not `safe`), Design note 22's safety toggles merged into one
 * `--enable-dawn-features` and one `--disable-dawn-features`. Never `--enable-unsafe-webgpu`.
 */
export function launchSwitches(
  options: GraphicsLaunchOptions,
  spike: SpikeMeasurementOptions | undefined,
): ReadonlyArray<ChromiumSwitch> {
  if (spike === undefined) {
    return graphicsSwitches(options);
  }
  const base = graphicsSwitches({ ...options, gpuTiming: true });
  if (spike.dawnSafety === "on" || options.mode === "safe") {
    return base;
  }
  return consolidate([
    ...base,
    { name: "enable-dawn-features", value: DAWN_SAFETY_OFF_ENABLED.join(",") },
    { name: "disable-dawn-features", value: DAWN_SAFETY_OFF_DISABLED.join(",") },
  ]);
}

/**
 * The trace categories of every spike run (Design note 18): the timeline's frames, tasks and GPU
 * tasks, V8's GC slices, the page's `performance.measure` spans and the GPU process's WebGPU work.
 *
 * @remarks
 * `toplevel` is left out: it repeats the timeline's `RunTask` slices and was about half of a
 * recorded trace's bytes (2026-10-02, Electron 44.4.3, a small WebGPU page on SwiftShader). V8's
 * CPU profiler ({@link SPIKE_PROFILER_CATEGORY}) is left out too, and recorded only in a profiled
 * run (decision-r05-trace-windows.md).
 */
export const SPIKE_TRACE_CATEGORIES: ReadonlyArray<string> = [
  "devtools.timeline",
  "disabled-by-default-devtools.timeline",
  "disabled-by-default-devtools.timeline.frame",
  "disabled-by-default-v8.gc",
  "blink.user_timing",
  "gpu",
];

/**
 * V8's CPU profiler's trace category, recorded only in a profiled run (`--trace-profile on`), a
 * diagnostic that is never judged.
 *
 * @remarks
 * V8 samples each isolate every 100 µs and keeps every sample until the trace stops, about
 * 0.31 MB/s an isolate, so in a timed run it would inflate the renderer's memory in proportion to
 * the worker count and interrupt every isolate 10,000 times a second
 * (decision-r05-trace-windows.md). It gives the engine adapter's share of the main thread.
 */
export const SPIKE_PROFILER_CATEGORY = "disabled-by-default-v8.cpu_profiler";

/**
 * Each trace window's buffer ceiling, KiB: 768 MiB, with `record-until-full`.
 *
 * @remarks
 * A ceiling, not a reservation (decision-r05-trace-windows.md). Without the profiler a hidden low
 * run's trace grew at about 1.15 MB/s, so a 120 s window holds about 138 MB and the busy 240 s one
 * about 276 MB, a third of the buffer. The tracing service crashed at the stop of a whole
 * descent's trace (1.35–1.71 GB), its working set doubling while it read the buffer out; even a
 * full 768 MiB buffer's stop (about 1.75 GB) stays under the 2.55–2.76 GB at which those services
 * were still alive. The buffer stops recording when full rather than dropping the start, so a
 * window that filled shows as a span short of its recorded time.
 */
export const SPIKE_TRACE_BUFFER_KB = 768 * 1024;

/** What the trace's configuration depends on. */
export interface SpikeTraceOptions {
  /** Whether {@link SPIKE_PROFILER_CATEGORY} is recorded (`--trace-profile on`). */
  readonly profiled: boolean;
}

/** The tracing configuration of every window of a spike run. */
export function spikeTraceConfig(options: SpikeTraceOptions = { profiled: false }): TraceConfig {
  return {
    recording_mode: "record-until-full",
    trace_buffer_size_in_kb: SPIKE_TRACE_BUFFER_KB,
    included_categories: [
      ...SPIKE_TRACE_CATEGORIES,
      ...(options.profiled ? [SPIKE_PROFILER_CATEGORY] : []),
    ],
    excluded_categories: ["*"],
  };
}

/** A run's trace settings, as its results file records them, from the configuration it records. */
export function traceSettingsOf(config: TraceConfig): TraceSettings {
  const categories = config.included_categories ?? [];
  return {
    profiled: categories.includes(SPIKE_PROFILER_CATEGORY),
    categories: [...categories],
    // Chromium's default when none is given.
    recordingMode: config.recording_mode ?? "record-until-full",
    bufferKb: config.trace_buffer_size_in_kb ?? 0,
  };
}

/** The part of Electron's `contentTracing` the spike's trace uses. */
export type SpikeTracing = Pick<
  ContentTracing,
  "startRecording" | "stopRecording" | "getTraceBufferUsage"
>;

/** Where the trace is: stopped, recording, or between the two in a start, stop or cycle. */
export type SpikeTraceState = "idle" | "recording" | "busy";

/**
 * The spike's trace over the descent, in windows, through Electron's `contentTracing`.
 *
 * @remarks
 * One recording at a time, and one operation at a time: a start while recording, a stop or cycle
 * with none running, and any call while another is in flight are refused, since Chromium keeps one
 * tracing session for the whole browser and resolves a second start at once without starting it.
 * Every window is recorded with the same configuration, {@link SpikeTrace.settings}.
 */
export class SpikeTrace {
  readonly #tracing: SpikeTracing;
  readonly #config: TraceConfig;
  /** How every window is recorded, for the results file. */
  readonly settings: TraceSettings;
  #state: SpikeTraceState = "idle";

  constructor(tracing: SpikeTracing, options: SpikeTraceOptions = { profiled: false }) {
    this.#tracing = tracing;
    this.#config = spikeTraceConfig(options);
    this.settings = traceSettingsOf(this.#config);
  }

  /** Where the trace is: `busy` while a start, stop or cycle is in flight. */
  get state(): SpikeTraceState {
    return this.#state;
  }

  /** Whether a recording is running and no operation is in flight. */
  get recording(): boolean {
    return this.#state === "recording";
  }

  /**
   * Starts recording.
   *
   * @throws Error if a recording is already running or an operation is in flight, or if Chromium
   * refuses the start.
   */
  async start(): Promise<void> {
    this.#refuseUnless("idle");
    this.#state = "busy";
    await this.#startRecording();
  }

  /**
   * Stops recording and writes the window's trace.
   *
   * @param path - Where Chromium writes the trace, as JSON with one event a line.
   * @returns The path written.
   * @throws Error if no recording is running or an operation is in flight; or, with the trace
   * stopped, if Chromium's stop fails.
   */
  async stop(path: string): Promise<string> {
    this.#refuseUnless("recording");
    this.#state = "busy";
    try {
      return await this.#tracing.stopRecording(path);
    } catch (error: unknown) {
      throw new Error("the spike's trace did not stop", { cause: error });
    } finally {
      this.#state = "idle";
    }
  }

  /**
   * Ends one window and begins the next: stops to `path`, then starts again.
   *
   * @returns The path written.
   * @throws Error if no recording is running or an operation is in flight; or, with the trace
   * stopped, if the stop or the start fails.
   */
  async cycle(path: string): Promise<string> {
    this.#refuseUnless("recording");
    this.#state = "busy";
    let written: string;
    try {
      written = await this.#tracing.stopRecording(path);
    } catch (error: unknown) {
      this.#state = "idle";
      throw new Error("the spike's trace did not stop", { cause: error });
    }
    await this.#startRecording();
    return written;
  }

  /**
   * The fullest process's share of its trace buffer, %, read before a stop; `null` when nothing
   * is recording or Electron reports nothing.
   *
   * @remarks
   * Electron's `percentage` is Chromium's `percent_full`, a fraction from 0 to 1 (CDP's
   * `Tracing.bufferUsage.percentFull`), so it is scaled to a percentage here.
   */
  async bufferUsage(): Promise<number | null> {
    if (this.#state !== "recording") {
      return null;
    }
    try {
      const { percentage } = await this.#tracing.getTraceBufferUsage();
      return Number.isFinite(percentage) && percentage >= 0 ? percentage * 100 : null;
    } catch {
      // A reading is optional: a window without one is still checked by its span.
      return null;
    }
  }

  /** Starts Chromium's recording from the busy state, leaving it recording or idle. */
  async #startRecording(): Promise<void> {
    try {
      await this.#tracing.startRecording(this.#config);
    } catch (error: unknown) {
      this.#state = "idle";
      throw new Error("the spike's trace did not start", { cause: error });
    }
    this.#state = "recording";
  }

  #refuseUnless(state: SpikeTraceState): void {
    if (this.#state === state) {
      return;
    }
    switch (this.#state) {
      case "busy":
        throw new Error("the spike's trace is busy with another start or stop");
      case "recording":
        throw new Error("the spike's trace is already recording");
      case "idle":
        throw new Error("the spike's trace is not recording");
    }
  }
}

/** The spike's IPC channels, one per operation (T13.c); no channel name crosses the bridge. */
export const SPIKE_CHANNELS = {
  startTrace: "hyperion:spike:start-trace",
  cycleTrace: "hyperion:spike:cycle-trace",
  stopTrace: "hyperion:spike:stop-trace",
  memory: "hyperion:spike:memory",
  writeResults: "hyperion:spike:write-results",
  writeCapture: "hyperion:spike:write-capture",
  end: "hyperion:spike:end",
} as const;

/**
 * What the spike's handlers do, injected so that a test drives them without Electron; `E` is the
 * IPC event (`IpcMainInvokeEvent`).
 */
export interface SpikeHandlerDeps<E = IpcMainInvokeEvent> {
  /** Registers a handler (`ipcMain.handle`). */
  readonly handle: (
    channel: string,
    listener: (event: E, ...args: unknown[]) => Promise<unknown>,
  ) => void;
  /** Whether an event comes from the spike window's own page (`isOwnPage`). */
  readonly isSender: (event: E) => boolean;
  /** Starts and stops the trace and the 1 Hz memory sampler together. */
  readonly startMeasuring: () => Promise<void>;
  readonly stopMeasuring: () => Promise<void>;
  /** Ends the trace's window and begins the next (T14.e). */
  readonly cycleTrace: () => Promise<void>;
  /** Keeps the renderer's private bytes for the sampler's next sample. */
  readonly rendererMemory: (bytes: number) => void;
  /** Builds and writes the results file from a checked report. */
  readonly writeResults: (report: DescentSpikeReport) => Promise<SpikeResultsPaths>;
  /** Writes a checked capture, returning its directory. */
  readonly writeCapture: (capture: SpikeCaptureFiles) => Promise<string>;
  /** Ends the run with an exit status, and a reason for a failure. */
  readonly end: (code: number, reason: string | null) => void;
}

/** Thrown back to the renderer for a call the main process refuses. */
export class SpikeCallRefused extends Error {}

/** `value` as an end of the run, or `null`. */
export function readSpikeEnd(value: unknown): SpikeEnd | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const status: unknown = Reflect.get(value, "status");
  const reason: unknown = Reflect.get(value, "reason");
  if (status === "pass") {
    return { status };
  }
  return status === "fail" && typeof reason === "string" ? { status, reason } : null;
}

/**
 * Registers the spike's handlers (T13.c, Design note 18), each refusing a sender other than the
 * spike window's own page and arguments that do not check, by rejecting the call.
 */
export function registerSpikeHandlers<E>(deps: SpikeHandlerDeps<E>): void {
  const guarded = (
    channel: string,
    run: (args: ReadonlyArray<unknown>) => Promise<unknown>,
  ): void => {
    deps.handle(channel, async (event, ...args) => {
      if (!deps.isSender(event)) {
        throw new SpikeCallRefused(`${channel} refused: not the spike window's own page`);
      }
      return run(args);
    });
  };
  guarded(SPIKE_CHANNELS.startTrace, () => deps.startMeasuring());
  guarded(SPIKE_CHANNELS.cycleTrace, () => deps.cycleTrace());
  guarded(SPIKE_CHANNELS.stopTrace, () => deps.stopMeasuring());
  guarded(SPIKE_CHANNELS.memory, async ([bytes]) => {
    if (typeof bytes !== "number" || !Number.isInteger(bytes) || bytes < 0) {
      throw new SpikeCallRefused(`${SPIKE_CHANNELS.memory} refused: not a byte count`);
    }
    deps.rendererMemory(bytes);
  });
  guarded(SPIKE_CHANNELS.writeResults, async ([value]) => {
    const report = readDescentSpikeReport(value);
    if (report === null) {
      throw new SpikeCallRefused(`${SPIKE_CHANNELS.writeResults} refused: not a spike report`);
    }
    return deps.writeResults(report);
  });
  guarded(SPIKE_CHANNELS.writeCapture, async ([value]) => {
    const capture = readSpikeCapture(value);
    if (capture === null) {
      throw new SpikeCallRefused(`${SPIKE_CHANNELS.writeCapture} refused: not a capture`);
    }
    return deps.writeCapture(capture);
  });
  guarded(SPIKE_CHANNELS.end, async ([value]) => {
    const end = readSpikeEnd(value);
    if (end === null) {
      throw new SpikeCallRefused(`${SPIKE_CHANNELS.end} refused: not an end of the run`);
    }
    deps.end(end.status === "pass" ? 0 : 1, end.status === "pass" ? null : end.reason);
  });
}
