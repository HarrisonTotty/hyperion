/**
 * The run record: one scene's run on one machine, a line of `runs.v1.jsonl` (plan R12, Design
 * notes 1, 4, 5, 7 and 13), and its validator.
 *
 * @remarks
 * Units are camelCase suffixes of the field names: `Ms`, `Ns`, `Us`, `Seconds`, `Bytes`, `PerS`,
 * `Hz`, `MHz`, `Watts`, `DegC` and `Px`. A figure the source could not read is `null`, never a
 * guess. The file is append-only and versioned in its name; a record of another `version` is
 * refused.
 */

import { isPassKey, isBudgetRow, isMemoryItem, VIEW_ROLES } from "./rows.ts";
import type { BudgetRow, MemoryItem, PassKey, ViewRole } from "./rows.ts";
import { isPerfSceneId } from "./scenes.ts";
import type { PerfSceneId } from "./scenes.ts";
import {
  array,
  bool,
  count,
  fraction,
  jsonObject,
  keyed,
  narrowed,
  nonEmpty,
  nonNegative,
  nullable,
  num,
  object,
  oneOf,
  PerfRecordError,
  positiveInt,
  str,
} from "./schema.ts";
import type { JsonObject, Schema } from "./schema.ts";

export { PerfRecordError } from "./schema.ts";
export type { JsonObject, JsonValue } from "./schema.ts";

/** The record's version, which `runs.v1.jsonl` holds alone. */
export const PERF_RECORD_VERSION = 1;

/**
 * Where a record comes from (Design note 1): a run of perf mode, R05's descent spike folded by
 * `foldSpikeResult`, or another plan's recorded benchmark entered by hand.
 */
export type PerfSource = "perf-run" | "descent-spike" | "plan-benchmark";

/** R01's `GpuTimer`, as `GraphicsStatus.timer` reports it: the record copies it (Design note 4). */
export type PerfTimer = "quantized" | "full" | "absent";

/** R05's `QualitySetting`. */
export type PerfQuality = "high" | "low";

/** The scene a record ran (Design note 8). */
export interface PerfSceneRef {
  readonly id: PerfSceneId;
  /**
   * The scene's version, bumped with any change to its pins or script, so that records from before
   * and after never compare; 0 for a fold of R05's spike, which ran R05's own world and not the
   * catalogue's scene.
   */
  readonly version: number;
  /** The universe seed, as the scene pins it. */
  readonly seed: string;
  /** The world flown: R05's `terrain-test` or a generated body's pin. */
  readonly world: string;
}

/**
 * The setting a record ran at (Design note 3): R05's `QualitySetting` and the value of every ladder
 * entry, read from R05's `SETTINGS` and the modules that hold values beside it.
 *
 * @remarks
 * `values` are keyed `<module>.<field>` (`quality.clouds`, `atmosphere.SKY_SUN_CAP`). Records
 * compare only at the same `quality`; a change of `values` within it is what R12.T11 measures, and
 * the comparison prints it.
 */
export interface SettingSnapshot {
  readonly quality: PerfQuality;
  readonly values: JsonObject;
}

/** The internal-scale controller's state over the run (R07 Design note 14), as fractions of 1. */
export interface InternalScaleRecord {
  readonly min: number;
  readonly median: number;
}

/** A view's exposure model (R02): its metering mode and the EV range it held over the run. */
export interface ExposureRecord {
  readonly mode: string;
  readonly evMin: number;
  readonly evMax: number;
}

/** One view of a run (Design note 13; R07 Design note 14). */
export interface PerfViewRecord {
  readonly role: ViewRole;
  readonly style: "wireframe" | "photorealistic";
  readonly widthPx: number;
  readonly heightPx: number;
  readonly internalScale: InternalScaleRecord;
  readonly rateHz: number;
  /**
   * A wireframe view's line antialiasing (R10.T11: the station's 4–9 ms holds with `analytic`, and
   * `msaa4` takes it to 10–15 ms); `none` for a photorealistic view; `null` where the source did not
   * record it.
   */
  readonly lineAntialiasing: "analytic" | "msaa4" | "none" | null;
  /** `null` where the source did not record it. */
  readonly exposure: ExposureRecord | null;
}

/** The CPU (Design note 5). */
export interface CpuRecord {
  readonly model: string;
  readonly logicalCores: number;
}

/** The active GPU from `app.getGPUInfo("complete")` (Design note 5). */
export interface GpuRecord {
  /** PCI vendor ID: 0x8086 Intel, 0x10de NVIDIA, 0x106b Apple. */
  readonly vendorId: number;
  readonly deviceId: number;
  /** Mesa's or NVIDIA's driver version. */
  readonly driverVersion: string | null;
  readonly description: string | null;
}

/** R01's `AdapterSummary`, the WebGPU adapter's own description. */
export interface AdapterRecord {
  readonly vendor: string;
  readonly architecture: string;
  readonly description: string;
  readonly fallback: boolean;
}

/** The software versions (Design note 5). */
export interface VersionsRecord {
  readonly app: string | null;
  readonly electron: string;
  readonly chromium: string;
  readonly node: string | null;
}

/** The display the run's window was on (Design note 5). */
export interface DisplayRecord {
  readonly widthPx: number;
  readonly heightPx: number;
  readonly refreshHz: number | null;
  readonly devicePixelRatio: number | null;
}

/**
 * A discrete card's published figures (Design note 11; R12.T9.a), beside the clocks its own
 * `nvidia-smi -q` reports.
 */
export interface CardRecord {
  readonly cudaCores: number | null;
  readonly boostClockMHz: number | null;
  readonly tgpWatts: number | null;
  readonly memoryBytes: number | null;
  readonly busWidthBits: number | null;
  readonly maxGraphicsClockMHz: number | null;
  readonly memoryClockMHz: number | null;
}

/** What a record says about the machine (Design note 5). */
export interface MachineRecord {
  /**
   * The machine's slug, the record ID's middle part: the hardware it names is a Target hardware
   * entry of the rendering plans' README, never a host name.
   */
  readonly name: string;
  readonly platform: "linux" | "darwin" | "win32";
  /** `os.release()`. */
  readonly kernel: string | null;
  readonly cpu: CpuRecord;
  readonly memoryBytes: number | null;
  readonly gpu: GpuRecord | null;
  readonly adapter: AdapterRecord | null;
  /** R01's `GpuCapabilities`, by member name (`timestampQuery`, `subgroups`, …). */
  readonly capabilities: Readonly<Record<string, boolean>> | null;
  /** The path `selectKernel` chose for each kernel pair, by pair. */
  readonly kernelPaths: Readonly<Record<string, string>> | null;
  readonly versions: VersionsRecord;
  readonly display: DisplayRecord | null;
  /** The window manager (`i3`), or `null` where unread. */
  readonly windowManager: string | null;
  /** The compositor, `none` where there is none, or `null` where unread. */
  readonly compositor: string | null;
  readonly windowSystem: "x11" | "xwayland" | "wayland" | "macos" | "windows" | null;
  readonly card: CardRecord | null;
}

/**
 * The power profile as found (Design note 7): two records compare only when it agrees.
 *
 * @remarks
 * Every field is `null` where the machine has no such file or setting: off Linux, all but
 * `lowPowerMode`; on a desktop, TLP's mode and `no_turbo` (R12.T2).
 */
export interface PowerProfileRecord {
  readonly tlpMode: string | null;
  readonly governor: string | null;
  readonly epp: string | null;
  readonly noTurbo: boolean | null;
  readonly pl1Watts: number | null;
  readonly pl2Watts: number | null;
  readonly tauSeconds: number | null;
  /** The RAPL zone read: `intel-rapl-mmio:0`, else `intel-rapl:0`. */
  readonly raplZone: string | null;
  /** macOS's Low Power Mode (`pmset -g`), the owner's Mac's lever (a lean, 2026-11-13). */
  readonly lowPowerMode: boolean | null;
}

/** The machine's power and thermal state at the start (Design note 7). */
export interface PowerThermalRecord {
  /**
   * `mains`: the `Mains` supply is online; `battery`: it is not; `desktop`: no `Mains` supply and
   * no system battery (hardware item 2); `null` where unread.
   */
  readonly supply: "mains" | "battery" | "desktop" | null;
  readonly profile: PowerProfileRecord;
  /** The one-minute load average before launch; `null` on Windows, which keeps none. */
  readonly loadAverage1m: number | null;
  /** The package temperature at the start, after any wait for it to fall below 60 °C. */
  readonly startPackageDegC: number | null;
  /** The zone read: `x86_pkg_temp`, or `k10temp`'s `Tctl` on the desktop. */
  readonly temperatureSource: string | null;
}

/** The warm-up (Design notes 7 and 13). */
export interface WarmUpRecord {
  readonly seconds: number;
  /** `false` is `NOT STEADY` after ten minutes; `null` where the source judged no steadiness. */
  readonly steady: boolean | null;
}

/** The server a run used (Design notes 8 and 13). */
export interface ServerRecord {
  readonly placement: "local" | "remote" | "co-resident" | "none";
  readonly numWorkers: number | null;
  /** Whether it was restarted with a fresh data directory before each repetition. */
  readonly restartedPerRepetition: boolean;
}

/** How the run was measured (Design notes 4, 7, 8 and 13). */
export interface HarnessRecord {
  /** R01's `--hyperion-gpu-timing`: `false` on a control run (Design note 4). */
  readonly gpuTiming: boolean;
  readonly timer: PerfTimer;
  /** Passes timed as an upper bound; empty since R01 Design note 24. */
  readonly bracketedPasses: ReadonlyArray<string>;
  /** Every switch in force (Design note 5). */
  readonly switches: ReadonlyArray<string>;
  /** R01's launch mode (`vulkan`, `default`, …). */
  readonly launchMode: string | null;
  readonly repetitions: number;
  /** One repetition's span (Design note 13); `null` where the source did not record it. */
  readonly repetitionSeconds: number | null;
  readonly warmUp: WarmUpRecord;
  /** A precondition was overridden with `--force`, or R05's run was provisional. */
  readonly forced: boolean;
  /** Which preconditions failed, in words. */
  readonly forcedReasons: ReadonlyArray<string>;
  /** Per scene (Design note 9). */
  readonly pacing: "every-vsync" | "every-second-vsync";
  /** The display's vsync period the run measured, from which T is taken. */
  readonly vsyncPeriodMs: number | null;
  readonly server: ServerRecord;
  readonly heightWorkers: number | null;
  /**
   * `physical`: frames paced by a panel's vertical blank. `virtual`: a virtual display or a hidden
   * window, paced by Chromium's timer (Design note 11), which the tables leave out.
   */
  readonly display: "physical" | "virtual";
}

/** Frame intervals over a span (R05's `FrameStats`; Design note 7). */
export interface FrameFigures {
  readonly source: "presentation" | "raf" | "gpu-completion";
  readonly count: number;
  readonly meanMs: number | null;
  readonly p50Ms: number | null;
  readonly p95Ms: number | null;
  readonly p99Ms: number | null;
  readonly maxMs: number | null;
  /** Intervals above 1.5 T, R05's convention. */
  readonly missedFraction: number | null;
  readonly hitches: number | null;
}

/** One segment's frame intervals (R05's segments of the descent). */
export interface SegmentFigures {
  readonly segment: string;
  readonly frames: FrameFigures;
}

/** A row's GPU time: each frame's sum of the row's passes (Design note 4). */
export interface RowFigures {
  readonly row: BudgetRow;
  readonly frames: number;
  readonly meanMs: number | null;
  readonly p50Ms: number | null;
  readonly p95Ms: number | null;
  readonly p99Ms: number | null;
}

/** One timed pass's GPU time (Design note 4). */
export interface PassFigures {
  readonly key: PassKey;
  /** Its row as `PASS_ROWS` had it when the record was made. */
  readonly row: BudgetRow;
  readonly frames: number;
  readonly p50Ms: number | null;
  readonly p95Ms: number | null;
  readonly p99Ms: number | null;
}

/** The GPU memory's headline and its bounds (Design note 6). */
export interface GpuMemoryFigures {
  readonly source: "drm-fdinfo" | "nvidia-smi" | "adapter-tally";
  /** Σ `drm-resident-system0`, or `nvidia-smi`'s figure, or the tally's, at its peak. */
  readonly residentPeakBytes: number;
  readonly totalPeakBytes: number | null;
  /** Σ resident − ½ Σ `drm-shared-system0`. */
  readonly residentLowerBoundBytes: number | null;
}

/** Memory over a repetition (Design note 6). */
export interface MemoryFigures {
  /** The allocation tally's peak by item (R12.T3.a); an item absent was not allocated. */
  readonly itemPeakBytes: Readonly<Partial<Record<MemoryItem, number>>>;
  /** The allocation tally's whole peak. */
  readonly tallyPeakBytes: number | null;
  readonly gpu: GpuMemoryFigures | null;
  /** Every process's memory from `app.getAppMetrics()`, at its peak. */
  readonly appPeakBytes: number | null;
  /** Each height worker's coarse-field copy. */
  readonly workerFieldBytes: ReadonlyArray<number>;
}

/** One segment's streaming against the demand (Design note 9). */
export interface StreamingFigures {
  readonly segment: string;
  readonly patchesPerS: number | null;
  readonly demandPerS: number | null;
  readonly streamingSeconds: number | null;
}

/** Throttling and clocks over a repetition (Design note 7), recorded and not excluded. */
export interface ThrottleFigures {
  readonly packageThrottleMs: number | null;
  readonly coreThrottleMs: number | null;
  /** The share of the span throttled. */
  readonly throttleFraction: number | null;
  /** The GPU's share of the span at `gt_RP0_freq_mhz`. */
  readonly rp0Share: number | null;
  readonly gpuMinMHz: number | null;
  readonly gpuMedianMHz: number | null;
  readonly maxPackageDegC: number | null;
}

/** One repetition: one pass over the scene's span (Design note 13). */
export interface PerfRepetition {
  /** 1-based. */
  readonly index: number;
  readonly startedAt: string;
  readonly frames: FrameFigures;
  readonly segments: ReadonlyArray<SegmentFigures>;
  /** Mean GPU busy time per frame, a compared metric (Design note 7). */
  readonly gpuBusyPerFrameMs: number | null;
  /** Mean main-thread time per frame, a compared metric (Design note 7). */
  readonly mainThreadPerFrameMs: number | null;
  readonly rows: ReadonlyArray<RowFigures>;
  readonly passes: ReadonlyArray<PassFigures>;
  readonly memory: MemoryFigures;
  readonly streaming: ReadonlyArray<StreamingFigures>;
  readonly throttle: ThrottleFigures;
  /** Pipelines created after the warm-up, counted by R05's shim. */
  readonly latePipelines: number;
  /** The height workers' cost, µs a point with its gradient. */
  readonly heightPointUs: number | null;
}

/** The pooled frames of every repetition (Design note 7): p99 over all of them. */
export interface PooledFramesRecord {
  readonly count: number;
  readonly p99Ms: number | null;
}

/** The arrival's posts over R03's binary frames (Design note 8). */
export interface ArrivalRecord {
  /** The coarse field as posted to each worker. */
  readonly fieldBytes: number;
  readonly fieldTransferMs: number;
  readonly skyBytes: number;
  readonly skyTransferMs: number;
}

/** One scene's run on one machine: a line of `runs.v1.jsonl`. */
export interface PerfRunRecord {
  readonly version: typeof PERF_RECORD_VERSION;
  /** `${YYYY-MM-DD}-${machine.name}-${scene.id}-${n}`, unique in the store. */
  readonly id: string;
  /** ISO 8601 in UTC: the run's start. */
  readonly date: string;
  /** `git rev-parse HEAD`; `null` where the source did not record it (R05's results files). */
  readonly commit: string | null;
  /** Whether the tree had changes beyond `commit`. */
  readonly dirty: boolean;
  /** `hyperion-sim`'s `GENERATOR_VERSION`; `null` where the source did not record it. */
  readonly generatorVersion: number | null;
  readonly source: PerfSource;
  readonly scene: PerfSceneRef;
  readonly setting: SettingSnapshot;
  readonly views: ReadonlyArray<PerfViewRecord>;
  readonly machine: MachineRecord;
  readonly state: PowerThermalRecord;
  readonly harness: HarnessRecord;
  readonly runs: ReadonlyArray<PerfRepetition>;
  readonly pooledFrames: PooledFramesRecord;
  readonly arrival: ArrivalRecord | null;
  readonly notes: string;
}

const SLUG = /^[a-z0-9][a-z0-9-]*$/;
const ISO_UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/;
const COMMIT = /^[0-9a-f]{40}$/;

const slug: Schema<string> = (value, path) => {
  const text = str(value, path);
  if (!SLUG.test(text)) {
    throw new PerfRecordError(path, `expected a lower-case slug ([a-z0-9-]), got "${text}"`);
  }
  return text;
};

const isoUtc: Schema<string> = (value, path) => {
  const text = str(value, path);
  if (!ISO_UTC.test(text) || !Number.isFinite(Date.parse(text))) {
    throw new PerfRecordError(path, `expected an ISO 8601 time in UTC, got "${text}"`);
  }
  return text;
};

const commit: Schema<string> = (value, path) => {
  const text = str(value, path);
  if (!COMMIT.test(text))
    throw new PerfRecordError(path, `expected a 40-digit commit, got "${text}"`);
  return text;
};

function booleans(value: unknown, path: string): Readonly<Record<string, boolean>> {
  const all = jsonObject(value, path);
  const out: Record<string, boolean> = {};
  for (const [key, item] of Object.entries(all)) out[key] = bool(item, `${path}.${key}`);
  return out;
}

function strings(value: unknown, path: string): Readonly<Record<string, string>> {
  const all = jsonObject(value, path);
  const out: Record<string, string> = {};
  for (const [key, item] of Object.entries(all)) out[key] = str(item, `${path}.${key}`);
  return out;
}

const nNum = nullable(num);
const nNonNegative = nullable(nonNegative);
const nCount = nullable(count);
const nFraction = nullable(fraction);
const nStr = nullable(str);

const frameFigures = object<FrameFigures>({
  source: oneOf(["presentation", "raf", "gpu-completion"]),
  count,
  meanMs: nNonNegative,
  p50Ms: nNonNegative,
  p95Ms: nNonNegative,
  p99Ms: nNonNegative,
  maxMs: nNonNegative,
  missedFraction: nFraction,
  hitches: nCount,
});

const budgetRow = narrowed<BudgetRow>(isBudgetRow, "a budget row");

const repetition = object<PerfRepetition>({
  index: positiveInt,
  startedAt: isoUtc,
  frames: frameFigures,
  segments: array(object<SegmentFigures>({ segment: nonEmpty, frames: frameFigures })),
  gpuBusyPerFrameMs: nNonNegative,
  mainThreadPerFrameMs: nNonNegative,
  rows: array(
    object<RowFigures>({
      row: budgetRow,
      frames: count,
      meanMs: nNonNegative,
      p50Ms: nNonNegative,
      p95Ms: nNonNegative,
      p99Ms: nNonNegative,
    }),
  ),
  passes: array(
    object<PassFigures>({
      key: narrowed<PassKey>(isPassKey, `a pass key (${VIEW_ROLES.join(" | ")}:<label>)`),
      row: budgetRow,
      frames: count,
      p50Ms: nNonNegative,
      p95Ms: nNonNegative,
      p99Ms: nNonNegative,
    }),
  ),
  memory: object<MemoryFigures>({
    itemPeakBytes: keyed<MemoryItem, number>(isMemoryItem, "a memory item", count),
    tallyPeakBytes: nCount,
    gpu: nullable(
      object<GpuMemoryFigures>({
        source: oneOf(["drm-fdinfo", "nvidia-smi", "adapter-tally"]),
        residentPeakBytes: count,
        totalPeakBytes: nCount,
        residentLowerBoundBytes: nCount,
      }),
    ),
    appPeakBytes: nCount,
    workerFieldBytes: array(count),
  }),
  streaming: array(
    object<StreamingFigures>({
      segment: nonEmpty,
      patchesPerS: nNonNegative,
      demandPerS: nNonNegative,
      streamingSeconds: nNonNegative,
    }),
  ),
  throttle: object<ThrottleFigures>({
    packageThrottleMs: nNonNegative,
    coreThrottleMs: nNonNegative,
    throttleFraction: nFraction,
    rp0Share: nFraction,
    gpuMinMHz: nNonNegative,
    gpuMedianMHz: nNonNegative,
    maxPackageDegC: nNum,
  }),
  latePipelines: count,
  heightPointUs: nNonNegative,
});

const recordFields = object<PerfRunRecord>({
  version: (value, path) => {
    if (value !== PERF_RECORD_VERSION) {
      throw new PerfRecordError(
        path,
        `unknown version ${JSON.stringify(value)}: this reader knows version ${String(PERF_RECORD_VERSION)} alone`,
      );
    }
    return PERF_RECORD_VERSION;
  },
  id: nonEmpty,
  date: isoUtc,
  commit: nullable(commit),
  dirty: bool,
  generatorVersion: nullable(positiveInt),
  source: oneOf<PerfSource>(["perf-run", "descent-spike", "plan-benchmark"]),
  scene: object<PerfSceneRef>({
    id: narrowed<PerfSceneId>(isPerfSceneId, "a scene of PERF_SCENE_IDS"),
    version: count,
    seed: nonEmpty,
    world: nonEmpty,
  }),
  setting: object<SettingSnapshot>({
    quality: oneOf<PerfQuality>(["high", "low"]),
    values: jsonObject,
  }),
  views: array(
    object<PerfViewRecord>({
      role: oneOf(VIEW_ROLES),
      style: oneOf(["wireframe", "photorealistic"]),
      widthPx: positiveInt,
      heightPx: positiveInt,
      internalScale: object<InternalScaleRecord>({ min: fraction, median: fraction }),
      rateHz: nonNegative,
      lineAntialiasing: nullable(oneOf(["analytic", "msaa4", "none"])),
      exposure: nullable(object<ExposureRecord>({ mode: nonEmpty, evMin: num, evMax: num })),
    }),
  ),
  machine: object<MachineRecord>({
    name: slug,
    platform: oneOf(["linux", "darwin", "win32"]),
    kernel: nStr,
    cpu: object<CpuRecord>({ model: nonEmpty, logicalCores: positiveInt }),
    memoryBytes: nCount,
    gpu: nullable(
      object<GpuRecord>({
        vendorId: count,
        deviceId: count,
        driverVersion: nStr,
        description: nStr,
      }),
    ),
    adapter: nullable(
      object<AdapterRecord>({ vendor: str, architecture: str, description: str, fallback: bool }),
    ),
    capabilities: nullable(booleans),
    kernelPaths: nullable(strings),
    versions: object<VersionsRecord>({
      app: nStr,
      electron: nonEmpty,
      chromium: nonEmpty,
      node: nStr,
    }),
    display: nullable(
      object<DisplayRecord>({
        widthPx: positiveInt,
        heightPx: positiveInt,
        refreshHz: nNonNegative,
        devicePixelRatio: nNonNegative,
      }),
    ),
    windowManager: nStr,
    compositor: nStr,
    windowSystem: nullable(oneOf(["x11", "xwayland", "wayland", "macos", "windows"])),
    card: nullable(
      object<CardRecord>({
        cudaCores: nCount,
        boostClockMHz: nNonNegative,
        tgpWatts: nNonNegative,
        memoryBytes: nCount,
        busWidthBits: nCount,
        maxGraphicsClockMHz: nNonNegative,
        memoryClockMHz: nNonNegative,
      }),
    ),
  }),
  state: object<PowerThermalRecord>({
    supply: nullable(oneOf(["mains", "battery", "desktop"])),
    profile: object<PowerProfileRecord>({
      tlpMode: nStr,
      governor: nStr,
      epp: nStr,
      noTurbo: nullable(bool),
      pl1Watts: nNonNegative,
      pl2Watts: nNonNegative,
      tauSeconds: nNonNegative,
      raplZone: nStr,
      lowPowerMode: nullable(bool),
    }),
    loadAverage1m: nNonNegative,
    startPackageDegC: nNum,
    temperatureSource: nStr,
  }),
  harness: object<HarnessRecord>({
    gpuTiming: bool,
    timer: oneOf<PerfTimer>(["quantized", "full", "absent"]),
    bracketedPasses: array(nonEmpty),
    switches: array(nonEmpty),
    launchMode: nStr,
    repetitions: positiveInt,
    repetitionSeconds: nNonNegative,
    warmUp: object<WarmUpRecord>({ seconds: nonNegative, steady: nullable(bool) }),
    forced: bool,
    forcedReasons: array(nonEmpty),
    pacing: oneOf(["every-vsync", "every-second-vsync"]),
    vsyncPeriodMs: nNonNegative,
    server: object<ServerRecord>({
      placement: oneOf(["local", "remote", "co-resident", "none"]),
      numWorkers: nullable(positiveInt),
      restartedPerRepetition: bool,
    }),
    heightWorkers: nCount,
    display: oneOf(["physical", "virtual"]),
  }),
  runs: array(repetition),
  pooledFrames: object<PooledFramesRecord>({ count, p99Ms: nNonNegative }),
  arrival: nullable(
    object<ArrivalRecord>({
      fieldBytes: count,
      fieldTransferMs: nonNegative,
      skyBytes: count,
      skyTransferMs: nonNegative,
    }),
  ),
  notes: str,
});

/** The ID a record's date, machine, scene and number give. */
export function perfRecordId(date: string, machine: string, scene: PerfSceneId, n: number): string {
  return `${date.slice(0, 10)}-${machine}-${scene}-${String(n)}`;
}

/** The trailing number of a record's ID, or `null` if the ID is not of {@link perfRecordId}'s form. */
export function perfRecordNumber(record: PerfRunRecord): number | null {
  const prefix = perfRecordId(record.date, record.machine.name, record.scene.id, 0).slice(0, -1);
  if (!record.id.startsWith(prefix)) return null;
  const tail = record.id.slice(prefix.length);
  return /^[1-9]\d*$/.test(tail) ? Number(tail) : null;
}

function checkRecord(record: PerfRunRecord): void {
  if (perfRecordNumber(record) === null) {
    const expected = perfRecordId(record.date, record.machine.name, record.scene.id, 1).replace(
      /1$/,
      "<n>",
    );
    throw new PerfRecordError("id", `expected "${expected}", got "${record.id}"`);
  }
  if (record.source === "perf-run") {
    if (record.commit === null)
      throw new PerfRecordError("commit", "a perf-run record states its commit");
    if (record.generatorVersion === null) {
      throw new PerfRecordError(
        "generatorVersion",
        "a perf-run record states its generator version",
      );
    }
    if (record.scene.version === 0) {
      throw new PerfRecordError(
        "scene.version",
        "version 0 is a fold's; a catalogue scene's starts at 1",
      );
    }
    if (record.harness.repetitionSeconds === null) {
      throw new PerfRecordError("harness.repetitionSeconds", "a perf-run record states its span");
    }
  }
  if (record.runs.length !== record.harness.repetitions) {
    throw new PerfRecordError(
      "runs",
      `${String(record.runs.length)} repetitions, but harness.repetitions is ${String(record.harness.repetitions)}`,
    );
  }
  record.runs.forEach((run, index) => {
    if (run.index !== index + 1) {
      throw new PerfRecordError(
        `runs[${String(index)}].index`,
        `expected ${String(index + 1)}, got ${String(run.index)}`,
      );
    }
  });
  if (record.harness.forced !== record.harness.forcedReasons.length > 0) {
    throw new PerfRecordError(
      "harness.forcedReasons",
      "a forced record names its reasons, and only a forced one",
    );
  }
  if (record.views.length === 0) throw new PerfRecordError("views", "a run has at least one view");
}

/**
 * Validates a value already parsed from JSON as a record: every field present with its type, no
 * field beyond them, and the record's own invariants (its ID, its repetitions' count and order).
 *
 * @throws {@link PerfRecordError} naming the first field at fault.
 */
export function validatePerfRecord(value: unknown, line: number | null = null): PerfRunRecord {
  try {
    const record = recordFields(value, "");
    checkRecord(record);
    return record;
  } catch (error) {
    if (error instanceof PerfRecordError && line !== null) {
      throw new PerfRecordError(error.field, error.reason, line);
    }
    throw error;
  }
}

/**
 * Parses one line of `runs.v1.jsonl` into a record.
 *
 * @param line - the 1-based line of the store it was read from, for the error, if any.
 * @throws {@link PerfRecordError} naming the first field at fault, or the line if it is not JSON.
 */
export function parsePerfRecord(text: string, line: number | null = null): PerfRunRecord {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new PerfRecordError("", `not JSON: ${reason}`, line);
  }
  return validatePerfRecord(value, line);
}

/**
 * Parses the whole of `runs.v1.jsonl`: one record a line, blank lines skipped, IDs unique.
 *
 * @throws {@link PerfRecordError} naming the line and field at fault.
 */
export function parsePerfRecords(text: string): ReadonlyArray<PerfRunRecord> {
  const records: PerfRunRecord[] = [];
  const ids = new Set<string>();
  text.split("\n").forEach((content, index) => {
    if (content.trim().length === 0) return;
    const record = parsePerfRecord(content, index + 1);
    if (ids.has(record.id))
      throw new PerfRecordError("id", `"${record.id}" is already in the store`, index + 1);
    ids.add(record.id);
    records.push(record);
  });
  return records;
}

/** One record as its line of `runs.v1.jsonl`, without the newline. */
export function serialisePerfRecord(record: PerfRunRecord): string {
  return JSON.stringify(record);
}
