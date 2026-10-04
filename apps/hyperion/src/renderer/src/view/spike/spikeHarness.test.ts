import { describe, expect, it } from "vitest";

import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { goldenLevelTable } from "../../test/terrainFixtures";
import { planetGeometry } from "../terrain/planet";
import type { SelectionInput } from "../terrain/select";
import { recordProfile, SETTING_VIEWS } from "./demandRecord";
import { PipelineTally } from "./pipelineShim";
import {
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
  callbackMs: number;
  passesSubmitted: number;
  patchesHard: number;
  streaming: boolean;
} {
  return {
    scriptTimeS,
    rafTimestampMs: 1000 * scriptTimeS,
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

describe("the resolve counter", () => {
  it("counts each resolve an encoder makes and passes it through", async () => {
    const counter = new ResolveCounter();
    const gpu = new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] })]);
    const dev = counter.wrap(await device(gpu));
    const querySet = dev.createQuerySet({ type: "timestamp", count: 2 });
    const buffer = dev.createBuffer({ size: 16, usage: 0 });
    const encoder = dev.createCommandEncoder();
    encoder.resolveQuerySet(querySet, 0, 2, buffer, 0);
    dev.createCommandEncoder().resolveQuerySet(querySet, 0, 2, buffer, 0);
    expect(counter.value).toBe(2);
  });

  it("numbers a rebuilt device's timer frames after the lost one's", async () => {
    const counter = new ResolveCounter();
    const gpu = new FakeGpu([
      new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }),
      new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }),
    ]);
    const first = counter.wrap(await device(gpu));
    resolveOn(first);
    resolveOn(first);
    expect(counter.runFrame(2)).toBe(2);
    // The rebuilt engine's new timer numbers its first frame 1 again: the run's third.
    const second = counter.wrap(await device(gpu));
    resolveOn(second);
    expect(counter.value).toBe(3);
    expect(counter.runFrame(1)).toBe(3);
  });
});

describe("the spike's engine source", () => {
  it("shims the device it gives for the pipeline tally and the resolve count", async () => {
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
    dev
      .createCommandEncoder()
      .resolveQuerySet(
        dev.createQuerySet({ type: "timestamp", count: 2 }),
        0,
        2,
        dev.createBuffer({ size: 16, usage: 0 }),
        0,
      );
    expect(measured.resolves.value).toBe(1);
  });
});

describe("the spike's recorder", () => {
  const descent = { planet: PLANET, profile: recordProfile(), omittedSigmaM: SIGMA };

  it("gives each frame the engine frames resolved since the one before", () => {
    // The counter stands at 0 when the run starts; each frame resolves three engine frames.
    const counted = { value: 0, runFrame: (n: number) => n };
    const recorder = new SpikeRecorder(descent, "low", {
      resolves: counted,
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
    expect(report.warmupS).toBe(SPIKE_WARMUP_S);
    expect(report.levels).toHaveLength(PLANET.finestLevel + 1);
    expect(report.streaming.map(({ segment }) => segment)).toEqual(
      recordProfile()
        .segmentSpans()
        .map(({ name }) => name),
    );
  });

  it("records the calibrated count of the same selection inputs", () => {
    const tally = new PipelineTally(() => 0);
    const recorder = new SpikeRecorder(descent, "low", { resolves: new ResolveCounter(), tally });
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
    const recorder = new SpikeRecorder(descent, "high", { resolves: new ResolveCounter(), tally });
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
