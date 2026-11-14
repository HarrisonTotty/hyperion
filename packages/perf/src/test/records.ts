/**
 * Records for the package's tests: a complete, valid perf-run record of five repetitions, and ways
 * to vary it.
 */

import type { FrameFigures, PerfRepetition, PerfRunRecord } from "../record.ts";

/** The compared metrics of one repetition, as {@link sampleRecord} sets them. */
export interface RepetitionMetrics {
  readonly gpuBusyPerFrameMs: number | null;
  readonly mainThreadPerFrameMs: number | null;
  readonly missedFraction: number | null;
}

function frames(missedFraction: number | null): FrameFigures {
  return {
    source: "presentation",
    count: 36_000,
    meanMs: 33.4,
    p50Ms: 33.3,
    p95Ms: 33.4,
    p99Ms: 50.1,
    maxMs: 120.4,
    missedFraction,
    hitches: 3,
  };
}

/** One repetition with the given metrics. */
export function sampleRepetition(index: number, metrics: RepetitionMetrics): PerfRepetition {
  return {
    index,
    startedAt: `2026-11-13T10:${String(10 + index * 2)}:00.000Z`,
    frames: frames(metrics.missedFraction),
    segments: [{ segment: "orbit coast", frames: frames(metrics.missedFraction) }],
    gpuBusyPerFrameMs: metrics.gpuBusyPerFrameMs,
    mainThreadPerFrameMs: metrics.mainThreadPerFrameMs,
    rows: [{ row: "terrain", frames: 36_000, meanMs: 9.1, p50Ms: 9.0, p95Ms: 11.2, p99Ms: 12.5 }],
    passes: [
      {
        key: "primary:terrain",
        row: "terrain",
        frames: 36_000,
        p50Ms: 9.0,
        p95Ms: 11.2,
        p99Ms: 12.5,
      },
    ],
    memory: {
      itemPeakBytes: { "height-cache": 64_000_000, "star-cubemap": 33_554_432 },
      tallyPeakBytes: 200_000_000,
      gpu: {
        source: "drm-fdinfo",
        residentPeakBytes: 250_000_000,
        totalPeakBytes: 300_000_000,
        residentLowerBoundBytes: 244_000_000,
      },
      appPeakBytes: 900_000_000,
      workerFieldBytes: [15_100_000, 15_100_000],
      workerWasmBytes: [40_000_000, 40_000_000],
      nvidiaDeviceLessBaselinePeakBytes: null,
      displayServer: { residentPeakBytes: 30_000_000, renderBusyMs: 1200 },
    },
    streaming: [{ segment: "orbit coast", patchesPerS: 4, demandPerS: 4.2, streamingSeconds: 0 }],
    throttle: {
      packageThrottleMs: 0,
      coreThrottleMs: 0,
      throttleFraction: 0,
      rp0Share: 0.9,
      gpuMinMHz: 900,
      gpuMedianMHz: 1100,
      maxPackageDegC: 78,
    },
    latePipelines: 0,
    heightPointUs: null,
  };
}

/** Five repetitions' metrics, each metric given as its five values. */
export function metricsOf(
  gpu: ReadonlyArray<number>,
  main: ReadonlyArray<number>,
  missed: ReadonlyArray<number>,
): ReadonlyArray<RepetitionMetrics> {
  return gpu.map((gpuBusyPerFrameMs, i) => ({
    gpuBusyPerFrameMs,
    mainThreadPerFrameMs: main[i] ?? null,
    missedFraction: missed[i] ?? null,
  }));
}

const STEADY: ReadonlyArray<RepetitionMetrics> = metricsOf(
  [20, 20.2, 19.9, 20.1, 20],
  [8, 8.1, 7.9, 8, 8.05],
  [0.01, 0.012, 0.011, 0.01, 0.009],
);

/**
 * A valid perf-run record of `cockpit-descent` at the low setting on the UHD 620 laptop, its
 * repetitions' metrics given, and any top-level field replaced.
 */
export function sampleRecord(
  metrics: ReadonlyArray<RepetitionMetrics> = STEADY,
  replace: Partial<PerfRunRecord> = {},
): PerfRunRecord {
  const runs = metrics.map((m, i) => sampleRepetition(i + 1, m));
  const base: PerfRunRecord = {
    version: 1,
    id: "2026-11-13-uhd620-cockpit-descent-1",
    date: "2026-11-13T10:00:00.000Z",
    commit: "0123456789abcdef0123456789abcdef01234567",
    dirty: false,
    generatorVersion: 24,
    source: "perf-run",
    scene: { id: "cockpit-descent", version: 1, seed: "7", world: "pin:earthlike" },
    setting: { quality: "low", values: { "quality.clouds": "layer", "quality.scatterDensity": 0 } },
    views: [
      {
        role: "primary",
        style: "photorealistic",
        widthPx: 1280,
        heightPx: 720,
        internalScale: { min: 0.75, median: 1 },
        rateHz: 30,
        lineAntialiasing: "none",
        exposure: { mode: "histogram", evMin: 9.5, evMax: 15.2 },
      },
      {
        role: "instrument",
        style: "wireframe",
        widthPx: 480,
        heightPx: 270,
        internalScale: { min: 1, median: 1 },
        rateHz: 30,
        lineAntialiasing: "analytic",
        exposure: null,
      },
    ],
    machine: {
      name: "uhd620",
      platform: "linux",
      kernel: "6.18.0",
      cpu: { model: "Intel(R) Core(TM) i7-8665U CPU @ 1.90GHz", logicalCores: 8 },
      memoryBytes: 16_000_000_000,
      gpu: { vendorId: 0x8086, deviceId: 0x3ea0, driverVersion: "Mesa 26.2.3", description: null },
      adapter: { vendor: "intel", architecture: "gen-9", description: "", fallback: false },
      capabilities: {
        timestampQuery: true,
        subgroups: true,
        shaderF16: true,
        rg11b10Renderable: true,
      },
      kernelPaths: { "atmosphere-lut": "subgroups" },
      versions: { app: "0.1.0", electron: "45.1.0", chromium: "156.0.0.0", node: "24.21.0" },
      display: { widthPx: 1920, heightPx: 1080, refreshHz: 60, devicePixelRatio: 1 },
      windowManager: "i3",
      compositor: "none",
      windowSystem: "x11",
      card: null,
    },
    state: {
      supply: "mains",
      profile: {
        tlpMode: "performance/AC",
        governor: "powersave",
        epp: "balance_performance",
        noTurbo: false,
        pl1Watts: 15,
        pl2Watts: 51,
        tauSeconds: 28,
        raplZone: "intel-rapl-mmio:0",
        lowPowerMode: null,
      },
      loadAverage1m: 0.4,
      startPackageDegC: 52,
      temperatureSource: "x86_pkg_temp",
    },
    harness: {
      gpuTiming: true,
      timer: "full",
      bracketedPasses: [],
      switches: ["--use-angle=vulkan", "--disable-dawn-features=timestamp_quantization"],
      launchMode: "vulkan",
      repetitions: runs.length,
      repetitionSeconds: 1200,
      warmUp: { seconds: 150, steady: true },
      forced: false,
      forcedReasons: [],
      pacing: "every-second-vsync",
      vsyncPeriodMs: 16.68,
      server: { placement: "local", numWorkers: 2, restartedPerRepetition: true },
      heightWorkers: 2,
      display: "physical",
    },
    runs,
    pooledFrames: { count: 180_000, p99Ms: 50.1 },
    arrival: null,
    notes: "",
  };
  return { ...base, ...replace };
}
