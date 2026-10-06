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

/** The spike's quality setting (R05 Design note 26). */
export type SpikeSettingName = "high" | "low";

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
 * The views check's narrow functions (plan R07, T20), present only when the client was launched
 * with `--views-check`; the main process checks each call's sender and arguments.
 */
export interface ViewsCheckApi {
  readonly launch: ViewsCheckLaunch;
  /** Starts a phase's trace window. */
  startPhase(name: ViewsCheckPhaseName): Promise<void>;
  /**
   * Ends it: stops the trace, reduces it over the measured `window` (marked in the trace by a
   * `performance.measure` span) and saves a capture of the page.
   */
  endPhase(name: ViewsCheckPhaseName, window: ViewsCheckWindow): Promise<void>;
  /** Asks the person at a shown run whether every view is the right way up, and keeps the answer. */
  askRightWayUp(): Promise<void>;
  /** Writes the results file and its summary from the renderer's record. */
  writeResults(record: ViewsCheckRecord): Promise<SpikeResultsPaths>;
  /** Ends the run, and the app with it: 0 for `pass`, 1 for `fail`. */
  end(outcome: SpikeEnd): Promise<void>;
}
