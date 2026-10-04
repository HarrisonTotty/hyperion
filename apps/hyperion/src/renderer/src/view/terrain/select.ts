/**
 * Patch selection: which patches each view needs, as a pure function of the views, the quality
 * setting and the grounded bodies (plan R05, T7.b–T7.d, Design note 7).
 *
 * @remarks
 * A level is drawn where its error bound subtends at most τ pixels at the nearest point of the
 * patch's bounding volume; a camera inside the volume refines, down to the finest level. The set is
 * a function of each view's pose, field of view and viewport, the setting, the grounded bodies,
 * the height ranges of the patches baked so far and the patch budget alone: no history, no arrival
 * order and no hysteresis, the morph hiding a level change. The
 * selected patches form a restricted quadtree: no two neighbours, across face edges and at cube
 * corners included, differ by more than one level. Patches no view can see, outside every frustum
 * or below every horizon, are not selected.
 */

import type { Quaternion } from "../camera/pose";
import type { ViewSize } from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import { type PatchBounds, patchBounds } from "./bounds";
import { vertexSpacing } from "./cube";
import { frustumOf, horizonCone } from "./cull";
import {
  childKeys,
  cornerNeighbours,
  EDGES,
  edgeNeighbour,
  FACES,
  MAX_LEVEL,
  type PatchKey,
  ancestorIndex,
  patchKeyIndex,
  patchKeyString,
  rootKey,
} from "./patchKey";
import { finestPatchSizeM, type GroundContact, inForcedRegion } from "./grounded";
import { compareRequests } from "./priority";
import { type BodyFixedVec3, levelBoundM, levelHeightRangeM, type PlanetGeometry } from "./planet";
import { type ViewGeometry, viewExcess, viewGeometry } from "./viewGeometry";

/** A patch the selection asks to draw. */
export interface SelectedPatch {
  readonly key: PatchKey;
  readonly bounds: PatchBounds;
  /** Whether it lies in a grounded body's forced region, which the cache never evicts. */
  readonly forced: boolean;
  /**
   * Whether any view sees it. A forced patch no view sees is selected so that it is baked and kept
   * resident before contact (Design note 9), but it is not drawn and does not count against
   * `maxPatches`.
   */
  readonly seen: boolean;
}

/** A request to bake a patch, with its streaming priority (Design note 24). */
export interface PatchRequest {
  readonly key: PatchKey;
  /** Higher is sooner: the largest over the views of w_view × (ρ ÷ τ). */
  readonly priority: number;
  /** Whether it lies in a grounded body's forced region, which outranks everything. */
  readonly forced: boolean;
}

/** What selection returns: the patches to draw, by `patchKeyString`, and the demand to bake. */
export interface Selection {
  readonly patches: ReadonlyMap<string, SelectedPatch>;
  readonly demand: ReadonlyArray<PatchRequest>;
  /**
   * Whether `maxPatches` stopped a split that a view's tolerance asked for, so that the drawn
   * error is above τ somewhere: what drives `TERRAIN: DETAIL LIMITED` (the patch-demand ruling,
   * 2026-10-03).
   */
  readonly limited: boolean;
  /**
   * The weighted excess w_view × ρ ÷ τ of the split `maxPatches` refused, from which each view's
   * effective tolerance τ′ = τ × max(1, limitExcess ÷ w) follows; 0 where `limited` is false
   * (decision-r05-high-bound.md, F1).
   *
   * @remarks
   * It is dimensionless: the refused patch's largest, over the views that see it, of ρ ÷ the view's
   * own `tauPx` times the view's weight, the order the greedy refines in. It can be below 1 while
   * `limited` holds, where only a view of weight under 1 wanted the split, so τ′ is never
   * τ × limitExcess alone.
   *
   * Selection splits the patch with the largest weighted excess first, forced patches before all,
   * and stops at the first split the budget refuses. That patch stays a leaf, and every other leaf
   * still wanting a split has a weighted excess no larger. So every leaf a view of weight w and
   * tolerance τ sees has ρ ≤ τ′. Where `heightRanges` are given this holds for the baked leaves
   * only: an unbaked leaf is held back by the streaming gate, not the budget, and is drawn by its
   * baked ancestor (`TERRAIN: STREAMING`). It is 0 rather than 1 when not limited so that τ′ is τ
   * for a secondary view too, where 1 would give τ ÷ w. It is for F2's morph bands and T13.a's
   * descent record.
   */
  readonly limitExcess: number;
  /**
   * The baked patches selection reached and found hidden, depth first from face 0: the children a
   * split left out, which no view sees and no forced region reaches, and the split patches with
   * nothing selected beneath them (R05.T8, the high-bound ruling's F3).
   *
   * @remarks
   * Their own baked ranges may be what hides them, so the cache marks them as used, below the draw
   * pins. Evicted, such a patch takes its ancestor's looser range, is selected again the next
   * frame and is drawn by its parent, in place of its siblings, until it is baked again: the
   * approach's coarse patches thrashed this way (decision-r05-high-bound.md, reason 3c).
   */
  readonly hiddenBaked: ReadonlyArray<PatchKey>;
}

/**
 * The baked height ranges selection reads (the patch-demand ruling, 2026-10-03): the cache's
 * patches' own ranges, so that a patch's bounds tighten once it or an ancestor is baked.
 */
export interface HeightRangeLookup {
  /** The lowest and highest baked height of `key`, metres, or `undefined` if it is not baked. */
  heightRangeM(key: PatchKey): readonly [number, number] | undefined;
}

/** One view's part in selection (Design notes 7, 23 and 24). */
export interface ViewSelectionInput {
  /**
   * The camera in the body-fixed axes: its position from the body's centre, metres, and its
   * orientation. R02's `CameraPose` never rotates, so the caller rotates it into the body-fixed
   * axes with `rotateToBodyFixed` and the body's `Rotation3`.
   */
  readonly camera: { readonly positionM: BodyFixedVec3; readonly orientation: Quaternion };
  /** The horizontal field of view, rad, in (0, π). */
  readonly fovXRad: number;
  /** The presented size in device pixels (Design note 23), not the render resolution. */
  readonly viewport: ViewSize;
  /** The view's streaming weight: 1 for the primary view, 0.25 for a secondary (Design note 24). */
  readonly weight: number;
  /** The view's tolerance τ, pixels: the setting's, or a style's own (the wireframe's 4 px). */
  readonly tauPx: number;
}

/** Everything selection reads. */
export interface SelectionInput {
  readonly planet: PlanetGeometry;
  readonly views: ReadonlyArray<ViewSelectionInput>;
  readonly setting: QualitySetting;
  readonly grounded: ReadonlyArray<GroundContact>;
  /**
   * The baked height ranges, from the patch cache; absent, every patch takes its level's range.
   * Selection stays a pure function of its inputs: the ranges are one of them.
   */
  readonly heightRanges?: HeightRangeLookup;
  /**
   * The most patches some view sees to select; absent, no limit. Forced patches a view sees count
   * against it but are never refused, so the selection can exceed it by the forced region and the
   * balance it brings, and by the six roots; forced patches no view sees (`SelectedPatch.seen`
   * false) do not count. The terrain pass passes half its slots (provisional; T18 sets it).
   */
  readonly maxPatches?: number;
  /**
   * The skirt margin the height workers bake with, metres (the bake's `skirtM`): the bounds reach
   * down to the skirts' bottoms. Absent, 0.
   */
  readonly skirtMarginM?: number;
}

/**
 * The screen-space error ρ, pixels, of an error `boundM` metres at `distanceM` metres in `view`:
 * ε · W_px ÷ (2 d tan(fov_h ÷ 2)) (Design note 7); infinite at no distance.
 */
export function screenSpaceErrorPx(
  boundM: number,
  distanceM: number,
  view: ViewSelectionInput,
): number {
  if (!(distanceM > 0)) {
    return boundM > 0 ? Infinity : 0;
  }
  return (boundM * view.viewport.widthPx) / (2 * distanceM * Math.tan(view.fovXRad / 2));
}

/**
 * How much larger than half its largest leg squared the smallest enclosing disc of a mesh triangle
 * can be, on the quadratic-warp cube sphere split on the (0, 0)–(1, 1) diagonal: r_n² ≤ 1.023 h_n²
 * ÷ 2, worst at (s, t) ≈ (0.234, 0.766) (measured, R05.T7; `select.test.ts` pins it), rounded up
 * to cover the height's stretch 1 + H ÷ ρ for |H| up to about 30 km.
 */
export const SAGITTA_FACTOR = 1.03;

/**
 * The most a level's flat triangles sag below the datum's curve between their vertices, metres:
 * K h_n² ÷ (4 ρ_min), for the level's largest vertex spacing h_n, the spheroid's smallest radius of
 * curvature ρ_min = c² ÷ a and K = {@link SAGITTA_FACTOR}: linear interpolation's error is at most
 * r² ÷ (2 ρ_min) for r the radius of the smallest disc holding the triangle (Waldron 1998, SIAM J.
 * Numer. Anal. 35(3) 1191–1200, Thm 4.1 eq. (4.5), doi:10.1137/S0036142996313154), and
 * r_n² ≤ 1.023 h_n² ÷ 2 on the quadratic-warp cube sphere's split. ε_n, a bound on heights above
 * the datum, leaves it out.
 *
 * @remarks
 * One function, so that a change of factor is one line (the science-checker's ruling relayed by the
 * orchestrator, 2026-10-03). It needs c ≤ a, which `planetGeometry` holds.
 */
export function chordSagittaM(planet: PlanetGeometry, level: number): number {
  const a = planet.figure.equatorialRadiusM;
  const c = planet.figure.polarRadiusM;
  const spacing = vertexSpacing(a, level).maxM;
  return (SAGITTA_FACTOR * spacing * spacing) / (4 * ((c * c) / a));
}

/**
 * The error selection charges a level, metres: the level bound ε_n against the finest level
 * (Design note 15) plus {@link chordSagittaM}; nothing at the finest level.
 *
 * @remarks
 * The sagitta is what a zero-height spheroid with no level table (R07's `mesh` regime) refines on;
 * on the test planet it is under a sixth of ε_n at level 0 and under a tenth from level 1.
 */
export function selectionErrorM(planet: PlanetGeometry, level: number): number {
  if (level >= planet.finestLevel) {
    return 0;
  }
  return levelBoundM(planet, level) + chordSagittaM(planet, level);
}

/** One view, prepared once a call: its frustum, its horizon and its error scale. */
interface PreparedView {
  readonly input: ViewSelectionInput;
  /** Its frustum, horizon and error scale as plain numbers. */
  readonly geometry: ViewGeometry;
}

/** A patch's nearest baked ancestor, or itself: its level and baked heights, metres. */
export interface BakedRange {
  readonly level: number;
  readonly lowM: number;
  readonly highM: number;
}

/** A node of the traversal: a patch and what the views make of it. */
interface TraversalNode {
  readonly key: PatchKey;
  readonly bounds: PatchBounds;
  /**
   * Whether any view sees the patch, or it is in a forced region, which is selected whether or not
   * a view sees it so that it is resident before contact (Design note 9).
   */
  readonly visible: boolean;
  /** The views that see it, a bit a view in input order. */
  readonly seenBy: number;
  /** The views that see it and find ρ > τ: the views that want it split, a bit a view. */
  readonly wantedBy: number;
  /** The largest ρ ÷ τ over the views that see it; 0 where none does. */
  readonly excess: number;
  /** The largest w_view × ρ ÷ τ over the views that see it: the refinement and streaming order. */
  readonly weighted: number;
  /** Whether its bounding box comes within a grounded body's forced region (Design note 9). */
  readonly forced: boolean;
  /** Its nearest baked ancestor or itself, which its height range comes from; `null` if none. */
  readonly baked: BakedRange | null;
  /** Whether it is baked itself. */
  readonly resident: boolean;
  /** The node it was split from, `null` for a root. */
  readonly parent: TraversalNode | null;
}

/** Bounds already computed, per planet and level, with the height range they were built for. */
const boundsMemo = new WeakMap<
  PlanetGeometry,
  Map<
    number,
    {
      readonly lowM: number;
      readonly highM: number;
      /** The baked range it was built from, `null` for the level's own. */
      readonly baked: BakedRange | null;
      readonly bounds: PatchBounds;
    }
  >[]
>();

/** The most bounds kept a level per planet; past it the older half is dropped. */
const BOUNDS_MEMO_LIMIT = 1 << 15;

function boundsOf(
  planet: PlanetGeometry,
  key: PatchKey,
  baked: BakedRange | null,
  skirtMarginM: number,
): PatchBounds {
  let levels = boundsMemo.get(planet);
  if (levels === undefined) {
    levels = Array.from({ length: MAX_LEVEL + 1 }, () => new Map());
    boundsMemo.set(planet, levels);
  }
  const memo = levels[key.level];
  if (memo === undefined) {
    throw new Error(`level ${key.level} is not a quadtree level`);
  }
  const index = patchKeyIndex(key);
  const known = memo.get(index);
  // The range only when the memo cannot answer with the level's own (nothing baked above it).
  if (known !== undefined && baked === null && known.baked === null) {
    return known.bounds;
  }
  const range = inheritedHeightRangeM(planet, key, baked, skirtMarginM);
  if (known !== undefined && known.lowM === range[0] && known.highM === range[1]) {
    return known.bounds;
  }
  if (memo.size >= BOUNDS_MEMO_LIMIT) {
    // Maps keep insertion order: drop the older half, so a moving camera never pays a cold start.
    let drop = memo.size / 2;
    for (const k of memo.keys()) {
      if (drop <= 0) {
        break;
      }
      memo.delete(k);
      drop -= 1;
    }
  }
  const bounds = patchBounds(planet, key, range);
  memo.set(index, { lowM: range[0], highM: range[1], baked, bounds });
  return bounds;
}

/** One `f32` step's relative size: a baked height rounded to `f32` is within this of its value. */
const F32_RELATIVE_STEP = 2 ** -23;

/** One call's state. */
interface Traversal {
  readonly planet: PlanetGeometry;
  readonly views: ReadonlyArray<PreparedView>;
  /** {@link selectionErrorM} per level, metres. */
  readonly errorM: ReadonlyArray<number>;
  readonly grounded: ReadonlyArray<GroundContact>;
  readonly heightRanges: HeightRangeLookup | null;
  /** The bake's skirt margin, metres. */
  readonly skirtMarginM: number;
  /** The finest level's patch edge, metres: the forced region's unit. */
  readonly patchSizeM: number;
}

/**
 * The height range a patch can reach, metres (the patch-demand ruling, 2026-10-03, item 4a): from
 * its nearest baked ancestor A at level m (or itself), A's baked range rounded outward from `f32`,
 * widened by ε_m (S_m lies within A's range and |S_f − S_m| ≤ ε_m) and ε_{n−1} (a vertex and its
 * morph target lie within ε_{n−1} of S_f), and below by the skirt (ε_n and an `f32` step), kept
 * within the level's range; the level's range itself with nothing baked.
 */
export function inheritedHeightRangeM(
  planet: PlanetGeometry,
  key: PatchKey,
  baked: BakedRange | null,
  skirtMarginM = 0,
): readonly [number, number] {
  const [levelLow, levelHigh] = levelHeightRangeM(planet, key.level);
  if (baked === null) {
    return [levelLow, levelHigh];
  }
  const n = key.level;
  const widen = levelBoundM(planet, baked.level) + levelBoundM(planet, Math.max(0, n - 1));
  // The largest |h| the patch's own heights can reach, for the f32 steps: the bake's own
  // `f32_step(largest_h)` in its skirt depth, and the rounding of the baked range.
  const largest = Math.max(Math.abs(baked.lowM), Math.abs(baked.highM)) + widen;
  const step = largest * F32_RELATIVE_STEP;
  // The bake's skirt depth: ε_n, an f32 step of the largest height and the caller's margin.
  const skirt = levelBoundM(planet, n) + step + skirtMarginM;
  const low = baked.lowM - step - widen - skirt;
  const high = baked.highM + step + widen;
  return [Math.max(levelLow, low), Math.min(levelHigh, high)];
}

function nodeOf(t: Traversal, key: PatchKey, parent: TraversalNode | null): TraversalNode {
  const own = t.heightRanges?.heightRangeM(key);
  const baked: BakedRange | null =
    own === undefined ? (parent?.baked ?? null) : { level: key.level, lowM: own[0], highM: own[1] };
  const bounds = boundsOf(t.planet, key, baked, t.skirtMarginM);
  const errorM = t.errorM[key.level] ?? 0;
  let seenBy = 0;
  let wantedBy = 0;
  let excess = 0;
  let weighted = 0;
  for (let n = 0; n < t.views.length; n += 1) {
    const v = t.views[n];
    if (v === undefined) {
      continue;
    }
    const e = excessOf(v, bounds, errorM);
    if (e === null) {
      continue;
    }
    seenBy |= 1 << n;
    if (e > 1) {
      wantedBy |= 1 << n;
    }
    excess = Math.max(excess, e);
    weighted = Math.max(weighted, v.input.weight * e);
  }
  const forced = inForcedRegion(bounds, t.grounded, t.patchSizeM);
  const node: TraversalNode = {
    key,
    bounds,
    visible: seenBy !== 0 || forced,
    seenBy,
    wantedBy,
    excess,
    weighted,
    forced,
    baked,
    resident: own !== undefined,
    parent,
  };
  return node;
}

/** The most views selection takes at once: one bit each in a node's view masks. */
export const MAX_SELECTION_VIEWS = 31;

/**
 * A view's ρ ÷ τ for a patch of bounds `bounds` at error `errorM`, or `null` where the view cannot
 * see it.
 */
function excessOf(v: PreparedView, bounds: PatchBounds, errorM: number): number | null {
  const e = viewExcess(v.geometry, bounds, errorM);
  return e < 0 ? null : e;
}

/**
 * A request's priority (Design note 24): the largest over the views that want it of
 * w_view × ρ ÷ τ of `standIn`, the patch drawn in its place. The views that want it are those that
 * see the request and want `standIn` split; where none does (a split the 2:1 balance made), those
 * that see both.
 */
function requestPriority(t: Traversal, target: TraversalNode, standIn: TraversalNode): number {
  let views = standIn.wantedBy & target.seenBy;
  if (views === 0) {
    views = standIn.seenBy & target.seenBy;
  }
  const errorM = t.errorM[standIn.key.level] ?? 0;
  let priority = 0;
  for (let n = 0; n < t.views.length; n += 1) {
    const v = t.views[n];
    if (v === undefined || (views & (1 << n)) === 0) {
      continue;
    }
    const e = excessOf(v, standIn.bounds, errorM);
    if (e !== null) {
      priority = Math.max(priority, v.input.weight * e);
    }
  }
  return priority;
}

/**
 * Whether a selected patch should be split: it is in a forced region, or a view finds ρ > τ and,
 * where baked ranges are given, it is baked itself (the streaming gate).
 *
 * @remarks
 * The gate is the cure for selection chasing its own loose bounds (R05.T13.a's probe, 2026-10-03):
 * an unbaked patch's bounds take the level's whole height range, so it looks far worse than its
 * baked neighbours, and without the gate the patch budget is spent on refining under loose bounds,
 * the refined patches tighten once baked, the budget moves to the next loose region, and the
 * selection jumps from frame to frame. With it, selection reaches at most one level below what is
 * baked, every split decided on a baked patch's own range, and descends as bakes land, which the
 * breadth-first demand already orders; the frontier below is drawn by its baked parent, which
 * `TERRAIN: STREAMING` reports. The bounds stay true bounds: the gate only stops a refinement, so
 * the drawn error is never claimed to be smaller than it is. Forced regions are not gated.
 */
function wantsRefining(t: Traversal, node: TraversalNode): boolean {
  if (!node.visible || node.key.level >= t.planet.finestLevel) {
    return false;
  }
  if (node.forced) {
    return true;
  }
  return node.excess > 1 && (t.heightRanges === null || node.resident);
}

/** Orders candidates for splitting: forced first, then the larger weighted error, then the key. */
function before(a: TraversalNode, b: TraversalNode): boolean {
  if (a.forced !== b.forced) {
    return a.forced;
  }
  if (a.weighted !== b.weighted) {
    return a.weighted > b.weighted;
  }
  if (a.key.level !== b.key.level) {
    return a.key.level < b.key.level;
  }
  return patchKeyIndex(a.key) < patchKeyIndex(b.key);
}

/** A binary max-heap of traversal nodes by {@link before}. */
class CandidateHeap {
  private readonly items: TraversalNode[] = [];

  get size(): number {
    return this.items.length;
  }

  push(node: TraversalNode): void {
    const items = this.items;
    items.push(node);
    let at = items.length - 1;
    while (at > 0) {
      const up = (at - 1) >> 1;
      const parent = items[up];
      if (parent === undefined || !before(node, parent)) {
        break;
      }
      items[at] = parent;
      at = up;
    }
    items[at] = node;
  }

  pop(): TraversalNode | undefined {
    const items = this.items;
    const top = items[0];
    const last = items.pop();
    if (top === undefined || last === undefined || items.length === 0) {
      return top;
    }
    let at = 0;
    for (;;) {
      const left = 2 * at + 1;
      if (left >= items.length) {
        break;
      }
      const right = left + 1;
      const leftItem = items[left];
      const rightItem = items[right];
      let child = left;
      let childItem = leftItem;
      if (rightItem !== undefined && leftItem !== undefined && before(rightItem, leftItem)) {
        child = right;
        childItem = rightItem;
      }
      if (childItem === undefined || !before(childItem, last)) {
        break;
      }
      items[at] = childItem;
      at = child;
    }
    items[at] = last;
    return top;
  }
}

/**
 * Selects the patches to draw for every view at once (Design note 7, with the patch-demand ruling
 * of 2026-10-03): from the six roots, the patch with the largest weighted error is split first,
 * wherever a view that sees it finds ρ > τ or a grounded body's forced region reaches it, each
 * split balanced at once into a restricted quadtree, until nothing wants splitting or the next
 * split would take the selection past `maxPatches`, which then sets `limited` and reports that
 * split's weighted excess as `limitExcess`. Forced splits are never refused, and a forced region is
 * selected whether or not a view sees it.
 *
 * @throws RangeError for more than {@link MAX_SELECTION_VIEWS} views.
 */
export function selectPatches(input: SelectionInput): Selection {
  const { planet } = input;
  if (input.views.length > MAX_SELECTION_VIEWS) {
    throw new RangeError(
      `selection takes at most ${MAX_SELECTION_VIEWS} views, got ${input.views.length}`,
    );
  }
  const t: Traversal = {
    planet,
    views: input.views.map((v) => ({
      input: v,
      geometry: viewGeometry(
        frustumOf({ orientation: v.camera.orientation, fovXRad: v.fovXRad, viewport: v.viewport }),
        v.camera.positionM,
        horizonCone(planet, v.camera.positionM).occluderRadiusM,
        v.viewport.widthPx / (2 * Math.tan(v.fovXRad / 2) * v.tauPx),
      ),
    })),
    errorM: Array.from({ length: MAX_LEVEL + 1 }, (_, level) => selectionErrorM(planet, level)),
    grounded: input.grounded,
    heightRanges: input.heightRanges ?? null,
    skirtMarginM: input.skirtMarginM ?? 0,
    patchSizeM: finestPatchSizeM(planet),
  };
  const maxPatches = input.maxPatches ?? Infinity;
  // A forced patch no view sees is selected, to be kept resident, but neither drawn nor counted.
  const tree = new PatchLeafSet<TraversalNode>((node) => node.seenBy !== 0);
  const heap = new CandidateHeap();
  const offer = (node: TraversalNode): void => {
    if (wantsRefining(t, node)) {
      heap.push(node);
    }
  };
  const childOf = (child: PatchKey, parent: TraversalNode): TraversalNode | null => {
    const node = nodeOf(t, child, parent);
    return node.visible ? node : null;
  };
  for (const face of FACES) {
    const root = nodeOf(t, rootKey(face), null);
    if (root.visible) {
      tree.addRoot(root.key, root);
      offer(root);
    }
  }
  let limited = false;
  let limitExcess = 0;
  for (let node = heap.pop(); node !== undefined; node = heap.pop()) {
    if (!tree.isLeaf(node.key)) {
      continue;
    }
    tree.begin();
    const added = tree.splitBalanced(node.key, childOf);
    if (!node.forced && tree.size > maxPatches) {
      tree.rollback();
      limited = true;
      limitExcess = node.weighted;
      break;
    }
    tree.commit();
    for (const leaf of added) {
      offer(leaf);
    }
  }
  const patches = new Map<string, SelectedPatch>();
  for (const node of tree.values()) {
    patches.set(patchKeyString(node.key), {
      key: node.key,
      bounds: node.bounds,
      forced: node.forced && node.key.level === planet.finestLevel,
      seen: node.seenBy !== 0,
    });
  }
  return {
    patches,
    demand: demandOf(t, tree),
    limited,
    limitExcess,
    hiddenBaked: hiddenBakedOf(t, tree),
  };
}

/** The baked patches among the tree's bare keys ({@link Selection.hiddenBaked}). */
function hiddenBakedOf(t: Traversal, tree: PatchLeafSet<TraversalNode>): PatchKey[] {
  const ranges = t.heightRanges;
  if (ranges === null) {
    return [];
  }
  return tree.bareKeys().filter((key) => ranges.heightRangeM(key) !== undefined);
}

/**
 * The patches to bake, by priority (Design note 24, with the ruling's breadth-first rule): for each
 * selected patch not yet baked, the shallowest unbaked patch on its way down whose parent is baked
 * (or a root), each once, at the largest over the views of w_view × ρ ÷ τ of the patch drawn in its
 * place, its parent; a forced patch is requested whatever its ancestors, and outranks everything.
 * Ordered forced first, then by priority, ties by `patchKeyString`.
 */
function demandOf(t: Traversal, tree: PatchLeafSet<TraversalNode>): PatchRequest[] {
  const requests = new Map<TraversalNode, PatchRequest>();
  for (const leaf of tree.values()) {
    if (leaf.resident) {
      continue;
    }
    const forced = leaf.forced && leaf.key.level === t.planet.finestLevel;
    let target: TraversalNode = leaf;
    if (!forced) {
      // Up to the shallowest unbaked ancestor, by number; an ancestor already requested ends it.
      for (let node = leaf.parent; node !== null; node = node.parent) {
        if (node.resident) {
          break;
        }
        target = node;
        if (requests.has(node)) {
          break;
        }
      }
    }
    if (requests.has(target)) {
      continue;
    }
    const standIn = target.parent ?? undefined;
    requests.set(target, {
      key: target.key,
      // A forced request outranks every other, so among forced ones its own error orders it.
      priority:
        forced || standIn === undefined ? target.weighted : requestPriority(t, target, standIn),
      forced,
    });
  }
  return [...requests.values()].toSorted(compareRequests);
}

/** The patches of a key's level sharing an edge or a corner with it. */
function neighbours(key: PatchKey): PatchKey[] {
  const last = 2 ** key.level - 1;
  if (key.i > 0 && key.j > 0 && key.i < last && key.j < last) {
    // Away from the face's edges every neighbour is on the same face: no fold to compute.
    const { face, level, i, j } = key;
    return [
      { face, level, i: i - 1, j },
      { face, level, i: i + 1, j },
      { face, level, i, j: j - 1 },
      { face, level, i, j: j + 1 },
      { face, level, i: i - 1, j: j - 1 },
      { face, level, i: i + 1, j: j - 1 },
      { face, level, i: i + 1, j: j + 1 },
      { face, level, i: i - 1, j: j + 1 },
    ];
  }
  const around = EDGES.map((e) => edgeNeighbour(key, e));
  for (const corner of cornerNeighbours(key)) {
    if (corner !== null) {
      around.push(corner);
    }
  }
  return around;
}

/** A node of {@link PatchLeafSet}'s trees: a leaf, or split into the children kept. */
interface TreeNode<T> {
  readonly key: PatchKey;
  readonly value: T;
  /** The four children in `childKeys`' order, `null` where left out; `null` for a leaf. */
  children: (TreeNode<T> | null)[] | null;
}

/**
 * The leaves of a cut of the six faces' quadtrees, with the patches split above them, kept a
 * restricted quadtree as it is split, and able to undo a split and the balance it brought.
 *
 * @remarks
 * Held as trees of nodes from the six roots, with every node also in a map per level by
 * {@link patchKeyIndex}, so that finding the leaf over a neighbour's cell is usually one lookup at
 * the parent's level, with no key or string built. The leaves come out depth first from
 * face 0, in `childKeys`' order, which is deterministic. Exported for its tests.
 */
export class PatchLeafSet<T> {
  private readonly roots: (TreeNode<T> | null)[] = [null, null, null, null, null, null];
  /** Every node, leaf or split, per level by {@link patchKeyIndex}: one lookup finds a cell's node. */
  private readonly byLevel: Map<number, TreeNode<T>>[] = Array.from(
    { length: MAX_LEVEL + 1 },
    () => new Map<number, TreeNode<T>>(),
  );
  private journal: TreeNode<T>[] | null = null;
  private count = 0;
  private readonly counts: (value: T) => boolean;

  /**
   * @param counts - Whether a leaf counts towards {@link PatchLeafSet.size}: every leaf by default;
   *   selection leaves out the forced patches no view sees, which are kept resident but not drawn.
   */
  constructor(counts: (value: T) => boolean = () => true) {
    this.counts = counts;
  }

  /** How many leaves there are that count. */
  get size(): number {
    return this.count;
  }

  /**
   * Adds a face's root as a leaf.
   *
   * @throws Error for a key that is not a root, or a root already present.
   */
  addRoot(key: PatchKey, value: T): void {
    if (key.level !== 0 || this.roots[key.face] !== null) {
      throw new Error(`patch ${patchKeyString(key)} is not a root to add`);
    }
    const node: TreeNode<T> = { key, value, children: null };
    this.roots[key.face] = node;
    this.byLevel[0]?.set(patchKeyIndex(key), node);
    if (this.counts(value)) {
      this.count += 1;
    }
  }

  /** The node at `key`, or `null` where the trees do not reach it. */
  private find(key: PatchKey): TreeNode<T> | null {
    return this.byLevel[key.level]?.get(patchKeyIndex(key)) ?? null;
  }

  /** Whether `key` is a leaf. */
  isLeaf(key: PatchKey): boolean {
    const node = this.find(key);
    return node !== null && node.children === null;
  }

  /** The leaves' values, depth first from face 0. */
  *values(): IterableIterator<T> {
    const stack: TreeNode<T>[] = [];
    for (let face = 5; face >= 0; face -= 1) {
      const root = this.roots[face];
      if (root !== null && root !== undefined) {
        stack.push(root);
      }
    }
    for (let node = stack.pop(); node !== undefined; node = stack.pop()) {
      if (node.children === null) {
        yield node.value;
        continue;
      }
      for (let n = 3; n >= 0; n -= 1) {
        const child = node.children[n];
        if (child !== null && child !== undefined) {
          stack.push(child);
        }
      }
    }
  }

  /**
   * The keys the trees reach that hold no leaf, depth first from face 0: each child a split left
   * out, and each split node with no leaf beneath it, after its children.
   */
  bareKeys(): PatchKey[] {
    const bare: PatchKey[] = [];
    for (const root of this.roots) {
      if (root !== null) {
        this.collectBare(root, bare);
      }
    }
    return bare;
  }

  /** Adds the bare keys at and under `node` to `bare`; returns whether a leaf lies at or under it. */
  private collectBare(node: TreeNode<T>, bare: PatchKey[]): boolean {
    const children = node.children;
    if (children === null) {
      return true;
    }
    let keys: readonly PatchKey[] | null = null;
    let leaf = false;
    for (let n = 0; n < 4; n += 1) {
      const child = children[n] ?? null;
      if (child === null) {
        keys ??= childKeys(node.key);
        const key = keys[n];
        if (key !== undefined) {
          bare.push(key);
        }
      } else if (this.collectBare(child, bare)) {
        leaf = true;
      }
    }
    if (!leaf) {
      bare.push(node.key);
    }
    return leaf;
  }

  /** Starts recording splits, so that {@link rollback} can undo them. */
  begin(): void {
    this.journal = [];
  }

  /** Keeps the splits since {@link begin}. */
  commit(): void {
    this.journal = null;
  }

  /** Undoes every split since {@link begin}, latest first. */
  rollback(): void {
    const journal = this.journal ?? [];
    for (let n = journal.length - 1; n >= 0; n -= 1) {
      const node = journal[n];
      if (node === undefined || node.children === null) {
        continue;
      }
      // Undone latest first, so the children are leaves again here.
      for (const child of node.children) {
        if (child !== null) {
          this.byLevel[child.key.level]?.delete(patchKeyIndex(child.key));
          if (this.counts(child.value)) {
            this.count -= 1;
          }
        }
      }
      node.children = null;
      if (this.counts(node.value)) {
        this.count += 1;
      }
    }
    this.journal = null;
  }

  private split(node: TreeNode<T>, makeChild: (child: PatchKey, parent: T) => T | null): void {
    if (node.children !== null) {
      return;
    }
    if (this.counts(node.value)) {
      this.count -= 1;
    }
    const children: (TreeNode<T> | null)[] = [null, null, null, null];
    const keys = childKeys(node.key);
    for (let n = 0; n < 4; n += 1) {
      const child = keys[n];
      const value = child === undefined ? null : makeChild(child, node.value);
      if (child !== undefined && value !== null) {
        const made: TreeNode<T> = { key: child, value, children: null };
        children[n] = made;
        this.byLevel[child.level]?.set(patchKeyIndex(child), made);
        if (this.counts(value)) {
          this.count += 1;
        }
      }
    }
    node.children = children;
    this.journal?.push(node);
  }

  /**
   * Splits leaf `key` into the children `makeChild` keeps, then splits every leaf two or more levels
   * coarser than an edge or corner neighbour of a new leaf, across face edges and at cube corners,
   * until the cut is a restricted quadtree again. Returns the leaves added.
   *
   * @param makeChild - The value of a child of the patch whose value is `parent`, or `null` to
   *   leave it out (a patch no view sees).
   */
  splitBalanced(key: PatchKey, makeChild: (child: PatchKey, parent: T) => T | null): T[] {
    const first = this.find(key);
    if (first === null || first.children !== null) {
      return [];
    }
    const work: TreeNode<T>[] = [];
    const added: TreeNode<T>[] = [];
    const splitAndQueue = (node: TreeNode<T>): void => {
      this.split(node, makeChild);
      for (const child of node.children ?? []) {
        if (child !== null) {
          added.push(child);
          work.push(child);
        }
      }
    };
    splitAndQueue(first);
    for (let leaf = work.pop(); leaf !== undefined; leaf = work.pop()) {
      if (leaf.children !== null) {
        continue;
      }
      for (const n of neighbours(leaf.key)) {
        const coarse = this.coarserLeafNode(n);
        if (coarse !== null && coarse.key.level < leaf.key.level - 1) {
          splitAndQueue(coarse);
          // The split may still leave a leaf too coarse beside this one: look again.
          work.push(leaf);
          break;
        }
      }
    }
    const leaves: T[] = [];
    for (const node of added) {
      if (node.children === null) {
        leaves.push(node.value);
      }
    }
    return leaves;
  }

  /**
   * The leaf covering `key`'s area at a coarser level than `key`, or `null` where none does: where
   * the area is split further or not selected.
   */
  coarserLeaf(key: PatchKey): PatchKey | null {
    return this.coarserLeafNode(key)?.key ?? null;
  }

  private coarserLeafNode(key: PatchKey): TreeNode<T> | null {
    // Up from the parent's level: usually the first lookup finds a node, and a split one or a leaf
    // one level up is no violation; only a gap walks further.
    for (let level = key.level - 1; level >= 0; level -= 1) {
      const node = this.byLevel[level]?.get(ancestorIndex(key, level));
      if (node !== undefined) {
        return node.children === null ? node : null;
      }
    }
    return null;
  }
}
