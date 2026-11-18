import { PERF_PINS, type PerfSceneId } from "@hyperion/perf";
import { describe, expect, it } from "vitest";

import type { PerfConfig } from "../../../../preload/api";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable } from "../../test/terrainFixtures";
import type { MeasureDescent } from "../spike/bodyDescent";
import type { SpikeDrawnFrame, SpikeDrawnSession } from "../spike/DescentSpike";
import { DescentProfile, landingSiteOf, trackStretches } from "../spike/descentProfile";
import { contactRule, TEST_PLANET_FIGURE } from "../spike/spikeScene";
import type { PreparedDescent, SpikeFrameSample } from "../spike/spikeRun";
import { planetGeometry } from "../terrain/planet";
import {
  capabilityFlags,
  DescentDriver,
  faultFailure,
  descentDriverFor,
  type DescentStage,
  DescentStages,
  measuredOnce,
  PerfEngine,
  undisposable,
} from "./descentDriver";
import type { PerfFailure, PerfFrameSample, PerfSceneSink } from "./runPerfScene";
import { PERF_SCENES } from "./scenes";

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
  frame(drawn: PerfFrameSample): void {
    this.frames.push(drawn);
  }
  patch(): void {}
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

/** A session whose lit pass holds `slots` patches. */
function session(slots: number): SpikeDrawnSession {
  return {
    workerMemory: [{ fieldBytes: 15_100_000, wasmBytes: 40_000_000 }],
    pass: () => ({ usedSlots: slots }),
    wirePass: () => null,
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
      seed: 7n,
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
    expect(drawn.spanS).toBe(1230);
    expect(drawn.segments).toEqual(PROFILE.segmentSpans());
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

describe("the driver for a scene", () => {
  const page = {
    engine: new PerfEngine(undefined),
    stages: new DescentStages(),
    launchMode: "vulkan" as const,
    webGpu: true,
    defaultWorkers: () => 5,
  };

  it("is the descent's for both descents", () => {
    expect(descentDriverFor(config("cockpit-descent"), page).kind).toBe("driver");
    expect(descentDriverFor(config("station-wireframe-descent"), page).kind).toBe("driver");
  });

  it("is none yet for the scenes that fly no descent, saying so", () => {
    for (const scene of ["ringed-giant-approach", "arrival-memory", "nuclear-disc-sky"] as const) {
      expect(descentDriverFor(config(scene), page)).toEqual({
        kind: "refused",
        reason: `${scene}: no renderer driver yet (R12.T4.c)`,
        shown: "this build draws no view of this scene",
      });
    }
  });

  it("is none in the graphics safe mode or without WebGPU", () => {
    expect(descentDriverFor(config("cockpit-descent"), { ...page, launchMode: "safe" })).toEqual({
      kind: "refused",
      reason: "the graphics safe mode draws no WebGPU view",
      shown: "the graphics safe mode draws no view, relaunch to retry",
    });
    expect(descentDriverFor(config("cockpit-descent"), { ...page, webGpu: false })).toEqual({
      kind: "refused",
      reason: "this renderer has no WebGPU",
      shown: "no WebGPU",
    });
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
