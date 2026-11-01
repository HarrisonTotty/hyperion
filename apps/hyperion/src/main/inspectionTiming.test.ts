import { describe, expect, it } from "vitest";

import type { InspectionTimingRecord } from "../preload/api";
import {
  buildInspectionTimingResults,
  inspectionTimingMarkdown,
  inspectionTimingTraceConfig,
  type InspectionTimingResults,
  ProfileScan,
  profileFigures,
  summariseTimes,
  writeInspectionTimingResults,
} from "./inspectionTiming";
import { measured } from "./measured";
import { readInspectionTimingRecord } from "./viewsCheckRecord";

/** The renderer's main thread and a worker of its process, and the GPU process's main thread. */
function named(scan: ProfileScan): ProfileScan {
  scan.add({ name: "process_name", ph: "M", pid: 1, tid: 0, args: { name: "Renderer" } });
  scan.add({ name: "thread_name", ph: "M", pid: 1, tid: 1, args: { name: "CrRendererMain" } });
  scan.add({
    name: "thread_name",
    ph: "M",
    pid: 1,
    tid: 2,
    args: { name: "DedicatedWorker thread" },
  });
  scan.add({ name: "process_name", ph: "M", pid: 9, tid: 0, args: { name: "GPU Process" } });
  scan.add({ name: "thread_name", ph: "M", pid: 9, tid: 9, args: { name: "CrGpuMain" } });
  return scan;
}

/** A call frame of the renderer's bundle. */
function frame(functionName: string, lineNumber: number): Record<string, unknown> {
  return { functionName, url: "file:///app/out/renderer/assets/styles-x.js", lineNumber };
}

/**
 * The main thread's profile from 1,000 µs: `tick` calls `draw`, which calls `surveyEdges`, then
 * idles; samples at 1,100 (`surveyEdges`, 300 µs), 1,400 (`draw`, 100 µs), 1,500 (`surveyEdges`,
 * 400 µs), 1,900 (idle, 100 µs) and 2,000, each lasting until the next.
 */
function profiled(scan: ProfileScan): ProfileScan {
  scan.add({
    name: "Profile",
    ph: "P",
    pid: 1,
    tid: 1,
    ts: 1_000,
    id: "0x1",
    args: { data: { startTime: 1_000 } },
  });
  scan.add({
    name: "ProfileChunk",
    ph: "P",
    pid: 1,
    tid: 77,
    ts: 2_000,
    id: "0x1",
    args: {
      data: {
        cpuProfile: {
          nodes: [
            { id: 1, callFrame: { functionName: "(root)", url: "" } },
            { id: 2, parent: 1, callFrame: frame("tick", 10) },
            { id: 3, parent: 2, callFrame: frame("draw", 20) },
            { id: 4, parent: 3, callFrame: frame("surveyEdges", 30) },
            { id: 5, parent: 1, callFrame: { functionName: "(idle)", url: "" } },
          ],
          samples: [4, 3, 4, 5, 5],
        },
        timeDeltas: [100, 300, 100, 400, 100],
      },
    },
  });
  return scan;
}

describe("the frame profile's trace (the owner's report)", () => {
  it("clips each thread's tasks and GC to the window, the busiest first", () => {
    const scan = named(new ProfileScan());
    scan.add({ name: "RunTask", ph: "X", pid: 1, tid: 1, ts: 0, dur: 5_000 });
    scan.add({ name: "RunTask", ph: "X", pid: 1, tid: 2, ts: 2_000, dur: 500 });
    scan.add({ name: "RunTask", ph: "X", pid: 9, tid: 9, ts: 1_500, dur: 2_000 });
    scan.add({ name: "MinorGC", ph: "X", pid: 1, tid: 1, ts: 1_200, dur: 300 });
    scan.add({
      name: "V8.GC_X",
      ph: "X",
      pid: 1,
      tid: 1,
      cat: "disabled-by-default-v8.gc",
      ts: 1_300,
      dur: 400,
    });
    scan.add("not an event");
    const { threads } = scan.figures(1_000, 3_000);
    expect(threads.map((t) => [t.process, t.thread, t.busyMs, t.gcMs])).toEqual([
      ["Renderer", "CrRendererMain", 2, 0.5],
      ["GPU Process", "CrGpuMain", 1.5, 0],
      ["Renderer", "DedicatedWorker thread", 0.5, 0],
    ]);
  });

  it("times each sample until the next, from the profile's start, by self and with its callees", () => {
    const scan = profiled(named(new ProfileScan()));
    scan.add({ name: "RunTask", ph: "X", pid: 1, tid: 1, ts: 1_000, dur: 1_000 });
    const main = scan.figures(1_000, 2_000).threads[0];
    expect(main?.self).toEqual([
      { name: "surveyEdges styles-x.js:31", ms: 0.7 },
      { name: "draw styles-x.js:21", ms: 0.1 },
    ]);
    expect(main?.inclusive.map((f) => [f.name, f.ms])).toEqual([
      ["draw styles-x.js:21", 0.8],
      ["tick styles-x.js:11", 0.8],
      ["surveyEdges styles-x.js:31", 0.7],
    ]);
    // Idle is sampled but not code: 0.8 ms of code, the idle sample's 0.1 ms left out.
    expect(main?.sampledMs).toBeCloseTo(0.8);
  });

  it("counts only the samples that fall inside the window", () => {
    const main = profiled(named(new ProfileScan())).figures(1_450, 1_600).threads[0];
    expect(main?.self).toEqual([{ name: "surveyEdges styles-x.js:31", ms: 0.4 }]);
  });

  it("sets the trace's clock against the page's, and says so where it cannot", () => {
    const scan = named(new ProfileScan());
    scan.add({ name: "RunTask", ph: "X", pid: 1, tid: 1, ts: 1_000_000, dur: 50_000 });
    expect(
      profileFigures({ clockOffsetUs: 990_000, scan }, { startMs: 10, endMs: 30 }).value?.threads[0]
        ?.busyMs,
    ).toBe(20);
    expect(profileFigures({ clockOffsetUs: null, scan }, { startMs: 0, endMs: 1 })).toEqual({
      value: null,
      reason: "the trace holds no span of the page's to set its clock by",
    });
  });

  it("records until full, every category but its own left out", () => {
    const config = inspectionTimingTraceConfig();
    expect([config.recording_mode, config.excluded_categories]).toEqual([
      "record-until-full",
      ["*"],
    ]);
    expect(config.included_categories).toContain("disabled-by-default-v8.cpu_profiler");
  });
});

/** A record of one window: two frames, the atmosphere's view pass and the tone map. */
const RECORD: InspectionTimingRecord = {
  timer: "quantized",
  devicePixelRatio: 1,
  windows: [
    {
      name: "close",
      startMs: 0,
      endMs: 2_000,
      canvas: { widthPx: 1140, heightPx: 640 },
      scales: [1, 0.85],
      intervalsMs: [1_000],
      mainThreadMs: [120, 140],
      submitMs: [8, 9],
      gpuMs: [900, 1_100],
      passes: [
        { label: "tonemap", ms: [100, 100] },
        { label: "atmosphere view", ms: [600, 800] },
      ],
      untimedFrames: 1,
      position: "6.69 Mm 023° -12°",
      statements: ["ATMOSPHERE: APPROXIMATE", "TERRAIN: STREAMING"],
    },
  ],
};

/** The timing file of {@link RECORD}, its trace a main thread busy 1,500 ms. */
function results(): InspectionTimingResults {
  return buildInspectionTimingResults({
    run: {
      startedAt: new Date("2026-11-01T12:00:00Z"),
      machine: {
        name: "pod",
        cpu: "cpu",
        logicalCores: 16,
        memoryBytes: 1,
        governor: measured("performance"),
        loadAverage: [1, 1, 1],
        gpu: measured({
          vendorId: 1,
          deviceId: 2,
          driverVersion: null,
          description: "SwiftShader",
        }),
      },
      versions: { app: "0", electron: "45", chromium: "156", node: "24", v8: "15" },
      platform: "linux",
      launchMode: "software",
      setting: "low",
      switches: [],
      shown: false,
      window: { widthDip: 1280, heightDip: 720 },
    },
    served: { universe: "9ada16dcbdbd080a", body: "41ffecb200000002.0400" },
    record: RECORD,
    traces: new Map([
      [
        "close",
        measured({
          threads: [
            {
              process: "Renderer",
              thread: "CrRendererMain",
              busyMs: 1_500,
              gcMs: 40,
              sampledMs: 1_400,
              self: [{ name: "surveyEdges styles-x.js:31", ms: 900 }],
              inclusive: [{ name: "tick index-x.js:5", ms: 1_300 }],
            },
          ],
        }),
      ],
    ]),
  });
}

describe("the frame profile's timing file (the owner's report)", () => {
  it("takes a renderer's record only as the main process checks it", () => {
    expect(readInspectionTimingRecord(RECORD)).toEqual(RECORD);
    expect(
      readInspectionTimingRecord({ ...RECORD, windows: [...RECORD.windows, ...RECORD.windows] }),
    ).toBeNull();
    expect(readInspectionTimingRecord({ ...RECORD, timer: "sometimes" })).toBeNull();
  });

  it("summarises each window's frames, its passes the dearest first with their share", () => {
    const [window] = results().windows;
    expect(window?.drawsPerSecond).toBe(1);
    expect(window?.frame.gpuMs).toEqual({
      count: 2,
      medianMs: 900,
      p95Ms: 1_100,
      meanMs: 1_000,
      maxMs: 1_100,
    });
    expect(window?.passes.map((pass) => [pass.label, pass.meanMs, pass.shareOfGpu])).toEqual([
      ["atmosphere view", 700, 0.7],
      ["tonemap", 100, 0.1],
    ]);
    expect(window?.trace.value?.threads[0]?.thread).toBe("CrRendererMain");
  });

  it("summarises no times as none", () => {
    expect(summariseTimes([])).toEqual({
      count: 0,
      medianMs: null,
      p95Ms: null,
      meanMs: null,
      maxMs: null,
    });
  });

  it("writes the summary's tables: the frame, the passes, the threads and their functions", () => {
    const markdown = inspectionTimingMarkdown(results());
    expect(markdown).toContain("## close: 2 draws in 2.0 s, 1.00 a second");
    expect(markdown).toContain("| atmosphere view | 600.00 | 800.00 | 700.00 | 70.0% |");
    expect(markdown).toContain("| Renderer / CrRendererMain | 1500 | 750.0 | 75.0% | 40 | 1400 |");
    expect(markdown).toContain(
      "| 900 | `surveyEdges styles-x.js:31` | 1300 | `tick index-x.js:5` |",
    );
    expect(markdown).toContain("- **Statements:** ATMOSPHERE: APPROXIMATE · TERRAIN: STREAMING");
  });

  it("names its files for the day, the machine and the setting, numbering a second run", async () => {
    const written = new Map<string, string>();
    const files = {
      mkdir: () => Promise.resolve(),
      exists: (path: string) => Promise.resolve(written.has(path)),
      writeFile: (path: string, text: string) => {
        written.set(path, text);
        return Promise.resolve();
      },
    };
    const first = await writeInspectionTimingResults("/timing", results(), files);
    const second = await writeInspectionTimingResults("/timing", results(), files);
    expect([first.json, second.markdown]).toEqual([
      "/timing/2026-11-01-pod-low-inspection-timing.json",
      "/timing/2026-11-01-pod-low-inspection-timing-2.md",
    ]);
    expect(JSON.parse(written.get(first.json) ?? "{}")).toMatchObject({
      schema: "hyperion.views-check.inspection-timing",
      version: 1,
    });
  });
});
