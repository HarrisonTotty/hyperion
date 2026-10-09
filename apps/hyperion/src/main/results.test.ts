import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

import { format, resolveConfig } from "prettier";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DescentSpikeReport } from "../preload/api";
import {
  callbackStartsOf,
  PROFILED,
  recordingOf,
  UNPROFILED,
  windowFile,
  windowTrace,
} from "./fixtures/traces";
import type { GpuClockSample } from "./gpuClocks";
import type { Measured } from "./measured";
import { type TraceFigures, TraceReducer } from "./reduceTrace";
import {
  ADDED_FILE_LIMIT_BYTES,
  boundedGpuRow,
  buildResults,
  clockNote,
  type DescentResults,
  describeClockSamples,
  describeMachine,
  formatAsPrettier,
  frameStats,
  type GpuClocks,
  gpuClocksOf,
  incompleteFramesReason,
  type MachineDescription,
  type MachineSources,
  measured,
  type MemorySample,
  MemorySampler,
  memorySeries,
  type MemorySources,
  missing,
  nearestRank,
  OVER_ESTIMATE_FINDING,
  READ_IN_FLIGHT_REASON,
  RESULTS_SCHEMA,
  RESULTS_VERSION,
  row,
  type ResultsFiles,
  type RunDescription,
  sampleMemory,
  summaryMarkdown,
  TERRAIN_ATMOSPHERE_ROW,
  TIMESTAMP_QUANTUM_MS,
  validateResults,
  writeResults,
} from "./results";
import {
  EMPTY_TRACE_REASON,
  LOST_DATA_REASON,
  NO_CLOCK_OFFSET_REASON,
  PROFILER_OFF_REASON,
  type TraceRecording,
  type TraceSettings,
  type TraceWindowFile,
} from "./traceWindows";

const MIB = 1024 ** 2;

const MACHINE: MachineDescription = {
  name: "devbox",
  cpu: "AMD Ryzen 7 3700X 8-Core Processor",
  logicalCores: 16,
  memoryBytes: 32 * 1024 ** 3,
  governor: measured("schedutil"),
  loadAverage: [0.4, 0.6, 0.8],
  gpu: measured({
    vendorId: 0x10_de,
    deviceId: 0x22_06,
    driverVersion: "615.71.09",
    description: "NVIDIA GeForce RTX 3080",
  }),
};

function runOf(overrides: Partial<RunDescription> = {}): RunDescription {
  return {
    startedAt: new Date("2026-10-02T18:00:00Z"),
    machine: MACHINE,
    versions: {
      app: "0.1.0",
      electron: "44.4.3",
      chromium: "152.0.7977.130",
      node: "24.0.0",
      v8: "15.2",
    },
    platform: "linux",
    launchMode: "vulkan",
    setting: "low",
    seed: "18446744073709551615",
    options: { setting: "low", seed: "18446744073709551615", workers: 3 },
    switches: ["--disable-dawn-features=timestamp_quantization"],
    shown: true,
    displayHz: 60,
    nvidiaBaselineBytes: 168 * MIB,
    ...overrides,
  };
}

/**
 * Twenty frames a second of script time over two segments, 10 s each, after a 10 s warm-up; the
 * script starts at 1,000 ms, and its one trace window runs from −0.1 s to 30.1 s.
 */
function reportOf(overrides: Partial<DescentSpikeReport> = {}): DescentSpikeReport {
  const scriptTimesS = Array.from({ length: 600 }, (_, i) => i / 20);
  const rafIntervalsMs = scriptTimesS.map((_, i) => (i === 0 ? 0 : i % 100 === 0 ? 120 : 33.3));
  return {
    scriptStartMs: 1000,
    traceWindows: [{ startedMs: 900, stopRequestedMs: 31_100, failure: null }],
    traceGuardS: 1,
    warmupS: 10,
    segments: [
      { name: "orbit coast", startS: 0, endS: 20 },
      { name: "descent arc", startS: 20, endS: 30 },
    ],
    levels: [
      { level: 0, epsilonM: 9000, k: 4.1 },
      { level: 19, epsilonM: 0.02, k: 5.2 },
    ],
    timer: "full",
    untimedPasses: 0,
    frames: {
      scriptTimesS,
      rafIntervalsMs,
      ourCodeMs: scriptTimesS.map(() => 4),
      callbackStartsMs: callbackStartsOf(1000, scriptTimesS),
      missingResolves: scriptTimesS.map(() => 0),
      inFlightResolves: scriptTimesS.map(() => 0),
      passes: [
        { label: "terrain", row: "terrain", gpuMs: scriptTimesS.map(() => 6) },
        { label: "atmosphere.sky", row: "atmosphere", gpuMs: scriptTimesS.map(() => 1.5) },
        {
          label: "tone",
          row: "other",
          gpuMs: scriptTimesS.map((_, i) => (i % 2 === 0 ? 0.5 : null)),
        },
      ],
    },
    streaming: [
      {
        segment: "orbit coast",
        requestedPerS: 4,
        bakedPerS: 4,
        residentPerS: 4,
        predictedHardPerS: 5,
        predictedCalibratedPerS: 2,
        patchesHard: 900,
        patchesCalibrated: 380,
        streamingS: 0,
      },
    ],
    uploadBytes: 64 * MIB,
    latePipelines: [{ label: "terrain.normals", kind: "compute", async: true, scriptTimeS: 21 }],
    adapterPeakBytes: 300 * MIB,
    canvas: { widthPx: 1280, heightPx: 720 },
    ...overrides,
  };
}

/** Page ms at script time `s`, for {@link reportOf}'s script. */
function at(s: number): number {
  return 1000 + 1000 * s;
}

/**
 * The one window's trace: presentations every 33.3 ms over the run, 5 ms after the frames, frames
 * dropped at 5, 15 and 25 s, and a frame span for each of {@link reportOf}'s frames.
 */
function windowOf(): TraceFigures {
  return windowTrace({
    offsetUs: 7e9,
    fromMs: at(-0.1),
    toMs: at(30.1),
    frames: reportOf(),
    presentedMs: Array.from({ length: 900 }, (_, i) => at(0.005 + i / 30)),
    droppedMs: [5, 15, 25].map(at),
  });
}

/** {@link windowOf} as the run's one window. */
function traceOf(settings: TraceSettings = UNPROFILED): Measured<TraceRecording> {
  return recordingOf([windowFile(windowOf())], settings);
}

/** Two windows with a boundary at 15 s: the trace stopped there, and started again at 15.5 s. */
const TWO_WINDOWS: DescentSpikeReport["traceWindows"] = [
  { startedMs: at(-0.1), stopRequestedMs: at(15), failure: null },
  { startedMs: at(15.5), stopRequestedMs: at(30.1), failure: null },
];

/**
 * The windows of {@link TWO_WINDOWS}: the first presenting every 33.3 ms to 15 s, the second twice
 * 500 ms apart in its exclusion, to 16.5 s, then every 33.3 ms; each with a frame span for each of
 * {@link twoWindowReport}'s frames.
 */
function twoWindowTraces(): [TraceFigures, TraceFigures] {
  return [
    windowTrace({
      offsetUs: 7e9,
      fromMs: at(-0.1),
      toMs: at(15),
      frames: twoWindowReport(),
      presentedMs: Array.from({ length: 450 }, (_, i) => at(0.005 + i / 30)),
    }),
    windowTrace({
      offsetUs: 9e9,
      fromMs: at(15.5),
      toMs: at(30.1),
      frames: twoWindowReport(),
      presentedMs: [15.505, 16.005, ...Array.from({ length: 404 }, (_, k) => 16.505 + k / 30)].map(
        at,
      ),
    }),
  ];
}

/**
 * {@link reportOf}'s report in two windows, its frames in the boundary's exclusion, [15, 16.5) s,
 * slow: 500 ms rAF intervals, 40 ms of our code and 50 ms of terrain.
 */
function twoWindowReport(): DescentSpikeReport {
  const base = reportOf({ traceWindows: TWO_WINDOWS });
  const inExclusion = base.frames.scriptTimesS.map((t) => t >= 15 && t < 16.5);
  return {
    ...base,
    frames: {
      ...base.frames,
      rafIntervalsMs: base.frames.rafIntervalsMs.map((ms, i) =>
        inExclusion[i] === true ? 500 : ms,
      ),
      ourCodeMs: base.frames.ourCodeMs.map((ms, i) => (inExclusion[i] === true ? 40 : ms)),
      passes: base.frames.passes.map((pass) =>
        pass.row === "terrain"
          ? Object.assign({}, pass, {
              gpuMs: pass.gpuMs.map((ms, i) => (inExclusion[i] === true ? 50 : ms)),
            })
          : pass,
      ),
    },
  };
}

/**
 * A sample's clocks as `nvidia-smi` gives them on the RTX 3080 (its maximum 2,115 MHz, the memory
 * at 9,501 MHz in P0), or at the maximum `maxMHz` given.
 */
function nvidiaClocksAt(
  graphicsMHz: number,
  { memoryMHz = 9501, state = 0, maxMHz = 2115 } = {},
): GpuClockSample {
  return {
    kind: "clocks",
    source: "nvidia-smi",
    maxGraphicsMHz: measured(maxMHz),
    graphicsMHz: measured(graphicsMHz),
    memoryMHz: measured(memoryMHz),
    performanceState: measured(state),
  };
}

function sample(tS: number, nvidiaDeviceBytes: number | null): MemorySample {
  return {
    tS,
    appBytes: 800 * MIB,
    gpuProcessBytes: 200 * MIB,
    tracingBytes: 400 * MIB,
    rendererPrivateBytes: 300 * MIB,
    drmResidentBytes: null,
    drmReason: "no DRM client in the process's fdinfo (NVIDIA's driver writes none)",
    drmTotalBytes: null,
    nvidiaDeviceBytes,
    nvidiaGpuProcessBytes: null,
    clocks: nvidiaClocksAt(1980),
  };
}

const MEMORY = [sample(0, 400 * MIB), sample(1, 700 * MIB), sample(2, 650 * MIB)];

const NO_DRM = "no DRM client in the process's fdinfo (NVIDIA's driver writes none)";

/**
 * Samples whose renderer has not reported at the first two and the fifth, and whose DRM reading
 * is missing for one reason at the first two and another at the fourth.
 */
function gappedMemory(): MemorySample[] {
  const drm = (
    i: number,
  ): Pick<MemorySample, "drmResidentBytes" | "drmTotalBytes" | "drmReason"> =>
    i < 2
      ? { drmResidentBytes: null, drmTotalBytes: null, drmReason: "no GPU process" }
      : i === 3
        ? { drmResidentBytes: null, drmTotalBytes: null, drmReason: "fdinfo unreadable" }
        : { drmResidentBytes: (100 + i) * MIB, drmTotalBytes: (120 + i) * MIB, drmReason: null };
  return [0, 1, 2, 3, 4, 5].map((i) =>
    Object.assign(sample(i + 0.2382, (400 + i) * MIB), drm(i), {
      rendererPrivateBytes: i < 2 || i === 4 ? null : (300 + i) * MIB,
    }),
  );
}

/**
 * A run of `count` samples at 1 Hz in the RTX 3080's layout (no DRM reading), whose readings wander
 * as the T14.c hidden run's did, over as many digits.
 */
function longRun(count: number): MemorySample[] {
  let state = 1;
  // A Lehmer generator: the same readings every run.
  const next = (): number => {
    state = (state * 48_271) % 2_147_483_647;
    return state / 2_147_483_647;
  };
  const kib = (base: number, spread: number): number => Math.round(base + spread * next()) * 1024;
  return Array.from({ length: count }, (_, i) => ({
    tS: i + 0.2 + 0.1 * next(),
    appBytes: kib(700_000 + 900 * i, 50_000),
    gpuProcessBytes: kib(240_000, 180_000),
    tracingBytes: kib(43_000 + 470 * i, 20_000),
    rendererPrivateBytes: kib(190_000 + 370 * i, 30_000),
    drmResidentBytes: null,
    drmReason: NO_DRM,
    drmTotalBytes: null,
    nvidiaDeviceBytes: (404 + Math.round(4 * next())) * MIB,
    nvidiaGpuProcessBytes: (186 + Math.round(6 * next())) * MIB,
    // The driver's clocks wander between P5 and P0 as T14's diagnosis saw them.
    clocks: nvidiaClocksAt(15 * Math.round(60 + 72 * next()), {
      memoryMHz: next() < 0.2 ? 810 : 9501,
      state: Math.round(5 * next()),
    }),
  }));
}

/** A results file built from `memory`, as the writer writes it. */
function fileOf(memory: ReadonlyArray<MemorySample>): string {
  const results = buildResults({
    run: runOf({ shown: false, displayHz: null }),
    report: reportOf(),
    trace: missing(EMPTY_TRACE_REASON),
    memory,
  });
  return `${JSON.stringify(results, null, 2)}\n`;
}

/** The repository's Prettier, with its own configuration, on a results file. */
async function prettier(text: string): Promise<string> {
  const config = await resolveConfig(join(__dirname, "results.json"));
  return format(text, { ...config, parser: "json" });
}

function memoryFiles(): ResultsFiles & { readonly written: Map<string, string> } {
  const written = new Map<string, string>();
  return {
    written,
    mkdir: () => Promise.resolve(),
    exists: (path) => Promise.resolve(written.has(path)),
    writeFile: (path, text) => {
      written.set(path, text);
      return Promise.resolve();
    },
  };
}

function rowOf(results: DescentResults, id: string): DescentResults["criteria"]["whole"][number] {
  const found = results.criteria.whole.find((entry) => entry.id === id);
  if (found === undefined) {
    throw new Error(`no row ${id}`);
  }
  return found;
}

describe("frame statistics", () => {
  it("take percentiles by nearest rank", () => {
    const values = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    expect(nearestRank(values, 0.5)).toBe(5);
    expect(nearestRank(values, 0.95)).toBe(10);
    expect(nearestRank(values, 0.1)).toBe(1);
  });

  it("count missed frames above 1.5 T and hitches above 3 T", () => {
    const stats = frameStats([16.7, 16.7, 16.7, 26, 30, 51, 16.6, 16.7], 16.68);
    expect(stats).toEqual({
      count: 8,
      p50Ms: 16.7,
      p95Ms: 51,
      p99Ms: 51,
      maxMs: 51,
      missed: 3,
      missedFraction: 3 / 8,
      hitches: 1,
    });
  });

  it("leave the missed and hitch counts null without a period", () => {
    expect(frameStats([10, 20], null)).toMatchObject({ missed: null, hitches: null });
    expect(frameStats([], 16.68)).toBeNull();
  });
});

describe("a results file", () => {
  it("parses against its schema once written, with its summary beside it", async () => {
    const files = memoryFiles();
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    const paths = await writeResults("/runs", results, files);
    expect(paths).toEqual({
      json: "/runs/2026-10-02-devbox-low.json",
      markdown: "/runs/2026-10-02-devbox-low.md",
    });
    const parsed: unknown = JSON.parse(files.written.get(paths.json) ?? "");
    expect(validateResults(parsed)).toEqual([]);
    expect(files.written.get(paths.markdown)).toContain("# Descent spike: devbox, low, 2026-10-02");
  });

  it("does not overwrite a run of the same day, machine and setting", async () => {
    const files = memoryFiles();
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    await writeResults("/runs", results, files);
    const second = await writeResults("/runs", results, files);
    expect(second.json).toBe("/runs/2026-10-02-devbox-low-2.json");
  });

  it("reads frames from presentation times by segment, after the warm-up, against T", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(results.run.periodMs).toEqual(measured(2000 / 60));
    expect(results.frames.source).toBe("presentation");
    // 900 presentations over 30 s, at 0.005 s + i / 30; an interval with an end in the first 10 s
    // is the warm-up's, which leaves 301 → 899, each in the segment of its end.
    expect(results.frames.presentation.value?.count).toBe(599);
    expect(
      results.frames.segments.map(({ segment, presentation }) => [
        segment,
        presentation.value?.count,
      ]),
    ).toEqual([
      ["orbit coast", 299],
      ["descent arc", 300],
    ]);
    // The drop at 5 s is the warm-up's.
    expect(results.frames.dropped).toEqual(measured(2));
    expect(results.frames.excludedFrames).toBe(0);
    expect(rowOf(results, "p50")).toMatchObject({ verdict: "pass" });
    expect(results.criteria.segments.map(({ segment }) => segment)).toEqual([
      "orbit coast",
      "descent arc",
    ]);
  });

  it("states a hidden run's missing presentation times and period, not a verdict on them", () => {
    const results = buildResults({
      run: runOf({ shown: false, displayHz: null }),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(results.frames.presentation).toEqual(missing("no window shown"));
    expect(results.run.periodMs).toEqual(missing("no window shown"));
    expect(results.frames.source).toBe("raf");
    expect(results.frames.raf.value?.count).toBe(400);
    for (const id of ["p50", "p95", "p99", "missed", "hitches", "headroom-main", "headroom-gpu"]) {
      expect(rowOf(results, id)).toMatchObject({
        verdict: "not-measured",
        note: "no window shown",
      });
    }
    expect(results.criteria.overall).toBe("not-measured");
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("records the timer, platform and launch mode, and a quantized timer's tolerance", () => {
    const results = buildResults({
      run: runOf({ platform: "win32", launchMode: "default" }),
      report: reportOf({ timer: "quantized" }),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(results.run).toMatchObject({
      timer: "quantized",
      platform: "win32",
      launchMode: "default",
    });
    expect(results.gpu.tolerancePerPassMs).toBe(TIMESTAMP_QUANTUM_MS);
    // Three passes a frame at most, each ± one quantum.
    expect(rowOf(results, "headroom-gpu").tolerance).toBeCloseTo(3 * TIMESTAMP_QUANTUM_MS, 12);
    // The terrain and atmosphere rows' two passes a frame.
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW).tolerance).toBeCloseTo(
      2 * TIMESTAMP_QUANTUM_MS,
      12,
    );
  });

  it("calls a row marginal within the quantized timer's tolerance of its limit", () => {
    const frames = reportOf().frames;
    const atLimit = reportOf({
      timer: "quantized",
      frames: {
        ...frames,
        passes: [{ label: "terrain", row: "terrain", gpuMs: frames.scriptTimesS.map(() => 18.03) }],
      },
    });
    const results = buildResults({
      run: runOf(),
      report: atLimit,
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      limit: 18,
      verdict: "marginal",
    });
    const exact = buildResults({
      run: runOf(),
      report: { ...atLimit, timer: "full" },
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(rowOf(exact, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({ verdict: "fail" });
  });

  it("reads the GPU rows against the setting's limits", () => {
    const high = buildResults({
      run: runOf({ setting: "high" }),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    // 6 ms of terrain and 1.5 of atmosphere a frame, against 5 + 1 on high and 14 + 4 on low.
    expect(rowOf(high, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      criterion:
        "terrain and atmosphere GPU time, summed per frame, ≤ 6 ms (5 + 1) at the 95th percentile",
      limit: 6,
      value: 7.5,
      verdict: "fail",
    });
    expect(rowOf(high, "headroom-gpu")).toMatchObject({ value: 8, verdict: "pass" });
    const low = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(rowOf(low, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      criterion:
        "terrain and atmosphere GPU time, summed per frame, ≤ 18 ms (14 + 4) at the 95th percentile",
      limit: 18,
      value: 7.5,
      verdict: "pass",
    });
  });

  it("records the patch counts and demand under both bounds", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(results.streaming).toEqual([
      expect.objectContaining({
        patchesHard: 900,
        patchesCalibrated: 380,
        sustainedFractionHard: 4 / 5,
        sustainedFractionCalibrated: 2,
      }),
    ]);
  });

  it("takes NVIDIA's memory less its baseline as the headline, the tracing service apart", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(results.memory.gpuHeadline).toEqual(
      measured({ bytes: (700 - 168) * MIB, source: "nvidia-smi" }),
    );
    expect(results.memory.peakTracingBytes).toEqual(measured(400 * MIB));
    expect(results.memory.peakDrmResidentBytes.value).toBeNull();
    expect(results.memory.peakDrmResidentBytes.reason).toContain("NVIDIA");
    expect(rowOf(results, "memory")).toMatchObject({ limit: 1e9, verdict: "pass" });
  });

  it("falls back to the adapter's tally with a note where no driver reads memory", () => {
    const results = buildResults({
      run: runOf({ nvidiaBaselineBytes: null }),
      report: reportOf(),
      trace: traceOf(),
      memory: [sample(0, null)],
    });
    expect(results.memory.gpuHeadline).toEqual(
      measured({ bytes: 300 * MIB, source: "adapter-tally" }),
    );
    expect(rowOf(results, "memory").note).toBe("read from the adapter's tally: no driver reading");
  });

  it("marks a run on a busy machine provisional", () => {
    const busy = { ...MACHINE, loadAverage: [14.2, 12, 10] as const };
    const results = buildResults({
      run: runOf({ machine: busy }),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(results.run.quiet.provisional).toBe(true);
    expect(summaryMarkdown(results)).toContain("provisional");
  });

  it("marks every run on Windows provisional, since Windows keeps no load average", () => {
    expect(windowsResults().run.quiet).toEqual({
      provisional: true,
      note: "Windows keeps no load average: the quiet-machine rule (Design note 27) is unchecked",
    });
  });

  it("says in the summary why a run on Windows is provisional", () => {
    expect(summaryMarkdown(windowsResults())).toContain(
      "(provisional: Windows keeps no load average)",
    );
  });

  it("states no load average in a Windows run's summary, not its zeros", () => {
    expect(summaryMarkdown(windowsResults())).toContain(
      "load average none (Windows keeps no load average)",
    );
  });

  it("records no load average for a Windows run, with the reason, not its zeros", () => {
    expect(windowsResults().run.machine.loadAverage).toEqual(
      missing("Windows keeps no load average"),
    );
  });

  it("validates a Windows run's file, which has no load average", () => {
    expect(validateResults(JSON.parse(JSON.stringify(windowsResults())))).toEqual([]);
  });

  it("records a macOS run's load average as measured", () => {
    expect(macResults().run.machine.loadAverage).toEqual(measured([0.5, 0.4, 0.3]));
  });

  it("does not mark a run on macOS at a load of 0.5 provisional", () => {
    expect(macResults().run.quiet).toEqual({ provisional: false, note: null });
  });

  it("states a macOS run's load average in its summary", () => {
    expect(summaryMarkdown(macResults())).toContain("load average 0.50, 0.40, 0.30");
  });

  it("states why the trace's figures are missing when there is no trace", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: missing("the trace was not recorded"),
      memory: MEMORY,
    });
    expect(results.mainThread.gc).toEqual(missing("the trace was not recorded"));
    expect(results.frames.source).toBe("raf");
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("takes a trace with no timed event as no trace, not as zero drops and pauses", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      // What the reducer gives for a file with no events, as Chromium's crashed export left.
      trace: recordingOf([
        windowFile(new TraceReducer({ categories: UNPROFILED.categories }).figures()),
      ]),
      memory: MEMORY,
    });
    const empty = missing(`trace window 1 of 1: ${EMPTY_TRACE_REASON}`);
    expect(results.run.trace.value?.windows[0]?.figures).toEqual(missing(EMPTY_TRACE_REASON));
    expect(results.frames.presentation).toEqual(empty);
    expect(results.frames.segments.map(({ presentation }) => presentation)).toEqual([empty, empty]);
    expect(results.frames.dropped).toEqual(empty);
    expect(results.gpu.gpuProcess).toEqual(empty);
    expect(results.mainThread.split).toEqual(empty);
    expect(results.mainThread.engine).toEqual(empty);
    expect(results.mainThread.gc).toEqual(empty);
    expect(results.frames.source).toBe("raf");
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });
});

/** The figures read from a trace, by path, as one line each. */
function traceFigures(results: DescentResults): Readonly<Record<string, unknown>> {
  return {
    "frames.presentation": results.frames.presentation,
    ...Object.fromEntries(
      results.frames.segments.map(({ segment, presentation }) => [
        `frames.segments[${segment}].presentation`,
        presentation,
      ]),
    ),
    "frames.dropped": results.frames.dropped,
    "gpu.gpuProcess": results.gpu.gpuProcess,
    "mainThread.split": results.mainThread.split,
    "mainThread.engine": results.mainThread.engine,
    "mainThread.gc": results.mainThread.gc,
  };
}

/** {@link twoWindowReport}'s results, from `windows`, unprofiled unless `settings` says otherwise. */
function twoWindowResults(
  windows: ReadonlyArray<TraceWindowFile> = twoWindowTraces().map(windowFile),
  settings: TraceSettings = UNPROFILED,
): DescentResults {
  return buildResults({
    run: runOf(),
    report: twoWindowReport(),
    trace: recordingOf(windows, settings),
    memory: MEMORY,
  });
}

/** The names of a results file's GPU-process slices, in order. */
function sliceNames(results: DescentResults): ReadonlyArray<string> | undefined {
  return results.gpu.gpuProcess.value?.slices.map(({ name }) => name);
}

/**
 * {@link reportOf}'s report with frame i's terrain and atmosphere times those given, its tone pass
 * kept.
 */
function restOfFrameReport(
  terrainMs: (i: number) => number,
  atmosphereMs: (i: number) => number,
): DescentSpikeReport {
  const base = reportOf();
  return {
    ...base,
    frames: {
      ...base.frames,
      passes: base.frames.passes.map((pass) => {
        const timeAt =
          pass.row === "terrain" ? terrainMs : pass.row === "atmosphere" ? atmosphereMs : null;
        return timeAt === null
          ? pass
          : Object.assign({}, pass, { gpuMs: pass.gpuMs.map((_, i) => timeAt(i)) });
      }),
    },
  };
}

/** {@link reportOf}'s report with no pass timed in any frame. */
function untimedPassesReport(): DescentSpikeReport {
  const base = reportOf();
  return { ...base, frames: { ...base.frames, passes: [] } };
}

/** `report`'s results on the high setting, whose estimates are 5 ms of terrain and 1 of atmosphere. */
function highResults(report: DescentSpikeReport): DescentResults {
  return buildResults({
    run: runOf({ setting: "high" }),
    report,
    trace: traceOf(),
    memory: MEMORY,
  });
}

describe("a results file's rest of the frame (decision-r05-high-atmosphere.md)", () => {
  it("passes terrain at 4.5 and atmosphere at 1.4 ms on high", () => {
    const results = highResults(
      restOfFrameReport(
        () => 4.5,
        () => 1.4,
      ),
    );
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      limit: 6,
      value: expect.closeTo(5.9, 12),
      verdict: "pass",
    });
  });

  it("records the atmosphere at 1.4 ms as over its 1 ms estimate, and terrain at 4.5 as within its 5", () => {
    const results = highResults(
      restOfFrameReport(
        () => 4.5,
        () => 1.4,
      ),
    );
    expect(results.gpu.rows).toEqual({
      terrain: {
        estimateMs: 5,
        p50Ms: measured(4.5),
        p95Ms: measured(4.5),
        p99Ms: measured(4.5),
        overEstimate: false,
      },
      atmosphere: {
        estimateMs: 1,
        p50Ms: measured(1.4),
        p95Ms: measured(1.4),
        p99Ms: measured(1.4),
        overEstimate: true,
      },
    });
  });

  it("judges terrain and atmosphere as one criterion row, with neither alone", () => {
    const results = highResults(reportOf());
    expect(results.criteria.whole.map(({ id }) => id)).toEqual([
      "p50",
      "p95",
      "p99",
      "missed",
      "hitches",
      "headroom-main",
      "headroom-gpu",
      TERRAIN_ATMOSPHERE_ROW,
      "memory",
    ]);
  });

  it("prints each pass row beside its estimate in the summary, the finding where it is over", () => {
    const summary = summaryMarkdown(
      highResults(
        restOfFrameReport(
          () => 4.5,
          () => 1.4,
        ),
      ),
    );
    expect(summary).toContain(
      `| atmosphere | 1.00 ms | 1.40 ms | 1.40 ms | 1.40 ms | ${OVER_ESTIMATE_FINDING} |`,
    );
    expect(summary).toContain("| terrain | 5.00 ms | 4.50 ms | 4.50 ms | 4.50 ms |  |");
  });

  it("validates a file whose atmosphere is over its estimate", () => {
    const results = highResults(
      restOfFrameReport(
        () => 4.5,
        () => 1.4,
      ),
    );
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("fails 6.2 ms of terrain and atmosphere on high", () => {
    const results = highResults(
      restOfFrameReport(
        () => 4.8,
        () => 1.4,
      ),
    );
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      limit: 6,
      value: expect.closeTo(6.2, 12),
      verdict: "fail",
    });
  });

  it("takes the 95th percentile of each frame's sum, not the sum of the two percentiles", () => {
    // A tenth of the frames each has its terrain at 5 ms or its atmosphere at 4, never both: each
    // row's 95th percentile is its slow value, 9 ms between them, but no frame's sum is over 5.5.
    const results = highResults(
      restOfFrameReport(
        (i) => (i % 10 === 0 ? 5 : 1),
        (i) => (i % 10 === 5 ? 4 : 0.5),
      ),
    );
    expect([results.gpu.rows.terrain.p95Ms, results.gpu.rows.atmosphere.p95Ms]).toEqual([
      measured(5),
      measured(4),
    ]);
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({ value: 5.5, verdict: "pass" });
  });

  it("reads no verdict from the split between terrain and atmosphere", () => {
    // The same 5.75 ms a frame, with the atmosphere over its estimate in one run, terrain in
    // another, and neither in the third.
    const neitherOver = highResults(
      restOfFrameReport(
        () => 4.75,
        () => 1,
      ),
    );
    const atmosphereOver = highResults(
      restOfFrameReport(
        () => 4.5,
        () => 1.25,
      ),
    );
    const terrainOver = highResults(
      restOfFrameReport(
        () => 5.25,
        () => 0.5,
      ),
    );
    expect(
      [neitherOver, atmosphereOver, terrainOver].map(({ gpu }) => [
        gpu.rows.terrain.overEstimate,
        gpu.rows.atmosphere.overEstimate,
      ]),
    ).toEqual([
      [false, false],
      [false, true],
      [true, false],
    ]);
    expect([atmosphereOver.criteria, terrainOver.criteria]).toEqual([
      neitherOver.criteria,
      neitherOver.criteria,
    ]);
  });

  it("reads each pass row over the complete frames, as the joint row does", () => {
    // The five partial frames' 100 ms terrain times, of 400, are in the pass's own percentiles alone.
    const results = incompleteResults([300, 301, 302, 303, 304], []);
    expect(results.gpu.passes.value?.find(({ label }) => label === "terrain")?.p99Ms).toBe(100);
    expect(results.gpu.rows.terrain).toMatchObject({ p99Ms: measured(6), overEstimate: false });
  });

  it("states why the joint row is not measured with no timed pass", () => {
    const results = highResults(untimedPassesReport());
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      verdict: "not-measured",
      note: "no timed terrain or atmosphere pass",
    });
  });

  it("states why a pass row has no figures", () => {
    expect(highResults(untimedPassesReport()).gpu.rows.atmosphere).toEqual({
      estimateMs: 1,
      p50Ms: missing("no timed atmosphere pass"),
      p95Ms: missing("no timed atmosphere pass"),
      p99Ms: missing("no timed atmosphere pass"),
      overEstimate: null,
    });
  });

  it("prints a pass row's missing figures with their reason in the summary", () => {
    expect(summaryMarkdown(highResults(untimedPassesReport()))).toContain(
      "| atmosphere | 1.00 ms | — | — | — | no timed atmosphere pass |",
    );
  });

  it("names each missing percentile with its reason when the three's reasons differ", () => {
    const results = highResults(
      restOfFrameReport(
        () => 4.5,
        () => 1.4,
      ),
    );
    const converted: DescentResults = {
      ...results,
      gpu: {
        ...results.gpu,
        rows: {
          ...results.gpu.rows,
          atmosphere: {
            ...results.gpu.rows.atmosphere,
            p50Ms: missing("not recorded"),
            p99Ms: missing("not recorded"),
          },
        },
      },
    };
    expect(summaryMarkdown(converted)).toContain(
      `| atmosphere | 1.00 ms | — | 1.40 ms | — | ${OVER_ESTIMATE_FINDING}; p50, p99: not recorded |`,
    );
  });
});

describe("a results file of a windowed trace", () => {
  it("leaves a frame in a boundary's exclusion out of every per-frame figure and counts it", () => {
    const results = twoWindowResults();
    // The 30 frames at 15, 15.05, …, 16.45 s are the boundary's.
    expect(results.run.trace.value?.boundaries).toEqual([
      expect.objectContaining({ excludedFrames: 30, maxRafIntervalMs: 500 }),
    ]);
    expect(results.frames.excludedFrames).toBe(30);
    expect(
      results.frames.segments.map(({ segment, excludedFrames }) => [segment, excludedFrames]),
    ).toEqual([
      ["orbit coast", 30],
      ["descent arc", 0],
    ]);
    // The rAF figures: 400 frames after the warm-up, less the 30; their 500 ms intervals gone.
    expect(results.frames.raf.value).toMatchObject({ count: 370, maxMs: 120 });
    expect(results.frames.segments[0]?.raf.value).toMatchObject({ count: 170, maxMs: 120 });
    // Our code's 40 ms and the terrain's 50 ms are the excluded frames' alone.
    expect(results.mainThread.ourCodeP95Ms).toEqual(measured(4));
    expect(results.gpu.rows.terrain.p99Ms).toEqual(measured(6));
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW).value).toBe(7.5);
    expect(results.gpu.passes.value?.find(({ label }) => label === "terrain")).toMatchObject({
      frames: 370,
      p99Ms: 6,
    });
    expect(results.gpu.sumP95Ms).toEqual(measured(8));
    // Presentations: the first window's 301 → 449, the second's from 16.505 s on; none across the
    // gap, and not the second window's two 500 ms apart in the exclusion.
    expect(results.frames.presentation.value?.count).toBe(149 + 403);
    expect(results.frames.presentation.value?.maxMs).toBeLessThan(34);
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("pools the windows' presentation intervals by the segment of their ends' script time", () => {
    const results = twoWindowResults();
    // 16.505 + k / 30 s ends in the orbit coast to k = 104.
    expect(
      results.frames.segments.map(({ segment, presentation }) => [
        segment,
        presentation.value?.count,
      ]),
    ).toEqual([
      ["orbit coast", 149 + 104],
      ["descent arc", 403 - 104],
    ]);
  });

  it.each<readonly [string, (second: TraceFigures) => TraceWindowFile, string]>([
    [
      "a missing file",
      () => ({
        trace: missing("its file could not be read: ENOENT"),
        bytes: null,
        bufferPercent: null,
        lostData: false,
      }),
      "its file could not be read: ENOENT",
    ],
    [
      "a file that is not a trace",
      () => ({
        trace: missing("the trace could not be reduced: spike-trace-1.pftrace is not a trace"),
        bytes: 12,
        bufferPercent: null,
        lostData: false,
      }),
      "the trace could not be reduced: spike-trace-1.pftrace is not a trace",
    ],
    [
      "an empty file",
      () => windowFile(new TraceReducer({ categories: UNPROFILED.categories }).figures()),
      EMPTY_TRACE_REASON,
    ],
    [
      "no clock offset",
      (second) => windowFile({ ...second, clockOffsetUs: null }),
      NO_CLOCK_OFFSET_REASON,
    ],
    [
      "a short span",
      (second) =>
        windowFile({
          ...second,
          span: { firstUs: second.span?.firstUs ?? 0, lastUs: (second.span?.firstUs ?? 0) + 9e6 },
        }),
      "it filled its buffer: it spans 9.0 s of the 14.6 s recorded",
    ],
    [
      "a full buffer",
      (second) => ({ ...windowFile(second), bufferPercent: 99 }),
      "it filled its buffer: 99 % of it was used",
    ],
    ["lost data", (second) => ({ ...windowFile(second), lostData: true }), LOST_DATA_REASON],
    [
      "frame spans that disagree with the renderer's frames",
      (second) =>
        windowFile({
          ...second,
          mainThread:
            second.mainThread === null
              ? null
              : {
                  ...second.mainThread,
                  frameSpans: { startsUs: [], durationsMs: [], startTimesMs: [] },
                },
        }),
      // The second window checks the frames whose callbacks start 0.5 s inside [15.5, 30.1] s:
      // those of 16.0 s to 29.55 s, each 0.1 ms after its rAF time.
      "the trace's frame spans disagree with the renderer's (272 of 272 frames)",
    ],
    [
      "another renderer",
      (second) => windowFile({ ...second, frames: { ...second.frames, pid: 9 } }),
      "it shows another renderer than trace window 1: renderer 9, main thread 2, compositor 1, against renderer 1, main thread 2, compositor 1",
    ],
  ])("makes every trace figure missing for a window with %s", (_case, broken, reason) => {
    const [first, second] = twoWindowTraces();
    const results = twoWindowResults([windowFile(first), broken(second)], PROFILED);
    const fromTrace = traceFigures(results);
    const expected = missing(`trace window 2 of 2: ${reason}`);
    expect(fromTrace).toEqual(
      Object.fromEntries(Object.keys(fromTrace).map((path) => [path, expected])),
    );
    // The windows still show which one failed.
    expect(results.run.trace.value?.windows.map(({ figures }) => figures.value === null)).toEqual([
      false,
      true,
    ]);
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("lists GPUTask alone in an unprofiled run, and all three GPU-process slices in a profiled one", () => {
    const unprofiled = twoWindowResults();
    const profiled = twoWindowResults(undefined, PROFILED);
    expect(sliceNames(unprofiled)).toEqual(["GPUTask"]);
    expect(sliceNames(profiled)).toEqual(["WebGPU", "GPUTask", "VulkanQueueSubmitHook"]);
    expect(validateResults(JSON.parse(JSON.stringify(unprofiled)))).toEqual([]);
    expect(validateResults(JSON.parse(JSON.stringify(profiled)))).toEqual([]);
  });

  it("records the trace's format", () => {
    expect(twoWindowResults().run.trace.value?.format).toBe("perfetto-proto");
    expect(
      twoWindowResults(undefined, { ...UNPROFILED, format: "json" }).run.trace.value?.format,
    ).toBe("json");
  });

  it("leaves the engine's figures out of an unprofiled run, and refuses them there", () => {
    const results = twoWindowResults();
    expect(results.mainThread.engine).toEqual(missing(PROFILER_OFF_REASON));
    expect(results.mainThread.split.value).not.toHaveProperty("engineSelfMs");
    const broken: unknown = JSON.parse(
      JSON.stringify({
        ...results,
        mainThread: { ...results.mainThread, engine: measured({ selfMs: 1, sampledMs: 2 }) },
      }),
    );
    expect(validateResults(broken)).toEqual(["mainThread.engine is measured in an unprofiled run"]);
  });

  it("records a profiled run's engine figures, summed over its windows", () => {
    const results = twoWindowResults(undefined, PROFILED);
    expect(results.run.trace.value?.profiled).toBe(true);
    expect(results.mainThread.engine).toEqual(measured({ selfMs: 20, sampledMs: 80 }));
  });

  it("names a profiled run's file apart, numbering a second as any other", async () => {
    const results = twoWindowResults(undefined, PROFILED);
    const files = memoryFiles();
    const first = await writeResults("/runs", results, files);
    const second = await writeResults("/runs", results, files);
    expect([first.json, second.json]).toEqual([
      "/runs/2026-10-02-devbox-low-profiled.json",
      "/runs/2026-10-02-devbox-low-profiled-2.json",
    ]);
  });

  it("heads a profiled run's summary as a diagnostic, not judged", () => {
    expect(summaryMarkdown(twoWindowResults(undefined, PROFILED))).toContain(
      "**PROFILED: diagnostic, not judged; renderer and app memory include the CPU profiler's samples.**",
    );
    expect(summaryMarkdown(twoWindowResults())).not.toContain("PROFILED");
  });

  it("names a failed window in the summary's trace line", () => {
    const [first, second] = twoWindowTraces();
    const summary = summaryMarkdown(
      twoWindowResults([windowFile(first), windowFile({ ...second, clockOffsetUs: null })]),
    );
    expect(summary).toContain(`; window 2 failed: ${NO_CLOCK_OFFSET_REASON}`);
  });

  it("summarises the windows, their boundaries and the frames left out", () => {
    const summary = summaryMarkdown(twoWindowResults());
    expect(summary).toContain(
      "- **Trace:** 2 perfetto-proto windows, 18.5 s traced after the warm-up, unprofiled; 1 boundary left out 30 frames (largest stall 500.00 ms); largest file 1 MiB, buffer use up to 10 %",
    );
    expect(summary).toContain("Frames left out at the trace's window boundaries: 30.");
  });

  it("keeps a 3,600-sample run with ten windows under 512,000 bytes once Prettier formats it", async () => {
    const windowMs = 360_000;
    const traceWindows = Array.from({ length: 10 }, (_, k) => ({
      startedMs: at(k * 360 + (k === 0 ? -0.1 : 2)),
      stopRequestedMs: at((k + 1) * 360),
      failure: null,
    }));
    const results = buildResults({
      run: runOf({ shown: false, displayHz: null }),
      report: reportOf({
        traceWindows,
        segments: [{ name: "orbit coast", startS: 0, endS: 3600 }],
      }),
      trace: recordingOf(
        traceWindows.map(({ startedMs }) =>
          windowFile(
            windowTrace({
              offsetUs: 7e9,
              fromMs: startedMs,
              toMs: startedMs + windowMs,
              frames: reportOf(),
            }),
          ),
        ),
      ),
      memory: longRun(3600),
    });
    expect(results.run.trace.value?.boundaries).toHaveLength(9);
    const formatted = await prettier(`${JSON.stringify(results, null, 2)}\n`);
    expect(Buffer.byteLength(formatted, "utf8")).toBeLessThan(512_000);
  });
});

describe("the memory series", () => {
  it("holds the times in whole ms and each reading in whole KiB, with its peak in bytes", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: gappedMemory(),
    });
    const { series } = results.memory;
    expect(series.tMs).toEqual([238, 1238, 2238, 3238, 4238, 5238]);
    expect(series.appKiB).toEqual(
      measured({ samples: [800, 800, 800, 800, 800, 800].map((mib) => mib * 1024), gaps: [] }),
    );
    expect(series.nvidiaDeviceKiB.value?.samples).toEqual(
      [400, 401, 402, 403, 404, 405].map((mib) => mib * 1024),
    );
    expect(results.memory.peakAppBytes).toEqual(measured(800 * MIB));
    expect(results.memory.peakRendererPrivateBytes).toEqual(measured(305 * MIB));
    expect(results.memory.peakDrmResidentBytes).toEqual(measured(105 * MIB));
    expect(results.memory.peakNvidiaDeviceLessBaselineBytes).toEqual(measured((405 - 168) * MIB));
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("writes -1 at a gap's samples and nowhere else, one gap a reason and stretch", () => {
    const series = memorySeries(gappedMemory());
    expect(series.rendererPrivateKiB).toEqual(
      measured({
        samples: [-1, -1, 302 * 1024, 303 * 1024, -1, 305 * 1024],
        gaps: [
          { from: 0, to: 1, reason: "the renderer reported no memory" },
          { from: 4, to: 4, reason: "the renderer reported no memory" },
        ],
      }),
    );
    expect(series.drmResidentKiB).toEqual(
      measured({
        samples: [-1, -1, 102 * 1024, -1, 104 * 1024, 105 * 1024],
        gaps: [
          { from: 0, to: 1, reason: "no GPU process" },
          { from: 3, to: 3, reason: "fdinfo unreadable" },
        ],
      }),
    );
  });

  it("writes a reading missing all run once, as null with its reason", () => {
    const series = memorySeries(MEMORY);
    expect(series.drmResidentKiB).toEqual(missing(NO_DRM));
    expect(series.drmTotalKiB).toEqual(missing(NO_DRM));
    expect(series.nvidiaGpuProcessKiB).toEqual(missing("nvidia-smi does not list the GPU process"));
    const none = memorySeries([]);
    expect(none.tMs).toEqual([]);
    expect(none.appKiB).toEqual(missing("no memory sample"));
  });

  it("keeps a 3,600-sample run's file under 512,000 bytes once Prettier formats it", async () => {
    const formatted = await prettier(fileOf(longRun(3600)));
    expect(Buffer.byteLength(formatted, "utf8")).toBeLessThan(512_000);
  });

  it.each([
    ["a long run's", () => longRun(600)],
    ["a run with gaps'", gappedMemory],
    ["a run with no sample's", () => []],
  ])("measures %s formatted size as Prettier formats the file", async (_run, memory) => {
    const text = fileOf(memory());
    expect(formatAsPrettier(text)).toBe(await prettier(text));
  });

  it("writes a file over the limit in full and warns of its size", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const files = memoryFiles();
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: longRun(8000),
    });
    const paths = await writeResults("/runs", results, files);
    const text = files.written.get(paths.json) ?? "";
    expect(text).toBe(`${JSON.stringify(results, null, 2)}\n`);
    expect(results.memory.series.tMs).toHaveLength(8000);
    const bytes = Buffer.byteLength(await prettier(text), "utf8");
    expect(bytes).toBeGreaterThan(ADDED_FILE_LIMIT_BYTES);
    expect(warn).toHaveBeenCalledExactlyOnceWith(
      expect.stringContaining(`is ${bytes} B once formatted`),
    );
  });

  it("does not warn of a file under the limit", async () => {
    const warn = vi.spyOn(console, "warn");
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    await writeResults("/runs", results, memoryFiles());
    expect(warn).not.toHaveBeenCalled();
  });
});

describe("a native replay's results", () => {
  it("summarise the GPU's frame intervals an offscreen replay records", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    const replay: DescentResults = {
      ...results,
      run: { ...results.run, launchMode: "native-replay" },
      frames: { ...results.frames, source: "gpu-completion", gpuCompletion: results.frames.raf },
    };
    expect(validateResults(JSON.parse(JSON.stringify(replay)))).toEqual([]);
    expect(summaryMarkdown(replay)).toContain(
      "Intervals from the GPU's ends of frames (an offscreen native replay).",
    );
  });
});

/** The gapped run's file with some of its series replaced. */
function withSeries(overrides: Readonly<Record<string, unknown>>): unknown {
  const results = buildResults({
    run: runOf(),
    report: reportOf(),
    trace: traceOf(),
    memory: gappedMemory(),
  });
  return JSON.parse(
    JSON.stringify({
      ...results,
      memory: { ...results.memory, series: { ...results.memory.series, ...overrides } },
    }),
  );
}

/** The gapped run's renderer gaps. */
const RENDERER_GAPS = [
  { from: 0, to: 1, reason: "the renderer reported no memory" },
  { from: 4, to: 4, reason: "the renderer reported no memory" },
];

/** KiB in a MiB, for readings written in KiB. */
const KIB_PER_MIB = 1024;

/** A renderer column with other samples or gaps than the gapped run's. */
function renderer(samples: ReadonlyArray<unknown>, gaps: ReadonlyArray<unknown> = RENDERER_GAPS) {
  return measured({ samples, gaps });
}

/** An edit of a parsed file: the path to a field, and its new value or {@link DELETE}. */
type Edit = readonly [ReadonlyArray<string | number>, unknown];

/** An {@link Edit}'s value that removes its field. */
const DELETE = Symbol("delete");

/** Sets, or with {@link DELETE} removes, the field at `path` of a parsed file. */
function editAt(file: unknown, path: ReadonlyArray<string | number>, value: unknown): void {
  const parent = path
    .slice(0, -1)
    .reduce<unknown>(
      (node, key) =>
        typeof node === "object" && node !== null ? Reflect.get(node, key) : undefined,
      file,
    );
  const key = path.at(-1);
  if (typeof parent !== "object" || parent === null || key === undefined) {
    throw new Error(`no field at ${path.join(".")}`);
  }
  if (value === DELETE) {
    Reflect.deleteProperty(parent, key);
  } else {
    Reflect.set(parent, key, value);
  }
}

/**
 * {@link reportOf}'s report with incomplete frames: at each of `partialAt`, one of five resolves
 * missing, its terrain time arriving as `partialTerrainMs` and its other passes' not; at each of
 * `droppedAt`, all five missing and no pass time.
 */
function incompleteReport(
  partialAt: ReadonlyArray<number>,
  droppedAt: ReadonlyArray<number>,
  partialTerrainMs = 100,
): DescentSpikeReport {
  const base = reportOf();
  const partial = new Set(partialAt);
  const dropped = new Set(droppedAt);
  const timeAt = (passRow: string, ms: number | null, i: number): number | null => {
    if (dropped.has(i)) {
      return null;
    }
    if (partial.has(i)) {
      return passRow === "terrain" ? partialTerrainMs : null;
    }
    return ms;
  };
  return {
    ...base,
    frames: {
      ...base.frames,
      missingResolves: base.frames.scriptTimesS.map((_, i) =>
        dropped.has(i) ? 5 : partial.has(i) ? 1 : 0,
      ),
      passes: base.frames.passes.map((pass) =>
        Object.assign({}, pass, { gpuMs: pass.gpuMs.map((ms, i) => timeAt(pass.row, ms, i)) }),
      ),
    },
  };
}

/** Frames 300 to 328 (15 to 16.4 s), after {@link reportOf}'s warm-up. */
const TWENTY_NINE = Array.from({ length: 29 }, (_, k) => 300 + k);

/** {@link incompleteReport}'s results on the `setting` given, its window shown at 60 Hz. */
function incompleteResults(
  partialAt: ReadonlyArray<number>,
  droppedAt: ReadonlyArray<number>,
  setting: RunDescription["setting"] = "low",
): DescentResults {
  return buildResults({
    run: runOf({ setting }),
    report: incompleteReport(partialAt, droppedAt),
    trace: traceOf(),
    memory: MEMORY,
  });
}

/** The GPU's clocks over {@link MEMORY}'s three samples, in a nvidia-smi reading's form. */
function clocksOf(overrides: Partial<GpuClocks> = {}): GpuClocks {
  return {
    source: "nvidia-smi",
    maxGraphicsMHz: measured(2100),
    graphicsMHz: measured({ samples: [1980, -1, 1100], gaps: [{ from: 1, to: 1, reason: "N/A" }] }),
    memoryMHz: measured({ samples: [9501, 9501, 405], gaps: [] }),
    performanceState: measured({ samples: [0, 0, 5], gaps: [] }),
    ...overrides,
  };
}

describe("a results file's incomplete pass times", () => {
  it("leaves partial and dropped frames out of every GPU sum, counted after the warm-up", () => {
    // 29 partial and one dropped after the warm-up, one of each in it (5 and 2.5 s).
    const results = incompleteResults([100, ...TWENTY_NINE], [50, 400]);
    expect(results.gpu.incompleteFrames).toEqual(
      measured({ dropped: 1, partial: 29, frames: 400 }),
    );
    // The partial frames' 100 ms terrain times enter no sum: every complete frame's is 6 ms, and
    // its sum 7.5 or 8 ms.
    expect(results.gpu.rows.terrain.p99Ms).toEqual(measured(6));
    expect(results.gpu.sumP95Ms).toEqual(measured(8));
    expect(results.gpu.rows.atmosphere.p99Ms).toEqual(measured(1.5));
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW).value).toBe(7.5);
  });

  it("counts no incomplete frame in a boundary's exclusion, which every per-frame figure leaves out", () => {
    // Frames 300 to 329 (15 to 16.45 s) are the boundary's; 301 and 400 are incomplete.
    const base = twoWindowReport();
    const report: DescentSpikeReport = {
      ...base,
      frames: {
        ...base.frames,
        missingResolves: base.frames.scriptTimesS.map((_, i) => (i === 301 || i === 400 ? 1 : 0)),
      },
    };
    const results = buildResults({
      run: runOf(),
      report,
      trace: recordingOf(twoWindowTraces().map(windowFile)),
      memory: MEMORY,
    });
    expect(results.frames.excludedFrames).toBe(30);
    expect(results.gpu.incompleteFrames).toEqual(measured({ dropped: 0, partial: 1, frames: 370 }));
    expect(results.gpu.incompleteFrames.value?.frames).toBe(results.frames.raf.value?.count);
  });

  it("keeps every time that arrived in each pass's own percentiles", () => {
    const results = incompleteResults(TWENTY_NINE, [400]);
    const terrain = results.gpu.passes.value?.find(({ label }) => label === "terrain");
    // 399 frames' terrain times arrived, the 29 partial frames' among them: 7.3% at 100 ms.
    expect(terrain).toMatchObject({ frames: 399, p50Ms: 6, p95Ms: 100 });
    const sky = results.gpu.passes.value?.find(({ label }) => label === "atmosphere.sky");
    expect(sky).toMatchObject({ frames: 370 });
  });

  it("does not measure a row that its incomplete frames could carry over its limit", () => {
    const results = incompleteResults(TWENTY_NINE, [400]);
    const reason = incompleteFramesReason({ frames: 30, inFlight: 0 });
    expect(reason).toBe(
      "30 frames' pass times were incomplete (the GPU was more than 27 frames behind)",
    );
    for (const [id, value] of [
      ["headroom-gpu", 8],
      [TERRAIN_ATMOSPHERE_ROW, 7.5],
    ] as const) {
      expect(rowOf(results, id)).toMatchObject({ value, verdict: "not-measured", note: reason });
    }
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("passes a row that passes with every incomplete frame above its limit", () => {
    const results = incompleteResults([300], [400]);
    expect(results.gpu.incompleteFrames.value).toMatchObject({ dropped: 1, partial: 1 });
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      value: 7.5,
      verdict: "pass",
      note: "2 frames with incomplete pass times left out; the verdict holds whatever their times",
    });
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("fails a row that fails with every incomplete frame below its limit", () => {
    // The high setting's limit is 5 + 1 ms, and the 370 complete frames take 6 + 1.5.
    const results = incompleteResults(TWENTY_NINE, [400], "high");
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      limit: 6,
      value: 7.5,
      verdict: "fail",
    });
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("gives row's own verdict with no incomplete frame", () => {
    const values = Array.from({ length: 100 }, (_, i) => 10 + i / 100);
    const p95 = values[94] ?? Number.NaN;
    for (const [limitMs, tolerance, verdict] of [
      [12, 0, "pass"],
      [p95, TIMESTAMP_QUANTUM_MS, "marginal"],
      [10.5, 0, "fail"],
    ] as const) {
      const bounded = boundedGpuRow(
        "terrain",
        "terrain",
        measured(limitMs),
        values,
        { frames: 0, inFlight: 0 },
        tolerance,
        "none",
      );
      expect(bounded).toEqual(
        row("terrain", "terrain", measured(limitMs), "ms", measured(p95), tolerance),
      );
      expect(bounded.verdict).toBe(verdict);
    }
  });

  it("leaves a row marginal when both placements are marginal", () => {
    const values = Array.from({ length: 100 }, () => 14.03);
    expect(
      boundedGpuRow(
        "terrain",
        "terrain",
        measured(14),
        values,
        { frames: 1, inFlight: 0 },
        TIMESTAMP_QUANTUM_MS,
        "none",
      ),
    ).toMatchObject({ value: 14.03, verdict: "marginal" });
  });

  it("states the count as the reason for a row with no complete frame", () => {
    const three = { frames: 3, inFlight: 0 };
    expect(boundedGpuRow("terrain", "terrain", measured(5), [], three, 0, "none")).toMatchObject({
      value: null,
      verdict: "not-measured",
      note: incompleteFramesReason(three),
    });
    expect(incompleteFramesReason({ frames: 1, inFlight: 0 })).toBe(
      "1 frame's pass times were incomplete (the GPU was more than 27 frames behind)",
    );
  });

  it("states the timer's absence for the count without a timer", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf({ timer: "absent" }),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(results.gpu.incompleteFrames).toEqual(
      missing("the pass timer is absent (no timestamp-query)"),
    );
  });

  it("counts no frame incomplete with no resolve missing", () => {
    const results = incompleteResults([], []);
    expect(results.gpu.incompleteFrames).toEqual(measured({ dropped: 0, partial: 0, frames: 400 }));
  });

  it("gives frames whose missing reads were in flight at the report that reason, not the backlog's", () => {
    // Of the 30 incomplete frames, the last 29 partial ones' single missing resolve was still in
    // flight; the dropped frame 400's five were not, so it is the backlog's.
    const base = incompleteReport(TWENTY_NINE, [400]);
    const report: DescentSpikeReport = {
      ...base,
      frames: {
        ...base.frames,
        inFlightResolves: base.frames.missingResolves.map((count, i) =>
          i >= 300 && i < 329 ? count : 0,
        ),
      },
    };
    const results = buildResults({ run: runOf(), report, trace: traceOf(), memory: MEMORY });
    const reason = incompleteFramesReason({ frames: 30, inFlight: 29 });
    expect(reason).toBe(
      "30 frames' pass times were incomplete (1 as the GPU was more than 27 frames behind, 29 read in flight at the report)",
    );
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      verdict: "not-measured",
      note: reason,
    });
    expect(results.gpu.incompleteFrames).toEqual(
      measured({ dropped: 1, partial: 29, frames: 400 }),
    );
  });

  it("names only the reads in flight when every incomplete frame's were", () => {
    expect(incompleteFramesReason({ frames: 2, inFlight: 2 })).toBe(
      `2 frames' pass times were incomplete (${READ_IN_FLIGHT_REASON})`,
    );
  });

  it("counts a frame with a resolve in flight and another dropped as the backlog's", () => {
    const base = incompleteReport([], [400]);
    const report: DescentSpikeReport = {
      ...base,
      frames: {
        ...base.frames,
        inFlightResolves: base.frames.missingResolves.map((_, i) => (i === 400 ? 4 : 0)),
      },
    };
    const results = buildResults({ run: runOf(), report, trace: traceOf(), memory: MEMORY });
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      verdict: "pass",
      note: "1 frame with incomplete pass times left out; the verdict holds whatever their times",
    });
  });

  it("names the frames in flight at the report in a verdict that holds", () => {
    const base = incompleteReport([300], []);
    const report: DescentSpikeReport = {
      ...base,
      frames: { ...base.frames, inFlightResolves: base.frames.missingResolves },
    };
    const results = buildResults({ run: runOf(), report, trace: traceOf(), memory: MEMORY });
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW).note).toBe(
      "1 frame with incomplete pass times left out (1 read in flight at the report); the verdict holds whatever their times",
    );
  });

  it("states the incomplete frames in the summary", () => {
    expect(summaryMarkdown(incompleteResults(TWENTY_NINE, [400]))).toContain(
      "- **Incomplete pass times:** 1 dropped and 29 partial of 400 frames after the warm-up, left out of the GPU rows' sums, whose verdicts hold whatever their times",
    );
    expect(summaryMarkdown(incompleteResults([], []))).toContain(
      "- **Incomplete pass times:** none of 400 frames after the warm-up",
    );
  });
});

/**
 * `count` samples at 1 Hz from 0.2 s in P2, the graphics clock at `beforeMHz` in the 10 s warm-up
 * and `afterMHz` after it, of a maximum of 1,980 MHz.
 */
function clockedMemory(count: number, beforeMHz: number, afterMHz: number): MemorySample[] {
  return Array.from({ length: count }, (_, i) => ({
    ...sample(i + 0.2, 400 * MIB),
    clocks: nvidiaClocksAt(i + 0.2 < 10 ? beforeMHz : afterMHz, { maxMHz: 1980, state: 2 }),
  }));
}

/** {@link reportOf}'s results over `memory`, its window shown at 60 Hz on the low setting. */
function clockedResults(
  memory: ReadonlyArray<MemorySample>,
  report: DescentSpikeReport = reportOf(),
): DescentResults {
  return buildResults({ run: runOf(), report, trace: traceOf(), memory });
}

/** `memory` with each sample's clocks those `clocksAt` gives, the others' kept. */
function reclocked(
  memory: ReadonlyArray<MemorySample>,
  clocksAt: (i: number) => GpuClockSample | null,
): MemorySample[] {
  return memory.map((entry, i) =>
    Object.assign({}, entry, { clocks: clocksAt(i) ?? entry.clocks }),
  );
}

/** `count` copies of `value`. */
function repeated(value: number, count: number): number[] {
  return Array.from({ length: count }, () => value);
}

/** The rows a GPU clock note may sit on. */
const GPU_ROWS = ["headroom-gpu", TERRAIN_ATMOSPHERE_ROW] as const;

/** The note of a run at a median 1,100 of 1,980 MHz. */
const NOTE_1100 = "measured at a median 1100 of 1980 MHz (the driver's choice at this load)";

describe("a results file's GPU clocks", () => {
  it("records every sample's clocks on the memory series' times", () => {
    const results = clockedResults(clockedMemory(30, 1980, 1100));
    expect(results.gpu.clocks).toEqual(
      measured({
        source: "nvidia-smi",
        maxGraphicsMHz: measured(1980),
        graphicsMHz: measured({
          samples: [...repeated(1980, 10), ...repeated(1100, 20)],
          gaps: [],
        }),
        memoryMHz: measured({ samples: repeated(9501, 30), gaps: [] }),
        performanceState: measured({ samples: repeated(2, 30), gaps: [] }),
      }),
    );
    expect(results.memory.series.tMs).toHaveLength(30);
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("notes a median of 1,100 of 1,980 MHz after the warm-up on the two GPU rows", () => {
    // The warm-up's ten samples at the maximum count for nothing.
    const results = clockedResults(clockedMemory(30, 1980, 1100));
    for (const id of GPU_ROWS) {
      expect(rowOf(results, id)).toMatchObject({ verdict: "pass", note: NOTE_1100 });
    }
    for (const id of ["p50", "headroom-main", "memory"]) {
      expect(rowOf(results, id).note ?? "").not.toContain("median");
    }
  });

  it("notes nothing at a median of 1,950 of 1,980 MHz", () => {
    const results = clockedResults(clockedMemory(30, 1100, 1950));
    for (const id of GPU_ROWS) {
      expect(rowOf(results, id).note).toBeNull();
    }
    expect(
      clockNote(results.gpu.clocks, results.memory.series.tMs, results.run.warmupS),
    ).toBeNull();
  });

  it("adds the note after a row's own", () => {
    const results = clockedResults(clockedMemory(30, 1980, 1100), incompleteReport([300], [400]));
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW).note).toBe(
      `2 frames with incomplete pass times left out; the verdict holds whatever their times; ${NOTE_1100}`,
    );
  });

  it("puts no note on a row with no value", () => {
    const results = clockedResults(clockedMemory(30, 1980, 1100), reportOf({ timer: "absent" }));
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW)).toMatchObject({
      value: null,
      note: "the pass timer is absent (no timestamp-query)",
    });
  });

  it("notes nothing without a maximum or a sample after the warm-up", () => {
    const clocks = gpuClocksOf(clockedMemory(30, 1100, 1100));
    const tMs = memorySeries(clockedMemory(30, 1100, 1100)).tMs;
    expect(clockNote(clocks, tMs, 10)).toBe(NOTE_1100);
    expect(clockNote(clocks, tMs, 60)).toBeNull();
    const noMaximum =
      clocks.value === null
        ? clocks
        : measured({ ...clocks.value, maxGraphicsMHz: missing("N/A") });
    expect(clockNote(noMaximum, tMs, 10)).toBeNull();
  });

  it("writes -1 for a sample whose clock is missing, with its gap", () => {
    const unavailable = reclocked(clockedMemory(5, 1100, 1100), (i) =>
      i === 1
        ? {
            ...nvidiaClocksAt(1100, { maxMHz: 1980 }),
            graphicsMHz: missing("nvidia-smi gives no clocks/graphics_clock (N/A)"),
          }
        : i === 3
          ? { kind: "unavailable", reason: "nvidia-smi failed: timed out" }
          : null,
    );
    expect(gpuClocksOf(unavailable).value?.graphicsMHz).toEqual(
      measured({
        samples: [1100, -1, 1100, -1, 1100],
        gaps: [
          { from: 1, to: 1, reason: "nvidia-smi gives no clocks/graphics_clock (N/A)" },
          { from: 3, to: 3, reason: "nvidia-smi failed: timed out" },
        ],
      }),
    );
    expect(gpuClocksOf(unavailable).value?.memoryMHz).toEqual(
      measured({
        samples: [9501, 9501, 9501, -1, 9501],
        gaps: [{ from: 3, to: 3, reason: "nvidia-smi failed: timed out" }],
      }),
    );
  });

  it.each<readonly [string, () => MemorySample[], string]>([
    [
      "no source for the run's GPU",
      () =>
        reclocked(clockedMemory(3, 1100, 1100), () => ({
          kind: "unavailable",
          reason: "no unprivileged GPU clock reading on darwin; powermetrics needs root",
        })),
      "no unprivileged GPU clock reading on darwin; powermetrics needs root",
    ],
    ["no sample", () => [], "no memory sample"],
    [
      "no graphics clock at any sample",
      () =>
        reclocked(clockedMemory(3, 1100, 1100), () => ({
          ...nvidiaClocksAt(1100),
          graphicsMHz: missing("no nvidia-smi on this machine"),
        })),
      "no nvidia-smi on this machine",
    ],
  ])("are null with the reason for %s", (_case, memory, reason) => {
    expect(gpuClocksOf(memory())).toEqual(missing(reason));
  });

  it("keeps a reading its source does not give null with the reason", () => {
    const memory = reclocked(clockedMemory(30, 600, 600), () => ({
      kind: "clocks",
      source: "i915-sysfs",
      maxGraphicsMHz: measured(1150),
      graphicsMHz: measured(600),
      memoryMHz: missing("i915 gives no memory clock: the GPU shares the system's memory"),
      performanceState: missing("i915 has no performance states"),
    }));
    const results = clockedResults(memory);
    expect(results.gpu.clocks.value).toMatchObject({
      source: "i915-sysfs",
      memoryMHz: missing("i915 gives no memory clock: the GPU shares the system's memory"),
      performanceState: missing("i915 has no performance states"),
    });
    expect(rowOf(results, TERRAIN_ATMOSPHERE_ROW).note).toBe(
      "measured at a median 600 of 1150 MHz (the driver's choice at this load)",
    );
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });

  it("states the clocks after the warm-up in the summary", () => {
    expect(summaryMarkdown(clockedResults(clockedMemory(30, 1980, 1100)))).toContain(
      "- **GPU clocks:** from nvidia-smi; graphics median 1100 MHz (56 %), p5 1100, p95 1100, after the warm-up, of 1980 MHz; memory median 9501 MHz; performance state P2",
    );
    expect(summaryMarkdown(clockedResults([]))).toContain("- **GPU clocks:** — (no memory sample)");
  });

  it("lists every sample for a smoke's log", () => {
    expect(describeClockSamples(gpuClocksOf(clockedMemory(3, 1980, 1100)))).toBe(
      "from nvidia-smi, maximum 1980 MHz; graphics MHz 1980 1980 1980; memory MHz 9501 9501 9501; performance states 2 2 2",
    );
    expect(describeClockSamples(missing("no memory sample"))).toBe("— (no memory sample)");
  });
});

describe("the schema check", () => {
  it("refuses a figure that is null without a reason", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    const broken: unknown = JSON.parse(
      JSON.stringify({ ...results, uploads: { bytes: { value: null, reason: "" } } }),
    );
    expect(validateResults(broken)).toEqual(["uploads.bytes is null without a reason"]);
  });

  it("refuses a figure read from the trace when there is no trace", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: missing("the trace was not recorded"),
      memory: MEMORY,
    });
    const broken: unknown = JSON.parse(
      JSON.stringify({
        ...results,
        frames: { ...results.frames, dropped: measured(0) },
        mainThread: { ...results.mainThread, gc: measured([]) },
      }),
    );
    expect(validateResults(broken)).toEqual([
      "frames.dropped is measured without a trace (the trace was not recorded)",
      "mainThread.gc is measured without a trace (the trace was not recorded)",
    ]);
  });

  it("refuses a client run's presentation times when there is no trace", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: missing("the trace was not recorded"),
      memory: MEMORY,
    });
    const [first, ...rest] = results.frames.segments;
    if (first === undefined) {
      throw new Error("the report has no segment");
    }
    const broken: unknown = JSON.parse(
      JSON.stringify({
        ...results,
        frames: {
          ...results.frames,
          presentation: results.frames.raf,
          segments: [{ ...first, presentation: first.raf }, ...rest],
        },
      }),
    );
    expect(validateResults(broken)).toEqual([
      "frames.presentation is measured without a trace (the trace was not recorded)",
      "frames.segments[0].presentation is measured without a trace (the trace was not recorded)",
    ]);
  });

  it("accepts a native replay's own presentation times without a trace", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: missing("a native replay has no trace"),
      memory: MEMORY,
    });
    const replay: DescentResults = {
      ...results,
      run: { ...results.run, launchMode: "native-replay" },
      frames: { ...results.frames, source: "presentation", presentation: results.frames.raf },
    };
    expect(validateResults(JSON.parse(JSON.stringify(replay)))).toEqual([]);
  });

  it("refuses another schema", () => {
    expect(validateResults({ schema: "x", version: 1 })).toContain(
      "schema is not hyperion.descent-spike.results",
    );
  });

  it("holds no clock column to a memory series without times, whose own problem it names", () => {
    const file: unknown = JSON.parse(JSON.stringify(clockedResults(clockedMemory(3, 1980, 1100))));
    editAt(file, ["memory", "series", "tMs"], DELETE);
    expect(validateResults(file)).toEqual(["memory.series.tMs is not a list of whole ms"]);
  });

  it("refuses version 1's memory samples", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    const { series: _series, ...peaks } = results.memory;
    const v1: unknown = JSON.parse(
      JSON.stringify({ ...results, version: 1, memory: { samples: MEMORY, ...peaks } }),
    );
    expect(validateResults(v1)).toEqual([
      `version is not ${RESULTS_VERSION}`,
      "memory.series is missing",
    ]);
  });

  it.each<readonly [string, Readonly<Record<string, unknown>>, string]>([
    [
      "times that are not whole ms",
      { tMs: [238.5, 1238, 2238, 3238, 4238, 5238] },
      "memory.series.tMs is not a list of whole ms",
    ],
    [
      "a column shorter than the times",
      { rendererPrivateKiB: renderer([-1, -1, 302 * KIB_PER_MIB, 303 * KIB_PER_MIB, -1]) },
      "memory.series.rendererPrivateKiB has 5 samples, not tMs's 6",
    ],
    [
      "-1 outside a gap",
      { rendererPrivateKiB: renderer([-1, -1, -1, 303 * KIB_PER_MIB, -1, 305 * KIB_PER_MIB]) },
      "memory.series.rendererPrivateKiB.value.samples[2] is -1 outside a gap",
    ],
    [
      "a reading within a gap",
      {
        rendererPrivateKiB: renderer([
          -1,
          301 * KIB_PER_MIB,
          302 * KIB_PER_MIB,
          303 * KIB_PER_MIB,
          -1,
          305 * KIB_PER_MIB,
        ]),
      },
      "memory.series.rendererPrivateKiB.value.samples[1] is within a gap but not -1",
    ],
    [
      "a reading that is not a whole number of KiB",
      {
        rendererPrivateKiB: renderer([
          -1,
          -1,
          302 * KIB_PER_MIB + 0.5,
          303 * KIB_PER_MIB,
          -1,
          305 * KIB_PER_MIB,
        ]),
      },
      "memory.series.rendererPrivateKiB.value.samples[2] is not a whole number of KiB",
    ],
    [
      "a gap past the last sample",
      {
        rendererPrivateKiB: renderer(
          [-1, -1, 302 * KIB_PER_MIB, 303 * KIB_PER_MIB, -1, 305 * KIB_PER_MIB],
          [{ from: 4, to: 6, reason: "the renderer reported no memory" }],
        ),
      },
      "memory.series.rendererPrivateKiB.value.gaps[0] is not a stretch of samples with a reason, after the gap before it",
    ],
    [
      "gaps out of order",
      {
        rendererPrivateKiB: renderer(
          [-1, -1, 302 * KIB_PER_MIB, 303 * KIB_PER_MIB, -1, 305 * KIB_PER_MIB],
          RENDERER_GAPS.toReversed(),
        ),
      },
      "memory.series.rendererPrivateKiB.value.gaps[1] is not a stretch of samples with a reason, after the gap before it",
    ],
    [
      "a gap without a reason",
      {
        rendererPrivateKiB: renderer(
          [-1, -1, 302 * KIB_PER_MIB, 303 * KIB_PER_MIB, 304 * KIB_PER_MIB, 305 * KIB_PER_MIB],
          [{ from: 0, to: 1, reason: "" }],
        ),
      },
      "memory.series.rendererPrivateKiB.value.gaps[0] is not a stretch of samples with a reason, after the gap before it",
    ],
    [
      "consecutive gaps of one reason left apart",
      {
        rendererPrivateKiB: renderer(
          [-1, -1, 302 * KIB_PER_MIB, 303 * KIB_PER_MIB, -1, 305 * KIB_PER_MIB],
          [
            { from: 0, to: 0, reason: "the renderer reported no memory" },
            { from: 1, to: 1, reason: "the renderer reported no memory" },
            { from: 4, to: 4, reason: "the renderer reported no memory" },
          ],
        ),
      },
      "memory.series.rendererPrivateKiB.value.gaps[1] continues the gap before it for the same reason",
    ],
    [
      "a column with no reading that is not null",
      {
        drmTotalKiB: measured({
          samples: [-1, -1, -1, -1, -1, -1],
          gaps: [{ from: 0, to: 5, reason: "no GPU process" }],
        }),
      },
      "memory.series.drmTotalKiB has no reading: a reading missing all run is null with its reason",
    ],
    [
      "a column null without a reason",
      { gpuProcessKiB: { value: null, reason: "" } },
      "memory.series.gpuProcessKiB is null without a reason",
    ],
    [
      "a column that is neither null nor samples",
      { gpuProcessKiB: measured([1, 2, 3, 4, 5, 6]) },
      "memory.series.gpuProcessKiB is neither null nor samples with their gaps",
    ],
  ])("refuses %s in the memory series", (_case, overrides, problem) => {
    expect(validateResults(withSeries(overrides))).toEqual([problem]);
  });

  it.each<readonly [string, string, unknown, string]>([
    [
      "a peak other than its column's maximum × 1024",
      "peakAppBytes",
      measured(800 * MIB + 1),
      `memory.peakAppBytes is not appKiB's maximum × 1024 (${800 * MIB} B)`,
    ],
    [
      "a peak of a column with no reading",
      "peakDrmResidentBytes",
      measured(0),
      "memory.peakDrmResidentBytes is not null, but drmResidentKiB has no reading",
    ],
    [
      "nvidia-smi's peak less its baseline above the device's maximum",
      "peakNvidiaDeviceLessBaselineBytes",
      measured(701 * MIB),
      "memory.peakNvidiaDeviceLessBaselineBytes is above nvidiaDeviceKiB's maximum",
    ],
  ])("refuses %s", (_case, key, peak, problem) => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    const broken: unknown = JSON.parse(
      JSON.stringify({ ...results, memory: { ...results.memory, [key]: peak } }),
    );
    expect(validateResults(broken)).toEqual([problem]);
  });

  it.each<readonly [string, ReadonlyArray<Edit>, string]>([
    [
      "no count of incomplete frames",
      [[["gpu", "incompleteFrames"], DELETE]],
      "gpu.incompleteFrames is missing",
    ],
    [
      "a count of incomplete frames that is not whole",
      [[["gpu", "incompleteFrames", "value", "partial"], 1.5]],
      "gpu.incompleteFrames.value is not whole counts of dropped, partial and counted frames",
    ],
    [
      "more incomplete frames than it counted",
      [[["gpu", "incompleteFrames", "value", "dropped"], 372]],
      "gpu.incompleteFrames.value counts 401 incomplete frames of 400",
    ],
    [
      "counted frames other than those after the warm-up",
      [[["gpu", "incompleteFrames", "value", "frames"], 500]],
      "gpu.incompleteFrames.value.frames is 500, not the 400 frames after the warm-up that the rAF figures read",
    ],
    [
      "a pass its incomplete frames make impossible",
      [[["criteria", "whole", 7, "verdict"], "pass"]],
      "the terrain-atmosphere row's pass is impossible with 30 of 400 frames' pass times incomplete",
    ],
    [
      "a marginal its incomplete frames make impossible",
      [[["criteria", "whole", 7, "verdict"], "marginal"]],
      "the terrain-atmosphere row's marginal is impossible with 30 of 400 frames' pass times incomplete",
    ],
    [
      "a fail its incomplete frames make impossible",
      [
        [["gpu", "incompleteFrames", "value", "partial"], 390],
        [["criteria", "whole", 7, "verdict"], "fail"],
      ],
      "the terrain-atmosphere row's fail is impossible with 391 of 400 frames' pass times incomplete",
    ],
    ["no clocks", [[["gpu", "clocks"], DELETE]], "gpu.clocks is missing"],
    [
      "a clock column shorter than the memory's times",
      [
        [
          ["gpu", "clocks"],
          measured(clocksOf({ memoryMHz: measured({ samples: [9501, 9501], gaps: [] }) })),
        ],
      ],
      "gpu.clocks.value.memoryMHz has 2 samples, not tMs's 3",
    ],
    [
      "a clock that is not whole MHz",
      [
        [
          ["gpu", "clocks"],
          measured(
            clocksOf({ graphicsMHz: measured({ samples: [1980, 1980.5, 1100], gaps: [] }) }),
          ),
        ],
      ],
      "gpu.clocks.value.graphicsMHz.value.samples[1] is not a whole number of MHz",
    ],
    [
      "an unknown clock source",
      [[["gpu", "clocks"], measured({ ...clocksOf(), source: "powermetrics" })]],
      "gpu.clocks.value.source is not nvidia-smi, i915-sysfs or amdgpu-sysfs",
    ],
    [
      "a maximum clock that is not whole MHz",
      [[["gpu", "clocks"], measured(clocksOf({ maxGraphicsMHz: measured(0) }))]],
      "gpu.clocks.value.maxGraphicsMHz is not whole MHz",
    ],
    ["no pass rows' figures", [[["gpu", "rows"], DELETE]], "gpu.rows is missing"],
    [
      "a pass row without its estimate",
      [[["gpu", "rows", "terrain", "estimateMs"], DELETE]],
      "gpu.rows.terrain is not an estimate and the percentiles of its sums",
    ],
    [
      "a pass row's percentiles out of order",
      [[["gpu", "rows", "terrain", "p50Ms", "value"], 7]],
      "gpu.rows.terrain's percentiles are out of order",
    ],
    [
      "a pass row over its estimate that its 95th percentile is not over",
      [[["gpu", "rows", "atmosphere", "overEstimate"], true]],
      "gpu.rows.atmosphere.overEstimate is not false, whether its 95th percentile is above its estimate",
    ],
    [
      "a joint limit other than the estimates' sum",
      [[["criteria", "whole", 7, "limit"], 19]],
      "the terrain-atmosphere row's limit is 19, not the estimates' sum, 18 ms",
    ],
    [
      "estimates whose sum is not the joint limit",
      [[["gpu", "rows", "atmosphere", "estimateMs"], 1]],
      "the terrain-atmosphere row's limit is 18, not the estimates' sum, 15 ms",
    ],
    [
      "version 5's terrain row",
      [[["criteria", "whole", 7, "id"], "terrain"]],
      "criteria.whole holds version 5's terrain row: terrain and atmosphere are one row, terrain-atmosphere",
    ],
    [
      "no joint row",
      [[["criteria", "whole", 7, "id"], "atmosphere"]],
      "criteria.whole has 0 terrain-atmosphere rows, not one",
    ],
    [
      "two joint rows",
      [[["criteria", "whole", 8, "id"], "terrain-atmosphere"]],
      "criteria.whole has 2 terrain-atmosphere rows, not one",
    ],
    [
      "version 5's bare load-average triple",
      [
        [
          ["run", "machine", "loadAverage"],
          [0.4, 0.6, 0.8],
        ],
      ],
      "run.machine.loadAverage is a bare triple, version 5's form: version 6 records it as a value with its reason",
    ],
    [
      "no load average without its reason",
      [[["run", "machine", "loadAverage"], { value: null }]],
      "run.machine.loadAverage is not three averages with no reason, or none with its reason",
    ],
    [
      "a load average without its reason's null",
      [[["run", "machine", "loadAverage"], { value: [0.4, 0.6, 0.8] }]],
      "run.machine.loadAverage is not three averages with no reason, or none with its reason",
    ],
    [
      "no load average",
      [[["run", "machine", "loadAverage"], DELETE]],
      "run.machine has no load average",
    ],
    [
      "a load average that is not three averages",
      [[["run", "machine", "loadAverage"], measured([0.4, 0.6])]],
      "run.machine.loadAverage.value is not the three load averages",
    ],
    [
      "a load average measured on Windows",
      [[["run", "platform"], "win32"]],
      "run.machine.loadAverage is measured on win32, which keeps no load average",
    ],
    [
      "no load average on a run that is not provisional",
      [[["run", "machine", "loadAverage"], missing("no load average on haiku")]],
      "run.quiet is not provisional, but run.machine.loadAverage is none",
    ],
  ])("refuses %s", (_case, edits, problem) => {
    const file: unknown = JSON.parse(JSON.stringify(incompleteResults(TWENTY_NINE, [400])));
    for (const [path, value] of edits) {
      editAt(file, path, value);
    }
    expect(validateResults(file)).toContain(problem);
  });

  it("accepts a native replay's count of its own frames, which it has no rAF figure for", () => {
    const results = incompleteResults(TWENTY_NINE, [400]);
    const replay: DescentResults = {
      ...results,
      run: { ...results.run, launchMode: "native-replay" },
      frames: { ...results.frames, raf: missing("a native replay has no requestAnimationFrame") },
      gpu: { ...results.gpu, incompleteFrames: measured({ dropped: 30, partial: 0, frames: 600 }) },
    };
    expect(validateResults(JSON.parse(JSON.stringify(replay)))).toEqual([]);
  });

  it("accepts the GPU's clocks with every column as long as the memory's times", () => {
    const results = incompleteResults(TWENTY_NINE, [400]);
    const withClocks: DescentResults = {
      ...results,
      gpu: {
        ...results.gpu,
        clocks: measured(clocksOf({ memoryMHz: missing("i915 gives no memory clock") })),
      },
    };
    expect(validateResults(JSON.parse(JSON.stringify(withClocks)))).toEqual([]);
  });

  it("refuses a file that is not an object", () => {
    expect(validateResults([])).toEqual(["the file is not an object"]);
  });

  it("refuses version 5", () => {
    const results = twoWindowResults();
    expect(RESULTS_VERSION).toBe(6);
    expect(validateResults(JSON.parse(JSON.stringify({ ...results, version: 5 })))).toEqual([
      "version is not 6",
    ]);
  });

  it("refuses a boundary count other than the windows less one", () => {
    const results = twoWindowResults();
    const trace = results.run.trace.value;
    if (trace === null) {
      throw new Error("the run has no trace");
    }
    const broken: unknown = JSON.parse(
      JSON.stringify({
        ...results,
        run: { ...results.run, trace: measured({ ...trace, boundaries: [] }) },
      }),
    );
    expect(validateResults(broken)).toEqual([
      "run.trace.value has 0 boundaries for 2 windows, not one between each pair",
      "frames.excludedFrames is 30, not the 0 the boundaries left out",
    ]);
  });

  it.each<readonly [string, ReadonlyArray<Edit>, ReadonlyArray<string>]>([
    [
      "a window that filled its buffer but is not marked failed",
      [[["run", "trace", "value", "windows", 0, "figures", "value", "bufferPercent"], 99]],
      ["run.trace.value.windows[0] filled its buffer, but is not marked failed"],
    ],
    [
      "a window whose span is short of its length but is not marked failed",
      [[["run", "trace", "value", "windows", 1, "figures", "value", "spanMs"], 1000]],
      ["run.trace.value.windows[1] filled its buffer, but is not marked failed"],
    ],
    [
      "a window's figures that are not a span, a size and a buffer's use",
      [[["run", "trace", "value", "windows", 0, "figures", "value", "bytes"], 1.5]],
      ["run.trace.value.windows[0].figures is not a span, a size and a buffer's use"],
    ],
    [
      "a window out of its place",
      [[["run", "trace", "value", "windows", 1, "index"], 5]],
      ["run.trace.value.windows[1] is not a window from its start to its stop"],
    ],
    [
      "windows that overlap",
      [[["run", "trace", "value", "windows", 1, "fromS"], 14]],
      // Starting earlier, the window is also longer than its trace's span.
      [
        "run.trace.value.windows[1] begins before the window before it stops",
        "run.trace.value.windows[1] filled its buffer, but is not marked failed",
        "run.trace.value.boundaries[0] is not the gap between windows 0 and 1, excluded to the guard after it",
      ],
    ],
    [
      "no windows",
      [
        [["run", "trace", "value", "windows"], []],
        [["run", "trace", "value", "boundaries"], []],
        [["frames", "excludedFrames"], 0],
        [["frames", "segments", 0, "excludedFrames"], 0],
      ],
      ["run.trace.value.windows is not a list of windows"],
    ],
    [
      "a boundary that is not one",
      [[["run", "trace", "value", "boundaries", 0], 5]],
      [
        "run.trace.value.boundaries[0] is not a boundary",
        "frames.excludedFrames is 30, not the 0 the boundaries left out",
      ],
    ],
    [
      "a boundary away from its windows' gap",
      [[["run", "trace", "value", "boundaries", 0, "resumedS"], 15.6]],
      [
        "run.trace.value.boundaries[0] is not the gap between windows 0 and 1, excluded to the guard after it",
      ],
    ],
    [
      "a boundary without its count",
      [[["run", "trace", "value", "boundaries", 0, "excludedFrames"], -1]],
      [
        "run.trace.value.boundaries[0] does not count its frames and their largest interval",
        "frames.excludedFrames is 30, not the 0 the boundaries left out",
      ],
    ],
    [
      "trace settings that are not",
      [[["run", "trace", "value", "profiled"], "yes"]],
      ["run.trace.value does not say how the trace was recorded"],
    ],
    [
      "a trace without its format",
      [[["run", "trace", "value", "format"], DELETE]],
      ["run.trace.value.format is neither json nor perfetto-proto"],
    ],
    [
      "a trace in another format",
      [[["run", "trace", "value", "format"], "protobuf"]],
      ["run.trace.value.format is neither json nor perfetto-proto"],
    ],
    [
      "a WebGPU slice without gpu among the categories",
      [
        [
          ["gpu", "gpuProcess", "value", "slices", 1],
          { name: "WebGPU", count: 0, totalMs: 0, maxMs: 0 },
        ],
      ],
      ["gpu.gpuProcess.value.slices holds WebGPU, but its category gpu was not recorded"],
    ],
    [
      "a GPU-process slice the reducer does not summarise",
      [
        [
          ["gpu", "gpuProcess", "value", "slices", 1],
          { name: "CommandBuffer", count: 0, totalMs: 0, maxMs: 0 },
        ],
      ],
      ["gpu.gpuProcess.value.slices holds CommandBuffer, which the reducer does not summarise"],
    ],
    [
      "no GPUTask slice, whose category was recorded",
      [[["gpu", "gpuProcess", "value", "slices"], []]],
      ["gpu.gpuProcess.value.slices lacks GPUTask, whose category was recorded"],
    ],
    [
      "excluded frames that are not a count",
      [[["frames", "excludedFrames"], 1.5]],
      ["frames.excludedFrames is not a count"],
    ],
    [
      "a segment's excluded frames that are not a count",
      [[["frames", "segments", 1, "excludedFrames"], null]],
      ["frames.segments[1].excludedFrames is not a count"],
    ],
    [
      "segments that leave out more frames than the whole run",
      [[["frames", "segments", 1, "excludedFrames"], 5]],
      ["the segments' excluded frames are more than the whole run's"],
    ],
    [
      "the engine's figures left in the split",
      [[["mainThread", "split", "value", "engineSelfMs"], 1]],
      ["mainThread.split holds the engine's figures, which belong in mainThread.engine"],
    ],
    ["no engine figure", [[["mainThread", "engine"], DELETE]], ["mainThread.engine is missing"]],
    [
      "an engine figure that is not a sampled self time",
      [
        [["mainThread", "engine"], { value: { selfMs: -1, sampledMs: 2 }, reason: null }],
        [["run", "trace", "value", "profiled"], true],
      ],
      ["mainThread.engine is not a sampled self time"],
    ],
  ])("refuses a windowed run's file with %s", (_case, edits, problems) => {
    const file: unknown = JSON.parse(JSON.stringify(twoWindowResults()));
    for (const [path, value] of edits) {
      editAt(file, path, value);
    }
    expect(validateResults(file)).toEqual(problems);
  });

  it("refuses exclusions out of order or overlapping", () => {
    // Three windows: the second stops 0.5 s after it starts, inside the first boundary's guard.
    const traceWindows = [
      { startedMs: at(-0.1), stopRequestedMs: at(15), failure: null },
      { startedMs: at(15.5), stopRequestedMs: at(16), failure: null },
      { startedMs: at(16.2), stopRequestedMs: at(30.1), failure: null },
    ];
    const overlapping = buildResults({
      run: runOf(),
      report: reportOf({ traceWindows }),
      trace: recordingOf(
        traceWindows.map(({ startedMs, stopRequestedMs }) =>
          windowFile(
            windowTrace({
              offsetUs: 7e9,
              fromMs: startedMs,
              toMs: stopRequestedMs,
              frames: reportOf(),
            }),
          ),
        ),
      ),
      memory: MEMORY,
    });
    // Overlapping exclusions count the frames they share twice, which the sum check sees too.
    expect(validateResults(JSON.parse(JSON.stringify(overlapping)))).toEqual([
      "run.trace.value.boundaries[1]'s exclusion begins before the one before it ends",
      "frames.excludedFrames is 44, not the 54 the boundaries left out",
    ]);
    const results = twoWindowResults();
    const trace = results.run.trace.value;
    const [boundary] = trace?.boundaries ?? [];
    if (trace === null || boundary === undefined) {
      throw new Error("the run has no boundary");
    }
    const late = { ...boundary, stopRequestedS: 17, resumedS: 17.5, excludedToS: 18.5 };
    const unordered: unknown = JSON.parse(
      JSON.stringify({
        ...results,
        run: {
          ...results.run,
          trace: measured({
            ...trace,
            windows: [...trace.windows, { ...trace.windows[1], index: 2 }],
            boundaries: [late, { ...boundary, afterWindow: 1 }],
          }),
        },
      }),
    );
    expect(validateResults(unordered)).toContain(
      "run.trace.value.boundaries[1]'s exclusion begins before the one before it ends",
    );
  });

  it("stops the writer before it writes a file that does not match", async () => {
    const files = memoryFiles();
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    const broken = { ...results, run: { ...results.run, setting: "medium" } };
    // @ts-expect-error -- a setting the schema does not allow, as a corrupt input would carry.
    await expect(writeResults("/runs", broken, files)).rejects.toThrow(
      "run.setting is neither high nor low",
    );
    expect(files.written.size).toBe(0);
  });
});

describe("the committed results files", () => {
  const dir = join(__dirname, "../../../../docs/measurements/descent-spike");

  it("each validate as the current version and stay within the added-file limit", async () => {
    const names = (await readdir(dir)).filter((name) => name.endsWith(".json")).toSorted();
    const files = await Promise.all(
      names.map(async (name) => {
        const text = await readFile(join(dir, name), "utf8");
        const parsed: unknown = JSON.parse(text);
        return { name, text, parsed };
      }),
    );
    // The descent-demand records beside them have a schema of their own.
    const results = files.filter(
      ({ parsed }) =>
        typeof parsed === "object" &&
        parsed !== null &&
        Reflect.get(parsed, "schema") === RESULTS_SCHEMA,
    );
    expect(results.length).toBeGreaterThan(0);
    expect(
      Object.fromEntries(results.map(({ name, parsed }) => [name, validateResults(parsed)])),
    ).toEqual(Object.fromEntries(results.map(({ name }) => [name, []])));
    expect(
      results
        .filter(({ text }) => Buffer.byteLength(text, "utf8") > ADDED_FILE_LIMIT_BYTES)
        .map(({ name }) => name),
    ).toEqual([]);
  });
});

/** A run on Windows, whose `os.loadavg()` gives zeros, kept as the description's triple. */
function windowsResults(): DescentResults {
  return buildResults({
    run: runOf({ platform: "win32", machine: { ...MACHINE, loadAverage: [0, 0, 0] } }),
    report: reportOf(),
    trace: traceOf(),
    memory: MEMORY,
  });
}

/** A run on macOS at a load average of 0.5. */
function macResults(): DescentResults {
  return buildResults({
    run: runOf({ platform: "darwin", machine: { ...MACHINE, loadAverage: [0.5, 0.4, 0.3] } }),
    report: reportOf(),
    trace: traceOf(),
    memory: MEMORY,
  });
}

/** Memory readers with a browser, a GPU process and the tracing service. */
function sources(overrides: Partial<MemorySources> = {}): MemorySources {
  return {
    appMetrics: () => [
      { pid: 10, type: "Browser", memory: { workingSetSize: 100_000, peakWorkingSetSize: 0 } },
      { pid: 11, type: "GPU", memory: { workingSetSize: 200_000, peakWorkingSetSize: 0 } },
      {
        pid: 12,
        type: "Utility",
        serviceName: "tracing.mojom.TracingService",
        memory: { workingSetSize: 50_000, peakWorkingSetSize: 0 },
      },
    ],
    rendererPrivateBytes: () => Promise.resolve(123),
    drm: (pid) =>
      Promise.resolve(
        pid === 11
          ? { kind: "drm", totalBytes: 9, residentBytes: 7, clients: 2, drivers: ["i915"] }
          : { kind: "unavailable", reason: "wrong process" },
      ),
    nvidia: () => Promise.resolve({ kind: "unavailable", reason: "no nvidia-smi on this machine" }),
    clocks: () =>
      Promise.resolve({
        kind: "clocks",
        source: "i915-sysfs",
        maxGraphicsMHz: measured(1150),
        graphicsMHz: measured(600),
        memoryMHz: missing("i915 gives no memory clock: the GPU shares the system's memory"),
        performanceState: missing("i915 has no performance states"),
      }),
    nowMs: () => 5000,
    ...overrides,
  };
}

describe("the memory sampler", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("samples at once and then each second until stopped", async () => {
    vi.useFakeTimers();
    let now = 1000;
    const sampler = new MemorySampler(sources({ nowMs: () => now }), (error) => {
      throw error;
    });
    sampler.start();
    await vi.advanceTimersByTimeAsync(0);
    now = 2000;
    await vi.advanceTimersByTimeAsync(1000);
    now = 3000;
    await vi.advanceTimersByTimeAsync(1000);
    const samples = await sampler.stop();
    expect(samples.map(({ tS }) => tS)).toEqual([0, 1, 2]);
    await vi.advanceTimersByTimeAsync(5000);
    await expect(sampler.stop()).resolves.toHaveLength(3);
  });

  it("reports a failed sample and keeps sampling", async () => {
    vi.useFakeTimers();
    const errors: unknown[] = [];
    let calls = 0;
    const sampler = new MemorySampler(
      sources({
        rendererPrivateBytes: () => {
          calls += 1;
          return calls === 1 ? Promise.reject(new Error("renderer gone")) : Promise.resolve(1);
        },
      }),
      (error) => {
        errors.push(error);
      },
    );
    sampler.start();
    await vi.advanceTimersByTimeAsync(1000);
    await expect(sampler.stop()).resolves.toHaveLength(1);
    expect(errors).toEqual([new Error("renderer gone")]);
  });
});

describe("a memory sample", () => {
  it("keeps the tracing service apart and reads the GPU process's fdinfo", async () => {
    await expect(sampleMemory(sources(), 3000)).resolves.toEqual({
      tS: 2,
      appBytes: 300_000 * 1024,
      gpuProcessBytes: 200_000 * 1024,
      tracingBytes: 50_000 * 1024,
      rendererPrivateBytes: 123,
      drmResidentBytes: 7,
      drmReason: null,
      drmTotalBytes: 9,
      nvidiaDeviceBytes: null,
      nvidiaGpuProcessBytes: null,
      clocks: {
        kind: "clocks",
        source: "i915-sysfs",
        maxGraphicsMHz: measured(1150),
        graphicsMHz: measured(600),
        memoryMHz: missing("i915 gives no memory clock: the GPU shares the system's memory"),
        performanceState: missing("i915 has no performance states"),
      },
    });
  });

  it("reads the clocks from the sample's own nvidia-smi reading", async () => {
    const readings: unknown[] = [];
    const unavailable = { kind: "unavailable", reason: "no nvidia-smi on this machine" } as const;
    await sampleMemory(
      sources({
        nvidia: () => Promise.resolve(unavailable),
        clocks: (nvidia) => {
          readings.push(nvidia);
          return Promise.resolve({ kind: "unavailable", reason: "none" });
        },
      }),
      0,
    );
    expect(readings).toEqual([unavailable]);
  });

  it("gives a sample whose clock reader rejects no clocks, with the reason", async () => {
    const reading = await sampleMemory(
      sources({ clocks: () => Promise.reject(new Error("sysfs gone")) }),
      0,
    );
    expect(reading.clocks).toEqual({
      kind: "unavailable",
      reason: "the GPU's clocks could not be read: sysfs gone",
    });
  });

  it("reads nvidia-smi's device total and the GPU process's own share", async () => {
    const reading = await sampleMemory(
      sources({
        nvidia: () =>
          Promise.resolve({
            kind: "nvidia",
            driverVersion: "615.71.09",
            gpus: [
              {
                busId: "00000000:08:00.0",
                productName: "NVIDIA GeForce RTX 3080",
                totalBytes: 10_240 * MIB,
                reservedBytes: 320 * MIB,
                usedBytes: 900 * MIB,
                processes: [{ pid: 11, type: "G", name: "electron", usedBytes: 600 * MIB }],
                clocks: {
                  graphicsMHz: 1980,
                  memoryMHz: 9501,
                  maxGraphicsMHz: 2115,
                  performanceState: 0,
                },
              },
            ],
          }),
      }),
      0,
    );
    expect(reading).toMatchObject({
      nvidiaDeviceBytes: 900 * MIB,
      nvidiaGpuProcessBytes: 600 * MIB,
    });
  });
});

/** Readers of a machine on `platform` with no `/proc` or `/sys`, and the paths it was asked to read. */
function withoutProcOrSys(
  platform: NodeJS.Platform,
  loadavg: ReadonlyArray<number>,
): { readonly read: ReadonlyArray<string>; readonly readers: MachineSources } {
  const read: string[] = [];
  return {
    read,
    readers: {
      platform,
      hostname: () => "mac-mini",
      cpus: () => [{ model: "Apple M2" }],
      totalmem: () => 16e9,
      loadavg: () => loadavg,
      readFile: (path) => {
        read.push(path);
        return Promise.reject(new Error(`ENOENT: no such file or directory, open '${path}'`));
      },
      gpuInfo: () => Promise.resolve({}),
    },
  };
}

describe("the machine's description", () => {
  it("records the CPU, governor, load and Chromium's active GPU", async () => {
    const machine = await describeMachine({
      platform: "linux",
      hostname: () => "Dev.Box_1",
      cpus: () => [{ model: "AMD Ryzen 7 3700X 8-Core Processor " }, { model: "same" }],
      totalmem: () => 32e9,
      loadavg: () => [0.5, 0.4, 0.3],
      readFile: () => Promise.resolve("schedutil\n"),
      gpuInfo: () =>
        Promise.resolve({
          gpuDevice: [
            { vendorId: 0x80_86, deviceId: 0x3e_a0, active: false },
            { vendorId: 0x10_de, deviceId: 0x22_06, active: true, driverVersion: "615.71.09" },
          ],
        }),
    });
    expect(machine).toEqual({
      name: "devbox1",
      cpu: "AMD Ryzen 7 3700X 8-Core Processor",
      logicalCores: 2,
      memoryBytes: 32e9,
      governor: measured("schedutil"),
      loadAverage: [0.5, 0.4, 0.3],
      gpu: measured({
        vendorId: 0x10_de,
        deviceId: 0x22_06,
        driverVersion: "615.71.09",
        description: null,
      }),
    });
  });

  it("states why the governor and the GPU are missing", async () => {
    const machine = await describeMachine({
      platform: "linux",
      hostname: () => "x",
      cpus: () => [],
      totalmem: () => 1,
      loadavg: () => [0.1, 0.1, 0.1],
      readFile: () => Promise.reject(new Error("ENOENT")),
      gpuInfo: () => Promise.resolve({}),
    });
    expect(machine.governor.reason).toContain("scaling_governor could not be read");
    expect(machine.gpu).toEqual(missing("Chromium reports no GPU device"));
  });

  it("reads no governor on macOS, and no file", async () => {
    const { read, readers } = withoutProcOrSys("darwin", [0.5, 0.4, 0.3]);
    const machine = await describeMachine(readers);
    expect({ governor: machine.governor, read }).toEqual({
      governor: missing("no cpufreq governor on macOS"),
      read: [],
    });
  });

  it("records the load average on macOS", async () => {
    const machine = await describeMachine(withoutProcOrSys("darwin", [0.5, 0.4, 0.3]).readers);
    expect(machine.loadAverage).toEqual([0.5, 0.4, 0.3]);
  });

  it("reads no governor on Windows, and no file", async () => {
    const { read, readers } = withoutProcOrSys("win32", [0, 0, 0]);
    const machine = await describeMachine(readers);
    expect({ governor: machine.governor, read }).toEqual({
      governor: missing("no cpufreq governor on Windows"),
      read: [],
    });
  });

  it("keeps Windows' zeros as the load average it lacks", async () => {
    const machine = await describeMachine(withoutProcOrSys("win32", [0, 0, 0]).readers);
    expect(machine.loadAverage).toEqual([0, 0, 0]);
  });
});
