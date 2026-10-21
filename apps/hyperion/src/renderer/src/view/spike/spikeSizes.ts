/**
 * The descent spike's measured sizes (plan R10, R10.T14.b; decision-r10-t14b-measure §2–§7), under
 * `--sizes on` over the terrain world: the second view's style, the counting of each of the
 * session's passes for the world's body, the bake clock at the height workers, the session's memory
 * at 1 Hz, each pass's streaming by segment, the storage binding, and `VIEW`'s terrain readout at
 * 4 Hz, gathered into the report's `sizes` block.
 *
 * @remarks
 * The run tells it each frame: {@link SpikeSizes.beforeStep} before the session's step, with the
 * script's moment, which the counters read at each selection the step makes, and
 * {@link SpikeSizes.afterStep} after the frame's draws, when it follows the passes and reads them.
 * It only listens: it observes each pass (`TerrainPass.observe`) and reads its last step, its
 * planet and its layout, and the session's memory and survey, and asks nothing of the session but
 * the readout `VIEW` asks, through a stand-in of the session whose `failComputation` is counted
 * instead of passed on ({@link SpikeSizes.publishReadout}), so that the measurement cannot end the
 * run.
 *
 * A cache's pass with no described view, one that only stood in, is counted but not sized
 * ({@link STAND_IN_ONLY}): its rule's figures are `null` and its samples are left out of the block.
 *
 * Each cache has one counter for the run (`PassSizeCounter`), made at its first pass, attached to
 * it, and attached again whenever the session's pass of the cache is another object, as the run's
 * follower of the pass in the style drawn observes it again. Its views are the cache's style's
 * (§4's table): the lit cache's the photorealistic views, the wireframe's the wireframe views; a
 * view in another style that stands in, the main view's wireframe while its photorealistic
 * pipelines compile, counts in the cache's unions alone. A selection's input carries no view IDs,
 * so each described view is found among the input's views by its streaming weight, 1 for the main
 * view and 0.25 for the second ({@link matchViewsByWeight}), and by its presented viewport where
 * weights tie.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import type {
  SpikeFigure,
  SpikeSizesBakeWindow,
  SpikeSizesBytePeak,
  SpikeSizesCache,
  SpikeSizesColumn,
  SpikeSizesCountColumns,
  SpikeSizesMeanMax,
  SpikeSizesPass,
  SpikeSizesReport,
  SpikeSizesSeries,
  SpikeSizesStreaming,
  SpikeSizesView,
} from "../../../../preload/api";
import { GroundReadouts } from "../../displays/view/groundReadout";
import { instrumentViewId } from "../../displays/view/viewNames";
import type { ViewRun } from "../../displays/view/viewRun";
import type { RenderStyle, ViewId } from "../camera/state";
import { type AdapterOutcome, type GpuCapabilities, requiredLimits } from "../engine/platform";
import type { ViewSize } from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import { bindingCeilingSlots } from "../terrain/cacheBudget";
import { lowestHeightM } from "../terrain/planet";
import { PRIMARY_VIEW_WEIGHT, SECONDARY_VIEW_WEIGHT } from "../terrain/priority";
import type { Selection, ViewSelectionInput } from "../terrain/select";
import type { TerrainSession, TerrainSessionMemory, TerrainSurvey } from "../terrain/session";
import { CASCADE_BYTES, CASCADE_MEMORY_CATEGORY } from "../terrain/shadows/cascades";
import type { SlotField } from "../terrain/slotLayout";
import type { TerrainPass } from "../terrain/terrainPass";
import type { WorkerLike } from "../terrain/workers/pool";
import { BakeClock, type BakeWindow } from "./bakeClock";
import type { DescentPose, DescentProfile } from "./descentProfile";
import { PassSizeCounter, type PassSizeSeries, type SizeCountContext } from "./sizeCount";

/** The second view's ID in the terrain session (§4): the first instrument slot's. */
export const SECOND_VIEW_ID: ViewId = instrumentViewId(1);

/** How often the session's memory is read, ms of the frames' clock: 1 Hz. */
export const SIZES_MEMORY_INTERVAL_MS = 1000;

/** How often the main view's terrain readout is published, ms: `VIEW`'s 4 Hz (§6(f)). */
export const SIZES_READOUT_INTERVAL_MS = 250;

/** Why a pass with no described view is not sized ({@link SpikeSizesPass}'s `notSized`). */
export const STAND_IN_ONLY = "stand-in only, not sized";

/**
 * How near its rest the camera's clearance must come for its pose to be at touchdown, m: 1 cm,
 * which the hover's last second of braking (0.05 m/s to rest) crosses 0.63 s before the stop.
 */
export const TOUCHDOWN_TOLERANCE_M = 0.01;

/**
 * Whether `pose` has reached the ground, the touchdown the forced region's lead runs to (T14.c's
 * finding 3): its clearance down to the script's last segment's end, the camera's height over the
 * site with the craft set down (the hover and touchdown's 1 m), within
 * {@link TOUCHDOWN_TOLERANCE_M}; or the script's end, where a profile never comes so close.
 *
 * @remarks
 * The script's own altitude above the site (`DescentPose.clearanceM`) is read, not the height over
 * a stretch's floor: over the site, through the vertical descent and the hover, the two agree.
 */
export function touchedDown(
  profile: Pick<DescentProfile, "segments" | "durationS">,
  pose: Pick<DescentPose, "tS" | "clearanceM">,
): boolean {
  const restM = profile.segments.at(-1)?.endAltitudeM ?? 0;
  return pose.tS >= profile.durationS || pose.clearanceM <= restM + TOUCHDOWN_TOLERANCE_M;
}

/**
 * The style the second view is asked to draw in (§4): the main view's on the high setting; the
 * wireframe on the low setting, which allows one photorealistic view, so that on low's
 * photorealistic run its own wireframe cache sits beside the main view's lit one, and on low's
 * wireframe run both are wireframe.
 */
export function secondViewStyle(setting: QualitySetting, mainStyle: RenderStyle): RenderStyle {
  let style: RenderStyle;
  switch (setting) {
    case "high":
      style = mainStyle;
      break;
    case "low":
      style = "wireframe";
      break;
  }
  return style;
}

/**
 * `session` in everything but `failComputation`, which calls `withheld` instead and does nothing
 * more: the stand-in of the terrain session that the measured sizes' readout is asked of
 * ({@link SpikeSizes.publishReadout}). Its other members are the session's own, its methods bound
 * to it, so that they read the session's private fields on the session itself.
 */
export function withheldFailures<T extends { failComputation(): void }>(
  session: T,
  withheld: () => void,
): T {
  return new Proxy(session, {
    get: (target, property): unknown => {
      if (property === "failComputation") {
        return withheld;
      }
      const value: unknown = Reflect.get(target, property, target);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
}

/** A view as {@link matchViewsByWeight} finds it: its streaming weight and its presented viewport. */
export interface WeightedView {
  readonly weight: number;
  /** Device px, or `null` where it is not known. */
  readonly viewport: ViewSize | null;
}

/**
 * Where each of `described` is among a selection's input views, `inputs`: the one input view of its
 * weight not already taken, or among several of its weight the one whose viewport is its own;
 * `null` where none is, or the tie stands.
 */
export function matchViewsByWeight(
  described: ReadonlyArray<WeightedView>,
  inputs: ReadonlyArray<Pick<ViewSelectionInput, "weight" | "viewport">>,
): Array<number | null> {
  const taken = new Set<number>();
  return described.map((view) => {
    const ofWeight = inputs.flatMap((input, i) =>
      input.weight === view.weight && !taken.has(i) ? [i] : [],
    );
    const viewport = view.viewport;
    const found =
      ofWeight.length <= 1 || viewport === null
        ? ofWeight
        : ofWeight.filter((i) => {
            const size = inputs[i]?.viewport;
            return size?.widthPx === viewport.widthPx && size.heightPx === viewport.heightPx;
          });
    const [index] = found;
    if (found.length !== 1 || index === undefined) {
      return null;
    }
    taken.add(index);
    return index;
  });
}

/** What the measured sizes read of one of the session's passes. */
export type SpikeSizesTerrainPass = Pick<TerrainPass, "observe" | "lastStep" | "planet" | "layout">;

/** What the measured sizes read of the terrain session (`TerrainSession`, or a test's stand-in). */
export interface SpikeSizesSession {
  pass(body: BodyIdHex): SpikeSizesTerrainPass | null;
  wirePass(body: BodyIdHex): SpikeSizesTerrainPass | null;
  memory(): TerrainSessionMemory;
  survey(body: BodyIdHex): Pick<TerrainSurvey, "header"> | null;
  /** The height workers it runs (`TerrainSession.workerCount`). */
  readonly workerCount: number;
}

/** One frame as the measured sizes read it, before and after the session's step. */
export interface SpikeSizesFrame {
  readonly session: SpikeSizesSession;
  /** The world's body, whose passes are counted. */
  readonly body: BodyIdHex;
  /** The frame's monotonic time, ms. */
  readonly nowMs: number;
  /** Script time, s. */
  readonly tS: number;
  /** The script's pose at `tS`: its segment, the camera's height and the ground speed. */
  readonly pose: DescentPose;
  /** Whether the script is under way; past its end the segment is `null`. */
  readonly underWay: boolean;
  /** Whether the pose has reached the ground ({@link touchedDown}). */
  readonly touchedDown: boolean;
  /** The main view's and the second view's presented viewports, device px, `null` before layout. */
  readonly mainViewport: ViewSize | null;
  readonly secondViewport: ViewSize | null;
}

/** What makes {@link SpikeSizes}. */
export interface SpikeSizesOptions {
  readonly setting: QualitySetting;
  /** The main view's style (`--style`). */
  readonly style: RenderStyle;
  /** The main view's ID in the session, whose terrain readout is asked. */
  readonly mainViewId: ViewId;
  /** The second view's engine view's name: the craft instrument's canvas's. */
  readonly secondViewName: string;
  /** The clock the bake clock and the counting read, ms; `performance.now()` by default. */
  readonly nowMs?: () => number;
}

/** A described view, with how to find its viewport now. */
interface DescribedView extends SpikeSizesView {
  readonly role: "main" | "second";
}

/** A segment's tally of one figure: its sum over the frames counted, and its largest. */
class MeanMax {
  sum = 0;
  max = 0;
  count = 0;

  add(value: number): void {
    this.sum += value;
    this.max = this.count === 0 ? value : Math.max(this.max, value);
    this.count += 1;
  }

  /** The mean and the largest, or zeros over no frame. */
  get value(): SpikeSizesMeanMax {
    return this.count === 0 ? { mean: 0, max: 0 } : { mean: this.sum / this.count, max: this.max };
  }
}

/** A pass's streaming in one segment. */
class StreamingTally {
  frames = 0;
  readonly drawn = new MeanMax();
  readonly standingIn = new MeanMax();
  readonly missing = new MeanMax();
  readonly standInShare = new MeanMax();
  limitedFrames = 0;
  residents = 0;
  firstS: number | null = null;
  lastS: number | null = null;

  /** The span of script time its frames covered, s. */
  get spanS(): number {
    return this.firstS === null || this.lastS === null ? 0 : this.lastS - this.firstS;
  }
}

/** The forced keys of `selection`. */
function forcedKeys(selection: Selection): Set<string> {
  const keys = new Set<string>();
  for (const [keyString, patch] of selection.patches) {
    if (patch.forced) {
      keys.add(keyString);
    }
  }
  return keys;
}

/** The size of the symmetric difference of `a` and `b`. */
function symmetricDifference(a: ReadonlySet<string>, b: ReadonlySet<string>): number {
  let n = 0;
  for (const key of a) {
    n += b.has(key) ? 0 : 1;
  }
  for (const key of b) {
    n += a.has(key) ? 0 : 1;
  }
  return n;
}

/** A present figure. */
function present<T>(value: T): SpikeFigure<T> {
  return { value, reason: null };
}

/** A missing figure and why. */
function absent(reason: string): SpikeFigure<never> {
  return { value: null, reason };
}

/** `value` as a figure, missing for `reason` where it is `null`. */
function figureOf(value: number | null, reason: string): SpikeFigure<number> {
  return value === null ? absent(reason) : present(value);
}

/**
 * A column of whole numbers from `values`, each `null` in a gap for the reason `reasonAt` gives
 * it, consecutive samples of one reason one gap.
 */
export function sizesColumn(
  values: ReadonlyArray<number | null>,
  reasonAt: (i: number) => string,
): SpikeSizesColumn {
  const samples: number[] = [];
  const gaps: Array<{ from: number; to: number; reason: string }> = [];
  values.forEach((value, i) => {
    if (value !== null) {
      samples.push(Math.round(value));
      return;
    }
    samples.push(-1);
    const reason = reasonAt(i);
    const last = gaps.at(-1);
    if (last !== undefined && last.to === i - 1 && last.reason === reason) {
      last.to = i;
    } else {
      gaps.push({ from: i, to: i, reason });
    }
  });
  return { samples, gaps };
}

/** Why a view's count is missing at a sample. */
const VIEW_NOT_IN_INPUT = "the view was not in the pass's selection input";

/** Why a diagnostic count is missing at a sample. */
const NO_DEPTH_YET = "no diagnostic depth yet: no header's lowest cell and no resident bake";

/** Why a count of the resident patches is missing. */
const NO_RANGES = "the pass's input held no baked ranges";

/** Why a count of the cache's slots is missing. */
const NOT_A_CACHE = "the pass's ranges were not a patch cache";

/** A cache's slots as last read from its pass's layout. */
interface CacheLayout {
  readonly slotCount: number;
  readonly bytesPerSlot: number;
  readonly fields: ReadonlyArray<SlotField>;
}

/** A cache's allocation: slot count × bytes a slot. */
interface CacheAllocation {
  readonly slotCount: number;
  readonly bytesPerSlot: number;
}

/** One cache's counting and reading over the run, following the session's pass of it. */
class CacheFollower {
  readonly cache: SpikeSizesCache;
  readonly views: ReadonlyArray<DescribedView>;
  #counter: PassSizeCounter | null = null;
  #pass: SpikeSizesTerrainPass | null = null;
  #off: (() => void) | null = null;
  #attachments = 0;
  /** The last layout read. */
  #layout: CacheLayout | null = null;
  /** The largest allocation read. */
  #allocation: CacheAllocation | null = null;
  #asBuiltOccluderM: number | null = null;
  readonly #streaming = new Map<string | null, StreamingTally>();

  constructor(cache: SpikeSizesCache, views: ReadonlyArray<DescribedView>) {
    this.cache = cache;
    this.views = views;
  }

  /** The last layout read, or `null` before any. */
  get layout(): CacheLayout | null {
    return this.#layout;
  }

  /** The largest allocation read, or `null` before any. */
  get allocation(): CacheAllocation | null {
    return this.#allocation;
  }

  /**
   * Follows `pass`, the session's pass of the cache now: observed by the counter, made with its
   * bytes a slot at the first, and told of each patch it makes resident, once it appears and again
   * whenever it is another object.
   */
  follow(
    pass: SpikeSizesTerrainPass | null,
    makeCounter: (bytesPerSlot: number) => PassSizeCounter,
    onResident: () => void,
  ): void {
    if (pass === this.#pass) {
      return;
    }
    this.#off?.();
    this.#off = null;
    this.#pass = pass;
    if (pass === null) {
      return;
    }
    const counter = this.#counter ?? makeCounter(pass.layout.slots.bytesPerSlot);
    this.#counter = counter;
    const offCounting = counter.attach(pass);
    const offResident = pass.observe({ onResident });
    this.#off = () => {
      offCounting();
      offResident();
    };
    this.#attachments += 1;
  }

  /** A patch the pass made resident, in `segment`. */
  resident(segment: string | null): void {
    this.#tally(segment).residents += 1;
  }

  /** Reads the pass after the frame's step: its last step's draw set, its layout and its occluder. */
  read(segment: string | null, tS: number): void {
    const pass = this.#pass;
    if (pass === null) {
      return;
    }
    const slots = pass.layout.slots;
    this.#layout = {
      slotCount: slots.slotCount,
      bytesPerSlot: slots.bytesPerSlot,
      fields: slots.fields,
    };
    const allocation = this.#allocation;
    if (
      allocation === null ||
      slots.slotCount * slots.bytesPerSlot > allocation.slotCount * allocation.bytesPerSlot
    ) {
      this.#allocation = { slotCount: slots.slotCount, bytesPerSlot: slots.bytesPerSlot };
    }
    this.#asBuiltOccluderM = lowestHeightM(pass.planet);
    const step = pass.lastStep;
    if (step === null) {
      return;
    }
    const tally = this.#tally(segment);
    tally.frames += 1;
    tally.firstS ??= tS;
    tally.lastS = tS;
    const { drawSet, selection } = step;
    tally.drawn.add(drawSet.count);
    tally.standingIn.add(drawSet.standingIn);
    tally.missing.add(drawSet.missing);
    const selected = selection.patches.size;
    if (selected > 0) {
      tally.standInShare.add(drawSet.standingIn / selected);
    }
    tally.limitedFrames += selection.limited ? 1 : 0;
  }

  /** The counter's touchdown (`PassSizeCounter.markTouchdown`). */
  markTouchdown(): void {
    this.#counter?.markTouchdown();
  }

  /** The counter's contact by the session's rule (`PassSizeCounter.markContact`). */
  markContact(): void {
    this.#counter?.markContact();
  }

  /** Stops observing the pass it follows; what it counted stays. */
  detach(): void {
    this.#off?.();
    this.#off = null;
    this.#pass = null;
  }

  /**
   * The cache's figures, or `null` where no pass of it was followed; a pass with no described view
   * is not sized ({@link STAND_IN_ONLY}): its rule's figures are `null` and its samples left out.
   */
  report(segments: ReadonlyArray<string>): SpikeSizesPass | null {
    const counter = this.#counter;
    if (counter === null) {
      return null;
    }
    const summary = counter.summary();
    const sized = this.views.length > 0;
    return {
      cache: this.cache,
      views: this.views.map(({ id, style, weight }) => ({ id, style, weight })),
      notSized: sized
        ? null
        : `${STAND_IN_ONLY}: no view of the ${this.cache} cache's style was asked, so its pass only stood in for another style's views`,
      bytesPerSlot: summary.bytesPerSlot,
      slots: summary.slots,
      attachments: this.#attachments,
      samples: summary.samples,
      failedSamples: summary.failedSamples,
      firstFailure: summary.firstFailure,
      counts: summary.counts,
      forced: summary.forced,
      residentShare: summary.residentShare,
      sweptShare: summary.sweptShare,
      allRoundEverResident: summary.allRoundEverResident,
      capAtAllRoundPeak: summary.capAtAllRoundPeak,
      deepestOccluderM: figureOf(
        summary.deepestOccluderM,
        "no diagnostic count was made: no header's lowest cell and no resident bake under the counts",
      ),
      asBuiltOccluderM: this.#asBuiltOccluderM ?? 0,
      boundRatios: summary.boundRatios,
      boxHeights: summary.boxHeights,
      countingMs: summary.countingMs,
      rule: sized ? summary.rule : null,
      ruleDiagnostic: sized ? summary.ruleDiagnostic : null,
      occluderPrice: sized ? summary.occluderPrice : null,
      contact: summary.contact,
      streaming: [...this.#streaming].map(([segment, tally]): SpikeSizesStreaming => ({
        segment,
        frames: tally.frames,
        drawn: tally.drawn.value,
        standingIn: tally.standingIn.value,
        missing: tally.missing.value,
        standInShare: tally.standInShare.value,
        limitedFrames: tally.limitedFrames,
        residents: tally.residents,
        residentsPerS: tally.spanS > 0 ? tally.residents / tally.spanS : null,
      })),
      logged: summary.logged,
      series: sized ? seriesOf(counter.series(), this.views.length, segments) : null,
    };
  }

  #tally(segment: string | null): StreamingTally {
    const known = this.#streaming.get(segment);
    if (known !== undefined) {
      return known;
    }
    const made = new StreamingTally();
    this.#streaming.set(segment, made);
    return made;
  }
}

/** The five count columns of `of`, a count's values at each sample, from the counter's series. */
function countColumns(
  of: (name: keyof SpikeSizesCountColumns) => ReadonlyArray<number | null>,
  occluderM: ReadonlyArray<number | null>,
  plainReason: string,
): SpikeSizesCountColumns {
  const diagnosticReason = (i: number): string =>
    (occluderM[i] ?? null) === null ? NO_DEPTH_YET : plainReason;
  return {
    frustum: sizesColumn(of("frustum"), () => plainReason),
    frustumNoMargin: sizesColumn(of("frustumNoMargin"), () => plainReason),
    allRound: sizesColumn(of("allRound"), () => plainReason),
    frustumDiagnostic: sizesColumn(of("frustumDiagnostic"), diagnosticReason),
    allRoundDiagnostic: sizesColumn(of("allRoundDiagnostic"), diagnosticReason),
  };
}

/**
 * The samples' whole seconds, ascending, compactly (`SpikeSizesSeries.seconds`): the first, and the
 * seconds after it that no sample fell in.
 */
export function secondsOf(seconds: ReadonlyArray<number>): SpikeSizesSeries["seconds"] {
  const first = seconds[0] ?? 0;
  const skipped: number[] = [];
  let expected = first;
  for (const second of seconds) {
    for (; expected < second; expected += 1) {
      skipped.push(expected);
    }
    expected = second + 1;
  }
  return { first, skipped };
}

/** The samples' segment indices as runs (`SpikeSizesSeries.segmentRuns`). */
export function segmentRunsOf(segments: ReadonlyArray<number>): SpikeSizesSeries["segmentRuns"] {
  const runs: Array<{ segment: number; from: number }> = [];
  segments.forEach((segment, i) => {
    if (runs.at(-1)?.segment !== segment) {
      runs.push({ segment, from: i });
    }
  });
  return runs;
}

/** The names of the five counts, in the counter's order. */
const COUNT_NAMES = [
  "frustum",
  "frustumNoMargin",
  "allRound",
  "frustumDiagnostic",
  "allRoundDiagnostic",
] as const;

/**
 * A counter's samples in the report's columns: whole seconds, segment indices into `segments`,
 * whole metres and whole counts, the unions left out for a pass of one described view, and only
 * the columns the record reads at 1 Hz (`SpikeSizesSeries`), the pass's own counts going by
 * segment instead (`PassSizeSummary.logged`).
 */
function seriesOf(
  series: PassSizeSeries,
  described: number,
  segments: ReadonlyArray<string>,
): SpikeSizesSeries {
  const union = series.union;
  const one = series.views[0];
  let unionDiffers = 0;
  if (described === 1 && one !== undefined) {
    for (let i = 0; i < series.scriptTimeS.length; i += 1) {
      unionDiffers += COUNT_NAMES.some(
        (name) => (union[name][i] ?? null) !== (one[name][i] ?? null),
      )
        ? 1
        : 0;
    }
  }
  const shareCount = (shares: ReadonlyArray<number | null>, i: number): number | null => {
    const whole = union.allRound[i] ?? null;
    if (whole === null) {
      return null;
    }
    if (whole === 0) {
      return 0;
    }
    const share = shares[i] ?? null;
    return share === null ? null : Math.round(share * whole);
  };
  return {
    seconds: secondsOf(series.scriptTimeS.map((t) => Math.floor(t))),
    segmentRuns: segmentRunsOf(
      series.segment.map((name) => (name === null ? -1 : segments.indexOf(name))),
    ),
    heightAboveGroundM: series.heightAboveGroundM.map((h) => (h === null ? null : Math.round(h))),
    views: series.views.map((view) =>
      countColumns((name) => view[name], series.occluderM, VIEW_NOT_IN_INPUT),
    ),
    union:
      described === 1
        ? null
        : countColumns((name) => union[name], series.occluderM, "not counted at the sample"),
    unionDiffers,
    forced: series.forced,
    forcedResident: sizesColumn(series.forcedResident, () => NO_RANGES),
    allRoundResident: sizesColumn(
      series.scriptTimeS.map((_, i) => shareCount(series.residentShare, i)),
      () => NO_RANGES,
    ),
    allRoundSwept: series.scriptTimeS.map((_, i) => shareCount(series.sweptShare, i) ?? 0),
    usedSlots: sizesColumn(series.usedSlots, () => NOT_A_CACHE),
  };
}

/** A byte peak and when it was read. */
interface Peak {
  bytes: number;
  atS: number;
}

/** Takes `peak` up to `bytes` at `atS` where it is larger, or makes it. */
function peakOf(peak: Peak | undefined, bytes: number, atS: number): Peak {
  if (peak === undefined) {
    return { bytes, atS };
  }
  if (bytes > peak.bytes) {
    peak.bytes = bytes;
    peak.atS = atS;
  }
  return peak;
}

/** A peak as the report gives it, zeros where none was read. */
function bytePeakOf(peak: Peak | undefined): SpikeSizesBytePeak {
  return { bytes: peak?.bytes ?? 0, atS: peak?.atS ?? 0 };
}

/** `slots`, or `null` where it is not finite: a ceiling with no storage field to bound it. */
function finiteOrNull(slots: number): number | null {
  return Number.isFinite(slots) ? slots : null;
}

/** The forced sets' comparison so far (T14.c's finding 4). */
interface ForcedSetTally {
  comparedFrames: number;
  differingFrames: number;
  firstDifferingS: number | null;
  largestDifference: number;
  /** The selections last compared, and whether their forced sets differed. */
  lit: Selection | null;
  wire: Selection | null;
  lastDiffered: boolean;
}

/** A bake window as the report gives it. */
function bakeWindowOf(window: BakeWindow): SpikeSizesBakeWindow {
  return {
    posted: window.posted,
    baked: window.baked,
    failed: window.failed,
    unsurveyed: window.unsurveyed,
    postToReplyMs: window.postToReplyMs,
    computeMs: window.computeMs,
    residents: window.residents,
    bakedNeverResident: window.bakedNeverResident,
    spanMs: window.spanMs,
    bakesPerSecond: window.bakesPerSecond,
  };
}

/**
 * The measured sizes of one `--sizes on` run (plan R10, R10.T14.b; decision-r10-t14b-measure): see
 * the module's remarks.
 *
 * @example
 * ```ts
 * const sizes = new SpikeSizes({
 *   setting: launch.setting,
 *   style: launch.style ?? "photorealistic",
 *   mainViewId: SPIKE_MAIN_VIEW_ID,
 *   secondViewName: SPIKE_VIEW_NAMES.craft,
 * });
 * sizes.noteAdapter(outcome);
 * // the session's worker factory, the same function for the same `make`:
 * const createWorker = sizes.heightWorkers(spikeWorkers.height);
 * // each frame, around the session's step, `touchedDown` read from the profile:
 * sizes.beforeStep(frame);
 * stepTerrainSession(session, guard, nowMs);
 * sizes.afterStep(frame);
 * // on the first frame whose pass's selection holds a contact, and at 4 Hz:
 * sizes.markContact();
 * sizes.publishReadout(session, run, nowMs);
 * // at the run's end:
 * const block = sizes.report();
 * ```
 */
export class SpikeSizes {
  /** The bake-time wrapper at the height workers (§6(c)). */
  readonly clock: BakeClock;
  readonly #mainStyle: RenderStyle;
  readonly #secondStyle: RenderStyle;
  readonly #secondViewName: string;
  readonly #readouts: GroundReadouts;
  readonly #followers: Readonly<Record<SpikeSizesCache, CacheFollower>>;
  readonly #nowMs: (() => number) | null;
  #moment: SizeCountContext = { scriptTimeS: 0, segment: null, heightAboveGroundM: null };
  #frame: SpikeSizesFrame | null = null;
  readonly #segments: string[] = [];
  #flightStarted = false;
  #contacted = false;
  #touchedDown = false;
  /** Each worker factory given to {@link SpikeSizes.heightWorkers}, wrapped once. */
  readonly #factories = new WeakMap<() => WorkerLike, () => WorkerLike>();
  /** Each session the readout was asked of, behind its stand-in ({@link SpikeSizes.publishReadout}). */
  readonly #readoutSessions = new WeakMap<TerrainSession, TerrainSession>();
  #failComputationCalls = 0;
  /** The header's lowest sampled height as last read, or why there is none. */
  #lowest: SpikeFigure<number> = absent("no field header was read");
  // The binding (§6(g)).
  #adapterBytes: SpikeFigure<number> = absent("no adapter was asked for");
  #requestedBytes: SpikeFigure<number> = absent("no adapter was asked for");
  #deviceBytes: SpikeFigure<number> = absent("no device was made");
  // The memory (§6(d)).
  #memoryAtMs = Number.NEGATIVE_INFINITY;
  #memorySamples = 0;
  #workerField: Peak | undefined;
  #keptField: Peak | undefined;
  readonly #sessionGpu = new Map<string, Peak>();
  #workersStarted = 0;
  readonly #forcedSets: ForcedSetTally = {
    comparedFrames: 0,
    differingFrames: 0,
    firstDifferingS: null,
    largestDifference: 0,
    lit: null,
    wire: null,
    lastDiffered: false,
  };
  readonly #groundSpeed = new Map<string | null, MeanMax>();
  readonly #viewports: Array<{ widthPx: number; heightPx: number; fromS: number }> = [];
  #publishes = 0;
  #answeredPublishes = 0;

  constructor(options: SpikeSizesOptions) {
    this.#nowMs = options.nowMs ?? null;
    this.clock = new BakeClock({
      ...(options.nowMs === undefined ? {} : { nowMs: options.nowMs }),
      segment: () => this.#moment.segment,
    });
    this.#mainStyle = options.style;
    this.#secondStyle = secondViewStyle(options.setting, options.style);
    this.#secondViewName = options.secondViewName;
    this.#readouts = new GroundReadouts(options.mainViewId);
    const main: DescribedView = {
      id: options.mainViewId,
      style: this.#mainStyle,
      weight: PRIMARY_VIEW_WEIGHT,
      role: "main",
    };
    const second: DescribedView = {
      id: SECOND_VIEW_ID,
      style: this.#secondStyle,
      weight: SECONDARY_VIEW_WEIGHT,
      role: "second",
    };
    const inStyle = (style: RenderStyle): DescribedView[] =>
      [main, second].filter((view) => view.style === style);
    this.#followers = {
      lit: new CacheFollower("lit", inStyle("photorealistic")),
      wireframe: new CacheFollower("wireframe", inStyle("wireframe")),
    };
  }

  /** The style the second view is asked to draw in ({@link secondViewStyle}). */
  get secondStyle(): RenderStyle {
    return this.#secondStyle;
  }

  /**
   * `make`, the session's height-worker factory, with each worker it starts wrapped by the bake
   * clock: one wrapper for each `make`, the same function at every call, so that it is stable in
   * identity for as long as `make` is (`useTerrainSession` makes its session anew on a new factory).
   */
  heightWorkers(make: () => WorkerLike): () => WorkerLike {
    const known = this.#factories.get(make);
    if (known !== undefined) {
      return known;
    }
    const wrapped = (): WorkerLike => this.clock.wrap(make());
    this.#factories.set(make, wrapped);
    return wrapped;
  }

  /**
   * The adapter the engine was made on: its reported `maxStorageBufferBindingSize`, and the limit
   * the engine requests of its device (`requiredLimits`, without the harness's overrides, as the
   * spike's engine is made).
   */
  noteAdapter(outcome: AdapterOutcome): void {
    if (outcome.kind !== "adapter") {
      this.#adapterBytes = absent(`no adapter (${outcome.kind})`);
      this.#requestedBytes = absent(`no adapter (${outcome.kind})`);
      return;
    }
    this.#adapterBytes = present(outcome.capabilities.maxStorageBufferBindingSize);
    this.#requestedBytes = figureOf(
      requiredLimits(outcome.adapter, undefined)["maxStorageBufferBindingSize"] ?? null,
      "the engine requests no storage-binding limit: WebGPU's default",
    );
  }

  /** The device the engine made: its own `maxStorageBufferBindingSize` (`RenderEngine.capabilities`). */
  noteDevice(capabilities: Pick<GpuCapabilities, "maxStorageBufferBindingSize">): void {
    this.#deviceBytes = present(capabilities.maxStorageBufferBindingSize);
  }

  /**
   * Before the frame's step: the script's moment, which the counters read at each selection the
   * step makes and the bake clock at each message; at the first, the flight's start
   * (`BakeClock.markFlightStart`), so that the measurement's bakes before it are left out.
   */
  beforeStep(frame: SpikeSizesFrame): void {
    this.#frame = frame;
    const segment = frame.underWay ? frame.pose.segment : null;
    this.#moment = {
      scriptTimeS: frame.tS,
      segment,
      heightAboveGroundM: frame.pose.heightAboveFloorM,
    };
    if (segment !== null && !this.#segments.includes(segment)) {
      this.#segments.push(segment);
    }
    if (!this.#flightStarted) {
      this.#flightStarted = true;
      this.clock.markFlightStart();
    }
    const viewport = frame.secondViewport;
    const last = this.#viewports.at(-1);
    if (
      viewport !== null &&
      (last === undefined ||
        last.widthPx !== viewport.widthPx ||
        last.heightPx !== viewport.heightPx)
    ) {
      this.#viewports.push({
        widthPx: viewport.widthPx,
        heightPx: viewport.heightPx,
        fromS: frame.tS,
      });
    }
  }

  /**
   * After the frame's draws: follows the session's lit and wireframe passes of the body, reads
   * each one's last step, compares their forced sets, marks touchdown at the first frame whose pose
   * has reached the ground (`SpikeSizesFrame.touchedDown`), and reads the session's memory once a
   * second.
   */
  afterStep(frame: SpikeSizesFrame): void {
    this.#frame = frame;
    const { session, body } = frame;
    const segment = this.#moment.segment;
    const lit = session.pass(body);
    const wire = session.wirePass(body);
    for (const [follower, pass] of [
      [this.#followers.lit, lit],
      [this.#followers.wireframe, wire],
    ] as const) {
      follower.follow(
        pass,
        (bytesPerSlot) => this.#counterOf(follower, bytesPerSlot),
        () => {
          this.clock.noteResident();
          follower.resident(this.#moment.segment);
        },
      );
      follower.read(segment, frame.tS);
    }
    this.#compareForced(lit, wire, frame.tS);
    if (frame.touchedDown && !this.#touchedDown) {
      this.#touchedDown = true;
      this.#followers.lit.markTouchdown();
      this.#followers.wireframe.markTouchdown();
    }
    const { pose } = frame;
    if (pose.heightAboveFloorM > 0) {
      const tally = this.#groundSpeed.get(segment) ?? new MeanMax();
      tally.add(pose.horizontalSpeedMps / pose.heightAboveFloorM);
      this.#groundSpeed.set(segment, tally);
    }
    if (frame.nowMs - this.#memoryAtMs >= SIZES_MEMORY_INTERVAL_MS) {
      this.#memoryAtMs = frame.nowMs;
      this.#readMemory(session, frame.tS);
    }
  }

  /**
   * The craft's first contact frame by the session's rule (`passContact`): each counter's
   * contact, once. The forced region's lead runs to touchdown instead (`afterStep`).
   */
  markContact(): void {
    if (this.#contacted) {
      return;
    }
    this.#contacted = true;
    this.#followers.lit.markContact();
    this.#followers.wireframe.markContact();
  }

  /**
   * One publish of the main view's terrain readout, `VIEW`'s (`GroundReadouts.publish`), at the
   * spike's 4 Hz ({@link SIZES_READOUT_INTERVAL_MS}): asked, its answers counted and not shown
   * (§6(f)).
   *
   * @remarks
   * The readout is given a stand-in of `session`, the same for each session, that is the session in
   * everything but `failComputation`: its guards' calls to fail the session's computation, after
   * 250 ms of refusals, are counted, the first logged, and never passed on, so that the
   * measurement's readout cannot end the run (`SpikeSizesReadouts.failComputationCalls`).
   */
  publishReadout(session: TerrainSession | null, run: ViewRun, nowMs: number): void {
    const asked = session === null ? null : this.#readoutSession(session);
    const snapshot = this.#readouts.publish(asked, run, nowMs);
    this.#publishes += 1;
    this.#answeredPublishes += snapshot !== null && snapshot.readout !== null ? 1 : 0;
  }

  /** Stops observing the passes: the run is disposed; what was counted stays for the report. */
  detach(): void {
    this.#followers.lit.detach();
    this.#followers.wireframe.detach();
  }

  /** The report's `sizes` block, from what has been counted and read so far. */
  report(): SpikeSizesReport {
    const segments = [...this.#segments];
    const clock = this.clock.summary();
    const passes = [this.#followers.lit, this.#followers.wireframe].flatMap((follower) => {
      const pass = follower.report(segments);
      return pass === null ? [] : [pass];
    });
    const forced = this.#forcedSets;
    return {
      segments,
      secondView: {
        id: SECOND_VIEW_ID,
        engineName: this.#secondViewName,
        style: this.#secondStyle,
        weight: SECONDARY_VIEW_WEIGHT,
        viewports: this.#viewports.map((viewport) => ({ ...viewport })),
      },
      sampledLowestM: this.#lowest,
      passes,
      forcedSets:
        forced.comparedFrames === 0
          ? absent("the lit and wireframe passes never stood in the same frame")
          : present({
              comparedFrames: forced.comparedFrames,
              differingFrames: forced.differingFrames,
              firstDifferingS: forced.firstDifferingS,
              largestDifference: forced.largestDifference,
            }),
      segmentFigures: clock.bySegment.map((window) => ({
        segment: window.segment,
        bakes: bakeWindowOf(window),
        groundSpeedPerHeight: this.#groundSpeed.get(window.segment)?.value ?? null,
      })),
      bakes: {
        workersMade: clock.workersMade,
        total: bakeWindowOf(clock.total),
        byWorker: clock.byWorker.map((worker) => ({
          worker: worker.worker,
          posted: worker.posted,
          baked: worker.baked,
          failed: worker.failed,
          unsurveyed: worker.unsurveyed,
          postToReplyMs: worker.postToReplyMs,
          computeMs: worker.computeMs,
        })),
        requests: Object.entries(clock.requests).map(([kind, count]) => ({ kind, count })),
        replies: Object.entries(clock.replies).map(([kind, count]) => ({ kind, count })),
        inFlight: clock.inFlight,
        abandoned: clock.abandoned,
      },
      memory: this.#memoryReport(),
      binding: this.#bindingReport(),
      readouts: {
        intervalMs: SIZES_READOUT_INTERVAL_MS,
        publishes: this.#publishes,
        answeredPublishes: this.#answeredPublishes,
        asked: clock.requests["ground-readout"],
        answered: clock.replies["ground-readout"],
        failed: clock.replies["ground-readout-failed"],
        failComputationCalls: this.#failComputationCalls,
      },
    };
  }

  /**
   * `session` behind the readout's stand-in: its own members, bound to it, but for a
   * `failComputation` that counts the call, logs the first and does nothing more.
   */
  #readoutSession(session: TerrainSession): TerrainSession {
    const known = this.#readoutSessions.get(session);
    if (known !== undefined) {
      return known;
    }
    const standIn = withheldFailures(session, () => {
      this.#failComputationCalls += 1;
      if (this.#failComputationCalls === 1) {
        console.warn(
          "descent spike: the measured sizes' terrain readout asked to fail the session's computation; counted, and not passed on",
        );
      }
    });
    this.#readoutSessions.set(session, standIn);
    return standIn;
  }

  /** A counter of `follower`'s cache, reading the moment, its views' places and the header. */
  #counterOf(follower: CacheFollower, bytesPerSlot: number): PassSizeCounter {
    return new PassSizeCounter({
      cache: follower.cache,
      views: follower.views.map(({ id, style }) => ({ id, style })),
      viewIndices: (inputs) =>
        matchViewsByWeight(
          follower.views.map((view) => ({
            weight: view.weight,
            viewport:
              (view.role === "main" ? this.#frame?.mainViewport : this.#frame?.secondViewport) ??
              null,
          })),
          inputs,
        ),
      bytesPerSlot,
      context: () => this.#moment,
      sampledLowestM: () => this.#sampledLowestM(),
      ...(this.#nowMs === null ? {} : { nowMs: this.#nowMs }),
    });
  }

  /**
   * The field header's lowest sampled height, m above the datum: its sea level where the body has
   * no main liquid, whose header defines the sea level as the lowest cell's elevation; `null` with
   * no header, and on a body with a main liquid, whose sea may stand above its lowest cell.
   */
  #sampledLowestM(): number | null {
    const frame = this.#frame;
    const header = frame === null ? null : (frame.session.survey(frame.body)?.header ?? null);
    if (header === null) {
      this.#lowest = absent("no field header was held");
      return null;
    }
    if (header.main_liquid !== null) {
      this.#lowest = absent(
        "the body has a main liquid: its header's sea level need not be its lowest cell",
      );
      return null;
    }
    this.#lowest = present(header.sea_level_m);
    return header.sea_level_m;
  }

  /** Compares the two passes' forced sets where both have a last step and either changed. */
  #compareForced(
    lit: SpikeSizesTerrainPass | null,
    wire: SpikeSizesTerrainPass | null,
    tS: number,
  ): void {
    const litStep = lit?.lastStep ?? null;
    const wireStep = wire?.lastStep ?? null;
    const tally = this.#forcedSets;
    if (litStep === null || wireStep === null) {
      return;
    }
    tally.comparedFrames += 1;
    if (litStep.selection === tally.lit && wireStep.selection === tally.wire) {
      // Neither pass selected again since the last comparison: the same verdict.
      tally.differingFrames += tally.lastDiffered ? 1 : 0;
      return;
    }
    tally.lit = litStep.selection;
    tally.wire = wireStep.selection;
    const difference = symmetricDifference(
      forcedKeys(litStep.selection),
      forcedKeys(wireStep.selection),
    );
    tally.lastDiffered = difference > 0;
    if (difference > 0) {
      tally.differingFrames += 1;
      tally.firstDifferingS ??= tS;
      tally.largestDifference = Math.max(tally.largestDifference, difference);
    }
  }

  /** One reading of the session's memory at script time `atS`. */
  #readMemory(session: SpikeSizesSession, atS: number): void {
    const memory = session.memory();
    this.#memorySamples += 1;
    this.#workerField = peakOf(this.#workerField, memory.workerFieldBytes, atS);
    this.#keptField = peakOf(this.#keptField, memory.keptFieldBytes, atS);
    const byName: ReadonlyMap<string, number> = memory.gpuBytes;
    for (const [category, bytes] of byName) {
      this.#sessionGpu.set(category, peakOf(this.#sessionGpu.get(category), bytes, atS));
    }
    this.#workersStarted = Math.max(this.#workersStarted, session.workerCount);
  }

  #memoryReport(): SpikeSizesReport["memory"] {
    const caches = [this.#followers.lit, this.#followers.wireframe].flatMap((follower) => {
      const allocation = follower.allocation;
      return allocation === null
        ? []
        : [
            {
              cache: follower.cache,
              slotCount: allocation.slotCount,
              bytesPerSlot: allocation.bytesPerSlot,
              allocationBytes: allocation.slotCount * allocation.bytesPerSlot,
            },
          ];
    });
    const sessionGpu = [...this.#sessionGpu].map(([category, peak]) => ({
      category,
      bytes: peak.bytes,
      atS: peak.atS,
    }));
    const shadows = this.#sessionGpu.get(CASCADE_MEMORY_CATEGORY);
    const cascades =
      shadows === undefined
        ? { bytes: CASCADE_BYTES, source: "computed" as const }
        : { bytes: shadows.bytes, source: "session" as const };
    let computedGpuBytes = shadows === undefined ? cascades.bytes : 0;
    for (const cache of caches) {
      computedGpuBytes += cache.allocationBytes;
    }
    for (const texture of sessionGpu) {
      computedGpuBytes += texture.bytes;
    }
    const workerFieldBytes = bytePeakOf(this.#workerField);
    return {
      samples: this.#memorySamples,
      workerFieldBytes,
      keptFieldBytes: bytePeakOf(this.#keptField),
      sessionGpu,
      caches,
      cascades,
      computedGpuBytes,
      computedTotalBytes: computedGpuBytes + workerFieldBytes.bytes,
      workersStarted: this.#workersStarted,
    };
  }

  #bindingReport(): SpikeSizesReport["binding"] {
    const requested = this.#requestedBytes.value;
    return {
      adapterBytes: this.#adapterBytes,
      requestedBytes: this.#requestedBytes,
      deviceBytes: this.#deviceBytes,
      caches: [this.#followers.lit, this.#followers.wireframe].flatMap((follower) => {
        const layout = follower.layout;
        return layout === null
          ? []
          : [
              {
                cache: follower.cache,
                slotCount: layout.slotCount,
                ceilingAtRequestedSlots:
                  requested === null
                    ? null
                    : finiteOrNull(bindingCeilingSlots(layout.fields, requested)),
                ceilingAtDefaultSlots: finiteOrNull(bindingCeilingSlots(layout.fields)),
              },
            ];
      }),
    };
  }
}
