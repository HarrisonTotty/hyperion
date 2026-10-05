import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { SpikeApi, SpikeEnd } from "../../../../preload/api";
import { TEST_GRAPHICS, TEST_SPIKE_LAUNCH } from "../../test/stubHyperionApi";
import { SpikeApp } from "./SpikeApp";
import type { SpikeWorkers } from "./spikeRun";
import type { SurfaceQueryWorker } from "./surfaceQuery";

/** A query worker that never answers: these tests end before the terrain is measured. */
const SILENT_WORKER: SurfaceQueryWorker = {
  postMessage: () => undefined,
  addEventListener: () => undefined,
  terminate: () => undefined,
};

const WORKERS: SpikeWorkers = {
  query: () => SILENT_WORKER,
  pool: () => () => ({
    reprioritise: () => undefined,
    onBaked: () => () => undefined,
    terminate: () => undefined,
  }),
};

function fakeSpike(ends: SpikeEnd[]): SpikeApi {
  return {
    launch: { ...TEST_SPIKE_LAUNCH, smoke: true, workers: 1 },
    startTrace: () => Promise.resolve(),
    cycleTrace: () => Promise.resolve(),
    stopTrace: () => Promise.resolve(),
    sampleMemory: () => Promise.resolve(),
    writeResults: () => Promise.resolve({ json: "a", markdown: "b" }),
    writeCapture: () => Promise.resolve("c"),
    end: (outcome) => {
      ends.push(outcome);
      return Promise.resolve();
    },
  };
}

describe("SpikeApp", () => {
  it("draws the descent spike on a spike launch", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    render(<SpikeApp spike={fakeSpike([])} graphics={TEST_GRAPHICS} spikeWorkers={WORKERS} />);
    expect(screen.getByRole("img", { name: "VIEW, SPIKE LIT, SCRIPTED" })).toBeDefined();
  });

  it("ends the run as failed at once where the renderer has no WebGPU", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const ends: SpikeEnd[] = [];
    render(<SpikeApp spike={fakeSpike(ends)} graphics={TEST_GRAPHICS} spikeWorkers={WORKERS} />);
    expect(ends).toEqual([{ status: "fail", reason: "this renderer has no WebGPU" }]);
  });

  it("ends the run as failed in the graphics safe mode", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const ends: SpikeEnd[] = [];
    render(
      <SpikeApp
        spike={fakeSpike(ends)}
        graphics={{ ...TEST_GRAPHICS, launchMode: "safe" }}
        spikeWorkers={WORKERS}
      />,
    );
    expect(ends[0]).toEqual({
      status: "fail",
      reason: "the graphics safe mode draws no WebGPU view",
    });
  });
});
