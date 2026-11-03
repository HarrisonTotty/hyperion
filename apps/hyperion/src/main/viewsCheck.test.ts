import { join } from "node:path";

import { describe, expect, it, vi } from "vitest";

import type {
  AtmosphereCheckRecord,
  GraphicsLaunchMode,
  HandoverCheckRecord,
  InspectionTimingRecord,
  TerrainCheckRecord,
  ViewsCheckLaunch,
  ViewsCheckRecord,
  ViewsCheckRunKind,
} from "../preload/api";
import type { AtmosphereCheckResults } from "./atmosphereCheckResults";
import { ATMOSPHERE_RECORD } from "./fixtures/atmosphereRecord";
import { HANDOVER_RECORD } from "./fixtures/handoverRecord";
import { TERRAIN_RECORD } from "./fixtures/terrainRecord";
import type { HandoverCheckResults } from "./handoverCheckResults";
import {
  INSPECTION_TIMING_TRACE_CATEGORIES,
  type InspectionTimingResults,
  ProfileScan,
  type ReducedProfile,
} from "./inspectionTiming";
import type { TraceFigures } from "./reduceTrace";
import { measured } from "./results";
import {
  phaseTraceFigures,
  type ReducedWindow,
  reduceWindowFile,
  registerViewsCheckHandlers,
  VIEWS_CHECK_CHANNELS,
  VIEWS_CHECK_TRACE_CATEGORIES,
  viewsCheckDirs,
  viewsCheckTraceConfig,
  ViewsCheckSession,
  type ViewsCheckSessionDeps,
  viewsCheckWatchdogMs,
  WindowScan,
} from "./viewsCheck";
import type { TerrainCheckResults, TerrainPhasesResults } from "./terrainCheckResults";
import { NO_WINDOW_REASON, type ViewsCheckResults } from "./viewsCheckResults";

/**
 * The descent spike's recorded traces (`traceProto.test.ts` and `reduceTrace.test.ts` give their
 * provenance), taken with the same five categories as a phase's window.
 */
const FIXTURES = join(__dirname, "fixtures");

const RECORD: ViewsCheckRecord = {
  timer: "full",
  devicePixelRatio: 1,
  phases: [],
  resize: { before: [], steps: [], allocations: [] },
  faults: [],
  perCanvasOverheadMs: 0.3,
};

/** The trace's clock less the page's, µs: page time t ms is trace time 10⁶ + 1000 t µs. */
const OFFSET_US = 1_000_000;

/** A page time as the trace's, µs. */
const us = (pageMs: number): number => OFFSET_US + 1000 * pageMs;

/** The phase's measured window, page ms. */
const SPAN = { startMs: 100, endMs: 200 } as const;

/** A trace's figures: presentations about the window, and a GPU process of pid 7, thread 8. */
function figures(clockOffsetUs: number | null = OFFSET_US): TraceFigures {
  return {
    span: { firstUs: us(0), lastUs: us(300) },
    clockOffsetUs,
    frames: {
      pid: 3,
      layerTreeHostId: 1,
      presentedAtUs: [us(50), us(101), us(117.7), us(134.4), us(250)],
      intervalsMs: [],
      presented: 5,
      dropped: 2,
      droppedAtUs: [us(150), us(260)],
      noUpdate: 0,
    },
    mainThread: null,
    gpuProcess: { pid: 7, tid: 8, busyMs: 999, slices: [] },
    threads: [],
    userTiming: [],
  };
}

/** The scan of a trace with the GPU process's tasks and slices about the window. */
function scanned(): WindowScan {
  const scan = new WindowScan();
  // 10 ms of a task that starts before the window, 10 ms inside it, one after it, another thread's.
  scan.add({ name: "RunTask", ph: "X", pid: 7, tid: 8, ts: us(90), dur: 20_000 });
  scan.add({ name: "RunTask", ph: "X", pid: 7, tid: 8, ts: us(150), dur: 10_000 });
  scan.add({ name: "RunTask", ph: "X", pid: 7, tid: 8, ts: us(250), dur: 10_000 });
  scan.add({ name: "RunTask", ph: "X", pid: 3, tid: 4, ts: us(150), dur: 10_000 });
  scan.add({ name: "GPUTask", ph: "X", pid: 7, tid: 8, ts: us(150), dur: 4_000 });
  scan.add({ name: "CopySharedImage", ph: "X", pid: 7, tid: 8, ts: us(160), dur: 1_500 });
  scan.add({ name: "BlitFramebuffer", ph: "B", pid: 7, tid: 8, ts: us(170) });
  scan.add({ name: "CopyOutputRequest", ph: "X", pid: 3, tid: 4, ts: us(160), dur: 1_000 });
  scan.add("not an event");
  return scan;
}

const WINDOW: ReducedWindow = { figures: figures(), scan: scanned() };

/** A profiled window's trace (the inspection run's `--timing`): one renderer task in its span. */
function profiled(): ReducedProfile {
  const scan = new ProfileScan();
  scan.add({ name: "process_name", ph: "M", pid: 1, tid: 1, args: { name: "Renderer" } });
  scan.add({ name: "thread_name", ph: "M", pid: 1, tid: 1, args: { name: "CrRendererMain" } });
  scan.add({ name: "RunTask", ph: "X", pid: 1, tid: 1, ts: us(SPAN.startMs), dur: 4_000 });
  return { clockOffsetUs: OFFSET_US, scan };
}

const PROFILE: ReducedProfile = profiled();

function session(overrides: Partial<ViewsCheckSessionDeps> = {}): {
  readonly session: ViewsCheckSession;
  readonly deps: ViewsCheckSessionDeps;
  readonly written: ViewsCheckResults[];
  readonly terrainWritten: TerrainCheckResults[];
  readonly terrainPhasesWritten: TerrainPhasesResults[];
  readonly handoverWritten: HandoverCheckResults[];
  readonly atmosphereWritten: AtmosphereCheckResults[];
} {
  const written: ViewsCheckResults[] = [];
  const terrainWritten: TerrainCheckResults[] = [];
  const terrainPhasesWritten: TerrainPhasesResults[] = [];
  const handoverWritten: HandoverCheckResults[] = [];
  const atmosphereWritten: AtmosphereCheckResults[] = [];
  const deps: ViewsCheckSessionDeps = {
    launch: {
      setting: "high",
      smoke: false,
      out: null,
      run: "several-views",
      fixture: "terrain-test",
    },
    describe: () =>
      Promise.resolve({
        startedAt: new Date("2026-10-05T12:00:00Z"),
        machine: {
          name: "effect",
          cpu: "cpu",
          logicalCores: 16,
          memoryBytes: 1,
          governor: measured("performance"),
          loadAverage: [0.1, 0.1, 0.1],
          gpu: measured({ vendorId: 1, deviceId: 2, driverVersion: null, description: null }),
        },
        versions: { app: "0", electron: "0", chromium: "0", node: "0", v8: "0" },
        platform: "linux",
        launchMode: "vulkan",
        setting: "high",
        smoke: false,
        switches: [],
        shown: true,
        displayHz: 59.94,
      }),
    windowSize: () => ({ widthDip: 2458, heightDip: 1382 }),
    tracing: {
      startRecording: vi.fn<ViewsCheckSessionDeps["tracing"]["startRecording"]>(() =>
        Promise.resolve(),
      ),
      stopRecording: vi.fn<ViewsCheckSessionDeps["tracing"]["stopRecording"]>((path) =>
        Promise.resolve(path ?? "/trace.json"),
      ),
    },
    tracePath: (phase) => `/profile/${phase}.json`,
    reduce: () => Promise.resolve(WINDOW),
    profilePath: (name) => `/profile/timing-${name}.pftrace`,
    reduceProfile: () => Promise.resolve(PROFILE),
    timingDir: "/timing",
    removeFile: vi.fn<(path: string) => Promise<void>>(() => Promise.resolve()),
    capturesDir: "/captures",
    capture: vi.fn<(path: string) => Promise<void>>(() => Promise.resolve()),
    readText: (path) => Promise.resolve(`# generator_version = 24\nbody of ${path}\n`),
    ask: () => Promise.resolve("yes" as const),
    outDir: "/out",
    write: (_dir, results) => {
      written.push(results);
      return Promise.resolve({ json: "/out/a.json", markdown: "/out/a.md" });
    },
    terrainWriter: {
      writePhases: (phases) => {
        terrainPhasesWritten.push(phases);
        return Promise.resolve({ json: "/out/t-partial.json", markdown: "/out/t-partial.md" });
      },
      writeResults: (results) => {
        terrainWritten.push(results);
        return Promise.resolve({ json: "/out/t.json", markdown: "/out/t.md" });
      },
    },
    writeHandover: (_dir, results) => {
      handoverWritten.push(results);
      return Promise.resolve({ json: "/out/h.json", markdown: "/out/h.md" });
    },
    writeAtmosphere: (_dir, results) => {
      atmosphereWritten.push(results);
      return Promise.resolve({ json: "/out/a.json", markdown: "/out/a.md" });
    },
    exit: vi.fn<(code: number) => void>(),
    log: () => undefined,
    ...overrides,
  };
  return {
    session: new ViewsCheckSession(deps),
    deps,
    written,
    terrainWritten,
    terrainPhasesWritten,
    handoverWritten,
    atmosphereWritten,
  };
}

/** A terrain launch's session (R10.T16.d). */
function terrainSession(
  overrides: Partial<ViewsCheckSessionDeps> = {},
): ReturnType<typeof session> {
  return session({
    launch: { setting: "high", smoke: false, out: null, run: "terrain", fixture: "terrain-test" },
    ...overrides,
  });
}

/** A hand-over check's session (R10.T10.g). */
function handoverSession(
  overrides: Partial<ViewsCheckSessionDeps> = {},
): ReturnType<typeof session> {
  return session({
    launch: { setting: "high", smoke: false, out: null, run: "handover", fixture: "terrain-test" },
    ...overrides,
  });
}

/** An atmosphere run's session (R08.T10.b). */
function atmosphereSession(
  overrides: Partial<ViewsCheckSessionDeps> = {},
): ReturnType<typeof session> {
  return session({
    launch: {
      setting: "high",
      smoke: false,
      out: null,
      run: "atmosphere",
      fixture: "terrain-test",
    },
    ...overrides,
  });
}

describe("the views check's trace", () => {
  it("records R05's five timed categories, without gpu or the profiler", () => {
    expect(VIEWS_CHECK_TRACE_CATEGORIES).toEqual([
      "devtools.timeline",
      "disabled-by-default-devtools.timeline",
      "disabled-by-default-devtools.timeline.frame",
      "disabled-by-default-v8.gc",
      "blink.user_timing",
    ]);
    expect(viewsCheckTraceConfig().recording_mode).toBe("record-until-full");
  });

  it("takes the GPU process's busy time within the window, overlaps once", () => {
    expect(scanned().busyMs(7, 8, us(SPAN.startMs), us(SPAN.endMs))).toBeCloseTo(20);
  });

  it("gives a shown run the presentations and drops within its window", () => {
    const shown = phaseTraceFigures(WINDOW, true, SPAN);
    const presented = shown.presentation.value;
    expect(presented?.dropped).toBe(1);
    expect(presented?.intervalsMs.map((ms) => Number(ms.toFixed(1)))).toEqual([16.7, 16.7]);
  });

  it("gives the GPU process's figures within the window, its own copy slices alone", () => {
    const { gpuProcess } = phaseTraceFigures(WINDOW, true, SPAN);
    expect(gpuProcess.value?.busyMs).toBeCloseTo(20);
    expect(gpuProcess.value?.slices).toEqual([{ name: "GPUTask", count: 1, totalMs: 4 }]);
    expect(gpuProcess.value?.copySlices).toEqual([
      { name: "BlitFramebuffer", count: 1, totalMs: 0 },
      { name: "CopySharedImage", count: 1, totalMs: 1.5 },
    ]);
  });

  it("gives a hidden run no presentation", () => {
    expect(phaseTraceFigures(WINDOW, false, SPAN).presentation.reason).toBe(NO_WINDOW_REASON);
  });

  it("reads nothing from a trace whose clock it cannot set against the page's", () => {
    const blind = phaseTraceFigures({ figures: figures(null), scan: scanned() }, true, SPAN);
    expect(blind.presentation.reason).toMatch(/clock/);
    expect(blind.gpuProcess.reason).toMatch(/clock/);
  });

  it("reads a Perfetto protobuf window, the only format Electron 45's contentTracing writes", async () => {
    const { figures: read } = await reduceWindowFile(join(FIXTURES, "spike.pftrace"));
    expect(read.frames).toMatchObject({ presented: 28, dropped: 2, noUpdate: 0 });
  });

  it("still reads a JSON window, the format Electron 44's contentTracing wrote", async () => {
    const { figures: read } = await reduceWindowFile(join(FIXTURES, "spike.trace.json"));
    expect(read.frames.presented).toBe(1);
  });
});

describe("the views check's session", () => {
  it("traces each phase's window, reduces it, removes it and captures the page", async () => {
    const { session: run, deps } = session();
    await run.startPhase("photoreal-alone");
    await run.endPhase("photoreal-alone", SPAN);
    expect(deps.tracing.startRecording).toHaveBeenCalledTimes(1);
    expect(deps.tracing.stopRecording).toHaveBeenCalledWith("/profile/photoreal-alone.json");
    expect(deps.removeFile).toHaveBeenCalledWith("/profile/photoreal-alone.json");
    expect(deps.capture).toHaveBeenCalledWith("/captures/photoreal-alone.png");
  });

  it("refuses a second window while one records", async () => {
    const { session: run } = session();
    await run.startPhase("photoreal-alone");
    await expect(run.startPhase("wireframe-alone")).rejects.toThrow(/still recording/);
  });

  it("refuses to end a phase that is not recording", async () => {
    const { session: run } = session();
    await run.startPhase("photoreal-alone");
    await expect(run.endPhase("wireframe-alone", SPAN)).rejects.toThrow(/not the phase recording/);
  });

  it("keeps a trace that does not reduce as a missing figure with its reason", async () => {
    const { session: run, written } = session({
      reduce: () => Promise.reject(new Error("bad line")),
    });
    await run.startPhase("photoreal-alone");
    await run.endPhase("photoreal-alone", SPAN);
    await run.writeResults({
      ...RECORD,
      phases: [
        {
          name: "photoreal-alone",
          startMs: 0,
          endMs: 1000,
          views: [],
          frameIntervalsMs: [],
          primaryIntervalsMs: [],
          mainThreadMs: [],
          frameGpuMs: [],
          untimedFrames: 0,
          droppedResolves: 0,
          unattributedGpuMs: 0,
          statements: [],
        },
      ],
    });
    expect(written[0]?.phases[0]?.presentation.reason).toBe("the trace did not reduce: bad line");
  });

  it("counts the pass timer's console warnings into the results", async () => {
    const { session: run, written } = session();
    run.consoleMessage("135 resolves' pass times are still being read; dropping some");
    run.consoleMessage("more than 64 passes in a frame; the rest are not timed");
    run.consoleMessage("an unrelated warning");
    await run.writeResults(RECORD);
    expect(written[0]?.findings.passTimer).toMatchObject({
      consoleDrops: 1,
      untimedPassWarnings: 1,
      verdict: "fail",
    });
  });

  it("counts the GPU process's exits into the results", async () => {
    const { session: run, written } = session();
    run.gpuProcessGone();
    await run.writeResults(RECORD);
    expect(written[0]?.findings.faults.gpuProcessExits).toBe(1);
  });

  it("keeps the person's answer", async () => {
    const { session: run, written } = session();
    await run.askRightWayUp();
    await run.writeResults(RECORD);
    expect(written[0]?.findings.rightWayUp.answer).toEqual(measured("yes"));
  });

  it("asks nobody in a hidden run", async () => {
    const { session: run, written } = session({ ask: null });
    await run.askRightWayUp();
    await run.writeResults(RECORD);
    expect(written[0]?.findings.rightWayUp.answer.reason).toMatch(/hidden run/);
  });

  it("ends the app with the run's status", () => {
    const { session: run, deps } = session();
    run.end(1, "INSTRUMENT 1 did not open");
    expect(run.ended).toBe(true);
    expect(deps.exit).toHaveBeenCalledWith(1);
  });
});

/** A T20 session whose run describes itself in `launchMode`. */
function moded(
  launchMode: GraphicsLaunchMode,
  overrides: Partial<ViewsCheckSessionDeps> = {},
): ReturnType<typeof session> {
  const base = session();
  return session({
    describe: () => base.deps.describe().then((run) => ({ ...run, launchMode })),
    ...overrides,
  });
}

/** A phase's record whose primary drew `style`. */
function primaryPhase(style: "photorealistic" | "wireframe"): ViewsCheckRecord["phases"][number] {
  return {
    name: "photoreal-alone",
    startMs: 0,
    endMs: 1000,
    views: [
      {
        name: "view",
        style,
        canvas: { widthPx: 1280, heightPx: 720 },
        draws: 10,
        gpuMs: [],
        submitMs: [],
        passLabels: [],
        scales: [],
      },
    ],
    frameIntervalsMs: [],
    primaryIntervalsMs: [],
    mainThreadMs: [],
    frameGpuMs: [],
    untimedFrames: 0,
    droppedResolves: 0,
    unattributedGpuMs: 0,
    statements: [],
  };
}

describe("the views check's session in the software mode (R01.T16)", () => {
  it("ends with 0 when every primary was drawn as asked", async () => {
    const { session: run, deps } = moded("software");
    await run.writeResults({ ...RECORD, phases: [primaryPhase("photorealistic")] });
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(0);
  });

  it("ends with 1 when the GPU process exited, whatever its end says", async () => {
    const lines: string[] = [];
    const exit = vi.fn<(code: number) => void>();
    const { session: run } = moded("software", {
      exit,
      log: (line) => {
        lines.push(line);
      },
    });
    run.gpuProcessGone();
    await run.writeResults({ ...RECORD, phases: [primaryPhase("photorealistic")] });
    run.end(0, null);
    expect([exit.mock.calls, lines.at(-1)]).toEqual([
      [[1]],
      "views check: FAIL the software run's views were not as asked: 1 exits of the GPU process",
    ]);
  });

  it("ends with 1 when the GPU process exited after the results were written", async () => {
    const { session: run, deps } = moded("software");
    await run.writeResults({ ...RECORD, phases: [primaryPhase("photorealistic")] });
    run.gpuProcessGone();
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
  });

  it("ends with 1 when a primary went back to the wireframe", async () => {
    const { session: run, deps } = moded("software");
    await run.writeResults({ ...RECORD, phases: [primaryPhase("wireframe")] });
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
  });

  it("leaves a hardware run recorded, not judged, as before", async () => {
    const { session: run, deps } = moded("vulkan");
    run.gpuProcessGone();
    await run.writeResults({ ...RECORD, phases: [primaryPhase("wireframe")] });
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(0);
  });
});

describe("a views check's directories and watchdog", () => {
  const LAUNCHES: ReadonlyArray<ViewsCheckLaunch> = (
    ["several-views", "terrain", "handover", "atmosphere"] as const
  ).flatMap((run) =>
    [false, true].map((smoke) => ({
      setting: "high" as const,
      smoke,
      out: null,
      run,
      fixture: "terrain-test" as const,
    })),
  );

  it("writes a software run under the smoke's directory, a full run's included", () => {
    for (const launch of LAUNCHES) {
      const dirs = viewsCheckDirs(launch, "software");
      expect([dirs.out, dirs.out === dirs.captures]).toEqual([dirs.captures, true]);
      expect(dirs.out.startsWith("target/views-check")).toBe(true);
    }
  });

  it("writes every other mode's full run under docs/measurements/, as before", () => {
    for (const mode of ["default", "vulkan", "safe"] as const) {
      expect(
        LAUNCHES.map((launch) => viewsCheckDirs(launch, mode).out).filter((dir) =>
          dir.startsWith("docs/measurements/"),
        ),
      ).toEqual([
        "docs/measurements/several-views",
        "docs/measurements/terrain-check",
        "docs/measurements/terrain-check",
        "docs/measurements/atmosphere-flight",
      ]);
    }
  });

  it("gives a software run five times the watchdog, the atmosphere run's full run twice that (R08.T17)", () => {
    for (const launch of LAUNCHES) {
      const placed = launch.run === "atmosphere" && !launch.smoke;
      expect(viewsCheckWatchdogMs(launch, "software")).toBe(
        (placed ? 10 : 5) * viewsCheckWatchdogMs(launch, "vulkan"),
      );
    }
  });

  it("keeps every other mode's watchdog as before", () => {
    for (const mode of ["default", "vulkan", "safe"] as const) {
      expect(LAUNCHES.map((launch) => viewsCheckWatchdogMs(launch, mode) / 60_000)).toEqual([
        20, 4, 30, 8, 30, 8, 15, 8,
      ]);
    }
  });

  it("gives a served world's hand-over check 15 minutes more, five times in software", () => {
    const served: ViewsCheckLaunch = {
      setting: "high",
      smoke: false,
      out: null,
      run: "handover",
      fixture: "terrain-test",
      served: { universe: "u", body: "b" },
    };
    expect([
      viewsCheckWatchdogMs(served, "vulkan"),
      viewsCheckWatchdogMs(served, "software"),
    ]).toEqual([45 * 60_000, 225 * 60_000]);
  });
});

describe("the inspection run (the owner's request)", () => {
  const LAUNCH: ViewsCheckLaunch = {
    setting: "high",
    smoke: false,
    out: null,
    run: "inspection",
    fixture: "terrain-test",
    served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
  };

  /** An inspection launch's session. */
  function inspectionSession(
    overrides: Partial<ViewsCheckSessionDeps> = {},
  ): ReturnType<typeof session> {
    return session({ launch: LAUNCH, ...overrides });
  }

  it("keeps its captures under target/, whatever the mode", () => {
    for (const mode of ["default", "vulkan", "safe", "software"] as const) {
      expect(viewsCheckDirs(LAUNCH, mode)).toEqual({
        out: "target/views-check/inspection",
        captures: "target/views-check/inspection",
      });
    }
  });

  it("is given 15 minutes, a smoke 8, five times as long in software", () => {
    expect([
      viewsCheckWatchdogMs(LAUNCH, "vulkan"),
      viewsCheckWatchdogMs({ ...LAUNCH, smoke: true }, "vulkan"),
      viewsCheckWatchdogMs(LAUNCH, "software"),
    ]).toEqual([15 * 60_000, 8 * 60_000, 75 * 60_000]);
  });

  it("names its capture between phases and passes once it is saved", async () => {
    const { session: run, deps } = inspectionSession();
    await run.capture("inspection-61ffd967fe000023-0100");
    run.end(0, null);
    expect(deps.capture).toHaveBeenCalledWith(
      "/captures/between-inspection-61ffd967fe000023-0100.png",
    );
    expect(deps.exit).toHaveBeenCalledWith(0);
  });

  it("fails a run that ends with no capture saved", () => {
    const lines: string[] = [];
    const { session: run, deps } = inspectionSession({ log: (line) => lines.push(line) });
    run.end(0, null);
    expect(lines.at(-1)).toBe("views check: FAIL the inspection run ended with no capture saved");
    expect(deps.exit).toHaveBeenCalledWith(1);
  });
});

describe("the inspection run's frame profile (the owner's report)", () => {
  const LAUNCH: ViewsCheckLaunch = {
    setting: "high",
    smoke: false,
    out: null,
    run: "inspection",
    fixture: "terrain-test",
    served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
    timing: true,
  };

  const TIMING_RECORD: InspectionTimingRecord = {
    timer: "quantized",
    devicePixelRatio: 1,
    windows: [
      {
        name: "orbit",
        startMs: SPAN.startMs,
        endMs: SPAN.endMs,
        canvas: { widthPx: 1140, heightPx: 640 },
        scales: [1],
        intervalsMs: [50],
        mainThreadMs: [10, 12],
        submitMs: [3, 4],
        gpuMs: [40, 44],
        passes: [
          {
            label: "atmosphere view",
            ms: [30, 33],
            kernels: ["atmosphere ray march"],
            workgroups: [100, 100],
          },
        ],
        untimedFrames: 0,
        memory: {
          uploaded: { count: 4, bytes: 2_097_152 },
          created: { count: 1, bytes: 1_024 },
          destroyed: { count: 0, bytes: 0 },
          uploads: [{ name: "view star sprites", count: 4, bytes: 2_097_152 }],
        },
        gpuCalls: null,
        clusters: null,
        marchSamples: 36,
        position: "28.9 Mm 268° +49°",
        statements: ["ATMOSPHERE: APPROXIMATE"],
        marchCensus: null,
      },
    ],
  };

  /** A profiled inspection launch's session, its timing files kept. */
  function timingSession(overrides: Partial<ViewsCheckSessionDeps> = {}): {
    readonly run: ViewsCheckSession;
    readonly deps: ViewsCheckSessionDeps;
    readonly timed: InspectionTimingResults[];
  } {
    const timed: InspectionTimingResults[] = [];
    const { session: run, deps } = session({
      launch: LAUNCH,
      writeTiming: (_dir, results) => {
        timed.push(results);
        return Promise.resolve({ json: "/timing/a.json", markdown: "/timing/a.md" });
      },
      ...overrides,
    });
    return { run, deps, timed };
  }

  it("is given ten minutes more, under target/ whatever the mode", () => {
    expect([
      viewsCheckWatchdogMs(LAUNCH, "vulkan"),
      viewsCheckWatchdogMs(LAUNCH, "software"),
    ]).toEqual([25 * 60_000, 125 * 60_000]);
    expect(viewsCheckDirs(LAUNCH, "default").out).toBe("target/views-check/inspection");
  });

  it("traces each window with V8's CPU profiler beside the views check's own categories", () => {
    expect(INSPECTION_TIMING_TRACE_CATEGORIES).toEqual([
      ...VIEWS_CHECK_TRACE_CATEGORIES,
      "disabled-by-default-v8.cpu_profiler",
    ]);
  });

  it("traces a window, reduces it over its span, removes it, and writes the timing file", async () => {
    const { run, deps, timed } = timingSession();
    await run.capture("inspection-61ffd967fe000023-0100");
    await run.startProfile("orbit");
    await run.endProfile("orbit", SPAN);
    await run.writeTimingResults(TIMING_RECORD);
    run.end(0, null);
    expect(vi.mocked(deps.tracing.startRecording).mock.calls[0]?.[0]).toMatchObject({
      included_categories: [...INSPECTION_TIMING_TRACE_CATEGORIES],
    });
    expect(deps.tracing.stopRecording).toHaveBeenCalledWith("/profile/timing-orbit.pftrace");
    expect(deps.removeFile).toHaveBeenCalledWith("/profile/timing-orbit.pftrace");
    const window = timed[0]?.windows[0];
    expect(window?.trace.value?.threads.map((thread) => [thread.thread, thread.busyMs])).toEqual([
      ["CrRendererMain", 4],
    ]);
    expect(window?.passes.map((pass) => [pass.label, pass.medianMs])).toEqual([
      ["atmosphere view", 30],
    ]);
    expect(timed[0]?.run.served?.body).toBe("61ffd967fe000023.0100");
    expect(deps.exit).toHaveBeenCalledWith(0);
  });

  it("records a window whose trace does not reduce as missing, with why", async () => {
    const { run, timed } = timingSession({
      reduceProfile: () => Promise.reject(new Error("the trace was cut short")),
    });
    await run.startProfile("orbit");
    await run.endProfile("orbit", SPAN);
    await run.writeTimingResults(TIMING_RECORD);
    expect(timed[0]?.windows[0]?.trace).toEqual({
      value: null,
      reason: "the trace did not reduce: the trace was cut short",
    });
  });

  it("refuses a second window while one records, and a window of a launch that profiles nothing", async () => {
    const { run } = timingSession();
    await run.startProfile("orbit");
    await expect(run.startProfile("close")).rejects.toThrow(/still recording/);
    await expect(run.endProfile("close", SPAN)).rejects.toThrow(/not the profile recording/);
    const { session: plain } = session({
      launch: {
        setting: "high",
        smoke: false,
        out: null,
        run: "inspection",
        fixture: "terrain-test",
        served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
      },
    });
    await expect(plain.startProfile("orbit")).rejects.toThrow(/profiles no frame/);
  });

  it("fails a run that ends with its capture saved but no timing file written", async () => {
    const lines: string[] = [];
    const { run, deps } = timingSession({ log: (line) => lines.push(line) });
    await run.capture("inspection-61ffd967fe000023-0100");
    run.end(0, null);
    expect(lines.at(-1)).toBe(
      "views check: FAIL the inspection run ended with no timing file written",
    );
    expect(deps.exit).toHaveBeenCalledWith(1);
  });
});

describe("the views check's session on a terrain launch (R10.T16.d)", () => {
  it("takes no trace of a phase, and captures the page at its end", async () => {
    const { session: run, deps } = terrainSession();
    await run.startPhase("terrain-alone");
    await run.endPhase("terrain-alone", SPAN);
    expect(deps.tracing.startRecording).not.toHaveBeenCalled();
    expect(deps.tracing.stopRecording).not.toHaveBeenCalled();
    expect(deps.capture).toHaveBeenCalledWith("/captures/terrain-alone.png");
  });

  it("captures a look named for the phase recording", async () => {
    const { session: run, deps } = terrainSession();
    await run.startPhase("terrain-alone");
    await run.capture("sunset-240s");
    await run.endPhase("terrain-alone", SPAN);
    await run.capture("restored");
    expect(deps.capture).toHaveBeenCalledWith("/captures/terrain-alone-sunset-240s.png");
    expect(deps.capture).toHaveBeenCalledWith("/captures/between-restored.png");
  });

  it("refuses a capture label of another shape", async () => {
    const { session: run } = terrainSession();
    await expect(run.capture("../escape")).rejects.toThrow(/label/);
  });

  describe("its whole-body field (R10.T14.c)", () => {
    const whole = {
      setting: "high",
      smoke: false,
      out: null,
      run: "terrain",
      fixture: "terrain-test-whole",
      wholeField: "/repo/target/views-check/terrain/whole.golden",
    } as const;

    it("reads the file --whole names, a terrain golden", async () => {
      const { session: run } = terrainSession({ launch: whole });
      await expect(run.wholeField()).resolves.toBe(
        "# generator_version = 24\nbody of /repo/target/views-check/terrain/whole.golden\n",
      );
    });

    it("refuses a launch with no whole-body field", async () => {
      const { session: run } = terrainSession();
      await expect(run.wholeField()).rejects.toThrow("the launch serves no whole-body field");
    });

    it("names the file and the remedy where it cannot be read or is no terrain golden", async () => {
      const missing = terrainSession({
        launch: whole,
        readText: () => Promise.reject(new Error("ENOENT")),
      }).session;
      await expect(missing.wholeField()).rejects.toThrow(
        "the whole-body field /repo/target/views-check/terrain/whole.golden was not read (run `just views-check --terrain --whole`, whose tool writes it)",
      );
      const other = terrainSession({
        launch: whole,
        readText: () => Promise.resolve("{}"),
      }).session;
      await expect(other.wholeField()).rejects.toThrow("is not a terrain golden");
    });
  });

  it("judges the record it writes, with the GPU process's exits and the console's errors", async () => {
    const { session: run, terrainWritten } = terrainSession();
    run.gpuProcessGone();
    run.consoleError("the terrain's depth material for the symbology failed");
    await run.writeTerrainResults(TERRAIN_RECORD);
    expect(terrainWritten[0]?.findings.faults.gpuProcessExits).toBe(1);
    expect(terrainWritten[0]?.findings.consoleErrors.count).toBe(1);
    expect(terrainWritten[0]?.overall).toBe("fail");
  });

  it("writes its finished phases as each ends, unjudged, before its results (R14's Mac measurements)", async () => {
    const { session: run, terrainPhasesWritten, terrainWritten } = terrainSession();
    const [alone, wireframe] = TERRAIN_RECORD.phases;
    if (alone === undefined || wireframe === undefined) {
      throw new Error("the fixture has no first two phases");
    }
    await run.writeTerrainPhase({ timer: "quantized", devicePixelRatio: 2, phase: alone });
    await run.writeTerrainPhase({ timer: "quantized", devicePixelRatio: 2, phase: wireframe });
    await run.writeTerrainResults(TERRAIN_RECORD);
    expect([
      terrainPhasesWritten.map((each) => each.phases.map((phase) => phase.name)),
      terrainPhasesWritten.map((each) => [each.run.timer, each.run.devicePixelRatio]),
      terrainWritten.length,
    ]).toEqual([
      [["terrain-alone"], ["terrain-alone", "terrain-photoreal-wireframe"]],
      [
        ["quantized", 2],
        ["quantized", 2],
      ],
      1,
    ]);
  });

  it("records its captures from the repository's root, not by the checkout's own path", async () => {
    const { session: run, terrainWritten } = terrainSession({
      capturesRecordedAs: "target/views-check/terrain/captures-x",
    });
    await run.startPhase("terrain-alone");
    await run.capture("early");
    await run.writeTerrainResults(TERRAIN_RECORD);
    expect(terrainWritten[0]?.run.captures.value).toBe("target/views-check/terrain/captures-x");
  });

  it("records the weathered fixture of a --weathered launch, and none of TERRAIN TEST's (R10.T1.c)", async () => {
    const plain = terrainSession();
    const weathered = terrainSession({
      launch: {
        setting: "high",
        smoke: false,
        out: null,
        run: "terrain",
        fixture: "terrain-test-weathered",
      },
    });
    await plain.session.writeTerrainResults(TERRAIN_RECORD);
    await weathered.session.writeTerrainResults(TERRAIN_RECORD);
    expect([
      Object.hasOwn(plain.terrainWritten[0]?.run ?? {}, "fixture"),
      weathered.terrainWritten[0]?.run.fixture,
    ]).toEqual([false, "terrain-test-weathered"]);
  });

  it("ends with 0 when its results pass", async () => {
    const { session: run, deps } = terrainSession();
    await run.writeTerrainResults(TERRAIN_RECORD);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(0);
  });

  it("ends with 1 when its results fail, whatever its end says", async () => {
    const failing: TerrainCheckRecord = {
      ...TERRAIN_RECORD,
      teardown: { ...TERRAIN_RECORD.teardown, workersLive: 1 },
    };
    const lines: string[] = [];
    const { session: run, deps } = terrainSession({ log: (line) => lines.push(line) });
    await run.writeTerrainResults(failing);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toMatch(/1 height workers left/);
  });

  it("ends with 1 when its terrain session showed the terrain's fault (R10.T16.e)", async () => {
    const failing: TerrainCheckRecord = {
      ...TERRAIN_RECORD,
      terrainFaults: [
        {
          atMs: 40_000,
          text: "TERRAIN NOT AVAILABLE: terrain computation failed, reopen VIEW to retry",
          views: ["view"],
        },
      ],
    };
    const lines: string[] = [];
    const { session: run, deps } = terrainSession({ log: (line) => lines.push(line) });
    await run.writeTerrainResults(failing);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toMatch(/the terrain's fault at 40000 ms on view: TERRAIN NOT AVAILABLE/);
  });

  it("ends with 1 when it ends with no results written", () => {
    const { session: run, deps } = terrainSession();
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
  });
});

describe("a hand-over check's session (R10.T10.g)", () => {
  it("judges the record it writes", async () => {
    const { session: run, handoverWritten } = handoverSession();
    await run.writeHandoverResults(HANDOVER_RECORD);
    expect(handoverWritten[0]?.overall).toBe("pass");
  });

  it("records the served world of a --universe --body launch (R10.T15.b)", async () => {
    const served = { universe: "000000000000002a", body: "0200080020000000.0300" };
    const { session: run, handoverWritten } = handoverSession({
      launch: {
        setting: "high",
        smoke: false,
        out: null,
        run: "handover",
        fixture: "terrain-test",
        served,
      },
    });
    await run.writeHandoverResults(HANDOVER_RECORD);
    expect(handoverWritten[0]?.run.served).toEqual(served);
  });

  it("records the weathered fixture of a --weathered launch, and none of TERRAIN TEST's (R10.T1.c)", async () => {
    const plain = handoverSession();
    const weathered = handoverSession({
      launch: {
        setting: "high",
        smoke: false,
        out: null,
        run: "handover",
        fixture: "terrain-test-weathered",
      },
    });
    await plain.session.writeHandoverResults(HANDOVER_RECORD);
    await weathered.session.writeHandoverResults(HANDOVER_RECORD);
    expect([
      Object.hasOwn(plain.handoverWritten[0]?.run ?? {}, "fixture"),
      weathered.handoverWritten[0]?.run.fixture,
    ]).toEqual([false, "terrain-test-weathered"]);
  });

  it("records its captures from the repository's root", async () => {
    const { session: run, handoverWritten } = handoverSession({
      capturesRecordedAs: "target/views-check/terrain/captures-x",
    });
    await run.capture("handover-30-disc");
    await run.writeHandoverResults(HANDOVER_RECORD);
    expect(handoverWritten[0]?.run.captures.value).toBe("target/views-check/terrain/captures-x");
  });

  it("judges with the GPU process's exits and records the console's errors", async () => {
    const { session: run, handoverWritten } = handoverSession();
    run.gpuProcessGone();
    run.consoleError("a pass timer's read-back failed");
    await run.writeHandoverResults(HANDOVER_RECORD);
    expect([
      handoverWritten[0]?.findings.faults.gpuProcessExits,
      handoverWritten[0]?.findings.consoleErrors.count,
    ]).toEqual([1, 1]);
  });

  it("names a capture between phases, since it has none", async () => {
    const { session: run, deps } = handoverSession();
    await run.capture("handover-30-disc");
    expect(deps.capture).toHaveBeenCalledWith("/captures/between-handover-30-disc.png");
  });

  it("ends with 0 when its verdict passes", async () => {
    const { session: run, deps } = handoverSession();
    await run.writeHandoverResults(HANDOVER_RECORD);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(0);
  });

  it("ends with 1 when its verdict fails, whatever its end says", async () => {
    const failing: HandoverCheckRecord = {
      ...HANDOVER_RECORD,
      looks: HANDOVER_RECORD.looks.slice(1),
    };
    const lines: string[] = [];
    const { session: run, deps } = handoverSession({ log: (line) => lines.push(line) });
    await run.writeHandoverResults(failing);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toMatch(/the looks ran handover-90, handover-terminator/);
  });

  it("ends with 1 when it ends with no results written", () => {
    const lines: string[] = [];
    const { session: run } = handoverSession({ log: (line) => lines.push(line) });
    run.end(0, null);
    expect(lines.at(-1)).toBe(
      "views check: FAIL the hand-over check ended with no results written",
    );
  });

  it("ends with 1 when it wrote only the terrain run's results", async () => {
    const lines: string[] = [];
    const { session: run } = handoverSession({ log: (line) => lines.push(line) });
    await run.writeTerrainResults(TERRAIN_RECORD);
    run.end(0, null);
    expect(lines.at(-1)).toBe(
      "views check: FAIL the hand-over check ended with no results written",
    );
  });
});

describe("an atmosphere run's session (R08.T10.b)", () => {
  /** The fault line of TEST MARS's air, as the renderer writes it. */
  const NOT_AVAILABLE =
    "ATMOSPHERE NOT AVAILABLE: computation failed for TEST MARS, reopen VIEW to retry";

  it("names its captures between phases, since it has none", async () => {
    const { session: run, deps } = atmosphereSession();
    await run.capture("fixture-test-earth");
    await run.capture("moonlit-night");
    expect(deps.capture).toHaveBeenCalledWith("/captures/between-fixture-test-earth.png");
    expect(deps.capture).toHaveBeenCalledWith("/captures/between-moonlit-night.png");
  });

  it("judges the record it writes, its captures recorded from the repository's root", async () => {
    const lines: string[] = [];
    const { session: run, atmosphereWritten } = atmosphereSession({
      capturesRecordedAs: "target/views-check/atmosphere/captures-x",
      log: (line) => lines.push(line),
    });
    await run.capture("fixture-test-earth");
    expect(await run.writeAtmosphereResults(ATMOSPHERE_RECORD)).toEqual({
      json: "/out/a.json",
      markdown: "/out/a.md",
    });
    expect([
      atmosphereWritten[0]?.overall,
      atmosphereWritten[0]?.run.captures.value,
      lines,
    ]).toEqual([
      "pass",
      "target/views-check/atmosphere/captures-x",
      ["views check: wrote /out/a.json and /out/a.md"],
    ]);
  });

  it("judges the GPU process's exits, and records the console's errors without judging them", async () => {
    const lines: string[] = [];
    const {
      session: run,
      deps,
      atmosphereWritten,
    } = atmosphereSession({
      log: (line) => lines.push(line),
    });
    run.gpuProcessGone();
    run.consoleError("a pass timer's read-back failed");
    await run.capture("fixture-test-earth");
    await run.writeAtmosphereResults(ATMOSPHERE_RECORD);
    run.end(0, null);
    expect([
      atmosphereWritten[0]?.overall,
      atmosphereWritten[0]?.findings.faults.gpuProcessExits,
      atmosphereWritten[0]?.findings.consoleErrors.count,
    ]).toEqual(["fail", 1, 1]);
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toBe(
      "views check: FAIL the atmosphere run's checks failed: 1 exits of the GPU process",
    );
  });

  it("ends with 0 when its captures pass", async () => {
    const { session: run, deps } = atmosphereSession();
    await run.capture("fixture-test-earth");
    await run.writeAtmosphereResults(ATMOSPHERE_RECORD);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(0);
  });

  it("ends with 1 when a capture was not drawn photorealistic, whatever its end says", async () => {
    const failing: AtmosphereCheckRecord = {
      ...ATMOSPHERE_RECORD,
      captures: ATMOSPHERE_RECORD.captures.map((capture) =>
        capture.label === "fixture-test-mars" ? { ...capture, style: "wireframe" } : capture,
      ),
    };
    const lines: string[] = [];
    const { session: run, deps } = atmosphereSession({ log: (line) => lines.push(line) });
    await run.capture("fixture-test-mars");
    await run.writeAtmosphereResults(failing);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toBe(
      "views check: FAIL the atmosphere run's checks failed: fixture-test-mars: drawn wireframe, not photorealistic",
    );
  });

  it("ends with 1 when a capture showed a fault line, whatever its end says", async () => {
    const failing: AtmosphereCheckRecord = {
      ...ATMOSPHERE_RECORD,
      captures: ATMOSPHERE_RECORD.captures.map((capture) =>
        capture.label === "fixture-test-mars" ? { ...capture, faults: [NOT_AVAILABLE] } : capture,
      ),
    };
    const lines: string[] = [];
    const { session: run, deps } = atmosphereSession({ log: (line) => lines.push(line) });
    await run.capture("fixture-test-mars");
    await run.writeAtmosphereResults(failing);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toBe(
      `views check: FAIL the atmosphere run's checks failed: fixture-test-mars: atmosphere fault: ${NOT_AVAILABLE}`,
    );
  });

  it("ends with 1 when no capture was saved", async () => {
    const lines: string[] = [];
    const { session: run, deps } = atmosphereSession({ log: (line) => lines.push(line) });
    await run.writeAtmosphereResults(ATMOSPHERE_RECORD);
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toBe(
      "views check: FAIL the atmosphere run's checks failed: the main process saved no capture",
    );
  });

  it("ends with 1 when it ends with no results written", () => {
    const lines: string[] = [];
    const { session: run, deps } = atmosphereSession({ log: (line) => lines.push(line) });
    run.end(0, null);
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toBe("views check: FAIL the atmosphere run ended with no results written");
  });

  it("ends with 1 when it wrote only the hand-over check's results", async () => {
    const lines: string[] = [];
    const { session: run } = atmosphereSession({ log: (line) => lines.push(line) });
    await run.writeHandoverResults(HANDOVER_RECORD);
    run.end(0, null);
    expect(lines.at(-1)).toBe("views check: FAIL the atmosphere run ended with no results written");
  });

  it("keeps the renderer's failure as its reason", () => {
    const lines: string[] = [];
    const { session: run, deps } = atmosphereSession({ log: (line) => lines.push(line) });
    run.end(1, "the media did not come ready");
    expect(deps.exit).toHaveBeenCalledWith(1);
    expect(lines.at(-1)).toBe("views check: FAIL the media did not come ready");
  });
});

describe("the views check's handlers", () => {
  type Listener = (event: { own: boolean }, ...args: unknown[]) => Promise<unknown>;

  function handlers(
    run: ViewsCheckRunKind = "several-views",
    timing = false,
  ): {
    readonly call: (channel: string, own: boolean, ...args: unknown[]) => Promise<unknown>;
    readonly calls: string[];
  } {
    const listeners = new Map<string, Listener>();
    const calls: string[] = [];
    registerViewsCheckHandlers<{ own: boolean }>({
      handle: (channel, listener) => {
        listeners.set(channel, listener);
      },
      isSender: (event) => event.own,
      session: {
        startPhase: (name) => {
          calls.push(`start ${name}`);
          return Promise.resolve();
        },
        endPhase: (name) => {
          calls.push(`end ${name}`);
          return Promise.resolve();
        },
        askRightWayUp: () => {
          calls.push("ask");
          return Promise.resolve();
        },
        writeResults: () => {
          calls.push("write");
          return Promise.resolve({ json: "a", markdown: "b" });
        },
        writeTerrainResults: () => {
          calls.push("write terrain");
          return Promise.resolve({ json: "a", markdown: "b" });
        },
        writeTerrainPhase: (update) => {
          calls.push(`write terrain phase ${update.phase.name}`);
          return Promise.resolve({ json: "a", markdown: "b" });
        },
        writeHandoverResults: () => {
          calls.push("write handover");
          return Promise.resolve({ json: "a", markdown: "b" });
        },
        writeAtmosphereResults: () => {
          calls.push("write atmosphere");
          return Promise.resolve({ json: "a", markdown: "b" });
        },
        startProfile: (name) => {
          calls.push(`start profile ${name}`);
          return Promise.resolve();
        },
        endProfile: (name) => {
          calls.push(`end profile ${name}`);
          return Promise.resolve();
        },
        writeTimingResults: () => {
          calls.push("write timing");
          return Promise.resolve({ json: "a", markdown: "b" });
        },
        capture: (label) => {
          calls.push(`capture ${label}`);
          return Promise.resolve();
        },
        wholeField: () => {
          calls.push("whole field");
          return Promise.resolve("# generator_version = 24\n");
        },
        end: (code, reason) => {
          calls.push(`exit ${String(code)} ${String(reason)}`);
        },
      },
      run,
      timing,
    });
    return {
      call: (channel, own, ...args) => {
        const listener = listeners.get(channel);
        if (listener === undefined) {
          throw new Error(`no handler on ${channel}`);
        }
        return listener({ own }, ...args);
      },
      calls,
    };
  }

  it("carry out each operation from the check window's page", async () => {
    const { call, calls } = handlers();
    await call(VIEWS_CHECK_CHANNELS.startPhase, true, "wireframe-alone");
    await call(VIEWS_CHECK_CHANNELS.endPhase, true, "wireframe-alone", SPAN);
    await call(VIEWS_CHECK_CHANNELS.askRightWayUp, true);
    await call(VIEWS_CHECK_CHANNELS.writeResults, true, RECORD);
    await call(VIEWS_CHECK_CHANNELS.end, true, { status: "fail", reason: "stopped" });
    expect(calls).toEqual([
      "start wireframe-alone",
      "end wireframe-alone",
      "ask",
      "write",
      "exit 1 stopped",
    ]);
  });

  it.each([
    ["another page's call", VIEWS_CHECK_CHANNELS.askRightWayUp, false, [], /own page/],
    [
      "a phase that is not one",
      VIEWS_CHECK_CHANNELS.startPhase,
      true,
      ["every-view"],
      /not a phase/,
    ],
    [
      "a window that ends before it starts",
      VIEWS_CHECK_CHANNELS.endPhase,
      true,
      ["wireframe-alone", { startMs: 2, endMs: 1 }],
      /its window/,
    ],
    [
      "a record that does not check",
      VIEWS_CHECK_CHANNELS.writeResults,
      true,
      [{ timer: "full" }],
      /record/,
    ],
    [
      "an end that is not one",
      VIEWS_CHECK_CHANNELS.end,
      true,
      [{ status: "maybe" }],
      /end of the run/,
    ],
  ] as const)("refuse %s", async (_what, channel, own, args, error) => {
    const { call, calls } = handlers();
    await expect(call(channel, own, ...args)).rejects.toThrow(error);
    expect(calls).toEqual([]);
  });

  it("carry out the frame profile's operations on an inspection launch with --timing", async () => {
    const { call, calls } = handlers("inspection", true);
    await call(VIEWS_CHECK_CHANNELS.startProfile, true, "orbit");
    await call(VIEWS_CHECK_CHANNELS.endProfile, true, "orbit", SPAN);
    await call(VIEWS_CHECK_CHANNELS.writeTimingResults, true, {
      timer: "absent",
      devicePixelRatio: 1,
      windows: [],
    });
    expect(calls).toEqual(["start profile orbit", "end profile orbit", "write timing"]);
  });

  it("take the low window over the ground, where the cascades draw (main's ask)", async () => {
    const { call, calls } = handlers("inspection", true);
    await call(VIEWS_CHECK_CHANNELS.startProfile, true, "low");
    await call(VIEWS_CHECK_CHANNELS.endProfile, true, "low", SPAN);
    expect(calls).toEqual(["start profile low", "end profile low"]);
  });

  it.each([
    ["an inspection launch without --timing", "inspection", false],
    ["an atmosphere launch", "atmosphere", true],
    ["T20's launch", "several-views", true],
  ] as const)("refuse the frame profile's operations on %s", async (_what, run, timing) => {
    const { call, calls } = handlers(run, timing);
    await expect(call(VIEWS_CHECK_CHANNELS.startProfile, true, "orbit")).rejects.toThrow(
      /not a profiled window/,
    );
    await expect(call(VIEWS_CHECK_CHANNELS.endProfile, true, "orbit", SPAN)).rejects.toThrow(
      /not a profiled window/,
    );
    await expect(
      call(VIEWS_CHECK_CHANNELS.writeTimingResults, true, {
        timer: "absent",
        devicePixelRatio: 1,
        windows: [],
      }),
    ).rejects.toThrow(/timing record/);
    expect(calls).toEqual([]);
  });

  it.each([
    ["a window that is not one", VIEWS_CHECK_CHANNELS.startProfile, ["descent"], /profiled window/],
    [
      "a window's span that ends before it starts",
      VIEWS_CHECK_CHANNELS.endProfile,
      ["close", { startMs: 2, endMs: 1 }],
      /its span/,
    ],
    [
      "a timing record whose pass times are not one a frame",
      VIEWS_CHECK_CHANNELS.writeTimingResults,
      [
        {
          timer: "full",
          devicePixelRatio: 1,
          windows: [
            {
              name: "orbit",
              startMs: 0,
              endMs: 1,
              canvas: { widthPx: 1, heightPx: 1 },
              scales: [],
              intervalsMs: [],
              mainThreadMs: [],
              submitMs: [],
              gpuMs: [1, 2],
              passes: [{ label: "sky", ms: [1], kernels: [], workgroups: [] }],
              untimedFrames: 0,
              memory: {
                uploaded: { count: 4, bytes: 2_097_152 },
                created: { count: 1, bytes: 1_024 },
                destroyed: { count: 0, bytes: 0 },
                uploads: [{ name: "view star sprites", count: 4, bytes: 2_097_152 }],
              },
              gpuCalls: null,
              clusters: null,
              marchSamples: 36,
              position: null,
              statements: [],
            },
          ],
        },
      ],
      /timing record/,
    ],
  ] as const)("refuse %s on a profiled launch", async (_what, channel, args, error) => {
    const { call, calls } = handlers("inspection", true);
    await expect(call(channel, true, ...args)).rejects.toThrow(error);
    expect(calls).toEqual([]);
  });

  it("carry out a terrain run's operations on a terrain launch", async () => {
    const { call, calls } = handlers("terrain");
    await call(VIEWS_CHECK_CHANNELS.startPhase, true, "terrain-loss");
    await call(VIEWS_CHECK_CHANNELS.capture, true, "restored");
    await call(VIEWS_CHECK_CHANNELS.endPhase, true, "terrain-loss", SPAN);
    await call(VIEWS_CHECK_CHANNELS.writeTerrainResults, true, TERRAIN_RECORD);
    expect(calls).toEqual([
      "start terrain-loss",
      "capture restored",
      "end terrain-loss",
      "write terrain",
    ]);
  });

  it.each([
    [VIEWS_CHECK_CHANNELS.startPhase, ["photoreal-alone"]],
    [VIEWS_CHECK_CHANNELS.endPhase, ["photoreal-alone", SPAN]],
  ] as const)("refuse T20's phase on %s on a terrain launch", async (channel, args) => {
    const { call, calls } = handlers("terrain");
    await expect(call(channel, true, ...args)).rejects.toThrow(/not a phase of the terrain run/);
    expect(calls).toEqual([]);
  });

  it.each([
    [VIEWS_CHECK_CHANNELS.startPhase, ["terrain-alone"]],
    [VIEWS_CHECK_CHANNELS.endPhase, ["terrain-alone", SPAN]],
  ] as const)("refuse a terrain phase on %s on T20's launch", async (channel, args) => {
    const { call, calls } = handlers("several-views");
    await expect(call(channel, true, ...args)).rejects.toThrow(/not a phase of T20's run/);
    expect(calls).toEqual([]);
  });

  it("read the whole-body field on a terrain launch alone (R10.T14.c)", async () => {
    const terrain = handlers("terrain");
    await expect(terrain.call(VIEWS_CHECK_CHANNELS.wholeField, true)).resolves.toBe(
      "# generator_version = 24\n",
    );
    expect(terrain.calls).toEqual(["whole field"]);
    const other = handlers("handover");
    await expect(other.call(VIEWS_CHECK_CHANNELS.wholeField, true)).rejects.toThrow(
      /the whole body's field is the terrain run's/,
    );
    expect(other.calls).toEqual([]);
  });

  it("take a terrain run's phase as it ends on a terrain launch alone (R14's Mac measurements)", async () => {
    const [phase] = TERRAIN_RECORD.phases;
    const update = { timer: "quantized", devicePixelRatio: 2, phase };
    const terrain = handlers("terrain");
    await terrain.call(VIEWS_CHECK_CHANNELS.writeTerrainPhase, true, update);
    await expect(
      terrain.call(VIEWS_CHECK_CHANNELS.writeTerrainPhase, true, { ...update, timer: "fast" }),
    ).rejects.toThrow(/not a terrain run's phase/);
    const other = handlers("several-views");
    await expect(other.call(VIEWS_CHECK_CHANNELS.writeTerrainPhase, true, update)).rejects.toThrow(
      /not a terrain run's phase/,
    );
    expect([terrain.calls, other.calls]).toEqual([["write terrain phase terrain-alone"], []]);
  });

  it("refuse T20's record on a terrain launch, which writes its own alone", async () => {
    const { call, calls } = handlers("terrain");
    await expect(call(VIEWS_CHECK_CHANNELS.writeResults, true, RECORD)).rejects.toThrow(
      /not the check's record/,
    );
    expect(calls).toEqual([]);
  });

  it("refuse a terrain record and a capture on T20's launch", async () => {
    const { call, calls } = handlers("several-views");
    await expect(
      call(VIEWS_CHECK_CHANNELS.writeTerrainResults, true, TERRAIN_RECORD),
    ).rejects.toThrow(/terrain run's record/);
    await expect(call(VIEWS_CHECK_CHANNELS.capture, true, "restored")).rejects.toThrow(/capture/);
    expect(calls).toEqual([]);
  });

  it("take a hand-over check's capture on a hand-over launch (R10.T10.g)", async () => {
    const { call, calls } = handlers("handover");
    await call(VIEWS_CHECK_CHANNELS.capture, true, "handover-90-back");
    expect(calls).toEqual(["capture handover-90-back"]);
  });

  it("take a hand-over check's record on a hand-over launch", async () => {
    const { call, calls } = handlers("handover");
    await call(VIEWS_CHECK_CHANNELS.writeHandoverResults, true, HANDOVER_RECORD);
    expect(calls).toEqual(["write handover"]);
  });

  it("refuse the terrain run's record on a hand-over launch", async () => {
    const { call, calls } = handlers("handover");
    await expect(
      call(VIEWS_CHECK_CHANNELS.writeTerrainResults, true, TERRAIN_RECORD),
    ).rejects.toThrow(/terrain run's record/);
    expect(calls).toEqual([]);
  });

  it.each([["several-views"], ["terrain"]] as const)(
    "refuse a hand-over record on a %s launch",
    async (run) => {
      const { call, calls } = handlers(run);
      await expect(
        call(VIEWS_CHECK_CHANNELS.writeHandoverResults, true, HANDOVER_RECORD),
      ).rejects.toThrow(/hand-over check's record/);
      expect(calls).toEqual([]);
    },
  );

  it.each([
    [VIEWS_CHECK_CHANNELS.startPhase, ["terrain-alone"]],
    [VIEWS_CHECK_CHANNELS.endPhase, ["terrain-alone", SPAN]],
    [VIEWS_CHECK_CHANNELS.startPhase, ["photoreal-alone"]],
  ] as const)(
    "refuse every phase on %s on a hand-over launch, which has none",
    async (channel, args) => {
      const { call, calls } = handlers("handover");
      await expect(call(channel, true, ...args)).rejects.toThrow(
        /not a phase of the hand-over check/,
      );
      expect(calls).toEqual([]);
    },
  );

  it("refuse a hand-over record that does not check", async () => {
    const { call, calls } = handlers("handover");
    await expect(
      call(VIEWS_CHECK_CHANNELS.writeHandoverResults, true, { ...HANDOVER_RECORD, looks: null }),
    ).rejects.toThrow(/hand-over check's record/);
    expect(calls).toEqual([]);
  });

  it("carry out an atmosphere run's operations on an atmosphere launch (R08.T10.b)", async () => {
    const { call, calls } = handlers("atmosphere");
    await call(VIEWS_CHECK_CHANNELS.capture, true, "fixture-test-earth");
    await call(VIEWS_CHECK_CHANNELS.capture, true, "moonlit-night");
    await call(VIEWS_CHECK_CHANNELS.writeAtmosphereResults, true, ATMOSPHERE_RECORD);
    await call(VIEWS_CHECK_CHANNELS.end, true, { status: "pass" });
    expect(calls).toEqual([
      "capture fixture-test-earth",
      "capture moonlit-night",
      "write atmosphere",
      "exit 0 null",
    ]);
  });

  it.each([["several-views"], ["terrain"], ["handover"]] as const)(
    "refuse an atmosphere record on a %s launch",
    async (run) => {
      const { call, calls } = handlers(run);
      await expect(
        call(VIEWS_CHECK_CHANNELS.writeAtmosphereResults, true, ATMOSPHERE_RECORD),
      ).rejects.toThrow(/not the atmosphere run's record/);
      expect(calls).toEqual([]);
    },
  );

  it.each([
    [VIEWS_CHECK_CHANNELS.writeResults, RECORD, /not the check's record/],
    [VIEWS_CHECK_CHANNELS.writeTerrainResults, TERRAIN_RECORD, /terrain run's record/],
    [VIEWS_CHECK_CHANNELS.writeHandoverResults, HANDOVER_RECORD, /hand-over check's record/],
  ] as const)("refuse %s's record on an atmosphere launch", async (channel, record, error) => {
    const { call, calls } = handlers("atmosphere");
    await expect(call(channel, true, record)).rejects.toThrow(error);
    expect(calls).toEqual([]);
  });

  it.each([
    [VIEWS_CHECK_CHANNELS.startPhase, ["photoreal-alone"]],
    [VIEWS_CHECK_CHANNELS.endPhase, ["photoreal-alone", SPAN]],
    [VIEWS_CHECK_CHANNELS.startPhase, ["terrain-alone"]],
  ] as const)(
    "refuse every phase on %s on an atmosphere launch, which has none",
    async (channel, args) => {
      const { call, calls } = handlers("atmosphere");
      await expect(call(channel, true, ...args)).rejects.toThrow(
        /not a phase of the atmosphere run/,
      );
      expect(calls).toEqual([]);
    },
  );

  it("refuse an atmosphere record that does not check, and one from another page", async () => {
    const { call, calls } = handlers("atmosphere");
    await expect(
      call(VIEWS_CHECK_CHANNELS.writeAtmosphereResults, true, {
        ...ATMOSPHERE_RECORD,
        captures: null,
      }),
    ).rejects.toThrow(/not the atmosphere run's record/);
    await expect(
      call(VIEWS_CHECK_CHANNELS.writeAtmosphereResults, false, ATMOSPHERE_RECORD),
    ).rejects.toThrow(/own page/);
    expect(calls).toEqual([]);
  });

  it("refuse a terrain record that does not check, and a capture label of another shape", async () => {
    const { call, calls } = handlers("terrain");
    await expect(
      call(VIEWS_CHECK_CHANNELS.writeTerrainResults, true, { ...TERRAIN_RECORD, phases: null }),
    ).rejects.toThrow(/terrain run's record/);
    await expect(call(VIEWS_CHECK_CHANNELS.capture, true, "A/B")).rejects.toThrow(/capture/);
    expect(calls).toEqual([]);
  });
});
