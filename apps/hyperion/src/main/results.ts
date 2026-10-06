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
 * - The trace is taken in windows of script time (decision-r05-trace-windows.md,
 *   `traceWindows.ts`): the frames at each window's boundary are left out of every per-frame
 *   figure and counted, and a profiled run (`run.trace.profiled`) is a diagnostic, never judged.
 *   Each window's frame spans are checked against the renderer's frames, and the GPU process's
 *   slices are those of the categories recorded (decision-r05-trace-windows-2.md).
 * - Incomplete frames (decision-r05-trace-windows-2.md, addendum B, ruling 1): a frame some of
 *   whose timer resolves never reported is left out of every per-frame GPU sum and counted, and a
 *   GPU row's verdict must hold whatever its times were ({@link boundedGpuRow}).
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
import type { GpuClockReadings, GpuClockSample, GpuClockSource } from "./gpuClocks";
import { type Measured, measured, missing } from "./measured";
import { GPU_PROCESS_SLICES, type GpuProcessFigures, recordedGpuSlices } from "./reduceTrace";
import {
  type EngineFigures,
  FULL_BUFFER_PERCENT,
  type GcFigures,
  type MainThreadSplit,
  mergeTraceWindows,
  SHORT_SPAN_FRACTION,
  type ScriptInterval,
  type TraceRecording,
  type TraceRun,
} from "./traceWindows";

export { type Measured, measured, missing } from "./measured";
export type { GpuClockSource } from "./gpuClocks";

/** The file's schema name and version, which T15.c's replayer writes too. */
export const RESULTS_SCHEMA = "hyperion.descent-spike.results";
/**
 * The schema's version; bumped with any change to the file's shape.
 *
 * @remarks
 * Version 2 stores the memory series as columns of whole KiB (decision-r05-results-size.md); v1's
 * object a sample made a 20-minute run's file larger than the repository accepts. Version 3 takes
 * the trace in windows (decision-r05-trace-windows.md): `run.trace` lists the windows and their
 * boundaries, `frames.excludedFrames` counts the frames left out at the boundaries, and the
 * engine's sampled self time moves from `mainThread.split` to `mainThread.engine`, present in a
 * profiled run only. Version 4 (decision-r05-trace-windows-2.md) adds the trace's `format`, counts
 * `mainThread.split.ourCodeMs` as the union of the per-frame `spike.frame` spans alone, and lists
 * only the GPU-process slices whose category was recorded; a window whose frame spans disagree
 * with the renderer's frames fails. Version 5 (the same decision's addendum B) takes the GPU rows'
 * sums over the frames whose pass times are complete, counts the others in `gpu.incompleteFrames`
 * and bounds the rows' verdicts by them, and adds `gpu.clocks`, the GPU's clocks beside the memory
 * series (R05.T14.k).
 */
export const RESULTS_VERSION = 5;

/**
 * The largest file the repository accepts as added, bytes: pre-commit's `check-added-large-files`
 * refuses a file whose size in KiB, rounded up, is over 500.
 */
export const ADDED_FILE_LIMIT_BYTES = 512_000;

/** The line width Prettier formats the repository's files to (`.prettierrc.json`). */
const PRETTIER_PRINT_WIDTH = 100;

/** Dawn's timestamp quantum, 65,536 ns (`timestamp_quantization`, Design note 18), ms. */
export const TIMESTAMP_QUANTUM_MS = 0.065_536;

/** The rows read from per-frame GPU times, whose verdicts the incomplete frames bound. */
export const GPU_ROW_IDS: ReadonlySet<string> = new Set(["headroom-gpu", "terrain", "atmosphere"]);

/** The percentile every GPU row reads (Design note 21). */
const GPU_ROW_PERCENTILE = 0.95;

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
  const value = sorted[rankIndex(sorted.length, p)];
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

/**
 * The index {@link nearestRank} reads among `count` ascending values at percentile `p`, from 0;
 * the schema check reads the same, so that it agrees with the writer to the last rank.
 */
function rankIndex(count: number, p: number): number {
  return Math.min(count - 1, Math.max(0, Math.ceil(p * count) - 1));
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
  /** The GPU's clocks at the same instant (R05.T14.k), or why the run's GPU has no reading. */
  readonly clocks: GpuClockSample;
}

/** Consecutive samples at which a reading is missing for one reason. */
export interface MemoryGap {
  /** The first sample's index. */
  readonly from: number;
  /** The last sample's index, inclusive. */
  readonly to: number;
  readonly reason: string;
}

/**
 * One reading over a run, a value a sample: memory in whole KiB, or the GPU's clocks in whole MHz
 * and its performance state's number (`GpuClocks`), the unit the field's name gives.
 */
export interface MemoryColumn {
  /** The reading at each sample, a whole number in its unit, or -1 within one of the gaps. */
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
 * One reading's column, in the reading's own whole unit.
 *
 * @param readingOf - The reading at a sample, a whole number, or why it is missing there.
 * @returns The column, or `null` with every reason given when no sample has the reading.
 */
function readingColumn(
  memory: ReadonlyArray<MemorySample>,
  readingOf: (sample: MemorySample) => Measured<number>,
): Measured<MemoryColumn> {
  if (memory.length === 0) {
    return missing(NO_MEMORY_SAMPLE);
  }
  const samples: number[] = [];
  const gaps: Array<{ from: number; to: number; reason: string }> = [];
  memory.forEach((sample, i) => {
    const reading = readingOf(sample);
    if (reading.value !== null) {
      samples.push(reading.value);
      return;
    }
    samples.push(-1);
    const reason = reading.reason;
    const last = gaps.at(-1);
    if (last !== undefined && last.to === i - 1 && last.reason === reason) {
      last.to = i;
    } else {
      gaps.push({ from: i, to: i, reason });
    }
  });
  if (samples.every((reading) => reading === -1)) {
    return missing([...new Set(gaps.map(({ reason }) => reason))].join("; "));
  }
  return measured({ samples, gaps });
}

/**
 * One memory reading's column, in whole KiB.
 *
 * @param bytesOf - The reading at a sample, bytes, or `null` where it is missing.
 * @param reasonOf - Why a sample has no reading.
 */
function memoryColumn(
  memory: ReadonlyArray<MemorySample>,
  bytesOf: (sample: MemorySample) => number | null,
  reasonOf: (sample: MemorySample) => string,
): Measured<MemoryColumn> {
  return readingColumn(memory, (sample) => {
    const bytes = bytesOf(sample);
    // Exact for every source, which reports KiB or MiB.
    return bytes === null ? missing(reasonOf(sample)) : measured(Math.round(bytes / 1024));
  });
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

/**
 * The results file's GPU clocks, built from the sampler's readings on the memory series' times.
 *
 * @remarks
 * The source is the first reading's, which the reader keeps for the run; a sample whose clocks
 * are missing is a gap in each column with its reason. Without a graphics clock at any sample the
 * figure is missing with that reason. The maximum is the largest any sample read.
 */
export function gpuClocksOf(memory: ReadonlyArray<MemorySample>): Measured<GpuClocks> {
  if (memory.length === 0) {
    return missing(NO_MEMORY_SAMPLE);
  }
  const read = memory.flatMap(({ clocks }) => (clocks.kind === "clocks" ? [clocks] : []));
  const first = read[0];
  if (first === undefined) {
    const reasons = memory.flatMap(({ clocks }) =>
      clocks.kind === "unavailable" ? [clocks.reason] : [],
    );
    return missing([...new Set(reasons)].join("; "));
  }
  const { source } = first;
  const readingOf =
    (pickReading: (clocks: GpuClockReadings) => Measured<number>) =>
    ({ clocks }: MemorySample): Measured<number> => {
      if (clocks.kind === "unavailable") {
        return missing(clocks.reason);
      }
      return clocks.source === source
        ? pickReading(clocks)
        : missing(`read from ${clocks.source}, not the run's ${source}`);
    };
  const graphicsMHz = readingColumn(
    memory,
    readingOf(({ graphicsMHz: mhz }) => mhz),
  );
  if (graphicsMHz.value === null) {
    return missing(graphicsMHz.reason);
  }
  let maxMHz: number | null = null;
  for (const clocks of read) {
    const mhz = clocks.source === source ? clocks.maxGraphicsMHz.value : null;
    if (mhz !== null) {
      maxMHz = Math.max(maxMHz ?? mhz, mhz);
    }
  }
  return measured({
    source,
    maxGraphicsMHz:
      maxMHz === null ? missing(first.maxGraphicsMHz.reason ?? "no maximum") : measured(maxMHz),
    graphicsMHz,
    memoryMHz: readingColumn(
      memory,
      readingOf(({ memoryMHz: mhz }) => mhz),
    ),
    performanceState: readingColumn(
      memory,
      readingOf(({ performanceState: state }) => state),
    ),
  });
}

/** The readings of a column at or after `fromMs` on `tMs`, gaps left out, ascending. */
function readingsFrom(
  column: Measured<MemoryColumn>,
  tMs: ReadonlyArray<number>,
  fromMs: number,
): number[] {
  if (column.value === null) {
    return [];
  }
  return column.value.samples
    .filter((reading, i) => reading >= 0 && (tMs[i] ?? Number.NEGATIVE_INFINITY) >= fromMs)
    .toSorted((a, b) => a - b);
}

/** The fraction of the maximum graphics clock below which a GPU row says what it was measured at. */
export const CLOCK_NOTE_FRACTION = 0.9;

/**
 * The note a GPU row carries when the median graphics clock after the warm-up was below
 * {@link CLOCK_NOTE_FRACTION} of the maximum (addendum B, ruling 2), or `null`.
 *
 * @remarks
 * The memory series' times count from the measuring's start, which is within a few hundred
 * milliseconds of script time 0 (the first trace window's start), so the warm-up is read on them
 * as it is in script time.
 *
 * @param tMs - The memory series' times, ms.
 * @param warmupS - The warm-up, s.
 */
export function clockNote(
  clocks: Measured<GpuClocks>,
  tMs: ReadonlyArray<number>,
  warmupS: number,
): string | null {
  const maxMHz = clocks.value?.maxGraphicsMHz.value ?? null;
  if (clocks.value === null || maxMHz === null) {
    return null;
  }
  const after = readingsFrom(clocks.value.graphicsMHz, tMs, 1000 * warmupS);
  if (after.length === 0) {
    return null;
  }
  const medianMHz = nearestRank(after, 0.5);
  return medianMHz < CLOCK_NOTE_FRACTION * maxMHz
    ? `measured at a median ${medianMHz} of ${maxMHz} MHz (the driver's choice at this load)`
    : null;
}

/** `entry` with `note` added after any note it has; unchanged when it has no value or no note. */
function withNote(entry: Criterion, note: string | null): Criterion {
  if (note === null || entry.value === null) {
    return entry;
  }
  return { ...entry, note: entry.note === null ? note : `${entry.note}; ${note}` };
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
  /** The GPU's clocks, given the sample's own `nvidia-smi` reading (`GpuClockReader.read`). */
  clocks(nvidia: NvidiaReading): Promise<GpuClockSample>;
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
  // The clocks' reader is not expected to reject; a failure costs this sample's clocks alone.
  const clocks = await sources.clocks(nvidia).catch((error: unknown): GpuClockSample => ({
    kind: "unavailable",
    reason: `the GPU's clocks could not be read: ${error instanceof Error ? error.message : String(error)}`,
  }));
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
    clocks,
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

/**
 * The frames whose pass times are incomplete (decision-r05-trace-windows-2.md, addendum B): some
 * timer resolve a frame numbered never reported its times, because the timer dropped it while
 * every read-back buffer was in flight or its read failed.
 */
export interface IncompleteFrames {
  /** Frames none of whose resolves reported. */
  readonly dropped: number;
  /** Frames some of whose resolves reported and some never did. */
  readonly partial: number;
  /**
   * The frames counted: those after the warm-up and outside the trace's boundary exclusions,
   * which every per-frame figure reads; a native replay's are its frames.
   */
  readonly frames: number;
}

/**
 * The GPU's clocks at 1 Hz on the memory series' times, as the driver chose them: a GPU row
 * measures each pass at them, never pinned or normalised (addendum B, ruling 2; R05.T14.k).
 */
export interface GpuClocks {
  readonly source: GpuClockSource;
  /** The GPU's maximum graphics clock, whole MHz: the largest any sample read. */
  readonly maxGraphicsMHz: Measured<number>;
  /** The graphics clock at each sample, whole MHz, in the memory series' column form. */
  readonly graphicsMHz: Measured<MemoryColumn>;
  /** The memory clock at each sample, whole MHz. */
  readonly memoryMHz: Measured<MemoryColumn>;
  /** The performance state at each sample, its whole number (P0 is 0). */
  readonly performanceState: Measured<MemoryColumn>;
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
    /** The trace's windows and boundaries, or why the run has no trace. */
    readonly trace: Measured<TraceRun>;
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
      /** The segment's frames left out at the trace's window boundaries. */
      readonly excludedFrames: number;
    }>;
    /** Frames Chromium dropped, after the warm-up and outside every boundary's exclusion. */
    readonly dropped: Measured<number>;
    /**
     * Frames after the warm-up left out at the trace's window boundaries, beside the warm-up's,
     * from every per-frame figure: the rAF and presentation statistics, the pass times, their sums
     * and our code's.
     */
    readonly excludedFrames: number;
  };
  readonly gpu: {
    readonly timer: SpikePassTimer;
    /** The tolerance of one pass's time, ms: the quantum on a `quantized` timer, else 0. */
    readonly tolerancePerPassMs: number;
    readonly untimedPasses: number;
    /** Each pass's times over every frame they arrived for, complete or not. */
    readonly passes: Measured<ReadonlyArray<PassFigures>>;
    /** The sum of a frame's timed passes, at the 95th percentile, over the complete frames, ms. */
    readonly sumP95Ms: Measured<number>;
    /** The frames left out of every per-frame GPU sum, their pass times incomplete. */
    readonly incompleteFrames: Measured<IncompleteFrames>;
    /**
     * The GPU process's main thread: the CPU side of Chromium's command transport and Dawn. Its
     * slices are only those whose category was recorded: `GPUTask` always, `WebGPU` and
     * `VulkanQueueSubmitHook` with `gpu` (a profiled run).
     */
    readonly gpuProcess: Measured<GpuProcessFigures>;
    /** The GPU's clocks beside the memory series, or why they are missing. */
    readonly clocks: Measured<GpuClocks>;
  };
  readonly mainThread: {
    /** Our code's time a frame (`performance.measure`), at the 95th percentile, ms. */
    readonly ourCodeP95Ms: Measured<number>;
    /**
     * The renderer main thread's wall, busy, our-code and idle time, summed over the windows. Our
     * code is the union of its per-frame `spike.frame` spans.
     */
    readonly split: Measured<MainThreadSplit>;
    /** The engine adapter's sampled self time: a profiled run's alone. */
    readonly engine: Measured<EngineFigures>;
    readonly gc: Measured<ReadonlyArray<GcFigures>>;
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
  /** The trace's windows, each reduced, or why there is no trace. */
  readonly trace: Measured<TraceRecording>;
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

/** A row of a criterion: its limit and value, judged; `not-measured` where either is missing. */
export function row(
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

/**
 * A GPU row's incomplete frames: how many, and how many of them are incomplete only because their
 * reads were still in flight when the report was taken.
 */
export interface IncompleteCount {
  readonly frames: number;
  /** Frames every missing resolve of which was still being read at the report, at most `frames`. */
  readonly inFlight: number;
}

/** No incomplete frame. */
const NO_INCOMPLETE: IncompleteCount = { frames: 0, inFlight: 0 };

/**
 * Why a frame's pass times are missing when its reads were still in flight at the report: the
 * controller waits for them up to 1 s after the trace's last stop (R05.T14.k, the orchestrator's
 * ruling on T14.j's open question), not the GPU's backlog of addendum B.
 */
export const READ_IN_FLIGHT_REASON = "read in flight at the report";

/** Why the timer dropped a resolve: the GPU's backlog (addendum B, ruling 1). */
const GPU_BACKLOG_REASON = "the GPU was more than 27 frames behind";

/**
 * Why a GPU row is not measured when its incomplete frames could carry its verdict either way
 * (decision-r05-trace-windows-2.md, addendum B, ruling 1): the GPU's backlog, or the reads still
 * in flight at the report, or both with their counts.
 */
export function incompleteFramesReason({ frames, inFlight }: IncompleteCount): string {
  const backlog = frames - inFlight;
  const cause =
    inFlight === 0
      ? GPU_BACKLOG_REASON
      : backlog === 0
        ? READ_IN_FLIGHT_REASON
        : `${backlog} as ${GPU_BACKLOG_REASON}, ${inFlight} ${READ_IN_FLIGHT_REASON}`;
  return `${frames} ${frames === 1 ? "frame's" : "frames'"} pass times were incomplete (${cause})`;
}

/**
 * A GPU row's figure: the 95th percentile of `values`, or why there is none.
 *
 * @param noValue - Why there is no value when there is no complete frame and no incomplete one.
 */
function gpuPercentile(
  values: ReadonlyArray<number>,
  incomplete: IncompleteCount,
  noValue: string,
): Measured<number> {
  if (values.length === 0) {
    return missing(incomplete.frames > 0 ? incompleteFramesReason(incomplete) : noValue);
  }
  return measured(
    nearestRank(
      values.toSorted((a, b) => a - b),
      GPU_ROW_PERCENTILE,
    ),
  );
}

/**
 * A GPU row, the 95th percentile of its per-frame figure over the complete frames, judged so that
 * its verdict holds whatever times its incomplete frames had (decision-r05-trace-windows-2.md,
 * addendum B, ruling 1).
 *
 * @remarks
 * The row passes only if it still passes with every incomplete frame placed above its limit, and
 * fails only if it still fails with every one placed below it; otherwise it is not measured, with
 * their count as its reason. A drop proves the GPU fell more than 27 frames behind, so the frames
 * a drop hides are the slow ones: leaving them out alone would bias the row towards passing. When
 * both placements are `marginal` (a quantized timer), every placement between is too, so the row
 * stays marginal. With no incomplete frame the verdict is {@link row}'s. The value written is the
 * percentile over the complete frames.
 *
 * @param limit - The row's limit, ms.
 * @param valuesMs - The row's figure in each complete frame that has one.
 * @param incomplete - The frames whose pass times are incomplete, which `valuesMs` leaves out.
 * @param toleranceMs - The timer's tolerance on the value.
 * @param noValue - Why there is no value when there is no complete frame and no incomplete one.
 */
export function boundedGpuRow(
  id: string,
  criterion: string,
  limit: Measured<number>,
  valuesMs: ReadonlyArray<number>,
  incomplete: IncompleteCount,
  toleranceMs: number,
  noValue: string,
): Criterion {
  const plain = row(
    id,
    criterion,
    limit,
    "ms",
    gpuPercentile(valuesMs, incomplete, noValue),
    toleranceMs,
  );
  const count = incomplete.frames;
  if (count === 0 || plain.value === null || plain.limit === null) {
    return plain;
  }
  const placed = (ms: number): number[] => Array.from({ length: count }, () => ms);
  const sorted = valuesMs.toSorted((a, b) => a - b);
  const above = judge(
    nearestRank([...sorted, ...placed(Infinity)], GPU_ROW_PERCENTILE),
    plain.limit,
    toleranceMs,
  );
  const below = judge(
    nearestRank([...placed(Number.NEGATIVE_INFINITY), ...sorted], GPU_ROW_PERCENTILE),
    plain.limit,
    toleranceMs,
  );
  if (above !== below) {
    return { ...plain, verdict: "not-measured", note: incompleteFramesReason(incomplete) };
  }
  const inFlight =
    incomplete.inFlight === 0 ? "" : ` (${incomplete.inFlight} ${READ_IN_FLIGHT_REASON})`;
  return {
    ...plain,
    verdict: above,
    note: `${count} ${count === 1 ? "frame" : "frames"} with incomplete pass times left out${inFlight}; the verdict holds whatever their times`,
  };
}

/** A limit that depends on T, or T's absence. */
export function ofPeriod(
  periodMs: Measured<number>,
  limit: (t: number) => number,
): Measured<number> {
  return periodMs.value === null ? missing(periodMs.reason) : measured(limit(periodMs.value));
}

/** The frame rows of Design note 21 for one set of intervals. */
export function frameRows(
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

/** A criterion's overall verdict: `fail` over `not-measured` over `marginal` over `pass`. */
export function overallOf(criteria: ReadonlyArray<Criterion>): Verdict {
  const verdicts = new Set(criteria.map(({ verdict }) => verdict));
  if (verdicts.has("fail")) {
    return "fail";
  }
  if (verdicts.has("not-measured")) {
    return "not-measured";
  }
  return verdicts.has("marginal") ? "marginal" : "pass";
}

/**
 * The indices of frames at or after the warm-up, outside every one of the trace's exclusions, and
 * within `[startS, endS)` when given.
 */
function frameIndices(
  scriptTimesS: ReadonlyArray<number>,
  warmupS: number,
  isExcluded: (t: number) => boolean,
  span?: { readonly startS: number; readonly endS: number },
): number[] {
  const indices: number[] = [];
  scriptTimesS.forEach((t, i) => {
    if (
      t >= warmupS &&
      !isExcluded(t) &&
      (span === undefined || (t >= span.startS && t < span.endS))
    ) {
      indices.push(i);
    }
  });
  return indices;
}

/** The frames at or after the warm-up that an exclusion leaves out, within `span` when given. */
function excludedCount(
  scriptTimesS: ReadonlyArray<number>,
  warmupS: number,
  isExcluded: (t: number) => boolean,
  span?: { readonly startS: number; readonly endS: number },
): number {
  return scriptTimesS.filter(
    (t) =>
      t >= warmupS && isExcluded(t) && (span === undefined || (t >= span.startS && t < span.endS)),
  ).length;
}

/** Whether a script time lies in one of `exclusions`. */
function excludedBy(exclusions: ReadonlyArray<ScriptInterval>): (t: number) => boolean {
  return (t) => exclusions.some(({ fromS, toS }) => t >= fromS && t < toS);
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
 * The trace's windows are merged first (`mergeTraceWindows`): the frames in a boundary's exclusion
 * are left out of every per-frame figure, and a failed window, one with no timed event among them
 * (`EMPTY_TRACE_REASON`), makes every figure read from the trace missing with its reason. A frame
 * the renderer reports missing resolves for is left out of the GPU rows' sums and counted, and
 * the rows are judged by {@link boundedGpuRow}; a frame whose missing reads were all still in
 * flight at the report is counted with that reason ({@link READ_IN_FLIGHT_REASON}). The GPU's
 * clocks come from the memory samples, and a GPU row measured at a median clock below 90% of the
 * maximum says so ({@link clockNote}).
 */
export function buildResults(input: ResultsInput): DescentResults {
  const { run, report, memory } = input;
  const merged = mergeTraceWindows(input.trace, report);
  const pooled = merged.figures;
  const isExcluded = excludedBy(merged.exclusions);
  const setting = run.setting;
  const vsyncMs = run.displayHz === null || run.displayHz <= 0 ? null : 1000 / run.displayHz;
  const periodMs: Measured<number> = !run.shown
    ? missing("no window shown")
    : vsyncMs === null
      ? missing("the display reports no refresh rate")
      : measured(setting === "low" ? 2 * vsyncMs : vsyncMs);
  const t = periodMs.value;

  // Frames: presentation times from the trace, else the rAF timestamps; the frames at the trace's
  // window boundaries left out of both, as the warm-up's are.
  const frames = report.frames;
  const warm = frameIndices(frames.scriptTimesS, report.warmupS, isExcluded);
  const rafWhole = statsOrMissing(
    pick(frames.rafIntervalsMs, warm),
    t,
    "no frame after the warm-up",
  );
  const presentationReason = !run.shown
    ? "no window shown"
    : pooled.value === null
      ? pooled.reason
      : "no presented frame after the warm-up";
  const presented = run.shown ? pooled.value : null;
  const presentationWhole =
    presented === null
      ? missing(presentationReason)
      : statsOrMissing(presented.presentationMs, t, presentationReason);
  const source = presentationWhole.value === null ? "raf" : "presentation";
  const segments = report.segments.map((span) => {
    const indices = frameIndices(frames.scriptTimesS, report.warmupS, isExcluded, span);
    const raf = statsOrMissing(
      pick(frames.rafIntervalsMs, indices),
      t,
      "no frame in the segment after the warm-up",
    );
    const presentation =
      presented === null
        ? missing(presentationReason)
        : statsOrMissing(
            presented.presentationBySegmentMs.get(span.name) ?? [],
            t,
            "no presented frame in the segment",
          );
    return {
      segment: span.name,
      presentation,
      raf,
      excludedFrames: excludedCount(frames.scriptTimesS, report.warmupS, isExcluded, span),
    };
  });
  const dropped = pooled.value === null ? missing(pooled.reason) : measured(pooled.value.dropped);

  const tolerancePerPassMs = report.timer === "quantized" ? TIMESTAMP_QUANTUM_MS : 0;
  const timerReason = "the pass timer is absent (no timestamp-query)";
  const timed = report.timer !== "absent";
  // Incomplete frames (addendum B, ruling 1): a frame with a resolve that never reported leaves
  // every per-frame sum, and is counted, dropped (no pass time at all) or partial, among the
  // frames those sums read. Each pass's own percentiles keep every time that arrived.
  const complete: number[] = [];
  let droppedTimes = 0;
  let partialTimes = 0;
  let inFlightTimes = 0;
  for (const i of warm) {
    const missingResolves = frames.missingResolves[i] ?? 0;
    if (missingResolves === 0) {
      complete.push(i);
      continue;
    }
    // A frame with a resolve the timer dropped is the backlog's, whatever else is in flight.
    if ((frames.inFlightResolves[i] ?? 0) >= missingResolves) {
      inFlightTimes += 1;
    }
    if (frames.passes.some((pass) => (pass.gpuMs[i] ?? null) !== null)) {
      partialTimes += 1;
    } else {
      droppedTimes += 1;
    }
  }
  const incomplete: IncompleteCount = timed
    ? { frames: droppedTimes + partialTimes, inFlight: inFlightTimes }
    : NO_INCOMPLETE;
  const incompleteFrames: Measured<IncompleteFrames> = timed
    ? measured({ dropped: droppedTimes, partial: partialTimes, frames: warm.length })
    : missing(timerReason);
  const passes: Measured<PassFigures[]> = !timed
    ? missing(timerReason)
    : measured(
        frames.passes.flatMap((pass) => {
          const arrived = pick(pass.gpuMs, warm).filter((ms): ms is number => ms !== null);
          const stats = frameStats(arrived, null);
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
    if (!timed) {
      return { values, maxPasses };
    }
    for (const i of complete) {
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
  const sumP95Ms = gpuPercentile(allSums.values, incomplete, timed ? "no timed pass" : timerReason);
  const terrain = sums("terrain");
  const atmosphere = sums("atmosphere");

  const ourCodeP95Ms = p95Of(pick(frames.ourCodeMs, warm), "no frame after the warm-up");

  const series = memorySeries(memory);
  const clocks = gpuClocksOf(memory);
  const clockMeasured = clockNote(clocks, series.tMs, report.warmupS);
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
    ...[
      boundedGpuRow(
        "headroom-gpu",
        "GPU pass sum ≤ 0.8 T at the 95th percentile",
        headroomLimit,
        allSums.values,
        incomplete,
        allSums.maxPasses * tolerancePerPassMs,
        timed ? "no timed pass" : timerReason,
      ),
      boundedGpuRow(
        "terrain",
        `terrain GPU time ≤ ${limits.terrainMs} ms at the 95th percentile`,
        measured(limits.terrainMs),
        terrain.values,
        incomplete,
        terrain.maxPasses * tolerancePerPassMs,
        timed ? "no timed terrain pass" : timerReason,
      ),
      boundedGpuRow(
        "atmosphere",
        `atmosphere GPU time ≤ ${limits.atmosphereMs} ms at the 95th percentile`,
        measured(limits.atmosphereMs),
        atmosphere.values,
        incomplete,
        atmosphere.maxPasses * tolerancePerPassMs,
        timed ? "no timed atmosphere pass" : timerReason,
      ),
    ].map((entry) => withNote(entry, clockMeasured)),
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
      trace: merged.run,
    },
    levels: report.levels,
    frames: {
      source,
      presentation: presentationWhole,
      raf: rafWhole,
      segments,
      dropped,
      excludedFrames: excludedCount(frames.scriptTimesS, report.warmupS, isExcluded),
    },
    gpu: {
      timer: report.timer,
      tolerancePerPassMs,
      untimedPasses: report.untimedPasses,
      passes,
      sumP95Ms,
      incompleteFrames,
      gpuProcess: pooled.value === null ? missing(pooled.reason) : pooled.value.gpuProcess,
      clocks,
    },
    mainThread: {
      ourCodeP95Ms,
      split: pooled.value === null ? missing(pooled.reason) : pooled.value.split,
      engine: pooled.value === null ? missing(pooled.reason) : pooled.value.engine,
      gc: pooled.value === null ? missing(pooled.reason) : measured(pooled.value.gc),
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

/**
 * The active GPU of Chromium's basic GPU information (`app.getGPUInfo("basic")`), narrowed from
 * what Electron returns: the run's GPU, whose clocks `GpuClockReader` reads.
 */
export function activeGpu(info: unknown): Measured<GpuDescription> {
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
 * every figure must be present or `null` with a stated reason; without a trace, or with a failed
 * window, no figure read from the trace may be present, nor the engine's in an unprofiled run; the
 * trace's windows must be in order with one boundary between each pair, whose exclusions neither
 * overlap nor fall out of order and whose frames are the file's excluded frames; the memory
 * series must be whole: every column as long as the times, readings in whole KiB, -1 at its gaps'
 * samples alone, and each peak its column's maximum; the incomplete frames must be whole counts
 * within the frames after the warm-up, and no GPU row's verdict one they make impossible; and each
 * clock column must be as long as the times.
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
  const trace = checkTrace(childAt(value, ["run", "trace"]), problems);
  checkTraceFigures(value, trace, problems);
  checkExcludedFrames(value, trace, problems);
  checkMainThread(value, trace, problems);
  checkGpuSlices(value, trace, problems);
  checkIncompleteFrames(value, problems);
  checkClocks(value, problems);
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
 * One column of the memory series, or of the GPU's clocks on its times.
 *
 * @param count - The number of samples, `tMs`'s length.
 * @param unit - The readings' unit, in the problems' words.
 * @returns The column's largest reading, in its unit; `null` for a column with no reading all run;
 * `undefined` when the column is malformed, its problem added to `problems`.
 */
function checkColumn(
  column: unknown,
  path: string,
  count: number,
  problems: string[],
  unit = "KiB",
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
  let maxReading = -1;
  for (const [i, sample] of samples.entries()) {
    const reading: unknown = sample;
    if (missingAt.has(i) ? reading !== -1 : !isWhole(reading, 0)) {
      problems.push(
        missingAt.has(i)
          ? `${path}.value.samples[${i}] is within a gap but not -1`
          : reading === -1
            ? `${path}.value.samples[${i}] is -1 outside a gap`
            : `${path}.value.samples[${i}] is not a whole number of ${unit}`,
      );
      return undefined;
    }
    if (typeof reading === "number") {
      maxReading = Math.max(maxReading, reading);
    }
  }
  if (maxReading === -1) {
    problems.push(`${path} has no reading: a reading missing all run is null with its reason`);
    return undefined;
  }
  return maxReading;
}

/** The figures {@link buildResults} reads from the trace alone, by path. */
const TRACE_FIGURE_PATHS: ReadonlyArray<ReadonlyArray<string>> = [
  ["frames", "dropped"],
  ["gpu", "gpuProcess"],
  ["mainThread", "split"],
  ["mainThread", "engine"],
  ["mainThread", "gc"],
];

function childAt(node: unknown, path: ReadonlyArray<string>): unknown {
  return path.reduce<unknown>((child, key) => (isRecord(child) ? child[key] : undefined), node);
}

/** What the rest of the check needs of `run.trace`. */
interface TraceState {
  /**
   * Why no figure read from the trace may be present, as the end of "<figure> is measured…", or
   * `null` when they may be.
   */
  readonly withoutFigures: string | null;
  /** Whether the run is profiled; `false` without a trace. */
  readonly profiled: boolean;
  /** The categories recorded, or `null` without a trace or a list of them. */
  readonly categories: ReadonlyArray<string> | null;
  /** The frames its boundaries leave out; 0 without a trace. */
  readonly boundaryFrames: number;
}

/** The trace's formats, as `run.trace.value.format` names them. */
const TRACE_FORMATS: ReadonlySet<unknown> = new Set<TraceRun["format"]>(["json", "perfetto-proto"]);

/** Times written from one computation, compared to a nanosecond. */
function sameS(a: unknown, b: unknown): boolean {
  return typeof a === "number" && typeof b === "number" && Math.abs(a - b) <= 1e-9;
}

function isFiniteAtLeast(value: unknown, min: number): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= min;
}

/** One window's figures, if it has them: its span, file size and buffer use, against its length. */
function checkWindowFigures(
  window: Readonly<Record<string, unknown>>,
  path: string,
  problems: string[],
): void {
  const figures = childAt(window, ["figures", "value"]);
  if (!isRecord(figures)) {
    return;
  }
  const { spanMs, bytes, bufferPercent } = figures;
  if (
    !isFiniteAtLeast(spanMs, 0) ||
    !isWhole(bytes, 0) ||
    (bufferPercent !== null && !isFiniteAtLeast(bufferPercent, 0))
  ) {
    problems.push(`${path}.figures is not a span, a size and a buffer's use`);
    return;
  }
  const { fromS, toS } = window;
  const recordedMs =
    typeof fromS === "number" && typeof toS === "number" ? 1000 * (toS - fromS) : 0;
  if (
    spanMs < SHORT_SPAN_FRACTION * recordedMs ||
    (bufferPercent !== null && bufferPercent >= FULL_BUFFER_PERCENT)
  ) {
    problems.push(`${path} filled its buffer, but is not marked failed`);
  }
}

/**
 * `run.trace`: its settings, its windows in order, and one boundary between each pair at their
 * times, whose exclusions are in order and do not overlap.
 */
function checkTrace(trace: unknown, problems: string[]): TraceState {
  if (!isRecord(trace)) {
    problems.push("run.trace is missing");
    return {
      withoutFigures: " without run.trace",
      profiled: false,
      categories: null,
      boundaryFrames: 0,
    };
  }
  const value = trace["value"];
  if (!isRecord(value)) {
    if (value !== null) {
      problems.push("run.trace.value is not a trace");
    }
    return {
      withoutFigures: ` without a trace (${String(trace["reason"])})`,
      profiled: false,
      categories: null,
      boundaryFrames: 0,
    };
  }
  const path = "run.trace.value";
  const { format, profiled, categories, recordingMode, bufferKb, guardS, tracedS } = value;
  if (!TRACE_FORMATS.has(format)) {
    problems.push(`${path}.format is neither json nor perfetto-proto`);
  }
  const recorded =
    Array.isArray(categories) &&
    categories.every((category: unknown): category is string => typeof category === "string")
      ? categories
      : null;
  if (
    typeof profiled !== "boolean" ||
    recorded === null ||
    typeof recordingMode !== "string" ||
    !isWhole(bufferKb, 0) ||
    !isFiniteAtLeast(guardS, 0) ||
    !isFiniteAtLeast(tracedS, 0)
  ) {
    problems.push(`${path} does not say how the trace was recorded`);
  }
  const windows = Array.isArray(value["windows"]) ? value["windows"] : [];
  const boundaries = Array.isArray(value["boundaries"]) ? value["boundaries"] : [];
  if (windows.length === 0) {
    problems.push(`${path}.windows is not a list of windows`);
  }
  let failed: string | null = null;
  for (const [i, window] of windows.entries()) {
    const at = `${path}.windows[${i}]`;
    if (
      !isRecord(window) ||
      window["index"] !== i ||
      typeof window["fromS"] !== "number" ||
      typeof window["toS"] !== "number" ||
      window["fromS"] > window["toS"] ||
      !isRecord(window["figures"])
    ) {
      problems.push(`${at} is not a window from its start to its stop`);
      continue;
    }
    const beforeToS = childAt(windows[i - 1], ["toS"]);
    if (typeof beforeToS === "number" && window["fromS"] < beforeToS) {
      problems.push(`${at} begins before the window before it stops`);
    }
    if (failed === null && childAt(window, ["figures", "value"]) === null) {
      failed = `, but ${at} failed`;
    }
    checkWindowFigures(window, at, problems);
  }
  if (boundaries.length !== Math.max(windows.length - 1, 0)) {
    problems.push(
      `${path} has ${boundaries.length} boundaries for ${windows.length} windows, not one between each pair`,
    );
  }
  let boundaryFrames = 0;
  for (const [k, boundary] of boundaries.entries()) {
    const at = `${path}.boundaries[${k}]`;
    if (!isRecord(boundary)) {
      problems.push(`${at} is not a boundary`);
      continue;
    }
    const { afterWindow, stopRequestedS, resumedS, excludedToS, excludedFrames } = boundary;
    const maxRaf = boundary["maxRafIntervalMs"];
    if (
      afterWindow !== k ||
      !sameS(stopRequestedS, childAt(windows[k], ["toS"])) ||
      !sameS(resumedS, childAt(windows[k + 1], ["fromS"])) ||
      typeof resumedS !== "number" ||
      !sameS(excludedToS, resumedS + (typeof guardS === "number" ? guardS : Number.NaN))
    ) {
      problems.push(
        `${at} is not the gap between windows ${k} and ${k + 1}, excluded to the guard after it`,
      );
    }
    const beforeToS = childAt(boundaries[k - 1], ["excludedToS"]);
    if (
      typeof stopRequestedS === "number" &&
      typeof beforeToS === "number" &&
      stopRequestedS < beforeToS
    ) {
      problems.push(`${at}'s exclusion begins before the one before it ends`);
    }
    if (!isWhole(excludedFrames, 0) || (maxRaf !== null && !isFiniteAtLeast(maxRaf, 0))) {
      problems.push(`${at} does not count its frames and their largest interval`);
      continue;
    }
    boundaryFrames += excludedFrames;
  }
  return {
    withoutFigures: failed,
    profiled: profiled === true,
    categories: recorded,
    boundaryFrames,
  };
}

/**
 * Without a trace (`run.trace` null), or with a failed window, every figure read from the trace is
 * null too, so that an empty trace's zeros cannot pass for measurements. A native replay times its
 * presentations without a trace, so only its other trace figures are checked.
 */
function checkTraceFigures(
  value: Readonly<Record<string, unknown>>,
  trace: TraceState,
  problems: string[],
): void {
  if (trace.withoutFigures === null) {
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
      problems.push(`${path} is measured${trace.withoutFigures}`);
    }
  }
}

/** The frames left out at the boundaries: the boundaries' count, and each segment's within it. */
function checkExcludedFrames(
  value: Readonly<Record<string, unknown>>,
  trace: TraceState,
  problems: string[],
): void {
  const whole = childAt(value, ["frames", "excludedFrames"]);
  if (!isWhole(whole, 0)) {
    problems.push("frames.excludedFrames is not a count");
    return;
  }
  if (whole !== trace.boundaryFrames) {
    problems.push(
      `frames.excludedFrames is ${whole}, not the ${trace.boundaryFrames} the boundaries left out`,
    );
  }
  const segments = childAt(value, ["frames", "segments"]);
  let inSegments = 0;
  for (const [i, segment] of (Array.isArray(segments) ? segments : []).entries()) {
    const count = childAt(segment, ["excludedFrames"]);
    if (!isWhole(count, 0)) {
      problems.push(`frames.segments[${i}].excludedFrames is not a count`);
      return;
    }
    inSegments += count;
  }
  if (inSegments > whole) {
    problems.push("the segments' excluded frames are more than the whole run's");
  }
}

/** The main thread's split without the engine's figures, which are a profiled run's alone. */
function checkMainThread(
  value: Readonly<Record<string, unknown>>,
  trace: TraceState,
  problems: string[],
): void {
  const split = childAt(value, ["mainThread", "split", "value"]);
  if (isRecord(split) && ("engineSelfMs" in split || "sampledMs" in split)) {
    problems.push("mainThread.split holds the engine's figures, which belong in mainThread.engine");
  }
  const engine = childAt(value, ["mainThread", "engine"]);
  if (!isRecord(engine)) {
    problems.push("mainThread.engine is missing");
    return;
  }
  const figures = engine["value"];
  if (figures === null) {
    return;
  }
  if (
    !isRecord(figures) ||
    !isFiniteAtLeast(figures["selfMs"], 0) ||
    !isFiniteAtLeast(figures["sampledMs"], 0)
  ) {
    problems.push("mainThread.engine is not a sampled self time");
  }
  if (trace.withoutFigures === null && !trace.profiled) {
    problems.push("mainThread.engine is measured in an unprofiled run");
  }
}

/**
 * The GPU process's slices: exactly those whose category the trace recorded, so that a name not
 * recorded is absent, never a count of 0.
 */
function checkGpuSlices(
  value: Readonly<Record<string, unknown>>,
  trace: TraceState,
  problems: string[],
): void {
  const slices = childAt(value, ["gpu", "gpuProcess", "value", "slices"]);
  if (!Array.isArray(slices) || trace.categories === null) {
    return;
  }
  const path = "gpu.gpuProcess.value.slices";
  const names = slices.map((slice: unknown) => childAt(slice, ["name"]));
  const expected = recordedGpuSlices(trace.categories);
  for (const name of names) {
    const known = GPU_PROCESS_SLICES.find((slice) => slice.name === name);
    if (known === undefined) {
      problems.push(`${path} holds ${String(name)}, which the reducer does not summarise`);
    } else if (!expected.includes(known.name)) {
      problems.push(
        `${path} holds ${known.name}, but its category ${known.category} was not recorded`,
      );
    }
  }
  for (const name of expected) {
    if (!names.includes(name)) {
      problems.push(`${path} lacks ${name}, whose category was recorded`);
    }
  }
}

/**
 * `gpu.incompleteFrames`: whole counts, dropped and partial within the frames counted, which in a
 * client's file are the frames its rAF figures read (after the warm-up, outside the trace's
 * exclusions); and no GPU row with a verdict the count makes impossible under ruling 1 of
 * decision-r05-trace-windows-2.md's addendum B.
 *
 * @remarks
 * With k incomplete frames among F, a row's percentile is over m = n + k values, n ≥ 1 of them
 * complete and m ≤ F. It can pass (or be marginal) only if the k placed above its limit stay
 * above its rank for some m, and fail only if the k placed below reach no further than below it.
 */
function checkIncompleteFrames(value: Readonly<Record<string, unknown>>, problems: string[]): void {
  const figure = childAt(value, ["gpu", "incompleteFrames"]);
  if (!isRecord(figure)) {
    problems.push("gpu.incompleteFrames is missing");
    return;
  }
  const counts = figure["value"];
  if (counts === null) {
    return;
  }
  const path = "gpu.incompleteFrames.value";
  const dropped = childAt(counts, ["dropped"]);
  const partial = childAt(counts, ["partial"]);
  const frames = childAt(counts, ["frames"]);
  if (!isWhole(dropped, 0) || !isWhole(partial, 0) || !isWhole(frames, 0)) {
    problems.push(`${path} is not whole counts of dropped, partial and counted frames`);
    return;
  }
  const incomplete = dropped + partial;
  if (incomplete > frames) {
    problems.push(`${path} counts ${incomplete} incomplete frames of ${frames}`);
    return;
  }
  if (childAt(value, ["run", "launchMode"]) !== "native-replay") {
    const raf = childAt(value, ["frames", "raf", "value", "count"]);
    const afterWarmup = typeof raf === "number" ? raf : 0;
    if (frames !== afterWarmup) {
      problems.push(
        `${path}.frames is ${frames}, not the ${afterWarmup} frames after the warm-up that the rAF figures read`,
      );
    }
  }
  if (incomplete === 0) {
    return;
  }
  let canPass = false;
  for (let m = incomplete + 1; m <= frames && !canPass; m += 1) {
    canPass = rankIndex(m, GPU_ROW_PERCENTILE) < m - incomplete;
  }
  const canFail = frames > incomplete && rankIndex(frames, GPU_ROW_PERCENTILE) >= incomplete;
  const whole = childAt(value, ["criteria", "whole"]);
  for (const entry of Array.isArray(whole) ? whole : []) {
    const id = childAt(entry, ["id"]);
    const verdict = childAt(entry, ["verdict"]);
    if (typeof id !== "string" || !GPU_ROW_IDS.has(id)) {
      continue;
    }
    if (
      ((verdict === "pass" || verdict === "marginal") && !canPass) ||
      (verdict === "fail" && !canFail)
    ) {
      problems.push(
        `the ${id} row's ${verdict} is impossible with ${incomplete} of ${frames} frames' pass times incomplete`,
      );
    }
  }
}

/** The clock sources `gpu.clocks` may name. */
const CLOCK_SOURCES: ReadonlySet<unknown> = new Set<GpuClockSource>([
  "nvidia-smi",
  "i915-sysfs",
  "amdgpu-sysfs",
]);

/** `gpu.clocks`: a source, a maximum in whole MHz, and each column as long as the memory's times. */
function checkClocks(value: Readonly<Record<string, unknown>>, problems: string[]): void {
  const figure = childAt(value, ["gpu", "clocks"]);
  if (!isRecord(figure)) {
    problems.push("gpu.clocks is missing");
    return;
  }
  const clocks = figure["value"];
  if (clocks === null) {
    return;
  }
  const path = "gpu.clocks.value";
  if (!isRecord(clocks)) {
    problems.push(`${path} is not the GPU's clocks`);
    return;
  }
  if (!CLOCK_SOURCES.has(clocks["source"])) {
    problems.push(`${path}.source is not nvidia-smi, i915-sysfs or amdgpu-sysfs`);
  }
  const max = childAt(clocks, ["maxGraphicsMHz", "value"]);
  if (!isRecord(clocks["maxGraphicsMHz"]) || (max !== null && !isWhole(max, 1))) {
    problems.push(`${path}.maxGraphicsMHz is not whole MHz`);
  }
  const tMs = childAt(value, ["memory", "series", "tMs"]);
  if (!Array.isArray(tMs)) {
    // The memory check names the series' problem; there are no times to hold the columns to.
    return;
  }
  const count = tMs.length;
  for (const [name, unit] of [
    ["graphicsMHz", "MHz"],
    ["memoryMHz", "MHz"],
    ["performanceState", "P-states"],
  ] as const) {
    checkColumn(clocks[name], `${path}.${name}`, count, problems, unit);
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

/** The trace's windows in words: how many, the time traced, the boundaries' cost, failures. */
function describeTrace(trace: TraceRun): string {
  const files = trace.windows.flatMap(({ figures }) =>
    figures.value === null ? [] : [figures.value],
  );
  const largestBytes = files.reduce((max, { bytes }) => Math.max(max, bytes), 0);
  const buffers = files.flatMap(({ bufferPercent }) =>
    bufferPercent === null ? [] : [bufferPercent],
  );
  const stalls = trace.boundaries.flatMap(({ maxRafIntervalMs }) =>
    maxRafIntervalMs === null ? [] : [maxRafIntervalMs],
  );
  const excluded = trace.boundaries.reduce((sum, { excludedFrames }) => sum + excludedFrames, 0);
  const count = trace.windows.length;
  return [
    `${count} ${trace.format} ${count === 1 ? "window" : "windows"}, ${trace.tracedS.toFixed(1)} s traced after the warm-up, ${trace.profiled ? "profiled" : "unprofiled"}`,
    trace.boundaries.length === 0
      ? "no boundary"
      : `${trace.boundaries.length} ${trace.boundaries.length === 1 ? "boundary" : "boundaries"} left out ${excluded} frames${stalls.length === 0 ? "" : ` (largest stall ${formatMs(Math.max(...stalls))} ms)`}`,
    `largest file ${mib(largestBytes)}, buffer use ${buffers.length === 0 ? "not reported" : `up to ${Math.max(...buffers).toFixed(0)} %`}`,
    ...trace.windows.flatMap(({ index, figures }) =>
      figures.value === null ? [`window ${index + 1} failed: ${figures.reason}`] : [],
    ),
  ].join("; ");
}

/** The frames whose pass times are incomplete, in words. */
function describeIncomplete({ dropped, partial, frames }: IncompleteFrames): string {
  return dropped + partial === 0
    ? `none of ${frames} frames after the warm-up`
    : `${dropped} dropped and ${partial} partial of ${frames} frames after the warm-up, left out of the GPU rows' sums, whose verdicts hold whatever their times`;
}

/**
 * The GPU's clocks after the warm-up in words: the graphics clock's median, 5th and 95th
 * percentiles against its maximum, the memory clock's median and the performance states, with
 * the source; the warm-up read on the memory series' times as {@link clockNote} reads it.
 *
 * @param tMs - The memory series' times, ms.
 * @param warmupS - The warm-up, s.
 */
export function describeClocks(
  clocks: GpuClocks,
  tMs: ReadonlyArray<number>,
  warmupS: number,
): string {
  const fromMs = 1000 * warmupS;
  const graphics = readingsFrom(clocks.graphicsMHz, tMs, fromMs);
  const maxMHz = clocks.maxGraphicsMHz.value;
  const maximum =
    maxMHz === null ? `an unknown maximum (${clocks.maxGraphicsMHz.reason})` : `${maxMHz} MHz`;
  const parts = [`from ${clocks.source}`];
  if (graphics.length === 0) {
    parts.push(`no graphics clock after the warm-up, of ${maximum}`);
  } else {
    const medianMHz = nearestRank(graphics, 0.5);
    const share = maxMHz === null ? "" : ` (${((100 * medianMHz) / maxMHz).toFixed(0)} %)`;
    parts.push(
      `graphics median ${medianMHz} MHz${share}, p5 ${nearestRank(graphics, 0.05)}, p95 ${nearestRank(graphics, 0.95)}, after the warm-up, of ${maximum}`,
    );
  }
  const memoryMHz = readingsFrom(clocks.memoryMHz, tMs, fromMs);
  if (memoryMHz.length > 0) {
    parts.push(`memory median ${nearestRank(memoryMHz, 0.5)} MHz`);
  }
  const states = readingsFrom(clocks.performanceState, tMs, fromMs);
  const [lowest] = states;
  const highest = states.at(-1);
  if (lowest !== undefined && highest !== undefined) {
    parts.push(
      lowest === highest
        ? `performance state P${lowest}`
        : `performance states P${lowest} to P${highest}`,
    );
  }
  return parts.join("; ");
}

/** A clock column's every sample in words, `—` in its gaps, after its name. */
function describeColumn(name: string, figure: Measured<MemoryColumn>): string {
  return figure.value === null
    ? `${name} — (${figure.reason})`
    : `${name} ${figure.value.samples.map((reading) => (reading < 0 ? "—" : String(reading))).join(" ")}`;
}

/**
 * Every sample of the GPU's clocks in words, for a run's log: a smoke, whose 10 s are all
 * warm-up, writes no results file, so its log is where its clocks are seen.
 */
export function describeClockSamples(clocks: Measured<GpuClocks>): string {
  if (clocks.value === null) {
    return `— (${clocks.reason})`;
  }
  const { source, maxGraphicsMHz, graphicsMHz, memoryMHz, performanceState } = clocks.value;
  return [
    `from ${source}, maximum ${textOr(maxGraphicsMHz, (mhz) => `${mhz} MHz`)}`,
    describeColumn("graphics MHz", graphicsMHz),
    describeColumn("memory MHz", memoryMHz),
    describeColumn("performance states", performanceState),
  ].join("; ");
}

/** The heading of a profiled run's summary: its figures are a diagnostic's. */
const PROFILED_HEADING =
  "**PROFILED: diagnostic, not judged; renderer and app memory include the CPU profiler's samples.**";

/** The Markdown summary written beside a results file. */
export function summaryMarkdown(results: DescentResults): string {
  const { run, criteria, memory, frames } = results;
  const fromPresentation = frames.source === "presentation";
  const whole = intervalsOf(frames);
  const headline = memory.gpuHeadline;
  return [
    `# Descent spike: ${run.machine.name}, ${run.setting}, ${run.startedAt.slice(0, 10)}`,
    "",
    ...(run.trace.value?.profiled === true ? [PROFILED_HEADING, ""] : []),
    `- **Overall:** ${criteria.overall}${run.quiet.provisional ? " (provisional: not a quiet machine)" : ""}`,
    `- **Machine:** ${run.machine.cpu}, ${run.machine.logicalCores} threads; GPU ${textOr(run.machine.gpu, (gpu) => gpu.description ?? `${gpu.vendorId}:${gpu.deviceId}`)}; governor ${textOr(run.machine.governor, (governor) => governor)}; load average ${run.machine.loadAverage.map((load) => load.toFixed(2)).join(", ")}`,
    `- **Versions:** app ${run.versions.app}, Electron ${run.versions.electron}, Chromium ${run.versions.chromium}`,
    `- **Launch:** ${run.platform}, ${run.launchMode} mode, timer ${run.timer}, seed ${run.seed}, window ${run.shown ? "shown" : "hidden"}, canvas ${run.canvas.widthPx} × ${run.canvas.heightPx} px`,
    `- **T:** ${textOr(run.periodMs, (period) => `${formatMs(period)} ms`)}; warm-up ${run.warmupS} s`,
    `- **Trace:** ${textOr(run.trace, describeTrace)}`,
    `- **Incomplete pass times:** ${textOr(results.gpu.incompleteFrames, describeIncomplete)}`,
    `- **GPU clocks:** ${textOr(results.gpu.clocks, (clocks) => describeClocks(clocks, memory.series.tMs, run.warmupS))}`,
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
    `Intervals from ${whole.source}. Whole descent: ${describeStats(whole.stats)}.${run.trace.value === null ? "" : ` Frames left out at the trace's window boundaries: ${frames.excludedFrames}.`}`,
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
 * `.md`, adding `-2`, `-3` and so on when a run of the same day, machine and setting is there. A
 * profiled run's are `<date>-<machine>-<setting>-profiled.json` and `.md`, numbered the same way.
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
  const profiled = results.run.trace.value?.profiled === true ? "-profiled" : "";
  const stem = `${results.run.startedAt.slice(0, 10)}-${results.run.machine.name}-${results.run.setting}${profiled}`;
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
