import { describe, expect, it } from "vitest";

import type { InspectionTimingMarchCensus, InspectionTimingRecord } from "../preload/api";
import {
  buildInspectionTimingResults,
  gpuCallKind,
  inspectionTimingMarkdown,
  inspectionTimingTraceConfig,
  type InspectionTimingResults,
  isMarchKernel,
  ProfileScan,
  profileFigures,
  summariseTimes,
  writeInspectionTimingResults,
  TERRAIN_PASS_LABELS,
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
};

/**
 * The timing file of {@link RECORD}, its trace a main thread busy 1,500 ms, drawn under
 * `compositeProbe`, `marchProbe`, `atmosphereLevers` and `litProbe`.
 */
function results(
  compositeProbe: InspectionTimingResults["run"]["compositeProbe"] = null,
  marchProbe: InspectionTimingResults["run"]["marchProbe"] = null,
  atmosphereLevers: InspectionTimingResults["run"]["atmosphereLevers"] = null,
  litProbe: InspectionTimingResults["run"]["litProbe"] = null,
): InspectionTimingResults {
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
    compositeProbe,
    marchProbe,
    atmosphereLevers,
    litProbe,
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

  it("counts the terrain's two rasters, its pyramid and its cluster dispatches in its share (plan R14, R14.T3.d)", () => {
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
        "bodies 2",
        "discs",
        "atmosphere view",
      ].map((label) => TERRAIN_PASS_LABELS.test(label)),
    ).toEqual([true, true, true, true, true, true, true, true, true, false, false, false]);
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
      version: 4,
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
        "- **Launch:** linux, software mode, setting low, window hidden 1280 × 720 DIP at a device-pixel ratio of 1; pass timer quantized; composite probe transmittance-off, not the picture a player sees",
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
