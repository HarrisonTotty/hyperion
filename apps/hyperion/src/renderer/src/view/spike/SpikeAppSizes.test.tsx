import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DescentSpikeReport, SpikeApi, SpikeEnd, SpikeLaunch } from "../../../../preload/api";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { FakeAdapter, FakeGpu, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { TEST_GRAPHICS, TEST_SPIKE_LAUNCH } from "../../test/stubHyperionApi";
import { goldenLevelTable } from "../../test/terrainFixtures";
import { planetGeometry } from "../terrain/planet";
import type { MeasureDescent } from "./bodyDescent";
import { DescentProfile, landingSiteOf, trackStretches } from "./descentProfile";
import { SMOKE_S } from "./spikeController";
import { SpikeApp } from "./SpikeApp";
import type { PreparedDescent, SpikeWorkers } from "./spikeRun";
import { contactRule } from "./spikeScene";
import { spikeWorldNamed } from "./spikeWorld";

/** `TERRAIN TEST`'s world, and a descent over it at the bare datum. */
const MOON = spikeWorldNamed("terrain-test");
const MOON_PROFILE = new DescentProfile(MOON.figure, landingSiteOf(7n));
const MOON_STRETCHES = trackStretches(MOON_PROFILE);

const MOON_DESCENT: PreparedDescent = {
  // A whole level table, which the run's recorder reads the descent's demand from.
  planet: planetGeometry(MOON.figure, goldenLevelTable("off")),
  profile: MOON_PROFILE,
  contact: contactRule(MOON_PROFILE),
  siteHeightM: 0,
  stretches: MOON_STRETCHES,
  stretchMaxHeightsM: MOON_STRETCHES.map(() => 0),
  trackMaxHeightM: 0,
  omittedSigmaM: new Float64Array(25).fill(Infinity),
  siteReading: null,
};

/** The descent measured at once: the terrain world's own measurement needs a field served. */
const MEASURE: MeasureDescent = () => Promise.resolve(MOON_DESCENT);

/** The browser's workers as fakes: the queries never answer, the height workers are scripted. */
const WORKERS: SpikeWorkers = {
  query: () => ({
    postMessage: () => undefined,
    addEventListener: () => undefined,
    terminate: () => undefined,
  }),
  pool: () => () => ({
    reprioritise: () => undefined,
    onBaked: () => () => undefined,
    terminate: () => undefined,
  }),
  height: () => new ScriptedHeightWorker(),
};

/** The adapter's storage-binding limit, B. */
const ADAPTER_BINDING_BYTES = 2_147_483_644;

/** The preload's spike functions on `launch`, keeping the reports written and the ends. */
function fakeSpike(launch: SpikeLaunch): {
  readonly spike: SpikeApi;
  readonly reports: DescentSpikeReport[];
  readonly ends: SpikeEnd[];
} {
  const reports: DescentSpikeReport[] = [];
  const ends: SpikeEnd[] = [];
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
    end: (outcome) => {
      ends.push(outcome);
      return Promise.resolve();
    },
  };
  return { spike, reports, ends };
}

/** A smoke over `TERRAIN TEST`'s moon in the wireframe, with `sizes` as the launch gives it. */
const SMOKE: SpikeLaunch = {
  ...TEST_SPIKE_LAUNCH,
  smoke: true,
  workers: 1,
  world: "terrain-test",
  style: "wireframe",
};

/**
 * Flies `launch`'s smoke through `SpikeApp` to its report, on a fake GPU whose adapter reports
 * {@link ADAPTER_BINDING_BYTES} and a counting engine, and gives the reports written.
 */
async function flySmoke(launch: SpikeLaunch): Promise<ReadonlyArray<DescentSpikeReport>> {
  vi.spyOn(console, "error").mockImplementation(() => undefined);
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
  fakeFramesAndTimeouts();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 640, height: 360 }),
  );
  // An adapter for each request: the graphics status's own and the spike's engine's.
  const gpu = new FakeGpu(
    Array.from(
      { length: 4 },
      () =>
        new FakeAdapter({
          info: SWIFTSHADER_INFO,
          features: [],
          maxStorageBufferBindingSize: ADAPTER_BINDING_BYTES,
        }),
    ),
  );
  vi.stubGlobal("navigator", { gpu, hardwareConcurrency: 4, userAgent: "jsdom" });
  const { spike, reports } = fakeSpike(launch);
  render(
    <SpikeApp
      spike={spike}
      graphics={TEST_GRAPHICS}
      spikeWorkers={WORKERS}
      loadEngine={() => countingRenderEngine()}
      measure={MEASURE}
    />,
  );
  // The smoke's script and a second more, a step at a time, each step's work let settle.
  for (let step = 0; step < 4 * (SMOKE_S + 1) && reports.length === 0; step += 1) {
    // Each step waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
  }
  return reports;
}

afterEach(() => {
  vi.useRealTimers();
});

// A smoke's 650 frames through the whole spike took about 6 s here (2026-10-21), CPU-bound work:
// ten times that, rounded up to 60 s, the suite's rule for such a limit (`vitest.config.mts`).
const SMOKE_TIMEOUT_MS = 60_000;

describe("SpikeApp with the measured sizes (R10.T14.b)", () => {
  it(
    "writes the adapter's storage binding and the bake clock's workers into the smoke's report",
    async () => {
      const [report] = await flySmoke({ ...SMOKE, sizes: "on" });
      expect([
        report?.sizes?.binding.adapterBytes.value,
        (report?.sizes?.bakes.workersMade ?? 0) > 0,
      ]).toEqual([ADAPTER_BINDING_BYTES, true]);
    },
    SMOKE_TIMEOUT_MS,
  );

  it(
    "writes no sizes block for a launch without them",
    async () => {
      const [report] = await flySmoke(SMOKE);
      expect(report === undefined ? null : "sizes" in report).toBe(false);
    },
    SMOKE_TIMEOUT_MS,
  );
});
