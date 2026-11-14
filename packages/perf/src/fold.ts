/**
 * The fold of one of R05's descent-spike results files into a run record (plan R12, Design note 1;
 * R12.T1.b).
 *
 * @remarks
 * R05's files (`docs/measurements/descent-spike/<date>-<machine>-<setting>.json`, schema
 * `hyperion.descent-spike.results`, versions 6 to 9) stay where they are; a fold is one repetition of
 * R05's own descent over R05's own world, so it takes scene version 0, which no catalogue scene
 * has, and never compares with a perf run. What R05 does not record is `null`: the commit, the
 * generator version, the kernel, the display, the power profile beyond the governor, the GPU's busy
 * time (R05 times the GPU process's CPU side, not the engine's), throttling. A hidden run's frames
 * are paced by Chromium's timer, so its display is `virtual` (Design note 11), which the tables
 * leave out.
 */

import {
  perfRecordId,
  PERF_RECORD_VERSION,
  PerfRecordError,
  validatePerfRecord,
} from "./record.ts";
import type {
  FrameFigures,
  JsonObject,
  JsonValue,
  PassFigures,
  PerfRunRecord,
  PerfViewRecord,
  RowFigures,
  SegmentFigures,
  StreamingFigures,
} from "./record.ts";
import type { BudgetRow, ViewRole } from "./rows.ts";
import { median } from "./stats.ts";

/** R05's results files' schema name. */
export const SPIKE_RESULTS_SCHEMA = "hyperion.descent-spike.results";

/** The versions of R05's results files the fold reads (R05's `RESULTS_VERSIONS_READ`). */
export const SPIKE_RESULTS_VERSIONS: ReadonlyArray<number> = [6, 7, 8, 9];

/** R05's wireframe view's one pass label (`WIREFRAME_PASS_LABEL`). */
const WIREFRAME_PASS = "view:wireframe";

/** R05's pass rows, to the budget's (R05's `SpikePassRow`). */
const SPIKE_ROWS: Readonly<Record<string, BudgetRow>> = {
  terrain: "terrain",
  atmosphere: "atmosphere-frame",
  other: "other",
};

type Obj = Readonly<Record<string, unknown>>;

function isObj(value: unknown): value is Obj {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fail(path: string, reason: string): never {
  throw new PerfRecordError(`spike:${path}`, reason);
}

function obj(value: unknown, path: string): Obj {
  if (!isObj(value)) fail(path, "expected an object");
  return value;
}

function field(from: Obj, key: string, path: string): unknown {
  if (!Object.hasOwn(from, key)) fail(`${path}.${key}`, "missing");
  return from[key];
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string") fail(path, "expected a string");
  return value;
}

function number(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) fail(path, "expected a finite number");
  return value;
}

function numberOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function textOrNull(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

/** R05's `Measured<T>`: its value, or `null` with the reason the file gives. */
function measured(value: unknown): unknown {
  return isObj(value) && Object.hasOwn(value, "value") ? value["value"] : null;
}

function arrayOf(value: unknown): ReadonlyArray<unknown> {
  return Array.isArray(value) ? value : [];
}

function frameFigures(stats: unknown, source: FrameFigures["source"]): FrameFigures {
  const s = isObj(stats) ? stats : {};
  return {
    source,
    count: numberOrNull(s["count"]) ?? 0,
    meanMs: null,
    p50Ms: numberOrNull(s["p50Ms"]),
    p95Ms: numberOrNull(s["p95Ms"]),
    p99Ms: numberOrNull(s["p99Ms"]),
    maxMs: numberOrNull(s["maxMs"]),
    missedFraction: numberOrNull(s["missedFraction"]),
    hitches: numberOrNull(s["hitches"]),
  };
}

/** R05's frame source, and the key of its statistics in `frames`. */
function frameSource(frames: Obj): { source: FrameFigures["source"]; key: string } {
  const source = text(field(frames, "source", "frames"), "frames.source");
  switch (source) {
    case "presentation":
    case "raf":
      return { source, key: source };
    case "gpu-completion":
      return { source, key: "gpuCompletion" };
    default:
      return fail("frames.source", `unknown source "${source}"`);
  }
}

function settingValues(options: unknown): JsonObject {
  const out: Record<string, JsonValue> = {};
  if (!isObj(options)) return out;
  for (const [key, value] of Object.entries(options)) {
    if (["setting", "seed", "world", "style", "smoke"].includes(key)) continue;
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      out[`spike.${key}`] = value;
    }
  }
  return out;
}

function views(run: Obj, sizes: unknown, rateHz: number): ReadonlyArray<PerfViewRecord> {
  const canvas = obj(field(run, "canvas", "run"), "run.canvas");
  const style = run["style"] === "wireframe" ? "wireframe" : "photorealistic";
  const primary: PerfViewRecord = {
    role: "primary",
    style,
    widthPx: number(canvas["widthPx"], "run.canvas.widthPx"),
    heightPx: number(canvas["heightPx"], "run.canvas.heightPx"),
    internalScale: { min: 1, median: 1 },
    rateHz,
    lineAntialiasing: style === "photorealistic" ? "none" : null,
    exposure: null,
  };
  const second = isObj(sizes) ? sizes["secondView"] : undefined;
  if (!isObj(second)) return [primary];
  const viewport = arrayOf(second["viewports"]).find(isObj);
  if (viewport === undefined) return [primary];
  const secondStyle = second["style"] === "wireframe" ? "wireframe" : "photorealistic";
  return [
    primary,
    {
      role: "instrument",
      style: secondStyle,
      widthPx: number(viewport["widthPx"], "sizes.secondView.viewports[0].widthPx"),
      heightPx: number(viewport["heightPx"], "sizes.secondView.viewports[0].heightPx"),
      internalScale: { min: 1, median: 1 },
      rateHz,
      lineAntialiasing: secondStyle === "photorealistic" ? "none" : null,
      exposure: null,
    },
  ];
}

function passes(gpu: Obj, viewList: ReadonlyArray<PerfViewRecord>): ReadonlyArray<PassFigures> {
  const instrumentWireframe = viewList.some(
    (v) => v.role === "instrument" && v.style === "wireframe",
  );
  const primaryWireframe = viewList.some((v) => v.role === "primary" && v.style === "wireframe");
  return arrayOf(measured(gpu["passes"]))
    .filter(isObj)
    .map((pass, i) => {
      const label = text(pass["label"], `gpu.passes.value[${String(i)}].label`);
      const role: ViewRole =
        label === WIREFRAME_PASS && instrumentWireframe && !primaryWireframe
          ? "instrument"
          : "primary";
      return {
        key: `${role}:${label}`,
        row: SPIKE_ROWS[text(pass["row"], `gpu.passes.value[${String(i)}].row`)] ?? "other",
        frames: numberOrNull(pass["frames"]) ?? 0,
        p50Ms: numberOrNull(pass["p50Ms"]),
        p95Ms: numberOrNull(pass["p95Ms"]),
        p99Ms: numberOrNull(pass["p99Ms"]),
      };
    });
}

function rows(gpu: Obj): ReadonlyArray<RowFigures> {
  const incomplete = measured(gpu["incompleteFrames"]);
  const frames = isObj(incomplete)
    ? (numberOrNull(incomplete["frames"]) ?? 0) -
      (numberOrNull(incomplete["dropped"]) ?? 0) -
      (numberOrNull(incomplete["partial"]) ?? 0)
    : 0;
  const byRow = isObj(gpu["rows"]) ? gpu["rows"] : {};
  const out: RowFigures[] = [];
  for (const [name, figures] of Object.entries(byRow)) {
    const row = SPIKE_ROWS[name];
    if (row === undefined || !isObj(figures)) continue;
    const p50Ms = numberOrNull(measured(figures["p50Ms"]));
    if (p50Ms === null) continue;
    out.push({
      row,
      frames: Math.max(0, frames),
      meanMs: null,
      p50Ms,
      p95Ms: numberOrNull(measured(figures["p95Ms"])),
      p99Ms: numberOrNull(measured(figures["p99Ms"])),
    });
  }
  return out;
}

function clockFigures(gpu: Obj): { minMHz: number | null; medianMHz: number | null } {
  const clocks = measured(gpu["clocks"]);
  const graphics = isObj(clocks) ? measured(clocks["graphicsMHz"]) : null;
  // A sample below zero is a reading the sampler missed, not a clock.
  const samples = isObj(graphics)
    ? arrayOf(graphics["samples"]).filter(
        (s): s is number => typeof s === "number" && Number.isFinite(s) && s >= 0,
      )
    : [];
  return { minMHz: samples.length > 0 ? Math.min(...samples) : null, medianMHz: median(samples) };
}

function loadAverage1m(machine: Obj): number | null {
  const load = measured(machine["loadAverage"]) ?? machine["loadAverage"];
  return Array.isArray(load) ? numberOrNull(load[0]) : null;
}

/**
 * Turns one of R05's descent-spike results files, as parsed JSON, into a record with
 * `source: "descent-spike"`: one repetition, scene `cockpit-descent` at version 0, numbered 1
 * (the store renumbers it if that ID is taken).
 *
 * @throws {@link PerfRecordError} naming the file's field at fault (`spike:<path>`), or the record's
 *   if the fold produced an invalid one.
 */
export function foldSpikeResult(json: unknown): PerfRunRecord {
  const file = obj(json, "");
  if (file["schema"] !== SPIKE_RESULTS_SCHEMA) {
    fail("schema", `expected "${SPIKE_RESULTS_SCHEMA}", got ${String(file["schema"])}`);
  }
  const version = number(file["version"], "version");
  if (!SPIKE_RESULTS_VERSIONS.includes(version)) {
    fail("version", `reads versions ${SPIKE_RESULTS_VERSIONS.join(", ")}, not ${String(version)}`);
  }
  const run = obj(field(file, "run", ""), "run");
  const machine = obj(field(run, "machine", "run"), "run.machine");
  const versions = obj(field(run, "versions", "run"), "run.versions");
  const frames = obj(field(file, "frames", ""), "frames");
  const gpu = obj(field(file, "gpu", ""), "gpu");
  const memory = obj(field(file, "memory", ""), "memory");
  const mainThread = obj(field(file, "mainThread", ""), "mainThread");

  const startedAt = text(field(run, "startedAt", "run"), "run.startedAt");
  const name = text(field(machine, "name", "run.machine"), "run.machine.name");
  const quality = text(field(run, "setting", "run"), "run.setting");
  if (quality !== "high" && quality !== "low") fail("run.setting", `unknown setting "${quality}"`);
  const platform = text(field(run, "platform", "run"), "run.platform");
  if (platform !== "linux" && platform !== "darwin" && platform !== "win32") {
    fail("run.platform", `unknown platform "${platform}"`);
  }
  const rateHz = quality === "low" ? 30 : 60;
  const switches = arrayOf(run["switches"]).filter(
    (s): s is string => typeof s === "string" && s.length > 0,
  );
  const shown = run["shown"] === true;

  const { source, key } = frameSource(frames);
  const whole = frameFigures(measured(frames[key]), source);
  const segments: ReadonlyArray<SegmentFigures> = arrayOf(frames["segments"])
    .filter(isObj)
    .map((segment, i) => ({
      segment: text(segment["segment"], `frames.segments[${String(i)}].segment`),
      frames: frameFigures(measured(segment[key]), source),
    }));
  const streaming: ReadonlyArray<StreamingFigures> = arrayOf(file["streaming"])
    .filter(isObj)
    .map((segment, i) => ({
      segment: text(segment["segment"], `streaming[${String(i)}].segment`),
      patchesPerS: numberOrNull(segment["residentPerS"]),
      demandPerS: numberOrNull(segment["predictedCalibratedPerS"]),
      streamingSeconds: numberOrNull(segment["streamingS"]),
    }));
  const split = measured(mainThread["split"]);
  const busyMs = isObj(split) ? numberOrNull(split["busyMs"]) : null;
  const headline = measured(memory["gpuHeadline"]);
  const headlineSource = isObj(headline) ? headline["source"] : null;
  const headlineBytes = isObj(headline) ? numberOrNull(headline["bytes"]) : null;
  const clocks = clockFigures(gpu);
  const viewList = views(run, file["sizes"], rateHz);
  const pacing = isObj(run["pacing"]) ? run["pacing"] : null;
  const periodMs = numberOrNull(measured(run["periodMs"]));
  const vsyncPeriodMs =
    pacing !== null
      ? numberOrNull(measured(pacing["measuredVsyncMs"]))
      : periodMs === null
        ? null
        : periodMs / (quality === "low" ? 2 : 1);
  const quiet = isObj(run["quiet"]) ? run["quiet"] : {};
  const provisional = quiet["provisional"] === true;
  const trace = measured(run["trace"]);
  const late = isObj(file["pipelines"]) ? arrayOf(file["pipelines"]["late"]) : [];
  const gpuDevice = measured(machine["gpu"]);
  const governor = textOrNull(measured(machine["governor"]));
  const criteria = isObj(file["criteria"]) ? textOrNull(file["criteria"]["overall"]) : null;
  const timer = run["timer"];

  const record: PerfRunRecord = {
    version: PERF_RECORD_VERSION,
    id: perfRecordId(startedAt, name, "cockpit-descent", 1),
    date: startedAt,
    commit: null,
    dirty: false,
    generatorVersion: null,
    source: "descent-spike",
    scene: {
      id: "cockpit-descent",
      version: 0,
      seed: text(field(run, "seed", "run"), "run.seed"),
      world: textOrNull(run["world"]) ?? "terrain-test",
    },
    setting: { quality, values: settingValues(run["options"]) },
    views: viewList,
    machine: {
      name,
      platform,
      kernel: null,
      cpu: {
        model: text(field(machine, "cpu", "run.machine"), "run.machine.cpu"),
        logicalCores: number(
          field(machine, "logicalCores", "run.machine"),
          "run.machine.logicalCores",
        ),
      },
      memoryBytes: numberOrNull(machine["memoryBytes"]),
      gpu: isObj(gpuDevice)
        ? {
            vendorId: number(gpuDevice["vendorId"], "run.machine.gpu.value.vendorId"),
            deviceId: number(gpuDevice["deviceId"], "run.machine.gpu.value.deviceId"),
            driverVersion: textOrNull(gpuDevice["driverVersion"]),
            description: textOrNull(gpuDevice["description"]),
          }
        : null,
      adapter: null,
      capabilities: null,
      kernelPaths: null,
      versions: {
        app: textOrNull(versions["app"]),
        electron: text(field(versions, "electron", "run.versions"), "run.versions.electron"),
        chromium: text(field(versions, "chromium", "run.versions"), "run.versions.chromium"),
        node: textOrNull(versions["node"]),
      },
      display: null,
      windowManager: null,
      compositor: null,
      windowSystem: null,
      card: null,
    },
    state: {
      supply: null,
      profile: {
        tlpMode: null,
        governor,
        epp: null,
        noTurbo: null,
        pl1Watts: null,
        pl2Watts: null,
        tauSeconds: null,
        raplZone: null,
        lowPowerMode: null,
      },
      loadAverage1m: loadAverage1m(machine),
      startPackageDegC: null,
      temperatureSource: null,
    },
    harness: {
      gpuTiming: switches.some(
        (s) => s.startsWith("--disable-dawn-features=") && s.includes("timestamp_quantization"),
      ),
      timer: timer === "full" || timer === "quantized" ? timer : "absent",
      bracketedPasses: [],
      switches,
      launchMode: textOrNull(run["launchMode"]),
      repetitions: 1,
      repetitionSeconds: isObj(trace) ? numberOrNull(trace["tracedS"]) : null,
      warmUp: { seconds: numberOrNull(run["warmupS"]) ?? 0, steady: null },
      forced: provisional,
      forcedReasons: provisional ? [textOrNull(quiet["note"]) ?? "R05's run was provisional"] : [],
      pacing: quality === "low" ? "every-second-vsync" : "every-vsync",
      vsyncPeriodMs,
      server: { placement: "local", numWorkers: null, restartedPerRepetition: false },
      heightWorkers: null,
      display: shown ? "physical" : "virtual",
    },
    runs: [
      {
        index: 1,
        startedAt,
        frames: whole,
        segments,
        gpuBusyPerFrameMs: null,
        mainThreadPerFrameMs: busyMs === null || whole.count === 0 ? null : busyMs / whole.count,
        rows: rows(gpu),
        passes: passes(gpu, viewList),
        memory: {
          itemPeakBytes: {},
          tallyPeakBytes: numberOrNull(memory["adapterPeakBytes"]),
          gpu:
            headlineBytes === null ||
            (headlineSource !== "nvidia-smi" &&
              headlineSource !== "drm-fdinfo" &&
              headlineSource !== "adapter-tally")
              ? null
              : {
                  source: headlineSource,
                  residentPeakBytes: headlineBytes,
                  totalPeakBytes: null,
                  residentLowerBoundBytes: null,
                },
          appPeakBytes: numberOrNull(measured(memory["peakAppBytes"])),
          workerFieldBytes: [],
          workerWasmBytes: [],
          nvidiaDeviceLessBaselinePeakBytes: numberOrNull(
            measured(memory["peakNvidiaDeviceLessBaselineBytes"]),
          ),
          displayServer: null,
        },
        streaming,
        throttle: {
          packageThrottleMs: null,
          coreThrottleMs: null,
          throttleFraction: null,
          rp0Share: null,
          gpuMinMHz: clocks.minMHz,
          gpuMedianMHz: clocks.medianMHz,
          maxPackageDegC: null,
        },
        latePipelines: late.length,
        heightPointUs: null,
      },
    ],
    pooledFrames: { count: whole.count, p99Ms: whole.p99Ms },
    arrival: null,
    notes:
      `Folded from R05's descent-spike results, version ${String(version)}` +
      (criteria === null ? "" : `; R05's verdict: ${criteria}`) +
      (shown ? "" : "; a hidden window, paced by Chromium's timer") +
      ".",
  };
  return validatePerfRecord(record);
}
