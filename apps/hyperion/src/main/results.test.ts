import { afterEach, describe, expect, it, vi } from "vitest";

import type { DescentSpikeReport } from "../preload/api";
import { type TraceFigures, TraceReducer } from "./reduceTrace";
import {
  buildResults,
  type DescentResults,
  describeMachine,
  EMPTY_TRACE_REASON,
  frameStats,
  type MachineDescription,
  measured,
  type MemorySample,
  MemorySampler,
  type MemorySources,
  missing,
  nearestRank,
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
