/**
 * The descent spike's main-process side (plan R05, T14.b and T13.c): its measurement switches, its
 * trace and its IPC handlers.
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
 * The trace categories of a spike run (Design note 18): the timeline's frames, tasks and GPU tasks,
 * V8's GC slices and CPU profile, the page's `performance.measure` spans and the GPU process's
 * WebGPU work.
 *
 * @remarks
 * `toplevel` is left out: it repeats the timeline's `RunTask` slices and was about half of a
 * recorded trace's bytes (2026-10-02, Electron 44.4.3, a small WebGPU page on SwiftShader).
 */
export const SPIKE_TRACE_CATEGORIES: ReadonlyArray<string> = [
  "devtools.timeline",
  "disabled-by-default-devtools.timeline",
  "disabled-by-default-devtools.timeline.frame",
  "disabled-by-default-v8.gc",
  "disabled-by-default-v8.cpu_profiler",
  "blink.user_timing",
  "gpu",
];

/**
 * The trace buffer's ceiling, 2 GiB. A trace of these categories grew at about 1.1 MB a second on a
 * small WebGPU page (2026-10-02), about 1.4 GB over the descent's 21 minutes (the sum of Design note 19's durations); the buffer stops
 * recording when full rather than dropping the start, and the reducer reports the span it saw, so
 * a truncated trace shows as a short span.
 */
export const SPIKE_TRACE_BUFFER_KB = 2 * 1024 * 1024;

/** The tracing configuration of a spike run. */
export function spikeTraceConfig(): TraceConfig {
  return {
    recording_mode: "record-until-full",
    trace_buffer_size_in_kb: SPIKE_TRACE_BUFFER_KB,
    included_categories: [...SPIKE_TRACE_CATEGORIES],
    excluded_categories: ["*"],
  };
}

/** The part of Electron's `contentTracing` the spike's trace uses. */
export type SpikeTracing = Pick<ContentTracing, "startRecording" | "stopRecording">;

/**
 * The spike's one trace over the descent, through Electron's `contentTracing`.
 *
 * @remarks
 * One recording at a time: a second start, or a stop with none running, is refused, since Chromium
 * keeps one tracing session for the whole browser.
 */
export class SpikeTrace {
  readonly #tracing: SpikeTracing;
  #recording = false;

  constructor(tracing: SpikeTracing) {
    this.#tracing = tracing;
  }

  /** Whether a recording is running. */
  get recording(): boolean {
    return this.#recording;
  }

  /**
   * Starts recording.
   *
   * @throws Error if a recording is already running.
   */
  async start(): Promise<void> {
    if (this.#recording) {
      throw new Error("the spike's trace is already recording");
    }
    this.#recording = true;
    try {
      await this.#tracing.startRecording(spikeTraceConfig());
    } catch (error: unknown) {
      this.#recording = false;
      throw new Error("the spike's trace did not start", { cause: error });
    }
  }

  /**
   * Stops recording and writes the trace.
   *
   * @param path - Where Chromium writes the trace, as JSON with one event a line.
   * @returns The path written.
   * @throws Error if no recording is running.
   */
  async stop(path: string): Promise<string> {
    if (!this.#recording) {
      throw new Error("the spike's trace is not recording");
    }
    this.#recording = false;
    return this.#tracing.stopRecording(path);
  }
}

/** The spike's IPC channels, one per operation (T13.c); no channel name crosses the bridge. */
export const SPIKE_CHANNELS = {
  startTrace: "hyperion:spike:start-trace",
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
