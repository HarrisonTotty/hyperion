import { describe, expect, it } from "vitest";

import type {
  ViewsCheckPhaseName,
  ViewsCheckPhaseRecord,
  ViewsCheckRecord,
  ViewsCheckStyle,
} from "../preload/api";
import { measured, missing, type ResultsFiles } from "./results";
import {
  buildViewsCheckResults,
  ENGINE_OWNER,
  LOW_BEFORE_T17,
  NO_WINDOW_REASON,
  PHASE_VIEWS,
  type PhaseTraceFigures,
  type ViewsCheckInput,
  viewsCheckMarkdown,
  writeViewsCheckResults,
} from "./viewsCheckResults";

const VSYNC_HZ = 59.94;
const T_MS = 1000 / VSYNC_HZ;

const NAMES = ["view", "instrument-1", "instrument-2"] as const;

/** A phase's record as the renderer would send it, each view drawing its phase's style. */
function phaseRecord(
  name: ViewsCheckPhaseName,
  styles: ReadonlyArray<ViewsCheckStyle> = PHASE_VIEWS[name],
): ViewsCheckPhaseRecord {
  return {
    name,
    startMs: 1000,
    endMs: 21_000,
    views: styles.map((style, i) => ({
      name: NAMES[i] ?? "view",
      style,
      canvas: i === 0 ? { widthPx: 1126, heightPx: 906 } : { widthPx: 240, heightPx: 180 },
      draws: i === 0 ? 100 : 50,
      gpuMs: [i === 0 ? 2 : 0.05],
      submitMs: [0.1],
      passLabels: style === "photorealistic" ? ["bloom", "tonemap"] : ["view:wireframe"],
      scales: style === "photorealistic" && i === 0 ? [1, 0.8] : [],
    })),
    frameIntervalsMs: Array.from({ length: 99 }, () => T_MS),
    primaryIntervalsMs: Array.from({ length: 99 }, () => T_MS),
    mainThreadMs: [4, 5, 6],
    frameGpuMs: [2, 2.1, 2.2],
    untimedFrames: 0,
    droppedResolves: 0,
    unattributedGpuMs: 0,
  };
}

const PHASES: ReadonlyArray<ViewsCheckPhaseName> = [
  "photoreal-alone",
  "photoreal-two-wireframe",
  "wireframe-two-wireframe",
  "wireframe-photoreal-wireframe",
  "wireframe-alone",
];

const RECORD: ViewsCheckRecord = {
  timer: "full",
  devicePixelRatio: 1,
  phases: PHASES.map((name) => phaseRecord(name)),
  resize: {
    before: [
      { name: "view", canvas: { widthPx: 1000, heightPx: 900 } },
      { name: "instrument-1", canvas: { widthPx: 240, heightPx: 180 } },
      { name: "instrument-2", canvas: { widthPx: 240, heightPx: 180 } },
    ],
    steps: [0.8, 0.65, 1].map((widthFraction) => ({
      widthFraction,
      longestFrameMs: 40,
      views: [
        { name: "view", canvas: { widthPx: Math.round(1000 * widthFraction), heightPx: 900 } },
        { name: "instrument-1", canvas: { widthPx: 240, heightPx: 180 } },
        { name: "instrument-2", canvas: { widthPx: 240, heightPx: 180 } },
      ],
    })),
    allocations: [
      { kind: "destroyed", name: "view:depth" },
      { kind: "created", name: "view:depth" },
      { kind: "created", name: "view:hdr:colour" },
      { kind: "created", name: "pass times readback 46" },
    ],
  },
  faults: [],
  perCanvasOverheadMs: 0.3,
};

/** Each phase's trace: presentations at T, and the GPU process busier with the instruments. */
function traces(
  intervalsMs: ReadonlyArray<number> = [T_MS, T_MS, T_MS],
): ReadonlyMap<ViewsCheckPhaseName, PhaseTraceFigures> {
  return new Map(
    PHASES.map((name) => [
      name,
      {
        presentation: measured({ intervalsMs, dropped: 0 }),
        gpuProcess: measured({
          busyMs: PHASE_VIEWS[name].length === 3 ? 300 : 200,
          slices: [{ name: "WebGPU", count: 10, totalMs: 50 }],
          copySlices: [],
        }),
      },
    ]),
  );
}

function input(overrides: Partial<ViewsCheckInput> = {}): ViewsCheckInput {
  return {
    run: {
      startedAt: new Date("2026-10-05T12:00:00Z"),
      machine: {
        name: "effect",
        cpu: "AMD Ryzen 7 3700X",
        logicalCores: 16,
        memoryBytes: 32e9,
        governor: measured("schedutil"),
        loadAverage: [0.4, 0.5, 0.6],
        gpu: measured({ vendorId: 4318, deviceId: 8710, driverVersion: "615", description: null }),
      },
      versions: { app: "0.1.0", electron: "44", chromium: "152", node: "24", v8: "15" },
      platform: "linux",
      launchMode: "vulkan",
      setting: "high",
      smoke: false,
      switches: ["--use-angle=vulkan"],
      shown: true,
      displayHz: VSYNC_HZ,
      window: { widthDip: 1920, heightDip: 1080 },
      captures: measured("/repo/target/views-check/captures"),
    },
    record: RECORD,
    traces: traces(),
    warnings: { passTimerDrops: 0, untimedPasses: 0 },
    gpuProcessExits: 0,
    rightWayUp: measured("yes"),
    ...overrides,
  };
}

describe("the views check's results", () => {
  it("judges the cockpit's frame time from presentation times against T", () => {
    const { findings, phases } = buildViewsCheckResults(input());
    const cockpit = phases.find((phase) => phase.name === "photoreal-two-wireframe");
    expect(cockpit?.rateHz).toBe(60);
    expect(cockpit?.periodMs.value).toBeCloseTo(T_MS);
    expect(findings.frameTime.verdict).toBe("pass");
    expect(findings.frameTime.rows.map((row) => row.id)).toEqual([
      "p50",
      "p95",
      "p99",
      "missed",
      "hitches",
      "headroom-main",
      "headroom-gpu",
    ]);
  });

  it("fails the frame time on missed presentations", () => {
    const late = buildViewsCheckResults(input({ traces: traces([T_MS, 3.5 * T_MS, T_MS]) }));
    expect(late.findings.frameTime.verdict).toBe("fail");
  });

  /** The results of a low-setting run whose presentations come at 2T. */
  const lowResults = (): ReturnType<typeof buildViewsCheckResults> =>
    buildViewsCheckResults(
      input({ run: { ...input().run, setting: "low" }, traces: traces([2 * T_MS]) }),
    );

  it("reads a photorealistic primary on the low setting at 30 Hz against 35 ms", () => {
    const cockpit = lowResults().phases.find((phase) => phase.name === "photoreal-two-wireframe");
    expect(cockpit?.rateHz).toBe(30);
    expect(cockpit?.periodMs.value).toBeCloseTo(2 * T_MS);
    expect(cockpit?.criteria.rows.find((row) => row.id === "p95")?.limit).toBe(35);
  });

  it("reads the low setting's wireframe primary at 60 Hz, as T20's low cases ask", () => {
    const results = lowResults();
    const wireframe = results.phases.find((phase) => phase.name === "wireframe-two-wireframe");
    expect(wireframe?.rateHz).toBe(60);
    expect(results.findings.lowCases.map((entry) => [entry.phase, entry.applies])).toEqual([
      ["wireframe-two-wireframe", true],
      ["wireframe-photoreal-wireframe", true],
    ]);
  });

  it("marks a low-setting run provisional until R07.T17", () => {
    expect(lowResults().run.quiet).toEqual({
      provisional: true,
      note: `provisional: ${LOW_BEFORE_T17}`,
    });
  });

  it("leaves a quiet, shown run on the high setting unmarked", () => {
    expect(buildViewsCheckResults(input()).run.quiet).toEqual({ provisional: false, note: null });
  });

  it("names the second low case's consequence where it misses alone", () => {
    const record: ViewsCheckRecord = {
      ...RECORD,
      phases: RECORD.phases.map((phase) =>
        phase.name === "wireframe-photoreal-wireframe"
          ? { ...phase, frameGpuMs: [100, 100, 100] }
          : phase,
      ),
    };
    const results = buildViewsCheckResults(
      input({ run: { ...input().run, setting: "low" }, record }),
    );
    expect(results.findings.lowCases.map((entry) => entry.triggered)).toEqual([false, true]);
  });

  it("leaves every row read against T unmeasured in a hidden run", () => {
    const hidden = buildViewsCheckResults(
      input({ run: { ...input().run, shown: false, displayHz: null } }),
    );
    expect(hidden.run.vsyncMs).toEqual(missing(NO_WINDOW_REASON));
    expect(hidden.findings.frameTime.verdict).toBe("not-measured");
    expect(hidden.run.quiet.provisional).toBe(true);
  });

  it("passes a resize that re-made the primary's attachments and none of the instruments'", () => {
    const { resize } = buildViewsCheckResults(input()).findings;
    expect(resize.verdict).toBe("pass");
    expect(resize.byView).toEqual([
      { view: "view", created: 2, destroyed: 1, names: ["view:depth", "view:hdr:colour"] },
      { view: ENGINE_OWNER, created: 1, destroyed: 0, names: ["pass times readback 46"] },
    ]);
  });

  it("fails a resize that touched an instrument's attachments", () => {
    const record: ViewsCheckRecord = {
      ...RECORD,
      resize: {
        ...RECORD.resize,
        allocations: [
          ...RECORD.resize.allocations,
          { kind: "created", name: "instrument-1:depth" },
        ],
      },
    };
    const { resize } = buildViewsCheckResults(input({ record })).findings;
    expect(resize.verdict).toBe("fail");
    expect(resize.othersUnchanged).toBe(false);
  });

  it("fails a resize whose primary did not change size", () => {
    const record: ViewsCheckRecord = {
      ...RECORD,
      resize: {
        ...RECORD.resize,
        steps: RECORD.resize.steps.map((step) => ({ ...step, views: RECORD.resize.before })),
      },
    };
    expect(buildViewsCheckResults(input({ record })).findings.resize.primaryResized).toBe(false);
  });

  it("finds a view's pass named as a copy", () => {
    const record: ViewsCheckRecord = {
      ...RECORD,
      phases: RECORD.phases.map((phase) => ({
        ...phase,
        views: phase.views.map((view) => ({ ...view, passLabels: [...view.passLabels, "copy"] })),
      })),
    };
    const { copies } = buildViewsCheckResults(input({ record })).findings;
    expect(copies).toMatchObject({ verdict: "fail", copyPasses: ["copy"] });
  });

  it("reads the per-canvas overhead from the GPU process's time a primary draw", () => {
    const { perCanvasOverhead } = buildViewsCheckResults(input()).findings;
    // 300 ms against 200 ms over 100 primary draws, for two canvases.
    expect(perCanvasOverhead.pairs[0]?.perCanvasMs.value).toBeCloseTo(0.5);
    expect(perCanvasOverhead.measuredMs.value).toBeCloseTo(0.5);
    expect(perCanvasOverhead.provisionalMs).toBe(0.3);
    expect(perCanvasOverhead.pairs[0]?.instruments.map((each) => each.name)).toEqual([
      "instrument-1",
      "instrument-2",
    ]);
  });

  it("fails the pass timer on a console warning", () => {
    expect(
      buildViewsCheckResults(input({ warnings: { passTimerDrops: 1, untimedPasses: 0 } })).findings
        .passTimer.verdict,
    ).toBe("fail");
  });

  it("fails the pass timer on a dropped resolve", () => {
    const record: ViewsCheckRecord = {
      ...RECORD,
      phases: RECORD.phases.map((phase) => ({ ...phase, droppedResolves: 1 })),
    };
    expect(buildViewsCheckResults(input({ record })).findings.passTimer).toMatchObject({
      verdict: "fail",
      droppedResolves: 5,
    });
  });

  it.each([
    [measured("yes" as const), "pass"],
    [measured("no" as const), "fail"],
    [missing("not answered"), "not-measured"],
  ])("judges the answer %j as %s", (rightWayUp, verdict) => {
    expect(buildViewsCheckResults(input({ rightWayUp })).findings.rightWayUp.verdict).toBe(verdict);
  });

  it("does not judge a phase that did not hold its configuration", () => {
    const record: ViewsCheckRecord = {
      ...RECORD,
      phases: [
        phaseRecord("photoreal-two-wireframe", ["photorealistic", "photorealistic", "wireframe"]),
      ],
    };
    expect(buildViewsCheckResults(input({ record })).findings.frameTime.verdict).toBe(
      "not-measured",
    );
  });

  it("reads no overhead from a phase that did not hold its configuration", () => {
    const record: ViewsCheckRecord = {
      ...RECORD,
      phases: RECORD.phases.map((phase) =>
        phase.name === "photoreal-two-wireframe"
          ? phaseRecord(phase.name, ["photorealistic", "photorealistic", "wireframe"])
          : phase,
      ),
    };
    const pair = buildViewsCheckResults(input({ record })).findings.perCanvasOverhead.pairs[0];
    expect(pair?.perCanvasMs.reason).toMatch(/did not hold/);
  });

  it("holds a phase only where every view drew the phase's style", () => {
    const record: ViewsCheckRecord = {
      ...RECORD,
      phases: [
        phaseRecord("photoreal-two-wireframe", ["photorealistic", "photorealistic", "wireframe"]),
      ],
    };
    expect(buildViewsCheckResults(input({ record })).phases[0]?.held).toBe(false);
  });

  it("summarises T20's checks for a person", () => {
    const text = viewsCheckMarkdown(buildViewsCheckResults(input()));
    expect(text).toContain("# Several views: effect, high, 2026-10-05");
    expect(text).toContain("| No GPU time in copies | pass |");
    expect(text).toContain(
      "| A resize of the primary leaves the instruments' attachments alone | pass |",
    );
    expect(text).toContain('- [x] Each view the right way up: answered "yes" at the run.');
  });
});

/** Files in memory: those that exist already, and every path written. */
function memoryFiles(
  existing: ReadonlyArray<string>,
): ResultsFiles & { readonly written: string[] } {
  const written: string[] = [];
  return {
    written,
    mkdir: () => Promise.resolve(),
    exists: (path) => Promise.resolve(existing.includes(path)),
    writeFile: (path) => {
      written.push(path);
      return Promise.resolve();
    },
  };
}

describe("writeViewsCheckResults", () => {
  it("names the files by date, machine and setting, numbering a second run", async () => {
    const results = buildViewsCheckResults(input());
    const files = memoryFiles(["/out/2026-10-05-effect-high.json"]);
    const paths = await writeViewsCheckResults("/out", results, files);
    expect(paths).toEqual({
      json: "/out/2026-10-05-effect-high-2.json",
      markdown: "/out/2026-10-05-effect-high-2.md",
    });
    expect(files.written).toEqual([paths.json, paths.markdown]);
  });

  it("marks a smoke run's files", async () => {
    const results = buildViewsCheckResults(input({ run: { ...input().run, smoke: true } }));
    const paths = await writeViewsCheckResults("/out", results, memoryFiles([]));
    expect(paths.json).toBe("/out/2026-10-05-effect-high-smoke.json");
  });
});
