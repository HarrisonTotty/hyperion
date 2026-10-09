import { describe, expect, it, vi } from "vitest";

import type {
  DescentSpikeReport,
  SpikeLaunch,
  SpikeResultsAnswer,
  SpikeResultsPaths,
} from "../preload/api";
import { DEFAULT_SPIKE_SEED } from "../preload/spikeLaunch";
import { CdpTracing } from "./cdpTracing";
import { FakeDebugger, memoryWriter } from "./fixtures/debugger";
import { measured, type MemorySample, type RunDescription, validateResults } from "./results";
import type { TraceFigures } from "./reduceTrace";
import { SpikeTrace, type TraceWindowStop } from "./spike";
import { SpikeSession, type SpikeSessionDeps, traceWindowFileName } from "./spikeSession";
import { smallReport } from "./fixtures/spikeReport";
import { callbackStartsOf, PROFILED, UNPROFILED, windowTrace } from "./fixtures/traces";
import { LOST_DATA_REASON, type TraceSettings } from "./traceWindows";

const LAUNCH: SpikeLaunch = {
  setting: "low",
  seed: DEFAULT_SPIKE_SEED,
  smoke: false,
  out: "/out",
  workers: null,
  vertexPath: null,
  normals: null,
  ridged: "off",
  dawnSafety: "on",
  capture: "/capture",
  traceProfile: "off",
};

const RUN: RunDescription = {
  startedAt: new Date("2026-10-03T18:00:00Z"),
  machine: {
    name: "devbox",
    cpu: "AMD Ryzen 7 3700X 8-Core Processor",
    logicalCores: 16,
    memoryBytes: 32 * 1024 ** 3,
    governor: measured("schedutil"),
    loadAverage: [0.4, 0.6, 0.8],
    gpu: measured({ vendorId: 0x10de, deviceId: 0x2206, driverVersion: null, description: null }),
  },
  versions: { app: "0.1.0", electron: "44.4.3", chromium: "152", node: "24", v8: "15" },
  platform: "linux",
  launchMode: "vulkan",
  setting: "low",
  seed: DEFAULT_SPIKE_SEED,
  options: { setting: "low", seed: DEFAULT_SPIKE_SEED },
  switches: [],
  shown: false,
  displayHz: null,
  nvidiaBaselineBytes: null,
};

/** How the fake trace's next cycle ends: whole, or failing in its stop or its start. */
type CycleOutcome = "whole" | "stop fails" | "start fails";

/** What the fake trace's stops tell: an eighth of the buffer used, no data lost. */
const EIGHTH: TraceWindowStop = { lostData: false, bufferPercent: 12.5 };

/** A trace that records its calls; each stop tells {@link EIGHTH} unless a test says otherwise. */
function fakeTrace(settings: TraceSettings = UNPROFILED): SpikeSessionDeps["trace"] & {
  readonly calls: string[];
  failStop: boolean;
  /** Holds the trace busy, as a start, stop or cycle in flight does. */
  busy: boolean;
  nextCycle: CycleOutcome;
  /** What the stops tell. */
  told: TraceWindowStop;
  /** Whether it has been closed, after which it refuses to start, as `CdpTracing` does. */
  closed: boolean;
  /** The files its stops wrote. */
  readonly files: Set<string>;
} {
  const calls: string[] = [];
  const files = new Set<string>();
  let state: SpikeSessionDeps["trace"]["state"] = "idle";
  return {
    calls,
    files,
    failStop: false,
    busy: false,
    nextCycle: "whole",
    told: EIGHTH,
    closed: false,
    settings,
    get state() {
      return this.busy ? "busy" : state;
    },
    start() {
      calls.push("start");
      if (this.closed) {
        return Promise.reject(
          new Error("the spike's trace did not start: the trace's debugger session is closed"),
        );
      }
      state = "recording";
      return Promise.resolve();
    },
    stop(path) {
      calls.push(`stop ${path}`);
      state = "idle";
      if (this.failStop) {
        return Promise.reject(new Error("the spike's trace did not stop: service gone"));
      }
      files.add(path);
      return Promise.resolve(this.told);
    },
    cycle(path, stopped) {
      calls.push(`cycle ${path}`);
      const outcome = this.nextCycle;
      this.nextCycle = "whole";
      if (outcome === "stop fails") {
        state = "idle";
        return Promise.reject(new Error("the spike's trace did not stop: service gone"));
      }
      files.add(path);
      stopped(this.told);
      if (outcome === "start fails") {
        state = "idle";
        return Promise.reject(new Error("the spike's trace did not start: service gone"));
      }
      return Promise.resolve();
    },
    close() {
      calls.push("close");
      this.closed = true;
    },
  };
}

/** A session's deps over `trace`, with what it writes and how it ends, its files in memory. */
function depsOf(
  trace: SpikeSessionDeps["trace"],
  reduce: SpikeSessionDeps["reduce"],
  size: (path: string) => Promise<number>,
  launch: SpikeLaunch = LAUNCH,
  memory: ReadonlyArray<MemorySample> = [],
) {
  const written = new Map<string, string | Uint8Array>();
  const removed: string[] = [];
  const exits: number[] = [];
  const logs: string[] = [];
  const memoryStart = vi.fn<() => void>();
  const deps: SpikeSessionDeps = {
    launch,
    describe: () => Promise.resolve(RUN),
    trace,
    traceDir: "/profile",
    reduce,
    memory: { start: memoryStart, stop: () => Promise.resolve(memory) },
    outDir: "/out",
    exit: (code) => {
      exits.push(code);
    },
    log: (line) => {
      logs.push(line);
    },
    files: {
      mkdir: () => Promise.resolve(),
      writeFile: (path, data) => {
        written.set(path, data);
        return Promise.resolve();
      },
      rm: (path) => {
        removed.push(path);
        return Promise.resolve();
      },
      size,
      results: {
        mkdir: () => Promise.resolve(),
        exists: (path) => Promise.resolve(written.has(path)),
        writeFile: (path, text) => {
          written.set(path, text);
          return Promise.resolve();
        },
      },
    },
  };
  return { deps, memoryStart, written, removed, exits, logs };
}

/**
 * A session over fakes, with what it wrote and how it ended; its trace files are 4 kB, and a file
 * the trace never wrote cannot be read.
 */
function sessionOf(
  reduce: SpikeSessionDeps["reduce"] = () => Promise.reject(new Error("bad")),
  size?: (path: string) => Promise<number>,
  launch: SpikeLaunch = LAUNCH,
  settings: TraceSettings = UNPROFILED,
  memory: ReadonlyArray<MemorySample> = [],
) {
  const trace = fakeTrace(settings);
  const sizeOf =
    size ??
    ((path: string) =>
      trace.files.has(path)
        ? Promise.resolve(4096)
        : Promise.reject(new Error(`ENOENT: no such file, stat '${path}'`)));
  const { deps, ...made } = depsOf(trace, reduce, sizeOf, launch, memory);
  return { session: new SpikeSession(deps), ...made, trace };
}

/** Memory samples at 1 Hz from 0.2 s, `nvidia-smi` reading each graphics clock in P2 of 2,115 MHz. */
function clockedSamples(graphicsMHz: ReadonlyArray<number>): MemorySample[] {
  return graphicsMHz.map((mhz, i) => ({
    tS: i + 0.2,
    appBytes: 1024,
    gpuProcessBytes: null,
    tracingBytes: 0,
    rendererPrivateBytes: null,
    drmResidentBytes: null,
    drmReason: "no GPU process",
    drmTotalBytes: null,
    nvidiaDeviceBytes: null,
    nvidiaGpuProcessBytes: null,
    clocks: {
      kind: "clocks",
      source: "nvidia-smi",
      maxGraphicsMHz: measured(2115),
      graphicsMHz: measured(mhz),
      memoryMHz: measured(9501),
      performanceState: measured(2),
    },
  }));
}

/** A window's trace on the report's clock, `smallReport`'s one window. */
function goodTrace(): TraceFigures {
  return windowTrace({ offsetUs: 7e9, fromMs: 900, toMs: 61_100 });
}

/** The results file a full run's results call wrote. */
function pathsOf(answer: SpikeResultsAnswer): SpikeResultsPaths {
  if (answer.kind !== "written") {
    throw new Error("a full run's results call wrote no file");
  }
  return answer.paths;
}

/** A smoke's report: 10 s of frames at 60 Hz from 1,000 ms, its trace in three windows. */
function smokeReport(): DescentSpikeReport {
  const scriptStartMs = 1000;
  const scriptTimesS = Array.from({ length: 600 }, (_, i) => i / 60);
  const base = smallReport();
  return {
    ...base,
    scriptStartMs,
    traceWindows: [
      { startedMs: 900, stopRequestedMs: 4000, failure: null },
      { startedMs: 4300, stopRequestedMs: 7000, failure: null },
      { startedMs: 7300, stopRequestedMs: 11_000, failure: null },
    ],
    segments: [{ name: "orbit coast", startS: 0, endS: 10 }],
    frames: {
      scriptTimesS,
      rafIntervalsMs: scriptTimesS.map((_, i) => (i === 0 ? 0 : 1000 / 60)),
      ourCodeMs: scriptTimesS.map(() => 4),
      callbackStartsMs: callbackStartsOf(scriptStartMs, scriptTimesS),
      missingResolves: scriptTimesS.map(() => 0),
      inFlightResolves: scriptTimesS.map(() => 0),
      passes: [{ label: "terrain", row: "terrain", gpuMs: scriptTimesS.map(() => 1) }],
    },
  };
}

/**
 * The smoke's `k`-th window's trace, its frame spans the report's, or, with `skew`, those of the
 * frames from its second second on lengthened by 0.3 ms.
 */
function smokeTrace(k: number, skew = false): TraceFigures {
  const report = smokeReport();
  const time = report.traceWindows[k];
  if (time === undefined) {
    throw new Error(`the smoke has no window ${k}`);
  }
  const frames = skew
    ? {
        frames: {
          ...report.frames,
          ourCodeMs: report.frames.ourCodeMs.map((ms, i) =>
            (report.frames.callbackStartsMs[i] ?? 0) > time.startedMs + 1000 ? ms + 0.3 : ms,
          ),
        },
      }
    : report;
  return windowTrace({
    offsetUs: 7e9,
    fromMs: time.startedMs,
    toMs: time.stopRequestedMs,
    frames,
  });
}

/** The window index of a smoke's window file. */
function windowOf(path: string): number {
  return [0, 1, 2].findIndex((k) => path.endsWith(traceWindowFileName(k)));
}

describe("a spike run's session", () => {
  it("measures between start and stop, removes the trace, and writes a valid results file", async () => {
    const { session, memoryStart, written, removed } = sessionOf();
    const ops = session.operations();
    await ops.startMeasuring();
    expect(memoryStart).toHaveBeenCalledOnce();
    ops.rendererMemory(1234);
    expect(session.rendererPrivateBytes()).toBe(1234);
    await ops.stopMeasuring();
    expect(removed).toEqual(["/profile/spike-trace-0.pftrace"]);
    const paths = pathsOf(await ops.writeResults(smallReport()));
    expect(paths.json).toBe("/out/2026-10-03-devbox-low.json");
    const text = written.get(paths.json);
    expect(typeof text).toBe("string");
    const results: unknown = JSON.parse(String(text));
    expect(validateResults(results)).toEqual([]);
    // A trace that could not be reduced is a missing figure with its reason, not a failed run.
    expect(String(text)).toContain("trace window 1 of 1: the trace could not be reduced: bad");
  });

  it("records its one window: the file's size, its buffer's use, and its figures pooled", async () => {
    // The report's window runs from 900 to 61,100 ms, its script from 1,000 ms.
    const { session, written, removed, trace } = sessionOf(() => Promise.resolve(goodTrace()));
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.stopMeasuring();
    expect(trace.calls).toEqual(["start", "stop /profile/spike-trace-0.pftrace", "close"]);
    expect(removed).toEqual(["/profile/spike-trace-0.pftrace"]);
    const paths = pathsOf(await ops.writeResults(smallReport()));
    const results: unknown = JSON.parse(String(written.get(paths.json)));
    expect(validateResults(results)).toEqual([]);
    expect(results).toMatchObject({
      run: {
        trace: {
          value: {
            profiled: false,
            windows: [
              {
                index: 0,
                fromS: -0.1,
                toS: 60.1,
                figures: { value: { spanMs: 60_200, bytes: 4096, bufferPercent: 12.5 } },
              },
            ],
            boundaries: [],
          },
        },
      },
      mainThread: { split: { value: { busyMs: 100 } } },
    });
  });

  it("fails the window whose file cannot be read, with the reason", async () => {
    const { session, written } = sessionOf(
      () => Promise.reject(new Error("not reached")),
      () => Promise.reject(new Error("ENOENT: no such file")),
    );
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.stopMeasuring();
    const paths = pathsOf(await ops.writeResults(smallReport()));
    const text = String(written.get(paths.json));
    expect(validateResults(JSON.parse(text))).toEqual([]);
    expect(text).toContain("trace window 1 of 1: its file could not be read: ENOENT: no such file");
  });

  it("fails a window whose stop said Chromium lost data", async () => {
    const { session, written, trace } = sessionOf(() => Promise.resolve(goodTrace()));
    trace.told = { lostData: true, bufferPercent: 3 };
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.stopMeasuring();
    const paths = pathsOf(await ops.writeResults(smallReport()));
    const text = String(written.get(paths.json));
    expect(validateResults(JSON.parse(text))).toEqual([]);
    expect(text).toContain(`trace window 1 of 1: ${LOST_DATA_REASON}`);
  });

  it("reduces each window with the categories its trace recorded", async () => {
    const seen: Array<ReadonlyArray<string>> = [];
    const { session } = sessionOf(
      (_path, categories) => {
        seen.push(categories);
        return Promise.resolve(goodTrace());
      },
      undefined,
      LAUNCH,
      PROFILED,
    );
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.cycleTrace();
    await ops.stopMeasuring();
    // A profiled run's, with gpu: not the timed runs' constant.
    expect(seen).toEqual([PROFILED.categories, PROFILED.categories]);
  });

  it("writes a window per cycle, then reduces and deletes each in order, failing only the one whose reduction throws", async () => {
    const reduced: string[] = [];
    const { session, removed, trace, logs } = sessionOf((path) => {
      reduced.push(path);
      return path.endsWith(traceWindowFileName(1))
        ? Promise.reject(new Error("not JSON"))
        : Promise.resolve(goodTrace());
    });
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.cycleTrace();
    await ops.cycleTrace();
    // Nothing is reduced during the run.
    expect(reduced).toEqual([]);
    await ops.stopMeasuring();
    const files = [0, 1, 2].map((k) => `/profile/spike-trace-${k}.pftrace`);
    expect(trace.calls).toEqual([
      "start",
      `cycle ${files[0]}`,
      `cycle ${files[1]}`,
      `stop ${files[2]}`,
      "close",
    ]);
    expect(reduced).toEqual(files);
    expect(removed).toEqual(files);
    expect(logs.filter((line) => line.includes("trace window"))).toEqual([
      "descent spike: trace window 1 of 3: 4096 B, 12.50 % of its buffer",
      "descent spike: trace window 2 of 3 failed: the trace could not be reduced: not JSON",
      "descent spike: trace window 3 of 3: 4096 B, 12.50 % of its buffer",
    ]);
  });

  it("refuses a cycle before measuring, or with the trace stopped, writing no window", async () => {
    const { session, trace } = sessionOf();
    const ops = session.operations();
    await expect(ops.cycleTrace()).rejects.toThrow(/not measuring/);
    await ops.startMeasuring();
    await trace.stop("/elsewhere");
    await expect(ops.cycleTrace()).rejects.toThrow(/not recording/);
    // A trace stopped already has no last window to stop, and the refused cycle wrote none; the
    // transport is still closed.
    await ops.stopMeasuring();
    expect(trace.calls).toEqual(["start", "stop /elsewhere", "close"]);
  });

  it.each<readonly [CycleOutcome, string]>([
    ["stop fails", "trace window 1 of 2: the spike's trace did not stop: service gone"],
    ["start fails", "trace window 2 of 2: the trace's cycle at 30 s failed"],
  ])(
    "keeps the window of a cycle whose %s, stops nothing more, and fails the trace",
    async (outcome, reason) => {
      const { session, trace, removed, written } = sessionOf(() =>
        Promise.resolve(windowTrace({ offsetUs: 7e9, fromMs: 900, toMs: 31_000 })),
      );
      const ops = session.operations();
      await ops.startMeasuring();
      trace.nextCycle = outcome;
      await expect(ops.cycleTrace()).rejects.toThrow(/did not/);
      await ops.stopMeasuring();
      // The cycle's window is reduced and removed; the stopped trace has no last stop.
      expect(trace.calls).toEqual(["start", "cycle /profile/spike-trace-0.pftrace", "close"]);
      expect(removed).toEqual(["/profile/spike-trace-0.pftrace"]);
      const report = smallReport();
      const paths = pathsOf(
        await ops.writeResults({
          ...report,
          traceWindows: [
            { startedMs: 900, stopRequestedMs: 31_000, failure: null },
            {
              startedMs: 31_500,
              stopRequestedMs: 61_100,
              failure: `the trace's cycle at 30 s failed: ${outcome}`,
            },
          ],
        }),
      );
      const text = String(written.get(paths.json));
      expect(validateResults(JSON.parse(text))).toEqual([]);
      expect(text).toContain(reason);
    },
  );

  it("refuses a cycle or a stop while the trace is busy", async () => {
    const { session, trace } = sessionOf();
    const ops = session.operations();
    await ops.startMeasuring();
    trace.busy = true;
    await expect(ops.cycleTrace()).rejects.toThrow(/busy/);
    await expect(ops.stopMeasuring()).rejects.toThrow(/busy/);
    // Nothing was stopped, and no window was begun.
    expect(trace.calls).toEqual(["start"]);
    trace.busy = false;
    await ops.stopMeasuring();
    expect(trace.calls).toEqual(["start", "stop /profile/spike-trace-0.pftrace", "close"]);
  });

  it("fails the last window, not the run, when its stop fails, with the stop's reason", async () => {
    const reduce = vi.fn<SpikeSessionDeps["reduce"]>(() => Promise.resolve(goodTrace()));
    const { session, written, trace, removed, logs } = sessionOf(reduce);
    trace.failStop = true;
    const ops = session.operations();
    await ops.startMeasuring();
    await expect(ops.stopMeasuring()).resolves.toBeUndefined();
    // No part of the failed stop's file is read, and whatever it left is removed.
    expect(reduce).not.toHaveBeenCalled();
    expect(removed).toEqual(["/profile/spike-trace-0.pftrace"]);
    expect(trace.calls.at(-1)).toBe("close");
    expect(logs).toContain(
      "descent spike: the trace's last stop failed: the spike's trace did not stop: service gone",
    );
    const paths = pathsOf(await ops.writeResults(smallReport()));
    expect(String(written.get(paths.json))).toContain(
      "trace window 1 of 1: the spike's trace did not stop: service gone",
    );
  });

  it("checks a smoke's three windows in its results call, logs each, and writes no results file", async () => {
    const { session, written, logs } = sessionOf(
      (path) => Promise.resolve(smokeTrace(windowOf(path))),
      undefined,
      { ...LAUNCH, smoke: true },
    );
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.cycleTrace();
    await ops.cycleTrace();
    await ops.stopMeasuring();
    await expect(ops.writeResults(smokeReport())).resolves.toEqual({
      kind: "smoke checked",
      failure: null,
    });
    expect(written.size).toBe(0);
    // Each window checks the frames at least 0.5 s inside it: about 2 s of 60 Hz frames.
    expect(logs.filter((line) => line.includes("frames match"))).toEqual([
      "descent spike: trace window 1 of 3: the spans of 126 frames match the renderer's",
      "descent spike: trace window 2 of 3: the spans of 102 frames match the renderer's",
      "descent spike: trace window 3 of 3: the spans of 162 frames match the renderer's",
    ]);
    expect(logs.filter((line) => line.includes("boundary"))).toEqual([
      "descent spike: trace boundary 1 of 2: 300 ms from the stop to the next start",
      "descent spike: trace boundary 2 of 2: 300 ms from the stop to the next start",
    ]);
  });

  it("logs a smoke's every clock sample, since it writes no results file", async () => {
    const { session, logs } = sessionOf(
      (path) => Promise.resolve(smokeTrace(windowOf(path))),
      undefined,
      { ...LAUNCH, smoke: true },
      UNPROFILED,
      clockedSamples([1410, 1965, 900]),
    );
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.cycleTrace();
    await ops.cycleTrace();
    await ops.stopMeasuring();
    await ops.writeResults(smokeReport());
    expect(logs).toContain(
      "descent spike: GPU clocks sampled from nvidia-smi, maximum 2115 MHz; graphics MHz 1410 1965 900; memory MHz 9501 9501 9501; performance states 2 2 2",
    );
  });

  it("logs a run's clocks after the warm-up beside its results file", async () => {
    const { session, logs } = sessionOf(
      () => Promise.resolve(goodTrace()),
      undefined,
      LAUNCH,
      UNPROFILED,
      clockedSamples([1965, ...Array.from({ length: 10 }, () => 1980), 1110, 1320]),
    );
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.stopMeasuring();
    await ops.writeResults(smallReport());
    expect(logs).toContain(
      "descent spike: GPU clocks from nvidia-smi; graphics median 1320 MHz (62 %), p5 1110, p95 1980, after the warm-up, of 2115 MHz; memory median 9501 MHz; performance state P2",
    );
  });

  it("answers a smoke's results call with the reason of a window whose frame spans disagree, and the smoke exits 1 with it", async () => {
    const { session, written, logs, exits } = sessionOf(
      (path) => Promise.resolve(smokeTrace(windowOf(path), windowOf(path) === 1)),
      undefined,
      { ...LAUNCH, smoke: true },
    );
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.cycleTrace();
    await ops.cycleTrace();
    await ops.stopMeasuring();
    const reason =
      "trace window 2 of 3: the trace's frame spans disagree with the renderer's (72 of 102 frames)";
    const answer = await ops.writeResults(smokeReport());
    expect(answer).toEqual({ kind: "smoke checked", failure: reason });
    expect(written.size).toBe(0);
    expect(logs).toContain(
      "descent spike: trace window 2 of 3 failed: the trace's frame spans disagree with the renderer's (72 of 102 frames)",
    );
    // The renderer ends the run with the answer's reason.
    ops.end(1, answer.kind === "smoke checked" ? answer.failure : null);
    expect(exits).toEqual([1]);
    expect(logs.at(-1)).toBe(`descent spike: FAIL ${reason}`);
  });

  it("fails a smoke whose window lost data, with the reason", async () => {
    const { session, trace } = sessionOf(
      (path) => Promise.resolve(smokeTrace(windowOf(path))),
      undefined,
      { ...LAUNCH, smoke: true },
    );
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.cycleTrace();
    trace.told = { lostData: true, bufferPercent: 1 };
    await ops.cycleTrace();
    trace.told = EIGHTH;
    await ops.stopMeasuring();
    await expect(ops.writeResults(smokeReport())).resolves.toEqual({
      kind: "smoke checked",
      failure: `trace window 2 of 3: ${LOST_DATA_REASON}`,
    });
  });

  it("goes on when the trace's debugger session detaches mid-run, failing its window with the reason", async () => {
    const fake = new FakeDebugger();
    const writer = memoryWriter();
    const logs: string[] = [];
    const trace = new SpikeTrace(
      new CdpTracing(fake, {
        log: (line) => {
          logs.push(line);
        },
        nowMs: () => 0,
        openFile: writer.openFile,
      }),
    );
    const { deps } = depsOf(
      trace,
      () => Promise.resolve(windowTrace({ offsetUs: 7e9, fromMs: 900, toMs: 31_000 })),
      (path) => Promise.resolve(writer.files.get(path)?.length ?? 0),
    );
    const ops = new SpikeSession(deps).operations();
    await ops.startMeasuring();
    fake.lose("Render process gone.");
    const reason = "the trace's debugger session detached: Render process gone.";
    await expect(ops.cycleTrace()).rejects.toThrow(`the spike's trace did not stop: ${reason}`);
    // The run goes on: its stop and its results.
    await expect(ops.stopMeasuring()).resolves.toBeUndefined();
    expect(fake.calls).toEqual(["attach 1.3", "Tracing.start"]);
    const paths = pathsOf(
      await ops.writeResults({
        ...smallReport(),
        traceWindows: [
          { startedMs: 900, stopRequestedMs: 31_000, failure: null },
          {
            startedMs: 31_000,
            stopRequestedMs: 61_100,
            failure: `the trace's cycle at 30 s failed: the spike's trace did not stop: ${reason}`,
          },
        ],
      }),
    );
    expect(paths.json).toBe("/out/2026-10-03-devbox-low.json");
    expect(logs).toContain(`descent spike: ${reason}`);
  });

  it("refuses to measure twice at once", async () => {
    const { session } = sessionOf();
    const ops = session.operations();
    await ops.startMeasuring();
    await expect(ops.startMeasuring()).rejects.toThrow(/already/);
  });

  it("refuses to measure again once its stop has closed the trace", async () => {
    const { session } = sessionOf();
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.stopMeasuring();
    await expect(ops.startMeasuring()).rejects.toThrow(/debugger session is closed/);
  });

  it("refuses to stop before starting, and to write before measuring", async () => {
    const { session } = sessionOf();
    const ops = session.operations();
    await expect(ops.stopMeasuring()).rejects.toThrow(/not measuring/);
    await expect(ops.writeResults(smallReport())).rejects.toThrow(/before it started/);
  });

  it("writes a capture's two files into --capture's directory", async () => {
    const { session, written } = sessionOf();
    const bin = new Uint8Array([1, 2, 3]);
    await expect(session.operations().writeCapture({ json: "{}", bin })).resolves.toBe("/capture");
    expect(written.get("/capture/capture.json")).toBe("{}");
    expect(written.get("/capture/capture.bin")).toBe(bin);
  });

  it("ends the app with the run's status", () => {
    const { session, exits } = sessionOf();
    expect(session.ended).toBe(false);
    session.operations().end(1, "no patch baked");
    session.operations().end(0, null);
    expect(exits).toEqual([1, 0]);
    expect(session.ended).toBe(true);
  });
});
