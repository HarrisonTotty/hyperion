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
 * What the descent spike's renderer reports at the end of a run, for the results file (R05.T14.c);
 * T14.a gathers it.
 */
export interface DescentSpikeReport {
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
}
