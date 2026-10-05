import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

import { format, resolveConfig } from "prettier";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DescentSpikeReport } from "../preload/api";
import { PROFILED, recordingOf, UNPROFILED, windowFile, windowTrace } from "./fixtures/traces";
import type { Measured } from "./measured";
import { type TraceFigures, TraceReducer } from "./reduceTrace";
import {
  ADDED_FILE_LIMIT_BYTES,
  buildResults,
  type DescentResults,
  describeMachine,
  formatAsPrettier,
  frameStats,
  type MachineDescription,
  measured,
  type MemorySample,
  MemorySampler,
  memorySeries,
  type MemorySources,
  missing,
  nearestRank,
  RESULTS_SCHEMA,
  RESULTS_VERSION,
  type ResultsFiles,
  type RunDescription,
  sampleMemory,
  summaryMarkdown,
  TIMESTAMP_QUANTUM_MS,
  validateResults,
  writeResults,
} from "./results";
import {
  EMPTY_TRACE_REASON,
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
    expect(rowOf(results, "terrain").tolerance).toBeCloseTo(TIMESTAMP_QUANTUM_MS, 12);
  });

  it("calls a row marginal within the quantized timer's tolerance of its limit", () => {
    const frames = reportOf().frames;
    const atLimit = reportOf({
      timer: "quantized",
      frames: {
        ...frames,
        passes: [{ label: "terrain", row: "terrain", gpuMs: frames.scriptTimesS.map(() => 14.03) }],
      },
    });
    const results = buildResults({
      run: runOf(),
      report: atLimit,
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(rowOf(results, "terrain")).toMatchObject({ limit: 14, verdict: "marginal" });
    const exact = buildResults({
      run: runOf(),
      report: { ...atLimit, timer: "full" },
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(rowOf(exact, "terrain")).toMatchObject({ verdict: "fail" });
  });

  it("reads the GPU rows against the setting's limits", () => {
    const results = buildResults({
      run: runOf({ setting: "high" }),
      report: reportOf(),
      trace: traceOf(),
      memory: MEMORY,
    });
    expect(rowOf(results, "terrain")).toMatchObject({ limit: 5, value: 6, verdict: "fail" });
    expect(rowOf(results, "atmosphere")).toMatchObject({ limit: 1, value: 1.5, verdict: "fail" });
    expect(rowOf(results, "headroom-gpu")).toMatchObject({ value: 8, verdict: "pass" });
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
    expect(rowOf(results, "terrain").value).toBe(6);
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
      }),
      "its file could not be read: ENOENT",
    ],
    [
      "a file that is not a trace",
      () => ({
        trace: missing("the trace could not be reduced: spike-trace-1.json is not a trace"),
        bytes: 12,
        bufferPercent: null,
      }),
      "the trace could not be reduced: spike-trace-1.json is not a trace",
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
    [
      "frame spans that disagree with the renderer's frames",
      (second) =>
        windowFile({
          ...second,
          mainThread:
            second.mainThread === null
              ? null
              : { ...second.mainThread, frameSpans: { startsUs: [], durationsMs: [] } },
        }),
      // The second window checks the frames from 16.0 s to 29.6 s, 0.5 s inside [15.5, 30.1] s.
      "the trace's frame spans disagree with the renderer's (273 of 273 frames)",
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
    expect(twoWindowResults().run.trace.value?.format).toBe("json");
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
      "- **Trace:** 2 json windows, 18.5 s traced after the warm-up, unprofiled; 1 boundary left out 30 frames (largest stall 500.00 ms); largest file 1 MiB, buffer use up to 10 %",
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

  it("refuses a file that is not an object", () => {
    expect(validateResults([])).toEqual(["the file is not an object"]);
  });

  it("refuses version 3", () => {
    const results = twoWindowResults();
    expect(RESULTS_VERSION).toBe(4);
    expect(validateResults(JSON.parse(JSON.stringify({ ...results, version: 3 })))).toEqual([
      "version is not 4",
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

describe("the machine's description", () => {
  it("records the CPU, governor, load and Chromium's active GPU", async () => {
    const machine = await describeMachine({
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
      hostname: () => "x",
      cpus: () => [],
      totalmem: () => 1,
      loadavg: () => [],
      readFile: () => Promise.reject(new Error("ENOENT")),
      gpuInfo: () => Promise.resolve({}),
    });
    expect(machine.governor.reason).toContain("scaling_governor could not be read");
    expect(machine.gpu).toEqual(missing("Chromium reports no GPU device"));
  });
});
