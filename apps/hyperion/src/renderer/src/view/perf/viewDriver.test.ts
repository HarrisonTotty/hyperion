import { PERF_PINS, type PerfSceneId } from "@hyperion/perf";
import { describe, expect, it } from "vitest";

import type { ViewPerfFrame } from "../../displays/view/viewPerf";
import { startServerRun, type ViewRun } from "../../displays/view/viewRun";
import { vec3 } from "../../geometry/vec3";
import { aViewRing } from "../../test/ringFixtures";
import { aBody, aViewScene } from "../../test/viewFixtures";
import { bodyKindSymbol, type ViewScene } from "../scene/model";
import { PerfEngine } from "./descentDriver";
import type {
  MeasuredSpan,
  PerfFailure,
  PerfFrameSample,
  PerfSceneSink,
  PerfStage,
} from "./runPerfScene";
import { PERF_SCENES } from "./scenes";
import { barycentrePlacement, placedPose } from "./scriptedCamera";
import { nuclearDiscSweep, SWEEP_DURATION_S } from "./scripts";
import { type ViewDrawnSession, ViewDriver, type ViewStage, ViewStages } from "./viewDriver";

const AU_M = 149_597_870_700;

/** A sink that records what it is told. */
class RecordingSink implements PerfSceneSink {
  readonly frames: PerfFrameSample[] = [];
  readonly failures: PerfFailure[] = [];
  /** Each `leaveOutResolves`' engine frame. */
  readonly leftOut: number[] = [];
  frame(drawn: PerfFrameSample): void {
    this.frames.push(drawn);
  }
  leaveOutResolves(engineFrame: number): void {
    this.leftOut.push(engineFrame);
  }
  patch(): void {}
  spanEnded(_span: MeasuredSpan): void {}
  passTimes(): void {}
  allocation(): void {}
  failed(failure: PerfFailure): void {
    this.failures.push(failure);
  }
}

/** The pinned system's served scene: its star, and its giant with one ring unless given none. */
function servedScene(system: string = PERF_PINS.system, ringed = true): ViewScene {
  return aViewScene({
    system,
    bodies: [
      aBody({
        id: `${PERF_PINS.system}.0000`,
        kind: "star",
        radiusM: 7e8,
        hillRadiusM: null,
        centreM: vec3(0, 0, 0),
        symbol: bodyKindSymbol("star"),
      }),
      aBody({ id: PERF_PINS.giant, radiusM: 7.5e7, centreM: vec3(4 * AU_M, AU_M, 0) }),
    ],
    rings: ringed ? [aViewRing(PERF_PINS.giant, 8.2e7, 1.3e8, vec3(0, 0, 1))] : [],
  });
}

const STAGE: PerfStage = {
  phase: "repetition",
  n: 2,
  serverUrl: "ws://b/bridge",
  arrival: { universe: "0000000000000003", system: PERF_PINS.system, body: PERF_PINS.giant },
};

/** A frame `VIEW` drew at `rafMs`, its scale `renderScale`. */
function frameAt(rafMs: number, renderScale = 1): ViewPerfFrame {
  return {
    rafTimestampMs: rafMs,
    callbackStartMs: rafMs + 0.5,
    callbackMs: 6,
    size: { widthPx: 1248, heightPx: 610 },
    drawnStyle: "photorealistic",
    photoreal: "ready",
    renderScale,
    streaming: false,
  };
}

interface Driven {
  readonly driver: ViewDriver;
  readonly stages: ViewStages;
  readonly sink: RecordingSink;
}

function driven(
  scene: PerfSceneId,
  stage: PerfStage = STAGE,
): Driven & { mounted: Promise<unknown> } {
  const stages = new ViewStages();
  const sink = new RecordingSink();
  const driver = new ViewDriver({
    scene: PERF_SCENES[scene],
    setting: "low",
    engine: new PerfEngine(undefined),
    stages,
  });
  const mounted = driver.mount(stage, sink);
  mounted.catch(() => undefined);
  return { driver, stages, sink, mounted };
}

function current(stages: ViewStages): ViewStage {
  const stage = stages.current();
  if (stage === null) {
    throw new Error("no stage is mounted");
  }
  return stage;
}

/** Draws one frame of `run` on `stage` at `rafMs`, as `VIEW`'s loop does. */
function draw(stage: ViewStage, run: ViewRun, rafMs: number, renderScale = 1): void {
  const camera = stage.perf.cameraAt(run, rafMs);
  if (camera !== null) {
    stage.perf.onDrawn(frameAt(rafMs, renderScale));
  }
}

describe("VIEW's scripted camera driver", () => {
  it("mounts VIEW on the arrival's server and universe under the repetition's key", () => {
    const { stages } = driven("nuclear-disc-sky");
    expect(current(stages)).toMatchObject({
      key: "repetition-2",
      serverUrl: "ws://b/bridge",
      universe: "0000000000000003",
      setting: "low",
    });
  });

  it("holds the warm-up at the script's first pose, in the scene's style", () => {
    const { stages } = driven("nuclear-disc-sky", { ...STAGE, phase: "warm-up", n: 0 });
    const run = startServerRun(servedScene(PERF_PINS.nuclearSystem));
    const stage = current(stages);
    const first = stage.perf.cameraAt(run, 1_000);
    expect(first).toEqual({
      pose: placedPose(nuclearDiscSweep().poseAt(0), barycentrePlacement(PERF_PINS.nuclearSystem)),
      style: "photorealistic",
      fovDeg: 60,
    });
    expect(stage.perf.cameraAt(run, 61_000)).toEqual(first);
  });

  it("flies a repetition from its first scripted frame on the frames' clock", () => {
    const { stages, sink } = driven("nuclear-disc-sky");
    const run = startServerRun(servedScene(PERF_PINS.nuclearSystem));
    const stage = current(stages);
    draw(stage, run, 4_000);
    expect(stage.perf.cameraAt(run, 6_500)?.pose).toEqual(
      placedPose(nuclearDiscSweep().poseAt(2.5), barycentrePlacement(PERF_PINS.nuclearSystem)),
    );
    stage.perf.onDrawn(frameAt(6_500));
    expect(
      sink.frames.map(({ scriptTimeS, rafTimestampMs }) => [scriptTimeS, rafTimestampMs]),
    ).toEqual([
      [0, 4_000],
      [2.5, 6_500],
    ]);
  });

  it("resolves its mount at the first frame with the view as drawn and the script's span", async () => {
    const { stages, mounted } = driven("nuclear-disc-sky");
    draw(current(stages), startServerRun(servedScene(PERF_PINS.nuclearSystem)), 1_000);
    expect(await mounted).toMatchObject({
      views: [
        {
          role: "primary",
          style: "photorealistic",
          widthPx: 1248,
          heightPx: 610,
          rateHz: 30,
          lineAntialiasing: "none",
        },
      ],
      span: {
        kind: "script",
        spanS: SWEEP_DURATION_S,
        segments: [{ name: "sweep", startS: 0, endS: SWEEP_DURATION_S }],
      },
    });
  });

  it("leaves the wireframe stand-in's frames out, its span's clock starting at the first in its style", () => {
    const { stages, sink } = driven("nuclear-disc-sky");
    const run = startServerRun(servedScene(PERF_PINS.nuclearSystem));
    const stage = current(stages);
    stage.perf.cameraAt(run, 1_000);
    stage.perf.onDrawn({ ...frameAt(1_000), drawnStyle: "wireframe", photoreal: "making" });
    expect(stage.perf.cameraAt(run, 9_000)?.pose).toEqual(
      placedPose(nuclearDiscSweep().poseAt(0), barycentrePlacement(PERF_PINS.nuclearSystem)),
    );
    stage.perf.onDrawn(frameAt(9_000));
    draw(stage, run, 10_000);
    expect(sink.frames.map(({ scriptTimeS }) => scriptTimeS)).toEqual([0, 1]);
    // The stand-in's resolves are left out once, as the first frame in the scene's style begins.
    expect(sink.leftOut).toEqual([0]);
  });

  it("fails a run whose photorealistic pipelines could not be made", async () => {
    const { stages, sink, mounted } = driven("nuclear-disc-sky");
    const stage = current(stages);
    stage.perf.cameraAt(startServerRun(servedScene(PERF_PINS.nuclearSystem)), 1_000);
    stage.perf.onDrawn({ ...frameAt(1_000), drawnStyle: "wireframe", photoreal: "failed" });
    await expect(mounted).rejects.toThrow("pipelines could not be made");
    expect([sink.frames, sink.failures[0]?.shown]).toEqual([
      [],
      "the photorealistic style's pipelines could not be made on this device",
    ]);
  });

  it("places the approach about the giant's ring once its scene is drawn", () => {
    const { stages } = driven("ringed-giant-approach");
    const camera = current(stages).perf.cameraAt(startServerRun(servedScene()), 1_000);
    expect(camera?.pose.frame).toEqual({ kind: "body", body: PERF_PINS.giant });
  });

  it("fails a run whose served scene is not the pinned system's", async () => {
    const { stages, sink, mounted } = driven("ringed-giant-approach");
    const camera = current(stages).perf.cameraAt(
      startServerRun(servedScene("4200000000000001")),
      1,
    );
    expect(camera).toBeNull();
    await expect(mounted).rejects.toThrow("not the pinned");
    expect(sink.failures[0]?.shown).toBe(
      "the served scene is of another system than the scene's pin, re-pin the scene",
    );
  });

  it("fails an approach whose giant the scene gives no ring", async () => {
    const { stages, sink, mounted } = driven("ringed-giant-approach");
    current(stages).perf.cameraAt(startServerRun(servedScene(PERF_PINS.system, false)), 1);
    await expect(mounted).rejects.toThrow("no ring");
    expect(sink.failures).toEqual([
      {
        reason: `ringed-giant-approach: the served scene gives ${PERF_PINS.giant} no ring`,
        shown: "the served scene gives the pinned giant no ring, re-pin the scene",
      },
    ]);
  });

  it("reads the patch cache and the workers of the terrain session VIEW made", () => {
    const { driver, stages } = driven("ringed-giant-approach");
    const session: ViewDrawnSession = {
      passBodies: new Set([PERF_PINS.world]),
      wirePassBodies: new Set([PERF_PINS.world]),
      pass: () => ({ usedSlots: 7 }),
      wirePass: () => ({ usedSlots: 2 }),
      workerMemory: [{ fieldBytes: 0, wasmBytes: 30_000_000 }],
      workerCount: 4,
    };
    expect(driver.cachedPatches()).toBe(0);
    const unwatch = current(stages).watchTerrain(session);
    expect([driver.cachedPatches(), driver.workerMemory()]).toEqual([
      9,
      [{ fieldBytes: 0, wasmBytes: 30_000_000 }],
    ]);
    unwatch();
    expect(driver.cachedPatches()).toBe(0);
  });

  it("states the primary's internal scale over the span: its least and its median", () => {
    const { driver, stages } = driven("nuclear-disc-sky");
    const stage = current(stages);
    const run = startServerRun(servedScene(PERF_PINS.nuclearSystem));
    [1, 0.75, 0.9, 1, 1].forEach((scale, i) => {
      draw(stage, run, 1_000 + i * 33, scale);
    });
    expect(driver.internalScale([])).toEqual([]);
    const [scale] = driver.internalScale([
      {
        role: "primary",
        style: "photorealistic",
        widthPx: 1,
        heightPx: 1,
        rateHz: 30,
        lineAntialiasing: "none",
        exposure: null,
      },
    ]);
    expect(scale).toEqual({ min: 0.75, median: 1 });
  });

  it("tells nothing of a stage it has left", () => {
    const { driver, stages, sink } = driven("nuclear-disc-sky");
    const left = current(stages);
    driver.unmount();
    expect(stages.current()).toBeNull();
    expect(left.perf.cameraAt(startServerRun(servedScene(PERF_PINS.nuclearSystem)), 1)).toBeNull();
    left.onFailed({ reason: "refused", shown: null });
    expect([sink.frames, sink.failures]).toEqual([[], []]);
  });
});
