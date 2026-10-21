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

/**
 * The world the spike flies (`--world`, plan R10, R10.T14.c): R05's test planet, or `TERRAIN TEST`'s
 * moon through the terrain session; the renderer's `SpikeWorldName` is this type.
 */
export type SpikeWorldName = "test-planet" | "terrain-test";

/**
 * The style the spike's main view draws in over a generated world (`--style`, R10.T14.c), through
 * `VIEW`'s frame drawer; the test planet's main view is the spike's own lit view.
 */
export type SpikeStyleName = "photorealistic" | "wireframe";

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
  /** The world flown (`--world`, R10.T14.c): the test planet unless given. */
  readonly world: SpikeWorldName;
  /**
   * The main view's style over a generated world (`--style`, R10.T14.c): photorealistic unless
   * given; `null` over the test planet, which takes no `--style`, its main view being the spike's
   * own lit view (`SPIKE LIT`).
   */
  readonly style: SpikeStyleName | null;
  /**
   * `on` where the run measures the terrain caches' sizes (`--sizes on`, plan R10, R10.T14.b;
   * decision-r10-t14b-measure): over the terrain world, a second view, the counting of each pass,
   * the bake clock and the 4 Hz readout, with a `sizes` block in the report. Refused over the test
   * planet. Absent is off, the default, so that an unmeasured launch, and the options its results
   * file records, are as they were before the option.
   */
  readonly sizes?: "on";
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
 * A figure of the measured sizes (plan R10, R10.T14.b): a value, or `null` with the reason it is
 * missing, the results file's own `Measured` form.
 */
export type SpikeFigure<T> =
  { readonly value: T; readonly reason: null } | { readonly value: null; readonly reason: string };

/** Which terrain cache a pass is: the lit one (photorealistic views) or the wireframe's. */
export type SpikeSizesCache = "lit" | "wireframe";

/** Where in the script a sample or a peak fell. */
export interface SpikeSizesMoment {
  /** Script time, s. */
  readonly scriptTimeS: number;
  /** The segment under way, or `null` past the script's end. */
  readonly segment: string | null;
  /**
   * The main view's camera above the ground, m: the craft's own pose, which the main view's
   * selection sees, above the floor of the stretch under way (`DescentPose.heightAboveFloorM`, the
   * highest ground the measurement found under the stretch), so at most its height above the
   * ground beneath it; `null` where none is known.
   */
  readonly heightAboveGroundM: number | null;
}

/** A count's peak over the run: the most patches at a sample, and the first sample it fell at. */
export interface SpikeSizesPeak {
  /** Patches; 0 where no sample counted it. */
  readonly patches: number;
  /** `null` where no sample counted it. */
  readonly at: SpikeSizesMoment | null;
}

/** One count's peaks: each described view's, and the union's (the leaves any view sees). */
export interface SpikeSizesCountPeaks {
  readonly views: ReadonlyArray<SpikeSizesPeak>;
  readonly union: SpikeSizesPeak;
}

/** The forced region's peak, counted once, and how many of it some view saw then. */
export interface SpikeSizesForcedPeak {
  readonly patches: number;
  readonly at: SpikeSizesMoment | null;
  readonly seen: number;
}

/** A share's peak over the run, 0 to 1, and where it fell; `null` where no sample gave one. */
export interface SpikeSizesShare {
  readonly share: number | null;
  readonly at: SpikeSizesMoment | null;
}

/** A sizing figure in slots (unrounded, as `cacheBudget.ts`' rules give them) and bytes (whole slots). */
export interface SpikeSizesRuleFigure {
  readonly slots: number;
  readonly bytes: number;
}

/**
 * The sizing rule's four forms (decision-r10-t14b-measure §2): the ruled Σ_views max(1.3 AR_peak,
 * 3 F_peak) + forced, (a) its largest same-sample value, (b) the union form, (c) the frustum rule.
 */
export interface SpikeSizesRule {
  readonly ruled: SpikeSizesRuleFigure;
  readonly sameSample: SpikeSizesRuleFigure;
  readonly union: SpikeSizesRuleFigure;
  readonly frustum: SpikeSizesRuleFigure;
}

/**
 * The diagnostic occluder's price on a view or the union (§5), in the rule's slots: each term's
 * difference at the peaks as the rule weighs it, and its share of the sized all-round peak.
 */
export interface SpikeSizesPriceTerm {
  /** AR_peak − AR′_peak, patches. */
  readonly allRoundPatches: number;
  /** F_peak − F′_peak, patches. */
  readonly frustumPatches: number;
  /** The all-round term's price, 1.3 (AR_peak − AR′_peak), slots. */
  readonly allRoundSlots: number;
  /** The frustum term's price, 3 (F_peak − F′_peak), slots. */
  readonly frustumSlots: number;
  /**
   * Each slots ÷ the sized all-round peak, the ruled sum's all-round term Σ_views 1.3 AR_peak
   * ({@link SpikeSizesPrice.sizedAllRoundSlots}), so that a view's all-round share is
   * (AR − AR′) ÷ AR_peak where it is the only view; `null` where that peak is 0.
   */
  readonly allRoundShare: number | null;
  readonly frustumShare: number | null;
}

/** The A/B's price (§5), over the samples that made both the plain and the diagnostic counts. */
export interface SpikeSizesPrice {
  readonly views: ReadonlyArray<SpikeSizesPriceTerm>;
  readonly union: SpikeSizesPriceTerm;
  /** The sized all-round peak, the ruled sum's all-round term Σ_views 1.3 AR_peak, slots. */
  readonly sizedAllRoundSlots: number;
  /** The rule's own difference, the ruled figure less its diagnostic one (rule − rule′), slots. */
  readonly ruledSlots: number;
  /**
   * That ÷ the sized all-round peak; `null` at 0. The reading §5's 10% test takes: over 0.1 is a
   * finding for main.
   */
  readonly ruledShare: number | null;
}

/** One level's bound ratio k_n = ε_n P₀ ÷ e_n (T14.a's k₀). */
export interface SpikeSizesBoundRatio {
  readonly level: number;
  /** ε_n, m. */
  readonly boundM: number;
  /** ε_n plus the chord's sagitta, the error selection charges, m. */
  readonly chargedM: number;
  /** e_n, the level's mean patch edge, m. */
  readonly edgeM: number;
  readonly ratio: number;
  readonly chargedRatio: number;
}

/** The bound ratios of one level table the pass selected over. */
export interface SpikeSizesBoundTable {
  /** The table's number, from 0 in the order the pass met them. */
  readonly version: number;
  /** The script time of its first selection, s. */
  readonly firstSeenS: number;
  readonly levels: ReadonlyArray<SpikeSizesBoundRatio>;
}

/** One level's box heights: the frustum count's seen leaves' extents along their normals, m. */
export interface SpikeSizesBoxHeights {
  readonly level: number;
  readonly maxM: number;
  /** The median at F∪'s peak sample, or `null` where none of the level was seen there. */
  readonly medianAtFrustumPeakM: number | null;
}

/** A moment of the forced region's residency: when, and the pass's forced patches then. */
export interface SpikeSizesForcedMark {
  /** Script time, s. */
  readonly atS: number;
  /** The forced patches of the pass's last selection, and those of them resident. */
  readonly forced: number;
  readonly forcedResident: number;
}

/** The forced region's lead before contact and its residency at contact (T14.c's finding 3). */
export interface SpikeSizesContact {
  /** The pass's first selection holding a forced patch, script s, or `null`. */
  readonly firstForcedS: number | null;
  /** The first sample whose forced patches were every one resident, script s, or `null`. */
  readonly allResidentS: number | null;
  /** Whether every later sample's were all resident too; `null` where no sample's ever were. */
  readonly allResidentHeld: boolean | null;
  /**
   * Touchdown: the first frame whose pose had reached the ground, its camera down to its rest over
   * the site (`touchedDown`), with the forced patches then; `null` where the run ended before it.
   */
  readonly touchdown: SpikeSizesForcedMark | null;
  /** The forced region's lead before contact, touchdown − all resident, s; `null` where either is. */
  readonly leadS: number | null;
  /**
   * The contact by the session's rule: the first frame whose pass's selection held a contact
   * (`passContact`), which fires in the step that first selects the forced patches.
   */
  readonly sessionContact: SpikeSizesForcedMark | null;
}

/** The 50th and 95th percentiles (nearest rank) and the largest of some times, ms; `null` with none. */
export interface SpikeSizesSpread {
  readonly p50: number | null;
  readonly p95: number | null;
  readonly max: number | null;
}

/** A figure's mean and largest over a segment's frames. */
export interface SpikeSizesMeanMax {
  readonly mean: number;
  readonly max: number;
}

/** One of a pass's described views: the cache's views in its style (§4's table). */
export interface SpikeSizesView {
  /** Its ID in the terrain session: the main view's `spike`, the second view's `instrument-1`. */
  readonly id: string;
  readonly style: SpikeStyleName;
  /** Its streaming weight, by which it is matched in the pass's selection input: 1 or 0.25. */
  readonly weight: number;
}

/**
 * One whole-number column of a pass's 1 Hz samples, in the results' memory-series form: a reading
 * a sample, -1 within one of the gaps, each gap a stretch of samples with its reason.
 */
export interface SpikeSizesColumn {
  readonly samples: ReadonlyArray<number>;
  readonly gaps: ReadonlyArray<{
    /** The first sample's index, and the last's, inclusive. */
    readonly from: number;
    readonly to: number;
    readonly reason: string;
  }>;
}

/** The five counts at each sample (§2): F, F₀, AR, F′ and AR′, patches. */
export interface SpikeSizesCountColumns {
  readonly frustum: SpikeSizesColumn;
  readonly frustumNoMargin: SpikeSizesColumn;
  readonly allRound: SpikeSizesColumn;
  readonly frustumDiagnostic: SpikeSizesColumn;
  readonly allRoundDiagnostic: SpikeSizesColumn;
}

/**
 * A pass's samples, once a second of script time, in column form: one entry a sample in each.
 *
 * @remarks
 * The columns are those the record reads at 1 Hz: the moment, each view's five counts, the forced
 * region and its residency, the slots in use and §3's shares' numerators. The pass's own counts
 * beside them (its selection, its budget, its resident patches and its pins) are kept by segment
 * instead ({@link SpikeSizesPass.logged}), and the seconds and the segments are written compactly,
 * so that the block stays within its budget, `SIZES_BLOCK_BUDGET_BYTES` formatted, on the longest
 * run.
 */
export interface SpikeSizesSeries {
  /**
   * The whole second of script time each sample was the first selection in, s, compactly: from
   * `first`, one a second but for the `skipped` seconds, which no selection fell in, so that
   * sample i is the i-th second from `first` that is not skipped.
   */
  readonly seconds: { readonly first: number; readonly skipped: ReadonlyArray<number> };
  /**
   * Each sample's segment, compactly, as runs of samples: each run's segment, its index in
   * {@link SpikeSizesReport.segments} or -1 past the script's end, from the run's first sample's
   * index to the next run's.
   */
  readonly segmentRuns: ReadonlyArray<{ readonly segment: number; readonly from: number }>;
  /** {@link SpikeSizesMoment.heightAboveGroundM} at each sample, whole m. */
  readonly heightAboveGroundM: ReadonlyArray<number | null>;
  /** Per described view, in the pass's `views` order. */
  readonly views: ReadonlyArray<SpikeSizesCountColumns>;
  /**
   * The unions; `null` for a pass of one described view, whose own columns stand for them except
   * at the samples {@link SpikeSizesSeries.unionDiffers} counts, where a view it does not describe
   * (the main view's wireframe stand-in) was in its input too. Left out so that the file stays
   * within the repository's added-file limit.
   */
  readonly union: SpikeSizesCountColumns | null;
  /** Samples at which a union count differed from the one described view's; 0 with the unions given. */
  readonly unionDiffers: number;
  /** The forced leaves, seen or not. */
  readonly forced: ReadonlyArray<number>;
  /**
   * The pass's own selection's forced patches resident: the same patches as the counts' forced
   * leaves, since selection never refuses a forced split.
   */
  readonly forcedResident: SpikeSizesColumn;
  /** |AR∪ ∩ resident|, §3's resident share's numerator over AR∪. */
  readonly allRoundResident: SpikeSizesColumn;
  /** |AR∪ ∩ every key the pass has selected so far|, the swept share's numerator. */
  readonly allRoundSwept: ReadonlyArray<number>;
  /** The cache's slots in use, as the last retain left them. */
  readonly usedSlots: SpikeSizesColumn;
}

/**
 * A pass's own counts in one segment, each at its largest over the segment's samples (§2, "Logged
 * beside"), which the samples' columns leave out.
 */
export interface SpikeSizesLoggedSegment {
  readonly segment: string | null;
  readonly samples: number;
  /** The pass's own budgeted selection's patches, and the samples whose budget limited it. */
  readonly selectedMax: number;
  readonly limitedSamples: number;
  /** Its selected patches resident; `null` where its input held no ranges. */
  readonly selectedResidentMax: number | null;
  /**
   * The cache's slots in use and its pins, forced and drawn outside the forced region, as the last
   * retain set them; `null` where the pass's ranges were no patch cache.
   */
  readonly usedSlotsMax: number | null;
  readonly pinnedForcedMax: number | null;
  readonly pinnedDrawnMax: number | null;
}

/** A pass's streaming in one segment, from its last step after each frame's step (§6(d), §6(e)). */
export interface SpikeSizesStreaming {
  readonly segment: string | null;
  /** Frames read: those with a last step. */
  readonly frames: number;
  /** Patches drawn, standing in (drawn by a coarser resident ancestor) and missing. */
  readonly drawn: SpikeSizesMeanMax;
  readonly standingIn: SpikeSizesMeanMax;
  readonly missing: SpikeSizesMeanMax;
  /** Standing in ÷ selected, 0 to 1, over the frames with a selection. */
  readonly standInShare: SpikeSizesMeanMax;
  /** Frames whose selection the budget limited. */
  readonly limitedFrames: number;
  /** Patches the pass made resident, and a second over the segment's frames' script time. */
  readonly residents: number;
  readonly residentsPerS: number | null;
}

/** Bakes posted and answered at the workers, and their times (`BakeClock`). */
export interface SpikeSizesBakeTimes {
  readonly posted: number;
  readonly baked: number;
  readonly failed: number;
  readonly unsurveyed: number;
  /** Post to reply, ms. */
  readonly postToReplyMs: SpikeSizesSpread;
  /** The estimated worker time, reply − max(post, the worker's previous reply), ms. */
  readonly computeMs: SpikeSizesSpread;
}

/** The flight's bakes, or a segment's, with their rate and the residents. */
export interface SpikeSizesBakeWindow extends SpikeSizesBakeTimes {
  readonly residents: number;
  /** Baked − residents (§6(c)): below 0 where earlier bakes became resident in it. */
  readonly bakedNeverResident: number;
  readonly spanMs: number;
  readonly bakesPerSecond: number | null;
}

/** One segment's figures beside the passes': the workers' bakes, and the profile's ground speed. */
export interface SpikeSizesSegment {
  readonly segment: string | null;
  readonly bakes: SpikeSizesBakeWindow;
  /** The ground speed ÷ the height above the floor over the segment's frames, s⁻¹. */
  readonly groundSpeedPerHeight: SpikeSizesMeanMax | null;
}

/** A count of messages of one kind. */
export interface SpikeSizesMessageCount {
  readonly kind: string;
  readonly count: number;
}

/** The bake clock's figures at the spike's worker boundary (§6(c)). */
export interface SpikeSizesBakes {
  /** The workers wrapped, restarts included. */
  readonly workersMade: number;
  readonly total: SpikeSizesBakeWindow;
  readonly byWorker: ReadonlyArray<SpikeSizesBakeTimes & { readonly worker: number }>;
  readonly requests: ReadonlyArray<SpikeSizesMessageCount>;
  readonly replies: ReadonlyArray<SpikeSizesMessageCount>;
  readonly inFlight: number;
  readonly abandoned: number;
}

/** A pass's counted sizes over the run (§2, §3, §5), with its streaming and its samples. */
export interface SpikeSizesPass {
  readonly cache: SpikeSizesCache;
  /** Its described views, the cache's style's; another view in its input counts in the unions. */
  readonly views: ReadonlyArray<SpikeSizesView>;
  /**
   * "stand-in only, not sized" and why, for a pass with no described view: one that only stood
   * in, as the wireframe pass a high photorealistic run's views draw while their pipelines
   * compile. It is no sized cache: its rule's figures are `null` and its samples are left out of
   * the file. `null` for a sized pass.
   */
  readonly notSized: string | null;
  /** B, the first pass's of the cache. */
  readonly bytesPerSlot: number;
  readonly slots: number | null;
  /**
   * The times its counter was attached to the session's pass of the cache: one, or more where the
   * session made the pass anew, or let it go and took it again.
   */
  readonly attachments: number;
  readonly samples: number;
  readonly failedSamples: number;
  readonly firstFailure: string | null;
  readonly counts: {
    readonly frustum: SpikeSizesCountPeaks;
    readonly frustumNoMargin: SpikeSizesCountPeaks;
    readonly allRound: SpikeSizesCountPeaks;
    readonly frustumDiagnostic: SpikeSizesCountPeaks;
    readonly allRoundDiagnostic: SpikeSizesCountPeaks;
  };
  readonly forced: SpikeSizesForcedPeak;
  readonly residentShare: SpikeSizesShare;
  readonly sweptShare: SpikeSizesShare;
  /** Whether the resident share reached 100% at some sample (§3's criterion). */
  readonly allRoundEverResident: boolean;
  /** Slots ÷ |AR∪| at AR∪'s peak. */
  readonly capAtAllRoundPeak: number | null;
  /** The deepest diagnostic occluder the primed counts used, m above the datum. */
  readonly deepestOccluderM: SpikeFigure<number>;
  /** The as-built occluder, `lowestHeightM` of the pass's last level table, m above the datum. */
  readonly asBuiltOccluderM: number;
  readonly boundRatios: ReadonlyArray<SpikeSizesBoundTable>;
  readonly boxHeights: ReadonlyArray<SpikeSizesBoxHeights>;
  /** The five counts' CPU time a sample, ms. */
  readonly countingMs: SpikeSizesSpread;
  /** The rule's forms on the plain counts; `null` for a pass {@link SpikeSizesPass.notSized}. */
  readonly rule: SpikeSizesRule | null;
  /** The same on the diagnostic counts; `null` where none was made, or the pass is not sized. */
  readonly ruleDiagnostic: SpikeSizesRule | null;
  readonly occluderPrice: SpikeSizesPrice | null;
  readonly contact: SpikeSizesContact;
  readonly streaming: ReadonlyArray<SpikeSizesStreaming>;
  /** The pass's own counts by segment, in the order the samples met them. */
  readonly logged: ReadonlyArray<SpikeSizesLoggedSegment>;
  /** Its 1 Hz samples; `null` for a pass {@link SpikeSizesPass.notSized}, whose are left out. */
  readonly series: SpikeSizesSeries | null;
}

/** A peak of the session's memory at 1 Hz: its bytes, and when it was first read, script s. */
export interface SpikeSizesBytePeak {
  readonly bytes: number;
  readonly atS: number;
}

/**
 * A peak of the engine's live bytes: its bytes, and the script time it was first reached at, s,
 * `null` where that was before the flight.
 */
export interface SpikeSizesEnginePeak {
  readonly bytes: number;
  readonly atS: number | null;
}

/**
 * The engine's own tally of its allocations (`RenderEngine.onAllocation`), from the run's control's
 * subscription on: each creation adds its bytes to its memory category's live bytes, the
 * destruction of an allocation whose creation was seen takes them away, and uploads are left out.
 */
export interface SpikeSizesEngineTally {
  /** Each memory category's peak live bytes, in the order of their first creations. */
  readonly categories: ReadonlyArray<SpikeSizesEnginePeak & { readonly category: string }>;
  /** The whole tally's peak live bytes: the peak of the sum, not the sum of the categories' peaks. */
  readonly peak: SpikeSizesEnginePeak;
  /**
   * Destructions of an allocation whose creation was not seen, made before the subscription, and
   * left out of the tally.
   */
  readonly unseenDestroyed: number;
}

/**
 * The shadow cascades as built (R10.T9.b): the engine's `shadows` category at its peak, or none
 * where nothing was allocated under it, beside T9.a's spec figure.
 */
export interface SpikeSizesCascades {
  /** The `shadows` category's peak live bytes in the engine's tally; 0 where none was allocated. */
  readonly bytes: number;
  readonly source: "engine" | "none";
  /** Why none was allocated (the low setting, or the wireframe alone); `null` where it was. */
  readonly reason: string | null;
  /** `CASCADE_BYTES`, T9.a's spec figure (67,108,864 B). */
  readonly specBytes: number;
  /** Whether the engine allocated other than the spec's bytes; `false` where none was allocated. */
  readonly differsFromSpec: boolean;
}

/** The resident total's parts, itemized, and the engine's own tally beside them (§6(d)). */
export interface SpikeSizesMemory {
  /** `TerrainSession.memory()` samples, at 1 Hz. */
  readonly samples: number;
  /** The height workers' copies of the fields together, and the pool's own copy, at their peaks. */
  readonly workerFieldBytes: SpikeSizesBytePeak;
  readonly keptFieldBytes: SpikeSizesBytePeak;
  /**
   * The session's textures by memory category, each at its peak, as the session reports them: its
   * masks and class maps under `coarse-field-gpu`, and on the high setting the cascades under
   * `shadows` and their placeholder under `other` (R10.T9.b).
   */
  readonly sessionGpu: ReadonlyArray<SpikeSizesBytePeak & { readonly category: string }>;
  /** Each cache's allocation, slot count × bytes a slot, at its largest. */
  readonly caches: ReadonlyArray<{
    readonly cache: SpikeSizesCache;
    readonly slotCount: number;
    readonly bytesPerSlot: number;
    readonly allocationBytes: number;
  }>;
  readonly cascades: SpikeSizesCascades;
  /** The engine's own tally: the app's own GPU figure, beside the itemized one, for the driver's. */
  readonly engine: SpikeSizesEngineTally;
  /**
   * The caches' allocations, the session's textures but its `shadows`, and the cascades as the
   * engine allocated them, B: the session's own `shadows` figure is the same set's, counted once.
   */
  readonly itemizedGpuBytes: number;
  /** That and the workers' field copies, the plan's resident total, B. */
  readonly itemizedTotalBytes: number;
  /** The height workers the session started, and the workers the bake clock wrapped. */
  readonly workersStarted: number;
}

/** The storage binding the caches are held in (§6(g)). */
export interface SpikeSizesBinding {
  /** The adapter's reported `maxStorageBufferBindingSize`, B. */
  readonly adapterBytes: SpikeFigure<number>;
  /** The limit the engine requested of the device (`requiredLimits`), B. */
  readonly requestedBytes: SpikeFigure<number>;
  /** The device's own (`RenderEngine.capabilities`), B. */
  readonly deviceBytes: SpikeFigure<number>;
  /**
   * Each cache's slots as last read, how its layout met the device's binding, and its binding
   * ceiling, the most slots whose storage fields fit one binding (`bindingCeilingSlots`), at the
   * requested binding and at WebGPU's default 128 MiB; `null` where no storage field has bytes.
   */
  readonly caches: ReadonlyArray<{
    readonly cache: SpikeSizesCache;
    readonly slotCount: number;
    /**
     * The slots the layout was asked for and its fallback past the binding; absent from a block
     * written before R10.T14.b's clamp, which reads as before.
     */
    readonly layout?: SpikeSizesCacheLayout;
    readonly ceilingAtRequestedSlots: number | null;
    readonly ceilingAtDefaultSlots: number | null;
  }>;
}

/**
 * How a cache's layout met the device's storage binding (`TerrainLayout`'s,
 * decision-r10-t14b-constants §4): the slots the setting asked for and the fallback that kept or
 * clamped them.
 *
 * @remarks
 * The cache's `slotCount` is `requestedSlotCount` where the fallback is `none`, and at most it on
 * `binding-limit`.
 */
export interface SpikeSizesCacheLayout {
  /** The setting's own slot count, over its cache budget on its own path, before any fallback. */
  readonly requestedSlotCount: number;
  /**
   * `binding-limit` where the layout changed path to `face-differences` or clamped its slots to
   * the device's binding, else `none`.
   */
  readonly fallback: "none" | "binding-limit";
}

/** The second view (§4): the craft instrument at its chase pose as a session view. */
export interface SpikeSizesSecondView {
  /** Its ID in the session (`instrumentViewId(1)`) and its engine view's name. */
  readonly id: string;
  readonly engineName: string;
  /** The style asked of it: the main view's on high, the wireframe on low. */
  readonly style: SpikeStyleName;
  readonly weight: number;
  /** Its presented viewport, device px, each time it changed, from script s. */
  readonly viewports: ReadonlyArray<{
    readonly widthPx: number;
    readonly heightPx: number;
    readonly fromS: number;
  }>;
}

/** `VIEW`'s terrain readout asked on the main view at 4 Hz, its answers not shown (§6(f)). */
export interface SpikeSizesReadouts {
  /** ms between publishes. */
  readonly intervalMs: number;
  readonly publishes: number;
  /** Publishes whose snapshot held an answer. */
  readonly answeredPublishes: number;
  /** Queries posted to the height workers in the flight, and their answers and refusals. */
  readonly asked: number;
  readonly answered: number;
  readonly failed: number;
  /**
   * The times the readout's guards asked to fail the session's computation (after 250 ms of
   * refusals): counted and logged, and never passed to the session, so that the measurement's
   * readout cannot end the run.
   */
  readonly failComputationCalls: number;
}

/** How often the lit and wireframe passes' forced key sets differed (T14.c's finding 4). */
export interface SpikeSizesForcedSets {
  /** Frames after the step at which both passes had a last step, and at which their sets differed. */
  readonly comparedFrames: number;
  readonly differingFrames: number;
  /** The first such frame, script s. */
  readonly firstDifferingS: number | null;
  /** The largest symmetric difference, patches. */
  readonly largestDifference: number;
}

/**
 * The measured sizes of a `--sizes on` run (plan R10, R10.T14.b; decision-r10-t14b-measure): each
 * pass's counted peaks, rules, shares and samples, the streaming, the bakes, the memory itemized and
 * as the engine tallied it, the binding and the readout.
 */
export interface SpikeSizesReport {
  /** The segments in the order the script met them, which the series' `segment` indexes. */
  readonly segments: ReadonlyArray<string>;
  readonly secondView: SpikeSizesSecondView;
  /**
   * The field header's lowest sampled height, the last read, m above the datum: its sea level on a
   * body with no main liquid, where the header defines it as the lowest cell's elevation.
   */
  readonly sampledLowestM: SpikeFigure<number>;
  readonly passes: ReadonlyArray<SpikeSizesPass>;
  /** Missing where the two passes never had a last step in the same frame. */
  readonly forcedSets: SpikeFigure<SpikeSizesForcedSets>;
  readonly segmentFigures: ReadonlyArray<SpikeSizesSegment>;
  readonly bakes: SpikeSizesBakes;
  readonly memory: SpikeSizesMemory;
  readonly binding: SpikeSizesBinding;
  readonly readouts: SpikeSizesReadouts;
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
  /**
   * The measured sizes of a `--sizes on` run (plan R10, R10.T14.b); absent on every other run,
   * whose report is as before.
   */
  readonly sizes?: SpikeSizesReport;
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
