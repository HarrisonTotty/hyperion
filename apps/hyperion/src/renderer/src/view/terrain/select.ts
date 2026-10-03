/**
 * Patch selection: which patches each view needs, as a pure function of the views, the quality
 * setting and the grounded bodies (plan R05, T7.b–T7.d, Design note 7).
 *
 * @remarks
 * A level is drawn where its error bound subtends at most τ pixels at the nearest point of the
 * patch's bounding volume; a camera inside the volume refines, down to the finest level. The set is
 * a function of each view's pose, field of view and viewport, the setting and the grounded bodies
 * alone: no history, no arrival order and no hysteresis, the morph hiding a level change. The
 * selected patches form a restricted quadtree: no two neighbours, across face edges and at cube
 * corners included, differ by more than one level. Patches no view can see, outside every frustum
 * or below every horizon, are not selected.
 */

import type { Quaternion } from "../camera/pose";
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
import { type BodyFixedVec3, levelBoundM, type PlanetGeometry } from "./planet";

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

/** A grounded or descending body: a sphere about its position (Design note 9). */
export interface GroundContact {
  /** Its position, body-fixed metres from the body's centre. */
  readonly positionM: BodyFixedVec3;
  /** Its bounding radius, metres. */
  readonly radiusM: number;
}

/** Everything selection reads. */
export interface SelectionInput {
  readonly planet: PlanetGeometry;
  readonly views: ReadonlyArray<ViewSelectionInput>;
  readonly setting: QualitySetting;
  readonly grounded: ReadonlyArray<GroundContact>;
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
  /** W_px ÷ (2 tan(fov_h ÷ 2) τ): ρ ÷ τ per metre of error per metre of distance's inverse. */
  readonly excessPerMetre: number;
}

/** A node of the traversal: a patch and what each view makes of it. */
interface TraversalNode {
  readonly key: PatchKey;
  readonly keyString: string;
  readonly bounds: PatchBounds;
  /** Per view: ρ ÷ τ, or `null` where the view cannot see the patch. */
  readonly excess: ReadonlyArray<number | null>;
}

/** Bounds already computed, per planet; the values are a pure function of the planet and key. */
const boundsMemo = new WeakMap<PlanetGeometry, Map<string, PatchBounds>>();

/** The most bounds kept per planet; past it the older half is dropped. */
const BOUNDS_MEMO_LIMIT = 1 << 17;

function boundsOf(planet: PlanetGeometry, key: PatchKey, keyString: string): PatchBounds {
  let memo = boundsMemo.get(planet);
  if (memo === undefined) {
    memo = new Map();
    boundsMemo.set(planet, memo);
  }
  const known = memo.get(keyString);
  if (known !== undefined) {
    return known;
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
  const b = patchBounds(planet, key);
  memo.set(keyString, b);
  return b;
}

/** One call's state: the planet, the views and the per-level error, and the traversal's record. */
interface Traversal {
  readonly planet: PlanetGeometry;
  readonly views: ReadonlyArray<PreparedView>;
  /** {@link selectionErrorM} per level, metres. */
  readonly errorM: ReadonlyArray<number>;
  readonly leaves: Map<string, TraversalNode>;
  /** The patches refined into their children: the selection's interior nodes. */
  readonly internal: Set<string>;
}

function nodeOf(t: Traversal, key: PatchKey): TraversalNode {
  const keyString = patchKeyString(key);
  const bounds = boundsOf(t.planet, key, keyString);
  const errorM = t.errorM[key.level] ?? 0;
  const excess = t.views.map((v) => {
    const rel = relativeBounds(bounds, v.input.camera.positionM);
    if (!inFrustum(rel, v.frustum) || !aboveHorizon(rel, v.horizon)) {
      return null;
    }
    const d = distanceToBoxM(rel);
    if (!(d > 0)) {
      return errorM > 0 ? Infinity : 0;
    }
    return (errorM * v.excessPerMetre) / d;
  });
  return { key, keyString, bounds, excess };
}

function visible(node: TraversalNode): boolean {
  return node.excess.some((e) => e !== null);
}

function wantsRefining(planet: PlanetGeometry, node: TraversalNode): boolean {
  return node.key.level < planet.finestLevel && node.excess.some((e) => e !== null && e > 1);
}

/**
 * Selects the patches to draw for every view at once (Design note 7): the union of the views'
 * selections, refined wherever any view that sees a patch finds its error above its tolerance,
 * then balanced to a restricted quadtree.
 */
export function selectPatches(input: SelectionInput): Selection {
  const { planet } = input;
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
    leaves: new Map(),
    internal: new Set(),
  };
  for (const face of FACES) {
    expand(t, rootKey(face));
  }
  restrictQuadtree(t.leaves, t.internal, (key) => expand(t, key));
  const patches = new Map<string, SelectedPatch>();
  for (const keyString of [...t.leaves.keys()].toSorted()) {
    const node = t.leaves.get(keyString);
    if (node !== undefined) {
      patches.set(keyString, { key: node.key, bounds: node.bounds, forced: false });
    }
  }
  return { patches, demand: [] };
}

/**
 * Adds to the traversal's leaves the patches selected under `key`: nothing where no view sees it,
 * `key` itself where no view that sees it wants it finer or it is the finest level, else its
 * children's in turn. Returns the leaves it added.
 */
function expand(t: Traversal, key: PatchKey): TraversalNode[] {
  const added: TraversalNode[] = [];
  const stack: PatchKey[] = [key];
  while (stack.length > 0) {
    const next = stack.pop();
    if (next === undefined) {
      break;
    }
    const node = nodeOf(t, next);
    if (!visible(node)) {
      continue;
    }
    if (wantsRefining(t.planet, node)) {
      t.internal.add(node.keyString);
      stack.push(...childKeys(next));
    } else {
      t.leaves.set(node.keyString, node);
      added.push(node);
    }
  }
  return added;
}

/**
 * The leaf covering `key`'s area at a coarser level than `key`, or `null` where none does: where
 * the area is refined further or not selected at all. The walk stops at the first ancestor that
 * is a leaf or interior, which is usually the parent.
 */
function coarserLeaf<T>(
  leaves: ReadonlyMap<string, T>,
  internal: ReadonlySet<string>,
  key: PatchKey,
): T | null {
  let ancestor = parentKey(key);
  while (ancestor !== null) {
    const keyString = patchKeyString(ancestor);
    const leaf = leaves.get(keyString);
    if (leaf !== undefined) {
      return leaf;
    }
    if (internal.has(keyString)) {
      return null;
    }
    ancestor = parentKey(ancestor);
  }
  return null;
}

/** The patches of a key's level sharing an edge or a corner with it. */
function neighbours(key: PatchKey): PatchKey[] {
  const around = EDGES.map((e) => edgeNeighbour(key, e));
  for (const corner of cornerNeighbours(key)) {
    if (corner !== null) {
      around.push(corner);
    }
  }
  return around;
}

/**
 * Splits leaves until no two neighbours differ by more than one level, across face edges and at
 * cube corners included (the restricted quadtree of Design note 6).
 *
 * @remarks
 * A leaf two or more levels coarser than a neighbour of a leaf is removed, recorded as interior,
 * and each of its children handed to `expandChild`, which adds the leaves it selects there to
 * `leaves` and returns them; they are checked in turn. Splits only add, so the result does not
 * depend on the order of the work.
 *
 * @param leaves - The selected leaves by `patchKeyString`, a cut of the quadtree; changed in place.
 * @param internal - The refined patches by `patchKeyString`; changed in place.
 */
export function restrictQuadtree<T extends { readonly key: PatchKey }>(
  leaves: Map<string, T>,
  internal: Set<string>,
  expandChild: (child: PatchKey) => ReadonlyArray<T>,
): void {
  // Deepest last, so that the finest leaves are checked first.
  const work = [...leaves.values()].toSorted((a, b) => a.key.level - b.key.level);
  while (work.length > 0) {
    const leaf = work.pop();
    if (leaf === undefined || !leaves.has(patchKeyString(leaf.key))) {
      continue;
    }
    for (const n of neighbours(leaf.key)) {
      const coarse = coarserLeaf(leaves, internal, n);
      if (coarse === null || coarse.key.level >= leaf.key.level - 1) {
        continue;
      }
      const coarseString = patchKeyString(coarse.key);
      leaves.delete(coarseString);
      internal.add(coarseString);
      for (const child of childKeys(coarse.key)) {
        // Each child is selected as the traversal would: its own error may call for finer still.
        work.push(...expandChild(child));
      }
      // The split may still leave a leaf too coarse beside this one: look again.
      work.push(leaf);
      break;
    }
  }
}
