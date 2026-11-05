/**
 * The heights a terrain pass keeps of its resident bakes on the render thread (plan R10,
 * R10.T16.c): what the survey edge's rim and its note's line of sight read of the ground a view
 * draws, beside the GPU's own copy in the cache's slots, which the render thread cannot read.
 *
 * @remarks
 * - **The rim** ({@link ResidentHeights.rimHeightAtM}). The survey edge's mark stands at
 *   max(drawn height, 0) (R10.T10.c's `surveyEdgeMark`), the drawn height the resident bakes' whose
 *   survey mask holds the edge, a vertex of it surveyed and another not. Of each such bake the
 *   pass keeps its own-level heights whole, 65 × 65 `f32` (16,900 B), with its packed survey mask
 *   (`packSurveyMask`, 4,228 B), "their heights kept on the render thread for those patches alone"
 *   (the task's text); the rim at a body-fixed direction is the finest such bake's over it,
 *   bilinear between its quad's four vertices, and `null`, the datum, where none is resident. The
 *   edge runs along coarse cells' sides, which are patches' sides at the field's level and every
 *   finer one, so a point on a patch's side (within {@link BOUNDARY_QUADS}) reads either patch,
 *   across a face's edge too: neighbouring bakes of one level share their side's vertices, and so
 *   its heights there.
 * - **The ground** ({@link ResidentHeights.groundHeightAtM}), which hides an edge's point behind
 *   nearer terrain from the camera, so that the `SURVEY EDGE` note counts only the points the
 *   operator can see (the r10-shader handoff's "For R10.T16", item b): the finest resident bake's
 *   over the direction, the drawn ground: on its surveyed ground its own-level height, on its
 *   unsurveyed ground the datum, which the reference surface draws there (Design note 12). An edge
 *   bake reads its whole heights and its mask (`surveyedAt`, the discard's twin); a bake wholly
 *   surveyed a 17 × 17 grid of every fourth vertex's height (1,156 B) and as many of its morph
 *   targets, so that the copy of a cache's few hundred to thousand resident patches stays near two
 *   megabytes rather than the cache's heights again (7 to 22 MB a pass); a bake surveying nothing
 *   holds nothing, its ground the datum. A ridge narrower than four quads of a wholly surveyed
 *   patch is so missed: its patches are drawn where a quad is about a pixel (Design note 7), so a
 *   point it hides by a few pixels may count as seen.
 * - The morph to the parent's heights is not followed here: the rim and the ground are each
 *   bake's own level's, off the drawn ground by at most its selection error, under τ pixels, once
 *   its transitions settle. Each bake keeps its morph targets on the same grid, which the view's
 *   drawn ground (`drawnGround.ts`) reads as it follows the shader's mix while they run.
 * - Only a body's bakes are kept (their `layers`); R05's test planet keeps none.
 */

import type { Vec3 } from "../../geometry/vec3";
import type { BakedPatch } from "./workers/messages";
import { packSurveyMask, surveyCellCode, surveyedAt } from "./gpu/resources";
import { uvToSt } from "./cube";
import {
  type Face,
  FACES,
  parsePatchKeyString,
  type PatchKey,
  patchIndex,
  patchKeyIndex,
  unreachable,
} from "./patchKey";
import { PATCH_VERTICES_PER_SIDE } from "./slotLayout";

/** Quads along a patch's side. */
const QUADS = PATCH_VERTICES_PER_SIDE - 1;

/** The coarse grid's stride, vertices: every fourth of the patch's 65, a 17 × 17 grid. */
export const COARSE_HEIGHT_STRIDE = 4;

/** The coarse grid's vertices along a side. */
const COARSE_SIDE = QUADS / COARSE_HEIGHT_STRIDE + 1;

/**
 * How near a patch's side a point reads the patch across it too, quads: 10⁻⁶ of one, far above the
 * rounding of a cell's side's direction and far below a pixel.
 */
export const BOUNDARY_QUADS = 1e-6;

/** How near a face's edge a direction is placed on the face across it too, of its largest component. */
const FACE_EDGE_FRACTION = 1e-12;

/** A survey mask's byte of a surveyed vertex (Design note 12). */
const SURVEYED = 255;

/** How a bake's survey mask stands: every vertex surveyed, none, or both kinds (the edge). */
export type SurveyStanding = "surveyed" | "bare" | "edge";

/**
 * A resident bake's heights as the drawn ground's twin reads them (plan R10, R10.T16.c after
 * R10.T16.i; `drawnGround.ts`), at (x, y) in quads of its patch: an edge bake's at every vertex, a
 * surveyed one's on its coarse grid, a bare one's the datum's.
 */
export interface ResidentBake {
  /** How its survey mask stands: some vertices surveyed and some not (the edge), all, or none. */
  readonly standing: SurveyStanding;
  /** Its own-level height, m: bilinear between its held vertices. */
  readonly ownAtM: (x: number, y: number) => number;
  /** Its morph target, the parent level's surface at its vertices, m, likewise. */
  readonly morphAtM: (x: number, y: number) => number;
  /** Whether its survey mask holds the place surveyed (the discard's twin, `surveyedAt`). */
  readonly surveyedAt: (x: number, y: number) => boolean;
}

/** What is kept of one resident bake. */
interface HeldBake {
  readonly key: PatchKey;
  readonly standing: SurveyStanding;
  /** Its own-level heights, m: 65 × 65 for an edge bake, 17 × 17 for a surveyed one; none bare. */
  readonly heights: Float32Array | null;
  /** Its morph targets, m, on the same grid as {@link HeldBake.heights}. */
  readonly morph: Float32Array | null;
  /** An edge bake's packed survey mask with its cell code (`packSurveyMask`); otherwise `null`. */
  readonly mask: Uint8Array | null;
  /** Its baked heights' highest, m above the datum. */
  readonly topM: number;
}

/** A direction's place on a face: its cell coordinates (s, t), each in [0, 1]. */
export interface FacePlace {
  readonly face: Face;
  readonly s: number;
  readonly t: number;
}

/** A held bake found over a direction, with the direction's point in it, in quads. */
interface Found {
  readonly held: HeldBake;
  readonly x: number;
  readonly y: number;
}

/** What a view's survey edge and its note read of a pass's resident heights. */
export interface ResidentHeightsView {
  /**
   * The rim's drawn height above the datum at a body-fixed unit direction (its parametric one, the
   * spheroid's point M d), m: the finest resident bake's over it whose survey mask holds the edge;
   * `null` where none is.
   */
  rimHeightAtM(direction: Vec3): number | null;
  /**
   * The drawn ground's height above the datum at a body-fixed unit direction, m: the finest
   * resident bake's over it, the datum (0) on its unsurveyed ground; `null` where none is.
   */
  groundHeightAtM(direction: Vec3): number | null;
  /** The highest any resident bake stands above the datum, m; 0 with none, or none above it. */
  readonly ceilingM: number;
  /**
   * The rim's revision (R10.T16.c's cost): it moves whenever a bake whose survey mask holds the
   * edge comes or goes, and only then, so that a rim read under one revision stands while it does.
   */
  readonly rimRevision: number;
}

/** How a bake's 65 × 65 mask stands. */
function standingOf(mask: Uint8Array): SurveyStanding {
  let surveyed = false;
  let bare = false;
  for (const byte of mask) {
    if (byte === SURVEYED) {
      surveyed = true;
    } else {
      bare = true;
    }
    if (surveyed && bare) {
      return "edge";
    }
  }
  return surveyed ? "surveyed" : "bare";
}

/**
 * A bake's own-level heights (`which` 0) or morph targets (1), 65 × 65, from its interleaved own
 * and morph heights.
 */
function ownHeights(bake: BakedPatch, which: 0 | 1 = 0): Float32Array {
  const out = new Float32Array(PATCH_VERTICES_PER_SIDE * PATCH_VERTICES_PER_SIDE);
  for (let n = 0; n < out.length; n += 1) {
    out[n] = bake.heights[2 * n + which] ?? 0;
  }
  return out;
}

/** Every fourth vertex's own-level height (`which` 0) or morph target (1), 17 × 17. */
function coarseHeights(bake: BakedPatch, which: 0 | 1 = 0): Float32Array {
  const out = new Float32Array(COARSE_SIDE * COARSE_SIDE);
  for (let b = 0; b < COARSE_SIDE; b += 1) {
    for (let a = 0; a < COARSE_SIDE; a += 1) {
      const vertex = COARSE_HEIGHT_STRIDE * b * PATCH_VERTICES_PER_SIDE + COARSE_HEIGHT_STRIDE * a;
      out[b * COARSE_SIDE + a] = bake.heights[2 * vertex + which] ?? 0;
    }
  }
  return out;
}

/** The bilinear value of a `side` × `side` grid at (x, y) in its own spacing, each in [0, side − 1]. */
function bilinear(grid: Float32Array, side: number, x: number, y: number): number {
  const x0 = Math.min(side - 2, Math.max(0, Math.floor(x)));
  const y0 = Math.min(side - 2, Math.max(0, Math.floor(y)));
  const tx = Math.min(1, Math.max(0, x - x0));
  const ty = Math.min(1, Math.max(0, y - y0));
  if (tx === 0 && ty === 0) {
    // At a vertex, as the drawn ground's twin reads its triangles' corners.
    return grid[y0 * side + x0] ?? 0;
  }
  const at = (a: number, b: number): number => grid[b * side + a] ?? 0;
  const low = at(x0, y0) + tx * (at(x0 + 1, y0) - at(x0, y0));
  const high = at(x0, y0 + 1) + tx * (at(x0 + 1, y0 + 1) - at(x0, y0 + 1));
  return low + ty * (high - low);
}

/** A held bake's own-level height at (x, y) in quads of its patch, or its morph target's. */
function heightOf(held: HeldBake, x: number, y: number, grid = held.heights): number {
  if (grid === null) {
    return 0;
  }
  return held.standing === "edge"
    ? bilinear(grid, PATCH_VERTICES_PER_SIDE, x, y)
    : bilinear(grid, COARSE_SIDE, x / COARSE_HEIGHT_STRIDE, y / COARSE_HEIGHT_STRIDE);
}

/** `held` as the drawn ground's twin reads it. */
function residentBakeOf(held: HeldBake): ResidentBake {
  return {
    standing: held.standing,
    ownAtM: (x, y) => heightOf(held, x, y),
    morphAtM: (x, y) => heightOf(held, x, y, held.morph),
    surveyedAt: (x, y) =>
      held.standing === "surveyed" ||
      (held.standing === "edge" && held.mask !== null && surveyedAt(held.mask, x, y)),
  };
}

/** The axis component, signed outward, that a face's points have largest (`faceUvToXyz`'s). */
function outwardOf(face: Face, x: number, y: number, z: number): number {
  switch (face) {
    case 0:
      return x;
    case 1:
      return y;
    case 2:
      return z;
    case 3:
      return -x;
    case 4:
      return -y;
    case 5:
      return -z;
  }
  return unreachable(face);
}

/** The (u, v) on `face` of the direction (x, y, z), as `xyzToFaceUv` takes them on its own face. */
function uvOn(face: Face, x: number, y: number, z: number): readonly [number, number] {
  switch (face) {
    case 0:
      return [y / x, z / x];
    case 1:
      return [-x / y, z / y];
    case 2:
      return [-x / z, -y / z];
    case 3:
      return [z / x, y / x];
    case 4:
      return [z / y, -x / y];
    case 5:
      return [-y / z, -x / z];
  }
  return unreachable(face);
}

/**
 * The faces `direction` lies on, with its cell coordinates on each: its own, and on a face's edge
 * or a cube's corner the faces across it.
 */
export function facePlaces(direction: Vec3): FacePlace[] {
  const { x, y, z } = direction;
  const largest = Math.max(Math.abs(x), Math.abs(y), Math.abs(z));
  if (!(largest > 0)) {
    return [];
  }
  const places: FacePlace[] = [];
  for (const face of FACES) {
    if (outwardOf(face, x, y, z) >= largest * (1 - FACE_EDGE_FRACTION)) {
      const [u, v] = uvOn(face, x, y, z);
      places.push({
        face,
        s: Math.min(1, Math.max(0, uvToSt(Math.min(1, Math.max(-1, u))))),
        t: Math.min(1, Math.max(0, uvToSt(Math.min(1, Math.max(-1, v))))),
      });
    }
  }
  return places;
}

/**
 * The `k`th patch column (or row) of `n` along a face holding `s`, k from 0: its own, then its
 * neighbour on its side where it lies within {@link BOUNDARY_QUADS} of it; −1 past them. Read by
 * index, so that a lookup builds no array of columns (R10.T16.c's cost).
 */
export function columnAt(s: number, n: number, k: number): number {
  const at = s * n;
  const i = Math.min(n - 1, Math.floor(at));
  if (k === 0) {
    return i;
  }
  const fraction = (at - i) * QUADS;
  const below = fraction <= BOUNDARY_QUADS && i > 0;
  const above = fraction >= QUADS - BOUNDARY_QUADS && i < n - 1;
  if (k === 1) {
    return below ? i - 1 : above ? i + 1 : -1;
  }
  return k === 2 && below && above ? i + 1 : -1;
}

/** One level's held bakes, as the lookups read them. */
interface HeldLevel {
  /** The level's patches along a face's side. */
  readonly n: number;
  /** Its bakes, by `patchKeyIndex`. */
  readonly bakes: ReadonlyMap<number, HeldBake>;
}

/** Held bakes by level, then by `patchKeyIndex`, read finest first. */
class LevelIndex {
  readonly #levels = new Map<number, Map<number, HeldBake>>();
  /** The levels held, finest first. */
  #ordered: ReadonlyArray<HeldLevel> = [];

  /** How many bakes it holds. */
  get size(): number {
    let size = 0;
    for (const level of this.#levels.values()) {
      size += level.size;
    }
    return size;
  }

  *bakes(): Generator<HeldBake> {
    for (const level of this.#levels.values()) {
      yield* level.values();
    }
  }

  set(held: HeldBake): void {
    let level = this.#levels.get(held.key.level);
    if (level === undefined) {
      level = new Map();
      this.#levels.set(held.key.level, level);
      this.#reorder();
    }
    level.set(patchKeyIndex(held.key), held);
  }

  /** The bake held for `key`, if one is. */
  get(key: PatchKey): HeldBake | undefined {
    return this.#levels.get(key.level)?.get(patchKeyIndex(key));
  }

  delete(key: PatchKey): boolean {
    const level = this.#levels.get(key.level);
    if (level === undefined || !level.delete(patchKeyIndex(key))) {
      return false;
    }
    if (level.size === 0) {
      this.#levels.delete(key.level);
      this.#reorder();
    }
    return true;
  }

  clear(): void {
    this.#levels.clear();
    this.#reorder();
  }

  /** Orders the levels held, finest first. */
  #reorder(): void {
    this.#ordered = [...this.#levels.keys()]
      .toSorted((a, b) => b - a)
      .flatMap((level) => {
        const bakes = this.#levels.get(level);
        return bakes === undefined ? [] : [{ n: 2 ** level, bakes }];
      });
  }

  /** The finest held bake over any of `places`, with its point in it, or `null`. */
  finest(places: ReadonlyArray<FacePlace>): Found | null {
    for (const { n, bakes } of this.#ordered) {
      for (const { face, s, t } of places) {
        for (let a = 0; columnAt(s, n, a) >= 0; a += 1) {
          const i = columnAt(s, n, a);
          for (let b = 0; columnAt(t, n, b) >= 0; b += 1) {
            const j = columnAt(t, n, b);
            const held = bakes.get(patchIndex(face, i, j));
            if (held !== undefined) {
              return {
                held,
                x: Math.min(QUADS, Math.max(0, (s * n - i) * QUADS)),
                y: Math.min(QUADS, Math.max(0, (t * n - j) * QUADS)),
              };
            }
          }
        }
      }
    }
    return null;
  }
}

/** A pass's resident heights (see the module's remarks), kept as its bakes land and leave. */
export class ResidentHeights implements ResidentHeightsView {
  /** Every held bake. */
  readonly #all = new LevelIndex();
  /** The held bakes whose survey mask holds the edge. */
  readonly #edges = new LevelIndex();
  #ceilingM: number | null = 0;
  #rimRevision = 0;
  #revision = 0;
  /** Each held bake as the drawn ground's twin reads it, made at its first reading. */
  readonly #bakes = new WeakMap<HeldBake, ResidentBake>();

  get rimRevision(): number {
    return this.#rimRevision;
  }

  /**
   * Moves whenever any bake comes or goes (lane perf-render-thread-ground), so that what the drawn
   * ground's twin read of the bakes under one revision stands while it does.
   */
  get revision(): number {
    return this.#revision;
  }

  /** The bytes the copies hold, for R10.T14's memory line. */
  get bytes(): number {
    let bytes = 0;
    for (const held of this.#all.bakes()) {
      bytes +=
        (held.heights?.byteLength ?? 0) +
        (held.morph?.byteLength ?? 0) +
        (held.mask?.byteLength ?? 0);
    }
    return bytes;
  }

  /** How many held bakes hold the edge, their heights whole. */
  get edgeBakes(): number {
    return this.#edges.size;
  }

  get ceilingM(): number {
    if (this.#ceilingM === null) {
      let top = 0;
      for (const held of this.#all.bakes()) {
        top = held.standing === "bare" ? top : Math.max(top, held.topM);
      }
      this.#ceilingM = top;
    }
    return this.#ceilingM;
  }

  /** Keeps what the rim and the ground read of a body's bake, in place of any held for its key. */
  take(bake: BakedPatch): void {
    const layers = bake.layers;
    if (layers === null) {
      return;
    }
    const standing = standingOf(layers.surveyMask);
    let held: HeldBake;
    switch (standing) {
      case "edge":
        held = {
          key: bake.key,
          standing,
          heights: ownHeights(bake),
          morph: ownHeights(bake, 1),
          mask: packSurveyMask(
            layers.surveyMask,
            surveyCellCode(bake.key.level, layers.fieldLevel),
          ),
          topM: bake.heightRangeM[1],
        };
        break;
      case "surveyed":
        held = {
          key: bake.key,
          standing,
          heights: coarseHeights(bake),
          morph: coarseHeights(bake, 1),
          mask: null,
          topM: bake.heightRangeM[1],
        };
        break;
      case "bare":
        held = { key: bake.key, standing, heights: null, morph: null, mask: null, topM: 0 };
        break;
    }
    this.#all.set(held);
    this.#revision += 1;
    if (standing === "edge") {
      this.#edges.set(held);
      this.#rimRevision += 1;
    } else if (this.#edges.delete(bake.key)) {
      this.#rimRevision += 1;
    }
    this.#ceilingM = null;
  }

  /** Forgets the bake of the patch keyed `keyString` (a `patchKeyString`), if one is held. */
  forget(keyString: string): void {
    const key = parsePatchKeyString(keyString);
    if (key === null || !this.#all.delete(key)) {
      return;
    }
    this.#revision += 1;
    if (this.#edges.delete(key)) {
      this.#rimRevision += 1;
    }
    this.#ceilingM = null;
  }

  /** Forgets every bake: a device rebuilt, or the pass disposed. */
  clear(): void {
    this.#all.clear();
    this.#edges.clear();
    this.#ceilingM = 0;
    this.#rimRevision += 1;
    this.#revision += 1;
  }

  /** The resident bake of `key` as the drawn ground's twin reads it, or `null` with none. */
  bakeAt(key: PatchKey): ResidentBake | null {
    const held = this.#all.get(key);
    if (held === undefined) {
      return null;
    }
    let bake = this.#bakes.get(held);
    if (bake === undefined) {
      bake = residentBakeOf(held);
      this.#bakes.set(held, bake);
    }
    return bake;
  }

  rimHeightAtM(direction: Vec3): number | null {
    const found = this.#edges.finest(facePlaces(direction));
    return found === null ? null : heightOf(found.held, found.x, found.y);
  }

  groundHeightAtM(direction: Vec3): number | null {
    const found = this.#all.finest(facePlaces(direction));
    if (found === null) {
      return null;
    }
    const { held, x, y } = found;
    let heightM: number;
    switch (held.standing) {
      case "bare":
        heightM = 0;
        break;
      case "surveyed":
        heightM = heightOf(held, x, y);
        break;
      case "edge":
        heightM = held.mask !== null && surveyedAt(held.mask, x, y) ? heightOf(held, x, y) : 0;
        break;
    }
    return heightM;
  }
}
