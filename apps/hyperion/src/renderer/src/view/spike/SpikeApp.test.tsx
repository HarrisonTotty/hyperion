import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { SpikeApi, SpikeEnd, SpikeLaunch } from "../../../../preload/api";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { statusLine } from "../../test/statusLine";
import { TEST_GRAPHICS, TEST_SPIKE_LAUNCH, TEST_SERVER_URL } from "../../test/stubHyperionApi";
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
    render(
      <SpikeApp
        spike={fakeSpike([])}
        graphics={TEST_GRAPHICS}
        spikeWorkers={WORKERS}
        serverUrl={TEST_SERVER_URL}
      />,
    );
    expect(screen.getByRole("img", { name: "VIEW, SPIKE LIT, SCRIPTED" })).toBeDefined();
  });

  it("flies the launch's world in its style", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    render(
      <SpikeApp
        spike={fakeSpike([], { world: "terrain-test", style: "wireframe" })}
        graphics={TEST_GRAPHICS}
        spikeWorkers={WORKERS}
        serverUrl={TEST_SERVER_URL}
      />,
    );
    expect(screen.getByRole("img", { name: "VIEW, WIREFRAME, SCRIPTED" })).toBeDefined();
    expect(screen.queryByText(statusLine(TEST_PLANET_STATEMENT))).toBeNull();
  });

  it("links to the server and reads its scene on a served world's launch (R10.T15.b)", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    vi.stubGlobal("WebSocket", FakeWebSocket);
    render(
      <SpikeApp
        spike={fakeSpike([], {
          world: "served",
          style: "photorealistic",
          served: { universe: "000000000000002a", body: "0200080020000000.0300" },
        })}
        graphics={TEST_GRAPHICS}
        spikeWorkers={WORKERS}
        serverUrl="ws://127.0.0.1:7879/ws"
      />,
    );
    expect(FakeWebSocket.latest().url).toBe("ws://127.0.0.1:7879/ws");
    // The link is not up yet: the spike's frame stands, its line the link's state.
    expect(screen.getByRole("heading", { level: 1, name: "DESCENT SPIKE" })).toBeDefined();
    expect(screen.getByText(statusLine("ESTABLISHING LINK"))).toBeDefined();
    FakeWebSocket.instances = [];
  });

  it("ends the run as failed at once where the renderer has no WebGPU", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const ends: SpikeEnd[] = [];
    render(
      <SpikeApp
        spike={fakeSpike(ends)}
        graphics={TEST_GRAPHICS}
        spikeWorkers={WORKERS}
        serverUrl={TEST_SERVER_URL}
      />,
    );
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
        serverUrl={TEST_SERVER_URL}
      />,
    );
    expect(ends[0]).toEqual({
      status: "fail",
      reason: "the graphics safe mode draws no WebGPU view",
    });
  });
});
