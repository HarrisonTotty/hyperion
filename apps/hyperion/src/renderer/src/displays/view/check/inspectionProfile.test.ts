import { describe, expect, it } from "vitest";

import type { ViewsCheckWindow } from "../../../../../preload/api";
import { SOFTWARE_WAIT_FACTOR, inspectionProfileTimingOf } from "./softwareTiming";
import {
  INSPECTION_PROFILE_TIMING,
  type InspectionProfileDeps,
  InspectionProfiler,
} from "./inspectionProfile";
import type { WindowFigures } from "./viewsProbe";

/** A window's figures as the probe would give them: one draw of the primary. */
function figures(startMs: number, endMs: number): WindowFigures {
  return {
    startMs,
    endMs,
    views: [
      {
        name: "view",
        style: "photorealistic",
        canvas: { widthPx: 1140, heightPx: 640 },
        draws: 1,
        gpuMs: [40],
        submitMs: [3],
        passLabels: ["view"],
        scales: [0.85],
      },
    ],
    frameIntervalsMs: [16],
    primaryIntervalsMs: [33],
    mainThreadMs: [12],
    frameGpuMs: [40],
    untimedFrames: 0,
    droppedResolves: 0,
    unattributedGpuMs: 0,
  };
}

/** A profiler over fakes, the clock moving by each wait, the primary drawing every `drawMs`. */
function profiler(drawMs: number): {
  readonly profile: InspectionProfiler;
  readonly calls: string[];
  readonly marked: ViewsCheckWindow[];
} {
  const calls: string[] = [];
  const marked: ViewsCheckWindow[] = [];
  let nowMs = 1_000;
  const deps: InspectionProfileDeps = {
    api: {
      startProfile: (name) => {
        calls.push(`start ${name} at ${String(nowMs)}`);
        return Promise.resolve();
      },
      endProfile: (name, span) => {
        calls.push(`end ${name} ${String(span.startMs)}–${String(span.endMs)} at ${String(nowMs)}`);
        return Promise.resolve();
      },
      writeTimingResults: (record) => {
        calls.push(`write ${record.windows.map((each) => each.name).join(" ")}`);
        return Promise.resolve({ json: "a", markdown: "b" });
      },
    },
    views: {
      // A draw every `drawMs` since the window's start.
      drawTimes: (_view, fromMs) =>
        Array.from({ length: Math.floor((nowMs - fromMs) / drawMs) }, (_, i) => fromMs + i),
      window: figures,
      passTimes: () => [
        { label: "atmosphere view", ms: [30], kernels: ["march"], workgroups: [9] },
      ],
      memoryTraffic: () => ({
        uploaded: { count: 3, bytes: 96 },
        created: { count: 0, bytes: 0 },
        destroyed: { count: 0, bytes: 0 },
        uploads: [{ name: "view star sprites", count: 3, bytes: 96 }],
      }),
      timer: () => "quantized",
    },
    markWindow: (span) => {
      marked.push(span);
    },
    nowMs: () => nowMs,
    sleep: (ms) => {
      nowMs += ms;
      return Promise.resolve();
    },
    devicePixelRatio: 1,
    setting: "high",
    timing: INSPECTION_PROFILE_TIMING,
  };
  return { profile: new InspectionProfiler(deps), calls, marked };
}

/** A plate as the window ends. */
const PLATE = (): { position: string; statements: string[] } => ({
  position: "28.9 Mm 268° +49°",
  statements: ["ATMOSPHERE: APPROXIMATE"],
});

describe("the inspection run's frame profile (the owner's report)", () => {
  it("ends a window at its draws, marks it, and waits for the last pass times before ending it", async () => {
    const { profile, calls, marked } = profiler(100);
    await profile.measure("orbit", PLATE);
    // 30 draws at 100 ms take 3 s; the trace ends a settle later.
    expect(marked).toEqual([{ startMs: 1_000, endMs: 4_000 }]);
    expect(calls).toEqual(["start orbit at 1000", "end orbit 1000–4000 at 5000"]);
    expect(profile.windows[0]).toMatchObject({
      name: "orbit",
      canvas: { widthPx: 1140, heightPx: 640 },
      scales: [0.85],
      intervalsMs: [33],
      gpuMs: [40],
      passes: [{ label: "atmosphere view", ms: [30], kernels: ["march"], workgroups: [9] }],
      memory: { uploaded: { count: 3, bytes: 96 } },
      marchSamples: 36,
      position: "28.9 Mm 268° +49°",
    });
  });

  it("ends a window of slow frames at its longest wait, with the draws it has", async () => {
    const { profile, marked } = profiler(5_000);
    await profile.measure("close", PLATE);
    expect(marked).toEqual([{ startMs: 1_000, endMs: 1_000 + INSPECTION_PROFILE_TIMING.maxMs }]);
  });

  it("writes every window measured, in order, with the pass timer's kind", async () => {
    const { profile, calls } = profiler(100);
    await profile.measure("orbit", PLATE);
    await profile.measure("close", PLATE);
    await profile.write();
    expect(calls.at(-1)).toBe("write orbit close");
    expect(profile.record().timer).toBe("quantized");
  });

  it("stretches its waits five times in software, never its count of draws", () => {
    const software = inspectionProfileTimingOf(INSPECTION_PROFILE_TIMING, "software");
    expect([
      inspectionProfileTimingOf(INSPECTION_PROFILE_TIMING, "vulkan"),
      software.maxMs / INSPECTION_PROFILE_TIMING.maxMs,
      software.settleMs / INSPECTION_PROFILE_TIMING.settleMs,
      software.draws,
    ]).toEqual([
      INSPECTION_PROFILE_TIMING,
      SOFTWARE_WAIT_FACTOR,
      SOFTWARE_WAIT_FACTOR,
      INSPECTION_PROFILE_TIMING.draws,
    ]);
  });
});
