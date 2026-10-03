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
import { NEAR_PLANE_M } from "../camera/projection";
import type { ViewSize } from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import { distanceToBoxM, type PatchBounds, patchBounds, relativeBounds } from "./bounds";
import { vertexSpacing } from "./cube";
import {
  aboveHorizon,
  type Frustum,
  frustumOf,
  type HorizonCone,
  horizonCone,
  inFrustum,
} from "./cull";
import {
  childKeys,
  cornerNeighbours,
  EDGES,
  edgeNeighbour,
  FACES,
  MAX_LEVEL,
  parentKey,
  type PatchKey,
  patchKeyString,
  rootKey,
} from "./patchKey";
import { finestPatchSizeM, type GroundContact, inForcedRegion } from "./grounded";
import { compareRequests } from "./priority";
import { type BodyFixedVec3, levelBoundM, levelHeightRangeM, type PlanetGeometry } from "./planet";

/** A patch the selection asks to draw. */
export interface SelectedPatch {
  readonly key: PatchKey;
  readonly bounds: PatchBounds;
  /** Whether it lies in a grounded body's forced region, which the cache never evicts. */
  readonly forced: boolean;
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
   * The most patches to select; absent, no limit. Forced patches count against it but are never
   * refused, so the selection can exceed it by the forced region and the balance it brings, and by
   * the six roots. The terrain pass passes half its slots (provisional; T18 sets it).
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
  readonly frustum: Frustum;
  readonly horizon: HorizonCone;
  /** W_px ÷ (2 tan(fov_h ÷ 2) τ): ρ ÷ τ is this times the error over the distance. */
  readonly excessPerMetre: number;
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
  readonly keyString: string;
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
}

/** A patch's index within its level: face · 2⁴⁸ + i · 2²⁴ + j, an exact integer below 2⁵¹. */
function levelIndex(key: PatchKey): number {
  return key.face * 2 ** 48 + key.i * 2 ** 24 + key.j;
}

/** Bounds already computed, per planet and level, with the height range they were built for. */
const boundsMemo = new WeakMap<
  PlanetGeometry,
  Map<number, { readonly lowM: number; readonly highM: number; readonly bounds: PatchBounds }>[]
>();

/** The most bounds kept a level per planet; past it the older half is dropped. */
const BOUNDS_MEMO_LIMIT = 1 << 15;

function boundsOf(
  planet: PlanetGeometry,
  key: PatchKey,
  range: readonly [number, number],
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
  const index = levelIndex(key);
  const known = memo.get(index);
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
  memo.set(index, { lowM: range[0], highM: range[1], bounds });
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
  /** Every node made this call, per level by {@link levelIndex}. */
  readonly nodes: ReadonlyArray<Map<number, TraversalNode>>;
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
  const level = t.nodes[key.level];
  const index = levelIndex(key);
  const known = level?.get(index);
  if (known !== undefined) {
    return known;
  }
  const keyString = patchKeyString(key);
  const own = t.heightRanges?.heightRangeM(key);
  const baked: BakedRange | null =
    own === undefined ? (parent?.baked ?? null) : { level: key.level, lowM: own[0], highM: own[1] };
  const bounds = boundsOf(
    t.planet,
    key,
    inheritedHeightRangeM(t.planet, key, baked, t.skirtMarginM),
  );
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
    const e = viewExcess(v, bounds, errorM);
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
    keyString,
    bounds,
    visible: seenBy !== 0 || forced,
    seenBy,
    wantedBy,
    excess,
    weighted,
    forced,
    baked,
    resident: own !== undefined,
  };
  level?.set(index, node);
  return node;
}

/** The most views selection takes at once: one bit each in a node's view masks. */
export const MAX_SELECTION_VIEWS = 31;

/**
 * A view's ρ ÷ τ for a patch of bounds `bounds` at error `errorM`, or `null` where the view cannot
 * see it.
 */
function viewExcess(v: PreparedView, bounds: PatchBounds, errorM: number): number | null {
  const rel = relativeBounds(bounds, v.input.camera.positionM);
  if (!inFrustum(rel, v.frustum) || !aboveHorizon(rel, v.horizon)) {
    return null;
  }
  // No nearer than the near plane: a camera inside a volume has the error of one 0.1 m away, so
  // the excess stays finite and a secondary view's weight still ranks it (Design note 24).
  const d = Math.max(distanceToBoxM(rel), NEAR_PLANE_M);
  return (errorM * v.excessPerMetre) / d;
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
    const e = viewExcess(v, standIn.bounds, errorM);
    if (e !== null) {
      priority = Math.max(priority, v.input.weight * e);
    }
  }
  return priority;
}

/** Whether a selected patch should be split: it is in a forced region, or a view finds ρ > τ. */
function wantsRefining(planet: PlanetGeometry, node: TraversalNode): boolean {
  return node.visible && node.key.level < planet.finestLevel && (node.forced || node.excess > 1);
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
  return levelIndex(a.key) < levelIndex(b.key);
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
 * split would take the selection past `maxPatches`, which then sets `limited`. Forced splits are
 * never refused, and a forced region is selected whether or not a view sees it.
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
      frustum: frustumOf({
        orientation: v.camera.orientation,
        fovXRad: v.fovXRad,
        viewport: v.viewport,
      }),
      horizon: horizonCone(planet, v.camera.positionM),
      excessPerMetre: v.viewport.widthPx / (2 * Math.tan(v.fovXRad / 2) * v.tauPx),
    })),
    errorM: Array.from({ length: MAX_LEVEL + 1 }, (_, level) => selectionErrorM(planet, level)),
    grounded: input.grounded,
    heightRanges: input.heightRanges ?? null,
    skirtMarginM: input.skirtMarginM ?? 0,
    patchSizeM: finestPatchSizeM(planet),
    nodes: Array.from({ length: MAX_LEVEL + 1 }, () => new Map()),
  };
  const maxPatches = input.maxPatches ?? Infinity;
  const tree = new PatchLeafSet<TraversalNode>();
  const heap = new CandidateHeap();
  const offer = (node: TraversalNode): void => {
    if (wantsRefining(planet, node)) {
      heap.push(node);
    }
  };
  const childOf = (child: PatchKey, parent: PatchKey): TraversalNode | null => {
    const p = t.nodes[parent.level]?.get(levelIndex(parent)) ?? null;
    const node = nodeOf(t, child, p);
    return node.visible ? node : null;
  };
  for (const face of FACES) {
    const root = nodeOf(t, rootKey(face), null);
    if (root.visible) {
      tree.add(root.key, root);
      offer(root);
    }
  }
  let limited = false;
  for (let node = heap.pop(); node !== undefined; node = heap.pop()) {
    if (!tree.isLeaf(node.key)) {
      continue;
    }
    tree.begin();
    const added = tree.splitBalanced(node.key, childOf);
    if (!node.forced && tree.size > maxPatches) {
      tree.rollback();
      limited = true;
      break;
    }
    tree.commit();
    for (const leaf of added) {
      offer(leaf);
    }
  }
  const patches = new Map<string, SelectedPatch>();
  for (const node of tree.values()) {
    patches.set(node.keyString, {
      key: node.key,
      bounds: node.bounds,
      forced: node.forced && node.key.level === planet.finestLevel,
    });
  }
  return { patches, demand: demandOf(t, tree), limited };
}

/**
 * The patches to bake, by priority (Design note 24, with the ruling's breadth-first rule): for each
 * selected patch not yet baked, the shallowest unbaked patch on its way down whose parent is baked
 * (or a root), each once, at the largest over the views of w_view × ρ ÷ τ of the patch drawn in its
 * place, its parent; a forced patch is requested whatever its ancestors, and outranks everything.
 * Ordered forced first, then by priority, ties by `patchKeyString`.
 */
function demandOf(t: Traversal, tree: PatchLeafSet<TraversalNode>): PatchRequest[] {
  const requests = new Map<string, PatchRequest>();
  const nodeAt = (key: PatchKey): TraversalNode | undefined =>
    t.nodes[key.level]?.get(levelIndex(key));
  for (const leaf of tree.values()) {
    if (leaf.resident) {
      continue;
    }
    const forced = leaf.forced && leaf.key.level === t.planet.finestLevel;
    let target: TraversalNode = leaf;
    if (!forced) {
      for (let up = parentKey(leaf.key); up !== null; up = parentKey(up)) {
        const node = nodeAt(up);
        if (node === undefined || node.resident) {
          break;
        }
        target = node;
      }
    }
    if (requests.has(target.keyString)) {
      continue;
    }
    const parent = parentKey(target.key);
    const standIn = parent === null ? undefined : nodeAt(parent);
    requests.set(target.keyString, {
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

/**
 * The leaves of a cut of the six faces' quadtrees, with the patches split above them, kept a
 * restricted quadtree as it is split, and able to undo a split and the balance it brought.
 *
 * @remarks
 * Keys are numeric within a level ({@link levelIndex}); the leaves come out in the order they were
 * added, which a deterministic sequence of splits makes deterministic. Exported for its tests.
 */
export class PatchLeafSet<T> {
  private readonly leaves: Map<number, { readonly key: PatchKey; readonly value: T }>[] =
    Array.from({ length: MAX_LEVEL + 1 }, () => new Map());
  private readonly internal: Set<number>[] = Array.from({ length: MAX_LEVEL + 1 }, () => new Set());
  private journal:
    { readonly key: PatchKey; readonly value: T; readonly children: PatchKey[] }[] | null = null;
  private count = 0;

  /** How many leaves there are. */
  get size(): number {
    return this.count;
  }

  /** Adds a leaf. */
  add(key: PatchKey, value: T): void {
    const level = this.levelOf(key);
    if (!level.has(levelIndex(key))) {
      this.count += 1;
    }
    level.set(levelIndex(key), { key, value });
  }

  /** Whether `key` is a leaf. */
  isLeaf(key: PatchKey): boolean {
    return this.levelOf(key).has(levelIndex(key));
  }

  /** Marks `key` as split, above the leaves, without a leaf of its own. */
  markInternal(key: PatchKey): void {
    this.internal[key.level]?.add(levelIndex(key));
  }

  /** The leaves' values, level by level, each level in the order its leaves were added. */
  *values(): IterableIterator<T> {
    for (const level of this.leaves) {
      for (const leaf of level.values()) {
        yield leaf.value;
      }
    }
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
      const entry = journal[n];
      if (entry === undefined) {
        continue;
      }
      for (const child of entry.children) {
        if (this.levelOf(child).delete(levelIndex(child))) {
          this.count -= 1;
        }
      }
      this.internal[entry.key.level]?.delete(levelIndex(entry.key));
      this.add(entry.key, entry.value);
    }
    this.journal = null;
  }

  /**
   * Splits leaf `key` into the children `makeChild` keeps, then splits every leaf two or more levels
   * coarser than an edge or corner neighbour of a new leaf, across face edges and at cube corners,
   * until the cut is a restricted quadtree again. Returns the leaves added.
   *
   * @param makeChild - The value of a child of `parent`, or `null` to leave it out (a patch no view
   *   sees).
   */
  splitBalanced(key: PatchKey, makeChild: (child: PatchKey, parent: PatchKey) => T | null): T[] {
    const added: { readonly key: PatchKey; readonly value: T }[] = [];
    const work: PatchKey[] = [];
    const split = (k: PatchKey): void => {
      const leaf = this.levelOf(k).get(levelIndex(k));
      if (leaf === undefined) {
        return;
      }
      this.levelOf(k).delete(levelIndex(k));
      this.count -= 1;
      this.markInternal(k);
      const children: PatchKey[] = [];
      for (const child of childKeys(k)) {
        const value = makeChild(child, k);
        if (value !== null) {
          this.add(child, value);
          children.push(child);
          added.push({ key: child, value });
          work.push(child);
        }
      }
      this.journal?.push({ key: k, value: leaf.value, children });
    };
    split(key);
    while (work.length > 0) {
      const leaf = work.pop();
      if (leaf === undefined || !this.isLeaf(leaf)) {
        continue;
      }
      for (const n of neighbours(leaf)) {
        const coarse = this.coarserLeaf(n);
        if (coarse !== null && coarse.level < leaf.level - 1) {
          split(coarse);
          // The split may still leave a leaf too coarse beside this one: look again.
          work.push(leaf);
          break;
        }
      }
    }
    return added.filter((a) => this.isLeaf(a.key)).map((a) => a.value);
  }

  /**
   * The leaf covering `key`'s area at a coarser level than `key`, or `null` where none does: where
   * the area is split further or not selected. The walk stops at the first ancestor that is a leaf
   * or split, usually the parent.
   */
  coarserLeaf(key: PatchKey): PatchKey | null {
    for (let up = parentKey(key); up !== null; up = parentKey(up)) {
      const index = levelIndex(up);
      const leaf = this.leaves[up.level]?.get(index);
      if (leaf !== undefined) {
        return leaf.key;
      }
      if (this.internal[up.level]?.has(index) === true) {
        return null;
      }
    }
    return null;
  }

  private levelOf(key: PatchKey): Map<number, { readonly key: PatchKey; readonly value: T }> {
    const level = this.leaves[key.level];
    if (level === undefined) {
      throw new Error(`level ${key.level} is not a quadtree level`);
    }
    return level;
  }
}
