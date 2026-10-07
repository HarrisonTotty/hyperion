/**
 * The descent's fixed-step run (plan R05, T13.a, Design note 19): the scripted path sampled at a
 * fixed rate through `selectPatches` and the terrain's own patch cache, CPU-only, so that the
 * selection sequence is identical bit for bit and its measured demand can be set beside the
 * prediction.
 *
 * @remarks
 * Each frame runs the terrain pass's order (T8, T11.c): select with the cache's baked ranges as
 * `heightRanges`, resolve the draw set, `retain` the selection and the draw set, then bake. The
 * pool is ideal, stated: every patch the selection requests is baked and inserted the same frame
 * (no bake latency, no worker limit), with the range `rangeOf` gives; the demand's breadth-first
 * rule still makes a deep patch wait a frame for each unbaked ancestor. So the measured demand is
 * the patches baked a second under an ideal pool, the plan's first-time-selected keys a second
 * with the cache's semantics. Each frame records the patches selected, the bakes, `limited`, the
 * draw set's stand-ins and missing patches, and how long `selectPatches` took, on the wall clock
 * and on the thread's CPU clock where the caller gives one; the bakes' own time is kept out of the
 * wall-time deadline, so that the cells' coverage compares. Under load the wall-clock times are
 * upper bounds: a process waiting for a core counts the wait, which its CPU time does not.
 *
 * It also records what decision-r05-high-bound.md's F4 asks of the record: the budget's effective
 * tolerance τ′ (`Selection.limitExcess`), the coarse stand-ins drawn, and the forced region's
 * bakes.
 *
 * It selects at the terrain pass's τ_sel = τ ÷ (1 + m), m = `SELECTION_MARGIN`
 * (`selectionTolerancePx`), and computes D at the same tolerance (decision-r05-record-tau.md). It
 * selects every frame, without the pass's cadence: the pass's selection at a frame is the run's at
 * a pose at most `RESELECT_MOVE_FRACTION` = m ÷ (1 + m), about 0.0909, × d_min earlier, which moves
 * the demand in time, not in size.
 */

import { vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M } from "../camera/projection";
import type { QualitySetting } from "../quality/qualitySetting";
import { type DrawSet, DrawSetResolver, PatchCache } from "../terrain/cache";
import {
  finestPatchSizeM,
  type GroundContact,
  heldRadiusM,
  isDescending,
} from "../terrain/grounded";
import { ancestorIndex, type PatchKey, patchKeyIndex, patchKeyString } from "../terrain/patchKey";
import type { PlanetGeometry } from "../terrain/planet";
import {
  type Selection,
  type SelectionInput,
  screenSpaceErrorPx,
  selectionErrorM,
  selectPatches,
  type ViewSelectionInput,
} from "../terrain/select";
import { SELECTION_MARGIN, selectionTolerancePx } from "../terrain/selectionTolerance";
import type { SlotLayout } from "../terrain/slotLayout";
import { distanceToBoxFromM } from "../terrain/viewGeometry";
import { type DemandView, perLevelDemand } from "./demand";
import type { DescentPose, DescentProfile } from "./descentProfile";
import { SPIKE_CRAFT_RADIUS_M } from "./spikeScene";

/**
 * The scripted craft as the selection's contacts (Design note 9): one contact at the ground point
 * beneath it while it is descending (`isDescending`) or grounded, none otherwise.
 *
 * @remarks
 * Both read the height above the floor under the craft (`heightAboveFloorM`, the clearance ruling's
 * honest h, never above the true height). Grounded is that height within the held radius r_g, the craft's radius (the scene's hull, `SPIKE_CRAFT_RADIUS_M`) plus one
 * finest patch:
 * the held sphere about the craft then reaches the ground, as it does at an exact hover, where the
 * vertical speed is 0 and `isDescending` does not hold. The contact sits on the ground, not at the
 * camera, since the forced region's rule is 3-D (T11.c's note).
 *
 * @param patchSizeM - The finest patch's edge, metres (`finestPatchSizeM`).
 */
export function craftContacts(pose: DescentPose, patchSizeM: number): GroundContact[] {
  const contact: GroundContact = { positionM: pose.groundPointM, radiusM: SPIKE_CRAFT_RADIUS_M };
  const heightM = pose.heightAboveFloorM;
  const grounded = heightM <= heldRadiusM(contact, patchSizeM);
  return grounded || isDescending(heightM, pose.verticalSpeedMps) ? [contact] : [];
}

/**
 * The finest stand-in level the record watches (decision-r05-high-bound.md, F4): a stand-in of
 * this level or coarser, drawn in place of selected patches finer than it, covers a large part of
 * the view ({@link coarseStandIns}).
 */
export const COARSE_STAND_IN_LEVEL = 12;

/**
 * The margin the terrain pass's morph bands keep over the selection's tolerance, 1 +
 * {@link SELECTION_MARGIN} (`terrainPass.ts`, R05.T11.c's F2 as built): where τ′ changes between
 * two selections by more than this factor, a split can start partly morphed, or a merge's children
 * have been.
 */
export const BAND_MARGIN = 1 + SELECTION_MARGIN;

/**
 * A selection's effective tolerance for a view, pixels: the view's τ × max(1, `limitExcess` ÷ w),
 * which every baked leaf the view sees meets (decision-r05-high-bound.md, F1 and F4; R05.T7 as
 * built). It is the view's τ where the budget does not bind, `limitExcess` then being 0; for the
 * run's view, which selects at τ_sel, that is τ_sel.
 */
export function effectiveTolerancePx(
  selection: Pick<Selection, "limitExcess">,
  view: Pick<ViewSelectionInput, "tauPx" | "weight">,
): number {
  return view.tauPx * Math.max(1, selection.limitExcess / view.weight);
}

/** The coarse stand-ins a frame draws, as {@link FixedStepFrame} records them. */
export interface CoarseStandIns {
  /**
   * The drawn stand-ins of level {@link COARSE_STAND_IN_LEVEL} or coarser, and those of them that
   * cover a selected patch the cache held earlier in the run and evicted (a return).
   */
  readonly count: number;
  readonly returns: number;
  /**
   * The largest ρ such a stand-in draws, and such a stand-in of a return, pixels of bound: the
   * stand-in's level's selection error at the distance of a selected patch it covers; 0 where none.
   */
  readonly rhoPx: number;
  readonly returnRhoPx: number;
}

const NO_COARSE_STAND_INS: CoarseStandIns = { count: 0, returns: 0, rhoPx: 0, returnRhoPx: 0 };

/**
 * Finds the frame's coarse stand-ins (decision-r05-high-bound.md, F4): the patches of level
 * {@link COARSE_STAND_IN_LEVEL} or coarser that `draw` draws in place of selected patches, and the
 * ρ they draw at each selected patch some view sees beneath them, its box's distance from the camera
 * floored at the near plane as selection measures it.
 *
 * @remarks
 * A stand-in covers a selected patch not yet resident, and with it any resident ones beneath it
 * (`DrawSetResolver`); outside a forced region and the balance's splits, which the streaming gate
 * does not hold, that is the patch's parent. A return is a covered patch the cache does not hold
 * now but evicted earlier (`evicted`): the coarse thrash F3 fixed. The rest are first bakes, which
 * an ideal pool lands after the frame's draw.
 *
 * @param evicted - The `patchKeyString`s of every patch the cache has evicted so far in the run.
 */
export function coarseStandIns(
  selection: Selection,
  draw: DrawSet,
  cache: PatchCache,
  evicted: ReadonlySet<string>,
  planet: PlanetGeometry,
  view: ViewSelectionInput,
): CoarseStandIns {
  // The drawn coarse stand-ins by level, each by its patchKeyIndex, with whether it covers a return.
  const coarse = new Map<number, Map<number, boolean>>();
  for (let n = 0; n < draw.count; n += 1) {
    const drawn = draw.patches[n];
    if (drawn?.standIn === true && drawn.patch.key.level <= COARSE_STAND_IN_LEVEL) {
      const { key } = drawn.patch;
      const level = coarse.get(key.level) ?? new Map<number, boolean>();
      level.set(patchKeyIndex(key), false);
      coarse.set(key.level, level);
    }
  }
  if (coarse.size === 0) {
    return NO_COARSE_STAND_INS;
  }
  let rhoPx = 0;
  let returnRhoPx = 0;
  for (const [keyString, patch] of selection.patches) {
    const { key } = patch;
    if (!patch.seen) {
      continue;
    }
    for (let level = Math.min(key.level - 1, COARSE_STAND_IN_LEVEL); level >= 0; level -= 1) {
      const standIns = coarse.get(level);
      const index = ancestorIndex(key, level);
      if (standIns?.has(index) !== true) {
        continue;
      }
      const distanceM = Math.max(
        distanceToBoxFromM(patch.bounds, view.camera.positionM),
        NEAR_PLANE_M,
      );
      const drawnRhoPx = screenSpaceErrorPx(selectionErrorM(planet, level), distanceM, view);
      rhoPx = Math.max(rhoPx, drawnRhoPx);
      if (!cache.has(keyString) && evicted.has(keyString)) {
        standIns.set(index, true);
        returnRhoPx = Math.max(returnRhoPx, drawnRhoPx);
      }
      // The draw set holds no stand-in beneath another.
      break;
    }
  }
  let count = 0;
  let returns = 0;
  for (const standIns of coarse.values()) {
    for (const covered of standIns.values()) {
      count += 1;
      returns += covered ? 1 : 0;
    }
  }
  return { count, returns, rhoPx, returnRhoPx };
}

/** What a fixed-step run reads. */
export interface FixedStepOptions {
  readonly profile: DescentProfile;
  /** The planet under the bound rule the run records (`boundedPlanet`). */
  readonly planet: PlanetGeometry;
  readonly setting: QualitySetting;
  /**
   * The setting's view, at the setting's τ: the run selects, and computes D, at its τ_sel
   * ({@link selectionTolerancePx}).
   */
  readonly view: DemandView & { readonly heightPx: number };
  /** Samples a second (64 in the plan). */
  readonly rateHz: number;
  /** The span sampled, script seconds. */
  readonly fromS: number;
  readonly toS: number;
  /**
   * Where measuring starts, script seconds: the frames before it fill the cache, whose first
   * bakes are the run's start rather than the motion, and are marked `warmup`.
   */
  readonly measureFromS: number;
  /** The patch budget, or `undefined` for none. */
  readonly maxPatches: number | undefined;
  /** The cache's slots, the setting's own. */
  readonly layout: SlotLayout;
  /** A baked patch's lowest and highest height, metres. */
  readonly rangeOf: (key: PatchKey) => readonly [number, number];
  /** The clock `selectPatches` and the bakes are timed by, ms. */
  readonly nowMs: () => number;
  /**
   * The thread's CPU-time clock `selectPatches` is also timed by, ms: user and system time, as
   * Node's `process.threadCpuUsage` gives it just after `process.cpuUsage` (the record's script).
   * None by default, the CPU times then null.
   */
  readonly cpuNowMs?: () => number;
  /**
   * The wall time on `nowMs`'s clock after which the run stops, cut short, the bakes' time not
   * counted; none by default.
   */
  readonly deadlineMs?: number;
  /**
   * The wall time on `nowMs`'s clock after which the run stops whatever the bakes took, so that a
   * process under an outer timeout always writes what it has; none by default.
   */
  readonly wallDeadlineMs?: number;
}

/** One frame of a run. */
export interface FixedStepFrame {
  readonly tS: number;
  /** Whether the frame is before `measureFromS`, filling the cache. */
  readonly warmup: boolean;
  readonly segment: string;
  readonly patches: number;
  /** Patches requested and baked this frame. */
  readonly demanded: number;
  readonly limited: boolean;
  /**
   * The selection's effective tolerance τ′ = τ_sel × max(1, `limitExcess` ÷ w), pixels
   * ({@link effectiveTolerancePx}): τ_sel where not `limited`.
   */
  readonly tauPrimePx: number;
  /** Selected patches drawn by a resident ancestor, and those with none. */
  readonly standingIn: number;
  readonly missing: number;
  /**
   * The drawn stand-ins of level {@link COARSE_STAND_IN_LEVEL} or coarser, those of them covering a
   * return, and the largest ρ each set draws, pixels of bound, 0 where none
   * ({@link coarseStandIns}).
   */
  readonly coarseStandIns: number;
  readonly coarseReturns: number;
  readonly coarseStandInRhoPx: number;
  readonly coarseReturnRhoPx: number;
  /** Patches of a grounded body's forced region requested and baked this frame. */
  readonly forcedDemanded: number;
  /**
   * How long `selectPatches` took on the wall clock, and on the thread's CPU clock, ms; the CPU
   * time null without `cpuNowMs`.
   */
  readonly selectMs: number;
  readonly selectCpuMs: number | null;
  /**
   * The per-level prediction at this frame, patches a second, at the height above the floor and at
   * τ_sel, the tolerance the frame selected at.
   */
  readonly predictedPerS: number;
  /** The camera's height above the floor under it, metres (`DescentPose.heightAboveFloorM`). */
  readonly heightAboveFloorM: number;
}

/** A run's frames and its selection sequence's hash. */
export interface FixedStepRun {
  /** The setting's τ the run was given, and τ_sel, the tolerance it selected at, pixels. */
  readonly tauPx: number;
  readonly selectionTauPx: number;
  readonly frames: ReadonlyArray<FixedStepFrame>;
  /** FNV-1a (64-bit) over every frame's selected keys in their order, as 16 hex digits. */
  readonly hash: string;
  /** Whether `deadlineMs` cut the run before `toS`. */
  readonly truncated: boolean;
}

/** FNV-1a 64-bit, in two 32-bit halves so that each byte costs no `bigint` work. */
class Fnv64 {
  #hi = 0xcbf2_9ce4;
  #lo = 0x8422_2325;

  /** Folds one byte. */
  byte(b: number): void {
    // h ^= b; h *= 0x100000001b3 = 2^40 + 0x1b3, as (hi, lo) halves mod 2^64.
    const lo = (this.#lo ^ b) >>> 0;
    const hi = this.#hi;
    const loTimes = lo * 0x1b3;
    const newLo = loTimes >>> 0;
    const carry = Math.floor(loTimes / 0x1_0000_0000);
    this.#hi = (Math.imul(hi, 0x1b3) + carry + ((lo << 8) >>> 0)) >>> 0;
    this.#lo = newLo;
  }

  /** Folds a string's UTF-16 code units, each as one byte (keys are ASCII), then a separator. */
  text(s: string): void {
    for (let i = 0; i < s.length; i += 1) {
      this.byte(s.charCodeAt(i) & 0xff);
    }
    this.byte(0x0a);
  }

  hex(): string {
    return this.#hi.toString(16).padStart(8, "0") + this.#lo.toString(16).padStart(8, "0");
  }
}

const ORIGIN = vec3(0, 0, 0);

/** Runs the descent at a fixed step from `fromS` to `toS`. */
export function runFixedStep(options: FixedStepOptions): FixedStepRun {
  const { profile, planet, view } = options;
  const cache = new PatchCache(options.layout);
  const resolver = new DrawSetResolver(cache);
  const frames: FixedStepFrame[] = [];
  const hash = new Fnv64();
  const steps = Math.round((options.toS - options.fromS) * options.rateHz);
  const deadline = options.deadlineMs ?? Infinity;
  const wallDeadline = options.wallDeadlineMs ?? Infinity;
  const patchSizeM = finestPatchSizeM(planet);
  // D predicts the selection it is set beside, so it takes the same tolerance.
  const selectionTauPx = selectionTolerancePx(view.tauPx);
  const demandView: DemandView = { ...view, tauPx: selectionTauPx };
  // Every patch evicted so far, to tell a return from a first bake.
  const evicted = new Set<string>();
  let bakeMs = 0;
  let truncated = false;
  for (let step = 0; step <= steps; step += 1) {
    const nowMs = options.nowMs();
    if (nowMs - bakeMs > deadline || nowMs > wallDeadline) {
      truncated = true;
      break;
    }
    const tS = options.fromS + step / options.rateHz;
    const pose = profile.poseAt(tS);
    const grounded = craftContacts(pose, patchSizeM);
    const selectionView: ViewSelectionInput = {
      camera: { positionM: pose.positionM, orientation: pose.orientation },
      fovXRad: view.fovXRad,
      viewport: { widthPx: view.widthPx, heightPx: view.heightPx },
      weight: 1,
      tauPx: selectionTauPx,
    };
    const input: SelectionInput = {
      planet,
      views: [selectionView],
      setting: options.setting,
      grounded,
      heightRanges: cache,
      ...(options.maxPatches === undefined ? {} : { maxPatches: options.maxPatches }),
    };
    const start = options.nowMs();
    const cpuStart = options.cpuNowMs?.() ?? null;
    const selection = selectPatches(input);
    const cpuEnd = options.cpuNowMs?.() ?? null;
    const selectMs = options.nowMs() - start;
    for (const keyString of selection.patches.keys()) {
      hash.text(keyString);
    }
    hash.text("|");
    const draw = resolver.resolve(selection);
    // Before this frame's bakes, as the draw set was resolved.
    const coarse = coarseStandIns(selection, draw, cache, evicted, planet, selectionView);
    cache.retain(selection, draw);
    let demanded = 0;
    let forcedDemanded = 0;
    for (const request of selection.demand) {
      if (cache.has(patchKeyString(request.key))) {
        continue;
      }
      const bakeStart = options.nowMs();
      const heightRangeM = options.rangeOf(request.key);
      bakeMs += options.nowMs() - bakeStart;
      const stored = cache.insert({
        key: request.key,
        generation: 0,
        originM: ORIGIN,
        heightRangeM,
        boundingRadiusM: 0,
      });
      if (stored.kind === "stored" && stored.evicted !== null) {
        evicted.add(stored.evicted);
      }
      demanded += 1;
      forcedDemanded += request.forced ? 1 : 0;
    }
    frames.push({
      tS,
      warmup: tS < options.measureFromS,
      segment: pose.segment,
      patches: selection.patches.size,
      demanded,
      limited: selection.limited,
      tauPrimePx: effectiveTolerancePx(selection, selectionView),
      standingIn: draw.standingIn,
      missing: draw.missing,
      coarseStandIns: coarse.count,
      coarseReturns: coarse.returns,
      coarseStandInRhoPx: coarse.rhoPx,
      coarseReturnRhoPx: coarse.returnRhoPx,
      forcedDemanded,
      selectMs,
      selectCpuMs: cpuStart === null || cpuEnd === null ? null : cpuEnd - cpuStart,
      predictedPerS: perLevelDemand(
        planet,
        { ...pose, altitudeM: pose.heightAboveFloorM },
        demandView,
      ).perS,
      heightAboveFloorM: pose.heightAboveFloorM,
    });
  }
  return { tauPx: view.tauPx, selectionTauPx, frames, hash: hash.hex(), truncated };
}

/** A segment's figures over a run. */
export interface SegmentFigures {
  readonly segment: string;
  readonly frames: number;
  readonly meanPatches: number;
  readonly maxPatches: number;
  /** Measured demand: patches baked over the segment's measured span, a second. */
  readonly demandPerS: number;
  /** The forced region's part of it: its patches baked, a second. */
  readonly forcedDemandPerS: number;
  /** The mean per-level prediction over the segment's frames. */
  readonly predictedPerS: number;
  readonly limitedFraction: number;
  /**
   * The share of the `limited` frames whose τ′ exceeds the setting's τ; null where none is
   * limited. Selecting at τ_sel, a limited frame may still draw within τ (decision-r05-record-tau.md).
   */
  readonly limitedOverTauFraction: number | null;
  /**
   * τ′ over the `limited` frames, pixels: the 50th and 95th percentiles (nearest rank) and the
   * largest; null where no frame is limited.
   */
  readonly tauPrimePxP50: number | null;
  readonly tauPrimePxP95: number | null;
  readonly tauPrimePxMax: number | null;
  /**
   * The change of τ′ from the selection before, |Δτ′|, pixels, over the steps where either
   * selection is `limited`: the 95th percentile and the largest; null where there is no such step.
   * A step belongs to its later frame's segment, so the one before may be the last warm-up frame's
   * or the previous segment's last.
   */
  readonly tauPrimeStepPxP95: number | null;
  readonly tauPrimeStepPxMax: number | null;
  /**
   * The same steps as the larger τ′ over the smaller, less 1: about Δτ′ ÷ τ′, of which a vertex
   * inside its morph band steps by about 6 times in morph factor (R05.T11.c, F2 as built).
   */
  readonly tauPrimeStepRatioP95: number | null;
  readonly tauPrimeStepRatioMax: number | null;
  /** The share of those steps whose factor exceeds {@link BAND_MARGIN}; null where none. */
  readonly tauPrimeStepsOverMargin: number | null;
  /** The share of frames drawing some selected patch by a stand-in. */
  readonly standingInFraction: number;
  /**
   * The share of frames drawing a stand-in of level {@link COARSE_STAND_IN_LEVEL} or coarser, and
   * the share drawing one for a return ({@link coarseStandIns}).
   */
  readonly coarseStandInFraction: number;
  readonly coarseReturnFraction: number;
  /** The largest ρ such stand-ins draw, and those for returns, pixels of bound; null where none. */
  readonly coarseStandInMaxRhoPx: number | null;
  readonly coarseReturnMaxRhoPx: number | null;
  /** The least and greatest height above the floor over the frames, metres. */
  readonly minHeightAboveFloorM: number;
  readonly maxHeightAboveFloorM: number;
  /**
   * `selectPatches`' wall-clock time, ms: the 50th and 95th percentiles (nearest rank) and the
   * largest; upper bounds under load.
   */
  readonly selectMsP50: number;
  readonly selectMsP95: number;
  readonly selectMsMax: number;
  /**
   * The same on the thread's CPU clock, the selection's own work, ms; null where the run had no
   * CPU clock.
   */
  readonly selectCpuMsP50: number | null;
  readonly selectCpuMsP95: number | null;
  readonly selectCpuMsMax: number | null;
}

function nearestRank(sorted: ReadonlyArray<number>, p: number): number {
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))] ?? 0;
}

/** The 50th and 95th percentiles (nearest rank) and the largest of some values. */
interface Spread {
  readonly p50: number | null;
  readonly p95: number | null;
  readonly max: number | null;
}

/** The {@link Spread} of `values`, nulls if there are none. */
function spread(values: ReadonlyArray<number>): Spread {
  if (values.length === 0) {
    return { p50: null, p95: null, max: null };
  }
  const sorted = values.toSorted((a, b) => a - b);
  return {
    p50: nearestRank(sorted, 0.5),
    p95: nearestRank(sorted, 0.95),
    max: sorted.at(-1) ?? null,
  };
}

/** The largest of `values`, or null if none is above 0. */
function largestPositive(values: ReadonlyArray<number>): number | null {
  const largest = values.reduce((most, x) => Math.max(most, x), 0);
  return largest > 0 ? largest : null;
}

/** A change of τ′ between two consecutive selections. */
interface TauPrimeStep {
  readonly px: number;
  /** The larger τ′ over the smaller. */
  readonly factor: number;
}

/** The change of τ′ into `frame` from `previous`, where either is limited; none otherwise. */
function tauPrimeStep(frame: FixedStepFrame, previous: FixedStepFrame | undefined): TauPrimeStep[] {
  if (previous === undefined || !(frame.limited || previous.limited)) {
    return [];
  }
  const [low, high] = [frame.tauPrimePx, previous.tauPrimePx].toSorted((a, b) => a - b);
  return [{ px: (high ?? NaN) - (low ?? NaN), factor: (high ?? NaN) / (low ?? NaN) }];
}

/** A run's figures by segment, over its measured frames (the warm-up left out). */
export function segmentFigures(run: FixedStepRun, rateHz: number): SegmentFigures[] {
  const bySegment = new Map<string, { frames: FixedStepFrame[]; steps: TauPrimeStep[] }>();
  run.frames.forEach((frame, n) => {
    if (frame.warmup) {
      return;
    }
    const entry = bySegment.get(frame.segment) ?? { frames: [], steps: [] };
    entry.frames.push(frame);
    entry.steps.push(...tauPrimeStep(frame, run.frames[n - 1]));
    bySegment.set(frame.segment, entry);
  });
  return [...bySegment].map(([segment, { frames, steps }]) => {
    const spanS = frames.length / rateHz;
    const times = spread(frames.map(({ selectMs }) => selectMs));
    const cpuTimes = spread(frames.flatMap(({ selectCpuMs }) => selectCpuMs ?? []));
    const sum = (pick: (f: FixedStepFrame) => number): number =>
      frames.reduce((total, f) => total + pick(f), 0);
    const share = (holds: (f: FixedStepFrame) => boolean): number =>
      frames.filter(holds).length / frames.length;
    const limitedTauPrimePx = frames.filter(({ limited }) => limited).map((f) => f.tauPrimePx);
    const tauPrime = spread(limitedTauPrimePx);
    const stepPx = spread(steps.map(({ px }) => px));
    const stepRatio = spread(steps.map(({ factor }) => factor - 1));
    return {
      segment,
      frames: frames.length,
      meanPatches: sum((f) => f.patches) / frames.length,
      maxPatches: frames.reduce((most, f) => Math.max(most, f.patches), 0),
      demandPerS: spanS > 0 ? sum((f) => f.demanded) / spanS : 0,
      forcedDemandPerS: spanS > 0 ? sum((f) => f.forcedDemanded) / spanS : 0,
      predictedPerS: sum((f) => f.predictedPerS) / frames.length,
      limitedFraction: share(({ limited }) => limited),
      limitedOverTauFraction:
        limitedTauPrimePx.length === 0
          ? null
          : limitedTauPrimePx.filter((px) => px > run.tauPx).length / limitedTauPrimePx.length,
      tauPrimePxP50: tauPrime.p50,
      tauPrimePxP95: tauPrime.p95,
      tauPrimePxMax: tauPrime.max,
      tauPrimeStepPxP95: stepPx.p95,
      tauPrimeStepPxMax: stepPx.max,
      tauPrimeStepRatioP95: stepRatio.p95,
      tauPrimeStepRatioMax: stepRatio.max,
      tauPrimeStepsOverMargin:
        steps.length === 0
          ? null
          : steps.filter(({ factor }) => factor > BAND_MARGIN).length / steps.length,
      standingInFraction: share(({ standingIn }) => standingIn > 0),
      coarseStandInFraction: share((f) => f.coarseStandIns > 0),
      coarseReturnFraction: share((f) => f.coarseReturns > 0),
      coarseStandInMaxRhoPx: largestPositive(frames.map((f) => f.coarseStandInRhoPx)),
      coarseReturnMaxRhoPx: largestPositive(frames.map((f) => f.coarseReturnRhoPx)),
      minHeightAboveFloorM: frames.reduce(
        (least, f) => Math.min(least, f.heightAboveFloorM),
        Infinity,
      ),
      maxHeightAboveFloorM: frames.reduce(
        (most, f) => Math.max(most, f.heightAboveFloorM),
        -Infinity,
      ),
      selectMsP50: times.p50 ?? 0,
      selectMsP95: times.p95 ?? 0,
      selectMsMax: times.max ?? 0,
      selectCpuMsP50: cpuTimes.p50,
      selectCpuMsP95: cpuTimes.p95,
      selectCpuMsMax: cpuTimes.max,
    };
  });
}
