import { describe, expect, it } from "vitest";

import type { DescentSpikeReport, SpikeApi, SpikeLaunch } from "../../../../preload/api";
import { TEST_SPIKE_LAUNCH } from "../../test/stubHyperionApi";
import { goldenLevelTable } from "../../test/terrainFixtures";
import { viewId } from "../camera/state";
import { planetGeometry } from "../terrain/planet";
import { recordProfile } from "./demandRecord";
import { PassReads } from "./passReads";
import { PipelineTally } from "./pipelineShim";
import { SMOKE_S, SMOKE_TRACE_BOUNDARIES_S, SpikeController } from "./spikeController";
import { SpikeSizes } from "./spikeSizes";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

const DESCENT = {
  planet: planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off")),
  profile: recordProfile(),
  omittedSigmaM: Array.from({ length: 25 }, () => 1e9),
};

/** A smoke's fake preload, keeping the reports the run hands the main process. */
function fakeSpike(launch: SpikeLaunch): {
  readonly spike: SpikeApi;
  readonly reports: DescentSpikeReport[];
} {
  const reports: DescentSpikeReport[] = [];
  const spike: SpikeApi = {
    launch,
    startTrace: () => Promise.resolve(),
    cycleTrace: () => Promise.resolve(),
    stopTrace: () => Promise.resolve(),
    sampleMemory: () => Promise.resolve(),
    writeResults: (report) => {
      reports.push(report);
      return Promise.resolve({ kind: "smoke checked", failure: null });
    },
    writeCapture: () => Promise.resolve("/capture"),
    end: () => Promise.resolve(),
  };
  return { spike, reports };
}

/** Lets the controller's awaited calls run. */
function settle(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

/** Flies a smoke to its end on `controller`, a frame at its start, each boundary and its end. */
async function flySmoke(controller: SpikeController): Promise<void> {
  controller.prepared(DESCENT);
  await settle();
  for (const scriptTimeS of [0, ...SMOKE_TRACE_BOUNDARIES_S, SMOKE_S]) {
    controller.frame({
      scriptTimeS,
      rafTimestampMs: 1000 * scriptTimeS,
      scriptStartMs: 0,
      callbackStartMs: 1000 * scriptTimeS + 0.1,
      callbackMs: 3,
      passesSubmitted: null,
      patchesHard: 100,
      streaming: false,
    });
    // Each cycle is answered before the next frame, as a run's are.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
  await settle();
}

/** The run's control over `spike`, with the measured sizes where given. */
function controllerOf(spike: SpikeApi, sizes: SpikeSizes | null): SpikeController {
  const resolves = { value: 0, runFrame: (n: number) => n };
  return new SpikeController({
    spike,
    gpu: {
      resolves,
      reads: new PassReads(() => resolves.value),
      tally: new PipelineTally(() => 0),
    },
    capture: null,
    canvas: () => ({ widthPx: 1280, heightPx: 720 }),
    nowMs: () => 0,
    log: () => undefined,
    ...(sizes === null ? {} : { sizes }),
  });
}

const LAUNCH: SpikeLaunch = {
  ...TEST_SPIKE_LAUNCH,
  smoke: true,
  world: "terrain-test",
  style: "wireframe",
  sizes: "on",
};

describe("the spike's run control with the measured sizes (R10.T14.b)", () => {
  it("hands the main process the measured sizes' block with the report", async () => {
    const { spike, reports } = fakeSpike(LAUNCH);
    const sizes = new SpikeSizes({
      setting: "low",
      style: "wireframe",
      mainViewId: viewId("spike"),
      secondViewName: "spike-craft",
    });
    await flySmoke(controllerOf(spike, sizes));
    expect(reports.map((report) => report.sizes)).toEqual([sizes.report()]);
  });

  it("hands none without them, as every other run's report", async () => {
    const { sizes: _sizes, ...unmeasured } = LAUNCH;
    const { spike, reports } = fakeSpike(unmeasured);
    await flySmoke(controllerOf(spike, null));
    expect(reports.map((report) => "sizes" in report)).toEqual([false]);
  });
});
