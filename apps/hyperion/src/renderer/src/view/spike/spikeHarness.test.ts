import { afterEach, describe, expect, it, vi } from "vitest";

import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { fakeEngineModule, type FakeRenderEngine } from "../../test/fakeRenderEngine";
import { goldenLevelTable } from "../../test/terrainFixtures";
import { loadRenderEngine } from "../engine/loadEngine";
import { requestAdapterOutcome } from "../engine/platform";
import { GraphicsStatusStore, initialGraphicsStatus } from "../engine/status";
import { planetGeometry } from "../terrain/planet";
import type { SelectionInput } from "../terrain/select";
import { selectionTolerancePx } from "../terrain/selectionTolerance";
import { boundedPlanet, type DemandView, levelRatio } from "./demand";
import { recordProfile, SETTING_VIEWS } from "./demandRecord";
import { PassReads } from "./passReads";
import { PipelineTally, wrapGpu } from "./pipelineShim";
import {
  meanDemand,
  ResolveCounter,
  rowOf,
  SPIKE_PASS_ROWS,
  SPIKE_WARMUP_S,
  spikeGpu,
  SpikeRecorder,
} from "./spikeHarness";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

const PLANET = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off"));

/** σ_n large enough that min(hard, 4σ_n) is the hard bound, but for a few levels. */
const SIGMA = Array.from({ length: 25 }, (_, level) => (level < 10 ? 1 : 1e9));

async function device(gpu: GPU): Promise<GPUDevice> {
  const adapter = await gpu.requestAdapter();
  if (adapter === null) {
    throw new Error("no adapter");
  }
  return adapter.requestDevice();
}

function frame(scriptTimeS: number): {
  scriptTimeS: number;
  rafTimestampMs: number;
  callbackStartMs: number;
  callbackMs: number;
  passesSubmitted: number;
  patchesHard: number;
  streaming: boolean;
} {
  return {
    scriptTimeS,
    rafTimestampMs: 1000 * scriptTimeS,
    callbackStartMs: 1000 * scriptTimeS + 0.1,
    callbackMs: 2,
    passesSubmitted: 5,
    patchesHard: 300,
    streaming: false,
  };
}

describe("the spike's pass rows", () => {
  it("puts the terrain's, the atmosphere's and the rest in their rows", () => {
    for (const [label, row] of Object.entries(SPIKE_PASS_ROWS)) {
      expect(rowOf(label)).toBe(row);
    }
    expect(rowOf("atmosphere tables")).toBe("atmosphere");
    expect(rowOf("terrain normals")).toBe("terrain");
  });
});

/** Makes one encoder on `dev` and resolves a query set with it, as R01's timer does a frame. */
function resolveOn(dev: GPUDevice): void {
  dev
    .createCommandEncoder()
    .resolveQuerySet(
      dev.createQuerySet({ type: "timestamp", count: 2 }),
      0,
      2,
      dev.createBuffer({ size: 16, usage: 0 }),
      0,
    );
}

/** The `index`th engine `module` made. */
function madeEngine(module: ReturnType<typeof fakeEngineModule>, index: number): FakeRenderEngine {
  const engine = module.engines.at(index);
  if (engine === undefined) {
    throw new Error(`no engine ${index} was made`);
  }
  return engine;
}

describe("the resolve counter", () => {
  it("takes the engine's count, a dropped resolve's number included", async () => {
    const counter = new ResolveCounter();
    const gpu = new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] })]);
    counter.deviceMade();
    const dev = await device(gpu);
    const engine = { passTimesFrame: 0 };
    counter.follow(engine);
    // Two resolves reach the device; the engine's timer numbered three, the second dropped.
    resolveOn(dev);
    resolveOn(dev);
    engine.passTimesFrame = 3;
    expect(counter.value).toBe(3);
    expect(counter.runFrame(3)).toBe(3);
  });

  it("numbers a rebuilt engine's resolves after the lost one's", async () => {
    const counter = new ResolveCounter();
    const gpu = wrapGpu(
      new FakeGpu([
        new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }),
        new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }),
      ]),
      (dev) => {
        counter.deviceMade();
        return dev;
      },
    );
    const outcome = await requestAdapterOutcome(gpu);
    if (outcome.kind !== "adapter") {
      throw new Error(`no adapter: ${outcome.kind}`);
    }
    const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    status.dispatch({ kind: "adapter-outcome", outcome });
    const module = fakeEngineModule();
    const engine = await loadRenderEngine(outcome, status, {
      importEngine: () => Promise.resolve(module),
      gpu,
    });
    counter.follow(engine);
    const restored = vi.fn<() => void>();
    engine.onRestored(restored);
    madeEngine(module, 0).passTimesFrame = 2;
    // A frame's read, which records the lost engine's count before the loss releases it.
    expect(counter.value).toBe(2);
    madeEngine(module, 0).loseDevice();
    await vi.waitFor(() => {
      expect(restored).toHaveBeenCalledOnce();
    });
    // The restored engine counts from 0 again, so its first resolve is the run's third.
    expect(counter.value).toBe(2);
    madeEngine(module, 1).passTimesFrame = 1;
    expect(counter.value).toBe(3);
    expect(counter.runFrame(1)).toBe(3);
  });
});

describe("the spike's engine source", () => {
  it("shims the device it gives for the pipeline tally", async () => {
    const fake = new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] })]);
    const measured = spikeGpu(fake, null, () => 12);
    const outcome = await measured.source.requestAdapter();
    if (outcome.kind !== "adapter") {
      throw new Error(`no adapter: ${outcome.kind}`);
    }
    const dev = await outcome.adapter.requestDevice();
    measured.tally.endWarmup();
    dev.createRenderPipeline({
      label: "late",
      layout: "auto",
      vertex: { module: dev.createShaderModule({ code: "" }) },
    });
    expect(measured.tally.late().map(({ label, scriptTimeS }) => [label, scriptTimeS])).toEqual([
      ["late", 12],
    ]);
  });

  it("numbers the resolves by the engine it loads", async () => {
    const fake = new FakeGpu([
      new FakeAdapter({ info: INTEL_UHD_620_INFO, features: ["timestamp-query"] }),
    ]);
    const measured = spikeGpu(fake, null, () => 0);
    const outcome = await measured.source.requestAdapter();
    if (outcome.kind !== "adapter") {
      throw new Error(`no adapter: ${outcome.kind}`);
    }
    const engine = await measured.source.load(
      outcome,
      new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    );
    engine
      .createRenderTarget({
        name: "timed",
        size: { widthPx: 4, heightPx: 4 },
        format: "rgba16float",
        mips: 1,
        depth: false,
        category: "render-targets",
      })
      .render({
        label: "timed",
        viewRotation: new Float32Array(16),
        projection: new Float32Array(16),
        draws: [],
        postProcesses: [],
      });
    expect([engine.passTimesFrame, measured.resolves.value]).toEqual([1, 1]);
    // The timer's staging buffer began its read, numbered by the engine's count.
    expect(measured.reads.inFlight()).toEqual([1]);
    engine.dispose();
  });
});

describe("the spike's recorder", () => {
  const descent = { planet: PLANET, profile: recordProfile(), omittedSigmaM: SIGMA };

  afterEach(() => {
    vi.useRealTimers();
  });

  it("gives each frame the engine frames resolved since the one before", () => {
    // The counter stands at 0 when the run starts; each frame resolves three engine frames.
    const counted = { value: 0, runFrame: (n: number) => n };
    const recorder = new SpikeRecorder(descent, "low", {
      resolves: counted,
      reads: new PassReads(() => counted.value),
      tally: new PipelineTally(() => 0),
    });
    counted.value = 3;
    recorder.frame(frame(0));
    counted.value = 6;
    recorder.frame(frame(0.016));
    for (const [n, label] of [
      [1, "terrain"],
      [2, "view:wireframe"],
      [3, "view:wireframe"],
      [5, "terrain"],
    ] as const) {
      recorder.passTimes({
        frame: n,
        timer: "full",
        passes: [{ label, ns: 1e6, bracketed: false }],
      });
    }
    const report = recorder.report({ widthPx: 1280, heightPx: 720 });
    expect(report.frames.passes).toEqual([
      { label: "terrain", row: "terrain", gpuMs: [1, 1] },
      { label: "view:wireframe", row: "other", gpuMs: [2, null] },
    ]);
    // The second frame's resolves 4 and 6 never reported.
    expect(report.frames.missingResolves).toEqual([0, 2]);
    expect(report.warmupS).toBe(SPIKE_WARMUP_S);
    expect(report.levels).toHaveLength(PLANET.finestLevel + 1);
    expect(report.streaming.map(({ segment }) => segment)).toEqual(
      recordProfile()
        .segmentSpans()
        .map(({ name }) => name),
    );
  });

  it("gives the frames after a dropped resolve their own pass times", async () => {
    const resolves = new ResolveCounter();
    resolves.deviceMade();
    const dev = await device(
      new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] })]),
    );
    const engine = { passTimesFrame: 0 };
    resolves.follow(engine);
    const recorder = new SpikeRecorder(descent, "low", {
      resolves,
      reads: new PassReads(() => resolves.value),
      tally: new PipelineTally(() => 0),
    });
    resolveOn(dev);
    engine.passTimesFrame = 1;
    recorder.frame(frame(0));
    // Every read-back buffer is in flight: the timer drops resolve 2, which reaches no device.
    engine.passTimesFrame = 2;
    recorder.frame(frame(0.016));
    resolveOn(dev);
    engine.passTimesFrame = 3;
    recorder.frame(frame(0.033));
    for (const n of [1, 3]) {
      recorder.passTimes({
        frame: n,
        timer: "full",
        passes: [{ label: "terrain", ns: n * 1e6, bracketed: false }],
      });
    }
    const { frames } = recorder.report({ widthPx: 1280, heightPx: 720 });
    expect(frames.passes).toEqual([{ label: "terrain", row: "terrain", gpuMs: [1, null, 3] }]);
    // The dropped resolve is its frame's one missing resolve.
    expect(frames.missingResolves).toEqual([0, 1, 0]);
  });

  it("ends a read with its report, and reports its frames' reads still in flight", () => {
    const counted = { value: 0, runFrame: (n: number) => n };
    const reads = new PassReads(() => counted.value);
    const recorder = new SpikeRecorder(descent, "low", {
      resolves: counted,
      reads,
      tally: new PipelineTally(() => 0),
    });
    // Two frames of two resolves each, every resolve read.
    for (const [i, last] of [2, 4].entries()) {
      for (let n = last - 1; n <= last; n += 1) {
        counted.value = n;
        reads.began();
      }
      recorder.frame(frame(i * 0.016));
    }
    for (const n of [1, 2, 3]) {
      recorder.passTimes({
        frame: n,
        timer: "full",
        passes: [{ label: "terrain", ns: 1e6, bracketed: false }],
      });
    }
    expect(reads.inFlight()).toEqual([4]);
    const { frames } = recorder.report({ widthPx: 1280, heightPx: 720 });
    expect(frames.missingResolves).toEqual([0, 1]);
    expect(frames.inFlightResolves).toEqual([0, 1]);
  });

  it("waits only for the reads of the frames it recorded", async () => {
    vi.useFakeTimers();
    const counted = { value: 1, runFrame: (n: number) => n };
    const reads = new PassReads(() => counted.value);
    // Resolve 1's read began before the run; resolve 3's after its last frame.
    reads.began();
    const recorder = new SpikeRecorder(descent, "low", {
      resolves: counted,
      reads,
      tally: new PipelineTally(() => 0),
    });
    counted.value = 2;
    reads.began();
    recorder.frame(frame(0));
    counted.value = 3;
    reads.began();
    const outcome: { settled: boolean | null } = { settled: null };
    void recorder.readsSettled(1000).then((settled) => {
      outcome.settled = settled;
      return settled;
    });
    await vi.advanceTimersByTimeAsync(10);
    expect(outcome.settled).toBeNull();
    recorder.passTimes({ frame: 2, timer: "full", passes: [] });
    await vi.advanceTimersByTimeAsync(0);
    expect(outcome.settled).toBe(true);
    expect(reads.inFlight()).toEqual([1, 3]);
  });

  it("counts a frame missing one of its five resolves, and one missing all five", () => {
    const counted = { value: 0, runFrame: (n: number) => n };
    const recorder = new SpikeRecorder(descent, "low", {
      resolves: counted,
      reads: new PassReads(() => counted.value),
      tally: new PipelineTally(() => 0),
    });
    for (const [i, last] of [5, 10, 15].entries()) {
      counted.value = last;
      recorder.frame(frame(i * 0.016));
    }
    for (const n of [1, 2, 3, 4, 5, 6, 8, 9, 10]) {
      recorder.passTimes({
        frame: n,
        timer: "full",
        passes: [{ label: "terrain", ns: 1e6, bracketed: false }],
      });
    }
    expect(recorder.report({ widthPx: 1280, heightPx: 720 }).frames.missingResolves).toEqual([
      0, 1, 5,
    ]);
  });

  /** The high setting's report, and its view at the terrain pass's τ_sel. */
  const highAtSelection = (): {
    report: ReturnType<SpikeRecorder["report"]>;
    atSelection: DemandView;
  } => {
    const view = SETTING_VIEWS[0]?.view;
    if (view === undefined) {
      throw new Error("no high setting");
    }
    const report = new SpikeRecorder(descent, "high", {
      resolves: new ResolveCounter(),
      reads: new PassReads(() => 0),
      tally: new PipelineTally(() => 0),
    }).report({ widthPx: 1, heightPx: 1 });
    return { report, atSelection: { ...view, tauPx: selectionTolerancePx(view.tauPx) } };
  };

  it("predicts each segment's D at the terrain pass's τ_sel", () => {
    const { report, atSelection } = highAtSelection();
    const profile = recordProfile();
    expect(report.streaming.map(({ predictedHardPerS }) => predictedHardPerS)).toEqual(
      profile.segmentSpans().map((span) => meanDemand(PLANET, profile, span, atSelection)),
    );
  });

  it("predicts each segment's D under min(hard, 4σ_n) at the terrain pass's τ_sel", () => {
    const { report, atSelection } = highAtSelection();
    const profile = recordProfile();
    const calibrated = boundedPlanet(PLANET, "calibrated", SIGMA);
    expect(report.streaming.map(({ predictedCalibratedPerS }) => predictedCalibratedPerS)).toEqual(
      profile.segmentSpans().map((span) => meanDemand(calibrated, profile, span, atSelection)),
    );
  });

  it("gives each level's k at the terrain pass's τ_sel", () => {
    const { report, atSelection } = highAtSelection();
    expect(report.levels.map(({ k }) => k)).toEqual(
      report.levels.map(({ level }) => levelRatio(PLANET, level, atSelection)),
    );
  });

  it("records the calibrated count of the same selection inputs", () => {
    const tally = new PipelineTally(() => 0);
    const recorder = new SpikeRecorder(descent, "low", {
      resolves: new ResolveCounter(),
      reads: new PassReads(() => 0),
      tally,
    });
    const view = SETTING_VIEWS[1]?.view;
    const pose = recordProfile().poseAt(1000);
    const input: SelectionInput = {
      planet: PLANET,
      views: [
        {
          camera: { positionM: pose.positionM, orientation: pose.orientation },
          fovXRad: view?.fovXRad ?? 1,
          viewport: { widthPx: 1280, heightPx: 720 },
          weight: 1,
          tauPx: view?.tauPx ?? 2,
        },
      ],
      setting: "low",
      grounded: [],
    };
    recorder.select(input);
    recorder.frame(frame(1000));
    expect(
      recorder
        .report({ widthPx: 1, heightPx: 1 })
        .streaming.find((s) => s.segment === "approach and flare")?.patchesCalibrated,
    ).toBeGreaterThan(0);
  });

  it("ends the pipeline warm-up at 10 s and tracks the adapter's live peak", () => {
    const tally = new PipelineTally(() => 0);
    const recorder = new SpikeRecorder(descent, "high", {
      resolves: new ResolveCounter(),
      reads: new PassReads(() => 0),
      tally,
    });
    recorder.allocation({ kind: "created", name: "a", bytes: 100, category: "other" });
    recorder.allocation({ kind: "created", name: "b", bytes: 50, category: "other" });
    recorder.allocation({ kind: "destroyed", name: "a", bytes: 100, category: "other" });
    recorder.allocation({ kind: "created", name: "c", bytes: 20, category: "other" });
    recorder.frame(frame(9.9));
    tally.record("render", false, "early");
    recorder.frame(frame(10));
    tally.record("render", false, "late");
    const report = recorder.report({ widthPx: 1, heightPx: 1 });
    expect(report.adapterPeakBytes).toBe(150);
    expect(report.latePipelines.map(({ label }) => label)).toEqual(["late"]);
  });
});
