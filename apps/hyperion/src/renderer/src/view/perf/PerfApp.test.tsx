import { PERF_PINS, type PerfSceneId } from "@hyperion/perf";
import { render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { describe, expect, it, vi } from "vitest";

import type { PerfApi, PerfEnd } from "../../../../preload/api";
import { statusLine } from "../../test/statusLine";
import { TEST_GRAPHICS } from "../../test/stubHyperionApi";
import { PERF_PREPARING, PerfApp } from "./PerfApp";

/** The main process's side of a run of `scene`, its end recorded. */
function fakePerf(scene: PerfSceneId, ends: PerfEnd[]): PerfApi {
  return {
    config: () =>
      Promise.resolve({
        launch: { scene, setting: "low", out: null, repetitions: 1, control: false, force: false },
        switches: [],
        gpuTiming: true,
        launchMode: "vulkan",
        hidden: false,
        serverUrl: "ws://127.0.0.1:40000/bridge",
        arrival: { universe: "0000000000000001", system: PERF_PINS.system, body: PERF_PINS.giant },
      }),
    describe: () => Promise.resolve(),
    warmUpSample: () => Promise.resolve({ kind: "steady", elapsedS: 120 }),
    beginRepetition: () => Promise.reject(new Error("no repetition in this test")),
    endRepetition: () => Promise.resolve(),
    finish: (end) => {
      ends.push(end);
      return Promise.resolve();
    },
  };
}

describe("PerfApp", () => {
  it("stands ready with the spike's frame before the scene's views are mounted", () => {
    render(<PerfApp perf={fakePerf("ringed-giant-approach", [])} graphics={TEST_GRAPHICS} />);
    expect(screen.getByText(statusLine(PERF_PREPARING))).toBeDefined();
  });

  it("ends a run of a scene it has no driver for at once, saying so", async () => {
    const ends: PerfEnd[] = [];
    render(<PerfApp perf={fakePerf("ringed-giant-approach", ends)} graphics={TEST_GRAPHICS} />);
    await vi.waitFor(() => {
      expect(ends).toEqual([
        { status: "failed", reason: "ringed-giant-approach: no renderer driver yet (R12.T4.c)" },
      ]);
    });
  });

  it("ends its run once though its effects run twice under StrictMode", async () => {
    const ends: PerfEnd[] = [];
    render(
      <StrictMode>
        <PerfApp perf={fakePerf("nuclear-disc-sky", ends)} graphics={TEST_GRAPHICS} />
      </StrictMode>,
    );
    await vi.waitFor(() => {
      expect(ends).toHaveLength(1);
    });
    await Promise.resolve();
    expect(ends).toHaveLength(1);
  });

  it("ends a descent's run at once in a renderer without WebGPU", async () => {
    const ends: PerfEnd[] = [];
    render(<PerfApp perf={fakePerf("cockpit-descent", ends)} graphics={TEST_GRAPHICS} />);
    await vi.waitFor(() => {
      expect(ends).toEqual([{ status: "failed", reason: "this renderer has no WebGPU" }]);
    });
  });
});
