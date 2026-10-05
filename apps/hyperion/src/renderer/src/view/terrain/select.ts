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
import {
  PACKED_BOUNDS_LENGTH,
  packPatchBounds,
  PATCH_GEOMETRY_LENGTH,
  type PatchBounds,
  patchGeometryInto,
  unpackPatchBounds,
} from "./bounds";
import { vertexSpacing } from "./cube";
import { frustumOf, horizonCone } from "./cull";
import {
  type CellOut,
  childKeys,
  type Face,
  FACES,
  MAX_LEVEL,
  type PatchKey,
  patchKeyIndex,
  patchKeyString,
  rootKey,
  stepCellInto,
} from "./patchKey";
import { finestPatchSizeM, type GroundContact, inForcedRegionPacked } from "./grounded";
import { compareRequests } from "./priority";
import { type BodyFixedVec3, levelBoundM, levelHeightRangeM, type PlanetGeometry } from "./planet";
import { type ViewGeometry, viewExcessPacked, viewGeometry } from "./viewGeometry";

/*
 * The imports selection reads for every node, split or comparison, bound once. Under a module
 * runner, as the descent record's harness runs selection (Vite's), each read of an imported binding
 * is a getter call; those cost about a tenth of a selection there (R05.T7 perf (d)).
 */
const excessOf = viewExcessPacked;
const forcedAt = inForcedRegionPacked;
const keyIndex = patchKeyIndex;
const DEEPEST_LEVEL = MAX_LEVEL;

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

/**
 * A node of the traversal: a patch some view sees, or in a forced region, which is selected whether
 * or not a view sees it so that it is resident before contact (Design note 9), and what the views
 * make of it. Patches neither is true of get no node.
 */
interface TraversalNode {
  readonly key: PatchKey;
  /** Its place in the bounds memo, which holds its bounds and keeps its key string. */
  readonly cell: MemoCell;
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
  /**
   * Whether it has been split this call: set as its children are made, so that a candidate split
   * already, by the balance or as itself, is passed over with no walk of the tree.
   */
  split: boolean;
  /**
   * The children its split left out that are baked, a bit a child in `childKeys`' order: what
   * {@link Selection.hiddenBaked} lists of them, found when they were looked up.
   */
  hiddenBaked: number;
}

/**
 * A patch's place in the bounds memo, kept from call to call (R05.T7 perf (d)): its bounds over the
 * height range they were last built for, and its children's places, so that a child's is found
 * from its parent's with no lookup.
 */
interface MemoCell {
  readonly key: PatchKey;
  /** Its bounds, packed ({@link PACKED_BOUNDS_LENGTH}'s layout), once {@link MemoCell.built}. */
  readonly packed: Float64Array;
  built: boolean;
  /** The height range the bounds were built for, metres. */
  lowM: number;
  highM: number;
  /** Whether that range was the level's own, with nothing baked above. */
  levelRange: boolean;
  /** The bounds as an object, made when first drawn and kept while the range holds. */
  bounds: PatchBounds | null;
  /**
   * Its {@link patchKeyString}, once a selection has drawn it: kept, so that the output map's key
   * is neither built nor hashed again (V8 keeps a string's hash with it).
   */
  keyString: string | null;
  /** The slot of {@link MemoState.geometry} holding its geometry, or −1 where none does. */
  geometrySlot: number;
  /** The last call that read it, for the memo's prune. */
  usedAt: number;
  /** What the last selection to draw it drew, kept while the drawn fields hold. */
  selected: SelectedPatch | null;
  /** Its children's places in `childKeys`' order, `null` where not yet made. */
  child0: MemoCell | null;
  child1: MemoCell | null;
  child2: MemoCell | null;
  child3: MemoCell | null;
}

function memoCell(key: PatchKey): MemoCell {
  return {
    key,
    packed: new Float64Array(PACKED_BOUNDS_LENGTH),
    built: false,
    lowM: 0,
    highM: 0,
    levelRange: false,
    bounds: null,
    keyString: null,
    geometrySlot: -1,
    usedAt: 0,
    selected: null,
    child0: null,
    child1: null,
    child2: null,
    child3: null,
  };
}

/**
 * The patches whose geometry ({@link patchGeometryInto}) is kept, latest first: a patch's bounds
 * are often built again a frame or two after their first build, once it or its parent is baked
 * and its height range tightens, and the geometry is about two thirds of a build's cost.
 */
const GEOMETRY_SLOTS = 512;

/** The bounds memo of one planet, kept from call to call: what selection reads of it is a function of the inputs alone. */
interface MemoState {
  /** The six roots' places, by face. */
  readonly roots: ReadonlyArray<MemoCell>;
  /** The cells below the roots. */
  cells: number;
  /** Past this many cells the next call prunes the memo. */
  limit: number;
  /** The calls so far. */
  call: number;
  /** The kept geometries, {@link GEOMETRY_SLOTS} of them, each with the cell it is of. */
  readonly geometry: Float64Array[];
  readonly geometryOwners: (MemoCell | null)[];
  /** The slot the next geometry goes in. */
  nextGeometry: number;
}

/** What selection reads of a planet on every call, computed once a planet (it is immutable). */
interface PlanetTables {
  /** {@link selectionErrorM} per level, metres. */
  readonly errorM: ReadonlyArray<number>;
  /** Per level: ε_n and the lowest and highest height, metres, from the level table. */
  readonly levelBoundM: ReadonlyArray<number>;
  readonly levelLowM: ReadonlyArray<number>;
  readonly levelHighM: ReadonlyArray<number>;
  /** Bounds already built. */
  readonly memo: MemoState;
}

const planetTables = new WeakMap<PlanetGeometry, PlanetTables>();

/** The planet's tables, made on first use. */
function tablesOf(planet: PlanetGeometry): PlanetTables {
  let tables = planetTables.get(planet);
  if (tables === undefined) {
    const levels = { length: MAX_LEVEL + 1 };
    tables = {
      errorM: Array.from(levels, (_, level) => selectionErrorM(planet, level)),
      levelBoundM: Array.from(levels, (_, level) => levelBoundM(planet, level)),
      levelLowM: Array.from(levels, (_, level) => levelHeightRangeM(planet, level)[0]),
      levelHighM: Array.from(levels, (_, level) => levelHeightRangeM(planet, level)[1]),
      memo: {
        roots: FACES.map((face) => memoCell(rootKey(face))),
        cells: 0,
        limit: MEMO_CELL_LIMIT,
        call: 0,
        geometry: [],
        geometryOwners: Array.from({ length: GEOMETRY_SLOTS }, () => null),
        nextGeometry: 0,
      },
    };
    planetTables.set(planet, tables);
  }
  return tables;
}

/**
 * The fewest cells at which a planet's bounds memo prunes; after a prune it waits for twice the
 * cells kept, so that a memo every recent call reads is not walked again at once.
 */
const MEMO_CELL_LIMIT = 1 << 17;

/** The latest calls whose cells a prune keeps. */
const MEMO_RECENT_CALLS = 64;

/**
 * Prunes `planet`'s bounds memo as a selection does once it holds more cells than its limit: drops
 * the cells none of the last {@link MEMO_RECENT_CALLS} calls read, or failing that, those the last
 * call did not read, and sets the next limit. Returns the cells kept below the six roots.
 *
 * @remarks
 * Exported for its tests, which cannot reach {@link MEMO_CELL_LIMIT}'s cells cheaply. What
 * selection returns does not depend on it.
 */
export function pruneSelectionMemo(planet: PlanetGeometry): number {
  return pruneMemo(tablesOf(planet).memo);
}

/** {@link pruneSelectionMemo} of a planet's memo. */
function pruneMemo(memo: MemoState): number {
  let kept = dropUnreadSince(memo, memo.call - MEMO_RECENT_CALLS + 1);
  if (kept > MEMO_CELL_LIMIT) {
    kept = dropUnreadSince(memo, memo.call);
  }
  memo.limit = Math.max(MEMO_CELL_LIMIT, 2 * kept);
  return kept;
}

/**
 * Drops every cell below the roots last read before call `since`, whole subtrees, since a cell is
 * read only after its parent; returns the cells kept and counts them as the memo's.
 */
function dropUnreadSince(memo: MemoState, since: number): number {
  let kept = 0;
  const stack: MemoCell[] = [...memo.roots];
  for (let cell = stack.pop(); cell !== undefined; cell = stack.pop()) {
    if (cell.child0 !== null && cell.child0.usedAt < since) {
      cell.child0 = null;
    }
    if (cell.child1 !== null && cell.child1.usedAt < since) {
      cell.child1 = null;
    }
    if (cell.child2 !== null && cell.child2.usedAt < since) {
      cell.child2 = null;
    }
    if (cell.child3 !== null && cell.child3.usedAt < since) {
      cell.child3 = null;
    }
    for (const child of [cell.child0, cell.child1, cell.child2, cell.child3]) {
      if (child !== null) {
        kept += 1;
        stack.push(child);
      }
    }
  }
  memo.cells = kept;
  return kept;
}

/** The memo cell of patch `key`, a root or a child of `parent`'s, made where there is none. */
function cellOf(t: Traversal, key: PatchKey, parent: TraversalNode | null): MemoCell {
  if (parent === null) {
    const root = t.memo.roots[key.face];
    if (root === undefined) {
      throw new Error(`face ${key.face} has no root`);
    }
    return root;
  }
  const up = parent.cell;
  const n = (key.i & 1) | ((key.j & 1) << 1);
  const known = n === 0 ? up.child0 : n === 1 ? up.child1 : n === 2 ? up.child2 : up.child3;
  if (known !== null) {
    return known;
  }
  const cell = memoCell(key);
  if (n === 0) {
    up.child0 = cell;
  } else if (n === 1) {
    up.child1 = cell;
  } else if (n === 2) {
    up.child2 = cell;
  } else {
    up.child3 = cell;
  }
  t.memo.cells += 1;
  return cell;
}

/**
 * Builds `cell`'s bounds over patch `key`'s height range, from its nearest baked ancestor `baked`,
 * where the memo does not hold them for that range already.
 */
function buildBounds(t: Traversal, cell: MemoCell, key: PatchKey, baked: BakedRange | null): void {
  // The range only when the memo cannot answer with the level's own (nothing baked above it).
  if (cell.built && baked === null && cell.levelRange) {
    return;
  }
  let lowM = t.levelLowM[key.level] ?? 0;
  let highM = t.levelHighM[key.level] ?? 0;
  if (baked !== null) {
    const range = t.range;
    inheritedRangeInto(
      range,
      lowM,
      highM,
      (t.levelBoundM[baked.level] ?? 0) + (t.levelBoundM[Math.max(0, key.level - 1)] ?? 0),
      t.levelBoundM[key.level] ?? 0,
      baked,
      t.skirtMarginM,
    );
    lowM = range[0] ?? lowM;
    highM = range[1] ?? highM;
  }
  if (cell.built && cell.lowM === lowM && cell.highM === highM) {
    return;
  }
  packPatchBounds(cell.packed, geometryOf(t.memo, t.planet, cell), lowM, highM);
  cell.built = true;
  cell.lowM = lowM;
  cell.highM = highM;
  cell.levelRange = baked === null;
  cell.bounds = null;
}

/** `cell`'s patch geometry, kept from an earlier build or made now in the oldest slot. */
function geometryOf(memo: MemoState, planet: PlanetGeometry, cell: MemoCell): Float64Array {
  const slot = cell.geometrySlot;
  if (slot >= 0 && memo.geometryOwners[slot] === cell) {
    const kept = memo.geometry[slot];
    if (kept !== undefined) {
      return kept;
    }
  }
  const next = memo.nextGeometry;
  memo.nextGeometry = (next + 1) % GEOMETRY_SLOTS;
  const previous = memo.geometryOwners[next] ?? null;
  if (previous !== null) {
    previous.geometrySlot = -1;
    memo.geometryOwners[next] = null;
  }
  let geometry = memo.geometry[next];
  if (geometry === undefined) {
    geometry = new Float64Array(PATCH_GEOMETRY_LENGTH);
    memo.geometry[next] = geometry;
  }
  patchGeometryInto(geometry, planet, cell.key);
  memo.geometryOwners[next] = cell;
  cell.geometrySlot = next;
  return geometry;
}

/** `cell`'s bounds as an object, made from its packed numbers on first use and kept. */
function boundsOfCell(cell: MemoCell): PatchBounds {
  cell.bounds ??= unpackPatchBounds(cell.packed);
  return cell.bounds;
}

/** One `f32` step's relative size: a baked height rounded to `f32` is within this of its value. */
const F32_RELATIVE_STEP = 2 ** -23;

/** One call's state, with its planet's tables. */
interface Traversal extends PlanetTables {
  readonly planet: PlanetGeometry;
  readonly views: ReadonlyArray<PreparedView>;
  /** Scratch for a height range, low then high, metres. */
  readonly range: Float64Array;
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
  const range = new Float64Array(2);
  inheritedRangeInto(
    range,
    levelLow,
    levelHigh,
    levelBoundM(planet, baked.level) + levelBoundM(planet, Math.max(0, n - 1)),
    levelBoundM(planet, n),
    baked,
    skirtMarginM,
  );
  return [range[0] ?? levelLow, range[1] ?? levelHigh];
}

/**
 * Writes {@link inheritedHeightRangeM}'s range for a baked ancestor into `out`, low then high,
 * metres, from the level's range, the widening ε_m + ε_{n−1} and the level's own bound ε_n.
 */
function inheritedRangeInto(
  out: Float64Array,
  levelLowM: number,
  levelHighM: number,
  widenM: number,
  ownBoundM: number,
  baked: BakedRange,
  skirtMarginM: number,
): void {
  // The largest |h| the patch's own heights can reach, for the f32 steps: the bake's own
  // `f32_step(largest_h)` in its skirt depth, and the rounding of the baked range.
  const largest = Math.max(Math.abs(baked.lowM), Math.abs(baked.highM)) + widenM;
  const step = largest * F32_RELATIVE_STEP;
  // The bake's skirt depth: ε_n, an f32 step of the largest height and the caller's margin.
  const skirt = ownBoundM + step + skirtMarginM;
  const low = baked.lowM - step - widenM - skirt;
  const high = baked.highM + step + widenM;
  out[0] = Math.max(levelLowM, low);
  out[1] = Math.min(levelHighM, high);
}

/** The traversal node of patch `key`, or `null` where no view sees it and no forced region reaches it. */
function nodeOf(t: Traversal, key: PatchKey, parent: TraversalNode | null): TraversalNode | null {
  const cell = cellOf(t, key, parent);
  cell.usedAt = t.memo.call;
  const own = t.heightRanges?.heightRangeM(key);
  const baked: BakedRange | null =
    own === undefined ? (parent?.baked ?? null) : { level: key.level, lowM: own[0], highM: own[1] };
  buildBounds(t, cell, key, baked);
  const packed = cell.packed;
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
    // Negative where the view cannot see the patch.
    const e = excessOf(v.geometry, packed, errorM);
    if (e < 0) {
      continue;
    }
    seenBy |= 1 << n;
    if (e > 1) {
      wantedBy |= 1 << n;
    }
    excess = Math.max(excess, e);
    weighted = Math.max(weighted, v.input.weight * e);
  }
  const forced = t.grounded.length > 0 && forcedAt(packed, t.grounded, t.patchSizeM);
  if (seenBy === 0 && !forced) {
    if (own !== undefined && parent !== null) {
      parent.hiddenBaked |= 1 << ((key.i & 1) | ((key.j & 1) << 1));
    }
    return null;
  }
  return {
    key,
    cell,
    seenBy,
    wantedBy,
    excess,
    weighted,
    forced,
    baked,
    resident: own !== undefined,
    parent,
    split: false,
    hiddenBaked: 0,
  };
}

/** The most views selection takes at once: one bit each in a node's view masks. */
export const MAX_SELECTION_VIEWS = 31;

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
    // Negative where the view cannot see the patch.
    const e = excessOf(v.geometry, standIn.cell.packed, errorM);
    if (!(e < 0)) {
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
  if (node.key.level >= t.planet.finestLevel) {
    return false;
  }
  if (node.forced) {
    return true;
  }
  return node.excess > 1 && (t.heightRanges === null || node.resident);
}

/**
 * Whether candidate `a` goes before `b` where both are as forced and as weighted as each other:
 * the coarser first, then the smaller `patchKeyIndex`.
 */
function keyBefore(a: TraversalNode, b: TraversalNode): boolean {
  if (a.key.level !== b.key.level) {
    return a.key.level < b.key.level;
  }
  return keyIndex(a.key) < keyIndex(b.key);
}

/**
 * A binary max-heap of the candidates for splitting: forced first, then the larger weighted error,
 * then by {@link keyBefore}.
 *
 * @remarks
 * Each slot's forced flag and weighted excess are kept beside it in typed arrays, so that most
 * comparisons read two adjacent numbers rather than two nodes (R05.T7 perf (d)), comparison for
 * comparison as the heap of nodes did.
 */
class CandidateHeap {
  private readonly items: TraversalNode[] = [];
  private forced = new Uint8Array(1024);
  private weighted = new Float64Array(1024);
  private length = 0;

  get size(): number {
    return this.length;
  }

  /** Whether the node in slot `a` goes before the node in slot `b`. */
  private before(a: number, b: number): boolean {
    const forced = this.forced;
    const fa = forced[a] ?? 0;
    const fb = forced[b] ?? 0;
    if (fa !== fb) {
      return fa === 1;
    }
    const weighted = this.weighted;
    const wa = weighted[a] ?? 0;
    const wb = weighted[b] ?? 0;
    if (wa !== wb) {
      return wa > wb;
    }
    const na = this.items[a];
    const nb = this.items[b];
    return na !== undefined && nb !== undefined && keyBefore(na, nb);
  }

  /** Puts `node` in slot `at`. */
  private place(at: number, node: TraversalNode): void {
    this.items[at] = node;
    this.forced[at] = node.forced ? 1 : 0;
    this.weighted[at] = node.weighted;
  }

  /** Moves the node in slot `from` to slot `to`. */
  private move(from: number, to: number): void {
    const node = this.items[from];
    if (node !== undefined) {
      this.items[to] = node;
    }
    this.forced[to] = this.forced[from] ?? 0;
    this.weighted[to] = this.weighted[from] ?? 0;
  }

  push(node: TraversalNode): void {
    if (this.length === this.weighted.length) {
      const forced = new Uint8Array(2 * this.length);
      forced.set(this.forced);
      this.forced = forced;
      const weighted = new Float64Array(2 * this.length);
      weighted.set(this.weighted);
      this.weighted = weighted;
    }
    let at = this.length;
    this.length += 1;
    this.place(at, node);
    while (at > 0) {
      const up = (at - 1) >> 1;
      if (!this.before(at, up)) {
        break;
      }
      this.swap(at, up);
      at = up;
    }
  }

  pop(): TraversalNode | undefined {
    if (this.length === 0) {
      return undefined;
    }
    const top = this.items[0];
    this.length -= 1;
    const last = this.length;
    if (last === 0) {
      return top;
    }
    this.move(last, 0);
    let at = 0;
    for (;;) {
      const left = 2 * at + 1;
      if (left >= last) {
        break;
      }
      const right = left + 1;
      const child = right < last && this.before(right, left) ? right : left;
      if (!this.before(child, at)) {
        break;
      }
      this.swap(child, at);
      at = child;
    }
    return top;
  }

  /** Swaps the nodes in slots `a` and `b`, with their sort keys, so that the arrays stay in step. */
  private swap(a: number, b: number): void {
    const items = this.items;
    const node = items[a];
    const other = items[b];
    if (node === undefined || other === undefined) {
      return;
    }
    items[a] = other;
    items[b] = node;
    const forced = this.forced;
    const f = forced[a] ?? 0;
    forced[a] = forced[b] ?? 0;
    forced[b] = f;
    const weighted = this.weighted;
    const w = weighted[a] ?? 0;
    weighted[a] = weighted[b] ?? 0;
    weighted[b] = w;
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
  const tables = tablesOf(planet);
  const memo = tables.memo;
  if (memo.cells > memo.limit) {
    pruneMemo(memo);
  }
  memo.call += 1;
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
    ...tables,
    range: new Float64Array(2),
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
    parent.split = true;
    return nodeOf(t, child, parent);
  };
  for (const face of FACES) {
    const root = nodeOf(t, rootKey(face), null);
    if (root !== null) {
      tree.addRoot(root.key, root);
      offer(root);
    }
  }
  let limited = false;
  let limitExcess = 0;
  for (let node = heap.pop(); node !== undefined; node = heap.pop()) {
    // Every candidate is in the tree, and nothing leaves it before the loop ends.
    if (node.split) {
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
  const leaves = tree.leafValues();
  const patches = new Map<string, SelectedPatch>();
  for (const node of leaves) {
    const cell = node.cell;
    cell.keyString ??= patchKeyString(node.key);
    const bounds = boundsOfCell(cell);
    const forced = node.forced && node.key.level === planet.finestLevel;
    const seen = node.seenBy !== 0;
    let selected = cell.selected;
    if (
      selected === null ||
      selected.bounds !== bounds ||
      selected.forced !== forced ||
      selected.seen !== seen
    ) {
      selected = { key: node.key, bounds, forced, seen };
      cell.selected = selected;
    }
    patches.set(cell.keyString, selected);
  }
  return {
    patches,
    demand: demandOf(t, leaves),
    limited,
    limitExcess,
    hiddenBaked: hiddenBakedOf(t, tree),
  };
}

/** The baked patches among the tree's bare keys ({@link Selection.hiddenBaked}). */
function hiddenBakedOf(t: Traversal, tree: PatchLeafSet<TraversalNode>): PatchKey[] {
  if (t.heightRanges === null) {
    return [];
  }
  // Each bare key's bake was looked up as selection reached it: the left-out children's by nodeOf,
  // the split patches' as their own `resident`.
  return tree.bareKeys({
    leftOut: (parent, n) => (parent.hiddenBaked & (1 << n)) !== 0,
    split: (node) => node.resident,
  });
}

/**
 * The patches to bake, by priority (Design note 24, with the ruling's breadth-first rule): for each
 * selected patch not yet baked, the shallowest unbaked patch on its way down whose parent is baked
 * (or a root), each once, at the largest over the views of w_view × ρ ÷ τ of the patch drawn in its
 * place, its parent; a forced patch is requested whatever its ancestors, and outranks everything.
 * Ordered forced first, then by priority, ties by `patchKeyString`.
 */
function demandOf(t: Traversal, leaves: ReadonlyArray<TraversalNode>): PatchRequest[] {
  const requests = new Map<TraversalNode, PatchRequest>();
  for (const leaf of leaves) {
    if (leaf.resident) {
      continue;
    }
    const forced = leaf.forced && leaf.key.level === t.planet.finestLevel;
    let target: TraversalNode = leaf;
    if (!forced) {
      // Up to the shallowest unbaked ancestor, by pointer: the top of the leaf's unbaked chain.
      for (let node = leaf.parent; node !== null && !node.resident; node = node.parent) {
        target = node;
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

/** The steps (di, dj) to a cell's eight neighbours, in the order the balance tries them. */
const NEIGHBOUR_DI: ReadonlyArray<number> = [-1, 1, 0, 0, -1, 1, 1, -1];
const NEIGHBOUR_DJ: ReadonlyArray<number> = [0, 0, -1, 1, -1, -1, 1, 1];

/** Which of {@link PatchLeafSet.bareKeys}' keys to list. */
export interface BareKeyFilter<T> {
  /** Whether to list child `n` (in `childKeys`' order) that the split of the node of `parent` left out. */
  leftOut(parent: T, n: number): boolean;
  /** Whether to list the split node of `value`, which has no leaf beneath it. */
  split(value: T): boolean;
}

/** A node of {@link PatchLeafSet}'s trees: a leaf, or split into the children kept. */
interface TreeNode<T> {
  readonly key: PatchKey;
  readonly value: T;
  /** The node it was split from, `null` for a root. */
  readonly parent: TreeNode<T> | null;
  /** Whether it is split; a leaf has no children. */
  split: boolean;
  /** Its children in `childKeys`' order, `null` where left out and in a leaf. */
  child0: TreeNode<T> | null;
  child1: TreeNode<T> | null;
  child2: TreeNode<T> | null;
  child3: TreeNode<T> | null;
}

function treeNode<T>(key: PatchKey, value: T, parent: TreeNode<T> | null): TreeNode<T> {
  return {
    key,
    value,
    parent,
    split: false,
    child0: null,
    child1: null,
    child2: null,
    child3: null,
  };
}

/** Child `n` of `node`, in `childKeys`' order: n = Δi + 2 Δj. */
function childAt<T>(node: TreeNode<T>, n: number): TreeNode<T> | null {
  if (n === 0) {
    return node.child0;
  }
  if (n === 1) {
    return node.child1;
  }
  return n === 2 ? node.child2 : node.child3;
}

/**
 * The leaves of a cut of the six faces' quadtrees, with the patches split above them, kept a
 * restricted quadtree as it is split, and able to undo a split and the balance it brought.
 *
 * @remarks
 * Held as trees of nodes from the six roots, each with its parent and its four children, so that
 * the leaf over a neighbour's cell is found by walking up the leaf's own ancestors to the first
 * holding the cell and down from there, a step or two, with no key, string or map lookup, and a
 * split allocates only its children (R05.T7 perf (c)). The leaves come out depth first from face 0,
 * in `childKeys`' order, which is deterministic. Exported for its tests.
 */
export class PatchLeafSet<T> {
  private readonly roots: (TreeNode<T> | null)[] = [null, null, null, null, null, null];
  /**
   * The splits since {@link begin}, in order, while recording: the first `journalLength` entries.
   * The scratch arrays are reused, never truncated, so that clearing one is a store.
   */
  private readonly journal: TreeNode<T>[] = [];
  private journalLength = 0;
  private recording = false;
  private count = 0;
  private readonly counts: (value: T) => boolean;
  /** {@link splitBalanced}'s scratch: the new leaves to check, and every node it added. */
  private readonly work: TreeNode<T>[] = [];
  private readonly added: TreeNode<T>[] = [];
  private addedLength = 0;
  /** The neighbour cell {@link stepCellInto} writes. */
  private readonly cell: CellOut = { face: 0, i: 0, j: 0 };

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
    this.roots[key.face] = treeNode(key, value, null);
    if (this.counts(value)) {
      this.count += 1;
    }
  }

  /** The node at `key`, or `null` where the trees do not reach it. */
  private find(key: PatchKey): TreeNode<T> | null {
    let node = this.roots[key.face] ?? null;
    while (node !== null && node.key.level < key.level) {
      if (!node.split) {
        return null;
      }
      const shift = key.level - node.key.level - 1;
      node = childAt(node, ((key.i >> shift) & 1) | (((key.j >> shift) & 1) << 1));
    }
    return node;
  }

  /** Whether `key` is a leaf. */
  isLeaf(key: PatchKey): boolean {
    const node = this.find(key);
    return node !== null && !node.split;
  }

  /** The leaves' values, depth first from face 0. */
  *values(): IterableIterator<T> {
    yield* this.leafValues();
  }

  /** The leaves' values, depth first from face 0, as an array. */
  leafValues(): T[] {
    const leaves: T[] = [];
    const stack: TreeNode<T>[] = [];
    for (let face = 5; face >= 0; face -= 1) {
      const root = this.roots[face];
      if (root !== null && root !== undefined) {
        stack.push(root);
      }
    }
    for (let node = stack.pop(); node !== undefined; node = stack.pop()) {
      if (!node.split) {
        leaves.push(node.value);
        continue;
      }
      for (let n = 3; n >= 0; n -= 1) {
        const child = childAt(node, n);
        if (child !== null) {
          stack.push(child);
        }
      }
    }
    return leaves;
  }

  /**
   * The keys the trees reach that hold no leaf, depth first from face 0: each child a split left
   * out, and each split node with no leaf beneath it, after its children; with `keep`, only those
   * it keeps.
   */
  bareKeys(keep?: BareKeyFilter<T>): PatchKey[] {
    const bare: PatchKey[] = [];
    for (const root of this.roots) {
      if (root !== null) {
        this.collectBare(root, bare, keep);
      }
    }
    return bare;
  }

  /** Adds the bare keys at and under `node` to `bare`; returns whether a leaf lies at or under it. */
  private collectBare(
    node: TreeNode<T>,
    bare: PatchKey[],
    keep: BareKeyFilter<T> | undefined,
  ): boolean {
    if (!node.split) {
      return true;
    }
    let keys: readonly PatchKey[] | null = null;
    let leaf = false;
    for (let n = 0; n < 4; n += 1) {
      const child = childAt(node, n);
      if (child === null) {
        if (keep === undefined || keep.leftOut(node.value, n)) {
          keys ??= childKeys(node.key);
          const key = keys[n];
          if (key !== undefined) {
            bare.push(key);
          }
        }
      } else if (this.collectBare(child, bare, keep)) {
        leaf = true;
      }
    }
    if (!leaf && (keep === undefined || keep.split(node.value))) {
      bare.push(node.key);
    }
    return leaf;
  }

  /** Starts recording splits, so that {@link rollback} can undo them. */
  begin(): void {
    this.journalLength = 0;
    this.recording = true;
  }

  /** Keeps the splits since {@link begin}. */
  commit(): void {
    this.journalLength = 0;
    this.recording = false;
  }

  /** Undoes every split since {@link begin}, latest first. */
  rollback(): void {
    const journal = this.journal;
    for (let n = this.journalLength - 1; n >= 0; n -= 1) {
      const node = journal[n];
      if (node === undefined || !node.split) {
        continue;
      }
      // Undone latest first, so the children are leaves again here.
      for (let c = 0; c < 4; c += 1) {
        const child = childAt(node, c);
        if (child !== null && this.counts(child.value)) {
          this.count -= 1;
        }
      }
      node.split = false;
      node.child0 = null;
      node.child1 = null;
      node.child2 = null;
      node.child3 = null;
      if (this.counts(node.value)) {
        this.count += 1;
      }
    }
    this.commit();
  }

  private split(node: TreeNode<T>, makeChild: (child: PatchKey, parent: T) => T | null): void {
    if (node.split) {
      return;
    }
    const { face, level, i, j } = node.key;
    if (level >= DEEPEST_LEVEL) {
      throw new Error(
        `patch ${patchKeyString(node.key)} is at the deepest level and has no children`,
      );
    }
    if (this.counts(node.value)) {
      this.count -= 1;
    }
    // `childKeys`' keys, made one at a time.
    const below = level + 1;
    node.child0 = this.child(node, { face, level: below, i: 2 * i, j: 2 * j }, makeChild);
    node.child1 = this.child(node, { face, level: below, i: 2 * i + 1, j: 2 * j }, makeChild);
    node.child2 = this.child(node, { face, level: below, i: 2 * i, j: 2 * j + 1 }, makeChild);
    node.child3 = this.child(node, { face, level: below, i: 2 * i + 1, j: 2 * j + 1 }, makeChild);
    node.split = true;
    if (this.recording) {
      this.journal[this.journalLength] = node;
      this.journalLength += 1;
    }
  }

  private child(
    parent: TreeNode<T>,
    key: PatchKey,
    makeChild: (child: PatchKey, parent: T) => T | null,
  ): TreeNode<T> | null {
    const value = makeChild(key, parent.value);
    if (value === null) {
      return null;
    }
    if (this.counts(value)) {
      this.count += 1;
    }
    return treeNode(key, value, parent);
  }

  /** Splits `node` and queues its children, as new leaves to check and as nodes added. */
  private splitAndQueue(
    node: TreeNode<T>,
    makeChild: (child: PatchKey, parent: T) => T | null,
  ): void {
    this.split(node, makeChild);
    for (let n = 0; n < 4; n += 1) {
      const child = childAt(node, n);
      if (child !== null) {
        this.added[this.addedLength] = child;
        this.addedLength += 1;
        this.work.push(child);
      }
    }
  }

  /**
   * Splits leaf `key` into the children `makeChild` keeps, then splits every leaf two or more levels
   * coarser than an edge or corner neighbour of a new leaf, across face edges and at cube corners,
   * until the cut is a restricted quadtree again. Returns the leaves added.
   *
   * @param makeChild - The value of a child of the patch whose value is `parent`, or `null` to
   *   leave it out (a patch no view sees). It must not change this set.
   */
  splitBalanced(key: PatchKey, makeChild: (child: PatchKey, parent: T) => T | null): T[] {
    const first = this.find(key);
    if (first === null || first.split) {
      return [];
    }
    const work = this.work;
    const added = this.added;
    this.splitAndQueue(first, makeChild);
    for (let leaf = work.pop(); leaf !== undefined; leaf = work.pop()) {
      if (leaf.split) {
        continue;
      }
      const coarse = this.tooCoarseBeside(leaf);
      if (coarse !== null) {
        this.splitAndQueue(coarse, makeChild);
        // The split may still leave a leaf too coarse beside this one: look again.
        work.push(leaf);
      }
    }
    const leaves: T[] = [];
    for (let n = 0; n < this.addedLength; n += 1) {
      const node = added[n];
      if (node !== undefined && !node.split) {
        leaves.push(node.value);
      }
    }
    this.addedLength = 0;
    return leaves;
  }

  /**
   * The first leaf, over `leaf`'s edge and then corner neighbours in turn, that is two or more
   * levels coarser than `leaf`, or `null` where none is.
   *
   * @remarks
   * The leaf over a neighbour's cell at the parent's level decides it, and nothing is split during
   * the scan, so a neighbour sharing that cell with an earlier one answers as that one did. Away
   * from the face's edges the eight neighbours fall in the parent's own cell (which is split, so no
   * leaf there is coarser) and three others, met first in the order (Δi, 0), (0, Δj), (Δi, Δj) for
   * the leaf's outward steps Δi and Δj; those three alone are tried, in that order, and the answer
   * is the eight-neighbour scan's.
   */
  private tooCoarseBeside(leaf: TreeNode<T>): TreeNode<T> | null {
    const key = leaf.key;
    const level = key.level;
    if (level < 2) {
      return null;
    }
    const last = (1 << level) - 1;
    if (key.i > 0 && key.j > 0 && key.i < last && key.j < last) {
      // The outward steps: towards i − 1 from an even column, i + 1 from an odd one, and so for j.
      const di = (key.i & 1) === 0 ? -1 : 1;
      const dj = (key.j & 1) === 0 ? -1 : 1;
      return (
        this.tooCoarseAt(leaf, key.face, key.i + di, key.j) ??
        this.tooCoarseAt(leaf, key.face, key.i, key.j + dj) ??
        this.tooCoarseAt(leaf, key.face, key.i + di, key.j + dj)
      );
    }
    const cell = this.cell;
    for (let n = 0; n < 8; n += 1) {
      if (stepCellInto(cell, key, NEIGHBOUR_DI[n] ?? 0, NEIGHBOUR_DJ[n] ?? 0)) {
        const coarse = this.tooCoarseAt(leaf, cell.face, cell.i, cell.j);
        if (coarse !== null) {
          return coarse;
        }
      }
    }
    return null;
  }

  /**
   * The leaf over cell (`i`, `j`) of `face` at `leaf`'s level, `leaf`'s neighbour, where it is two
   * or more levels coarser than `leaf`; otherwise `null`.
   *
   * @remarks
   * Such a leaf holds the cell's grandparent cell, two levels up, so it is the leaf found going
   * down to that cell: from the first of `leaf`'s ancestors holding it (on another face, that
   * face's root). Where that ancestor is `leaf`'s own grandparent, which is split, there is none.
   */
  private tooCoarseAt(leaf: TreeNode<T>, face: Face, i: number, j: number): TreeNode<T> | null {
    const level = leaf.key.level - 2;
    const gi = i >> 2;
    const gj = j >> 2;
    let from: TreeNode<T> | null = null;
    if (face === leaf.key.face) {
      for (let node = leaf.parent?.parent ?? null; node !== null; node = node.parent) {
        const shift = level - node.key.level;
        if (node.key.i === gi >> shift && node.key.j === gj >> shift) {
          if (shift === 0) {
            return null;
          }
          from = node;
          break;
        }
      }
    }
    return this.leafDown(from ?? this.roots[face] ?? null, level, gi, gj);
  }

  /**
   * The leaf covering `key`'s area at a coarser level than `key`, or `null` where none does: where
   * the area is split further or not selected.
   */
  coarserLeaf(key: PatchKey): PatchKey | null {
    if (key.level === 0) {
      return null;
    }
    const node = this.leafDown(this.roots[key.face] ?? null, key.level - 1, key.i >> 1, key.j >> 1);
    return node?.key ?? null;
  }

  /**
   * From `node`, which holds cell (`i`, `j`) of `level` or is `null`, down to the deepest node
   * holding the cell, at `level` at most: that node if it is a leaf, `null` if it is split (the
   * cell is split further, or its part was left out) or there is none.
   */
  private leafDown(
    node: TreeNode<T> | null,
    level: number,
    i: number,
    j: number,
  ): TreeNode<T> | null {
    let at = node;
    while (at !== null && at.key.level < level) {
      if (!at.split) {
        return at;
      }
      const shift = level - at.key.level - 1;
      at = childAt(at, ((i >> shift) & 1) | (((j >> shift) & 1) << 1));
    }
    return at === null || at.split ? null : at;
  }
}
