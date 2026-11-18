import { PERF_PINS, type PerfSceneId } from "@hyperion/perf";
import { describe, expect, it } from "vitest";

import type { PerfConfig } from "../../../../preload/api";
import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable } from "../../test/terrainFixtures";
import type { MeasureDescent } from "../spike/bodyDescent";
import type { SpikeDrawnFrame, SpikeDrawnSession } from "../spike/DescentSpike";
import { DescentProfile, landingSiteOf, trackStretches } from "../spike/descentProfile";
import { contactRule, TEST_PLANET_FIGURE } from "../spike/spikeScene";
import type { PreparedDescent, SpikeFrameSample } from "../spike/spikeRun";
import { planetGeometry } from "../terrain/planet";
import type { TerrainSurvey } from "../terrain/session";
import { ARRIVAL_SETTLE_S } from "./arrivalHolds";
import {
  capabilityFlags,
  DescentDriver,
  type DescentStage,
  DescentStages,
  faultFailure,
  fieldResident,
  measuredOnce,
  PerfEngine,
  siteSeaNote,
  undisposable,
} from "./descentDriver";
import { attributePasses, passRolesOf } from "./passRows";
import type { MeasuredSpan, PerfFailure, PerfFrameSample, PerfSceneSink } from "./runPerfScene";
import { PERF_LANDING_SEED, PERF_SCENES } from "./scenes";

const PROFILE = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(7n));

const PREPARED: PreparedDescent = {
  planet: planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off")),
  profile: PROFILE,
  contact: contactRule(PROFILE),
  siteHeightM: 0,
  stretches: trackStretches(PROFILE),
  stretchMaxHeightsM: trackStretches(PROFILE).map(() => 0),
  trackMaxHeightM: 0,
  omittedSigmaM: new Float64Array(25),
};

function config(scene: PerfSceneId): PerfConfig {
  return {
    launch: { scene, setting: "low", out: null, repetitions: 1, control: false, force: false },
    switches: [],
    gpuTiming: true,
    launchMode: "vulkan",
    hidden: false,
    serverUrl: "ws://127.0.0.1:40000/bridge",
    arrival: { universe: "0000000000000001", system: PERF_PINS.system, body: PERF_PINS.world },
  };
}

/** A sink that records what it is told. */
class RecordingSink implements PerfSceneSink {
  readonly frames: PerfFrameSample[] = [];
  readonly failures: PerfFailure[] = [];
  readonly spans: MeasuredSpan[] = [];
  frame(drawn: PerfFrameSample): void {
    this.frames.push(drawn);
  }
  patch(): void {}
  spanEnded(span: MeasuredSpan): void {
    this.spans.push(span);
  }
  leaveOutResolves(): void {}
  passTimes(): void {}
  allocation(): void {}
  failed(failure: PerfFailure): void {
    this.failures.push(failure);
  }
}

const SIZES: SpikeDrawnFrame["sizes"] = {
  main: { widthPx: 1280, heightPx: 720 },
  orbit: { widthPx: 240, heightPx: 180 },
  craft: { widthPx: 240, heightPx: 180 },
};

function sample(scriptTimeS: number): SpikeFrameSample {
  return {
    scriptTimeS,
    rafTimestampMs: 1000 + scriptTimeS,
    scriptStartMs: 1000,
    callbackStartMs: 1000 + scriptTimeS,
    callbackMs: 3,
    passesSubmitted: null,
    patchesHard: 0,
    streaming: false,
    geometry: null,
  };
}

/** The world's survey with its field's answer held, or still asked for. */
function survey(held: boolean, failure: TerrainSurvey["status"]["failure"] = null): TerrainSurvey {
  return {
    status: { reason: "local-body", pending: !held, notModelled: null, failure },
    coverage: null,
    header: held ? moonLikeHeader(PERF_PINS.world) : null,
    stale: false,
  };
}

/**
 * A session whose lit pass holds `slots` patches, its workers each holding `fieldBytes` of the
 * world's field, its answer held and every block reported where any is.
 */
function session(
  slots: number,
  fieldBytes: ReadonlyArray<number> = [15_100_000],
  surveyed: TerrainSurvey = survey(fieldBytes.some((bytes) => bytes > 0)),
): SpikeDrawnSession {
  return {
    workerMemory: fieldBytes.map((bytes) => ({ fieldBytes: bytes, wasmBytes: 40_000_000 })),
    pass: () => ({ usedSlots: slots }),
    wirePass: () => null,
    survey: () => surveyed,
    levels: () => (surveyed.header === null ? null : new Float64Array(100)),
  };
}

interface Driven {
  readonly driver: DescentDriver;
  readonly stages: DescentStages;
  readonly measured: string[];
}

function driven(scene: PerfSceneId = "cockpit-descent"): Driven {
  const stages = new DescentStages();
  const measured: string[] = [];
  const measure: MeasureDescent = (_session, world) => {
    measured.push(world.body);
    return Promise.resolve(PREPARED);
  };
  const driver = new DescentDriver({
    scene: PERF_SCENES[scene],
    setting: "low",
    workers: 3,
    engine: new PerfEngine(undefined),
    stages,
    measure,
  });
  return { driver, stages, measured };
}

/** The stage now, which the test draws as `PerfApp`'s spike would. */
function current(stages: DescentStages): DescentStage {
  const stage = stages.current();
  if (stage === null) {
    throw new Error("no stage is mounted");
  }
  return stage;
}

/** Draws one frame on `stage` as the spike does: its sample, then what it drew on. */
function draw(stage: DescentStage, tS: number, drawnOn: SpikeDrawnSession | null): void {
  stage.listeners.onFrame?.(sample(tS));
  stage.perf.onDrawn?.({ session: drawnOn, sizes: SIZES });
}

describe("the descent driver", () => {
  it("mounts the warm-up on the config's server and arrival, the script held at time 0", async () => {
    const { driver, stages } = driven();
    const sink = new RecordingSink();
    const mounted = driver.mount(
      {
        phase: "warm-up",
        n: 0,
        serverUrl: "ws://a/bridge",
        arrival: config("cockpit-descent").arrival,
      },
      sink,
    );
    const stage = current(stages);
    expect(stage).toMatchObject({
      key: "warm-up",
      serverUrl: "ws://a/bridge",
      universe: "0000000000000001",
      body: PERF_PINS.world,
      seed: BigInt(PERF_LANDING_SEED),
      setting: "low",
      style: "photorealistic",
      workers: 3,
    });
    expect(stage.perf.scriptNowMs?.(123_456)).toBe(0);
    stage.listeners.onPrepared?.(PREPARED);
    draw(stage, 0, session(0));
    const drawn = await mounted;
    expect(drawn.views.map(({ role, style, widthPx }) => [role, style, widthPx])).toEqual([
      ["primary", "photorealistic", 1280],
      ["instrument", "wireframe", 240],
      ["instrument", "wireframe", 240],
    ]);
    expect(drawn.span).toMatchObject({
      kind: "script",
      spanS: 1230,
      segments: PROFILE.segmentSpans(),
    });
    expect(drawn.heightWorkers).toBe(3);
    expect(sink.frames.map(({ scriptTimeS }) => scriptTimeS)).toEqual([0]);
  });

  it("mounts each repetition anew under its own key, flying from its first frame", () => {
    const { driver, stages } = driven();
    void driver
      .mount(
        {
          phase: "repetition",
          n: 2,
          serverUrl: "ws://b/bridge",
          arrival: { ...config("cockpit-descent").arrival, universe: "0000000000000003" },
        },
        new RecordingSink(),
      )
      .catch(() => undefined);
    const stage = current(stages);
    expect([stage.key, stage.serverUrl, stage.universe]).toEqual([
      "repetition-2",
      "ws://b/bridge",
      "0000000000000003",
    ]);
    expect(stage.perf.scriptNowMs).toBeUndefined();
    driver.unmount();
    expect(stages.current()).toBeNull();
  });

  it("reads the patch cache and the workers of the session its views last drew through", () => {
    const { driver, stages } = driven();
    void driver
      .mount(
        {
          phase: "repetition",
          n: 1,
          serverUrl: "ws://b/bridge",
          arrival: config("cockpit-descent").arrival,
        },
        new RecordingSink(),
      )
      .catch(() => undefined);
    expect(driver.cachedPatches()).toBe(0);
    draw(current(stages), 0, session(12));
    expect(driver.cachedPatches()).toBe(12);
    expect(driver.workerMemory()).toEqual([{ fieldBytes: 15_100_000, wasmBytes: 40_000_000 }]);
    driver.unmount();
    expect(driver.cachedPatches()).toBe(0);
  });

  it("tells no frame of a stage it has left", () => {
    const { driver, stages } = driven();
    const sink = new RecordingSink();
    void driver
      .mount(
        {
          phase: "repetition",
          n: 1,
          serverUrl: "ws://b/bridge",
          arrival: config("cockpit-descent").arrival,
        },
        sink,
      )
      .catch(() => undefined);
    const left = current(stages);
    driver.unmount();
    draw(left, 0, session(0));
    left.listeners.onFailed?.("SCENE TIMED OUT");
    expect(sink.frames).toEqual([]);
    expect(sink.failures).toEqual([]);
  });

  it("hands every mount the one measurement", () => {
    const { driver, stages } = driven();
    const measures = [1, 2].map((n) => {
      void driver
        .mount(
          {
            phase: "repetition",
            n,
            serverUrl: "ws://b/bridge",
            arrival: config("cockpit-descent").arrival,
          },
          new RecordingSink(),
        )
        .catch(() => undefined);
      return current(stages).measure;
    });
    expect(measures[0]).toBe(measures[1]);
  });

  it("draws the station's main view in the wireframe style, its role the station", () => {
    const { driver, stages } = driven("station-wireframe-descent");
    void driver
      .mount(
        {
          phase: "warm-up",
          n: 0,
          serverUrl: "ws://a/bridge",
          arrival: config("cockpit-descent").arrival,
        },
        new RecordingSink(),
      )
      .catch(() => undefined);
    expect(current(stages).style).toBe("wireframe");
  });

  it("draws the station's wireframe alone at 1920 x 1080, no instrument among its views or rows", async () => {
    const { driver, stages } = driven("station-wireframe-descent");
    const mounted = driver.mount(
      {
        phase: "warm-up",
        n: 0,
        serverUrl: "ws://a/bridge",
        arrival: config("cockpit-descent").arrival,
      },
      new RecordingSink(),
    );
    const stage = current(stages);
    expect([stage.perf.instruments, stage.perf.mainSizePx]).toEqual([
      "off",
      { widthPx: 1920, heightPx: 1080 },
    ]);
    stage.listeners.onPrepared?.(PREPARED);
    stage.listeners.onFrame?.(sample(0));
    stage.perf.onDrawn?.({
      session: session(0),
      sizes: { ...SIZES, main: { widthPx: 1920, heightPx: 1080 } },
    });
    const drawn = await mounted;
    expect(drawn.views).toEqual([
      {
        role: "station",
        style: "wireframe",
        widthPx: 1920,
        heightPx: 1080,
        rateHz: 60,
        lineAntialiasing: "analytic",
        exposure: null,
      },
    ]);
    const roles = passRolesOf(drawn.views);
    const keys = attributePasses(
      { passes: [{ label: "view:wireframe", ns: 65_536, bracketed: false }] },
      roles,
    ).map(({ key, row }) => `${key} ${row}`);
    expect([roles, keys]).toEqual([
      { first: "station", wireframe: "station" },
      ["station:view:wireframe station-wireframe"],
    ]);
  });

  it("fails a mount whose arrival names no body", async () => {
    const { driver } = driven();
    const sink = new RecordingSink();
    await expect(
      driver.mount(
        {
          phase: "warm-up",
          n: 0,
          serverUrl: "ws://a/bridge",
          arrival: { ...config("cockpit-descent").arrival, body: null },
        },
        sink,
      ),
    ).rejects.toThrow("beside no body");
    expect(sink.failures).toEqual([
      {
        reason: "cockpit-descent: the arrival placed the ship beside no body",
        shown: "the arrival placed the ship beside no body",
      },
    ]);
  });

  it("tells a failure the spike shows as its own fault line as one the screen keeps", async () => {
    const { driver, stages } = driven();
    const sink = new RecordingSink();
    const mounted = driver.mount(
      {
        phase: "warm-up",
        n: 0,
        serverUrl: "ws://a/bridge",
        arrival: config("cockpit-descent").arrival,
      },
      sink,
    );
    current(stages).listeners.onFailed?.("SCENE TIMED OUT: relaunch to retry");
    await expect(mounted).rejects.toThrow("SCENE TIMED OUT");
    expect(sink.failures).toEqual([{ reason: "SCENE TIMED OUT: relaunch to retry", shown: null }]);
  });
});

/** Draws one arrival frame at `rafMs` on `stage`, `streaming` or not, through `drawnOn`. */
function drawAt(
  stage: DescentStage,
  rafMs: number,
  drawnOn: SpikeDrawnSession,
  streaming = false,
): void {
  stage.listeners.onFrame?.({ ...sample(0), rafTimestampMs: rafMs, streaming });
  stage.perf.onDrawn?.({ session: drawnOn, sizes: SIZES });
}

/** `arrival-memory`'s repetition mounted, its descent measured, and the stage it shows. */
function arrivalRepetition(sink: RecordingSink): { readonly stage: DescentStage } {
  const { driver, stages } = driven("arrival-memory");
  void driver
    .mount(
      {
        phase: "repetition",
        n: 1,
        serverUrl: "ws://b/bridge",
        arrival: config("arrival-memory").arrival,
      },
      sink,
    )
    .catch(() => undefined);
  const stage = current(stages);
  stage.listeners.onPrepared?.(PREPARED);
  return { stage };
}

describe("arrival-memory's driver", () => {
  const DEEPEST_MS = PROFILE.durationS * 1000;

  it("describes a span it measures itself, with no script segments", async () => {
    const { driver, stages } = driven("arrival-memory");
    const mounted = driver.mount(
      {
        phase: "warm-up",
        n: 0,
        serverUrl: "ws://a/bridge",
        arrival: config("arrival-memory").arrival,
      },
      new RecordingSink(),
    );
    const stage = current(stages);
    expect(stage.perf.scriptNowMs?.(5_000)).toBe(0);
    stage.listeners.onPrepared?.(PREPARED);
    draw(stage, 0, session(0));
    const drawn = await mounted;
    expect([drawn.span, drawn.heightWorkers]).toEqual([{ kind: "measured" }, 3]);
  });

  it("holds the first pose until the field is whole in every worker, then the deepest", () => {
    const { stage } = arrivalRepetition(new RecordingSink());
    expect(stage.perf.scriptNowMs?.(1_000)).toBe(0);
    drawAt(stage, 1_000, session(4, [15_100_000, 7_000_000]));
    expect(stage.perf.scriptNowMs?.(1_016)).toBe(0);
    drawAt(stage, 1_016, session(4, [15_100_000, 15_100_000]));
    expect(stage.perf.scriptNowMs?.(1_033)).toBe(DEEPEST_MS);
  });

  it("times its frames from the span's first", () => {
    const sink = new RecordingSink();
    const { stage } = arrivalRepetition(sink);
    drawAt(stage, 5_000, session(0, [0]));
    drawAt(stage, 5_500, session(0, [0]));
    expect(sink.frames.map(({ scriptTimeS }) => scriptTimeS)).toEqual([0, 0.5]);
  });

  it("ends the span a minute after the deepest pose's patches drew without streaming", () => {
    const sink = new RecordingSink();
    const { stage } = arrivalRepetition(sink);
    drawAt(stage, 0, session(4));
    drawAt(stage, 1_000, session(9), true);
    const settledMs = 2_000 + ARRIVAL_SETTLE_S * 1000;
    for (let ms = 2_000; ms <= settledMs + 60_000; ms += 1_000) {
      drawAt(stage, ms, session(30));
    }
    expect(sink.spans).toEqual([
      {
        spanS: (settledMs + 60_000) / 1000,
        segments: [
          { name: "arrival", startS: 0, endS: 0 },
          { name: "deepest", startS: 0, endS: settledMs / 1000 },
          { name: "held", startS: settledMs / 1000, endS: (settledMs + 60_000) / 1000 },
        ],
      },
    ]);
  });

  it("starts the held minute only once no patch has been made resident for the settle window", () => {
    const sink = new RecordingSink();
    const { stage } = arrivalRepetition(sink);
    drawAt(stage, 0, session(4));
    for (let ms = 1_000; ms <= 4_000; ms += 1_000) {
      drawAt(stage, ms, session(30));
      stage.listeners.onPatch?.("resident", { face: 0, level: 9, i: 3, j: 4 });
    }
    const quietMs = 4_000 + ARRIVAL_SETTLE_S * 1000;
    for (let ms = 5_000; ms <= quietMs + 60_000; ms += 1_000) {
      drawAt(stage, ms, session(30));
    }
    expect(sink.spans.map(({ segments }) => segments.at(-1))).toEqual([
      { name: "held", startS: quietMs / 1000, endS: (quietMs + 60_000) / 1000 },
    ]);
  });

  it("fails the run on a field the server refused, which would never arrive", () => {
    const sink = new RecordingSink();
    const { stage } = arrivalRepetition(sink);
    const refused = survey(false, {
      kind: "rejected",
      code: "bad_request",
      message: "no survey",
      refusal: true,
    });
    drawAt(stage, 0, session(0, [0], refused));
    expect(sink.failures).toEqual([
      {
        reason:
          "arrival-memory: the world's coarse field was not received (refused, bad_request: no survey)",
        shown: null,
      },
    ]);
  });
});

describe("the landing site against the sea", () => {
  it("states the site's ground against sea level, under the sea where it is below", () => {
    expect(siteSeaNote(-3873.1, { sea_level_m: -1372.4, main_liquid: 2 })).toEqual({
      note: "The landing site's ground stands -3873.1 m above the datum, 2500.7 m below sea level -1372.4 m: under the sea.",
      belowSea: true,
    });
    expect(siteSeaNote(120, { sea_level_m: -1372.4, main_liquid: 2 })).toEqual({
      note: "The landing site's ground stands 120.0 m above the datum, 1492.4 m above sea level -1372.4 m.",
      belowSea: false,
    });
  });

  it("states a world with no sea, or no field, as no sea level", () => {
    expect(siteSeaNote(5, { sea_level_m: -40, main_liquid: null }).note).toBe(
      "The landing site's ground stands 5.0 m above the datum; the world has no sea.",
    );
    expect(siteSeaNote(5, null).belowSea).toBe(false);
  });
});

describe("a field resident in the height workers", () => {
  it("needs the field's answer held, every block reported and the same copy in every worker", () => {
    expect(fieldResident(session(0, [10, 10]), PERF_PINS.world)).toBe(true);
    const unreported = { ...session(0, [10, 10]), levels: () => null };
    expect(fieldResident(unreported, PERF_PINS.world)).toBe(false);
    expect(fieldResident(session(0, [10, 9]), PERF_PINS.world)).toBe(false);
    expect(fieldResident(session(0, [10, 10], survey(false)), PERF_PINS.world)).toBe(false);
    expect(fieldResident(session(0, [0, 0]), PERF_PINS.world)).toBe(false);
    expect(fieldResident(null, PERF_PINS.world)).toBe(false);
  });
});

describe("a graphics fault's failure", () => {
  it("keeps its kind and detail for the record and shows the guide's words alone", () => {
    expect(
      faultFailure({ kind: "device-lost", reason: "unknown", message: "the driver reset" }),
    ).toEqual({
      reason: "GraphicsFault: the device was lost (unknown: the driver reset)",
      shown: "the graphics device was lost, relaunch to retry",
    });
  });
});

describe("the descent's measurement", () => {
  it("is made once for a world and a seed and reused, a failed one not kept", async () => {
    const asked: string[] = [];
    let fails = true;
    const once = measuredOnce((_session: null, world: { readonly body: string }) => {
      asked.push(world.body);
      if (fails) {
        fails = false;
        return Promise.reject(new Error("the field was not held"));
      }
      return Promise.resolve(PREPARED);
    });
    const signal = new AbortController().signal;
    await expect(once(null, { body: PERF_PINS.world }, 7n, signal)).rejects.toThrow("not held");
    expect(await once(null, { body: PERF_PINS.world }, 7n, signal)).toBe(PREPARED);
    expect(await once(null, { body: PERF_PINS.world }, 7n, signal)).toBe(PREPARED);
    expect(asked).toEqual([PERF_PINS.world, PERF_PINS.world]);
  });
});

describe("the kept engine", () => {
  it("does nothing at a mount's dispose, its other members the engine's own", async () => {
    const engine = await countingRenderEngine();
    const kept = undisposable(engine);
    kept.dispose();
    expect(kept.capabilities).toBe(engine.capabilities);
    expect(kept.passTimesFrame).toBe(engine.passTimesFrame);
    expect(capabilityFlags(engine.capabilities)).toMatchObject({
      timestampQuery: expect.any(Boolean),
    });
    engine.dispose();
  });
});
