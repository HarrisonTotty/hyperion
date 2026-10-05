/**
 * The descent spike's results file (plan R05, T14.c, Design notes 18 and 21): the run's
 * description, every figure of the brainstorm's step 3, the pass criterion's rows and the memory
 * read at 1 Hz, written as `docs/measurements/descent-spike/<date>-<machine>-<setting>.json` with
 * a Markdown summary beside it.
 *
 * @remarks
 * Every figure is a {@link Measured}: a value, or `null` with the reason it is missing, so that a
 * hidden run's file (no window, so no presentation times) is complete and says why. Rulings built
 * in:
 *
 * - The timer (decisions-r06-r07.md item 8): each file records `PassTimes.timer`, the platform and
 *   the launch mode. Frame intervals are the pass criterion; on a `quantized` timer a GPU-time row
 *   carries ±65.5 µs a pass (±k × 65.5 µs for a sum of k passes) and is `marginal` within that of
 *   its limit.
 * - The selection bound (decisions-r05.md items 6 and 7): streaming is recorded under the hard
 *   bound selection uses and under min(hard, 4σ_n), so that a failure on demand alone that the
 *   calibrated bound would meet can be called ours to fix.
 * - The quiet-machine rule (Design note 27): a run started with the load average at or above 1 is
 *   marked provisional.
 * - Chromium's tracing service is the measurement's own process, so its memory is reported apart
 *   from the app's.
 */

import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, hostname, loadavg, totalmem } from "node:os";
import { join } from "node:path";

import type { ProcessMetric } from "electron";

import type {
  DescentSpikeReport,
  GraphicsLaunchMode,
  SpikePassRow,
  SpikePassTimer,
} from "../preload/api";
import type { DrmMemoryReading, NvidiaReading } from "./fdinfo";
import type { TraceFigures } from "./reduceTrace";

/** The file's schema name and version, which T15.c's replayer writes too. */
export const RESULTS_SCHEMA = "hyperion.descent-spike.results";
/**
 * The schema's version; bumped with any change to the file's shape.
 *
 * @remarks
 * Version 2 stores the memory series as columns of whole KiB (decision-r05-results-size.md); v1's
 * object a sample made a 20-minute run's file larger than the repository accepts.
 */
export const RESULTS_VERSION = 2;

/**
 * The largest file the repository accepts as added, bytes: pre-commit's `check-added-large-files`
 * refuses a file whose size in KiB, rounded up, is over 500.
 */
export const ADDED_FILE_LIMIT_BYTES = 512_000;

/** The line width Prettier formats the repository's files to (`.prettierrc.json`). */
const PRETTIER_PRINT_WIDTH = 100;

/** Dawn's timestamp quantum, 65,536 ns (`timestamp_quantization`, Design note 18), ms. */
export const TIMESTAMP_QUANTUM_MS = 0.065_536;

/** The `performance.measure` name prefix of a segment's span, `spike.segment:<name>` (T14.a). */
export const SEGMENT_MEASURE_PREFIX = "spike.segment:";

/**
 * Why every figure read from a trace with no timed event is missing.
 *
 * @remarks
 * Such a trace measured nothing: on 2026-10-04 Chromium's tracing service crashed while writing a
 * 1.71 GB trace and left the file without events. Its frame drops and GC pauses are then missing,
 * not zero.
 */
export const EMPTY_TRACE_REASON = "the trace has no timed event";

/** A figure, or `null` with the reason it is missing. */
export type Measured<T> =
  { readonly value: T; readonly reason: null } | { readonly value: null; readonly reason: string };

/** A present figure. */
export function measured<T>(value: T): Measured<T> {
  return { value, reason: null };
}

/** A missing figure and why. */
export function missing(reason: string): Measured<never> {
  return { value: null, reason };
}

/** Frame intervals summarised as Design note 21 reads them. */
export interface FrameStats {
  readonly count: number;
  readonly p50Ms: number;
  readonly p95Ms: number;
  readonly p99Ms: number;
  readonly maxMs: number;
  /** Intervals above 1.5 T, and their fraction; `null` without T. */
  readonly missed: number | null;
  readonly missedFraction: number | null;
  /** Intervals above 3 T; `null` without T. */
  readonly hitches: number | null;
}

/**
 * The `p`th percentile by nearest rank (the smallest value with at least p of the values at or
 * below it), the convention of frame-time tools.
 *
 * @param sorted - Ascending, not empty.
 * @param p - From 0 (exclusive) to 1.
 */
export function nearestRank(sorted: ReadonlyArray<number>, p: number): number {
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1));
  const value = sorted[index];
  if (value === undefined) {
    throw new Error("a percentile of no values");
  }
  return value;
}

/**
 * Summarises intervals against the period T.
 *
 * @param periodMs - T, or `null` when it is not known, leaving the missed and hitch counts null.
 * @returns The summary, or `null` for no intervals.
 */
export function frameStats(
  intervalsMs: ReadonlyArray<number>,
  periodMs: number | null,
): FrameStats | null {
  if (intervalsMs.length === 0) {
    return null;
  }
  const sorted = intervalsMs.toSorted((a, b) => a - b);
  const missed = periodMs === null ? null : sorted.filter((ms) => ms > 1.5 * periodMs).length;
  return {
    count: sorted.length,
    p50Ms: nearestRank(sorted, 0.5),
    p95Ms: nearestRank(sorted, 0.95),
    p99Ms: nearestRank(sorted, 0.99),
    maxMs: sorted.at(-1) ?? 0,
    missed,
    missedFraction: missed === null ? null : missed / sorted.length,
    hitches: periodMs === null ? null : sorted.filter((ms) => ms > 3 * periodMs).length,
  };
}

/** The descent's quality setting. */
export type SpikeSetting = "high" | "low";

/** The machine a run is on. */
export interface MachineDescription {
  /** The machine's name in the file name: its host name, lower-cased, other than `[a-z0-9-]` dropped. */
  readonly name: string;
  readonly cpu: string;
  readonly logicalCores: number;
  readonly memoryBytes: number;
  readonly governor: Measured<string>;
  /** The 1-, 5- and 15-minute load averages when the run started. */
  readonly loadAverage: readonly [number, number, number];
  /** Chromium's GPU description (`app.getGPUInfo("basic")`), or why it is missing. */
  readonly gpu: Measured<{
    readonly vendorId: number;
    readonly deviceId: number;
    readonly driverVersion: string | null;
    readonly description: string | null;
  }>;
}

/** What describes a run, gathered by the main process at its start. */
export interface RunDescription {
  readonly startedAt: Date;
  readonly machine: MachineDescription;
  readonly versions: {
    /** The app's own version, `package.json`'s. */
    readonly app: string;
    /** Electron's version, which the app runs on. */
    readonly electron: string;
    readonly chromium: string;
    readonly node: string;
    readonly v8: string;
  };
  readonly platform: NodeJS.Platform;
  readonly launchMode: GraphicsLaunchMode;
  readonly setting: SpikeSetting;
  /** The spike's seed, a u64 in decimal. */
  readonly seed: string;
  /** Every option of the spike flag as given (T13.c). */
  readonly options: Readonly<Record<string, string | number | boolean>>;
  /** The Chromium switches the launch applied, as `--name=value`. */
  readonly switches: ReadonlyArray<string>;
  /** Whether the run's window was shown; a hidden run has no presentation times. */
  readonly shown: boolean;
  /** The display's refresh rate (`Display.displayFrequency`), Hz, or `null` without one. */
  readonly displayHz: number | null;
  /** `nvidia-smi`'s device memory used before the launch, bytes, or `null` off NVIDIA. */
  readonly nvidiaBaselineBytes: number | null;
}

/** Memory at one instant of a run (Design note 18). */
export interface MemorySample {
  /** Seconds since the run started. */
  readonly tS: number;
  /** The working sets of every app process but the tracing service, bytes. */
  readonly appBytes: number;
  /** The GPU process's working set, bytes. */
  readonly gpuProcessBytes: number | null;
  /** Chromium's tracing service's working set, bytes: the measurement's own. */
  readonly tracingBytes: number;
  /** The renderer's own `process.getProcessMemoryInfo()` private bytes, from the preload. */
  readonly rendererPrivateBytes: number | null;
  /** The GPU process's DRM fdinfo, resident bytes summed over its clients. */
  readonly drmResidentBytes: number | null;
  /** Why the DRM fdinfo gave no reading, or `null` when it gave one. */
  readonly drmReason: string | null;
  readonly drmTotalBytes: number | null;
  /** `nvidia-smi`'s device memory used, bytes. */
  readonly nvidiaDeviceBytes: number | null;
  /** `nvidia-smi`'s figure for the GPU process, bytes, where its process list has it. */
  readonly nvidiaGpuProcessBytes: number | null;
}

/** Consecutive samples at which a reading is missing for one reason. */
export interface MemoryGap {
  /** The first sample's index. */
  readonly from: number;
  /** The last sample's index, inclusive. */
  readonly to: number;
  readonly reason: string;
}

/** One reading over a run, a value a sample. */
export interface MemoryColumn {
  /** The reading at each sample, whole KiB, or -1 at a sample within one of the gaps. */
  readonly samples: ReadonlyArray<number>;
  /** Where the reading is missing, in sample order; consecutive samples of one reason are one gap. */
  readonly gaps: ReadonlyArray<MemoryGap>;
}

/**
 * The memory samples of a run as columns: the times, and one column a reading of
 * {@link MemorySample}, in whole KiB.
 *
 * @remarks
 * Every source reports KiB (Electron's metrics, the renderer's private memory, DRM fdinfo) or MiB
 * (`nvidia-smi`), so the columns hold the readings exactly. A reading missing at every sample is
 * its column's `null` and reason, written once. The form is plain, without delta encoding, so that
 * the file reads by eye and with `jq` (decision-r05-results-size.md).
 */
export interface MemorySeries {
  /** Each sample's time since the run started, whole ms. */
  readonly tMs: ReadonlyArray<number>;
  /** The working sets of every app process but the tracing service. */
  readonly appKiB: Measured<MemoryColumn>;
  /** The GPU process's working set. */
  readonly gpuProcessKiB: Measured<MemoryColumn>;
  /** Chromium's tracing service's working set: the measurement's own. */
  readonly tracingKiB: Measured<MemoryColumn>;
  /** The renderer's own private memory. */
  readonly rendererPrivateKiB: Measured<MemoryColumn>;
  /** The GPU process's DRM fdinfo, resident, summed over its clients. */
  readonly drmResidentKiB: Measured<MemoryColumn>;
  /** The GPU process's DRM fdinfo, total, summed over its clients. */
  readonly drmTotalKiB: Measured<MemoryColumn>;
  /** `nvidia-smi`'s device memory used. */
  readonly nvidiaDeviceKiB: Measured<MemoryColumn>;
  /** `nvidia-smi`'s figure for the GPU process. */
  readonly nvidiaGpuProcessKiB: Measured<MemoryColumn>;
}

/** The series' reading columns. */
type MemoryColumnName = Exclude<keyof MemorySeries, "tMs">;

/** Why a series with no sample has no reading. */
const NO_MEMORY_SAMPLE = "no memory sample";

/**
 * One reading's column.
 *
 * @param bytesOf - The reading at a sample, bytes, or `null` where it is missing.
 * @param reasonOf - Why a sample has no reading.
 * @returns The column, or `null` with every reason given when no sample has the reading.
 */
function memoryColumn(
  memory: ReadonlyArray<MemorySample>,
  bytesOf: (sample: MemorySample) => number | null,
  reasonOf: (sample: MemorySample) => string,
): Measured<MemoryColumn> {
  if (memory.length === 0) {
    return missing(NO_MEMORY_SAMPLE);
  }
  const samples: number[] = [];
  const gaps: Array<{ from: number; to: number; reason: string }> = [];
  memory.forEach((sample, i) => {
    const bytes = bytesOf(sample);
    if (bytes !== null) {
      // Exact for every source, which reports KiB or MiB.
      samples.push(Math.round(bytes / 1024));
      return;
    }
    samples.push(-1);
    const reason = reasonOf(sample);
    const last = gaps.at(-1);
    if (last !== undefined && last.to === i - 1 && last.reason === reason) {
      last.to = i;
    } else {
      gaps.push({ from: i, to: i, reason });
    }
  });
  if (samples.every((kib) => kib === -1)) {
    return missing([...new Set(gaps.map(({ reason }) => reason))].join("; "));
  }
  return measured({ samples, gaps });
}

/**
 * The results file's memory series, built from the sampler's readings.
 *
 * @remarks
 * Times are rounded to the millisecond and readings to the KiB, which every source reports in.
 */
export function memorySeries(memory: ReadonlyArray<MemorySample>): MemorySeries {
  const column = (
    bytesOf: (sample: MemorySample) => number | null,
    reason: string | ((sample: MemorySample) => string),
  ): Measured<MemoryColumn> =>
    memoryColumn(memory, bytesOf, typeof reason === "string" ? () => reason : reason);
  return {
    tMs: memory.map(({ tS }) => Math.round(tS * 1000)),
    appKiB: column(({ appBytes }) => appBytes, NO_MEMORY_SAMPLE),
    gpuProcessKiB: column(({ gpuProcessBytes }) => gpuProcessBytes, "no GPU process"),
    tracingKiB: column(({ tracingBytes }) => tracingBytes, NO_MEMORY_SAMPLE),
    rendererPrivateKiB: column(
      ({ rendererPrivateBytes }) => rendererPrivateBytes,
      "the renderer reported no memory",
    ),
    drmResidentKiB: column(
      ({ drmResidentBytes }) => drmResidentBytes,
      ({ drmReason }) => drmReason ?? "the DRM fdinfo gave no reading",
    ),
    drmTotalKiB: column(
      ({ drmTotalBytes }) => drmTotalBytes,
      ({ drmReason }) => drmReason ?? "not every DRM client gives a total",
    ),
    nvidiaDeviceKiB: column(({ nvidiaDeviceBytes }) => nvidiaDeviceBytes, "no nvidia-smi reading"),
    nvidiaGpuProcessKiB: column(
      ({ nvidiaGpuProcessBytes }) => nvidiaGpuProcessBytes,
      "nvidia-smi does not list the GPU process",
    ),
  };
}

/** A column's peak, bytes, or its reason. */
function columnPeakBytes(column: Measured<MemoryColumn>): Measured<number> {
  if (column.value === null) {
    return missing(column.reason);
  }
  const maxKiB = column.value.samples.reduce((max, kib) => Math.max(max, kib), -1);
  return measured(maxKiB * 1024);
}

/** The tracing service's name in `app.getAppMetrics()`. */
const TRACING_SERVICE = "tracing.mojom.TracingService";

/** The parts of a memory sample the sampler reads, for tests. */
export interface MemorySources {
  appMetrics(): ReadonlyArray<Pick<ProcessMetric, "pid" | "type" | "serviceName" | "memory">>;
  rendererPrivateBytes(): Promise<number | null>;
  drm(pid: number): Promise<DrmMemoryReading>;
  nvidia(): Promise<NvidiaReading>;
  nowMs(): number;
}

/**
 * One memory sample.
 *
 * @param startMs - The run's start on `sources.nowMs`'s clock.
 */
export async function sampleMemory(sources: MemorySources, startMs: number): Promise<MemorySample> {
  const metrics = sources.appMetrics();
  let appBytes = 0;
  let tracingBytes = 0;
  let gpuPid: number | null = null;
  let gpuProcessBytes: number | null = null;
  for (const metric of metrics) {
    // Electron gives working sets in KiB.
    const bytes = metric.memory.workingSetSize * 1024;
    if (metric.serviceName === TRACING_SERVICE) {
      tracingBytes += bytes;
      continue;
    }
    appBytes += bytes;
    if (metric.type === "GPU") {
      gpuPid = metric.pid;
      gpuProcessBytes = bytes;
    }
  }
  const [rendererPrivateBytes, drm, nvidia] = await Promise.all([
    sources.rendererPrivateBytes(),
    gpuPid === null
      ? Promise.resolve<DrmMemoryReading>({ kind: "unavailable", reason: "no GPU process" })
      : sources.drm(gpuPid),
    sources.nvidia(),
  ]);
  const gpu = nvidia.kind === "nvidia" ? nvidia.gpus[0] : undefined;
  return {
    tS: (sources.nowMs() - startMs) / 1000,
    appBytes,
    gpuProcessBytes,
    tracingBytes,
    rendererPrivateBytes,
    drmResidentBytes: drm.kind === "drm" ? drm.residentBytes : null,
    drmReason: drm.kind === "drm" ? null : drm.reason,
    drmTotalBytes: drm.kind === "drm" ? drm.totalBytes : null,
    nvidiaDeviceBytes: gpu?.usedBytes ?? null,
    nvidiaGpuProcessBytes: gpu?.processes.find(({ pid }) => pid === gpuPid)?.usedBytes ?? null,
  };
}

/**
 * Samples memory at 1 Hz from {@link MemorySampler.start} to {@link MemorySampler.stop}.
 *
 * @remarks
 * A sample still being taken when the next is due is not overlapped: that tick is skipped.
 */
export class MemorySampler {
  readonly #sources: MemorySources;
  readonly #intervalMs: number;
  readonly #onError: (error: unknown) => void;
  #samples: MemorySample[] = [];
  #timer: ReturnType<typeof setInterval> | undefined;
  /** The sample being taken, if any. */
  #inFlight: Promise<void> | undefined;
  #startMs = 0;

  constructor(sources: MemorySources, onError: (error: unknown) => void, intervalMs = 1000) {
    this.#sources = sources;
    this.#onError = onError;
    this.#intervalMs = intervalMs;
  }

  /** Starts sampling afresh; the first sample is taken at once. */
  start(): void {
    if (this.#timer !== undefined) {
      return;
    }
    this.#samples = [];
    this.#startMs = this.#sources.nowMs();
    this.#tick();
    this.#timer = setInterval(() => {
      this.#tick();
    }, this.#intervalMs);
  }

  /** Stops sampling and returns every sample taken, the one in flight included. */
  async stop(): Promise<ReadonlyArray<MemorySample>> {
    if (this.#timer !== undefined) {
      clearInterval(this.#timer);
      this.#timer = undefined;
    }
    await this.#inFlight;
    return [...this.#samples];
  }

  #tick(): void {
    if (this.#inFlight !== undefined) {
      return;
    }
    this.#inFlight = this.#take(this.#samples)
      .catch((error: unknown) => {
        console.error("the memory sampler's error handler failed:", error);
      })
      .finally(() => {
        this.#inFlight = undefined;
      });
  }

  /** Takes one sample into `samples`, the run's list when the sample began. */
  async #take(samples: MemorySample[]): Promise<void> {
    let sample: MemorySample;
    try {
      sample = await sampleMemory(this.#sources, this.#startMs);
    } catch (error: unknown) {
      this.#onError(error);
      return;
    }
    samples.push(sample);
  }
}

/** A row's outcome. `marginal` is within a quantized timer's tolerance of its limit. */
export type Verdict = "pass" | "fail" | "marginal" | "not-measured";

/** One row of Design note 21's criterion. */
export interface Criterion {
  readonly id: string;
  /** The row as the plan words it, with its limit. */
  readonly criterion: string;
  /** The limit, or `null` where it depends on a period T that is not known. */
  readonly limit: number | null;
  /** The unit of `limit` and `value`. */
  readonly unit: "ms" | "fraction" | "count" | "bytes";
  readonly value: number | null;
  /** The timer's tolerance on `value`, in its unit; 0 when exact. */
  readonly tolerance: number;
  readonly verdict: Verdict;
  /** Why it is not measured, or a finding beside the verdict; `null` otherwise. */
  readonly note: string | null;
}

/** The streaming figures of one segment, with the calibrated bound's beside the hard one's. */
export interface StreamingFigures {
  readonly segment: string;
  readonly requestedPerS: number;
  readonly bakedPerS: number;
  readonly residentPerS: number;
  readonly predictedHardPerS: number;
  readonly predictedCalibratedPerS: number;
  readonly patchesHard: number;
  readonly patchesCalibrated: number;
  /** Baked against the hard bound's demand: below 1 where the workers fell behind. */
  readonly sustainedFractionHard: number | null;
  /** Baked against the calibrated bound's demand. */
  readonly sustainedFractionCalibrated: number | null;
  readonly streamingS: number;
}

/** A pass's GPU time over the run. */
export interface PassFigures {
  readonly label: string;
  readonly row: SpikePassRow;
  readonly frames: number;
  readonly p50Ms: number;
  readonly p95Ms: number;
  readonly p99Ms: number;
}

/** The results file of one run. */
export interface DescentResults {
  readonly schema: typeof RESULTS_SCHEMA;
  readonly version: typeof RESULTS_VERSION;
  readonly run: {
    readonly startedAt: string;
    readonly machine: MachineDescription;
    readonly versions: RunDescription["versions"];
    readonly platform: NodeJS.Platform;
    /** The client's launch mode, or `native-replay` for `tools/gpu-replay`'s results (T15.c). */
    readonly launchMode: GraphicsLaunchMode | "native-replay";
    readonly setting: SpikeSetting;
    readonly seed: string;
    readonly options: RunDescription["options"];
    readonly switches: ReadonlyArray<string>;
    readonly timer: SpikePassTimer;
    readonly shown: boolean;
    /** T, the period the criterion is read against, ms: the vsync period, doubled on `low`. */
    readonly periodMs: Measured<number>;
    readonly warmupS: number;
    readonly canvas: DescentSpikeReport["canvas"];
    readonly quiet: { readonly provisional: boolean; readonly note: string | null };
    /** The trace's span, ms, and whether it is shorter than the scripted descent. */
    readonly trace: Measured<{ readonly spanMs: number; readonly truncated: boolean }>;
  };
  /** T6's level table as the run used it. */
  readonly levels: DescentSpikeReport["levels"];
  readonly frames: {
    /**
     * Where the criterion's intervals come from: presentation times, else `requestAnimationFrame`;
     * an offscreen native replay (T15.c) times the gaps between the GPU's ends of frames instead.
     */
    readonly source: "presentation" | "raf" | "gpu-completion";
    readonly presentation: Measured<FrameStats>;
    readonly raf: Measured<FrameStats>;
    /** An offscreen native replay's intervals between the GPU's ends of successive frames. */
    readonly gpuCompletion?: Measured<FrameStats>;
    readonly segments: ReadonlyArray<{
      readonly segment: string;
      readonly presentation: Measured<FrameStats>;
      readonly raf: Measured<FrameStats>;
    }>;
    readonly dropped: Measured<number>;
  };
  readonly gpu: {
    readonly timer: SpikePassTimer;
    /** The tolerance of one pass's time, ms: the quantum on a `quantized` timer, else 0. */
    readonly tolerancePerPassMs: number;
    readonly untimedPasses: number;
    readonly passes: Measured<ReadonlyArray<PassFigures>>;
    /** The sum of a frame's timed passes, at the 95th percentile, ms. */
    readonly sumP95Ms: Measured<number>;
    /** The GPU process's main thread: the CPU side of Chromium's command transport and Dawn. */
    readonly gpuProcess: Measured<NonNullable<TraceFigures["gpuProcess"]>>;
  };
  readonly mainThread: {
    /** Our code's time a frame (`performance.measure`), at the 95th percentile, ms. */
    readonly ourCodeP95Ms: Measured<number>;
    readonly split: Measured<NonNullable<TraceFigures["mainThread"]>>;
    readonly gc: Measured<
      ReadonlyArray<{
        readonly process: string | null;
        readonly thread: string | null;
        readonly count: number;
        readonly totalMs: number;
        readonly maxMs: number;
      }>
    >;
  };
  readonly streaming: ReadonlyArray<StreamingFigures>;
  readonly uploads: { readonly bytes: number };
  readonly pipelines: { readonly late: DescentSpikeReport["latePipelines"] };
  readonly memory: {
    /** The 1 Hz samples. Each peak below is its column's maximum, in bytes. */
    readonly series: MemorySeries;
    /** The headline GPU memory and where it is read from. */
    readonly gpuHeadline: Measured<{
      readonly bytes: number;
      readonly source: "nvidia-smi" | "drm-fdinfo" | "adapter-tally";
    }>;
    readonly peakAppBytes: Measured<number>;
    readonly peakTracingBytes: Measured<number>;
    readonly peakRendererPrivateBytes: Measured<number>;
    readonly peakDrmResidentBytes: Measured<number>;
    /** `nvidia-smi`'s device memory used less the baseline before launch, at its peak. */
    readonly peakNvidiaDeviceLessBaselineBytes: Measured<number>;
    /**
     * `nvidia-smi`'s figure for the GPU process alone, at its peak: the headline counts every
     * process on the device, the local LLM's included (hardware item 5).
     */
    readonly peakNvidiaGpuProcessBytes: Measured<number>;
    readonly adapterPeakBytes: number;
  };
  readonly criteria: {
    readonly whole: ReadonlyArray<Criterion>;
    readonly segments: ReadonlyArray<{
      readonly segment: string;
      readonly criteria: ReadonlyArray<Criterion>;
    }>;
    /** `fail` if any row fails, else `not-measured` if any is, else `marginal` if any is. */
    readonly overall: Verdict;
  };
}

/** Everything a results file is built from. */
export interface ResultsInput {
  readonly run: RunDescription;
  readonly report: DescentSpikeReport;
  /** The reduced trace, or why there is none. */
  readonly trace: Measured<TraceFigures>;
  readonly memory: ReadonlyArray<MemorySample>;
}

/** The rows' limits per setting (Design note 21). */
const LIMITS = {
  high: { p95Ms: (t: number) => t + 1, terrainMs: 5, atmosphereMs: 1, memoryBytes: 3e9 },
  low: { p95Ms: () => 35, terrainMs: 14, atmosphereMs: 4, memoryBytes: 1e9 },
} as const;

/** GPU memory above this on the high setting is a finding, not a failure (Design note 21). */
const HIGH_MEMORY_FINDING_BYTES = 2e9;

function judge(value: number, limit: number, tolerance: number): Verdict {
  if (value + tolerance <= limit) {
    return "pass";
  }
  return value - tolerance > limit ? "fail" : "marginal";
}

function row(
  id: string,
  criterion: string,
  limit: Measured<number>,
  unit: Criterion["unit"],
  value: Measured<number>,
  tolerance = 0,
  note: string | null = null,
): Criterion {
  if (value.value === null || limit.value === null) {
    return {
      id,
      criterion,
      limit: limit.value,
      unit,
      value: value.value,
      tolerance,
      verdict: "not-measured",
      note: limit.value === null ? limit.reason : value.reason,
    };
  }
  return {
    id,
    criterion,
    limit: limit.value,
    unit,
    value: value.value,
    tolerance,
    verdict: judge(value.value, limit.value, tolerance),
    note,
  };
}

/** A limit that depends on T, or T's absence. */
function ofPeriod(periodMs: Measured<number>, limit: (t: number) => number): Measured<number> {
  return periodMs.value === null ? missing(periodMs.reason) : measured(limit(periodMs.value));
}

/** The frame rows of Design note 21 for one set of intervals. */
function frameRows(
  stats: Measured<FrameStats>,
  periodMs: Measured<number>,
  setting: SpikeSetting,
): Criterion[] {
  const of = (pickStat: (s: FrameStats) => number | null): Measured<number> => {
    if (stats.value === null) {
      return missing(stats.reason);
    }
    const value = pickStat(stats.value);
    return value === null ? missing(periodMs.reason ?? "no period") : measured(value);
  };
  return [
    row(
      "p50",
      "50th percentile ≤ T + 0.5 ms",
      ofPeriod(periodMs, (t) => t + 0.5),
      "ms",
      of((s) => s.p50Ms),
    ),
    row(
      "p95",
      setting === "high" ? "95th percentile ≤ T + 1 ms" : "95th percentile ≤ 35 ms",
      ofPeriod(periodMs, LIMITS[setting].p95Ms),
      "ms",
      of((s) => s.p95Ms),
    ),
    row(
      "p99",
      "99th percentile ≤ 2T",
      ofPeriod(periodMs, (t) => 2 * t),
      "ms",
      of((s) => s.p99Ms),
    ),
    row(
      "missed",
      "≤ 1% of intervals above 1.5 T",
      measured(0.01),
      "fraction",
      of((s) => s.missedFraction),
    ),
    row(
      "hitches",
      "none above 3 T",
      measured(0),
      "count",
      of((s) => s.hitches),
    ),
  ];
}

function overallOf(criteria: ReadonlyArray<Criterion>): Verdict {
  const verdicts = new Set(criteria.map(({ verdict }) => verdict));
  if (verdicts.has("fail")) {
    return "fail";
  }
  if (verdicts.has("not-measured")) {
    return "not-measured";
  }
  return verdicts.has("marginal") ? "marginal" : "pass";
}

/** The indices of frames at or after the warm-up, and within `[startS, endS)` when given. */
function frameIndices(
  scriptTimesS: ReadonlyArray<number>,
  warmupS: number,
  span?: { readonly startS: number; readonly endS: number },
): number[] {
  const indices: number[] = [];
  scriptTimesS.forEach((t, i) => {
    if (t >= warmupS && (span === undefined || (t >= span.startS && t < span.endS))) {
      indices.push(i);
    }
  });
  return indices;
}

function pick<T>(values: ReadonlyArray<T>, indices: ReadonlyArray<number>): T[] {
  const picked: T[] = [];
  for (const i of indices) {
    const value = values[i];
    if (value !== undefined) {
      picked.push(value);
    }
  }
  return picked;
}

/** The presentation intervals of a trace, by segment, after the warm-up. */
function presentationIntervals(
  trace: TraceFigures,
  warmupS: number,
): { readonly whole: number[]; readonly bySegment: Map<string, number[]> | null } {
  const marks = trace.userTiming
    .filter(({ name }) => name.startsWith(SEGMENT_MEASURE_PREFIX))
    .flatMap(({ name, startsUs, durationsMs }) =>
      startsUs.map((startUs, i) => ({
        segment: name.slice(SEGMENT_MEASURE_PREFIX.length),
        startUs,
        endUs: startUs + (durationsMs[i] ?? 0) * 1000,
      })),
    )
    .toSorted((a, b) => a.startUs - b.startUs);
  const presented = trace.frames.presentedAtUs;
  const descentStartUs = marks[0]?.startUs ?? presented[0] ?? 0;
  const warmEndUs = descentStartUs + warmupS * 1e6;
  const whole: number[] = [];
  const bySegment = marks.length === 0 ? null : new Map<string, number[]>();
  for (let i = 1; i < presented.length; i += 1) {
    const endUs = presented[i] ?? 0;
    const intervalMs = (endUs - (presented[i - 1] ?? endUs)) / 1000;
    if (endUs < warmEndUs) {
      continue;
    }
    whole.push(intervalMs);
    const mark = marks.find(({ startUs, endUs: markEnd }) => endUs >= startUs && endUs < markEnd);
    if (bySegment !== null && mark !== undefined) {
      const list = bySegment.get(mark.segment) ?? [];
      list.push(intervalMs);
      bySegment.set(mark.segment, list);
    }
  }
  return { whole, bySegment };
}

function statsOrMissing(
  intervals: ReadonlyArray<number>,
  periodMs: number | null,
  reason: string,
): Measured<FrameStats> {
  const stats = frameStats(intervals, periodMs);
  return stats === null ? missing(reason) : measured(stats);
}

/**
 * Builds a results file from a run's description, the renderer's report, the trace and memory.
 *
 * @remarks
 * A trace with no timed event is taken as no trace: every figure read from it is missing with
 * {@link EMPTY_TRACE_REASON}.
 */
export function buildResults(input: ResultsInput): DescentResults {
  const { run, report, memory } = input;
  const trace: Measured<TraceFigures> =
    input.trace.value !== null && input.trace.value.span === null
      ? missing(EMPTY_TRACE_REASON)
      : input.trace;
  const setting = run.setting;
  const vsyncMs = run.displayHz === null || run.displayHz <= 0 ? null : 1000 / run.displayHz;
  const periodMs: Measured<number> = !run.shown
    ? missing("no window shown")
    : vsyncMs === null
      ? missing("the display reports no refresh rate")
      : measured(setting === "low" ? 2 * vsyncMs : vsyncMs);
  const t = periodMs.value;

  // Frames: presentation times from the trace, else the rAF timestamps.
  const frames = report.frames;
  const warm = frameIndices(frames.scriptTimesS, report.warmupS);
  const rafWhole = statsOrMissing(
    pick(frames.rafIntervalsMs, warm),
    t,
    "no frame after the warm-up",
  );
  const presentationReason = !run.shown
    ? "no window shown"
    : trace.value === null
      ? trace.reason
      : "no presented frame after the warm-up";
  const presented =
    run.shown && trace.value !== null ? presentationIntervals(trace.value, report.warmupS) : null;
  const presentationWhole =
    presented === null
      ? missing(presentationReason)
      : statsOrMissing(presented.whole, t, presentationReason);
  const source = presentationWhole.value === null ? "raf" : "presentation";
  const segments = report.segments.map((span) => {
    const indices = frameIndices(frames.scriptTimesS, report.warmupS, span);
    const raf = statsOrMissing(
      pick(frames.rafIntervalsMs, indices),
      t,
      "no frame in the segment after the warm-up",
    );
    const list = presented?.bySegment?.get(span.name);
    const presentation =
      presented === null
        ? missing(presentationReason)
        : presented.bySegment === null
          ? missing("the trace has no segment marks")
          : statsOrMissing(list ?? [], t, "no presented frame in the segment");
    return { segment: span.name, presentation, raf };
  });
  const dropped =
    trace.value === null ? missing(trace.reason) : measured(trace.value.frames.dropped);

  const tolerancePerPassMs = report.timer === "quantized" ? TIMESTAMP_QUANTUM_MS : 0;
  const timerReason = "the pass timer is absent (no timestamp-query)";
  const passes: Measured<PassFigures[]> =
    report.timer === "absent"
      ? missing(timerReason)
      : measured(
          frames.passes.flatMap((pass) => {
            const timed = pick(pass.gpuMs, warm).filter((ms): ms is number => ms !== null);
            const stats = frameStats(timed, null);
            return stats === null
              ? []
              : [
                  {
                    label: pass.label,
                    row: pass.row,
                    frames: stats.count,
                    p50Ms: stats.p50Ms,
                    p95Ms: stats.p95Ms,
                    p99Ms: stats.p99Ms,
                  },
                ];
          }),
        );
  const sums = (rowFilter: SpikePassRow | null): { values: number[]; maxPasses: number } => {
    const values: number[] = [];
    let maxPasses = 0;
    for (const i of warm) {
      let sum = 0;
      let k = 0;
      for (const pass of frames.passes) {
        const ms = pass.gpuMs[i];
        if ((rowFilter === null || pass.row === rowFilter) && ms !== undefined && ms !== null) {
          sum += ms;
          k += 1;
        }
      }
      if (k > 0) {
        values.push(sum);
        maxPasses = Math.max(maxPasses, k);
      }
    }
    return { values, maxPasses };
  };
  const p95Of = (values: ReadonlyArray<number>, reason: string): Measured<number> =>
    values.length === 0
      ? missing(reason)
      : measured(
          nearestRank(
            values.toSorted((a, b) => a - b),
            0.95,
          ),
        );
  const allSums = sums(null);
  const sumP95Ms =
    report.timer === "absent" ? missing(timerReason) : p95Of(allSums.values, "no timed pass");
  const terrain = sums("terrain");
  const atmosphere = sums("atmosphere");

  const ourCodeP95Ms = p95Of(pick(frames.ourCodeMs, warm), "no frame after the warm-up");

  const series = memorySeries(memory);
  const nvidiaDevicePeakBytes = columnPeakBytes(series.nvidiaDeviceKiB);
  const nvidiaLessBaseline: Measured<number> =
    run.nvidiaBaselineBytes === null
      ? missing("no nvidia-smi baseline (not NVIDIA)")
      : nvidiaDevicePeakBytes.value === null
        ? missing(nvidiaDevicePeakBytes.reason)
        : measured(nvidiaDevicePeakBytes.value - run.nvidiaBaselineBytes);
  const drmResidentPeakBytes = columnPeakBytes(series.drmResidentKiB);
  const gpuHeadline: DescentResults["memory"]["gpuHeadline"] =
    nvidiaLessBaseline.value !== null
      ? measured({ bytes: nvidiaLessBaseline.value, source: "nvidia-smi" })
      : drmResidentPeakBytes.value !== null
        ? measured({ bytes: drmResidentPeakBytes.value, source: "drm-fdinfo" })
        : measured({ bytes: report.adapterPeakBytes, source: "adapter-tally" });

  const limits = LIMITS[setting];
  const criterionStats = source === "presentation" ? presentationWhole : rafWhole;
  const headroomLimit = ofPeriod(periodMs, (period) => 0.8 * period);
  const memoryValue: Measured<number> =
    gpuHeadline.value === null ? missing(gpuHeadline.reason) : measured(gpuHeadline.value.bytes);
  const whole: Criterion[] = [
    ...frameRows(criterionStats, periodMs, setting),
    row(
      "headroom-main",
      "main thread ≤ 0.8 T at the 95th percentile",
      headroomLimit,
      "ms",
      ourCodeP95Ms,
    ),
    row(
      "headroom-gpu",
      "GPU pass sum ≤ 0.8 T at the 95th percentile",
      headroomLimit,
      "ms",
      sumP95Ms,
      allSums.maxPasses * tolerancePerPassMs,
    ),
    row(
      "terrain",
      `terrain GPU time ≤ ${limits.terrainMs} ms at the 95th percentile`,
      measured(limits.terrainMs),
      "ms",
      report.timer === "absent"
        ? missing(timerReason)
        : p95Of(terrain.values, "no timed terrain pass"),
      terrain.maxPasses * tolerancePerPassMs,
    ),
    row(
      "atmosphere",
      `atmosphere GPU time ≤ ${limits.atmosphereMs} ms at the 95th percentile`,
      measured(limits.atmosphereMs),
      "ms",
      report.timer === "absent"
        ? missing(timerReason)
        : p95Of(atmosphere.values, "no timed atmosphere pass"),
      atmosphere.maxPasses * tolerancePerPassMs,
    ),
    row(
      "memory",
      setting === "high" ? "GPU resident ≤ 3 GB" : "GPU resident ≤ 1 GB",
      measured(limits.memoryBytes),
      "bytes",
      memoryValue,
      0,
      setting === "high" &&
        gpuHeadline.value !== null &&
        gpuHeadline.value.bytes > HIGH_MEMORY_FINDING_BYTES
        ? "above 2 GB: a finding"
        : gpuHeadline.value?.source === "adapter-tally"
          ? "read from the adapter's tally: no driver reading"
          : null,
    ),
  ];
  const segmentCriteria = segments.map(({ segment, presentation, raf }) => ({
    segment,
    criteria: frameRows(source === "presentation" ? presentation : raf, periodMs, setting),
  }));
  const overall = overallOf([...whole, ...segmentCriteria.flatMap(({ criteria }) => criteria)]);

  const scriptEndS = Math.max(0, ...report.segments.map(({ endS }) => endS));
  // An empty trace was taken as none above; the span's own check only narrows its type.
  const traceFigure: DescentResults["run"]["trace"] =
    trace.value === null
      ? missing(trace.reason)
      : trace.value.span === null
        ? missing(EMPTY_TRACE_REASON)
        : measured({
            spanMs: (trace.value.span.lastUs - trace.value.span.firstUs) / 1000,
            truncated:
              (trace.value.span.lastUs - trace.value.span.firstUs) / 1e6 < 0.95 * scriptEndS,
          });
  const load = run.machine.loadAverage[0];
  const provisional = load >= 1;

  return {
    schema: RESULTS_SCHEMA,
    version: RESULTS_VERSION,
    run: {
      startedAt: run.startedAt.toISOString(),
      machine: run.machine,
      versions: run.versions,
      platform: run.platform,
      launchMode: run.launchMode,
      setting,
      seed: run.seed,
      options: run.options,
      switches: run.switches,
      timer: report.timer,
      shown: run.shown,
      periodMs,
      warmupS: report.warmupS,
      canvas: report.canvas,
      quiet: {
        provisional,
        note: provisional
          ? `load average ${load.toFixed(2)} at the start (Design note 27 asks under 1): provisional`
          : null,
      },
      trace: traceFigure,
    },
    levels: report.levels,
    frames: {
      source,
      presentation: presentationWhole,
      raf: rafWhole,
      segments,
      dropped,
    },
    gpu: {
      timer: report.timer,
      tolerancePerPassMs,
      untimedPasses: report.untimedPasses,
      passes,
      sumP95Ms,
      gpuProcess:
        trace.value === null
          ? missing(trace.reason)
          : trace.value.gpuProcess === null
            ? missing("the trace has no GPU process")
            : measured(trace.value.gpuProcess),
    },
    mainThread: {
      ourCodeP95Ms,
      split:
        trace.value === null
          ? missing(trace.reason)
          : trace.value.mainThread === null
            ? missing("the trace has no renderer main thread")
            : measured(trace.value.mainThread),
      gc:
        trace.value === null
          ? missing(trace.reason)
          : measured(
              trace.value.threads
                .filter(({ gc }) => gc.count > 0)
                .map(({ process, thread, gc }) => Object.assign({ process, thread }, gc)),
            ),
    },
    streaming: report.streaming.map((segment) =>
      Object.assign({}, segment, {
        sustainedFractionHard:
          segment.predictedHardPerS > 0 ? segment.bakedPerS / segment.predictedHardPerS : null,
        sustainedFractionCalibrated:
          segment.predictedCalibratedPerS > 0
            ? segment.bakedPerS / segment.predictedCalibratedPerS
            : null,
      }),
    ),
    uploads: { bytes: report.uploadBytes },
    pipelines: { late: report.latePipelines },
    memory: {
      series,
      gpuHeadline,
      peakAppBytes: columnPeakBytes(series.appKiB),
      peakTracingBytes: columnPeakBytes(series.tracingKiB),
      peakRendererPrivateBytes: columnPeakBytes(series.rendererPrivateKiB),
      peakDrmResidentBytes: drmResidentPeakBytes,
      peakNvidiaDeviceLessBaselineBytes: nvidiaLessBaseline,
      peakNvidiaGpuProcessBytes: columnPeakBytes(series.nvidiaGpuProcessKiB),
      adapterPeakBytes: report.adapterPeakBytes,
    },
    criteria: { whole, segments: segmentCriteria, overall },
  };
}

/** The readers {@link describeMachine} uses, for tests. */
export interface MachineSources {
  hostname(): string;
  cpus(): ReadonlyArray<{ readonly model: string }>;
  totalmem(): number;
  loadavg(): ReadonlyArray<number>;
  readFile(path: string): Promise<string>;
  /** Chromium's basic GPU information (`app.getGPUInfo("basic")`). */
  gpuInfo(): Promise<unknown>;
}

/** The scaling governor of the first CPU, which the run records (Design note 27). */
const GOVERNOR_PATH = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The GPU a run's description names. */
type GpuDescription = NonNullable<MachineDescription["gpu"]["value"]>;

/** The active GPU of Chromium's basic GPU information, narrowed from what Electron returns. */
function activeGpu(info: unknown): Measured<GpuDescription> {
  const devices = isRecord(info) ? info["gpuDevice"] : undefined;
  const list = Array.isArray(devices) ? devices.filter(isRecord) : [];
  const device = list.find((entry) => entry["active"] === true) ?? list[0];
  const vendorId = device?.["vendorId"];
  const deviceId = device?.["deviceId"];
  if (device === undefined || typeof vendorId !== "number" || typeof deviceId !== "number") {
    return missing("Chromium reports no GPU device");
  }
  const driver = device["driverVersion"];
  const description = device["deviceString"];
  return measured({
    vendorId,
    deviceId,
    driverVersion: typeof driver === "string" ? driver : null,
    description: typeof description === "string" ? description : null,
  });
}

/** The machine a run is on, read at its start. */
export async function describeMachine(sources: MachineSources): Promise<MachineDescription> {
  const [l1 = 0, l5 = 0, l15 = 0] = sources.loadavg();
  const governor = await sources.readFile(GOVERNOR_PATH).then(
    (text) => measured(text.trim()),
    () => missing(`${GOVERNOR_PATH} could not be read`),
  );
  const gpu = await sources
    .gpuInfo()
    .then(activeGpu, (error: unknown) =>
      missing(
        `Chromium's GPU information failed: ${error instanceof Error ? error.message : String(error)}`,
      ),
    );
  const cpuList = sources.cpus();
  const name = sources
    .hostname()
    .toLowerCase()
    .replaceAll(/[^a-z0-9-]/g, "");
  return {
    name: name.length > 0 ? name : "machine",
    cpu: cpuList[0]?.model.trim() ?? "unknown",
    logicalCores: cpuList.length,
    memoryBytes: sources.totalmem(),
    governor,
    loadAverage: [l1, l5, l15],
    gpu,
  };
}

/** This process's machine readers, with Chromium's GPU information from `gpuInfo`. */
export function nodeMachineSources(gpuInfo: () => Promise<unknown>): MachineSources {
  return {
    hostname,
    cpus,
    totalmem,
    loadavg,
    readFile: (path) => readFile(path, "utf8"),
    gpuInfo,
  };
}

const VERDICTS: ReadonlySet<string> = new Set(["pass", "fail", "marginal", "not-measured"]);

/**
 * Checks a parsed results file against its schema.
 *
 * @returns The problems found, empty for a valid file. Beyond the shape of each required part,
 * every figure must be present or `null` with a stated reason, without a trace no figure read
 * from the trace may be present, and the memory series must be whole: every column as long as the
 * times, readings in whole KiB, -1 at its gaps' samples alone, and each peak its column's maximum.
 */
export function validateResults(value: unknown): string[] {
  const problems: string[] = [];
  if (!isRecord(value)) {
    return ["the file is not an object"];
  }
  if (value["schema"] !== RESULTS_SCHEMA) {
    problems.push(`schema is not ${RESULTS_SCHEMA}`);
  }
  if (value["version"] !== RESULTS_VERSION) {
    problems.push(`version is not ${RESULTS_VERSION}`);
  }
  const run = value["run"];
  if (isRecord(run)) {
    for (const key of ["startedAt", "platform", "launchMode", "setting", "seed", "timer"]) {
      if (typeof run[key] !== "string") {
        problems.push(`run.${key} is not a string`);
      }
    }
    if (run["setting"] !== "high" && run["setting"] !== "low") {
      problems.push("run.setting is neither high nor low");
    }
    if (!["full", "quantized", "absent"].includes(String(run["timer"]))) {
      problems.push("run.timer is not full, quantized or absent");
    }
    const machine = run["machine"];
    if (!isRecord(machine) || !Array.isArray(machine["loadAverage"])) {
      problems.push("run.machine has no load average");
    }
    if (!Array.isArray(run["switches"])) {
      problems.push("run.switches is not a list");
    }
  } else {
    problems.push("run is missing");
  }
  for (const key of ["levels", "streaming"]) {
    if (!Array.isArray(value[key])) {
      problems.push(`${key} is not a list`);
    }
  }
  for (const key of ["frames", "gpu", "mainThread", "memory", "criteria", "uploads", "pipelines"]) {
    if (!isRecord(value[key])) {
      problems.push(`${key} is missing`);
    }
  }
  const criteria = value["criteria"];
  if (isRecord(criteria)) {
    checkCriteria(criteria, problems);
  }
  const memory = value["memory"];
  if (isRecord(memory)) {
    checkMemory(memory, problems);
  }
  checkFigures(value, "", problems);
  checkTraceFigures(value, problems);
  // A whole-run null column without a reason is found by both checks.
  return [...new Set(problems)];
}

/** Each column's peak among the memory figures, or `null` for a column without one. */
const COLUMN_PEAKS: Readonly<Record<MemoryColumnName, keyof DescentResults["memory"] | null>> = {
  appKiB: "peakAppBytes",
  gpuProcessKiB: null,
  tracingKiB: "peakTracingBytes",
  rendererPrivateKiB: "peakRendererPrivateBytes",
  drmResidentKiB: "peakDrmResidentBytes",
  drmTotalKiB: null,
  nvidiaDeviceKiB: "peakNvidiaDeviceLessBaselineBytes",
  nvidiaGpuProcessKiB: "peakNvidiaGpuProcessBytes",
};

/** The one peak read less a baseline, which the file does not hold: at most its column's maximum. */
const LESS_BASELINE_PEAK = "peakNvidiaDeviceLessBaselineBytes";

function isWhole(value: unknown, min: number): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= min;
}

/**
 * The memory series: times in whole ms, columns as long as the times, readings in whole KiB, -1
 * at a gap's samples and nowhere else, and each peak its column's maximum.
 */
function checkMemory(memory: Readonly<Record<string, unknown>>, problems: string[]): void {
  const series = memory["series"];
  if (!isRecord(series)) {
    problems.push("memory.series is missing");
    return;
  }
  const tMs = series["tMs"];
  if (!Array.isArray(tMs) || !tMs.every((t: unknown) => isWhole(t, 0))) {
    problems.push("memory.series.tMs is not a list of whole ms");
    return;
  }
  for (const [name, peakName] of Object.entries(COLUMN_PEAKS)) {
    const maxKiB = checkColumn(series[name], `memory.series.${name}`, tMs.length, problems);
    if (maxKiB === undefined || peakName === null) {
      continue;
    }
    const peak = memory[peakName];
    const peakBytes = isRecord(peak) ? peak["value"] : undefined;
    if (maxKiB === null) {
      if (peakBytes !== null) {
        problems.push(`memory.${peakName} is not null, but ${name} has no reading`);
      }
    } else if (peakName === LESS_BASELINE_PEAK) {
      if (peakBytes !== null && (typeof peakBytes !== "number" || peakBytes > maxKiB * 1024)) {
        problems.push(`memory.${peakName} is above ${name}'s maximum`);
      }
    } else if (peakBytes !== maxKiB * 1024) {
      problems.push(`memory.${peakName} is not ${name}'s maximum × 1024 (${maxKiB * 1024} B)`);
    }
  }
}

/**
 * One column of the memory series.
 *
 * @param count - The number of samples, `tMs`'s length.
 * @returns The column's largest reading, KiB; `null` for a column with no reading all run;
 * `undefined` when the column is malformed, its problem added to `problems`.
 */
function checkColumn(
  column: unknown,
  path: string,
  count: number,
  problems: string[],
): number | null | undefined {
  if (!isRecord(column)) {
    problems.push(`${path} is missing`);
    return undefined;
  }
  const value = column["value"];
  if (value === null) {
    const reason = column["reason"];
    if (typeof reason !== "string" || reason.length === 0) {
      problems.push(`${path} is null without a reason`);
      return undefined;
    }
    return null;
  }
  const samples = isRecord(value) ? value["samples"] : undefined;
  const gaps = isRecord(value) ? value["gaps"] : undefined;
  if (!Array.isArray(samples) || !Array.isArray(gaps)) {
    problems.push(`${path} is neither null nor samples with their gaps`);
    return undefined;
  }
  if (samples.length !== count) {
    problems.push(`${path} has ${samples.length} samples, not tMs's ${count}`);
    return undefined;
  }
  const missingAt = new Set<number>();
  let previous: { readonly to: number; readonly reason: string } | undefined;
  for (const [i, gap] of gaps.entries()) {
    const from: unknown = isRecord(gap) ? gap["from"] : undefined;
    const to: unknown = isRecord(gap) ? gap["to"] : undefined;
    const reason: unknown = isRecord(gap) ? gap["reason"] : undefined;
    if (
      !isWhole(from, previous === undefined ? 0 : previous.to + 1) ||
      !isWhole(to, from) ||
      to >= count ||
      typeof reason !== "string" ||
      reason.length === 0
    ) {
      problems.push(
        `${path}.value.gaps[${i}] is not a stretch of samples with a reason, after the gap before it`,
      );
      return undefined;
    }
    if (previous !== undefined && from === previous.to + 1 && reason === previous.reason) {
      problems.push(`${path}.value.gaps[${i}] continues the gap before it for the same reason`);
      return undefined;
    }
    for (let sample = from; sample <= to; sample += 1) {
      missingAt.add(sample);
    }
    previous = { to, reason };
  }
  let maxKiB = -1;
  for (const [i, kib] of samples.entries()) {
    const kibValue: unknown = kib;
    if (missingAt.has(i) ? kibValue !== -1 : !isWhole(kibValue, 0)) {
      problems.push(
        missingAt.has(i)
          ? `${path}.value.samples[${i}] is within a gap but not -1`
          : kibValue === -1
            ? `${path}.value.samples[${i}] is -1 outside a gap`
            : `${path}.value.samples[${i}] is not a whole number of KiB`,
      );
      return undefined;
    }
    if (typeof kibValue === "number") {
      maxKiB = Math.max(maxKiB, kibValue);
    }
  }
  if (maxKiB === -1) {
    problems.push(`${path} has no reading: a reading missing all run is null with its reason`);
    return undefined;
  }
  return maxKiB;
}

/** The figures {@link buildResults} reads from the trace alone, by path. */
const TRACE_FIGURE_PATHS: ReadonlyArray<ReadonlyArray<string>> = [
  ["frames", "dropped"],
  ["gpu", "gpuProcess"],
  ["mainThread", "split"],
  ["mainThread", "gc"],
];

function childAt(node: unknown, path: ReadonlyArray<string>): unknown {
  return path.reduce<unknown>((child, key) => (isRecord(child) ? child[key] : undefined), node);
}

/**
 * Without a trace (`run.trace` null), every figure read from the trace is null too, so that an
 * empty trace's zeros cannot pass for measurements. A native replay times its presentations
 * without a trace, so only its other trace figures are checked.
 */
function checkTraceFigures(value: Readonly<Record<string, unknown>>, problems: string[]): void {
  const trace = childAt(value, ["run", "trace"]);
  if (!isRecord(trace) || trace["value"] !== null) {
    return;
  }
  const figures: Array<readonly [string, unknown]> = TRACE_FIGURE_PATHS.map((path) => [
    path.join("."),
    childAt(value, path),
  ]);
  if (childAt(value, ["run", "launchMode"]) !== "native-replay") {
    figures.push(["frames.presentation", childAt(value, ["frames", "presentation"])]);
    const segments = childAt(value, ["frames", "segments"]);
    if (Array.isArray(segments)) {
      segments.forEach((segment: unknown, i) => {
        figures.push([`frames.segments[${i}].presentation`, childAt(segment, ["presentation"])]);
      });
    }
  }
  for (const [path, figure] of figures) {
    if (isRecord(figure) && figure["value"] !== null && figure["value"] !== undefined) {
      problems.push(`${path} is measured without a trace (${String(trace["reason"])})`);
    }
  }
}

function checkCriteria(criteria: Readonly<Record<string, unknown>>, problems: string[]): void {
  const whole = criteria["whole"];
  const segments = criteria["segments"];
  const rows: unknown[] = [
    ...(Array.isArray(whole) ? whole : []),
    ...(Array.isArray(segments)
      ? segments.flatMap((segment: unknown) =>
          isRecord(segment) && Array.isArray(segment["criteria"]) ? segment["criteria"] : [],
        )
      : []),
  ];
  if (rows.length === 0) {
    problems.push("criteria has no rows");
  }
  for (const entry of rows) {
    if (!isRecord(entry) || !VERDICTS.has(String(entry["verdict"]))) {
      problems.push(`a criterion has no verdict: ${JSON.stringify(entry)}`);
    }
  }
  if (!VERDICTS.has(String(criteria["overall"]))) {
    problems.push("criteria.overall is not a verdict");
  }
}

/** Every `{ value, reason }` figure under `node` is present, or null with a reason. */
function checkFigures(node: unknown, path: string, problems: string[]): void {
  if (Array.isArray(node)) {
    node.forEach((child: unknown, i) => {
      checkFigures(child, `${path}[${i}]`, problems);
    });
    return;
  }
  if (!isRecord(node)) {
    return;
  }
  const keys = Object.keys(node);
  if (keys.length === 2 && keys.includes("value") && keys.includes("reason")) {
    const figure = node["value"];
    const reason = node["reason"];
    if (figure === null && (typeof reason !== "string" || reason.length === 0)) {
      problems.push(`${path} is null without a reason`);
    }
    if (figure !== null && reason !== null) {
      problems.push(`${path} has both a value and a reason`);
    }
    checkFigures(figure, `${path}.value`, problems);
    return;
  }
  for (const key of keys) {
    checkFigures(node[key], path === "" ? key : `${path}.${key}`, problems);
  }
}

function formatMs(value: number): string {
  return value.toFixed(2);
}

/** How a criterion's value or limit in each unit is written. */
const FORMAT_IN: Readonly<Record<Criterion["unit"], (value: number, digits: number) => string>> = {
  ms: (value) => `${formatMs(value)} ms`,
  fraction: (value, digits) => `${(value * 100).toFixed(digits)} %`,
  count: (value) => String(value),
  bytes: (value, digits) => `${(value / 1e9).toFixed(digits + 1)} GB`,
};

function formatValue(entry: Criterion): string {
  if (entry.value === null) {
    return "—";
  }
  const text = FORMAT_IN[entry.unit](entry.value, 2);
  return entry.tolerance > 0 ? `${text} ± ${(entry.tolerance * 1000).toFixed(0)} µs` : text;
}

function formatLimit(entry: Criterion): string {
  return entry.limit === null ? "—" : FORMAT_IN[entry.unit](entry.limit, 0);
}

function describeStats(stats: Measured<FrameStats>): string {
  if (stats.value === null) {
    return `— (${stats.reason})`;
  }
  const { count, p50Ms, p95Ms, p99Ms, maxMs } = stats.value;
  return `${count} intervals, p50 ${formatMs(p50Ms)} ms, p95 ${formatMs(p95Ms)} ms, p99 ${formatMs(p99Ms)} ms, max ${formatMs(maxMs)} ms`;
}

function mib(bytes: number): string {
  return `${(bytes / 1024 ** 2).toFixed(0)} MiB`;
}

function mibOr(figure: Measured<number>): string {
  return figure.value === null ? `— (${figure.reason})` : mib(figure.value);
}

function textOr<T>(figure: Measured<T>, show: (value: T) => string): string {
  return figure.value === null ? `— (${figure.reason})` : show(figure.value);
}

/** The intervals a file's criterion reads, and where they come from, in words. */
function intervalsOf(frames: DescentResults["frames"]): {
  readonly source: string;
  readonly stats: Measured<FrameStats>;
} {
  const sources: Readonly<
    Record<DescentResults["frames"]["source"], { source: string; stats: Measured<FrameStats> }>
  > = {
    presentation: { source: "presentation times", stats: frames.presentation },
    raf: { source: "requestAnimationFrame timestamps", stats: frames.raf },
    "gpu-completion": {
      source: "the GPU's ends of frames (an offscreen native replay)",
      stats: frames.gpuCompletion ?? missing("the replay recorded no GPU intervals"),
    },
  };
  return sources[frames.source];
}

/** The Markdown summary written beside a results file. */
export function summaryMarkdown(results: DescentResults): string {
  const { run, criteria, memory, frames } = results;
  const fromPresentation = frames.source === "presentation";
  const whole = intervalsOf(frames);
  const headline = memory.gpuHeadline;
  return [
    `# Descent spike: ${run.machine.name}, ${run.setting}, ${run.startedAt.slice(0, 10)}`,
    "",
    `- **Overall:** ${criteria.overall}${run.quiet.provisional ? " (provisional: not a quiet machine)" : ""}`,
    `- **Machine:** ${run.machine.cpu}, ${run.machine.logicalCores} threads; GPU ${textOr(run.machine.gpu, (gpu) => gpu.description ?? `${gpu.vendorId}:${gpu.deviceId}`)}; governor ${textOr(run.machine.governor, (governor) => governor)}; load average ${run.machine.loadAverage.map((load) => load.toFixed(2)).join(", ")}`,
    `- **Versions:** app ${run.versions.app}, Electron ${run.versions.electron}, Chromium ${run.versions.chromium}`,
    `- **Launch:** ${run.platform}, ${run.launchMode} mode, timer ${run.timer}, seed ${run.seed}, window ${run.shown ? "shown" : "hidden"}, canvas ${run.canvas.widthPx} × ${run.canvas.heightPx} px`,
    `- **T:** ${textOr(run.periodMs, (period) => `${formatMs(period)} ms`)}; warm-up ${run.warmupS} s`,
    `- **Trace:** ${textOr(run.trace, (trace) => `${(trace.spanMs / 1000).toFixed(1)} s${trace.truncated ? ", truncated" : ""}`)}`,
    `- **Options:** ${Object.entries(run.options)
      .map(([key, value]) => `\`--${key} ${String(value)}\``)
      .join(" ")}`,
    "",
    "## The criterion (Design note 21)",
    "",
    "| Row | Limit | Value | Verdict | Note |",
    "| --- | --- | --- | --- | --- |",
    ...criteria.whole.map(
      (entry) =>
        `| ${entry.criterion} | ${formatLimit(entry)} | ${formatValue(entry)} | ${entry.verdict} | ${entry.note ?? ""} |`,
    ),
    "",
    "## Frames by segment",
    "",
    `Intervals from ${whole.source}. Whole descent: ${describeStats(whole.stats)}.`,
    "",
    "| Segment | Intervals | Verdicts (p50, p95, p99, missed, hitches) |",
    "| --- | --- | --- |",
    ...criteria.segments.map(({ segment, criteria: rows }) => {
      const stats = frames.segments.find((entry) => entry.segment === segment);
      const shown = fromPresentation ? stats?.presentation : stats?.raf;
      return `| ${segment} | ${shown === undefined ? "—" : describeStats(shown)} | ${rows.map(({ verdict }) => verdict).join(", ")} |`;
    }),
    "",
    "## Streaming (patches a second)",
    "",
    "| Segment | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...results.streaming.map(
      (s) =>
        `| ${s.segment} | ${s.requestedPerS.toFixed(1)} | ${s.bakedPerS.toFixed(1)} | ${s.residentPerS.toFixed(1)} | ${s.predictedHardPerS.toFixed(1)} | ${s.predictedCalibratedPerS.toFixed(1)} | ${s.patchesHard.toFixed(0)} | ${s.patchesCalibrated.toFixed(0)} | ${s.streamingS.toFixed(1)} |`,
    ),
    "",
    "## Memory (peaks)",
    "",
    `- GPU headline: ${textOr(headline, ({ bytes, source }) => `${mib(bytes)} from ${source}`)}`,
    `- App processes: ${mibOr(memory.peakAppBytes)}; tracing service (the measurement's own): ${mibOr(memory.peakTracingBytes)}`,
    `- Renderer private: ${mibOr(memory.peakRendererPrivateBytes)}`,
    `- DRM fdinfo resident: ${mibOr(memory.peakDrmResidentBytes)}`,
    `- nvidia-smi less its baseline: ${mibOr(memory.peakNvidiaDeviceLessBaselineBytes)}; the GPU process alone: ${mibOr(memory.peakNvidiaGpuProcessBytes)}`,
    `- Adapter tally: ${mib(memory.adapterPeakBytes)}`,
    "",
    `Uploads ${mib(results.uploads.bytes)}; ${results.pipelines.late.length} pipelines created after warm-up; ${results.gpu.untimedPasses} untimed passes.`,
    "",
  ].join("\n");
}

/** A JSON number's text, as `JSON.stringify` writes one. */
const JSON_NUMBER = /^-?\d+(?:\.\d+)?(?:e[+-]\d+)?$/;

/** A line holding one number, as an array's element or a property's value. */
const NUMBER_LINE = /^(\s*(?:"(?:[^"\\]|\\.)*": )?-?\d+(?:\.\d+)?e)\+(\d+,?)$/;

/**
 * A results file's text as the repository's Prettier writes it at {@link PRETTIER_PRINT_WIDTH},
 * from `JSON.stringify`'s indented form, which {@link writeResults} writes.
 *
 * @remarks
 * What Prettier changes in that form, and no more: it keeps each object broken as the input breaks
 * it, prints an array of numbers, strings, booleans or nulls on one line where it fits, and packs
 * one that does not as many elements a line as fit when every element is a number (an array with
 * a `null` stays one element a line). It drops an exponent's `+`. The writer uses this to warn of a
 * file over {@link ADDED_FILE_LIMIT_BYTES}, since the client does not ship Prettier.
 */
export function formatAsPrettier(json: string): string {
  const lines = json.split("\n").map((line) => line.replace(NUMBER_LINE, "$1$2"));
  const out: string[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i] ?? "";
    i += 1;
    if (!line.endsWith("[")) {
      out.push(line);
      continue;
    }
    const indent = line.length - line.trimStart().length;
    const elements: string[] = [];
    let close: string | undefined;
    for (let j = i; j < lines.length; j += 1) {
      const element = lines[j] ?? "";
      const trimmed = element.trimStart();
      if (element.length - trimmed.length === indent && /^\],?$/.test(trimmed)) {
        close = trimmed;
        break;
      }
      if (trimmed.endsWith("{") || trimmed.endsWith("[")) {
        break;
      }
      elements.push(trimmed.endsWith(",") ? trimmed.slice(0, -1) : trimmed);
    }
    if (close === undefined) {
      // An array of objects or arrays: Prettier keeps it as it is.
      out.push(line);
      continue;
    }
    i += elements.length + 1;
    const flat = `${line}${elements.join(", ")}${close}`;
    if (flat.length <= PRETTIER_PRINT_WIDTH) {
      out.push(flat);
      continue;
    }
    const pad = " ".repeat(indent + 2);
    if (!elements.every((element) => JSON_NUMBER.test(element))) {
      out.push(
        line,
        ...elements.map((element, k) => `${pad}${element}${k < elements.length - 1 ? "," : ""}`),
      );
      out.push(`${" ".repeat(indent)}${close}`);
      continue;
    }
    out.push(line);
    let packed = "";
    elements.forEach((element, k) => {
      const item = k < elements.length - 1 ? `${element},` : element;
      if (packed.length === 0) {
        packed = `${pad}${item}`;
      } else if (packed.length + 1 + item.length <= PRETTIER_PRINT_WIDTH) {
        packed = `${packed} ${item}`;
      } else {
        out.push(packed);
        packed = `${pad}${item}`;
      }
    });
    out.push(packed, `${" ".repeat(indent)}${close}`);
  }
  return out.join("\n");
}

/** Runs of one day, machine and setting that {@link writeResults} numbers before it gives up. */
const MAX_RUNS_A_DAY = 100;

/** The file system calls {@link writeResults} makes, for tests. */
export interface ResultsFiles {
  mkdir(path: string): Promise<unknown>;
  exists(path: string): Promise<boolean>;
  writeFile(path: string, text: string): Promise<void>;
}

const NODE_RESULTS_FILES: ResultsFiles = {
  mkdir: (path) => mkdir(path, { recursive: true }),
  exists: (path) =>
    access(path).then(
      () => true,
      () => false,
    ),
  writeFile: (path, text) => writeFile(path, text, "utf8"),
};

/**
 * Writes a results file and its summary into `dir` as `<date>-<machine>-<setting>.json` and
 * `.md`, adding `-2`, `-3` and so on when a run of the same day, machine and setting is there.
 *
 * @remarks
 * A file that Prettier would format to more than {@link ADDED_FILE_LIMIT_BYTES} is still written
 * in full, with a warning that names its size: the series is never thinned or split
 * (decision-r05-results-size.md). Such a run is a finding.
 *
 * @returns The paths written.
 * @throws Error if the results do not match their schema, naming each problem, or if the day's
 * names are used up.
 */
export async function writeResults(
  dir: string,
  results: DescentResults,
  files: ResultsFiles = NODE_RESULTS_FILES,
): Promise<{ readonly json: string; readonly markdown: string }> {
  const problems = validateResults(results);
  if (problems.length > 0) {
    throw new Error(`the results do not match their schema: ${problems.join("; ")}`);
  }
  await files.mkdir(dir);
  const stem = `${results.run.startedAt.slice(0, 10)}-${results.run.machine.name}-${results.run.setting}`;
  const candidates = Array.from({ length: MAX_RUNS_A_DAY }, (_, i) =>
    i === 0 ? stem : `${stem}-${i + 1}`,
  );
  const taken = await Promise.all(
    candidates.map((candidate) => files.exists(join(dir, `${candidate}.json`))),
  );
  const name = candidates[taken.indexOf(false)];
  if (name === undefined) {
    throw new Error(`${dir} already holds ${MAX_RUNS_A_DAY} runs named ${stem}`);
  }
  const json = join(dir, `${name}.json`);
  const markdown = join(dir, `${name}.md`);
  const text = `${JSON.stringify(results, null, 2)}\n`;
  await files.writeFile(json, text);
  await files.writeFile(markdown, summaryMarkdown(results));
  const formattedBytes = Buffer.byteLength(formatAsPrettier(text), "utf8");
  if (formattedBytes > ADDED_FILE_LIMIT_BYTES) {
    console.warn(
      `descent spike: ${json} is ${formattedBytes} B once formatted, over the ${ADDED_FILE_LIMIT_BYTES} B the repository accepts for an added file; it is written in full`,
    );
  }
  return { json, markdown };
}
