import { PERF_PINS, type PerfSceneId } from "@hyperion/perf";
import { describe, expect, it } from "vitest";

import type { PerfConfig } from "../../../../preload/api";
import { DescentDriver, DescentStages, PerfEngine } from "./descentDriver";
import { perfDriverFor } from "./driverFor";
import { ViewDriver, ViewStages } from "./viewDriver";

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

describe("the driver for a scene", () => {
  const page = {
    engine: new PerfEngine(undefined),
    stages: new DescentStages(),
    views: new ViewStages(),
    launchMode: "vulkan" as const,
    webGpu: true,
    defaultWorkers: () => 5,
  };

  /** The driver chosen for `scene`, or `null` where the run is refused. */
  function driverOf(scene: PerfSceneId): unknown {
    const choice = perfDriverFor(config(scene), page);
    return choice.kind === "driver" ? choice.driver : null;
  }

  it("is the descent spike's for both descents and the arrival", () => {
    for (const scene of [
      "cockpit-descent",
      "station-wireframe-descent",
      "arrival-memory",
    ] as const) {
      expect(driverOf(scene)).toBeInstanceOf(DescentDriver);
    }
  });

  it("is VIEW's scripted camera's for the giant's approach and the sky's sweep", () => {
    for (const scene of ["ringed-giant-approach", "nuclear-disc-sky"] as const) {
      expect(driverOf(scene)).toBeInstanceOf(ViewDriver);
    }
  });

  it("is none in the graphics safe mode or without WebGPU", () => {
    expect(perfDriverFor(config("nuclear-disc-sky"), { ...page, launchMode: "safe" })).toEqual({
      kind: "refused",
      reason: "the graphics safe mode draws no WebGPU view",
      shown: "the graphics safe mode draws no view, relaunch to retry",
    });
    expect(perfDriverFor(config("cockpit-descent"), { ...page, webGpu: false })).toEqual({
      kind: "refused",
      reason: "this renderer has no WebGPU",
      shown: "no WebGPU",
    });
  });
});
