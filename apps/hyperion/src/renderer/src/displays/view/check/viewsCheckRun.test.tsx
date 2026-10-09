import { act, cleanup, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type {
  ViewsCheckPhaseName,
  ViewsCheckRecord,
  ViewsCheckWindow,
} from "../../../../../preload/api";
import {
  nominalStore,
  renderViewDisplay,
  settle,
  timedEngineSource,
} from "../../../test/viewDisplayHarness";
import type { QualitySetting } from "../../../view/quality/qualitySetting";
import {
  runViewsCheck,
  VIEWS_CHECK_PLAN,
  type ViewsCheckDeps,
  type ViewsCheckTiming,
} from "./viewsCheckRun";
import { ViewsProbe } from "./viewsProbe";
import { PER_CANVAS_OVERHEAD_MS } from "../../../view/budget/viewBudget";

afterEach(() => {
  vi.useRealTimers();
});

/** Short windows on the fake clock. */
const TIMING: ViewsCheckTiming = {
  settleMs: 50,
  guardMs: 50,
  measureMs: 200,
  graceMs: 50,
  resizeStepMs: 50,
  waitMs: 5_000,
  pollMs: 16,
};

/** How the script ended: its record, or what it threw. */
type Outcome = { readonly record: ViewsCheckRecord } | { readonly error: unknown };

/** What the script asked of the main process, in order. */
type Call = readonly [operation: string, phase?: ViewsCheckPhaseName];

/** When each trace started, the windows the script marked, and those it handed on. */
interface Windows {
  readonly tracesStartedMs: number[];
  readonly marked: ViewsCheckWindow[];
  readonly handed: ViewsCheckWindow[];
}

/**
 * Runs the script against `VIEW` with a fake engine to its end: the fake clock advanced a frame
 * at a time, every resolve reported as it is made.
 */
async function runCheck(
  setting: QualitySetting = "high",
  stopAt: ViewsCheckPhaseName | null = null,
): Promise<Driven> {
  const timed = timedEngineSource();
  const probe = new ViewsProbe(timed.source, () => frameMs);
  let frameMs = 0;
  const harness = renderViewDisplay({
    store: await nominalStore(),
    source: probe.source,
    engines: timed.engines,
    setting,
  });
  // The fake clock's frames are stamped on it; the script reads the same clock through them. The
  // wrapper is set through `window`, not `vi.stubGlobal`. Vitest's jsdom global holds the frame
  // function behind a getter and setter that answer the last value set, the fake clock's here. The
  // stub would swap that pair for a plain value, `vi.useRealTimers()` would put jsdom's function
  // on the plain value, and Vitest's unstub, which runs before the next test, would put the pair
  // back, still answering the dead fake clock's function to every later file in the worker
  // (`isolate: false`).
  const fakeFrame = window.requestAnimationFrame;
  window.requestAnimationFrame = (callback) =>
    fakeFrame.call(window, (timeMs) => {
      frameMs = timeMs;
      callback(timeMs);
    });
  const restoreFrames = probe.installFrameClock(window);
  try {
    return await driven(harness, timed, probe, () => frameMs, stopAt);
  } finally {
    restoreFrames();
    window.requestAnimationFrame = fakeFrame;
  }
}

/** What a run of the script did, and how it ended. */
interface Driven {
  readonly outcome: Outcome;
  readonly calls: ReadonlyArray<Call>;
  readonly stageWidths: ReadonlyArray<string>;
  readonly windows: Windows;
}

/**
 * The script run to its end on the fake clock read through `nowMs`, stopped through its signal as
 * the phase `stopAt` starts, if one is given.
 */
async function driven(
  harness: ReturnType<typeof renderViewDisplay>,
  timed: ReturnType<typeof timedEngineSource>,
  probe: ViewsProbe,
  nowMs: () => number,
  stopAt: ViewsCheckPhaseName | null,
): Promise<Driven> {
  await settle();
  harness.advance(100);
  const calls: Call[] = [];
  const stageWidths: string[] = [];
  const windows: Windows = { tracesStartedMs: [], marked: [], handed: [] };
  const stop = new AbortController();
  const deps: ViewsCheckDeps = {
    api: {
      startPhase: (name) => {
        calls.push(["start", name]);
        windows.tracesStartedMs.push(nowMs());
        if (name === stopAt) {
          stop.abort();
        }
        return Promise.resolve();
      },
      endPhase: (name, window) => {
        calls.push(["end", name]);
        windows.handed.push(window);
        const stage = screen
          .getByRole("application", { name: /PRIMARY/ })
          .closest<HTMLElement>(".view__stage");
        stageWidths.push(stage?.style.width ?? "none");
        return Promise.resolve();
      },
      askRightWayUp: () => {
        calls.push(["ask"]);
        return Promise.resolve();
      },
    },
    probe,
    document,
    timing: TIMING,
    nowMs,
    markWindow: (window) => {
      windows.marked.push(window);
    },
    sleep: (ms) =>
      new Promise((resolve) => {
        setTimeout(resolve, ms);
      }),
    signal: stop.signal,
    devicePixelRatio: 1,
  };
  const run: { outcome: Outcome | null } = { outcome: null };
  const running = (async (): Promise<void> => {
    try {
      run.outcome = { record: await runViewsCheck(deps) };
    } catch (error: unknown) {
      run.outcome = { error };
    }
  })();
  // Read through a call, since the awaits below are where it changes.
  const outcome = (): Outcome | null => run.outcome;
  for (let frame = 0; frame < 4_000 && outcome() === null; frame += 1) {
    // Each frame acts on the page the frame before left.
    // oxlint-disable-next-line no-await-in-loop
    await act(async () => {
      await vi.advanceTimersByTimeAsync(16);
    });
    timed.deliver((name) => (name.startsWith("instrument") ? 0.05 : 1));
  }
  const ended = outcome();
  if (ended === null) {
    throw new Error("the script did not end");
  }
  await running;
  return { outcome: ended, calls, stageWidths, windows };
}

/** The script run to its end, its record and what it asked of the main process. */
async function completed(setting: QualitySetting = "high"): Promise<{
  readonly record: ViewsCheckRecord;
  readonly calls: ReadonlyArray<Call>;
  readonly stageWidths: ReadonlyArray<string>;
}> {
  const { outcome, calls, stageWidths } = await runCheck(setting);
  if ("error" in outcome) {
    throw outcome.error;
  }
  return { record: outcome.record, calls, stageWidths };
}

describe("the views check's script", () => {
  it("measures every phase in its order and asks last, with the cockpit set up again", async () => {
    const { record, calls } = await completed();
    expect(record.phases.map((phase) => phase.name)).toEqual(
      VIEWS_CHECK_PLAN.map((plan) => plan.name),
    );
    expect(calls).toEqual([
      ...VIEWS_CHECK_PLAN.flatMap((plan): Call[] => [
        ["start", plan.name],
        ["end", plan.name],
      ]),
      ["ask"],
    ]);
    expect(
      screen.getAllByRole("application").map((canvas) => canvas.getAttribute("aria-label")),
    ).toEqual([
      expect.stringMatching(/^VIEW, PHOTOREALISTIC, PRIMARY/),
      expect.stringMatching(/^VIEW, WIREFRAME, INSTRUMENT 1/),
      expect.stringMatching(/^VIEW, WIREFRAME, INSTRUMENT 2/),
    ]);
  });

  it("holds each phase's views in their styles, and measures their frames", async () => {
    const { record } = await completed();
    for (const plan of VIEWS_CHECK_PLAN) {
      const phase = record.phases.find((each) => each.name === plan.name);
      expect(phase?.views.map((view) => view.style)).toEqual([
        plan.primary,
        ...plan.instruments.filter((style) => style !== null),
      ]);
      expect(phase?.views[0]?.draws).toBeGreaterThan(0);
      expect(phase?.frameGpuMs.length).toBeGreaterThan(0);
    }
  });

  it("frees the low setting's one photorealistic view before an instrument takes it", async () => {
    const { record } = await completed("low");
    const both = record.phases.find((phase) => phase.name === "wireframe-photoreal-wireframe");
    expect(both?.views.map((view) => view.style)).toEqual([
      "wireframe",
      "photorealistic",
      "wireframe",
    ]);
  });

  it("starts each measured window a guard after its trace starts", async () => {
    const { windows } = await runCheck();
    expect(windows.marked).toHaveLength(windows.tracesStartedMs.length);
    // The test's clock is the last frame's stamp, so a wait is its length to within a frame.
    for (const [i, { startMs }] of windows.marked.entries()) {
      expect(startMs - (windows.tracesStartedMs[i] ?? Number.NaN)).toBeGreaterThanOrEqual(
        TIMING.guardMs - 16,
      );
    }
  });

  it("hands each window on as it marked it", async () => {
    const { windows } = await runCheck();
    expect(windows.handed).toEqual(windows.marked);
  });

  it("records the client's per-canvas overhead constant", async () => {
    const { record } = await completed();
    expect(record.perCanvasOverheadMs).toBe(PER_CANVAS_OVERHEAD_MS);
  });

  it("asks nothing more of the main process once it is stopped", async () => {
    const { outcome, calls } = await runCheck("high", "photoreal-two-wireframe");
    expect(outcome).toEqual({ error: new Error("views check: stopped") });
    expect(calls).toEqual([
      ["start", "photoreal-alone"],
      ["end", "photoreal-alone"],
      ["start", "photoreal-two-wireframe"],
    ]);
  });

  it("narrows the primary's stage in steps and gives its width back, the instruments open", async () => {
    const { record, stageWidths } = await completed();
    expect(record.resize.steps.map((step) => step.widthFraction)).toEqual([0.8, 0.65, 1]);
    expect(record.resize.before.map((view) => view.name)).toEqual([
      "view",
      "instrument-1",
      "instrument-2",
    ]);
    // Every phase ends with the stage at its own width.
    expect(stageWidths.every((width) => width === "")).toBe(true);
  });

  it("leaves the page's own animation frames to the tests after it", async () => {
    const own = window.requestAnimationFrame;
    await runCheck();
    // What runs between this test and the next, in this file or a later one in the worker: the
    // `afterEach` hooks, then Vitest's unstub.
    vi.useRealTimers();
    cleanup();
    vi.unstubAllGlobals();
    expect(window.requestAnimationFrame).toBe(own);
  });
});
