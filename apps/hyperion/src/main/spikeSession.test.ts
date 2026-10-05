import { describe, expect, it, vi } from "vitest";

import type { SpikeLaunch } from "../preload/api";
import { DEFAULT_SPIKE_SEED } from "../preload/spikeLaunch";
import { measured, type RunDescription, validateResults } from "./results";
import type { TraceFigures } from "./reduceTrace";
import { SpikeSession, type SpikeSessionDeps, traceWindowFileName } from "./spikeSession";
import { smallReport } from "./fixtures/spikeReport";
import { UNPROFILED, windowTrace } from "./fixtures/traces";

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

/** A trace that records its calls, its buffer an eighth used before every stop. */
function fakeTrace(): SpikeSessionDeps["trace"] & {
  readonly calls: string[];
  failStop: boolean;
  /** Holds the trace busy, as a start, stop or cycle in flight does. */
  busy: boolean;
  nextCycle: CycleOutcome;
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
    settings: UNPROFILED,
    get state() {
      return this.busy ? "busy" : state;
    },
    start() {
      calls.push("start");
      state = "recording";
      return Promise.resolve();
    },
    stop(path) {
      calls.push(`stop ${path}`);
      state = "idle";
      if (this.failStop) {
        return Promise.reject(new Error("service gone"));
      }
      files.add(path);
      return Promise.resolve(path);
    },
    cycle(path) {
      calls.push(`cycle ${path}`);
      const outcome = this.nextCycle;
      this.nextCycle = "whole";
      if (outcome === "stop fails") {
        state = "idle";
        return Promise.reject(new Error("the spike's trace did not stop"));
      }
      files.add(path);
      if (outcome === "start fails") {
        state = "idle";
        return Promise.reject(new Error("the spike's trace did not start"));
      }
      return Promise.resolve(path);
    },
    bufferUsage() {
      calls.push("buffer");
      return Promise.resolve(12.5);
    },
  };
}

/**
 * A session over fakes, with what it wrote and how it ended; its trace files are 4 kB, and a file
 * the trace never wrote cannot be read.
 */
function sessionOf(
  reduce: SpikeSessionDeps["reduce"] = () => Promise.reject(new Error("bad")),
  size?: (path: string) => Promise<number>,
  launch: SpikeLaunch = LAUNCH,
) {
  const written = new Map<string, string | Uint8Array>();
  const removed: string[] = [];
  const exits: number[] = [];
  const logs: string[] = [];
  const memoryStart = vi.fn<() => void>();
  const trace = fakeTrace();
  const sizeOf =
    size ??
    ((path: string) =>
      trace.files.has(path)
        ? Promise.resolve(4096)
        : Promise.reject(new Error(`ENOENT: no such file, stat '${path}'`)));
  const deps: SpikeSessionDeps = {
    launch,
    describe: () => Promise.resolve(RUN),
    trace,
    traceDir: "/profile",
    reduce,
    memory: { start: memoryStart, stop: () => Promise.resolve([]) },
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
      size: sizeOf,
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
  return { session: new SpikeSession(deps), memoryStart, written, removed, exits, logs, trace };
}

/** A window's trace on the report's clock, `smallReport`'s one window. */
function goodTrace(): TraceFigures {
  return windowTrace({ offsetUs: 7e9, fromMs: 900, toMs: 61_100 });
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
    expect(removed).toEqual(["/profile/spike-trace-0.json"]);
    const paths = await ops.writeResults(smallReport());
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
    expect(trace.calls).toEqual(["start", "buffer", "stop /profile/spike-trace-0.json"]);
    expect(removed).toEqual(["/profile/spike-trace-0.json"]);
    const paths = await ops.writeResults(smallReport());
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
    const paths = await ops.writeResults(smallReport());
    const text = String(written.get(paths.json));
    expect(validateResults(JSON.parse(text))).toEqual([]);
    expect(text).toContain("trace window 1 of 1: its file could not be read: ENOENT: no such file");
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
    const files = [0, 1, 2].map((k) => `/profile/spike-trace-${k}.json`);
    expect(trace.calls).toEqual([
      "start",
      "buffer",
      `cycle ${files[0]}`,
      "buffer",
      `cycle ${files[1]}`,
      "buffer",
      `stop ${files[2]}`,
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
    // A trace stopped already has no last window to stop, and the refused cycle wrote none.
    await ops.stopMeasuring();
    expect(trace.calls).toEqual(["start", "stop /elsewhere"]);
  });

  it.each<readonly [CycleOutcome, string]>([
    ["stop fails", "trace window 1 of 2: its file could not be read: ENOENT"],
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
      expect(trace.calls).toEqual(["start", "buffer", "cycle /profile/spike-trace-0.json"]);
      expect(removed).toEqual(["/profile/spike-trace-0.json"]);
      const report = smallReport();
      const paths = await ops.writeResults({
        ...report,
        traceWindows: [
          { startedMs: 900, stopRequestedMs: 31_000, failure: null },
          {
            startedMs: 31_500,
            stopRequestedMs: 61_100,
            failure: `the trace's cycle at 30 s failed: ${outcome}`,
          },
        ],
      });
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
    expect(trace.calls).toEqual(["start", "buffer", "stop /profile/spike-trace-0.json"]);
  });

  it("fails the last window, not the run, when its stop fails", async () => {
    const { session, written, trace } = sessionOf(
      () => Promise.resolve(goodTrace()),
      (path) =>
        path.endsWith(traceWindowFileName(0))
          ? Promise.reject(new Error("ENOENT: no such file"))
          : Promise.resolve(4096),
    );
    trace.failStop = true;
    const ops = session.operations();
    await ops.startMeasuring();
    await expect(ops.stopMeasuring()).resolves.toBeUndefined();
    const paths = await ops.writeResults(smallReport());
    expect(String(written.get(paths.json))).toContain(
      "trace window 1 of 1: its file could not be read: ENOENT: no such file",
    );
  });

  it("fails a smoke run's stop when a window failed, and passes one whose windows are whole", async () => {
    const smoke = { ...LAUNCH, smoke: true };
    const bad = sessionOf(
      (path) =>
        Promise.resolve(
          path.endsWith(traceWindowFileName(1)) ? { ...goodTrace(), span: null } : goodTrace(),
        ),
      undefined,
      smoke,
    );
    await bad.session.operations().startMeasuring();
    await bad.session.operations().cycleTrace();
    await expect(bad.session.operations().stopMeasuring()).rejects.toThrow(
      "trace window 2 of 2: the trace has no timed event",
    );
    const good = sessionOf(() => Promise.resolve(goodTrace()), undefined, smoke);
    await good.session.operations().startMeasuring();
    await good.session.operations().cycleTrace();
    await expect(good.session.operations().stopMeasuring()).resolves.toBeUndefined();
  });

  it("measures again after a stop, but not twice at once", async () => {
    const { session } = sessionOf();
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.stopMeasuring();
    await expect(ops.startMeasuring()).resolves.toBeUndefined();
    await expect(ops.startMeasuring()).rejects.toThrow(/already/);
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
