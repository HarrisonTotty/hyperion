import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { SpikeApi, SpikeEnd, SpikeLaunch } from "../../../../preload/api";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { statusLine } from "../../test/statusLine";
import { TEST_GRAPHICS, TEST_SPIKE_LAUNCH } from "../../test/stubHyperionApi";
import { TEST_PLANET_STATEMENT } from "./DescentSpike";
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
  height: () => new ScriptedHeightWorker(),
};

function fakeSpike(ends: SpikeEnd[], launch: Partial<SpikeLaunch> = {}): SpikeApi {
  return {
    launch: { ...TEST_SPIKE_LAUNCH, smoke: true, workers: 1, ...launch },
    startTrace: () => Promise.resolve(),
    cycleTrace: () => Promise.resolve(),
    stopTrace: () => Promise.resolve(),
    sampleMemory: () => Promise.resolve(),
    writeResults: () => Promise.resolve({ kind: "smoke checked", failure: null }),
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

  it("flies the launch's world in its style", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    render(
      <SpikeApp
        spike={fakeSpike([], { world: "terrain-test", style: "wireframe" })}
        graphics={TEST_GRAPHICS}
        spikeWorkers={WORKERS}
      />,
    );
    expect(screen.getByRole("img", { name: "VIEW, WIREFRAME, SCRIPTED" })).toBeDefined();
    expect(screen.queryByText(statusLine(TEST_PLANET_STATEMENT))).toBeNull();
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
