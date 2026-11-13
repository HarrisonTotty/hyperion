/**
 * The measured sizes in the descent spike's results file (plan R10, R10.T14.b;
 * decision-r10-t14b-measure §6(d), §6(g)): the renderer's `sizes` block, with what the main process
 * adds to it, the machine's 1-minute load at the start, each minute and the end, and the driver's
 * memory figures against the renderer's itemized ones and the engine's own tally; its check and its
 * summary's section.
 *
 * @remarks
 * The block is optional and the schema's version stays 6: a file without it, every run's but a
 * `--sizes on` run's, reads as before. The run block keeps its shape, its options recording
 * `sizes: "on"` with the others on a measured run and nothing on any other, so that no other run's
 * file gains a field: the binding's limits and the load readings sit in the `sizes` block instead
 * (a deviation the plan's T14.b record states, from the ruling's "run block adds").
 *
 * The block's budget is {@link SIZES_BLOCK_BUDGET_BYTES} formatted, on the longest run, 1,230
 * samples a pass, so that a results file, about 150–165 kB without it, stays well under the
 * repository's 512,000 B limit for an added file: the samples keep the columns the record reads at
 * 1 Hz, and the pass's own counts go by segment.
 *
 * The driver's figure is the memory series' `nvidia-smi` device reading less its pre-launch
 * baseline (`memory.peakNvidiaDeviceLessBaselineBytes`). Two of the renderer's figures stand
 * against it: the itemized GPU bytes, the plan's parts (the caches, the coverage mask and class map,
 * the cascades as allocated), and the engine's whole tally at its peak, every buffer and texture
 * the app made from the run's control's subscription on. Since the local LLM shares the card, the
 * device is read at the run's end too, at the last memory sample, and the rest of the card there,
 * the device's reading less the GPU process's, stands beside the pre-launch baseline as its
 * second reading.
 */

import type {
  SpikeSizesCacheLayout,
  SpikeSizesCascades,
  SpikeSizesEngineTally,
  SpikeSizesForcedMark,
  SpikeSizesMoment,
  SpikeSizesPass,
  SpikeSizesReport,
  SpikeSizesRule,
} from "../preload/api";
import { type LoadSources, readLoadAverage } from "./machineLoad";
import { type Measured, measured, missing } from "./measured";
import type { SpikeSetting } from "./results";
import { CASCADE_CATEGORY, checkSchema, type Schema, sizesReportProblems } from "./sizesSchema";

/**
 * The `sizes` block's budget, bytes, as Prettier formats it in the results file, on the worst
 * case: a 1,230-second run's two passes of one view each (low photorealistic, the larger) or one
 * pass of two views (high photorealistic).
 */
export const SIZES_BLOCK_BUDGET_BYTES = 250_000;

/** One reading of the machine's load: when, and its 1-minute average. */
export interface LoadReading {
  /** Since the sampler's start, s. */
  readonly tS: number;
  readonly oneMinute: number;
}

/** How often the load is read, ms: each minute (§6(g)). */
export const LOAD_INTERVAL_MS = 60_000;

/** What a {@link LoadSampler} reads: the load average, and a clock. */
export interface LoadClockSources extends LoadSources {
  /** ms. */
  nowMs(): number;
}

/**
 * Reads the machine's 1-minute load at {@link LoadSampler.start}, each minute, and at
 * {@link LoadSampler.stop} (decision-r10-t14b-measure §6(g), "the 1-min load (start, each minute,
 * end)"), beside the memory sampler's loop.
 */
export class LoadSampler {
  readonly #sources: LoadClockSources;
  readonly #intervalMs: number;
  #readings: LoadReading[] = [];
  /** Why there is no reading, where the platform keeps none. */
  #reason: string | null = null;
  #timer: ReturnType<typeof setInterval> | undefined;
  #startMs = 0;

  constructor(sources: LoadClockSources, intervalMs = LOAD_INTERVAL_MS) {
    this.#sources = sources;
    this.#intervalMs = intervalMs;
  }

  /** Starts afresh, with a reading at once. */
  start(): void {
    if (this.#timer !== undefined) {
      return;
    }
    this.#readings = [];
    this.#reason = null;
    this.#startMs = this.#sources.nowMs();
    this.#read();
    this.#timer = setInterval(() => {
      this.#read();
    }, this.#intervalMs);
  }

  /** Stops with a last reading, and gives every reading, or why there is none. */
  stop(): Measured<ReadonlyArray<LoadReading>> {
    if (this.#timer !== undefined) {
      clearInterval(this.#timer);
      this.#timer = undefined;
      this.#read();
    }
    if (this.#reason !== null) {
      return missing(this.#reason);
    }
    return this.#readings.length === 0
      ? missing("the load was never read")
      : measured([...this.#readings]);
  }

  #read(): void {
    let load: ReturnType<typeof readLoadAverage>;
    try {
      load = readLoadAverage(this.#sources);
    } catch (error: unknown) {
      this.#reason = error instanceof Error ? error.message : String(error);
      return;
    }
    if (load.value === null) {
      this.#reason = load.reason;
      return;
    }
    const tS = (this.#sources.nowMs() - this.#startMs) / 1000;
    this.#readings.push({ tS: Math.round(tS * 1000) / 1000, oneMinute: load.value[0] });
  }
}

/**
 * The driver's memory figures beside the renderer's, its itemized GPU bytes and the engine's own
 * tally (§6(d)), bytes.
 */
export interface SizesDriver {
  /** `nvidia-smi`'s device memory used before the launch. */
  readonly preLaunchBaselineBytes: Measured<number>;
  /** The device's peak over the run less that baseline: the driver's figure. */
  readonly peakLessBaselineBytes: Measured<number>;
  /** `nvidia-smi`'s figure for the GPU process at its peak. */
  readonly gpuProcessPeakBytes: Measured<number>;
  /** At the run's last memory sample: the device's reading, and the GPU process's. */
  readonly endDeviceBytes: Measured<number>;
  readonly endGpuProcessBytes: Measured<number>;
  /**
   * The rest of the card at the run's end, the device less the GPU process: the baseline's second
   * reading, the local LLM's and any other process's memory.
   */
  readonly endOthersBytes: Measured<number>;
  /** That less the pre-launch baseline: how far the rest of the card moved over the run. */
  readonly othersDriftBytes: Measured<number>;
  /** The renderer's itemized GPU bytes less the driver's figure. */
  readonly itemizedLessDriverBytes: Measured<number>;
  /** The engine's whole tally at its peak, the app's own GPU figure, less the driver's. */
  readonly engineLessDriverBytes: Measured<number>;
}

/** The results file's `sizes` block: the renderer's, with the main process's load and driver. */
export interface SizesResults extends SpikeSizesReport {
  /** The machine's 1-minute load at the run's start, each minute and its end, or why none. */
  readonly load: Measured<ReadonlyArray<LoadReading>>;
  readonly driver: SizesDriver;
}

/** What the driver's figures are made from, of the results' memory. */
export interface SizesDriverInput {
  /** `nvidia-smi`'s device memory used before the launch, bytes, or `null` off NVIDIA. */
  readonly nvidiaBaselineBytes: number | null;
  /** The results' `memory.peakNvidiaDeviceLessBaselineBytes`. */
  readonly peakLessBaselineBytes: Measured<number>;
  /** The results' `memory.peakNvidiaGpuProcessBytes`. */
  readonly gpuProcessPeakBytes: Measured<number>;
  /** The run's last memory sample's device and GPU-process readings, bytes; `null` with none. */
  readonly last: {
    readonly deviceBytes: number | null;
    readonly gpuProcessBytes: number | null;
  } | null;
}

/** `a − b` of two figures, missing with the first reason where either is. */
function less(a: Measured<number>, b: Measured<number>): Measured<number> {
  if (a.value === null) {
    return missing(a.reason);
  }
  return b.value === null ? missing(b.reason) : measured(a.value - b.value);
}

/** The results' `sizes` block from the renderer's, `load` and the run's memory. */
export function buildSizesResults(
  sizes: SpikeSizesReport,
  load: Measured<ReadonlyArray<LoadReading>>,
  driver: SizesDriverInput,
): SizesResults {
  const baseline: Measured<number> =
    driver.nvidiaBaselineBytes === null
      ? missing("no nvidia-smi baseline (not NVIDIA)")
      : measured(driver.nvidiaBaselineBytes);
  const last = driver.last;
  const endDeviceBytes: Measured<number> =
    last === null
      ? missing("no memory sample")
      : last.deviceBytes === null
        ? missing("no nvidia-smi reading at the run's last memory sample")
        : measured(last.deviceBytes);
  const endGpuProcessBytes: Measured<number> =
    last === null
      ? missing("no memory sample")
      : last.gpuProcessBytes === null
        ? missing("nvidia-smi does not list the GPU process at the run's last memory sample")
        : measured(last.gpuProcessBytes);
  const endOthersBytes = less(endDeviceBytes, endGpuProcessBytes);
  return {
    ...sizes,
    load,
    driver: {
      preLaunchBaselineBytes: baseline,
      peakLessBaselineBytes: driver.peakLessBaselineBytes,
      gpuProcessPeakBytes: driver.gpuProcessPeakBytes,
      endDeviceBytes,
      endGpuProcessBytes,
      endOthersBytes,
      othersDriftBytes: less(endOthersBytes, baseline),
      itemizedLessDriverBytes: less(
        measured(sizes.memory.itemizedGpuBytes),
        driver.peakLessBaselineBytes,
      ),
      engineLessDriverBytes: less(
        measured(sizes.memory.engine.peak.bytes),
        driver.peakLessBaselineBytes,
      ),
    },
  };
}

const LOAD_SCHEMA: Schema<SizesResults["load"]> = {
  figure: { list: { fields: { tS: "nonNegative", oneMinute: "nonNegative" } } },
};

const DRIVER_SCHEMA: Schema<SizesDriver> = {
  fields: {
    preLaunchBaselineBytes: { figure: "count" },
    peakLessBaselineBytes: { figure: "number" },
    gpuProcessPeakBytes: { figure: "count" },
    endDeviceBytes: { figure: "count" },
    endGpuProcessBytes: { figure: "count" },
    endOthersBytes: { figure: "number" },
    othersDriftBytes: { figure: "number" },
    itemizedLessDriverBytes: { figure: "number" },
    engineLessDriverBytes: { figure: "number" },
  },
};

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * The problems of a results file's `sizes` block: the renderer's block as its own check reads it
 * (`sizesReportProblems`), and the load and driver figures the main process added.
 */
export function sizesResultsProblems(value: unknown): string[] {
  if (!isRecord(value)) {
    return ["sizes is not an object"];
  }
  const { load, driver, ...report } = value;
  const problems = sizesReportProblems(report);
  checkSchema(load, LOAD_SCHEMA, "sizes.load", problems);
  checkSchema(driver, DRIVER_SCHEMA, "sizes.driver", problems);
  return problems;
}

/** `x` to `digits` decimals, grouped by thousands. */
function num(x: number, digits = 0): string {
  return x.toLocaleString("en-GB", {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

/** Bytes as MiB to one decimal. */
function mib(bytes: number): string {
  return `${num(bytes / 1024 ** 2, 1)} MiB`;
}

/** A share as a percentage to one decimal, or `—`. */
function percent(share: number | null): string {
  return share === null ? "—" : `${num(100 * share, 1)} %`;
}

/** A figure's text, or `—` with its reason. */
function figureText<T>(figure: Measured<T>, show: (value: T) => string): string {
  return figure.value === null ? `— (${figure.reason})` : show(figure.value);
}

/** Where a peak fell, in words: its script time, segment and height above the ground. */
function momentText(at: SpikeSizesMoment | null): string {
  if (at === null) {
    return "never";
  }
  const height = at.heightAboveGroundM === null ? "height —" : `${num(at.heightAboveGroundM)} m up`;
  return `${num(at.scriptTimeS, 1)} s, ${at.segment ?? "after the script"}, ${height}`;
}

/** The count rows' names, as the ruling writes them. */
const COUNT_ROWS = [
  ["frustum", "F"],
  ["frustumNoMargin", "F₀ (margin 0)"],
  ["allRound", "AR (all round)"],
  ["frustumDiagnostic", "F′ (diagnostic occluder)"],
  ["allRoundDiagnostic", "AR′ (diagnostic occluder)"],
] as const;

/** The rule's forms' names. */
const RULE_ROWS = [
  ["ruled", "Ruled: Σ max(1.3 AR, 3 F) + forced"],
  ["sameSample", "(a) its largest same-sample value"],
  ["union", "(b) max(1.3 AR∪, 3 F∪) + forced"],
  ["frustum", "(c) the frustum rule, Σ 3 F + forced"],
] as const;

/**
 * The share of the sized all-round peak above which the A/B's price is a finding (§5), read on the
 * ruled form: (rule − rule′) ÷ Σ_views 1.3 AR_peak (`SpikeSizesPrice.ruledShare`).
 */
const PRICE_FINDING_SHARE = 0.1;

/** One pass's part of the summary's section. */
function passLines(pass: SpikeSizesPass): string[] {
  const viewNames = pass.views.map((view) => `${view.id} (${view.style}, weight ${view.weight})`);
  const slots = pass.slots;
  const rule = (figures: SpikeSizesRule | null, name: (typeof RULE_ROWS)[number][0]): string =>
    figures === null ? "— | —" : `${num(figures[name].slots, 1)} | ${num(figures[name].bytes)} B`;
  const price = pass.occluderPrice;
  const lastTable = pass.boundRatios.at(-1);
  const { contact } = pass;
  const mark = (at: SpikeSizesForcedMark | null, none: string): string =>
    at === null
      ? none
      : `${num(at.atS, 1)} s (${num(at.forced)} forced, ${num(at.forcedResident)} of them resident)`;
  const lines = [
    `### The ${pass.cache} cache: ${pass.notSized ?? (viewNames.length === 0 ? "no view in its style" : viewNames.join(", "))}`,
    "",
    `${num(pass.samples)} samples${pass.failedSamples === 0 ? "" : `, ${pass.failedSamples} failed (${pass.firstFailure ?? "no message"})`}; attached to ${pass.attachments} ${pass.attachments === 1 ? "pass" : "passes"}; ${slots === null ? "no patch cache" : `${num(slots)} slots`} of ${num(pass.bytesPerSlot)} B.`,
    "",
    `| Count | ${[...pass.views.map((view) => view.id), "union"].join(" | ")} |`,
    `| --- | ${[...pass.views, null].map(() => "---").join(" | ")} |`,
    ...COUNT_ROWS.map(([name, label]) => {
      const peaks = pass.counts[name];
      const cells = [...peaks.views, peaks.union].map(
        (peak) => `${num(peak.patches)} (${momentText(peak.at)})`,
      );
      return `| ${label} | ${cells.join(" | ")} |`;
    }),
    `| Forced, once | ${pass.views.map(() => "").join(" | ")}${pass.views.length === 0 ? "" : " | "}${num(pass.forced.patches)}, ${num(pass.forced.seen)} seen (${momentText(pass.forced.at)}) |`,
    "",
    `| Form | Slots | Bytes | Slots, diagnostic | Bytes, diagnostic | Against the as-built ${slots === null ? "—" : num(slots)} slots |`,
    "| --- | --- | --- | --- | --- | --- |",
    ...RULE_ROWS.map(
      ([name, label]) =>
        `| ${label} | ${rule(pass.rule, name)} | ${rule(pass.ruleDiagnostic, name)} | ${slots === null || slots === 0 || pass.rule === null ? "—" : `× ${num(pass.rule[name].slots / slots, 2)}`} |`,
    ),
    "",
    `- **A/B:** the diagnostic occluder at its deepest ${figureText(pass.deepestOccluderM, (m) => `${num(m, 1)} m`)}, against the as-built ${num(pass.asBuiltOccluderM, 1)} m; ${
      price === null
        ? "no diagnostic count was made, or the pass is not sized"
        : `the price in slots, each a share of the sized all-round peak Σ 1.3 AR (${num(price.sizedAllRoundSlots, 1)} slots): the rule's own, rule − rule′, ${num(price.ruledSlots, 1)} slots, ${percent(price.ruledShare)}, the reading the 10 % test takes${(price.ruledShare ?? 0) > PRICE_FINDING_SHARE ? " (over 10 %: a finding for main)" : ""}; the union's all-round term 1.3 (AR − AR′) ${num(price.union.allRoundSlots, 1)} (${percent(price.union.allRoundShare)}), its frustum term 3 (F − F′) ${num(price.union.frustumSlots, 1)} (${percent(price.union.frustumShare)})`
    }.`,
    `- **The all-round peak's residency:** the resident share's peak ${percent(pass.residentShare.share)} (${momentText(pass.residentShare.at)}), the swept share's ${percent(pass.sweptShare.share)} (${momentText(pass.sweptShare.at)}), the cap at AR∪'s peak ${pass.capAtAllRoundPeak === null ? "—" : `× ${num(pass.capAtAllRoundPeak, 2)}`}; resident at 100 % ${pass.allRoundEverResident ? "at some sample" : "never"}. The descent never turns, its views look along the track and VIEW's LOOK turns are not flown: a "never" is the descent's.`,
    `- **The forced region:** first selected at ${contact.firstForcedS === null ? "—" : `${num(contact.firstForcedS, 1)} s`}; every forced patch resident first at ${contact.allResidentS === null ? "— (never)" : `${num(contact.allResidentS, 1)} s, ${contact.allResidentHeld === true ? "and at every sample after" : "not at every sample after"}`}; touchdown at ${mark(contact.touchdown, "— (none)")}; a lead of ${contact.leadS === null ? "—" : `${num(contact.leadS, 1)} s`} (touchdown − every forced patch resident); the contact by the session's rule at ${mark(contact.sessionContact, "— (none)")}.`,
    `- **Box heights by level** (the frustum count's seen leaves): ${
      pass.boxHeights.length === 0
        ? "none seen"
        : pass.boxHeights
            .map(
              (level) =>
                `L${level.level} max ${num(level.maxM, 1)} m, median at F∪'s peak ${level.medianAtFrustumPeakM === null ? "—" : `${num(level.medianAtFrustumPeakM, 1)} m`}`,
            )
            .join("; ")
    }.`,
    `- **k_n = ε_n P₀ ÷ e_n**, ${pass.boundRatios.length} level ${pass.boundRatios.length === 1 ? "table" : "tables"} met${
      lastTable === undefined
        ? ""
        : `; the last (from ${num(lastTable.firstSeenS, 1)} s): ${lastTable.levels.map((level) => `L${level.level} ${num(level.ratio, 2)}`).join(", ")}`
    }.`,
    `- **The counting's cost:** p50 ${pass.countingMs.p50 === null ? "—" : num(pass.countingMs.p50, 2)} ms, p95 ${pass.countingMs.p95 === null ? "—" : num(pass.countingMs.p95, 2)} ms, max ${pass.countingMs.max === null ? "—" : num(pass.countingMs.max, 2)} ms a sample.`,
    "",
    "| Segment | Frames | Drawn, mean / max | Standing in | Missing | Stand-in share | Limited frames | Made resident / s |",
    "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ...pass.streaming.map(
      (s) =>
        `| ${s.segment ?? "after the script"} | ${num(s.frames)} | ${num(s.drawn.mean, 1)} / ${num(s.drawn.max)} | ${num(s.standingIn.mean, 1)} / ${num(s.standingIn.max)} | ${num(s.missing.mean, 1)} / ${num(s.missing.max)} | ${percent(s.standInShare.mean)} / ${percent(s.standInShare.max)} | ${num(s.limitedFrames)} | ${s.residentsPerS === null ? "—" : num(s.residentsPerS, 1)} |`,
    ),
    "",
    "The pass's own counts at the samples, the largest in each segment:",
    "",
    "| Segment | Samples | Selected | Limited samples | Selected resident | Slots used | Pinned forced | Pinned drawn |",
    "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ...pass.logged.map(
      (s) =>
        `| ${s.segment ?? "after the script"} | ${num(s.samples)} | ${num(s.selectedMax)} | ${num(s.limitedSamples)} | ${countText(s.selectedResidentMax)} | ${countText(s.usedSlotsMax)} | ${countText(s.pinnedForcedMax)} | ${countText(s.pinnedDrawnMax)} |`,
    ),
    "",
  ];
  return lines;
}

/** A count, or `—` where there is none. */
function countText(count: number | null): string {
  return count === null ? "—" : num(count);
}

/** The ceiling the computed total stands against on `setting` (Design note 21's rows). */
function ceilingText(setting: SpikeSetting): string {
  let text: string;
  switch (setting) {
    case "low":
      text = "1 GB (low)";
      break;
    case "high":
      text = "2–3 GB (discrete)";
      break;
  }
  return text;
}

/** When an engine peak was first reached: its script time, or before the flight. */
function peakTimeText(atS: number | null): string {
  return atS === null ? "before the flight" : `${num(atS, 1)} s`;
}

/**
 * A cache's requested slots and its fallback past the binding, as its line states them
 * (decision-r10-t14b-constants §4); nothing for a block written before the clamp, without its
 * layout.
 */
function requestedText(layout: SpikeSizesCacheLayout | undefined): string {
  if (layout === undefined) {
    return "";
  }
  let path: string;
  switch (layout.fallback) {
    case "none":
      path = "on its own path, no fallback";
      break;
    case "binding-limit":
      path = "on the binding-limit fallback";
      break;
  }
  return ` of ${num(layout.requestedSlotCount)} requested, ${path}`;
}

/** The cascades' line: as the engine allocated them against T9.a's spec, or none and why. */
function cascadesText(cascades: SpikeSizesCascades): string {
  const spec = `T9.a's spec ${mib(cascades.specBytes)}`;
  let text: string;
  switch (cascades.source) {
    case "engine":
      text = `The cascades: ${mib(cascades.bytes)} as the engine allocated them (its ${CASCADE_CATEGORY} category at its peak), ${
        cascades.differsFromSpec
          ? `**not** ${spec}: ${num(cascades.bytes)} B against ${num(cascades.specBytes)} B, a finding for main`
          : `${spec} as built`
      }`;
      break;
    case "none":
      text = `The cascades: none allocated (${cascades.reason ?? "no reason given"}); ${spec}`;
      break;
  }
  return text;
}

/** The engine's tally's line: its whole peak, each category's, and the destructions left out. */
function engineTallyText(engine: SpikeSizesEngineTally): string {
  const categories =
    engine.categories.length === 0
      ? "no allocation seen"
      : engine.categories
          .map((c) => `${c.category} ${mib(c.bytes)} (${peakTimeText(c.atS)})`)
          .join(", ");
  const unseen =
    engine.unseenDestroyed === 0
      ? ""
      : `; ${num(engine.unseenDestroyed)} destructions of allocations made before the run's control listened, left out`;
  return `The engine's tally, the app's own GPU figure: ${mib(engine.peak.bytes)} at its whole peak (${peakTimeText(engine.peak.atS)}); each category at its own peak: ${categories}${unseen}`;
}

/** The summary's "Sizes" section (§6(g)): see the module's remarks and each line's words. */
export function sizesSummaryLines(sizes: SizesResults, setting: SpikeSetting): string[] {
  const { memory, binding, readouts, driver, secondView } = sizes;
  const viewports = secondView.viewports
    .map((v) => `${v.widthPx} × ${v.heightPx} px from ${num(v.fromS, 1)} s`)
    .join(", ");
  return [
    "## Sizes (R10.T14.b)",
    "",
    "Counted once a second of script time, by unbudgeted re-selections of each pass's own inputs: F at the pass's margins, F₀ at margin 0, AR all round, and F′ and AR′ with the diagnostic occluder; each view's counts leave the forced region out, which is counted once. The views are found in each selection's input by their streaming weight, then their viewport.",
    "",
    `- **Second view:** ${secondView.id} on ${secondView.engineName}, ${secondView.style}, weight ${secondView.weight}; viewport ${viewports.length === 0 ? "never laid out" : viewports}.`,
    `- **The header's lowest sampled height:** ${figureText(sizes.sampledLowestM, (m) => `${num(m, 1)} m`)}.`,
    `- **The lit and wireframe passes' forced sets** (T14.c's finding 4): ${figureText(sizes.forcedSets, (f) => `compared at ${num(f.comparedFrames)} frames, different at ${num(f.differingFrames)}${f.firstDifferingS === null ? "" : `, first at ${num(f.firstDifferingS, 1)} s, by at most ${num(f.largestDifference)} patches`}`)}.`,
    "",
    ...sizes.passes.flatMap(passLines),
    "### The height workers by segment",
    "",
    `${num(sizes.bakes.workersMade)} workers wrapped; the flight's ${num(sizes.bakes.total.baked)} bakes of ${num(sizes.bakes.total.posted)} posted, ${num(sizes.bakes.total.bakedNeverResident)} never made resident, ${sizes.bakes.total.bakesPerSecond === null ? "—" : num(sizes.bakes.total.bakesPerSecond, 1)} a second.`,
    "",
    "| Segment | Bakes / s | Baked | Never resident | Post to reply, p50 / p95 (ms) | Ground speed ÷ height, mean / max (s⁻¹) |",
    "| --- | --- | --- | --- | --- | --- |",
    ...sizes.segmentFigures.map(
      (s) =>
        `| ${s.segment ?? "after the script"} | ${s.bakes.bakesPerSecond === null ? "—" : num(s.bakes.bakesPerSecond, 1)} | ${num(s.bakes.baked)} | ${num(s.bakes.bakedNeverResident)} | ${s.bakes.postToReplyMs.p50 === null ? "—" : num(s.bakes.postToReplyMs.p50, 1)} / ${s.bakes.postToReplyMs.p95 === null ? "—" : num(s.bakes.postToReplyMs.p95, 1)} | ${s.groundSpeedPerHeight === null ? "—" : `${num(s.groundSpeedPerHeight.mean, 3)} / ${num(s.groundSpeedPerHeight.max, 3)}`} |`,
    ),
    "",
    "### Memory: itemized, the engine's tally and the driver",
    "",
    ...memory.caches.map(
      (cache) =>
        `- The ${cache.cache} cache: ${num(cache.slotCount)} slots × ${num(cache.bytesPerSlot)} B = ${mib(cache.allocationBytes)}`,
    ),
    ...memory.sessionGpu.map(
      (texture) =>
        `- The session's ${texture.category}: ${mib(texture.bytes)} at its peak (${num(texture.atS, 1)} s)${texture.category === CASCADE_CATEGORY ? ", its own figure for the cascades, which the itemized total counts as the engine allocated them" : ""}`,
    ),
    `- ${cascadesText(memory.cascades)}`,
    `- Itemized GPU (the caches, the session's masks and maps, the cascades): ${mib(memory.itemizedGpuBytes)}; with the workers' field copies (${mib(memory.workerFieldBytes.bytes)} at their peak): ${mib(memory.itemizedTotalBytes)}, against ${ceilingText(setting)}; the pool's own copy beside them ${mib(memory.keptFieldBytes.bytes)}; ${num(memory.workersStarted)} workers started; ${num(memory.samples)} samples at 1 Hz`,
    `- ${engineTallyText(memory.engine)}`,
    `- The driver: nvidia-smi's peak less its pre-launch baseline ${figureText(driver.peakLessBaselineBytes, mib)}; the GPU process's peak ${figureText(driver.gpuProcessPeakBytes, mib)}`,
    `- At the run's end: the device ${figureText(driver.endDeviceBytes, mib)}, the GPU process ${figureText(driver.endGpuProcessBytes, mib)}, the rest of the card ${figureText(driver.endOthersBytes, mib)} (the pre-launch baseline ${figureText(driver.preLaunchBaselineBytes, mib)}, a drift of ${figureText(driver.othersDriftBytes, mib)})`,
    `- Less the driver: the itemized GPU ${figureText(driver.itemizedLessDriverBytes, mib)}, the engine's tally ${figureText(driver.engineLessDriverBytes, mib)}`,
    "",
    "### The binding",
    "",
    `- maxStorageBufferBindingSize: the adapter's ${figureText(binding.adapterBytes, (b) => `${num(b)} B`)}, requested ${figureText(binding.requestedBytes, (b) => `${num(b)} B`)}, the device's ${figureText(binding.deviceBytes, (b) => `${num(b)} B`)}`,
    ...binding.caches.map(
      (cache) =>
        `- The ${cache.cache} cache's ${num(cache.slotCount)} slots${requestedText(cache.layout)}: its ceiling ${cache.ceilingAtRequestedSlots === null ? "unbounded" : `${num(cache.ceilingAtRequestedSlots)} slots`} at the requested binding, ${cache.ceilingAtDefaultSlots === null ? "unbounded" : `${num(cache.ceilingAtDefaultSlots)} slots`} at WebGPU's default 128 MiB`,
    ),
    "",
    "### The readout and the load",
    "",
    `- VIEW's terrain readout every ${num(readouts.intervalMs)} ms, not shown: ${num(readouts.publishes)} publishes, ${num(readouts.answeredPublishes)} holding an answer; ${num(readouts.asked)} asked of the workers, ${num(readouts.answered)} answered, ${num(readouts.failed)} failed; ${num(readouts.failComputationCalls)} of its guards' calls to fail the session counted and withheld`,
    `- The 1-minute load: ${figureText(sizes.load, (readings) => readings.map((r) => `${num(r.oneMinute, 2)} at ${num(r.tS)} s`).join(", "))}`,
    "",
  ];
}
