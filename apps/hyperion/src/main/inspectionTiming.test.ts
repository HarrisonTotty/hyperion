import { describe, expect, it, vi } from "vitest";

import type {
  InspectionGpuAttachment,
  InspectionLongTasks,
  InspectionTimingMarchCensus,
  InspectionTimingRecord,
} from "../preload/api";
import {
  buildInspectionTimingResults,
  gpuAttachmentFigures,
  gpuCallKind,
  type InspectionTimingInput,
  inspectionTimingMarkdown,
  inspectionTimingTraceConfig,
  type InspectionTimingResults,
  isMarchKernel,
  longTaskFigures,
  ProfileScan,
  profileFigures,
  summariseTimes,
  writeInspectionTimingResults,
  TERRAIN_PASS_LABELS,
} from "./inspectionTiming";
import { measured } from "./measured";
import { longTaskFunctions, type PageProfile } from "./pageProfile";
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

  it("states each thread's longest task, clipped to the window (lane perf-render-thread-ground)", () => {
    const scan = named(new ProfileScan());
    scan.add({ name: "RunTask", ph: "X", pid: 1, tid: 1, ts: 0, dur: 5_000 });
    scan.add({ name: "RunTask", ph: "X", pid: 1, tid: 1, ts: 1_100, dur: 300 });
    scan.add({ name: "RunTask", ph: "X", pid: 9, tid: 9, ts: 1_500, dur: 700 });
    scan.add({ name: "RunTask", ph: "X", pid: 9, tid: 9, ts: 2_400, dur: 900 });
    const { threads } = scan.figures(1_000, 3_000);
    expect(threads.map((t) => [t.thread, t.longestTaskMs])).toEqual([
      ["CrRendererMain", 2],
      ["CrGpuMain", 0.7],
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

/**
 * A march's census (R08.T8.d): 1,000 texels, 900 of them its centre ray alone, 60 with 2 nodes and
 * 40 with 10, 30 of those at w_L 1; 4 of 20 tiles hold a node ray.
 */
const CENSUS: InspectionTimingMarchCensus = {
  kernel: "atmosphere ray march",
  texelsByNodes: [900, 0, 60, 0, 0, 0, 0, 0, 0, 0, 40],
  wholeMeans: 30,
  centresInAir: 600,
  tiles: 20,
  limbTiles: 4,
  rays: 1_150,
  laneRays: 2_300,
};

/** A kind of attachment two passes of 100 × 50 px took, as the counter tallies it. */
function attached(
  label: string,
  attachment: InspectionGpuAttachment["attachment"],
  format: string,
  loadOp: InspectionGpuAttachment["loadOp"],
  storeOp: InspectionGpuAttachment["storeOp"],
  bytesLoaded: number,
  bytesStored: number,
): InspectionGpuAttachment {
  return {
    label,
    attachment,
    view: { format, widthPx: 100, heightPx: 50, sampleCount: 1 },
    loadOp,
    storeOp,
    passes: 2,
    bytesLoaded,
    bytesStored,
  };
}

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
      frameIntervalsMs: [1_000],
      mainThreadMs: [120, 140],
      submitMs: [8, 9],
      gpuMs: [900, 1_100],
      passes: [
        { label: "tonemap", ms: [100, 100], kernels: [], workgroups: [] },
        {
          label: "atmosphere view",
          ms: [600, 800],
          kernels: ["atmosphere ray march (footprint)"],
          workgroups: [1_000, 1_000],
        },
      ],
      untimedFrames: 1,
      memory: {
        uploaded: { count: 4, bytes: 2_097_152 },
        created: { count: 1, bytes: 1_024 },
        destroyed: { count: 0, bytes: 0 },
        uploads: [{ name: "view star sprites", count: 4, bytes: 2_097_152 }],
      },
      gpuCalls: [
        { label: "bodies", call: "render pass.draw", count: 6, bytes: 0 },
        { label: "bodies", call: "render pass.setBindGroup", count: 12, bytes: 0 },
        { label: "bodies", call: "queue.writeBuffer", count: 2, bytes: 4_096 },
        { label: "atmosphere view", call: "device.createBindGroup", count: 4, bytes: 0 },
      ],
      // Two frames' passes at 100 × 50 px: 40,000 bytes an rgba16float target, 20,000 a 4-byte one.
      gpuAttachments: [
        attached("bodies", "colour 0", "rgba16float", "clear", "store", 0, 80_000),
        attached("bodies", "depth", "depth32float", "clear", "store", 0, 40_000),
        attached("terrain resolve", "colour 0", "rgba16float", "load", "store", 80_000, 80_000),
        attached("tonemap", "colour 0", "bgra8unorm-srgb", "clear", "store", 0, 40_000),
      ],
      atmosphereReads: [
        {
          air: 1,
          read: "deck",
          thermal: false,
          frames: 2,
          kernels: ["atmosphere sky view, thick", "atmosphere ray march, thick, over a deck"],
        },
        {
          air: 2,
          read: "thin",
          thermal: true,
          frames: 1,
          kernels: ["atmosphere sky view, thermal"],
        },
      ],
      react: {
        commits: 4,
        renders: 12,
        components: [
          {
            name: "ViewStage",
            renders: 4,
            mounts: 0,
            unchanged: 1,
            stateChanged: 3,
            propsChanged: [],
          },
          {
            name: "ViewLabelBlock",
            renders: 4,
            mounts: 1,
            unchanged: 1,
            stateChanged: 0,
            propsChanged: [
              { prop: "lines", renders: 2 },
              { prop: "faults", renders: 1 },
            ],
          },
        ],
      },
      clusters: {
        passes: [
          { label: "lit", clusters: 900, triangles: 100_000 },
          { label: "lit late", clusters: 40, triangles: 5_000 },
        ],
      },
      marchSamples: 36,
      position: "6.69 Mm 023° -12°",
      statements: ["ATMOSPHERE: APPROXIMATE", "TERRAIN: STREAMING"],
      marchCensus: CENSUS,
    },
  ],
  longTasks: null,
};

/**
 * The timing file of `record`, {@link RECORD} by default, its trace a main thread busy 1,500 ms,
 * drawn under `compositeProbe`, `marchProbe`, `atmosphereLevers` and `litProbe`, its long tasks
 * named by `profile`, the page's CPU profile, where given.
 */
function results(
  compositeProbe: InspectionTimingResults["run"]["compositeProbe"] = null,
  marchProbe: InspectionTimingResults["run"]["marchProbe"] = null,
  atmosphereLevers: InspectionTimingResults["run"]["atmosphereLevers"] = null,
  litProbe: InspectionTimingResults["run"]["litProbe"] = null,
  record: InspectionTimingRecord = RECORD,
  profile: PageProfile | null = null,
): InspectionTimingResults {
  return buildInspectionTimingResults(
    inputOf(compositeProbe, marchProbe, atmosphereLevers, litProbe, record, profile),
  );
}

/** What {@link results} builds its file from. */
function inputOf(
  compositeProbe: InspectionTimingResults["run"]["compositeProbe"] = null,
  marchProbe: InspectionTimingResults["run"]["marchProbe"] = null,
  atmosphereLevers: InspectionTimingResults["run"]["atmosphereLevers"] = null,
  litProbe: InspectionTimingResults["run"]["litProbe"] = null,
  record: InspectionTimingRecord = RECORD,
  profile: PageProfile | null = null,
): InspectionTimingInput {
  return {
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
      framePacing: "rate",
      frameRateHz: 120,
      window: { widthDip: 1280, heightDip: 720 },
    },
    served: { universe: "9ada16dcbdbd080a", body: "41ffecb200000002.0400" },
    compositeProbe,
    marchProbe,
    atmosphereLevers,
    litProbe,
    record,
    pageProfile: profile,
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
              longestTaskMs: 96,
              sampledMs: 1_400,
              self: [{ name: "surveyEdges styles-x.js:31", ms: 900 }],
              inclusive: [
                { name: "tick index-x.js:5", ms: 1_300 },
                { name: "ViewLabelBlock index-x.js:900", ms: 12 },
              ],
              reactMs: { renderAndCommit: 30, passiveEffects: 4 },
            },
          ],
        }),
      ],
    ]),
  };
}

/**
 * A page's CPU profile on the page's clock, a sample a millisecond: 2,000 ms in a height worker's
 * reply blocked in `writeBuffer` from 100 ms, then 300 ms of the draw loop's tick from 5,000 ms,
 * and idle otherwise.
 */
function pageProfile(): PageProfile {
  const nodes = new Map([
    [1, { parent: undefined, name: "(root)", named: false }],
    [2, { parent: 1, name: "(idle)", named: false }],
    [3, { parent: 1, name: "onMessage useTerrainSession-x.js:9", named: true }],
    [4, { parent: 3, name: "upload useTerrainSession-x.js:20", named: true }],
    [5, { parent: 4, name: "writeBuffer", named: false }],
    [6, { parent: 1, name: "tick index-x.js:5", named: true }],
    [7, { parent: 6, name: "draw useTerrainSession-x.js:40", named: true }],
  ]);
  const sampleNodes: number[] = [];
  const sampleAtMs: number[] = [];
  for (let ms = 0; ms < 8_000; ms += 1) {
    sampleAtMs.push(ms);
    sampleNodes.push(
      ms >= 100 && ms < 2_100 ? (ms < 150 ? 4 : 5) : ms >= 5_000 && ms < 5_300 ? 7 : 2,
    );
  }
  return { nodes, sampleNodes, sampleAtMs, endMs: 8_000 };
}

describe("the frame profile's timing file (the owner's report)", () => {
  it("takes a renderer's record only as the main process checks it", () => {
    expect(readInspectionTimingRecord(RECORD)).toEqual(RECORD);
    expect(
      readInspectionTimingRecord({ ...RECORD, windows: [...RECORD.windows, ...RECORD.windows] }),
    ).toBeNull();
    expect(readInspectionTimingRecord({ ...RECORD, timer: "sometimes" })).toBeNull();
  });

  it("drops alone a window whose passes are not timed as many frames as its GPU times, and says so (lane fix-inspection-timing-record)", () => {
    const [window] = RECORD.windows;
    if (window === undefined) {
      throw new Error("the record holds a window");
    }
    const mistimed = {
      ...window,
      name: "orbit",
      // A resolve that reported after the frames' GPU times were read: 3 pass times against 2.
      passes: window.passes.map((pass) =>
        pass.label === "tonemap" ? { ...pass, ms: [...pass.ms, 100] } : pass,
      ),
    };
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const read = readInspectionTimingRecord({ ...RECORD, windows: [window, mistimed] });
    expect([read?.windows.map((each) => each.name), warn.mock.calls]).toEqual([
      ["close"],
      [
        [
          "views check: the timing window orbit dropped: its pass tonemap has 3 times against 2 frame GPU times",
        ],
      ],
    ]);
    warn.mockRestore();
  });

  it("takes a window's march census only whole, or none (R08.T8.d)", () => {
    const [window] = RECORD.windows;
    if (window === undefined) {
      throw new Error("the record holds a window");
    }
    const withCensus = (marchCensus: unknown): unknown => ({
      ...RECORD,
      windows: [{ ...window, marchCensus }],
    });
    expect(readInspectionTimingRecord(withCensus(null))?.windows[0]?.marchCensus).toBeNull();
    expect(
      [
        { ...CENSUS, texelsByNodes: CENSUS.texelsByNodes.slice(1) },
        { ...CENSUS, limbTiles: 21 },
        { ...CENSUS, laneRays: 1_000 },
        { ...CENSUS, rays: 0.5 },
        { ...CENSUS, kernel: "" },
      ].map((census) => readInspectionTimingRecord(withCensus(census))),
    ).toEqual([null, null, null, null, null]);
    expect(
      readInspectionTimingRecord({ ...RECORD, windows: [{ ...window, marchCensus: undefined }] }),
    ).toBeNull();
  });

  it("records the run's pacing and each window's callbacks and measured vsync (R07.T19)", () => {
    const [window] = RECORD.windows;
    if (window === undefined) {
      throw new Error("the record has no window");
    }
    const ticked = buildInspectionTimingResults({
      ...inputOf(),
      record: {
        ...RECORD,
        windows: [
          {
            ...window,
            frameIntervalsMs: Array.from({ length: 40 }, () => 1000 / 120),
            mainThreadMs: Array.from({ length: 41 }, () => 2),
          },
        ],
      },
    });
    const text = inspectionTimingMarkdown(ticked);
    expect([
      ticked.run.framePacing,
      ticked.run.frameRateHz,
      ticked.windows[0]?.callbacks,
      ticked.windows[0]?.measuredVsyncMs.value?.toFixed(2),
      results().windows[0]?.measuredVsyncMs.reason,
      text.includes("- **Pacing:** 41 animation frames for 2 draws; measured vsync 8.33 ms"),
    ]).toEqual(["rate", 120, 41, "8.33", "fewer than 30 animation frames measured", true]);
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

  it("times the ray march a workgroup, and a texel a step at the window's samples (main's ask)", () => {
    const [window] = results().windows;
    const march = window?.passes.find((pass) => pass.label === "atmosphere view");
    const tonemap = window?.passes.find((pass) => pass.label === "tonemap");
    // 600 ms over 1,000 workgroups of 64 texels, 36 samples a ray: 600 µs, 260.4 ns.
    expect([march?.workgroupsMedian, march?.nsPerWorkgroup]).toEqual([1_000, 600_000]);
    expect(march?.nsPerTexelStep).toBeCloseTo(600_000 / 64 / 36, 9);
    expect([tonemap?.workgroupsMedian, tonemap?.nsPerTexelStep]).toEqual([null, null]);
    expect(
      [
        "atmosphere ray march",
        "atmosphere ray march by slot, thick",
        "atmosphere ray march's fill",
      ].map(isMarchKernel),
    ).toEqual([true, true, false]);
  });

  it("sums the counted WebGPU calls a draw by kind and by pass, the most first (plan R14)", () => {
    const calls = results().windows[0]?.gpuCalls;
    expect(calls?.perDraw).toBe(12);
    expect(
      calls?.kinds.filter((each) => each.perDraw > 0).map((each) => [each.kind, each.perDraw]),
    ).toEqual([
      ["draws", 3],
      ["setBindGroup", 6],
      ["writeBuffer", 1],
      ["createBindGroup", 2],
    ]);
    expect(calls?.kinds.find((each) => each.kind === "writeBuffer")?.bytesPerDraw).toBe(2_048);
    expect(calls?.passes.map((pass) => [pass.label, pass.perDraw, pass.kinds[0]?.kind])).toEqual([
      ["bodies", 10, "setBindGroup"],
      ["atmosphere view", 2, "createBindGroup"],
    ]);
    expect(
      [
        "render pass.drawIndexedIndirect",
        "compute pass.dispatchWorkgroupsIndirect",
        "compute pass.setPipeline",
        "device.createTexture",
        "encoder.beginComputePass",
        "encoder.copyTextureToTexture",
      ].map(gpuCallKind),
    ).toEqual([
      "draws",
      "dispatches",
      "setPipeline",
      "createBuffer or createTexture",
      "passes",
      "other",
    ]);
    const markdown = inspectionTimingMarkdown(results());
    expect(markdown).toContain("- **Every call:** 12.0 a draw of the primary");
    expect(markdown).toContain("| writeBuffer | 1.0 | 2.0 |");
    expect(markdown).toContain("| bodies | 10.0 | setBindGroup 6.0, draws 3.0, writeBuffer 1.0 |");
  });

  it("states React's commits, renders and work a draw, and each component's renders by what changed (lane perf-render-thread-2)", () => {
    const markdown = inspectionTimingMarkdown(results());
    expect(markdown).toContain(
      "- **Commits:** 4 (2.0 a draw of the primary); **renders:** 12 (6.0 a draw)",
    );
    expect(markdown).toContain(
      "render and commit 30 ms (15.0 a draw), passive effects 4 ms (2.0 a draw)",
    );
    expect(markdown).toContain("| ViewLabelBlock | 4 | 2.0 | 12 | 1 | 1 | 0 | lines 2, faults 1 |");
    expect(markdown).toContain("| ViewStage | 4 | 2.0 | — | 0 | 1 | 3 | — |");
  });

  it("takes a window's React commits, or none, and refuses them inconsistent (lane perf-render-thread-2)", () => {
    const [window] = RECORD.windows;
    if (window === undefined || window.react === null) {
      throw new Error("the record has a window with React's commits");
    }
    const { react } = window;
    const withReact = (value: unknown): unknown => ({
      ...RECORD,
      windows: [{ ...window, react: value }],
    });
    const [stage] = react.components;
    if (stage === undefined) {
      throw new Error("the record lists a component");
    }
    expect([
      readInspectionTimingRecord(withReact(null))?.windows[0]?.react,
      readInspectionTimingRecord(withReact({ ...react, renders: 7 })),
      readInspectionTimingRecord(
        withReact({ ...react, components: [stage, { ...stage, renders: 1 }] }),
      ),
      readInspectionTimingRecord(withReact({ ...react, components: [{ ...stage, unchanged: 2 }] })),
      readInspectionTimingRecord(withReact(undefined)),
    ]).toEqual([null, null, null, null, null]);
  });

  it("sums React's work in a thread's inclusive time by its functions, whatever their line (lane perf-render-thread-2)", () => {
    const scan = named(new ProfileScan());
    scan.add({
      name: "Profile",
      ph: "P",
      pid: 1,
      tid: 1,
      ts: 1_000,
      id: "0x2",
      args: { data: { startTime: 1_000 } },
    });
    scan.add({
      name: "ProfileChunk",
      ph: "P",
      pid: 1,
      tid: 77,
      ts: 1_000,
      id: "0x2",
      args: {
        data: {
          cpuProfile: {
            nodes: [
              { id: 1, callFrame: frame("(root)", 0) },
              { id: 2, parent: 1, callFrame: frame("performWorkOnRoot", 12_068) },
              { id: 3, parent: 2, callFrame: frame("ViewStage", 40) },
              { id: 4, parent: 1, callFrame: frame("flushPassiveEffects", 13_228) },
            ],
            samples: [3, 2, 4, 1],
          },
          timeDeltas: [100, 300, 200, 100],
        },
      },
    });
    const main = scan
      .figures(0, 10_000)
      .threads.find((thread) => thread.thread === "CrRendererMain");
    expect(main?.reactMs).toEqual({ renderAndCommit: 0.5, passiveEffects: 0.1 });
  });

  it("sums the counted attachments' bytes a draw by pass, the most first, where no tile is kept (R14.T2.e)", () => {
    const figures = results().windows[0]?.attachments;
    expect([figures?.bytesLoadedPerDraw, figures?.bytesStoredPerDraw]).toEqual([40_000, 120_000]);
    expect(
      figures?.passes.map((pass) => [
        pass.label,
        pass.passesPerDraw,
        pass.bytesLoadedPerDraw,
        pass.bytesStoredPerDraw,
      ]),
    ).toEqual([
      ["terrain resolve", 1, 40_000, 40_000],
      ["bodies", 1, 0, 60_000],
      ["tonemap", 1, 0, 20_000],
    ]);
  });

  it("leaves a pass's bytes unknown where one of its attachments' is", () => {
    const unseen: InspectionGpuAttachment = {
      ...attached("canvas", "colour 0", "bgra8unorm", "load", "store", 0, 0),
      view: null,
      bytesLoaded: null,
      bytesStored: null,
    };
    const figures = gpuAttachmentFigures(
      [unseen, attached("canvas", "depth", "depth32float", "clear", "store", 0, 40_000)],
      2,
    );
    expect([figures.bytesLoadedPerDraw, figures.passes[0]?.bytesStoredPerDraw]).toEqual([
      null,
      null,
    ]);
  });

  it("writes each window's atmosphere reads and attachments in its summary (R14.T2.e)", () => {
    const markdown = inspectionTimingMarkdown(results());
    expect(markdown).toContain(
      "- **Atmosphere read** (each body with air, from the kernels its frames dispatched): air 1: deck, 2 frames; air 2: thin (thermal), 1 frame",
    );
    expect(markdown).toContain(
      "| terrain resolve | 1.0 | 0.04 | 0.04 | colour 0 rgba16float 100 × 50 load/store |",
    );
  });

  it("takes a window's attachments, or none, and its reads only named (R14.T2.e)", () => {
    const [window] = RECORD.windows;
    const read = (changes: object): unknown =>
      readInspectionTimingRecord({
        ...RECORD,
        windows: window === undefined ? [] : [{ ...window, ...changes }],
      })?.windows[0];
    expect([
      read({ gpuAttachments: null }) !== undefined,
      read({
        gpuAttachments: [
          {
            ...attached("bodies", "depth", "depth32float", "clear", "store", 0, 1),
            loadOp: "keep",
          },
        ],
      }),
      read({
        gpuAttachments: [
          {
            ...attached("bodies", "depth", "depth32float", "clear", "store", 0, 1),
            view: { format: "depth32float", widthPx: 100 },
          },
        ],
      }),
      read({
        gpuAttachments: [
          {
            ...attached("bodies", "depth", "depth32float", "clear", "store", 0, 1),
            attachment: "colour",
          },
        ],
      }),
      read({
        atmosphereReads: [{ air: 1, read: "dense", thermal: false, frames: 1, kernels: [] }],
      }),
      read({ atmosphereReads: undefined }),
    ]).toEqual([true, undefined, undefined, undefined, undefined, undefined]);
  });

  it("leaves the WebGPU calls out of a window that did not count them", () => {
    const [window] = RECORD.windows;
    const uncounted = window === undefined ? [] : [{ ...window, gpuCalls: null }];
    expect(
      readInspectionTimingRecord({ ...RECORD, windows: uncounted })?.windows[0]?.gpuCalls,
    ).toBe(null);
    const twice = [
      { label: "bodies", call: "render pass.draw", count: 1, bytes: 0 },
      { label: "bodies", call: "render pass.draw", count: 2, bytes: 0 },
    ];
    const repeated = window === undefined ? [] : [{ ...window, gpuCalls: twice }];
    expect(readInspectionTimingRecord({ ...RECORD, windows: repeated })).toBeNull();
  });

  it("takes a window's cluster counters, or none, and refuses them malformed (plan R14, R14.T3.d)", () => {
    const [window] = RECORD.windows;
    if (window === undefined) {
      throw new Error("the record has a window");
    }
    const withClusters = (clusters: unknown): unknown => ({
      ...RECORD,
      windows: [{ ...window, clusters }],
    });
    expect([
      readInspectionTimingRecord(withClusters(null))?.windows[0]?.clusters,
      readInspectionTimingRecord(
        withClusters({ passes: [{ label: "lit", clusters: -1, triangles: 0 }] }),
      ),
      readInspectionTimingRecord(
        withClusters({
          passes: [
            { label: "lit", clusters: 1, triangles: 2 },
            { label: "lit", clusters: 1, triangles: 2 },
          ],
        }),
      ),
      readInspectionTimingRecord(withClusters(undefined)),
      results().windows[0]?.clusters,
    ]).toEqual([null, null, null, null, window.clusters]);
    expect(inspectionTimingMarkdown(results())).toContain(
      "- **Clusters** (the terrain's cluster counters as the window ended, by pass): lit 900 (100000 triangles), lit late 40 (5000 triangles)",
    );
  });

  it("counts the terrain's two rasters, its pyramid, its cluster dispatches and the visibility buffer's rasters and resolve in its share (plan R14, R14.T3.d, T2.c and T2.d)", () => {
    expect(
      [
        "bodies",
        "bodies late",
        "depth pyramid",
        "depth pyramid #2",
        "terrain cluster cull",
        "terrain cluster cull #2",
        "terrain cluster compact #3",
        "terrain shadows #4",
        "symbology",
        "terrain id raster",
        "terrain id raster late",
        "terrain resolve #2",
        "bodies 2",
        "discs",
        "atmosphere view",
        "terrain resolver",
      ].map((label) => TERRAIN_PASS_LABELS.test(label)),
    ).toEqual([
      true,
      true,
      true,
      true,
      true,
      true,
      true,
      true,
      true,
      true,
      true,
      true,
      false,
      false,
      false,
      false,
    ]);
  });

  it("takes the run's long tasks, or none, and refuses them malformed or out of order (lane perf-page-long-tasks)", () => {
    const tasks: InspectionLongTasks = {
      observed: { longtask: true, longAnimationFrame: true },
      fromMs: 0,
      toMs: 10_000,
      tasks: [
        { startMs: 1_000, durationMs: 950, renderMs: 0 },
        { startMs: 3_000, durationMs: 60, renderMs: 30 },
      ],
    };
    const [first, second] = tasks.tasks;
    if (first === undefined || second === undefined) {
      throw new Error("the record holds two tasks");
    }
    expect(readInspectionTimingRecord({ ...RECORD, longTasks: tasks })?.longTasks).toEqual(tasks);
    expect(readInspectionTimingRecord({ ...RECORD, longTasks: null })?.longTasks).toBeNull();
    expect(
      [
        { ...tasks, toMs: -1 },
        { ...tasks, tasks: [second, first] },
        { ...tasks, tasks: [{ ...first, renderMs: 951 }] },
        { ...tasks, tasks: [{ ...first, durationMs: Number.NaN }] },
        { ...tasks, observed: { longtask: true } },
        undefined,
      ].map((longTasks) => readInspectionTimingRecord({ ...RECORD, longTasks })),
    ).toEqual([null, null, null, null, null, null]);
  });

  it("names what ran in a long task from the page's profile: its entry and its hottest functions (lane perf-page-long-tasks)", () => {
    expect([
      longTaskFunctions(pageProfile(), 100, 2_100),
      longTaskFunctions(pageProfile(), 5_000, 5_300).entry,
      longTaskFunctions(pageProfile(), 6_000, 6_100),
    ]).toEqual([
      {
        sampledMs: 2_000,
        entry: "onMessage useTerrainSession-x.js:9",
        self: [
          { name: "writeBuffer", ms: 1_950 },
          { name: "upload useTerrainSession-x.js:20", ms: 50 },
        ],
      },
      "tick index-x.js:5",
      { sampledMs: 0, entry: null, self: [] },
    ]);
  });

  it("counts the long tasks, the stalling ones apart, sums them by what ran in them and places the longest in their windows (lane perf-page-long-tasks)", () => {
    const record: InspectionLongTasks = {
      observed: { longtask: true, longAnimationFrame: true },
      fromMs: 0,
      toMs: 60_000,
      tasks: [
        { startMs: 100, durationMs: 2_000, renderMs: 0 },
        { startMs: 3_000, durationMs: 80, renderMs: 70 },
        { startMs: 5_000, durationMs: 300, renderMs: 300 },
        { startMs: 6_000, durationMs: 50, renderMs: 0 },
      ],
    };
    const windows = [{ name: "orbit" as const, startMs: 4_000, endMs: 8_000 }];
    const figures = longTaskFigures(record, windows, pageProfile());
    expect([
      figures.profiled,
      figures.count,
      figures.stallingCount,
      figures.longestMs,
      figures.totalMs,
      figures.byTrigger,
      figures.longest.map((task) => [task.durationMs, task.window, task.functions?.entry]),
    ]).toEqual([
      true,
      4,
      1,
      2_000,
      2_430,
      [
        {
          trigger: "onMessage useTerrainSession-x.js:9 → writeBuffer",
          count: 1,
          totalMs: 2_000,
          longestMs: 2_000,
        },
        {
          trigger: "tick index-x.js:5 → draw useTerrainSession-x.js:40",
          count: 1,
          totalMs: 300,
          longestMs: 300,
        },
        { trigger: "(no entry) → (not sampled)", count: 2, totalMs: 130, longestMs: 80 },
      ],
      [
        [2_000, null, "onMessage useTerrainSession-x.js:9"],
        [300, "orbit", "tick index-x.js:5"],
        [80, null, null],
        [50, "orbit", null],
      ],
    ]);
    const unprofiled = longTaskFigures(record, windows, null);
    expect([unprofiled.profiled, unprofiled.byTrigger.map((each) => each.trigger)]).toEqual([
      false,
      ["(not profiled)", "(rendering update)"],
    ]);
  });

  it("writes the long tasks' section of the summary, or none without them (lane perf-page-long-tasks)", () => {
    const withTasks = results(null, null, null, null, {
      ...RECORD,
      longTasks: {
        observed: { longtask: true, longAnimationFrame: false },
        fromMs: 0,
        toMs: 120_000,
        tasks: [{ startMs: 1_500, durationMs: 1_250, renderMs: 0 }],
      },
    });
    const markdown = inspectionTimingMarkdown(withTasks);
    expect(markdown).toContain(
      "- **Over the run:** 1 tasks of 50 ms or more in 120 s (the page reported no long animation frames), 1.3 s in all; 1 of 1 s or more; the longest 1250 ms, the median 1250 ms and the p95 1250 ms",
    );
    expect(markdown).toContain("| (not profiled) | 1 | 1250 | 1250 |");
    expect(markdown).toContain("| 1.5 | 1250 | close | 0 | — |");
    const attributed = inspectionTimingMarkdown(
      results(
        null,
        null,
        null,
        null,
        {
          ...RECORD,
          longTasks: {
            observed: { longtask: true, longAnimationFrame: true },
            fromMs: 0,
            toMs: 120_000,
            tasks: [{ startMs: 100, durationMs: 2_000, renderMs: 0 }],
          },
        },
        pageProfile(),
      ),
    );
    expect(attributed).toContain(
      "| 0.1 | 2000 | close | 0 | onMessage useTerrainSession-x.js:9; 1950 `writeBuffer`; 50 `upload useTerrainSession-x.js:20` |",
    );
    expect(inspectionTimingMarkdown(results())).not.toContain("long tasks");
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
    expect(markdown).toContain(
      "| Renderer / CrRendererMain | 1500 | 750.0 | 75.0% | 96.0 | 40 | 1400 |",
    );
    expect(markdown).toContain(
      "| 900 | `surveyEdges styles-x.js:31` | 1300 | `tick index-x.js:5` |",
    );
    expect(markdown).toContain("- **Statements:** ATMOSPHERE: APPROXIMATE · TERRAIN: STREAMING");
  });

  it("writes each window's march census: its texels by node rays and the tiles that wait (R08.T8.d)", () => {
    expect(inspectionTimingMarkdown(results())).toContain(
      "The march's far limb, the frame after the window (`atmosphere ray march`): texels by node rays 0: 900, 2: 60, 10: 40; 30 at w_L 1, with no centre ray; 600 centre rays in the air; 4 of 20 tiles hold a node ray; 1150 rays in the air, 2300 as each tile waits on its slowest texel (2.00 times).",
    );
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
      version: 9,
      run: { compositeProbe: null, marchProbe: null, atmosphereLevers: null, litProbe: null },
    });
  });

  it("names and states a run drawn under a composite probe (the owner's priority)", async () => {
    const written = new Map<string, string>();
    const files = {
      mkdir: () => Promise.resolve(),
      exists: (path: string) => Promise.resolve(written.has(path)),
      writeFile: (path: string, text: string) => {
        written.set(path, text);
        return Promise.resolve();
      },
    };
    const probed = results("transmittance-off");
    const paths = await writeInspectionTimingResults("/timing", probed, files);
    expect({
      json: paths.json,
      launch: inspectionTimingMarkdown(probed)
        .split("\n")
        .find((line) => line.startsWith("- **Launch:**")),
      plain: inspectionTimingMarkdown(results()).includes("composite probe"),
    }).toEqual({
      json: "/timing/2026-11-01-pod-low-inspection-timing-composite-transmittance-off.json",
      launch:
        "- **Launch:** linux, software mode, setting low, window hidden 1280 × 720 DIP at a device-pixel ratio of 1; pacing rate, offscreen frame clock 120 Hz; pass timer quantized; composite probe transmittance-off, not the picture a player sees",
      plain: false,
    });
  });

  it("names and states a run drawn with its atmosphere's levers over its setting's (R08.T8.d)", async () => {
    const written = new Map<string, string>();
    const files = {
      mkdir: () => Promise.resolve(),
      exists: (path: string) => Promise.resolve(written.has(path)),
      writeFile: (path: string, text: string) => {
        written.set(path, text);
        return Promise.resolve();
      },
    };
    const both = results(null, null, { marchCover: "tiles", limbRays: "packed" });
    const one = results(null, null, { limbRays: "per-texel" });
    const sources = results(null, null, { marchSourceTransmittance: "per-step" });
    expect([
      (await writeInspectionTimingResults("/timing", both, files)).json,
      (await writeInspectionTimingResults("/timing", one, files)).json,
      (await writeInspectionTimingResults("/timing", sources, files)).json,
      inspectionTimingMarkdown(both)
        .split("\n")
        .find((line) => line.startsWith("- **Launch:**"))
        ?.endsWith("; atmosphere levers march cover tiles, limb rays packed over the setting's"),
      inspectionTimingMarkdown(sources)
        .split("\n")
        .find((line) => line.startsWith("- **Launch:**"))
        ?.endsWith("; atmosphere levers march sources per-step over the setting's"),
    ]).toEqual([
      "/timing/2026-11-01-pod-low-inspection-timing-cover-tiles-limb-packed.json",
      "/timing/2026-11-01-pod-low-inspection-timing-limb-per-texel.json",
      "/timing/2026-11-01-pod-low-inspection-timing-sources-per-step.json",
      true,
      true,
    ]);
  });

  it("names and states a run drawn under the march's probe (R08.T8.d)", async () => {
    const written = new Map<string, string>();
    const files = {
      mkdir: () => Promise.resolve(),
      exists: (path: string) => Promise.resolve(written.has(path)),
      writeFile: (path: string, text: string) => {
        written.set(path, text);
        return Promise.resolve();
      },
    };
    const probed = results("per-pixel", "centre");
    const paths = await writeInspectionTimingResults("/timing", probed, files);
    expect({
      json: paths.json,
      launch: inspectionTimingMarkdown(probed)
        .split("\n")
        .find((line) => line.startsWith("- **Launch:**"))
        ?.endsWith(
          "; composite probe per-pixel, not the picture a player sees; march probe centre, not the picture a player sees",
        ),
      plain: inspectionTimingMarkdown(results()).includes("march probe"),
    }).toEqual({
      json: "/timing/2026-11-01-pod-low-inspection-timing-composite-per-pixel-march-centre.json",
      launch: true,
      plain: false,
    });
  });

  it("names and states a run drawn under a lit probe (R08.T9.b)", async () => {
    const written = new Map<string, string>();
    const files = {
      mkdir: () => Promise.resolve(),
      exists: (path: string) => Promise.resolve(written.has(path)),
      writeFile: (path: string, text: string) => {
        written.set(path, text);
        return Promise.resolve();
      },
    };
    const probed = results(null, null, { limbRays: "packed" }, "transmittance-one");
    const paths = await writeInspectionTimingResults("/timing", probed, files);
    expect({
      json: paths.json,
      launch: inspectionTimingMarkdown(probed)
        .split("\n")
        .find((line) => line.startsWith("- **Launch:**"))
        ?.endsWith(
          "; atmosphere levers limb rays packed over the setting's; lit probe transmittance-one, not the picture a player sees",
        ),
      plain: inspectionTimingMarkdown(results()).includes("lit probe"),
    }).toEqual({
      json: "/timing/2026-11-01-pod-low-inspection-timing-limb-packed-lit-transmittance-one.json",
      launch: true,
      plain: false,
    });
  });
});
