/**
 * The seamless probe's record and its summary (the owner's priority, 2026-11-01: "terrain pops in
 * with very distinct chunks"): what `seamlessProbe.wasm.test.ts` records of every change of the
 * drawn ground over TERRAIN TEST's script, and the figures its acceptance bounds read.
 *
 * @remarks
 * The probe diffs each frame's draw set against the frame before's:
 *
 * - **refine**: a drawn ancestor replaced by descendants;
 * - **coarsen**: drawn descendants replaced by an ancestor;
 * - **rebake**: a drawn key's bake replaced in place, after a field block;
 * - **appear**: a patch drawn where nothing was drawn before (new survey, or the survey's edge).
 *
 * For each change it measures, over the fine patch's vertices in front of the camera and inside
 * the viewport, the largest on-screen move of the drawn surface, |Δh| · |ν × r̂| ÷ r in pixels, and
 * the largest turn of the drawn shading normal. A pop is a change that moves more than
 * {@link POP_PX} or turns more than {@link POP_NORMAL_DEG}. Pure: no file, clock or module state.
 */

import type { QualitySetting } from "../quality/qualitySetting";

/** The change of the drawn ground a {@link SeamEvent} records. */
export type SeamEventKind = "refine" | "coarsen" | "rebake" | "appear";

/** One frame of a probe run. */
export interface SeamFrame {
  /** The script's time, seconds. */
  readonly tS: number;
  /** Patches drawn. */
  readonly drawn: number;
  /** Selected patches drawn by a coarser resident ancestor (the draw set's `standingIn`). */
  readonly standingIn: number;
  /** Selected patches with no resident ancestor (the draw set's `missing`). */
  readonly missing: number;
  /** The most levels between a seen selected patch and its nearest resident patch or ancestor. */
  readonly maxGap: number;
  /** The selection's `limitExcess`. */
  readonly limitExcess: number;
  /** Patches selected. */
  readonly selected: number;
}

/** One change of the drawn ground. */
export interface SeamEvent {
  readonly tS: number;
  readonly kind: SeamEventKind;
  /** The finer side's level: the descendant's, or the rebaked or appearing patch's own. */
  readonly fineLevel: number;
  /** The levels between the two sides; 0 for a rebake or an appearance. */
  readonly levelJump: number;
  /** The largest on-screen move, pixels; −1 where nothing was measured (an appearance). */
  readonly maxPx: number;
  /** The largest turn of the shading normal, degrees; −1 where nothing was measured. */
  readonly maxNormalDeg: number;
  /** The least CDLOD morph factor k over the finer side's measured vertices. */
  readonly minFineK: number;
  /** Whether the patch drawn after stands in for a missing one. */
  readonly standIn: boolean;
}

/** A link of a missing patch's chain of ancestors at a collapse. */
export interface SeamChainLink {
  readonly level: number;
  /** Resident in the pass's cache at the collapse. */
  readonly resident: boolean;
  /** Drawn the frame before. */
  readonly drawnBefore: boolean;
}

/** A frame whose drawn set fell below half the frame before's, with what was missing. */
export interface SeamCollapse {
  readonly tS: number;
  readonly before: number;
  readonly after: number;
  /** The keys drawn after, as `patchKeyString`s. */
  readonly drawnKeys: ReadonlyArray<string>;
  /**
   * Up to six seen selected patches not resident, the farthest from a resident ancestor first, each
   * with the levels up to its nearest resident ancestor (`gap`) and up to eight of its ancestors,
   * parent first.
   */
  readonly missing: ReadonlyArray<{
    readonly key: string;
    readonly drawnBefore: boolean;
    readonly gap: number;
    readonly chain: ReadonlyArray<SeamChainLink>;
  }>;
}

/** A whole probe run. */
export interface SeamRun {
  /** The modelled bake's cost, milliseconds, and the workers baking. */
  readonly costMs: number;
  readonly workers: number;
  readonly setting: QualitySetting;
  readonly fromS: number;
  readonly toS: number;
  /** When the pass was made (the hand-over), and when it first drew, seconds; `null` if never. */
  readonly handOverS: number | null;
  readonly firstDrawS: number | null;
  readonly bakesAnswered: number;
  /** Bakes the cache refused a slot (`TerrainPassObserver.onRefused`), each asked for again. */
  readonly refusedBakes: number;
  /**
   * Selected patches the frame before, resident then, still selected and no longer resident: a
   * patch of the selection evicted while selected.
   */
  readonly selectedEvicted: number;
  /** Each demanded patch's time from its first demand to its residency, seconds. */
  readonly latenciesS: ReadonlyArray<number>;
  readonly frames: ReadonlyArray<SeamFrame>;
  readonly events: ReadonlyArray<SeamEvent>;
  readonly collapses: ReadonlyArray<SeamCollapse>;
}

/** A change moving the drawn surface more than this on screen is a pop, pixels. */
export const POP_PX = 1;

/** A change turning the drawn shading normal more than this is a pop, degrees. */
export const POP_NORMAL_DEG = 2;

/** A change of this many levels or more is a jump (the research's "changes of ≥ 3 levels"). */
export const JUMP_LEVELS = 3;

/** The probe's figures, which its acceptance bounds read. */
export interface SeamSummary {
  /** The span measured, from the first frame to the last, seconds. */
  readonly spanS: number;
  readonly frames: number;
  /** The share of frames with a selected patch standing in, 0 to 1. */
  readonly standInFrames: number;
  /** Frames with a selected, seen patch that has no resident ancestor. */
  readonly holeFrames: number;
  /** Demand → resident, seconds; `null` with no residency. */
  readonly demandToResidentS: {
    readonly p50: number | null;
    readonly p95: number | null;
    readonly max: number | null;
    readonly count: number;
  };
  /** Frames whose drawn count fell below half the frame before's (of more than 20). */
  readonly collapses: number;
  /** Refines and coarsens of {@link JUMP_LEVELS} levels or more, and their largest move, pixels. */
  readonly jumps: number;
  readonly jumpMaxPx: number;
  /** The largest coarsen's move, pixels; 0 with none. */
  readonly largestCoarsenPx: number;
  readonly selectedEvicted: number;
  readonly refusedBakes: number;
  /** Pops a second over the span from the first draw. */
  readonly popsPerS: number;
  /** The share of refines whose finer side had a vertex at k < 0.5, 0 to 1; 0 with none. */
  readonly refinesBelowHalfK: number;
  /** The shading normal's turn over the measured changes, degrees; 0 with none. */
  readonly normalJumpDeg: { readonly p95: number; readonly max: number };
  /**
   * The bands' τ′ = τ × max(1, `limitExcess`) between frames (w = 1): how many frames it stepped,
   * how many by more than 5%, and the largest step, as a ratio less one.
   */
  readonly bandSteps: {
    readonly count: number;
    readonly over5Percent: number;
    readonly max: number;
  };
}

/**
 * The `p`th percentile of `values`, by rank (the nearest rank of p ÷ 100 × (n − 1), as the
 * research's `seamsum.py`); `null` for none.
 */
export function percentile(values: ReadonlyArray<number>, p: number): number | null {
  if (values.length === 0) {
    return null;
  }
  const sorted = values.toSorted((a, b) => a - b);
  const at = Math.min(sorted.length - 1, Math.round((p / 100) * (sorted.length - 1)));
  return sorted[at] ?? null;
}

/** The largest of `values`, at least 0: a loop, since a run's tens of thousands overflow a spread. */
function maxOf(values: ReadonlyArray<number>): number {
  let max = 0;
  for (const value of values) {
    max = Math.max(max, value);
  }
  return max;
}

/** Whether `event` is a pop: a measured move over {@link POP_PX} or turn over {@link POP_NORMAL_DEG}. */
export function isPop(event: SeamEvent): boolean {
  return event.maxPx > POP_PX || event.maxNormalDeg > POP_NORMAL_DEG;
}

/** Whether frame `after` holds fewer than half of `before`'s drawn patches, of more than 20. */
export function isCollapse(before: SeamFrame, after: SeamFrame): boolean {
  return before.drawn > 20 && after.drawn < 0.5 * before.drawn;
}

/** The figures of `run` ({@link SeamSummary}). */
export function summariseSeamRun(run: SeamRun): SeamSummary {
  const { frames, events } = run;
  const first = frames[0];
  const last = frames.at(-1);
  const spanS = first === undefined || last === undefined ? 0 : last.tS - first.tS;
  let collapses = 0;
  let bandStepCount = 0;
  let bandOver5 = 0;
  let bandMax = 0;
  for (let n = 1; n < frames.length; n += 1) {
    const before = frames[n - 1];
    const after = frames[n];
    if (before === undefined || after === undefined) {
      continue;
    }
    collapses += isCollapse(before, after) ? 1 : 0;
    const a = Math.max(1, before.limitExcess);
    const b = Math.max(1, after.limitExcess);
    if (a !== b) {
      const step = Math.max(a, b) / Math.min(a, b) - 1;
      bandStepCount += 1;
      bandOver5 += step > 0.05 ? 1 : 0;
      bandMax = Math.max(bandMax, step);
    }
  }
  const measured = events.filter((e) => e.maxPx >= 0);
  const jumps = measured.filter(
    (e) => (e.kind === "refine" || e.kind === "coarsen") && e.levelJump >= JUMP_LEVELS,
  );
  const refines = measured.filter((e) => e.kind === "refine");
  const normals = events.filter((e) => e.maxNormalDeg >= 0).map((e) => e.maxNormalDeg);
  const drawnFromS = run.firstDrawS ?? last?.tS ?? 0;
  const drawnSpanS = (last?.tS ?? 0) - drawnFromS;
  const pops = events.filter((e) => e.tS >= drawnFromS && isPop(e)).length;
  const latencies = run.latenciesS;
  return {
    spanS,
    frames: frames.length,
    standInFrames:
      frames.length === 0 ? 0 : frames.filter((f) => f.standingIn > 0).length / frames.length,
    holeFrames: frames.filter((f) => f.missing > 0).length,
    demandToResidentS: {
      p50: percentile(latencies, 50),
      p95: percentile(latencies, 95),
      max: latencies.length === 0 ? null : maxOf(latencies),
      count: latencies.length,
    },
    collapses,
    jumps: jumps.length,
    jumpMaxPx: maxOf(jumps.map((e) => e.maxPx)),
    largestCoarsenPx: maxOf(measured.filter((e) => e.kind === "coarsen").map((e) => e.maxPx)),
    selectedEvicted: run.selectedEvicted,
    refusedBakes: run.refusedBakes,
    popsPerS: drawnSpanS > 0 ? pops / drawnSpanS : 0,
    refinesBelowHalfK:
      refines.length === 0 ? 0 : refines.filter((e) => e.minFineK < 0.5).length / refines.length,
    normalJumpDeg: { p95: percentile(normals, 95) ?? 0, max: maxOf(normals) },
    bandSteps: { count: bandStepCount, over5Percent: bandOver5, max: bandMax },
  };
}
