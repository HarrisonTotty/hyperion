import { PERF_PINS, type PerfSceneId } from "@hyperion/perf";
import { render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { describe, expect, it } from "vitest";

import type { PerfApi, PerfEnd } from "../../../../preload/api";
import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { statusLine } from "../../test/statusLine";
import { TEST_GRAPHICS } from "../../test/stubHyperionApi";
import { PERF_PREPARING, PerfApp } from "./PerfApp";
import {
  PASS_TIMES_SETTLE_MS,
  PERF_COMPLETE,
  type PerfDrawn,
  type PerfDriverChoice,
  perfFailedLine,
  type PerfSceneDriver,
  WARM_UP_SAMPLE_MS,
} from "./runPerfScene";

/** The main process's side of a run of `scene`, one repetition long, its end recorded. */
function fakePerf(scene: PerfSceneId, ends: PerfEnd[]): PerfApi {
  const arrival = { universe: "0000000000000001", system: PERF_PINS.system, body: PERF_PINS.giant };
  const serverUrl = "ws://127.0.0.1:40000/bridge";
  return {
    config: () =>
      Promise.resolve({
        launch: { scene, setting: "low", out: null, repetitions: 1, control: false, force: false },
        switches: [],
        gpuTiming: true,
        launchMode: "vulkan",
        hidden: false,
        serverUrl,
        arrival,
      }),
    describe: () => Promise.resolve(),
    warmUpSample: () => Promise.resolve({ kind: "steady", elapsedS: 120 }),
    beginRepetition: () => Promise.resolve({ serverUrl, arrival }),
    endRepetition: () => Promise.resolve(),
    finish: (end) => {
      ends.push(end);
      return Promise.resolve();
    },
  };
}

/** The fake scene's span, s. */
const SPAN_S = 10;

/** What the fake scene's views are. */
const DRAWN: PerfDrawn = {
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
  ],
  adapter: null,
  capabilities: null,
  kernelPaths: null,
  heightWorkers: 1,
  span: {
    kind: "script",
    spanS: SPAN_S,
    segments: [{ name: "all", startS: 0, endS: SPAN_S }],
    demandPerS: () => null,
  },
  notes: [],
};

/** A scene whose views mount at once and fly a repetition's whole span in its first frame. */
function flownAtOnce(): PerfDriverChoice {
  const driver: PerfSceneDriver = {
    mount: (stage, sink) => {
      if (stage.phase === "repetition") {
        sink.frame({
          engineFrame: 1,
          scriptTimeS: SPAN_S,
          rafTimestampMs: 1000,
          callbackStartMs: 1000,
          callbackMs: 4,
          streaming: false,
        });
      }
      return Promise.resolve(DRAWN);
    },
    unmount: () => undefined,
    cachedPatches: () => 0,
    workerMemory: () => [],
    memory: () => null,
    latePipelines: () => 0,
    endWarmUp: () => undefined,
    resolveCount: () => 1,
    internalScale: (views) => views.map(() => ({ min: 1, median: 1 })),
  };
  return { kind: "driver", driver };
}

describe("PerfApp", () => {
  it("stands ready with the spike's frame before the scene's views are mounted", () => {
    render(<PerfApp perf={fakePerf("ringed-giant-approach", [])} graphics={TEST_GRAPHICS} />);
    expect(screen.getByText(statusLine(PERF_PREPARING))).toBeDefined();
  });

  it("ends a run of a scene it has no driver for at once, its reason in full for the record", async () => {
    const ends: PerfEnd[] = [];
    render(<PerfApp perf={fakePerf("ringed-giant-approach", ends)} graphics={TEST_GRAPHICS} />);
    await waitFor(() => {
      expect(ends).toEqual([
        { status: "failed", reason: "ringed-giant-approach: no renderer driver yet (R12.T4.c)" },
      ]);
    });
  });

  it("shows a failed run's fault line with its cause in the guide's words", async () => {
    render(<PerfApp perf={fakePerf("ringed-giant-approach", [])} graphics={TEST_GRAPHICS} />);
    expect(
      await screen.findByText(statusLine(perfFailedLine("this build draws no view of this scene"))),
    ).toBeDefined();
  });

  it("shows PERFORMANCE RUN COMPLETE once its run has ended complete", async () => {
    fakeFramesAndTimeouts();
    render(
      <PerfApp
        perf={fakePerf("cockpit-descent", [])}
        graphics={TEST_GRAPHICS}
        driverFor={flownAtOnce}
      />,
    );
    // The warm-up's sample and the pass times' settling, on the faked clock.
    const waitMs = WARM_UP_SAMPLE_MS + PASS_TIMES_SETTLE_MS + 1000;
    expect(
      await screen.findByText(statusLine(PERF_COMPLETE), {}, { timeout: waitMs }),
    ).toBeDefined();
  });

  it("ends its run once though its effects run twice under StrictMode", async () => {
    const ends: PerfEnd[] = [];
    render(
      <StrictMode>
        <PerfApp perf={fakePerf("nuclear-disc-sky", ends)} graphics={TEST_GRAPHICS} />
      </StrictMode>,
    );
    await waitFor(() => {
      expect(ends).toHaveLength(1);
    });
    await Promise.resolve();
    expect(ends).toHaveLength(1);
  });

  it("ends a descent's run at once in a renderer without WebGPU", async () => {
    const ends: PerfEnd[] = [];
    render(<PerfApp perf={fakePerf("cockpit-descent", ends)} graphics={TEST_GRAPHICS} />);
    await waitFor(() => {
      expect(ends).toEqual([{ status: "failed", reason: "this renderer has no WebGPU" }]);
    });
  });
});
