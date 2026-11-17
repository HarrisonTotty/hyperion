import {
  type MachineRecord,
  PERF_SCENE_HARNESS,
  type PerfRunRecord,
  type PowerThermalRecord,
  parsePerfRecord,
  parsePerfRecords,
  type ThrottleFigures,
} from "@hyperion/perf";
import { describe, expect, it } from "vitest";

import type { PerfLaunch, PerfRepetitionResult, PerfRunDescription } from "../../preload/api";
import type { FrameTraceResult } from "./frameTrace";
import type { MemorySpan } from "./memory";
import type { PreconditionReport } from "./preconditions";
import {
  DESCRIBE_SILENCE_MS,
  PerfRunner,
  PerfRunRefused,
  type PerfRunnerDeps,
  type RunMemory,
  type RunSampler,
  type RunServer,
  type RunTrace,
} from "./runner";
import type { PerfSample } from "./samplers";
import type { Wait } from "./server";
import { WARM_UP_MAX_S, WARM_UP_MIN_S } from "./steadiness";

const LAUNCH: PerfLaunch = {
  scene: "cockpit-descent",
  setting: "low",
  out: null,
  repetitions: 5,
  control: false,
  force: false,
};

const MACHINE: MachineRecord = {
  name: "uhd620",
  platform: "linux",
  kernel: "6.18.0",
  cpu: { model: "Intel(R) Core(TM) i7-8665U CPU @ 1.90GHz", logicalCores: 8 },
  memoryBytes: 16_000_000_000,
  gpu: { vendorId: 0x8086, deviceId: 0x3ea0, driverVersion: "Mesa 26.2.3", description: null },
  adapter: { vendor: "intel", architecture: "gen-9", description: "", fallback: false },
  capabilities: { timestampQuery: true },
  kernelPaths: null,
  versions: { app: "0.1.0", electron: "45.1.0", chromium: "156.0.0.0", node: "24.21.0" },
  display: { widthPx: 1920, heightPx: 1080, refreshHz: 60, devicePixelRatio: 1 },
  windowManager: "i3",
  compositor: "none",
  windowSystem: "x11",
  card: null,
};

const STATE: PowerThermalRecord = {
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
};

const DESCRIPTION: PerfRunDescription = {
  scene: { version: 1, seed: "00000000000004d2", world: "4200acb20000000d.0004" },
  settingValues: { "quality.tauPx": 2 },
  views: [
    {
      role: "primary",
      style: "photorealistic",
      widthPx: 1280,
      heightPx: 720,
      rateHz: 30,
      lineAntialiasing: "none",
      exposure: null,
    },
    {
      role: "instrument",
      style: "wireframe",
      widthPx: 240,
      heightPx: 180,
      rateHz: 30,
      lineAntialiasing: "analytic",
      exposure: null,
    },
  ],
  pacing: "every-second-vsync",
  repetitionSeconds: 1230,
  heightWorkers: 2,
  adapter: { vendor: "intel", architecture: "gen-9", description: "", fallback: false },
  capabilities: { timestampQuery: true },
  kernelPaths: null,
};

const NO_THROTTLE: ThrottleFigures = {
  packageThrottleMs: null,
  coreThrottleMs: null,
  throttleFraction: null,
  rp0Share: null,
  gpuMinMHz: null,
  gpuMedianMHz: null,
  gpuThrottleShare: null,
  cpuMedianMHz: null,
  maxPackageDegC: null,
  zoneMaxDegC: {},
  packageWatts: null,
};

/** A repetition's result whose frames take `frameMs` each, from page time `fromMs`. */
function result(frameMs: number, fromMs = 0, frames = 40): PerfRepetitionResult {
  return {
    spanS: 1230,
    timeOriginMs: 1_800_000_000_000,
    frames: {
      intervalsMs: Array.from({ length: frames }, () => frameMs),
      callbackStartsMs: Array.from({ length: frames }, (_, i) => fromMs + i * frameMs),
      callbackMs: Array.from({ length: frames }, () => 4),
      scriptTimesS: Array.from({ length: frames }, (_, i) => (i * 1230) / frames),
    },
    segments: [
      { name: "orbit coast", startS: 0, endS: 60 },
      { name: "rest", startS: 60, endS: 1230 },
    ],
    passes: [{ key: "primary:terrain", row: "terrain", frames, p50Ms: 5, p95Ms: 6, p99Ms: 7 }],
    rows: [{ row: "terrain", frames, meanMs: 5, p50Ms: 5, p95Ms: 6, p99Ms: 7 }],
    timedPassesPerFrameMs: 12,
    timer: "full",
    memory: {
      itemPeakBytes: { "height-cache": 64 * 1024 ** 2 },
      tallyPeakBytes: 300_000_000,
      workerFieldBytes: [15_100_000, 15_100_000],
      workerWasmBytes: [40_000_000, 40_000_000],
    },
    releases: [],
    streaming: [{ segment: "orbit coast", patchesPerS: 10, demandPerS: 12, streamingSeconds: 1 }],
    latePipelines: 0,
    heightPointUs: null,
    internalScale: [
      { min: 0.75, median: 1 },
      { min: 1, median: 1 },
    ],
    arrival: null,
  };
}

/** A clock the test moves, ms since the epoch. */
class Clock {
  ms = 1_800_000_000_000;
  readonly now = (): number => this.ms;
  advance(seconds: number): void {
    this.ms += seconds * 1000;
  }
}

/** The 10 Hz sampler's stand-in: package temperatures stamped on the clock. */
class FakeSampler implements RunSampler {
  readonly temperatures: Array<{ readonly atMs: number; readonly degC: number }> = [];
  started = false;
  stopped = false;
  readonly spans: Array<readonly [number, number]> = [];

  start(): void {
    this.started = true;
  }

  stop(): void {
    this.stopped = true;
  }

  get samples(): ReadonlyArray<PerfSample> {
    return [];
  }

  /** The span's figures: its hottest package alone, of the samples within it. */
  summarise(fromMs: number, toMs: number): ThrottleFigures {
    this.spans.push([fromMs, toMs]);
    const within = this.temperatures.filter(({ atMs }) => atMs >= fromMs && atMs <= toMs);
    return {
      ...NO_THROTTLE,
      maxPackageDegC: within.length === 0 ? null : Math.max(...within.map(({ degC }) => degC)),
    };
  }

  discardBefore(): void {
    // The stand-in keeps every sample, so that a test can place one outside a span.
  }
}

/** The memory sampler's stand-in: working sets stamped on the clock. */
class FakeMemory implements RunMemory {
  readonly totals: Array<{ readonly atMs: number; readonly bytes: number }> = [];

  start(): void {
    // Nothing to start.
  }

  stop(): void {
    // Nothing to stop.
  }

  span(fromMs: number, toMs: number): MemorySpan {
    const within = this.totals.filter(({ atMs }) => atMs >= fromMs && atMs <= toMs);
    return {
      drm: {
        residentPeakBytes: 400_000_000,
        totalPeakBytes: 500_000_000,
        residentLowerBoundPeakBytes: 390_000_000,
        busyNs: 40 * 20e6,
        settlingSamples: 0,
      },
      nvidia: null,
      displayServer: null,
      appPeakBytes: within.length === 0 ? null : Math.max(...within.map(({ bytes }) => bytes)),
    };
  }

  discardBefore(): void {
    // The stand-in keeps every sample.
  }
}

/** The local server's stand-in: each start's data directory and a URL a start. */
class FakeServer implements RunServer {
  readonly starts: string[] = [];
  stops = 0;

  start(dataDir: string): Promise<string> {
    this.starts.push(dataDir);
    return Promise.resolve(`ws://127.0.0.1:${String(40_000 + this.starts.length)}/ws`);
  }

  stop(): Promise<null> {
    this.stops += 1;
    return Promise.resolve(null);
  }
}

/** The trace's stand-in: no presentations, so the renderer's intervals are taken. */
class FakeTrace implements RunTrace {
  readonly started: number[] = [];

  start(repetition: number): Promise<void> {
    this.started.push(repetition);
    return Promise.resolve();
  }

  stop(): Promise<FrameTraceResult> {
    return Promise.resolve({ kind: "none", reason: "a hidden window presents nothing" });
  }
}

/** Waits the test fires by hand: the watchdog's. */
class Waits {
  readonly pending: Array<{ readonly ms: number; readonly fire: () => void }> = [];
  readonly wait: Wait = (ms, signal) =>
    new Promise((resolve) => {
      const entry = { ms, fire: () => resolve() };
      this.pending.push(entry);
      signal.addEventListener("abort", () => {
        this.pending.splice(this.pending.indexOf(entry), 1);
      });
    });
}

const QUIET: PreconditionReport = { quiet: true, refusals: [], state: STATE, cooledMs: 0 };

/** A runner over stand-ins, and what it did. */
function harness(
  launch: PerfLaunch = LAUNCH,
  report: PreconditionReport = QUIET,
  overrides: Partial<PerfRunnerDeps> = {},
) {
  const clock = new Clock();
  const sampler = new FakeSampler();
  const memory = new FakeMemory();
  const server = new FakeServer();
  const trace = new FakeTrace();
  const waits = new Waits();
  let out = "";
  const exits: number[] = [];
  const log: string[] = [];
  const deps: PerfRunnerDeps = {
    launch,
    harness: PERF_SCENE_HARNESS[launch.scene],
    graphics: {
      launchMode: "vulkan",
      gpuTiming: !launch.control,
      switches: ["--use-angle=vulkan", "--disable-dawn-features=timestamp_quantization"],
    },
    build: { commit: "a".repeat(40), dirty: false, generatorVersion: 31 },
    hidden: false,
    remoteUrl: null,
    server,
    pinned: true,
    place: () =>
      Promise.resolve({
        universe: "3504954ff4feabf4",
        system: "4200acb20000000d",
        body: "4200acb20000000d.0004",
      }),
    freshDataDir: (n) => Promise.resolve(`/userData/perf-run/server-data-${String(n)}`),
    sampler,
    memory,
    trace,
    checkQuiet: () => Promise.resolve(report),
    describeMachine: () => Promise.resolve(MACHINE),
    vsyncPeriodMs: () => 16.68,
    now: clock.now,
    wait: waits.wait,
    patience: 1,
    readOut: () => Promise.resolve(out),
    writeOut: (text) => {
      out = text;
      return Promise.resolve();
    },
    outPath: "target/perf-run/runs.v1.jsonl",
    log: (line) => log.push(line),
    exit: (code) => exits.push(code),
    ...overrides,
  };
  const runner = new PerfRunner(deps);
  return {
    runner,
    clock,
    sampler,
    memory,
    server,
    trace,
    waits,
    exits,
    log,
    out: () => out,
  };
}

/** Drives the warm-up a second a sample with steady 33 ms frames until its verdict. */
async function warmUp(
  ops: ReturnType<PerfRunner["operations"]>,
  clock: Clock,
  intervalAt: (tS: number) => number = () => 33.4,
  t = 0,
): Promise<string> {
  if (t > WARM_UP_MAX_S + 1) {
    throw new Error("the warm-up never ended");
  }
  const verdict = await ops.warmUpSample({ intervalsMs: [intervalAt(t)] });
  if (verdict.kind !== "warming") {
    return verdict.kind;
  }
  clock.advance(1);
  return warmUp(ops, clock, intervalAt, t + 1);
}

/** Runs repetitions `n` to `count`, the `i`th's frames `frameMs(i)` long. */
async function repetitions(
  ops: ReturnType<PerfRunner["operations"]>,
  clock: Clock,
  count: number,
  frameMs: (i: number) => number = () => 33.4,
  n = 1,
): Promise<void> {
  if (n > count) {
    return;
  }
  await ops.beginRepetition(n);
  clock.advance(1230);
  await ops.endRepetition(n, result(frameMs(n)));
  clock.advance(5);
  await repetitions(ops, clock, count, frameMs, n + 1);
}

/** The one record the run appended. */
function onlyRecord(text: string): PerfRunRecord {
  const records = parsePerfRecords(text);
  expect(records).toHaveLength(1);
  const [record] = records;
  if (record === undefined) {
    throw new Error("no record");
  }
  return record;
}

describe("the perf runner", () => {
  it("produces a record that parsePerfRecord accepts, and exits 0", async () => {
    const { runner, clock, out, exits } = harness();
    expect(await runner.prepare()).toBe(true);
    const ops = runner.operations();
    const config = await ops.config();
    expect(config.serverUrl).toBe("ws://127.0.0.1:40001/ws");
    await ops.describe(DESCRIPTION);
    expect(await warmUp(ops, clock)).toBe("steady");
    await repetitions(ops, clock, 5);
    await ops.finish({ status: "complete", notes: "a test run." });
    const record = onlyRecord(out());
    expect(parsePerfRecord(out().trim())).toEqual(record);
    expect(record.id).toMatch(/^\d{4}-\d{2}-\d{2}-uhd620-cockpit-descent-1$/);
    expect(record.runs).toHaveLength(5);
    expect(record.harness.warmUp).toEqual({ seconds: WARM_UP_MIN_S, steady: true });
    expect(record.harness.server).toEqual({
      placement: "local",
      numWorkers: 2,
      restartedPerRepetition: true,
    });
    expect(record.commit).toBe("a".repeat(40));
    expect(record.generatorVersion).toBe(31);
    expect(record.runs[0]?.frames.source).toBe("raf");
    expect(record.runs[0]?.memory.gpu?.source).toBe("drm-fdinfo");
    expect(record.notes).toBe("a test run.");
    expect(exits).toEqual([0]);
  });

  it("restarts the server once per repetition, each in a fresh data directory", async () => {
    const { runner, clock, server, trace } = harness();
    await runner.prepare();
    const ops = runner.operations();
    await ops.describe(DESCRIPTION);
    await warmUp(ops, clock);
    await repetitions(ops, clock, 5);
    await ops.finish({ status: "complete", notes: "" });
    expect(server.starts).toEqual(
      [0, 1, 2, 3, 4, 5].map((n) => `/userData/perf-run/server-data-${String(n)}`),
    );
    // Once a repetition, and once more as the run ends.
    expect(server.stops).toBe(6);
    expect(trace.started).toEqual([1, 2, 3, 4, 5]);
  });

  it("arrives for the warm-up and again inside each repetition's span", async () => {
    const places: string[] = [];
    const { runner, clock } = harness({ ...LAUNCH, repetitions: 2 }, QUIET, {
      place: (url) => {
        places.push(url);
        return Promise.resolve({ universe: "00000000000000aa", system: "s", body: "b" });
      },
    });
    await runner.prepare();
    const ops = runner.operations();
    expect((await ops.config()).arrival.universe).toBe("00000000000000aa");
    await ops.describe(DESCRIPTION);
    await warmUp(ops, clock);
    await repetitions(ops, clock, 2);
    expect(places).toEqual([
      "ws://127.0.0.1:40001/ws",
      "ws://127.0.0.1:40002/ws",
      "ws://127.0.0.1:40003/ws",
    ]);
  });

  it("leaves the samples outside a repetition's span out of it", async () => {
    const { runner, clock, sampler, memory, out } = harness({ ...LAUNCH, repetitions: 1 });
    await runner.prepare();
    const ops = runner.operations();
    await ops.describe(DESCRIPTION);
    await warmUp(ops, clock);
    // A hot package and a large working set during the warm-up, before the span.
    sampler.temperatures.push({ atMs: clock.ms, degC: 99 });
    memory.totals.push({ atMs: clock.ms, bytes: 9e9 });
    clock.advance(1);
    await ops.beginRepetition(1);
    clock.advance(600);
    sampler.temperatures.push({ atMs: clock.ms, degC: 70 });
    memory.totals.push({ atMs: clock.ms, bytes: 1e9 });
    clock.advance(630);
    await ops.endRepetition(1, result(33.4));
    clock.advance(1);
    sampler.temperatures.push({ atMs: clock.ms, degC: 98 });
    await ops.finish({ status: "complete", notes: "" });
    const [run] = onlyRecord(out()).runs;
    expect(run?.throttle.maxPackageDegC).toBe(70);
    expect(run?.memory.appPeakBytes).toBe(1e9);
  });

  it("ends a warm-up that never settles at ten minutes as NOT STEADY", async () => {
    const { runner, clock, out } = harness({ ...LAUNCH, repetitions: 1 });
    await runner.prepare();
    const ops = runner.operations();
    await ops.describe(DESCRIPTION);
    expect(await warmUp(ops, clock, (t) => 30 + (Math.floor(t / 30) % 2) * 5)).toBe("not-steady");
    await repetitions(ops, clock, 1);
    await ops.finish({ status: "complete", notes: "" });
    expect(onlyRecord(out()).harness.warmUp).toEqual({ seconds: WARM_UP_MAX_S, steady: false });
  });

  it("flags a monotone five in its notes, and not four up and one down", async () => {
    const monotone = harness();
    await monotone.runner.prepare();
    const ops = monotone.runner.operations();
    await ops.describe(DESCRIPTION);
    await warmUp(ops, monotone.clock);
    await repetitions(ops, monotone.clock, 5, (n) => 33 + n * 0.1);
    await ops.finish({ status: "complete", notes: "" });
    expect(onlyRecord(monotone.out()).notes).toContain("TREND");
    expect(onlyRecord(monotone.out()).notes).toContain("rising");

    const mixed = harness();
    await mixed.runner.prepare();
    const other = mixed.runner.operations();
    await other.describe(DESCRIPTION);
    await warmUp(other, mixed.clock);
    const means = [33.1, 33.2, 33.3, 33.5, 33.4];
    await repetitions(other, mixed.clock, 5, (n) => means[n - 1] ?? 33);
    await other.finish({ status: "complete", notes: "" });
    expect(onlyRecord(mixed.out()).notes).not.toContain("TREND");
  });

  it("refuses a machine that is not quiet unless forced, and marks a forced record", async () => {
    const loud: PreconditionReport = {
      quiet: false,
      refusals: ["a one-minute load average of 3.20, above 1.0 (Design note 7)"],
      state: STATE,
      cooledMs: 0,
    };
    const refused = harness(LAUNCH, loud);
    expect(await refused.runner.prepare()).toBe(false);
    expect(refused.server.starts).toEqual([]);
    expect(refused.log.join("\n")).toContain("REFUSED a one-minute load average");

    const forced = harness({ ...LAUNCH, repetitions: 1, force: true }, loud);
    expect(await forced.runner.prepare()).toBe(true);
    const ops = forced.runner.operations();
    await ops.describe(DESCRIPTION);
    await warmUp(ops, forced.clock);
    await repetitions(ops, forced.clock, 1);
    await ops.finish({ status: "complete", notes: "" });
    const record = onlyRecord(forced.out());
    expect(record.harness.forced).toBe(true);
    expect(record.harness.forcedReasons).toEqual(loud.refusals);
  });

  it("ends a run whose renderer stops answering with a record whose notes say so", async () => {
    const { runner, clock, waits, out, exits } = harness();
    await runner.prepare();
    const ops = runner.operations();
    await ops.describe(DESCRIPTION);
    await warmUp(ops, clock);
    await repetitions(ops, clock, 2);
    // The renderer begins no third repetition: the watchdog's wait is the last armed.
    const watchdog = waits.pending.at(-1);
    expect(watchdog?.ms).toBe(DESCRIBE_SILENCE_MS);
    watchdog?.fire();
    await new Promise((resolve) => setTimeout(resolve, 0));
    const record = onlyRecord(out());
    expect(record.runs).toHaveLength(2);
    expect(record.harness.repetitions).toBe(2);
    expect(record.notes).toContain("the renderer stopped answering");
    expect(record.notes).toContain("Ended after 2 of 5 repetitions.");
    expect(exits).toEqual([1]);
  });

  it("ends a fault before any repetition with no record, saying why", async () => {
    const { runner, out, exits, log } = harness();
    await runner.prepare();
    await runner.operations().describe(DESCRIPTION);
    await runner.fault("the GPU process exited (crashed)");
    expect(out()).toBe("");
    expect(exits).toEqual([1]);
    expect(log.join("\n")).toContain("no record: the run ended before its first repetition did");
  });

  it("refuses a call out of turn", async () => {
    const { runner } = harness();
    await runner.prepare();
    const ops = runner.operations();
    await expect(ops.beginRepetition(1)).rejects.toBeInstanceOf(PerfRunRefused);
    await ops.describe(DESCRIPTION);
    await expect(ops.describe(DESCRIPTION)).rejects.toBeInstanceOf(PerfRunRefused);
  });

  it("records a station's co-resident server as such", async () => {
    const station = { ...LAUNCH, scene: "station-wireframe-descent" as const, repetitions: 1 };
    const { runner, clock, out } = harness(station);
    await runner.prepare();
    const ops = runner.operations();
    await ops.describe({ ...DESCRIPTION, pacing: "every-vsync" });
    await warmUp(ops, clock);
    await repetitions(ops, clock, 1);
    await ops.finish({ status: "complete", notes: "" });
    expect(onlyRecord(out()).harness.server).toEqual({
      placement: "co-resident",
      numWorkers: 1,
      restartedPerRepetition: true,
    });
  });

  it("names a remote server, never starting or restarting one", async () => {
    const remote = "ws://192.0.2.7:7878/ws";
    const station = { ...LAUNCH, scene: "station-wireframe-descent" as const, repetitions: 2 };
    const { runner, clock, out } = harness(station, QUIET, { server: null, remoteUrl: remote });
    await runner.prepare();
    const ops = runner.operations();
    expect((await ops.config()).serverUrl).toBe(remote);
    await ops.describe({ ...DESCRIPTION, pacing: "every-vsync" });
    await warmUp(ops, clock);
    expect((await ops.beginRepetition(1)).serverUrl).toBe(remote);
    clock.advance(1230);
    await ops.endRepetition(1, result(16.7));
    await ops.beginRepetition(2);
    await ops.endRepetition(2, result(16.7));
    await ops.finish({ status: "complete", notes: "" });
    expect(onlyRecord(out()).harness.server).toEqual({
      placement: "remote",
      numWorkers: null,
      restartedPerRepetition: false,
    });
  });
});
