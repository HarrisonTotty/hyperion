import { describe, expect, it } from "vitest";

import type { DescentSpikeReport, SpikeApi, SpikeEnd, SpikeLaunch } from "../../../../preload/api";
import { goldenLevelTable } from "../../test/terrainFixtures";
import type { PassTimes } from "../engine/types";
import { planetGeometry } from "../terrain/planet";
import { recordProfile } from "./demandRecord";
import { GpuCapture } from "./capture";
import { PipelineTally } from "./pipelineShim";
import {
  CAPTURE_FRAMES,
  SMOKE_S,
  type SpanCapture,
  SpikeController,
  variantOf,
} from "./spikeController";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

const LAUNCH: SpikeLaunch = {
  setting: "low",
  seed: "7",
  smoke: false,
  out: null,
  workers: null,
  vertexPath: null,
  normals: null,
  ridged: "off",
  dawnSafety: "on",
  capture: null,
};

const DESCENT = {
  planet: planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off")),
  profile: recordProfile(),
  omittedSigmaM: Array.from({ length: 25 }, () => 1e9),
};

/** A fake preload, recording each call. */
function fakeSpike(launch: SpikeLaunch): {
  readonly spike: SpikeApi;
  readonly calls: string[];
  readonly ends: SpikeEnd[];
  readonly reports: DescentSpikeReport[];
} {
  const calls: string[] = [];
  const ends: SpikeEnd[] = [];
  const reports: DescentSpikeReport[] = [];
  const spike: SpikeApi = {
    launch,
    startTrace: () => {
      calls.push("startTrace");
      return Promise.resolve();
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
      return Promise.resolve({ json: "a.json", markdown: "a.md" });
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
  return { spike, calls, ends, reports };
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
} {
  const fake = fakeSpike(launch);
  const resolves = { value: 0, runFrame: (n: number) => n };
  const controller = new SpikeController({
    spike: fake.spike,
    gpu: { resolves, tally: new PipelineTally(() => 0) },
    capture,
    canvas: () => ({ widthPx: 1280, heightPx: 720 }),
    log: () => undefined,
  });
  return { ...fake, controller, resolves };
}

function frame(scriptTimeS: number) {
  return {
    scriptTimeS,
    rafTimestampMs: scriptTimeS * 1000,
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

describe("the spike's run control", () => {
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
    expect(calls.filter((c) => c !== "sampleMemory")).toEqual([
      "startTrace",
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

  it("passes a smoke run that baked a patch, at 10 s, with no trace or results", async () => {
    const { controller, calls, ends } = controllerOf({ ...LAUNCH, smoke: true });
    controller.prepared(DESCENT);
    controller.frame(frame(0));
    controller.patch("requested");
    controller.patch("baked");
    controller.frame(frame(SMOKE_S - 0.1));
    expect(ends).toEqual([]);
    controller.frame(frame(SMOKE_S));
    await settle();
    expect(calls).toEqual(["end"]);
    expect(ends).toEqual([{ status: "pass" }]);
  });

  it("fails a smoke run that baked nothing", async () => {
    const { controller, ends } = controllerOf({ ...LAUNCH, smoke: true });
    controller.prepared(DESCENT);
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
    expect(calls).toEqual(["writeCapture"]);
    controller.frame(frame(SMOKE_S));
    await settle();
    expect(calls).toEqual(["writeCapture", "end"]);
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
    expect(calls).toEqual(["writeCapture", "end"]);
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
