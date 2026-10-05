import { join } from "node:path";

import { format, resolveConfig } from "prettier";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DescentSpikeReport } from "../preload/api";
import { type TraceFigures, TraceReducer } from "./reduceTrace";
import {
  ADDED_FILE_LIMIT_BYTES,
  buildResults,
  type DescentResults,
  describeMachine,
  EMPTY_TRACE_REASON,
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
  RESULTS_VERSION,
  type ResultsFiles,
  type RunDescription,
  sampleMemory,
  SEGMENT_MEASURE_PREFIX,
  summaryMarkdown,
  TIMESTAMP_QUANTUM_MS,
  validateResults,
  writeResults,
} from "./results";

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

/** Twenty frames a second of script time over two segments, 10 s each, after a 10 s warm-up. */
function reportOf(overrides: Partial<DescentSpikeReport> = {}): DescentSpikeReport {
  const scriptTimesS = Array.from({ length: 600 }, (_, i) => i / 20);
  const rafIntervalsMs = scriptTimesS.map((_, i) => (i === 0 ? 0 : i % 100 === 0 ? 120 : 33.3));
  return {
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

/** A trace whose presentations come every 33.3 ms over the run, with segment marks. */
function traceOf(): TraceFigures {
  const startUs = 1_000_000;
  const presentedAtUs = Array.from(
    { length: 900 },
    (_, i) => startUs + Math.round((i * 100_000) / 3),
  );
  return {
    span: { firstUs: startUs, lastUs: startUs + 30_000_000 },
    frames: {
      pid: 1,
      layerTreeHostId: 1,
      presentedAtUs,
      intervalsMs: [],
      presented: presentedAtUs.length,
      dropped: 2,
      noUpdate: 0,
    },
    mainThread: null,
    gpuProcess: null,
    threads: [
      {
        pid: 1,
        tid: 2,
        process: "Renderer",
        thread: "CrRendererMain",
        busyMs: 900,
        gc: { count: 3, totalMs: 6, maxMs: 3 },
      },
    ],
    userTiming: [
      {
        name: `${SEGMENT_MEASURE_PREFIX}orbit coast`,
        pid: 1,
        tid: 2,
        startsUs: [startUs],
        durationsMs: [20_000],
        count: 1,
        totalMs: 20_000,
        maxMs: 20_000,
      },
      {
        name: `${SEGMENT_MEASURE_PREFIX}descent arc`,
        pid: 1,
        tid: 2,
        startsUs: [startUs + 20_000_000],
        durationsMs: [10_000],
        count: 1,
        totalMs: 10_000,
        maxMs: 10_000,
      },
    ],
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
      memory: MEMORY,
    });
    expect(results.run.periodMs).toEqual(measured(2000 / 60));
    expect(results.frames.source).toBe("presentation");
    // 900 presentations over 30 s; those ending in the first 10 s are the warm-up.
    expect(results.frames.presentation.value?.count).toBe(600);
    expect(
      results.frames.segments.map(({ segment, presentation }) => [
        segment,
        presentation.value?.count,
      ]),
    ).toEqual([
      ["orbit coast", 300],
      ["descent arc", 300],
    ]);
    expect(results.frames.dropped).toEqual(measured(2));
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
      memory: MEMORY,
    });
    expect(rowOf(results, "terrain")).toMatchObject({ limit: 14, verdict: "marginal" });
    const exact = buildResults({
      run: runOf(),
      report: { ...atLimit, timer: "full" },
      trace: measured(traceOf()),
      memory: MEMORY,
    });
    expect(rowOf(exact, "terrain")).toMatchObject({ verdict: "fail" });
  });

  it("reads the GPU rows against the setting's limits", () => {
    const results = buildResults({
      run: runOf({ setting: "high" }),
      report: reportOf(),
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
      memory: [sample(0, null)],
    });
    expect(results.memory.gpuHeadline).toEqual(
      measured({ bytes: 300 * MIB, source: "adapter-tally" }),
    );
    expect(rowOf(results, "memory").note).toBe("read from the adapter's tally: no driver reading");
  });

  it("marks a run on a busy machine provisional, and a short trace truncated", () => {
    const busy = { ...MACHINE, loadAverage: [14.2, 12, 10] as const };
    const trace = traceOf();
    const results = buildResults({
      run: runOf({ machine: busy }),
      report: reportOf(),
      trace: measured({ ...trace, span: { firstUs: 0, lastUs: 20_000_000 } }),
      memory: MEMORY,
    });
    expect(results.run.quiet.provisional).toBe(true);
    expect(results.run.trace).toEqual(measured({ spanMs: 20_000, truncated: true }));
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
      trace: measured(new TraceReducer().figures()),
      memory: MEMORY,
    });
    const empty = missing(EMPTY_TRACE_REASON);
    expect(results.run.trace).toEqual(empty);
    expect(results.frames.presentation).toEqual(empty);
    expect(results.frames.segments.map(({ presentation }) => presentation)).toEqual([empty, empty]);
    expect(results.frames.dropped).toEqual(empty);
    expect(results.gpu.gpuProcess).toEqual(empty);
    expect(results.mainThread.split).toEqual(empty);
    expect(results.mainThread.gc).toEqual(empty);
    expect(results.frames.source).toBe("raf");
    expect(validateResults(JSON.parse(JSON.stringify(results)))).toEqual([]);
  });
});

describe("the memory series", () => {
  it("holds the times in whole ms and each reading in whole KiB, with its peak in bytes", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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
    trace: measured(traceOf()),
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

describe("the schema check", () => {
  it("refuses a figure that is null without a reason", () => {
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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
      trace: measured(traceOf()),
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

  it("stops the writer before it writes a file that does not match", async () => {
    const files = memoryFiles();
    const results = buildResults({
      run: runOf(),
      report: reportOf(),
      trace: measured(traceOf()),
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
