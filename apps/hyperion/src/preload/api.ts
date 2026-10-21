/**
 * How this launch runs the GPU: Chromium's own path off Linux (no switches, never relaunched), the
 * forced Vulkan path on Linux, or the declared safe mode, which has no WebGPU.
 */
export type GraphicsLaunchMode = "default" | "vulkan" | "safe";

/** A GPU-process crash, as the main process reports it to the renderer. */
export interface GpuProcessGoneReport {
  /** Electron's reason for the exit, such as `crashed` or `killed`. */
  readonly reason: string;
  /** GPU-process crashes in this launch, this one included. */
  readonly count: number;
}

/** What the renderer learns of the launch's GPU set-up from the main process. */
export interface GraphicsApi {
  readonly launchMode: GraphicsLaunchMode;
  /** True when `--hyperion-gpu-timing` lifted timestamp quantization for this launch. */
  readonly gpuTiming: boolean;
  /**
   * Registers a listener for GPU-process crashes.
   *
   * @returns The listener's removal.
   */
  onGpuProcessGone(listener: (event: GpuProcessGoneReport) => void): () => void;
}

/** A quality setting's name as the command line gives it (`--setting`, R05 Design note 26). */
export type QualitySettingName = "high" | "low";

/** The spike's quality setting (R05 Design note 26). */
export type SpikeSettingName = QualitySettingName;

/** The terrain's vertex path (R05 Design note 4), the T16 and T17 variants. */
export type SpikeVertexPath = "baked-offsets" | "face-differences";

/** The terrain's normal resolution (R05 Design note 25). */
export type SpikeNormals = "double" | "mesh";

/** An on/off option. */
export type SpikeSwitch = "on" | "off";

/** The spike's options as the command line gave them (T13.c). */
export interface SpikeLaunch {
  readonly setting: SpikeSettingName;
  /** The seed, a u64 in decimal (a string, since a `bigint` does not cross the bridge as one). */
  readonly seed: string;
  /** A 10 s run that writes no results file and exits with a status. */
  readonly smoke: boolean;
  /** Where the results file goes, or `null` for `docs/measurements/descent-spike/`. */
  readonly out: string | null;
  /** The height-worker count, or `null` for Design note 11's default. */
  readonly workers: number | null;
  /** The vertex path and normals, or `null` for the setting's own. */
  readonly vertexPath: SpikeVertexPath | null;
  readonly normals: SpikeNormals | null;
  /** Whether the test planet's ridges are on. */
  readonly ridged: SpikeSwitch;
  /** Whether Dawn's safety checks stay on (Design note 22). */
  readonly dawnSafety: SpikeSwitch;
  /** The directory a GPU capture is written to (T15.a), or `null` for none. */
  readonly capture: string | null;
  /**
   * Whether the trace records V8's CPU profiler (`--trace-profile`, T14.e): a profiled run is a
   * diagnostic, never judged (decision-r05-trace-windows.md).
   */
  readonly traceProfile: SpikeSwitch;
}

/** Where a spike run's results file and its summary were written. */
export interface SpikeResultsPaths {
  readonly json: string;
  readonly markdown: string;
}

/**
 * What the main process did with a spike run's report (T14.c, T14.i): a full run's results file
 * written, or a smoke's trace checked against it with no file written.
 */
export type SpikeResultsAnswer =
  | { readonly kind: "written"; readonly paths: SpikeResultsPaths }
  /** `failure` is the first failed trace window's reason ("trace window k of n: …"), or `null`. */
  | { readonly kind: "smoke checked"; readonly failure: string | null };

/** How a spike run ends: the app exits with 0 for `pass` and 1 for `fail`. */
export type SpikeEnd =
  { readonly status: "pass" } | { readonly status: "fail"; readonly reason: string };

/**
 * The descent spike's narrow functions (plan R05, T13.c), present only when the client was
 * launched with `--descent-spike`; the main process checks each call's sender and arguments.
 */
export interface SpikeApi {
  /** The spike's options as the command line gave them. */
  readonly launch: SpikeLaunch;
  /** Starts the run's trace and its 1 Hz memory sampling (Design note 18). */
  startTrace(): Promise<void>;
  /**
   * Ends the trace's window at a boundary and begins the next (T14.e): the main process writes
   * the window's file, and starts recording again.
   */
  cycleTrace(): Promise<void>;
  /** Stops the trace and the 1 Hz memory sampling, and reduces the trace's windows in the main process. */
  stopTrace(): Promise<void>;
  /** Hands the main process's sampler the renderer's own memory (`getProcessMemoryInfo`). */
  sampleMemory(): Promise<void>;
  /**
   * Writes the results file of T14.c from the renderer's report. A smoke run writes none: the main
   * process checks its trace's windows against the report and answers whether one failed (T14.i).
   */
  writeResults(report: DescentSpikeReport): Promise<SpikeResultsAnswer>;
  /** Writes a GPU capture (T15.a) as `capture.json` and `capture.bin` into `--capture`'s directory. */
  writeCapture(capture: { readonly json: string; readonly bin: Uint8Array }): Promise<string>;
  /** Ends the run, and the app with it. */
  end(outcome: SpikeEnd): Promise<void>;
}

/** The API the preload script exposes to the renderer as `window.hyperion`. */
export interface HyperionApi {
  readonly platform: string;
  /**
   * WebSocket URL of the hyperion-server this client was launched to link to, from its
   * `--address` and `--port` options.
   */
  readonly serverUrl: string;
  /** The launch's graphics mode and the GPU process's crashes. */
  readonly graphics: GraphicsApi;
  /**
   * The quality setting `VIEW` draws at, from the launch's `--setting` (plan R07, T17): `high`
   * unless it gave `low`; a spike's or a check's launch gives its own.
   */
  readonly setting: QualitySettingName;
  readonly versions: {
    readonly electron: string;
    readonly chrome: string;
    readonly node: string;
  };
  /** The descent spike's functions, present only on a `--descent-spike` launch. */
  readonly spike?: SpikeApi;
  /** The several-views check's functions, present only on a `--views-check` launch (R07.T20). */
  readonly viewsCheck?: ViewsCheckApi;
}

declare global {
  interface Window {
    readonly hyperion: HyperionApi;
  }
}

/** Which of Design note 21's GPU rows a pass counts towards. */
export type SpikePassRow = "terrain" | "atmosphere" | "other";

/** The state of R01's pass timer over a run (`PassTimes.timer`). */
export type SpikePassTimer = "full" | "quantized" | "absent";

/** One segment of the scripted descent, in script time. */
export interface SpikeSegmentSpan {
  readonly name: string;
  readonly startS: number;
  readonly endS: number;
}

/** One pass's GPU time each frame, aligned with {@link SpikeFrameSeries.scriptTimesS}. */
export interface SpikePassSeries {
  /** The pass's `FrameSubmission.label`. */
  readonly label: string;
  readonly row: SpikePassRow;
  /** Its GPU time each frame, ms; `null` in a frame it did not run or was not timed. */
  readonly gpuMs: ReadonlyArray<number | null>;
}

/** The renderer's per-frame series of a run. */
export interface SpikeFrameSeries {
  /** Each frame's script time, s, ascending. */
  readonly scriptTimesS: ReadonlyArray<number>;
  /** Each frame's `requestAnimationFrame` interval from the one before, ms (the first is 0). */
  readonly rafIntervalsMs: ReadonlyArray<number>;
  /**
   * The main thread's time in each frame's `requestAnimationFrame` callback, engine submission
   * included (one `performance.measure` span a frame), ms: the headroom row's main-thread figure.
   * It leaves out the browser's own work on the thread (style, layout, GC between callbacks), so
   * the row is a lower bound; the trace's split gives the rest.
   */
  readonly ourCodeMs: ReadonlyArray<number>;
  /**
   * Each frame's callback start, `performance.now()` ms: the start its `spike.frame` span receives,
   * which the trace carries exactly as the span's `args.startTime`. The main process matches the
   * trace's spans to the frames by it (R05.T14.h, decision-r05-trace-windows-2.md, addendum A).
   */
  readonly callbackStartsMs: ReadonlyArray<number>;
  /**
   * Each frame's timer resolves whose pass times never reached the report: 0 for a complete frame
   * (R05.T14.j, decision-r05-trace-windows-2.md, addendum B). R01's timer drops a resolve while
   * every read-back buffer is in flight, and a failed read loses one too. Every resolve the timer
   * reports carries at least one pass, so a frame with a missing resolve and no pass time is
   * _dropped_, and one with some pass time is _partial_.
   */
  readonly missingResolves: ReadonlyArray<number>;
  /**
   * Each frame's missing resolves whose reads were still in flight when the report was taken, at
   * most its `missingResolves` (R05.T14.k, the orchestrator's ruling on T14.j's open question).
   * The run's control waits up to 1 s after the trace's last stop for the reads in flight; one
   * still outstanding then is missing for that reason, not because the timer dropped it.
   */
  readonly inFlightResolves: ReadonlyArray<number>;
  readonly passes: ReadonlyArray<SpikePassSeries>;
}

/** Patches a second in one segment, measured and predicted (Design note 19). */
export interface SpikeStreamingSegment {
  readonly segment: string;
  readonly requestedPerS: number;
  readonly bakedPerS: number;
  readonly residentPerS: number;
  /** The per-level D under the hard bound ε_n, the bound selection uses. */
  readonly predictedHardPerS: number;
  /** The per-level D under min(hard, 4σ_n) (decisions-r05.md item 6). */
  readonly predictedCalibratedPerS: number;
  /** Mean patches selected a frame, under the hard and the calibrated bound. */
  readonly patchesHard: number;
  readonly patchesCalibrated: number;
  /** Seconds `TERRAIN: STREAMING` showed. */
  readonly streamingS: number;
}

/** A pipeline created after warm-up (Design note 18). */
export interface SpikeLatePipeline {
  readonly label: string;
  readonly kind: "render" | "compute";
  readonly async: boolean;
  readonly scriptTimeS: number;
}

/**
 * One window of the run's trace, in `performance.now()` ms (R05.T14.d, T14.e): Chromium keeps one
 * trace session at a time, so the trace is stopped and started again between windows, which never
 * overlap (decision-r05-trace-windows.md).
 */
export interface SpikeTraceWindow {
  /** When its start resolved; for a failed last window, when the trace ended. */
  readonly startedMs: number;
  /** When its stop was asked for. */
  readonly stopRequestedMs: number;
  /**
   * Why the renderer failed it, or `null`. A cycle that failed, or was still pending at the next
   * boundary, ends the trace: one last window, failed with the reason, stands for the rest of the
   * run.
   */
  readonly failure: string | null;
}

/**
 * What the descent spike's renderer reports at the end of a run, for the results file (R05.T14.c);
 * T14.a gathers it.
 */
export interface DescentSpikeReport {
  /**
   * Where script time 0 is in `performance.now()` ms: the run's first frame's
   * `requestAnimationFrame` timestamp, from which `SpikeRun` counts script time (T14.d).
   */
  readonly scriptStartMs: number;
  /**
   * The trace's windows in order; the main process wrote one trace file for each, but perhaps not
   * for a failed last one.
   */
  readonly traceWindows: ReadonlyArray<SpikeTraceWindow>;
  /**
   * How long after each window's start its boundary's frames are still left out, s: the
   * renderer's `TRACE_BOUNDARY_GUARD_S`, which the main process's merge applies.
   */
  readonly traceGuardS: number;
  readonly warmupS: number;
  readonly segments: ReadonlyArray<SpikeSegmentSpan>;
  /** T6's level table: per level, the bound ε_n, m, and the ratio k_n. */
  readonly levels: ReadonlyArray<{
    readonly level: number;
    readonly epsilonM: number;
    readonly k: number;
  }>;
  readonly timer: SpikePassTimer;
  /** Passes beyond the timer's 64 a frame, untimed, over the run. */
  readonly untimedPasses: number;
  readonly frames: SpikeFrameSeries;
  readonly streaming: ReadonlyArray<SpikeStreamingSegment>;
  /** Bytes through the engine's `writeBuffer` and `writeTexture` (R01's `uploaded` events). */
  readonly uploadBytes: number;
  readonly latePipelines: ReadonlyArray<SpikeLatePipeline>;
  /** The adapter's tally of buffers and textures at its peak, bytes. */
  readonly adapterPeakBytes: number;
  /** The view canvas's size in device pixels. */
  readonly canvas: { readonly widthPx: number; readonly heightPx: number };
  /**
   * The terrain's vertex path and normals the run drew with: the setting's own, or the variant
   * `--vertex-path` and `--normals` chose (T13.c).
   */
  readonly terrain?: { readonly vertexPath: SpikeVertexPath; readonly normals: SpikeNormals };
}

/** The several-views check's options as the command line gave them (plan R07, T20). */
export interface ViewsCheckLaunch {
  /** The quality setting `VIEW` is given, and the window's size: 1920 × 1080 high, 1280 × 720 low. */
  readonly setting: SpikeSettingName;
  /** A short hidden run that proves the harness, its file written under `target/views-check/`. */
  readonly smoke: boolean;
  /** Where the results file goes, or `null` for the default. */
  readonly out: string | null;
  /**
   * Whether the launch runs the terrain run (`--terrain`, plan R10, R10.T16.d;
   * decision-r10-t16-contracts §6) on `TERRAIN TEST` in place of T20's phases on `PHASE TEST`.
   */
  readonly terrain: boolean;
}

/**
 * The check's phases, each a configuration of `VIEW` held while it is measured (R07.T20): the
 * primary's style, then the instruments' (`two-wireframe`, or instrument 1 photorealistic beside a
 * wireframe instrument 2), or no instrument (`alone`).
 */
export type ViewsCheckPhaseName =
  | "photoreal-alone"
  | "photoreal-two-wireframe"
  | "wireframe-two-wireframe"
  | "wireframe-photoreal-wireframe"
  | "wireframe-alone";

/** A view's style, as `RenderStyle` names it. */
export type ViewsCheckStyle = "wireframe" | "photorealistic";

/** A view's engine name: the primary's `view`, or an instrument's. */
export type ViewsCheckViewName = "view" | "instrument-1" | "instrument-2";

/** A canvas's size, device pixels. */
export interface ViewsCheckCanvas {
  readonly widthPx: number;
  readonly heightPx: number;
}

/** One view over one phase. */
export interface ViewsCheckViewRecord {
  readonly name: ViewsCheckViewName;
  /** The style its canvas's accessible name said it drew when the phase ended. */
  readonly style: ViewsCheckStyle;
  /** Its canvas when the phase ended. */
  readonly canvas: ViewsCheckCanvas;
  /** The animation frames in which it submitted a pass. */
  readonly draws: number;
  /** Its timed GPU time in each frame it drew whose resolves all arrived, ms. */
  readonly gpuMs: ReadonlyArray<number>;
  /** The engine's CPU time in its `render` calls (encoding and submission) each frame it drew, ms. */
  readonly submitMs: ReadonlyArray<number>;
  /** The labels of its timed passes. */
  readonly passLabels: ReadonlyArray<string>;
  /** Its scene target's internal scale (width over the canvas's) at each size it was given. */
  readonly scales: ReadonlyArray<number>;
}

/** What the renderer measured over one phase's window. */
export interface ViewsCheckPhaseRecord {
  readonly name: ViewsCheckPhaseName;
  /** The window, `performance.now()` ms. */
  readonly startMs: number;
  readonly endMs: number;
  /** The views shown, the primary first. */
  readonly views: ReadonlyArray<ViewsCheckViewRecord>;
  /** Intervals between consecutive animation frames, ms. */
  readonly frameIntervalsMs: ReadonlyArray<number>;
  /** Intervals between the frames in which the primary drew, ms. */
  readonly primaryIntervalsMs: ReadonlyArray<number>;
  /** Each animation frame's time in the page's `requestAnimationFrame` callbacks, ms. */
  readonly mainThreadMs: ReadonlyArray<number>;
  /** Each frame's timed GPU time, every view's passes, where all its resolves arrived, ms. */
  readonly frameGpuMs: ReadonlyArray<number>;
  /** Frames in which a view drew whose resolves did not all arrive. */
  readonly untimedFrames: number;
  /** Resolves that took a number and were never reported: the pass timer's drops. */
  readonly droppedResolves: number;
  /** The GPU time of resolves no view's or target's `render` made (uploads, mipmaps), ms. */
  readonly unattributedGpuMs: number;
}

/** A view's canvas, by the view's name. */
export interface ViewsCheckNamedCanvas {
  readonly name: ViewsCheckViewName;
  readonly canvas: ViewsCheckCanvas;
}

/** One step of the resize: the primary's stage at a width, and every view's canvas after it. */
export interface ViewsCheckResizeStep {
  /** The stage's width as a fraction of its laid-out width; 1 restores it. */
  readonly widthFraction: number;
  /**
   * The longest animation frame's time in the page's callbacks over the step, ms: where the
   * photorealistic primary's renderer refits its bloom at the new size (R07.T19's about 35 ms).
   */
  readonly longestFrameMs: number;
  readonly views: ReadonlyArray<ViewsCheckNamedCanvas>;
}

/** The resize of the primary alone, with both instruments open (R07.T20). */
export interface ViewsCheckResizeRecord {
  /** Its canvas's size before the first step, by view. */
  readonly before: ReadonlyArray<ViewsCheckNamedCanvas>;
  readonly steps: ReadonlyArray<ViewsCheckResizeStep>;
  /** Every texture and buffer made or destroyed while it ran, by name, in order. */
  readonly allocations: ReadonlyArray<{
    readonly kind: "created" | "destroyed";
    readonly name: string;
  }>;
}

/** What the views check's renderer reports at the end of a run, for the results file. */
export interface ViewsCheckRecord {
  /** The pass timer's kind, as its reports gave it (`absent` when none arrived). */
  readonly timer: SpikePassTimer;
  readonly devicePixelRatio: number;
  readonly phases: ReadonlyArray<ViewsCheckPhaseRecord>;
  readonly resize: ViewsCheckResizeRecord;
  /** The engine's faults over the run, each as a line. */
  readonly faults: ReadonlyArray<string>;
  /** `PER_CANVAS_OVERHEAD_MS` as the client holds it, against which the run's figure reads. */
  readonly perCanvasOverheadMs: number;
}

/** A phase's measured window, `performance.now()` ms, which the main process clips its trace to. */
export interface ViewsCheckWindow {
  readonly startMs: number;
  readonly endMs: number;
}

/**
 * The terrain run's phases (plan R10, R10.T16.d; decision-r10-t16-contracts §6), each a
 * configuration of `VIEW` on `TERRAIN TEST`; a launch runs one setting's three, in order: on high
 * `terrain-alone`, `terrain-photoreal-wireframe` and `terrain-loss`, on low `terrain-alone`,
 * `terrain-wireframe` and `terrain-loss`.
 */
export type TerrainCheckPhaseName =
  /** The photorealistic primary alone, through the hand-over, the descent and the sunset. */
  | "terrain-alone"
  /** High: the same, with instrument 1 photorealistic and instrument 2 in the wireframe. */
  | "terrain-photoreal-wireframe"
  /** Low: the primary with one wireframe instrument, since `QUALITY LOW` allows one photorealistic view. */
  | "terrain-wireframe"
  /** The primary alone on a fresh session, its device destroyed by the harness mid-stream. */
  | "terrain-loss";

/** A phase of either run of the views check: T20's or the terrain run's. */
export type CheckPhaseName = ViewsCheckPhaseName | TerrainCheckPhaseName;

/** Bytes of one memory category (R01's `MemoryCategory` names). */
export interface TerrainCheckBytes {
  readonly category: string;
  readonly bytes: number;
}

/** The terrain session's memory as a phase ends (`TerrainSession.memory`, and the probe's tally). */
export interface TerrainCheckMemory {
  /** What the height workers hold of the fields, bytes. */
  readonly workerFieldBytes: number;
  /** The blocks the pool keeps for a worker made again, bytes. */
  readonly keptFieldBytes: number;
  /** The session's own textures (masks and class maps), by category. */
  readonly sessionGpuBytes: ReadonlyArray<TerrainCheckBytes>;
  /**
   * The terrain's GPU allocations alive as the phase ends, by category: the session's textures and
   * its passes' resources, every allocation named `terrain …`, `coverage mask …` or `class map …`.
   */
  readonly liveBytes: ReadonlyArray<TerrainCheckBytes>;
  /** The most of them alive at once over the phase, by category. */
  readonly peakBytes: ReadonlyArray<TerrainCheckBytes>;
}

/** One view over one phase of the terrain run. */
export interface TerrainCheckViewRecord {
  readonly name: ViewsCheckViewName;
  /** The style its canvas's accessible name said it drew when the phase ended. */
  readonly style: ViewsCheckStyle;
  readonly canvas: ViewsCheckCanvas;
  /** The animation frames in which it submitted a pass. */
  readonly draws: number;
  /** Its timed GPU time in each frame it drew whose resolves all arrived, ms. */
  readonly gpuMs: ReadonlyArray<number>;
  /** The engine's CPU time in its `render` calls each frame it drew, ms. */
  readonly submitMs: ReadonlyArray<number>;
  /**
   * The animation frames in which it was due at its own cadence (its most common count of frames
   * between two draws: 1 at 60 Hz, 2 at 30 Hz) and did not draw, outside the loss's restore (from
   * the device's destruction to the terrain's first draw after the restore).
   */
  readonly missedFrames: number;
  /**
   * The longest time between two of its draws, ms, outside the loss's restore: a stall of the
   * whole page shows here and not among the missed frames, since no animation frame ran. The run
   * fails on one over its limit (plan R10, R10.T14.d; `TERRAIN_GAP_LIMIT_MS`).
   */
  readonly longestGapMs: number;
  /**
   * The script's time at that gap's end, s: its draw after the gap, or the phase's end; `null`
   * where the stage had told no script time by then (R10.T14.d).
   */
  readonly longestGapEndScriptS: number | null;
  /**
   * Its draws while `TEST MOON` was handed over to its terrain with a drawn patch in its view (the
   * pass took the view, and some patch of its draw set lies in the view's).
   */
  readonly handedOverDraws: number;
  /**
   * Its draws while handed over with no drawn patch in its view: past the survey, where the
   * reference surface alone is drawn, or every patch it sees still to come.
   */
  readonly offSurveyDraws: number;
  /** Of those, the draws whose submissions held a draw of the terrain's lit material. */
  readonly terrainDraws: number;
  /**
   * Handed-over draws without the terrain after its first terrain draw of the phase, outside the
   * loss's restore: a terrain once drawn that went missing.
   */
  readonly terrainGaps: number;
  /** The first spans of those draws, each its first and last draw's script time, s. */
  readonly terrainGapSpansS: ReadonlyArray<readonly [number, number]>;
  /** When it first handed over in the phase, ms, and the script's time then, s; `null` for never. */
  readonly firstHandoverMs: number | null;
  readonly firstHandoverScriptS: number | null;
  /** When it first drew the terrain in the phase, ms; `null` for never. */
  readonly firstTerrainMs: number | null;
  /**
   * When the hand-over it first drew the terrain in began, ms: the first of the handed-over draws
   * that ran unbroken to that draw (R10.T14.d), so that a lone handed-over draw before it, such as
   * the frame the script's loop moves the camera in, before the pass has let the view go, is not
   * taken for its hand-over; `null` for never.
   */
  readonly terrainHandoverMs: number | null;
  /** Its scene target's read-backs over the phase. */
  readonly readBacks: number;
  /** Texels of those read-backs not finite in any channel. */
  readonly nonFiniteTexels: number;
}

/** What the renderer measured over one phase of the terrain run. */
export interface TerrainCheckPhaseRecord {
  readonly name: TerrainCheckPhaseName;
  /** The window, `performance.now()` ms. */
  readonly startMs: number;
  readonly endMs: number;
  /** The script's time at the window's start and at its end, s; it loops at its length. */
  readonly scriptStartS: number;
  readonly scriptEndS: number;
  /** The views shown, the primary first. */
  readonly views: ReadonlyArray<TerrainCheckViewRecord>;
  /** Intervals between consecutive animation frames, ms. */
  readonly frameIntervalsMs: ReadonlyArray<number>;
  /** Each animation frame's time in the page's `requestAnimationFrame` callbacks, ms. */
  readonly mainThreadMs: ReadonlyArray<number>;
  /** Each frame's timed GPU time, every view's passes, where all its resolves arrived, ms. */
  readonly frameGpuMs: ReadonlyArray<number>;
  /** The patches `TEST MOON`'s pass drew, at each of the primary's draws while it had a pass. */
  readonly patchesDrawn: ReadonlyArray<number>;
  /** Bakes posted to the height workers, and their answers, over the window. */
  readonly bakes: {
    readonly posted: number;
    readonly baked: number;
    /** Answered `bake-failed` or `bake-unsurveyed`. */
    readonly refused: number;
  };
  /** Field messages posted to the height workers over the window, each worker's counted. */
  readonly fieldPosts: {
    readonly inits: number;
    readonly chunks: number;
    readonly releases: number;
  };
  /** The fixture's `surface_field` asks over the window, and the payload blocks it sent. */
  readonly fieldAsks: number;
  readonly blocksSent: number;
  /** The session's memory as the window ends, or `null` with no session then. */
  readonly memory: TerrainCheckMemory | null;
}

/** An engine fault the terrain run saw, with its time. */
export interface TerrainCheckFault {
  /** `performance.now()` ms. */
  readonly atMs: number;
  /** The fault's kind, as R01's `GraphicsFault` names it. */
  readonly kind: "device-lost" | "gpu-process-gone" | "shader-refused" | "view-refused";
  /** A device loss's reason (`destroyed`, `unknown`); `null` for any other fault. */
  readonly reason: string | null;
  /** The fault as a line, as T20's record gives it. */
  readonly line: string;
}

/**
 * The terrain session's fault as the terrain run saw it raised or changed (plan R10, R10.T16.e;
 * decision-r10-t16-leans §8): the views whose label blocks showed it, and its text.
 */
export interface TerrainCheckSessionFault {
  /** `performance.now()` ms. */
  readonly atMs: number;
  /**
   * Its fault line by the session's first cause, which every view shows alike: the renderer's
   * `TERRAIN_NOT_AVAILABLE` or `TERRAIN_COMPUTATION_FAILED` (`view/terrain/annunciation.ts`).
   */
  readonly text: string;
  /** The views that showed it, by their engine names (`view`, `instrument-1`, `instrument-2`). */
  readonly views: ReadonlyArray<string>;
}

/** The forced loss (decision-r10-t16-contracts §6): what stood when the harness destroyed the device, and after. */
export interface TerrainCheckLossRecord {
  /** When the harness destroyed the device, ms, and the script's time then, s; `null` if it did not. */
  readonly destroyedAtMs: number | null;
  readonly destroyedAtScriptS: number | null;
  /** Why the harness did not destroy the device, or `null`. */
  readonly notDestroyed: string | null;
  /** At the destruction: the payload's blocks the fixture had still to send, and bakes in flight. */
  readonly blocksToCome: number;
  readonly bakesInFlight: number;
  /** After the destruction, to the phase's end. */
  readonly after: {
    readonly fieldAsks: number;
    readonly fieldInits: number;
    readonly fieldChunks: number;
    /** The blocks the fixture sent: those of the answer in flight at the destruction. */
    readonly blocksSent: number;
  };
  /** The session's height workers, by which each block is posted. */
  readonly workers: number;
  /** When the primary first drew the terrain after the restore, ms; `null` for never. */
  readonly terrainAgainAtMs: number | null;
  /** The body's GPU coverage mask, read back: surveyed cells, `null` where unread. */
  readonly masks: {
    /** Just before the destruction. */
    readonly before: number | null;
    /** As soon as it could be read after the restore. */
    readonly afterRestore: number | null;
    /** At the phase's end, every block taken. */
    readonly end: number | null;
    /** The store's coverage at the phase's end: the cells the mask should hold. */
    readonly expected: number;
    /** Whether every cell of `before` is in `afterRestore`. */
    readonly beforeKept: boolean;
    /** Whether `end` holds exactly the cells of `expected`. */
    readonly endAsExpected: boolean;
  };
}

/** The terrain session's teardown at the run's end: `VIEW` on a kept scene with no terrain. */
export interface TerrainCheckTeardownRecord {
  /** Whether the session was disposed (its watch ended). */
  readonly disposed: boolean;
  /** The height workers made over the run, and those not terminated. */
  readonly workersMade: number;
  readonly workersLive: number;
  /** The session's own textures' bytes after it, by its memory report. */
  readonly sessionGpuBytes: number;
  /** The terrain's GPU allocations still alive after it, by category. */
  readonly liveBytes: ReadonlyArray<TerrainCheckBytes>;
}

/**
 * A terrain session the run saw `VIEW` make, and the compile of its materials, which it starts as
 * it is made, before any hand-over (plan R10, R10.T14.d): a stall a compile cost the page would
 * fall here, outside every phase, which starts once the compile has ended, and is judged by the
 * same limit as a phase's gaps.
 */
export interface TerrainCheckSessionRecord {
  /** When it was made, its materials' compile beginning, `performance.now()` ms. */
  readonly madeAtMs: number;
  readonly compile: TerrainCheckSessionCompile;
}

/** A session's materials' compile: still running at the run's end, or ended (R10.T14.d). */
export type TerrainCheckSessionCompile =
  | { readonly kind: "pending" }
  | {
      readonly kind: "ended";
      /** From the session's making to its materials' compile ended, ms. */
      readonly compileMs: number;
      /** How many of its setting's materials compiled. */
      readonly compiled: number;
      /**
       * The longest time between two animation frames from its making to the first frame after
       * the compile ended, ms, a stall of the page showing here; `null` where no frame followed.
       */
      readonly longestFrameGapMs: number | null;
    };

/**
 * VIEW's opening as the terrain run saw it (plan R10, R10.T14.d): from the run's start until the
 * materials' preparations its views made as they opened had settled. A disc drawn in its first
 * frames compiles before any preparation can land, so its longest gap is recorded, not judged.
 */
export interface TerrainCheckOpeningRecord {
  /** From the run's start to its views' preparations settled, ms. */
  readonly preparedMs: number;
  /** How many preparations its views made. */
  readonly preparations: number;
  /**
   * The longest time between two animation frames from the run's start to the first frame after the
   * preparations settled, ms; `null` where no frame followed.
   */
  readonly longestFrameGapMs: number | null;
}

/**
 * The terrain run's judged span (plan R10, R10.T14.d; decision-r10-t14d-stall §3): from the end of
 * VIEW's opening to the teardown's choice of `PHASE TEST`, every phase, each session's making and
 * compile, and the scene choices and configurations between them, outside the forced loss's
 * restore and the run's own computations.
 */
export interface TerrainCheckSpanRecord {
  /** Its ends, `performance.now()` ms. */
  readonly fromMs: number;
  readonly toMs: number;
  /**
   * The page's longest time between two animation frames over it, outside the restore and the
   * run's own computations, ms.
   */
  readonly longestFrameGapMs: number;
  /** When that gap ended, ms, and the script's time then, s (`null` where none was told). */
  readonly longestGapEndMs: number;
  readonly longestGapEndScriptS: number | null;
  /**
   * The time the run's own computations held the page's main thread, its phases' records made at
   * their ends, ms: the gaps they made are left out, as the restore is.
   */
  readonly harnessMs: number;
}

/** What the terrain run's renderer reports at the end of a run, for its results file. */
export interface TerrainCheckRecord {
  /** The pass timer's kind, as its reports gave it (`absent` when none arrived). */
  readonly timer: SpikePassTimer;
  readonly devicePixelRatio: number;
  readonly phases: ReadonlyArray<TerrainCheckPhaseRecord>;
  /** Every engine fault over the run, timed. */
  readonly faults: ReadonlyArray<TerrainCheckFault>;
  /**
   * Each time the terrain session's `terrainFaults` held a view or changed while it did, timed
   * (R10.T16.e): the run fails on any, of either cause.
   */
  readonly terrainFaults: ReadonlyArray<TerrainCheckSessionFault>;
  /** Each terrain session `VIEW` made over the run, in order, and its materials' compile (R10.T14.d). */
  readonly sessions: ReadonlyArray<TerrainCheckSessionRecord>;
  /** VIEW's opening, until its views' preparations settled (R10.T14.d); recorded, not judged. */
  readonly opening: TerrainCheckOpeningRecord;
  /** The judged span's page-wide longest gap (R10.T14.d). */
  readonly span: TerrainCheckSpanRecord;
  /** Every restore of the engine after a loss, `performance.now()` ms. */
  readonly restoresAtMs: ReadonlyArray<number>;
  readonly loss: TerrainCheckLossRecord;
  readonly teardown: TerrainCheckTeardownRecord;
  /**
   * `TEST MOON`'s hand-over range at the primary's canvas and field of view, m, and the script's
   * time at which the descent crosses it, s (`handoverRangeM`); `null` before its field's header.
   */
  readonly handover: { readonly rangeM: number; readonly scriptS: number } | null;
}

/**
 * The views check's narrow functions (plan R07, T20), present only when the client was launched
 * with `--views-check`; the main process checks each call's sender and arguments.
 */
export interface ViewsCheckApi {
  readonly launch: ViewsCheckLaunch;
  /**
   * Starts a phase's trace window; a terrain run's phases (R10.T16.d) take no trace. The main
   * process refuses a phase that is not of the launch's run.
   */
  startPhase(name: CheckPhaseName): Promise<void>;
  /**
   * Ends it: stops the trace, reduces it over the measured `window` (marked in the trace by a
   * `performance.measure` span) and saves a capture of the page.
   */
  endPhase(name: CheckPhaseName, window: ViewsCheckWindow): Promise<void>;
  /** Asks the person at a shown run whether every view is the right way up, and keeps the answer. */
  askRightWayUp(): Promise<void>;
  /** Writes the results file and its summary from the renderer's record. */
  writeResults(record: ViewsCheckRecord): Promise<SpikeResultsPaths>;
  /**
   * Writes a terrain run's results file and its summary (R10.T16.d) from its renderer's record;
   * the main process judges it, and a run whose checks fail ends with 1 whatever its end says.
   */
  writeTerrainResults(record: TerrainCheckRecord): Promise<SpikeResultsPaths>;
  /**
   * Saves a picture of the page under the run's captures, named for the phase recording and
   * `label` (lower case letters, digits and hyphens, at most 48): a terrain run's look at a moment
   * of its script, for the looks by eye (R10.T16.d).
   */
  capture(label: string): Promise<void>;
  /**
   * Ends the run, and the app with it: 0 for `pass`, 1 for `fail` (and for a terrain run whose
   * results fail).
   */
  end(outcome: SpikeEnd): Promise<void>;
}
