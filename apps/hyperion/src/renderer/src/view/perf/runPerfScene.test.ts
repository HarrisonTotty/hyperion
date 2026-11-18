import { PERF_PINS, perfSchema } from "@hyperion/perf";
import { describe, expect, it } from "vitest";

import type {
  PerfApi,
  PerfConfig,
  PerfEnd,
  PerfRepetitionResult,
  PerfRunDescription,
  PerfWarmUpSample,
  PerfWarmUpVerdict,
} from "../../../../preload/api";
import type { PassTimes } from "../engine/types";
import type { WorkerMemory } from "../terrain/workers/pool";
import type { LedgerFigures } from "./ledger";
import { RELEASE_MIN_BYTES } from "./repetitionMetrics";
import {
  type PerfClock,
  type PerfDrawn,
  type PerfDriverChoice,
  type PerfFailure,
  type PerfRunOutcome,
  PerfSceneRun,
  type PerfSceneDriver,
  type PerfSceneSink,
  type PerfStage,
  PERF_FAILED_LOGGED,
  settingValuesOf,
} from "./runPerfScene";

/** The fake script's span, s, and its frames' interval in a repetition, ms. */
const SPAN_S = 10;
const FRAME_MS = 16;

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
  adapter: null,
  capabilities: { timestampQuery: true },
  kernelPaths: null,
  heightWorkers: 3,
  spanS: SPAN_S,
  segments: [
    { name: "high", startS: 0, endS: 5 },
    { name: "low", startS: 5, endS: SPAN_S },
  ],
  demandPerS: () => null,
  notes: ["The fake's views."],
};

const LEDGER: LedgerFigures = {
  liveBytes: {
    "star-cubemap": 0,
    "height-cache": 0,
    "atmosphere-tables": 0,
    "atmosphere-view": 0,
    "coarse-field-gpu": 0,
    "render-targets": 0,
    shadows: 0,
    clouds: 0,
    ocean: 0,
    rings: 0,
    scatter: 0,
    stills: 0,
    other: 0,
  },
  peakBytes: {
    "star-cubemap": 0,
    "height-cache": 64_000_000,
    "atmosphere-tables": 0,
    "atmosphere-view": 0,
    "coarse-field-gpu": 0,
    "render-targets": 8_000_000,
    shadows: 0,
    clouds: 0,
    ocean: 0,
    rings: 0,
    scatter: 0,
    stills: 0,
    other: 0,
  },
  tallyPeakBytes: 72_000_000,
};

/** How the fake draws: its passes' labels and times, and whether its reset keeps its cache. */
interface FakeOptions {
  /** The primary's pass label in the repetitions. */
  readonly label?: string;
  /** Whether unmounting leaves the patches in the cache, a reset that does not happen. */
  readonly keepsCache?: boolean;
  /** Why the views cannot be made, where they cannot. */
  readonly refuses?: PerfFailure;
  /** The tally's releases at each repetition's mount, bytes. */
  readonly releases?: ReadonlyArray<number>;
}

/** A scene that draws its frames when mounted, and its warm-up's at each of the clock's waits. */
class FakeDriver implements PerfSceneDriver {
  readonly #options: FakeOptions;
  #sink: PerfSceneSink | null = null;
  #stage: PerfStage | null = null;
  #resolves = 0;
  #rafMs = 1000;
  #cache = 0;
  /** The warm-up's last frame's pass times, delivered late, at the next mount. */
  #late: PassTimes | null = null;
  readonly mounts: PerfStage[] = [];
  /** The cache's size each time it was read: at each repetition's first frame. */
  readonly cachedAtStart: number[] = [];
  unmounts = 0;
  warmedUp = false;

  constructor(options: FakeOptions = {}) {
    this.#options = options;
  }

  mount(stage: PerfStage, sink: PerfSceneSink): Promise<PerfDrawn> {
    this.mounts.push(stage);
    this.#sink = sink;
    this.#stage = stage;
    if (this.#options.refuses !== undefined) {
      sink.failed(this.#options.refuses);
      return Promise.reject(new Error(this.#options.refuses.reason));
    }
    if (this.#late !== null) {
      sink.passTimes(this.#late);
      this.#late = null;
    }
    if (stage.phase === "warm-up") {
      this.tick();
    } else {
      for (const bytes of this.#options.releases ?? []) {
        sink.allocation({
          kind: "destroyed",
          name: "patch slots",
          bytes,
          category: "height-cache",
        });
      }
      for (let tS = 0; tS <= SPAN_S; tS += 0.5) {
        this.#frame(tS, FRAME_MS, this.#options.label ?? "bodies", 2_000_000);
      }
    }
    return Promise.resolve(DRAWN);
  }

  /** The warm-up's frames: slow, and their passes long, at script time 0. */
  tick(): void {
    if (this.#stage?.phase !== "warm-up") {
      return;
    }
    for (let i = 0; i < 3; i += 1) {
      this.#frame(0, 500, "bodies", 50_000_000);
    }
    // The last frame's times arrive after the warm-up has ended.
    this.#late = this.#held;
  }

  #held: PassTimes | null = null;

  #frame(tS: number, intervalMs: number, label: string, ns: number): void {
    const sink = this.#sink;
    if (sink === null) {
      return;
    }
    this.#rafMs += intervalMs;
    this.#resolves += 2;
    sink.frame({
      engineFrame: this.#resolves,
      scriptTimeS: tS,
      rafTimestampMs: this.#rafMs,
      callbackStartMs: this.#rafMs,
      callbackMs: 4,
      streaming: tS < 5,
    });
    this.#cache += 3;
    sink.patch("resident");
    sink.passTimes({
      frame: this.#resolves - 1,
      timer: "full",
      passes: [{ label, ns, bracketed: false }],
    });
    const instrument: PassTimes = {
      frame: this.#resolves,
      timer: "full",
      passes: [{ label: "view:wireframe", ns: ns / 4, bracketed: false }],
    };
    if (this.#stage?.phase === "warm-up") {
      this.#held = instrument;
    }
    sink.passTimes(instrument);
  }

  unmount(): void {
    this.unmounts += 1;
    this.#stage = null;
    if (this.#options.keepsCache !== true) {
      this.#cache = 0;
    }
  }

  cachedPatches(): number {
    this.cachedAtStart.push(this.#cache);
    return this.#cache;
  }

  workerMemory(): ReadonlyArray<WorkerMemory> {
    return [{ fieldBytes: 15_100_000, wasmBytes: 40_000_000 }];
  }

  memory(): { figures(): LedgerFigures; resetPeaks(): void } {
    return { figures: () => LEDGER, resetPeaks: () => undefined };
  }

  latePipelines(): number {
    return 0;
  }

  endWarmUp(): void {
    this.warmedUp = true;
  }

  resolveCount(): number {
    return this.#resolves;
  }

  internalScale(views: ReadonlyArray<unknown>): ReadonlyArray<{ min: number; median: number }> {
    return views.map(() => ({ min: 1, median: 1 }));
  }
}

const CONFIG: PerfConfig = {
  launch: {
    scene: "cockpit-descent",
    setting: "low",
    out: null,
    repetitions: 2,
    control: false,
    force: false,
  },
  switches: ["--hyperion-gpu-timing"],
  gpuTiming: true,
  launchMode: "vulkan",
  hidden: false,
  serverUrl: "ws://127.0.0.1:40000/bridge",
  arrival: { universe: "0000000000000001", system: PERF_PINS.system, body: PERF_PINS.world },
};

/** The main process's side, recorded: every call in order, and what each was given. */
interface FakeMain {
  readonly api: PerfApi;
  readonly calls: string[];
  readonly descriptions: PerfRunDescription[];
  readonly samples: PerfWarmUpSample[];
  readonly results: PerfRepetitionResult[];
  readonly ends: PerfEnd[];
}

function fakeMain(verdicts: ReadonlyArray<PerfWarmUpVerdict["kind"]>): FakeMain {
  const left = [...verdicts];
  const main: FakeMain = {
    calls: [],
    descriptions: [],
    samples: [],
    results: [],
    ends: [],
    api: {
      config: () => {
        main.calls.push("config");
        return Promise.resolve(CONFIG);
      },
      describe: (description) => {
        main.calls.push("describe");
        main.descriptions.push(description);
        return Promise.resolve();
      },
      warmUpSample: (sample) => {
        main.calls.push("warmUpSample");
        main.samples.push(sample);
        return Promise.resolve({ kind: left.shift() ?? "steady", elapsedS: 1 });
      },
      beginRepetition: (n) => {
        main.calls.push(`beginRepetition ${String(n)}`);
        return Promise.resolve({
          serverUrl: `ws://127.0.0.1:${String(40000 + n)}/bridge`,
          arrival: { ...CONFIG.arrival, universe: `000000000000000${String(n + 1)}` },
        });
      },
      endRepetition: (n, result) => {
        main.calls.push(`endRepetition ${String(n)}`);
        main.results.push(result);
        return Promise.resolve();
      },
      finish: (end) => {
        main.calls.push("finish");
        main.ends.push(end);
        return Promise.resolve();
      },
    },
  };
  return main;
}

/** A clock that moves only as the run waits, the fake's warm-up drawing at each wait. */
function fakeClock(driver: FakeDriver): PerfClock & { readonly waits: number[] } {
  let nowMs = 0;
  const waits: number[] = [];
  return {
    waits,
    nowMs: () => nowMs,
    timeOriginMs: () => 1_763_400_000_000,
    wait: (ms) => {
      waits.push(ms);
      nowMs += ms;
      driver.tick();
      return Promise.resolve();
    },
  };
}

interface Ran {
  readonly main: FakeMain;
  readonly driver: FakeDriver;
  /** What the screen was told of the run's end. */
  readonly outcomes: ReadonlyArray<PerfRunOutcome>;
}

async function ran(
  options: FakeOptions = {},
  verdicts: ReadonlyArray<PerfWarmUpVerdict["kind"]> = ["warming", "warming", "steady"],
  choice?: PerfDriverChoice,
): Promise<Ran> {
  const driver = new FakeDriver(options);
  const main = fakeMain(verdicts);
  const outcomes: PerfRunOutcome[] = [];
  const run = new PerfSceneRun({
    api: main.api,
    clock: fakeClock(driver),
    driverFor: () => choice ?? { kind: "driver", driver },
    signal: new AbortController().signal,
    onEnded: (outcome) => {
      main.calls.push(`ended ${outcome.kind}`);
      outcomes.push(outcome);
    },
  });
  await run.run();
  return { main, driver, outcomes };
}

describe("a perf run in the renderer", () => {
  it("asks for its scene, describes it, warms up until steady, brackets each repetition and finishes complete", async () => {
    const { main, driver } = await ran();
    expect(main.calls).toEqual([
      "config",
      "describe",
      "warmUpSample",
      "warmUpSample",
      "warmUpSample",
      "beginRepetition 1",
      "endRepetition 1",
      "beginRepetition 2",
      "endRepetition 2",
      "ended complete",
      "finish",
    ]);
    expect(main.ends).toEqual([{ status: "complete", notes: "The fake's views." }]);
    expect(driver.warmedUp).toBe(true);
    expect(driver.mounts.map(({ phase, serverUrl }) => [phase, serverUrl])).toEqual([
      ["warm-up", CONFIG.serverUrl],
      ["repetition", "ws://127.0.0.1:40001/bridge"],
      ["repetition", "ws://127.0.0.1:40002/bridge"],
    ]);
    expect(driver.mounts[2]?.arrival.universe).toBe("0000000000000003");
  });

  it("describes the scene's pins, setting, pacing and span with the views as drawn", async () => {
    const { main } = await ran();
    const description = main.descriptions[0];
    expect(description?.scene).toEqual({
      version: 1,
      seed: PERF_PINS.seed,
      world: PERF_PINS.world,
    });
    expect(description?.pacing).toBe("every-second-vsync");
    expect(description?.repetitionSeconds).toBe(1230);
    expect(description?.views).toEqual(DRAWN.views);
    expect(description?.settingValues["quality.terrain.tauPx"]).toBe(2);
  });

  it("hands the warm-up's frame intervals to the main process with each sample", async () => {
    const { main } = await ran();
    expect(main.samples.map(({ intervalsMs }) => intervalsMs)).toEqual([
      [500, 500, 500, 500, 500],
      [500, 500, 500],
      [500, 500, 500],
    ]);
  });

  it("leaves the warm-up's frames out of a repetition's figures", async () => {
    const { main } = await ran();
    for (const result of main.results) {
      // The first interval is from the last frame before the span, which the main process drops.
      expect(new Set(result.frames.intervalsMs.slice(1))).toEqual(new Set([FRAME_MS]));
      expect(result.frames.scriptTimesS).toHaveLength(2 * SPAN_S + 1);
      const bodies = result.passes.find(({ key }) => key === "primary:bodies");
      expect(bodies).toEqual({
        key: "primary:bodies",
        row: "terrain",
        frames: 2 * SPAN_S + 1,
        p50Ms: 2,
        p95Ms: 2,
        p99Ms: 2,
      });
      expect(result.rows.find(({ row }) => row === "cockpit-instruments")?.p99Ms).toBe(0.5);
      expect(result.timedPassesPerFrameMs).toBe(2.5);
    }
  });

  it("finds the patch cache empty at each repetition's start", async () => {
    const { main, driver } = await ran();
    expect(driver.cachedAtStart).toEqual([0, 0]);
    expect(main.ends[0]?.status).toBe("complete");
  });

  it("fails the run where a repetition's reset left patches in the cache", async () => {
    const { main } = await ran({ keepsCache: true });
    expect(main.calls).not.toContain("endRepetition 1");
    expect(main.ends).toEqual([
      {
        status: "failed",
        reason: expect.stringMatching(/^the patch cache held \d+ patches at repetition 1's start/),
      },
    ]);
  });

  it("fails the run on a pass with no row, naming it", async () => {
    const { main } = await ran({ label: "ocean" });
    expect(main.calls).not.toContain("endRepetition 1");
    expect(main.ends).toEqual([
      {
        status: "failed",
        reason: "the timed pass primary:ocean has no row in PASS_ROWS (Design note 4)",
      },
    ]);
  });

  it("fails once where the views cannot be made, before describing them", async () => {
    const reason = "GRAPHICS NO ADAPTER: views not available";
    const { main } = await ran({ refuses: { reason, shown: null } });
    expect(main.calls).toEqual(["config", "ended failed", "finish"]);
    expect(main.ends).toEqual([{ status: "failed", reason }]);
  });

  it("leaves the views mounted on a failure they show their own fault line for", async () => {
    const reason = "GRAPHICS NO ADAPTER: views not available";
    const { driver, outcomes } = await ran({ refuses: { reason, shown: null } });
    expect(driver.unmounts).toBe(0);
    expect(outcomes).toEqual([{ kind: "failed", shown: null }]);
  });

  it("drops the views on a failure the run's own fault line states", async () => {
    const failure = { reason: "the scene: no view", shown: "the scene has no view" };
    const { driver, outcomes } = await ran({ refuses: failure });
    expect(driver.unmounts).toBe(1);
    expect(outcomes).toEqual([{ kind: "failed", shown: "the scene has no view" }]);
  });

  it("states a failure it did not word as one its log gives the reason of", async () => {
    const driver = new FakeDriver();
    const main = fakeMain(["steady"]);
    const outcomes: PerfRunOutcome[] = [];
    await new PerfSceneRun({
      api: {
        ...main.api,
        describe: () => Promise.reject(new Error("PerfRunRefused: describe refused")),
      },
      clock: fakeClock(driver),
      driverFor: () => ({ kind: "driver", driver }),
      signal: new AbortController().signal,
      onEnded: (outcome) => outcomes.push(outcome),
    }).run();
    expect(main.ends).toEqual([{ status: "failed", reason: "PerfRunRefused: describe refused" }]);
    expect(outcomes).toEqual([{ kind: "failed", shown: PERF_FAILED_LOGGED }]);
  });

  it("ends at once where the renderer has no driver for its scene", async () => {
    const reason = "ringed-giant-approach: no renderer driver yet (R12.T4.c)";
    const shown = "this build draws no view of this scene";
    const { main, outcomes } = await ran({}, ["steady"], { kind: "refused", reason, shown });
    expect(main.calls).toEqual(["config", "ended failed", "finish"]);
    expect(main.ends).toEqual([{ status: "failed", reason }]);
    expect(outcomes).toEqual([{ kind: "failed", shown }]);
  });

  it("goes on to the repetitions after a warm-up that is not steady", async () => {
    const { main } = await ran({}, ["not-steady"]);
    expect(main.calls.filter((call) => call === "warmUpSample")).toHaveLength(1);
    expect(main.ends[0]?.status).toBe("complete");
  });

  it("stops without finishing once its signal aborts", async () => {
    const driver = new FakeDriver();
    const main = fakeMain(["warming"]);
    const stop = new AbortController();
    const run = new PerfSceneRun({
      api: {
        ...main.api,
        describe: () => {
          stop.abort();
          return Promise.resolve();
        },
      },
      clock: fakeClock(driver),
      driverFor: () => ({ kind: "driver", driver }),
      signal: stop.signal,
      onEnded: () => undefined,
    });
    await run.run();
    expect(main.ends).toEqual([]);
    expect(driver.unmounts).toBe(1);
  });

  it("tells the screen nothing of a run its signal stopped", async () => {
    const driver = new FakeDriver();
    const main = fakeMain(["warming"]);
    const stop = new AbortController();
    const outcomes: PerfRunOutcome[] = [];
    await new PerfSceneRun({
      api: {
        ...main.api,
        describe: () => {
          stop.abort();
          return Promise.resolve();
        },
      },
      clock: fakeClock(driver),
      driverFor: () => ({ kind: "driver", driver }),
      signal: stop.signal,
      onEnded: (outcome) => outcomes.push(outcome),
    }).run();
    expect(outcomes).toEqual([]);
  });

  it("tells of each release of 16 MiB or more within a repetition, at the page's epoch time", async () => {
    const { main } = await ran({ releases: [RELEASE_MIN_BYTES, RELEASE_MIN_BYTES - 1] }, [
      "steady",
    ]);
    expect(main.results[0]?.releases).toEqual([
      { atEpochMs: 1_763_400_000_000 + 1000, bytes: RELEASE_MIN_BYTES },
    ]);
  });

  it("hands over the itemised tally's peaks and each height worker's memory", async () => {
    const { main } = await ran();
    expect(main.results[0]?.memory).toEqual({
      itemPeakBytes: { "height-cache": 64_000_000, "render-targets": 8_000_000 },
      tallyPeakBytes: 72_000_000,
      workerFieldBytes: [15_100_000],
      workerWasmBytes: [40_000_000],
    });
  });
});

describe("a setting's values for the record", () => {
  it("are every ladder value of R05's SETTINGS under quality.<field path>, as JSON", () => {
    const values = settingValuesOf("high");
    expect(perfSchema.jsonObject(values, "")).toEqual(values);
    expect(Object.keys(values).every((key) => key.startsWith("quality."))).toBe(true);
    expect(values["quality.terrain.tauPx"]).toBe(1);
    expect(values["quality.internalScaleBounds"]).toEqual([0.5, 1]);
    expect(values["quality.budget.photorealisticViews"]).toBeNull();
  });
});
