/**
 * The descent spike's main-process side (plan R05, T14.b, T13.c, T14.e and T14.i): its measurement
 * switches, its trace in windows and its IPC handlers.
 *
 * @remarks
 * Nothing here runs on an ordinary launch: the switches are added only when the spike flag is given
 * (T13.c), so that a shipped binary never lifts timestamp quantization or turns off Dawn's safety
 * checks by default (the plan's Risks, "Measurement switches in a shipped binary"). The switches are
 * R01's {@link ChromiumSwitch} entries, applied by `applyGraphicsSwitches` before `ready`, which
 * merges each list switch into the value already on the command line.
 */

import type { IpcMainInvokeEvent, TraceConfig } from "electron";

import type {
  DescentSpikeReport,
  SpikeEnd,
  SpikeLaunch,
  SpikeReportPart,
  SpikeResultsAnswer,
} from "../preload/api";

import { SOFTWARE_WAIT_FACTOR } from "./graphics/softwareMode";
import {
  type ChromiumSwitch,
  type GraphicsLaunchMode,
  type GraphicsLaunchOptions,
  graphicsSwitches,
  LIST_SWITCHES,
  mergeSwitchValue,
} from "./graphics/switches";
import {
  type RejectedSizes,
  readSpikeCapture,
  readSpikeReport,
  readSpikeReportPart,
  type SpikeCaptureFiles,
} from "./spikeReport";
import type { TraceFormat, TraceSettings } from "./traceWindows";

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
 * The renderer's hold of a generated world's field before its measurement, at most, ms (plan R10,
 * R10.T14.c; `bodyDescent.ts`' constant of the same name, which this project cannot import).
 */
export const FIELD_HOLD_TIMEOUT_MS = 120_000;

/**
 * The renderer's hold of a served world's field, at most, ms (R10.T15.b; `bodyDescent.ts`'
 * constant of the same name): the server computes the body's coarse field before its first answer.
 */
export const SERVED_FIELD_HOLD_TIMEOUT_MS = 600_000;

/**
 * The renderer's wait for one site's bakes in a generated world's measurement, at most, ms, an
 * attempt (R10.T14.c; `bodyDescent.ts`' constant of the same name).
 */
export const MEASURE_TIMEOUT_MS = 600_000;

/** The room past those waits for the site's reading and the session's own start, ms. */
export const MEASUREMENT_MARGIN_MS = 60_000;

/**
 * How long a spike run may take before the main process ends it with status 3: the smoke's 10 s
 * from its first worker bake, which it waits a minute of script for (the renderer's
 * `smokeBakeLimitS`), and the descent's 20.5 minutes, each with room for building the scene and
 * writing the results; and over a generated world (R10.T14.c) 13 minutes more, its measurement's
 * longest for one site: the field's hold at its 120 s limit, then the site's bakes at their 600 s
 * limit, and a minute's margin, against some 40 s in a typical run on three workers; over a served
 * world (R10.T15.b) 21 minutes more, its field's hold at its 600 s limit, since the server computes
 * the body's coarse field before its first answer (about a minute natively at level 8, R09.T16.a).
 *
 * @remarks
 * A site whose floors the profile refuses is measured afresh at the next candidate, each attempt's
 * bakes within their own 600 s. Past one attempt at its limit this watchdog is the backstop: it ends
 * the run, rather than the renderer's own timeouts. The spike's script (`descentSpike.sh`) keeps its
 * limit above this one, so that the watchdog ends a run first. In the check-only software mode
 * (plan R01, R01.T16; main's ruling for the spike, 2026-11-01) every part is
 * {@link SOFTWARE_WAIT_FACTOR} times as long, as the views check's is, and the script's limit with
 * it; a GPU launch's is unchanged.
 */
export function spikeWatchdogMs(
  launch: Pick<SpikeLaunch, "smoke" | "world">,
  mode: GraphicsLaunchMode,
): number {
  let measurementMs: number;
  switch (launch.world) {
    case "test-planet":
      measurementMs = 0;
      break;
    case "terrain-test":
      measurementMs = FIELD_HOLD_TIMEOUT_MS + MEASURE_TIMEOUT_MS + MEASUREMENT_MARGIN_MS;
      break;
    case "served":
      measurementMs = SERVED_FIELD_HOLD_TIMEOUT_MS + MEASURE_TIMEOUT_MS + MEASUREMENT_MARGIN_MS;
      break;
  }
  const ms = (launch.smoke ? 180_000 : 45 * 60_000) + measurementMs;
  return mode === "software" ? ms * SOFTWARE_WAIT_FACTOR : ms;
}

/**
 * Where a spike run's results go, from the repository's root unless `--out` names a directory:
 * `docs/measurements/descent-spike/`, or under `target/` in the software mode, whose figures are
 * SwiftShader's and never a GPU's, as a software views check's are (main's ruling, 2026-10-29: the
 * main process refuses `--out` with the switch).
 */
export function spikeResultsDir(out: string | null, mode: GraphicsLaunchMode): string {
  return out ?? (mode === "software" ? SOFTWARE_SPIKE_RESULTS_DIR : SPIKE_RESULTS_DIR);
}

/** A spike run's results directory, from the repository's root. */
export const SPIKE_RESULTS_DIR = "docs/measurements/descent-spike";

/** A software-mode spike run's results directory, from the repository's root. */
export const SOFTWARE_SPIKE_RESULTS_DIR = "target/descent-spike/results";

/**
 * The trace categories of every spike run (Design note 18): the timeline's frames, tasks and the
 * GPU process's tasks (`GPUTask`), V8's GC slices and the page's `performance.measure` spans.
 *
 * @remarks
 * `toplevel` is left out: it repeats the timeline's `RunTask` slices and was about half of a
 * recorded trace's bytes (2026-10-02, Electron 44.4.3, a small WebGPU page on SwiftShader). V8's
 * CPU profiler ({@link SPIKE_PROFILER_CATEGORY}) and {@link SPIKE_GPU_CATEGORY} are left out too,
 * and recorded only in a profiled run (decision-r05-trace-windows.md,
 * decision-r05-trace-windows-2.md).
 */
export const SPIKE_TRACE_CATEGORIES: ReadonlyArray<string> = [
  "devtools.timeline",
  "disabled-by-default-devtools.timeline",
  "disabled-by-default-devtools.timeline.frame",
  "disabled-by-default-v8.gc",
  "blink.user_timing",
];

/**
 * The GPU process's own trace category, recorded only in a profiled run (`--trace-profile on`).
 *
 * @remarks
 * It was 58.8 % of a JSON window's bytes and about 46 % of a protobuf one's (T14.f's measurements,
 * 2026-10-05), for two slice names, `WebGPU` and `VulkanQueueSubmitHook`, which split the GPU
 * process's busy time into Dawn's command execution and queue submission: a diagnostic's detail,
 * which no criterion reads. A timed run keeps the GPU process's busy time and its `GPUTask` slices
 * from the timeline (decision-r05-trace-windows-2.md, ruling 2).
 */
export const SPIKE_GPU_CATEGORY = "gpu";

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
 * Each trace window's buffer ceiling in a timed run, KiB: 768 MiB, with `record-until-full`, on
 * every machine (decision-r05-trace-windows-2.md, ruling 6).
 *
 * @remarks
 * A ceiling, committed only as written, not a reservation (decision-r05-trace-windows.md). The
 * tracing service crashed at the stop of a whole descent's JSON trace (1.35–1.71 GB), since
 * Chromium builds a JSON export whole in memory, 4.3–5.7 times the buffer's bytes; the protobuf
 * stream does not grow the service at the stop (T14.f's measurements). Each window's buffer holds
 * a fraction of a descent's trace: the last, the largest, is estimated at under half of it. The
 * buffer stops recording when full rather than dropping the start, so a window that filled shows
 * as a span short of its recorded time, or as lost data.
 */
export const SPIKE_TRACE_BUFFER_KB = 768 * 1024;

/**
 * Each trace window's buffer ceiling in a profiled run, KiB: 1.5 GiB, since `gpu` and the CPU
 * profiler about double the trace's rate; profiled runs are made on the RTX 3080 only
 * (decision-r05-trace-windows-2.md, ruling 6).
 */
export const SPIKE_PROFILED_TRACE_BUFFER_KB = 1536 * 1024;

/** What the trace's configuration depends on. */
export interface SpikeTraceOptions {
  /**
   * Whether {@link SPIKE_GPU_CATEGORY} and {@link SPIKE_PROFILER_CATEGORY} are recorded
   * (`--trace-profile on`).
   */
  readonly profiled: boolean;
}

/**
 * The format {@link SpikeTrace} writes each window in: a Perfetto protobuf stream over CDP
 * (`cdpTracing.ts`), decoded by the spike's own decoder (`traceProto.ts`).
 */
export const SPIKE_TRACE_FORMAT: TraceFormat = "perfetto-proto";

/** The tracing configuration of every window of a spike run. */
export function spikeTraceConfig(options: SpikeTraceOptions = { profiled: false }): TraceConfig {
  return {
    recording_mode: "record-until-full",
    trace_buffer_size_in_kb: options.profiled
      ? SPIKE_PROFILED_TRACE_BUFFER_KB
      : SPIKE_TRACE_BUFFER_KB,
    included_categories: [
      ...SPIKE_TRACE_CATEGORIES,
      ...(options.profiled ? [SPIKE_GPU_CATEGORY, SPIKE_PROFILER_CATEGORY] : []),
    ],
    excluded_categories: ["*"],
  };
}

/**
 * A run's trace settings, as its results file records them, from the configuration it records and
 * the format its windows are written in.
 */
export function traceSettingsOf(config: TraceConfig, format: TraceFormat): TraceSettings {
  const categories = config.included_categories ?? [];
  return {
    format,
    profiled: categories.includes(SPIKE_PROFILER_CATEGORY),
    categories: [...categories],
    // Chromium's default when none is given.
    recordingMode: config.recording_mode ?? "record-until-full",
    bufferKb: config.trace_buffer_size_in_kb ?? 0,
  };
}

/** What a window's stop tells besides its file. */
export interface TraceWindowStop {
  /** Whether Chromium lost some of the window's data (`Tracing.tracingComplete`'s flag). */
  readonly lostData: boolean;
  /**
   * The fullest buffer's last reported use before the stop, % (CDP's
   * `Tracing.bufferUsage.percentFull`, a fraction, × 100), or `null` when none was reported.
   */
  readonly bufferPercent: number | null;
}

/** The transport the spike's trace records its windows through: `CdpTracing`. */
export interface SpikeTracing {
  /** Starts a window's recording with `config`; the first start opens the transport. */
  start(config: TraceConfig): Promise<void>;
  /** Ends the window's recording and writes its trace to `path`. */
  stop(path: string): Promise<TraceWindowStop>;
  /** Ends the transport after the last stop; every later start is refused. */
  close(): void;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Where the trace is: stopped, recording, or between the two in a start, stop or cycle. */
export type SpikeTraceState = "idle" | "recording" | "busy";

/**
 * The spike's trace over the descent, in windows, through a {@link SpikeTracing} transport.
 *
 * @remarks
 * One recording at a time, and one operation at a time: a start while recording, a stop or cycle
 * with none running, and any call while another is in flight are refused, since Chromium keeps one
 * tracing session for the whole browser. Every window is recorded with the same configuration,
 * {@link SpikeTrace.settings}.
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
    this.settings = traceSettingsOf(this.#config, SPIKE_TRACE_FORMAT);
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
   * @param path - Where the window's trace is written, a Perfetto protobuf stream.
   * @returns What the stop tells besides the file.
   * @throws Error if no recording is running or an operation is in flight; or, with the trace
   * stopped, if the transport's stop fails, its message given.
   */
  async stop(path: string): Promise<TraceWindowStop> {
    this.#refuseUnless("recording");
    this.#state = "busy";
    try {
      return await this.#stopRecording(path);
    } finally {
      this.#state = "idle";
    }
  }

  /**
   * Ends one window and begins the next: stops to `path`, then starts again.
   *
   * @param stopped - Called with the stop's outcome once the window is written, before the start,
   * so that a window whose next start fails is still known.
   * @throws Error if no recording is running or an operation is in flight; or, with the trace
   * stopped, if the stop or the start fails, its message given.
   */
  async cycle(path: string, stopped: (stop: TraceWindowStop) => void): Promise<void> {
    this.#refuseUnless("recording");
    this.#state = "busy";
    try {
      stopped(await this.#stopRecording(path));
    } catch (error: unknown) {
      this.#state = "idle";
      throw error;
    }
    await this.#startRecording();
  }

  /** Ends the transport after the last stop (`SpikeTracing.close`). */
  close(): void {
    this.#tracing.close();
  }

  /** Stops the transport's recording to `path`. */
  async #stopRecording(path: string): Promise<TraceWindowStop> {
    try {
      return await this.#tracing.stop(path);
    } catch (error: unknown) {
      throw new Error(`the spike's trace did not stop: ${messageOf(error)}`, { cause: error });
    }
  }

  /** Starts the transport's recording from the busy state, leaving it recording or idle. */
  async #startRecording(): Promise<void> {
    try {
      await this.#tracing.start(this.#config);
    } catch (error: unknown) {
      this.#state = "idle";
      throw new Error(`the spike's trace did not start: ${messageOf(error)}`, { cause: error });
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
  writePart: "hyperion:spike:write-part",
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
  /**
   * Builds and writes the results file from a checked report, or, for a smoke, checks the trace's
   * windows against it and writes nothing; with `rejectedSizes`, the report's `sizes` block that
   * its check refused (R10.T14.b), left out of `report`, whose problems are logged and which is
   * written beside the results.
   */
  readonly writeResults: (
    report: DescentSpikeReport,
    rejectedSizes?: RejectedSizes,
  ) => Promise<SpikeResultsAnswer>;
  /**
   * Takes a checked part of the report as the run flies, from which the finished segments are
   * written (R14's Mac measurements).
   */
  readonly writePart: (part: SpikeReportPart) => Promise<void>;
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
    const reading = readSpikeReport(value);
    if (reading === null) {
      throw new SpikeCallRefused(`${SPIKE_CHANNELS.writeResults} refused: not a spike report`);
    }
    // A malformed `sizes` block (R10.T14.b) alone loses the run nothing: the results go without it,
    // and it goes beside them with its problems.
    const { report, rejectedSizes } = reading;
    return rejectedSizes === null
      ? deps.writeResults(report)
      : deps.writeResults(report, rejectedSizes);
  });
  guarded(SPIKE_CHANNELS.writePart, async ([value]) => {
    const part = readSpikeReportPart(value);
    if (part === null) {
      throw new SpikeCallRefused(`${SPIKE_CHANNELS.writePart} refused: not a part of a report`);
    }
    await deps.writePart(part);
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
