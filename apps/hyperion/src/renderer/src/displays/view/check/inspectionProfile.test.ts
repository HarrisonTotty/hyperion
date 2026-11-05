import { describe, expect, it, vi } from "vitest";

import type { ViewsCheckWindow } from "../../../../../preload/api";
import { TABLE_SIZES } from "../../../view/atmosphere/hillaire";
import type { MarchCensus } from "../../../view/atmosphere/marchProbe";
import { IMAGE_DETAIL_LIMITED } from "../../../view/budget/viewBudget";
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

/** A march's census as the views probe would read it back (R08.T8.d). */
const CENSUS: MarchCensus = {
  kernel: "atmosphere ray march",
  texelsByNodes: [60, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2],
  wholeMeans: 1,
  centresInAir: 40,
  tiles: 1,
  limbTiles: 1,
  rays: 63,
  laneRays: 704,
};

/**
 * A profiler over fakes, the clock moving by each wait, the primary drawing every `drawMs`, its
 * calls counted by `counter` where given, and each census asked of the probe answered by `census`
 * (none by default: no engine made) and its signals kept, React's commits counted by `react`
 * where given.
 */
function profiler(
  drawMs: number,
  untimedUntilMs = Number.NEGATIVE_INFINITY,
  counter: InspectionProfileDeps["calls"] = null,
  census: () => Promise<MarchCensus> | null = () => null,
  react: InspectionProfileDeps["react"] = null,
): {
  readonly profile: InspectionProfiler;
  readonly calls: string[];
  readonly marked: ViewsCheckWindow[];
  readonly signals: AbortSignal[];
  readonly clusterReads: number[];
} {
  const calls: string[] = [];
  const marked: ViewsCheckWindow[] = [];
  const signals: AbortSignal[] = [];
  const clusterReads: number[] = [];
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
      // Its frames untimed until `untimedUntilMs`, as a slow renderer's resolves land late.
      window: (startMs, endMs) => ({
        ...figures(startMs, endMs),
        untimedFrames: nowMs < untimedUntilMs ? 1 : 0,
      }),
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
      marchCensus: (signal) => {
        signals.push(signal);
        return census();
      },
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
    calls: counter,
    react,
    // The counters as read: one more cluster each read, so that a window's are its end's.
    clusters: () => {
      clusterReads.push(nowMs);
      return { passes: [{ label: "lit", clusters: clusterReads.length, triangles: 896 }] };
    },
  };
  return { profile: new InspectionProfiler(deps), calls, marked, signals, clusterReads };
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
    // 30 draws at 100 ms take 3 s; the trace ends once their times are in, a poll later.
    expect(marked).toEqual([{ startMs: 1_000, endMs: 4_000 }]);
    expect(calls).toEqual(["start orbit at 1000", "end orbit 1000–4000 at 4250"]);
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

  it("counts the WebGPU calls of its draws alone, and none on a launch that does not count", async () => {
    let nowMs = 0;
    const said: string[] = [];
    const counter: InspectionProfileDeps["calls"] = {
      begin: () => {
        said.push("begin");
      },
      take: () => {
        said.push("take");
        return [{ label: "bodies", call: "render pass.draw", count: nowMs, bytes: 0 }];
      },
    };
    const counted = profiler(100, 6_000, counter);
    await counted.profile.measure("orbit", () => {
      nowMs = 3;
      return PLATE();
    });
    expect([said, counted.profile.windows[0]?.gpuCalls]).toEqual([
      ["begin", "take"],
      [{ label: "bodies", call: "render pass.draw", count: 0, bytes: 0 }],
    ]);
    const uncounted = profiler(100);
    await uncounted.profile.measure("orbit", PLATE);
    expect(uncounted.profile.windows[0]?.gpuCalls).toBeNull();
  });

  it("counts React's commits over its draws alone, and none where React took no hook (lane perf-render-thread-2)", async () => {
    const said: string[] = [];
    const react: InspectionProfileDeps["react"] = {
      begin: () => {
        said.push("begin");
      },
      take: () => {
        said.push("take");
        return { commits: 2, renders: 0, components: [] };
      },
    };
    const counted = profiler(100, 6_000, null, () => null, react);
    await counted.profile.measure("orbit", PLATE);
    const uncounted = profiler(100);
    await uncounted.profile.measure("orbit", PLATE);
    expect([said, counted.profile.windows[0]?.react, uncounted.profile.windows[0]?.react]).toEqual([
      ["begin", "take"],
      { commits: 2, renders: 0, components: [] },
      null,
    ]);
  });

  it("waits for its frames' late pass times, and no longer than its settle", async () => {
    const late = profiler(100, 6_000);
    await late.profile.measure("orbit", PLATE);
    expect(late.calls.at(-1)).toBe("end orbit 1000–4000 at 6000");
    const never = profiler(100, Number.POSITIVE_INFINITY);
    await never.profile.measure("orbit", PLATE);
    expect(never.calls.at(-1)).toBe(
      `end orbit 1000–4000 at ${String(4_000 + INSPECTION_PROFILE_TIMING.settleMs)}`,
    );
  });

  it("ends a window of slow frames at its longest wait, with the draws it has", async () => {
    const { profile, marked } = profiler(5_000);
    await profile.measure("close", PLATE);
    expect(marked).toEqual([{ startMs: 1_000, endMs: 1_000 + INSPECTION_PROFILE_TIMING.maxMs }]);
  });

  it("ends a window whose step throws, its trace stopped and nothing measured", async () => {
    const { profile, calls } = profiler(100);
    await expect(
      profile.measure("orbit", () => {
        throw new Error("views check: the primary's label block is not shown");
      }),
    ).rejects.toThrow(/label block/);
    expect([calls, profile.windows]).toEqual([
      ["start orbit at 1000", "end orbit 1000–4000 at 4000"],
      [],
    ]);
  });

  it("switches the terrain's cluster counters on at a window's start and records them at its end (plan R14, R14.T3.d)", async () => {
    const { profile, clusterReads } = profiler(100);
    await profile.measure("orbit", PLATE);
    expect([clusterReads, profile.windows[0]?.clusters]).toEqual([
      [1_000, 4_250],
      { passes: [{ label: "lit", clusters: 2, triangles: 896 }] },
    ]);
  });

  it("reads the march's samples at the low setting's while IMAGE: DETAIL LIMITED stands", async () => {
    const { profile } = profiler(100);
    await profile.measure("orbit", () => ({ position: null, statements: [IMAGE_DETAIL_LIMITED] }));
    await profile.measure("close", PLATE);
    expect(profile.windows.map((each) => each.marchSamples)).toEqual([
      TABLE_SIZES.low.rayMarchSamples,
      TABLE_SIZES.high.rayMarchSamples,
    ]);
  });

  it("records the march's census the frame after the window, its ask then withdrawn (R08.T8.d)", async () => {
    const { profile, signals } = profiler(100, Number.NEGATIVE_INFINITY, null, () =>
      Promise.resolve(CENSUS),
    );
    await profile.measure("orbit", PLATE);
    expect([profile.windows[0]?.marchCensus, signals.map((signal) => signal.aborted)]).toEqual([
      CENSUS,
      [true],
    ]);
  });

  it("records no census where none is taken within its settle, or where it fails", async () => {
    const never = profiler(100, Number.NEGATIVE_INFINITY, null, () => new Promise(() => undefined));
    await never.profile.measure("orbit", PLATE);
    const failed = profiler(100, Number.NEGATIVE_INFINITY, null, () =>
      Promise.reject(new Error("the census's read-back failed")),
    );
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    await failed.profile.measure("close", PLATE);
    const none = profiler(100);
    await none.profile.measure("orbit", PLATE);
    expect([
      never.profile.windows[0]?.marchCensus,
      never.signals.map((signal) => signal.aborted),
      failed.profile.windows[0]?.marchCensus,
      warn.mock.calls.length,
      none.profile.windows[0]?.marchCensus,
    ]).toEqual([null, [true], null, 1, null]);
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
