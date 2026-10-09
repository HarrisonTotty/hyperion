import { afterEach, describe, expect, it, vi } from "vitest";

import type { DescentSpikeReport, SpikeApi, SpikeEnd, SpikeLaunch } from "../../../../preload/api";
import { TEST_SPIKE_LAUNCH } from "../../test/stubHyperionApi";
import { goldenLevelTable } from "../../test/terrainFixtures";
import type { PassTimes } from "../engine/types";
import { planetGeometry } from "../terrain/planet";
import { RECORD_SEED, recordProfile } from "./demandRecord";
import { DESCENT_SEGMENTS, DescentProfile, landingSiteOf } from "./descentProfile";
import { GpuCapture } from "./capture";
import { PassReads } from "./passReads";
import { PipelineTally } from "./pipelineShim";
import {
  CAPTURE_FRAMES,
  PASS_READS_WAIT_MS,
  SMOKE_S,
  SMOKE_TRACE_BOUNDARIES_S,
  type SpanCapture,
  SpikeController,
  variantOf,
} from "./spikeController";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";
import { TRACE_BOUNDARY_GUARD_S, traceBoundaries } from "./traceWindows";

const LAUNCH: SpikeLaunch = TEST_SPIKE_LAUNCH;

const DESCENT = {
  planet: planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off")),
  profile: recordProfile(),
  omittedSigmaM: Array.from({ length: 25 }, () => 1e9),
};

/**
 * A fake preload, recording each call; each cycle answers as the next queued answer says, and the
 * results call as the main process does: a full run's file written, or a smoke's trace checked,
 * failing with `results.smokeFailure` or, when `results.checkTrace` is set, with the first window
 * the renderer failed, as the main process's merge does.
 */
function fakeSpike(launch: SpikeLaunch): {
  readonly spike: SpikeApi;
  readonly calls: string[];
  readonly ends: SpikeEnd[];
  readonly reports: DescentSpikeReport[];
  /** How the next cycles answer, in order; a cycle beyond them resolves. */
  readonly cycles: Array<() => Promise<void>>;
  readonly results: { smokeFailure: string | null; checkTrace: boolean };
} {
  const calls: string[] = [];
  const ends: SpikeEnd[] = [];
  const reports: DescentSpikeReport[] = [];
  const cycles: Array<() => Promise<void>> = [];
  const results: { smokeFailure: string | null; checkTrace: boolean } = {
    smokeFailure: null,
    checkTrace: true,
  };
  const spike: SpikeApi = {
    launch,
    startTrace: () => {
      calls.push("startTrace");
      return Promise.resolve();
    },
    cycleTrace: () => {
      calls.push("cycleTrace");
      return (cycles.shift() ?? (() => Promise.resolve()))();
    },
    stopTrace: () => {
      calls.push("stopTrace");
      return Promise.resolve();
    },
    sampleMemory: () => {
      calls.push("sampleMemory");
      return Promise.resolve();
    },
    writeResults: (report) => {
      calls.push("writeResults");
      reports.push(report);
      if (!launch.smoke) {
        return Promise.resolve({ kind: "written", paths: { json: "a.json", markdown: "a.md" } });
      }
      const n = report.traceWindows.length;
      const failed = report.traceWindows.findIndex(({ failure }) => failure !== null);
      const window = report.traceWindows[failed];
      const renderer =
        results.checkTrace && window !== undefined && window.failure !== null
          ? `trace window ${failed + 1} of ${n}: ${window.failure}`
          : null;
      return Promise.resolve({ kind: "smoke checked", failure: results.smokeFailure ?? renderer });
    },
    writeCapture: () => {
      calls.push("writeCapture");
      return Promise.resolve("/capture");
    },
    end: (outcome) => {
      calls.push("end");
      ends.push(outcome);
      return Promise.resolve();
    },
  };
  return { spike, calls, ends, reports, cycles, results };
}

/** A capture that records what the controller asks of it. */
function fakeCapture(): SpanCapture & { readonly log: string[] } {
  const log: string[] = [];
  return {
    log,
    startSpan: () => {
      log.push("start");
      return Promise.resolve();
    },
    frame: () => {
      log.push("frame");
    },
    endSpan: () => {
      log.push("end");
    },
    dispose: () => {
      log.push("dispose");
    },
    result: () => {
      log.push("result");
      // An empty capture's own result: the controller only passes it on.
      return new GpuCapture({ contexts: null, meta: {} }).result();
    },
  };
}

function controllerOf(
  launch: SpikeLaunch,
  capture: SpanCapture | null = null,
): ReturnType<typeof fakeSpike> & {
  readonly controller: SpikeController;
  readonly resolves: { value: number; runFrame: (n: number) => number };
  /** The pass-time reads, numbered by `resolves`. */
  readonly reads: PassReads;
  /** The controller's `performance.now()`, ms, which a test moves. */
  readonly clock: { ms: number };
} {
  const fake = fakeSpike(launch);
  const resolves = { value: 0, runFrame: (n: number) => n };
  const reads = new PassReads(() => resolves.value);
  const clock = { ms: 0 };
  const controller = new SpikeController({
    spike: fake.spike,
    gpu: { resolves, reads, tally: new PipelineTally(() => 0) },
    capture,
    canvas: () => ({ widthPx: 1280, heightPx: 720 }),
    nowMs: () => clock.ms,
    log: () => undefined,
  });
  return { ...fake, controller, resolves, reads, clock };
}

/** A frame at a script time, its script starting at `scriptStartMs`. */
function frame(scriptTimeS: number, scriptStartMs = 0) {
  return {
    scriptTimeS,
    rafTimestampMs: scriptStartMs + scriptTimeS * 1000,
    scriptStartMs,
    callbackStartMs: scriptStartMs + scriptTimeS * 1000 + 0.1,
    callbackMs: 3,
    passesSubmitted: 5,
    patchesHard: 100,
    streaming: false,
  };
}

/** Lets the controller's awaited calls run. */
function settle(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

/** The calls other than the trace's and the memory sampler's. */
function untraced(calls: ReadonlyArray<string>): string[] {
  return calls.filter((c) => !c.endsWith("Trace") && c !== "sampleMemory");
}

/** Today's boundaries, s. */
const BOUNDARIES = traceBoundaries(DESCENT.profile.segmentSpans());

/**
 * Flies a prepared run on the fake clock: its start answered, a frame at 0, then one at each
 * boundary, its cycle answered 300 ms later, and one at `endS`, the clock at each frame's script
 * time (script start 0).
 */
async function flyBoundaries(
  run: ReturnType<typeof controllerOf>,
  boundariesS: ReadonlyArray<number>,
  endS: number,
): Promise<void> {
  await settle();
  run.controller.frame(frame(0));
  for (const b of boundariesS) {
    run.clock.ms = b * 1000;
    run.controller.frame(frame(b));
    run.clock.ms = b * 1000 + 300;
    // Each cycle is answered before the next boundary's frame, as a run's are.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
  run.clock.ms = endS * 1000;
  run.controller.frame(frame(endS));
  await settle();
}

/**
 * A prepared run on fake timers whose two frames number two resolves each, every one read: the
 * first frame at 0 s, the second at the script's end, which finishes the run. Each listener the
 * engine's times reach is returned, with a report of a resolve's times.
 */
async function readRun(): Promise<
  ReturnType<typeof controllerOf> & { readonly report: (frame: number) => void }
> {
  const run = controllerOf(LAUNCH);
  const listeners: Array<(times: PassTimes) => void> = [];
  run.controller.engine({
    onPassTimes: (listener) => {
      listeners.push(listener);
      return () => undefined;
    },
    onAllocation: () => () => undefined,
  });
  run.controller.prepared(DESCENT);
  await vi.advanceTimersByTimeAsync(0);
  for (const [t, last] of [
    [0, 2],
    [DESCENT.profile.durationS, 4],
  ] as const) {
    for (let n = last - 1; n <= last; n += 1) {
      run.resolves.value = n;
      run.reads.began();
    }
    run.controller.frame(frame(t));
  }
  const report = (n: number): void => {
    for (const listener of listeners) {
      listener({
        frame: n,
        timer: "full",
        passes: [{ label: "terrain", ns: 2e6, bracketed: false }],
      });
    }
  };
  return { ...run, report };
}

describe("the spike's run control", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("waits after the trace's last stop for the pass-time reads in flight, then reports", async () => {
    vi.useFakeTimers();
    const run = await readRun();
    for (const n of [1, 2]) {
      run.report(n);
    }
    // The trace's stop has resolved; the last frame's two reads are still in flight.
    await vi.advanceTimersByTimeAsync(400);
    expect(run.calls).toContain("stopTrace");
    expect(run.calls).not.toContain("writeResults");
    run.report(3);
    run.report(4);
    await vi.advanceTimersByTimeAsync(0);
    expect(run.calls.at(-2)).toBe("writeResults");
    expect(run.reports[0]?.frames.missingResolves).toEqual([0, 0]);
    expect(run.reports[0]?.frames.inFlightResolves).toEqual([0, 0]);
  });

  it("counts a read still in flight after the wait as missing, in flight at the report", async () => {
    vi.useFakeTimers();
    const run = await readRun();
    for (const n of [1, 2, 3]) {
      run.report(n);
    }
    await vi.advanceTimersByTimeAsync(PASS_READS_WAIT_MS - 1);
    expect(run.calls).not.toContain("writeResults");
    await vi.advanceTimersByTimeAsync(1);
    expect(run.reports[0]?.frames.missingResolves).toEqual([0, 1]);
    expect(run.reports[0]?.frames.inFlightResolves).toEqual([0, 1]);
    expect(run.ends).toEqual([{ status: "pass" }]);
  });

  it("measures the whole descent, then writes its results and ends with a pass", async () => {
    const { controller, calls, ends, reports, resolves } = controllerOf(LAUNCH);
    const listeners: Array<(times: PassTimes) => void> = [];
    controller.engine({
      onPassTimes: (listener) => {
        listeners.push(listener);
        return () => undefined;
      },
      onAllocation: () => () => undefined,
    });
    controller.prepared(DESCENT);
    // The trace's start resolves before the first frame, as it does in a run.
    await settle();
    for (const t of [0, 0.5, 1.2, 600, DESCENT.profile.durationS]) {
      resolves.value += 3;
      controller.frame(frame(t));
    }
    for (const listener of listeners) {
      listener({
        frame: 4,
        timer: "full",
        passes: [{ label: "terrain", ns: 2e6, bracketed: false }],
      });
    }
    await settle();
    // The frame at 600 s passes the first boundary, at 120 s.
    expect(calls.filter((c) => c !== "sampleMemory")).toEqual([
      "startTrace",
      "cycleTrace",
      "stopTrace",
      "writeResults",
      "end",
    ]);
    expect(calls.filter((c) => c === "sampleMemory").length).toBeGreaterThanOrEqual(3);
    expect(ends).toEqual([{ status: "pass" }]);
    expect(reports[0]?.frames.scriptTimesS).toHaveLength(5);
    expect(reports[0]?.canvas).toEqual({ widthPx: 1280, heightPx: 720 });
    expect(reports[0]?.terrain).toEqual({ vertexPath: "face-differences", normals: "mesh" });
    expect(controller.ended).toBe(true);
  });

  it("reports where script time starts, the guard, and a window that no boundary cut", async () => {
    const { controller, reports, clock } = controllerOf(LAUNCH);
    clock.ms = 50;
    controller.prepared(DESCENT);
    await settle();
    controller.frame(frame(0, 120));
    clock.ms = 1_300_000;
    // A frame at the script's end passes no boundary: the run ends there.
    controller.frame(frame(DESCENT.profile.durationS, 120));
    await settle();
    expect(reports[0]?.scriptStartMs).toBe(120);
    expect(reports[0]?.traceGuardS).toBe(TRACE_BOUNDARY_GUARD_S);
    expect(reports[0]?.traceWindows).toEqual([
      { startedMs: 50, stopRequestedMs: 1_300_000, failure: null },
    ]);
  });

  it("cycles the trace once per boundary, and reports every window's times", async () => {
    const run = controllerOf(LAUNCH);
    run.clock.ms = -2000;
    run.controller.prepared(DESCENT);
    await flyBoundaries(run, BOUNDARIES, DESCENT.profile.durationS);
    // Eight boundaries, the last at 950 s: nine windows.
    expect(run.calls.filter((c) => c === "cycleTrace")).toHaveLength(8);
    expect(untraced(run.calls)).toEqual(["writeResults", "end"]);
    const starts = [-2000, ...BOUNDARIES.map((b) => b * 1000 + 300)];
    const stops = [...BOUNDARIES.map((b) => b * 1000), DESCENT.profile.durationS * 1000];
    expect(run.reports[0]?.traceWindows).toEqual(
      starts.map((startedMs, k) => ({ startedMs, stopRequestedMs: stops[k], failure: null })),
    );
    expect(run.ends).toEqual([{ status: "pass" }]);
  });

  it("ends the trace, not the run, when a cycle is refused", async () => {
    const run = controllerOf(LAUNCH);
    run.cycles.push(() => Promise.reject(new Error("the tracing service is gone")));
    run.controller.prepared(DESCENT);
    await flyBoundaries(run, BOUNDARIES, DESCENT.profile.durationS);
    // No cycle follows the refused one.
    expect(run.calls.filter((c) => c === "cycleTrace")).toHaveLength(1);
    const reason = "the trace's cycle at 120 s failed: the tracing service is gone";
    expect(run.reports[0]?.traceWindows).toEqual([
      { startedMs: 0, stopRequestedMs: 120_000, failure: null },
      { startedMs: 120_300, stopRequestedMs: 1_230_000, failure: reason },
    ]);
    expect(run.ends).toEqual([{ status: "pass" }]);
  });

  it("refuses to cycle while the last cycle is pending, ending the trace", async () => {
    const run = controllerOf(LAUNCH);
    const held: { release: (() => void) | null } = { release: null };
    run.cycles.push(
      () =>
        new Promise((resolve) => {
          held.release = resolve;
        }),
    );
    run.controller.prepared(DESCENT);
    await settle();
    run.controller.frame(frame(0));
    run.clock.ms = 120_000;
    run.controller.frame(frame(120));
    await settle();
    run.clock.ms = 240_000;
    run.controller.frame(frame(240));
    expect(held.release).not.toBeNull();
    held.release?.();
    await settle();
    run.clock.ms = 1_230_000;
    run.controller.frame(frame(DESCENT.profile.durationS));
    await settle();
    expect(run.calls.filter((c) => c === "cycleTrace")).toHaveLength(1);
    expect(run.reports[0]?.traceWindows).toEqual([
      { startedMs: 0, stopRequestedMs: 120_000, failure: null },
      {
        startedMs: 240_000,
        stopRequestedMs: 1_230_000,
        failure: "the trace's cycle at 120 s was still pending at the boundary at 240 s",
      },
    ]);
    expect(run.ends).toEqual([{ status: "pass" }]);
  });

  it("stops the trace only once a cycle still in flight at the script's end has settled", async () => {
    const run = controllerOf(LAUNCH);
    const held: { release: (() => void) | null } = { release: null };
    run.controller.prepared(DESCENT);
    const last = BOUNDARIES.at(-1) ?? 0;
    await flyBoundaries(run, BOUNDARIES.slice(0, -1), last - 1);
    run.cycles.push(
      () =>
        new Promise((resolve) => {
          held.release = resolve;
        }),
    );
    run.clock.ms = last * 1000;
    run.controller.frame(frame(last));
    run.clock.ms = 1_230_000;
    run.controller.frame(frame(DESCENT.profile.durationS));
    await settle();
    expect(run.calls).not.toContain("stopTrace");
    run.clock.ms = 1_231_000;
    held.release?.();
    await settle();
    expect(run.calls.slice(-3)).toEqual(["stopTrace", "writeResults", "end"]);
    // The last window began when its late cycle resolved, and stopped at once.
    expect(run.reports[0]?.traceWindows.at(-1)).toEqual({
      startedMs: 1_231_000,
      stopRequestedMs: 1_231_000,
      failure: null,
    });
  });

  it("ends the run when the profile's windows cannot be placed", async () => {
    const long = new DescentProfile(
      DESCENT.profile.figure,
      landingSiteOf(RECORD_SEED),
      {},
      DESCENT_SEGMENTS.map((segment) =>
        segment.name === "approach and flare"
          ? Object.assign({}, segment, { durationS: 150 })
          : segment,
      ),
    );
    const run = controllerOf(LAUNCH);
    run.controller.prepared({ ...DESCENT, profile: long });
    await settle();
    expect(run.calls).toEqual(["end"]);
    expect(run.ends[0]).toMatchObject({ status: "fail" });
    expect(run.ends[0]?.status === "fail" ? run.ends[0].reason : "").toMatch(
      /^the trace's windows could not be placed: the last window, .* is 310 s, more than the 300 s/,
    );
  });

  it("ends the trace when its start is still pending at the first boundary", async () => {
    const run = controllerOf(LAUNCH);
    run.controller.prepared(DESCENT);
    // The start's answer has not run yet when the script passes 120 s.
    run.controller.frame(frame(0));
    run.clock.ms = 120_000;
    run.controller.frame(frame(120));
    await settle();
    run.clock.ms = 1_230_000;
    run.controller.frame(frame(DESCENT.profile.durationS));
    await settle();
    expect(run.calls.filter((c) => c === "cycleTrace")).toEqual([]);
    expect(run.reports[0]?.traceWindows).toEqual([
      {
        startedMs: 120_000,
        stopRequestedMs: 1_230_000,
        failure: "the trace's start was still pending at the boundary at 120 s",
      },
    ]);
  });

  it("passes a smoke run that baked a patch, at 10 s, its trace in three windows", async () => {
    const run = controllerOf({ ...LAUNCH, smoke: true });
    run.controller.prepared(DESCENT);
    run.controller.patch("baked");
    await flyBoundaries(run, SMOKE_TRACE_BOUNDARIES_S, SMOKE_S - 0.1);
    expect(run.ends).toEqual([]);
    run.controller.frame(frame(SMOKE_S));
    await settle();
    expect(run.calls).toEqual([
      "startTrace",
      "cycleTrace",
      "cycleTrace",
      "stopTrace",
      "writeResults",
      "end",
    ]);
    expect(run.ends).toEqual([{ status: "pass" }]);
  });

  it("hands a smoke's report, with its three windows' times, to the results call", async () => {
    const run = controllerOf({ ...LAUNCH, smoke: true });
    run.controller.prepared(DESCENT);
    run.controller.patch("baked");
    await flyBoundaries(run, SMOKE_TRACE_BOUNDARIES_S, SMOKE_S);
    expect(run.reports).toHaveLength(1);
    expect(run.reports[0]?.traceWindows).toEqual([
      { startedMs: 0, stopRequestedMs: 3000, failure: null },
      { startedMs: 3300, stopRequestedMs: 6000, failure: null },
      { startedMs: 6300, stopRequestedMs: 10_000, failure: null },
    ]);
    expect(run.reports[0]?.frames.callbackStartsMs).toHaveLength(
      run.reports[0]?.frames.scriptTimesS.length ?? -1,
    );
  });

  it("fails a smoke whose results call answers a failed window, with that window's reason", async () => {
    const run = controllerOf({ ...LAUNCH, smoke: true });
    const reason =
      "trace window 2 of 3: the trace's frame spans disagree with the renderer's (3 of 102 frames)";
    run.results.smokeFailure = reason;
    run.controller.prepared(DESCENT);
    run.controller.patch("baked");
    await flyBoundaries(run, SMOKE_TRACE_BOUNDARIES_S, SMOKE_S);
    expect(run.calls.slice(-2)).toEqual(["writeResults", "end"]);
    expect(run.ends).toEqual([{ status: "fail", reason }]);
  });

  it("fails a smoke run whose trace ended early, with its failed window's reason", async () => {
    const run = controllerOf({ ...LAUNCH, smoke: true });
    run.cycles.push(() => Promise.reject(new Error("the tracing service is gone")));
    run.controller.prepared(DESCENT);
    run.controller.patch("baked");
    await flyBoundaries(run, SMOKE_TRACE_BOUNDARIES_S, SMOKE_S);
    expect(run.ends).toEqual([
      {
        status: "fail",
        reason: "trace window 2 of 2: the trace's cycle at 3 s failed: the tracing service is gone",
      },
    ]);
  });

  it("fails a smoke run whose trace ended early even if the check passed it", async () => {
    const run = controllerOf({ ...LAUNCH, smoke: true });
    run.results.checkTrace = false;
    run.cycles.push(() => Promise.reject(new Error("the tracing service is gone")));
    run.controller.prepared(DESCENT);
    run.controller.patch("baked");
    await flyBoundaries(run, SMOKE_TRACE_BOUNDARIES_S, SMOKE_S);
    expect(run.ends).toEqual([
      { status: "fail", reason: "the trace's cycle at 3 s failed: the tracing service is gone" },
    ]);
  });

  it("fails a smoke run that baked nothing", async () => {
    const { controller, ends } = controllerOf({ ...LAUNCH, smoke: true });
    controller.prepared(DESCENT);
    await settle();
    controller.frame(frame(SMOKE_S));
    await settle();
    expect(ends).toEqual([{ status: "fail", reason: "no patch was baked in a worker" }]);
  });

  it("refuses a variant the setting cannot take, and records the one drawn", async () => {
    const refused = controllerOf({ ...LAUNCH, setting: "low", vertexPath: "baked-offsets" });
    refused.controller.prepared(DESCENT);
    await settle();
    expect(refused.ends[0]?.status).toBe("fail");
    const run = controllerOf({ ...LAUNCH, setting: "high", vertexPath: "face-differences" });
    run.controller.prepared(DESCENT);
    run.controller.frame(frame(DESCENT.profile.durationS));
    await settle();
    expect(run.reports[0]?.terrain).toEqual({ vertexPath: "face-differences", normals: "double" });
    expect(variantOf({ ...LAUNCH, normals: "mesh" })).toEqual({ normals: "mesh" });
    expect(variantOf(LAUNCH)).toEqual({});
  });

  it("ends once, whatever follows", async () => {
    const { controller, ends } = controllerOf({ ...LAUNCH, smoke: true });
    controller.prepared(DESCENT);
    controller.fail("lost the device");
    controller.fail("again");
    controller.frame(frame(SMOKE_S));
    await settle();
    expect(ends).toEqual([{ status: "fail", reason: "lost the device" }]);
  });

  it("captures a span from 5 s into a smoke run, and writes it before the end", async () => {
    const capture = fakeCapture();
    const { controller, calls } = controllerOf({ ...LAUNCH, smoke: true, capture: "/c" }, capture);
    controller.prepared(DESCENT);
    controller.patch("baked");
    controller.frame(frame(4.9));
    expect(capture.log).toEqual([]);
    controller.frame(frame(5));
    for (let n = 0; n < CAPTURE_FRAMES; n += 1) {
      controller.frame(frame(5 + (n + 1) / 60));
    }
    await settle();
    expect(capture.log.filter((e) => e === "frame")).toHaveLength(CAPTURE_FRAMES);
    expect(capture.log.slice(-2)).toEqual(["end", "result"]);
    expect(untraced(calls)).toEqual(["writeCapture"]);
    controller.frame(frame(SMOKE_S));
    await settle();
    expect(untraced(calls)).toEqual(["writeCapture", "writeResults", "end"]);
  });

  it("ends and writes a span the run outlasts, where the run ends", async () => {
    const capture = fakeCapture();
    const { controller, calls } = controllerOf({ ...LAUNCH, smoke: true, capture: "/c" }, capture);
    controller.prepared(DESCENT);
    controller.patch("baked");
    controller.frame(frame(5));
    controller.frame(frame(6));
    controller.frame(frame(SMOKE_S));
    await settle();
    expect(capture.log).toContain("end");
    expect(untraced(calls)).toEqual(["writeCapture", "writeResults", "end"]);
  });

  it("starts a full run's span 5 s into the low fast pass", () => {
    const capture = fakeCapture();
    const { controller } = controllerOf({ ...LAUNCH, capture: "/c" }, capture);
    controller.prepared(DESCENT);
    const pass = DESCENT.profile.segmentSpans().find(({ name }) => name === "low fast pass");
    controller.frame(frame((pass?.startS ?? 0) + 4.9));
    expect(capture.log).toEqual([]);
    controller.frame(frame((pass?.startS ?? 0) + 5));
    expect(capture.log).toEqual(["start"]);
  });
});
