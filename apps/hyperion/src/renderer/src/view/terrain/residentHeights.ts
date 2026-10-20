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
 *   surveyed a 17 × 17 grid of every fourth vertex's height (1,156 B), so that the copy of a
 *   cache's few hundred to thousand resident patches stays near a megabyte rather than the cache's
 *   heights again (7 to 22 MB a pass); a bake surveying nothing holds nothing, its ground the
 *   datum. A ridge narrower than four quads of a wholly surveyed patch is so missed: its patches
 *   are drawn where a quad is about a pixel (Design note 7), so a point it hides by a few pixels
 *   may count as seen.
 * - The morph to the parent's heights is not followed: the rim and the ground are each bake's own
 *   level's, off the drawn ground by at most its selection error, under τ pixels.
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
type SurveyStanding = "surveyed" | "bare" | "edge";

/** What is kept of one resident bake. */
interface HeldBake {
  readonly key: PatchKey;
  readonly standing: SurveyStanding;
  /** Its own-level heights, m: 65 × 65 for an edge bake, 17 × 17 for a surveyed one; none bare. */
  readonly heights: Float32Array | null;
  /** An edge bake's packed survey mask with its cell code (`packSurveyMask`); otherwise `null`. */
  readonly mask: Uint8Array | null;
  /** Its baked heights' highest, m above the datum. */
  readonly topM: number;
}

/** A direction's place on a face: its cell coordinates (s, t), each in [0, 1]. */
interface FacePlace {
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

/** A bake's own-level heights, 65 × 65, from its interleaved own and morph heights. */
function ownHeights(bake: BakedPatch): Float32Array {
  const out = new Float32Array(PATCH_VERTICES_PER_SIDE * PATCH_VERTICES_PER_SIDE);
  for (let n = 0; n < out.length; n += 1) {
    out[n] = bake.heights[2 * n] ?? 0;
  }
  return out;
}

/** Every fourth vertex's own-level height, 17 × 17. */
function coarseHeights(bake: BakedPatch): Float32Array {
  const out = new Float32Array(COARSE_SIDE * COARSE_SIDE);
  for (let b = 0; b < COARSE_SIDE; b += 1) {
    for (let a = 0; a < COARSE_SIDE; a += 1) {
      const vertex = COARSE_HEIGHT_STRIDE * b * PATCH_VERTICES_PER_SIDE + COARSE_HEIGHT_STRIDE * a;
      out[b * COARSE_SIDE + a] = bake.heights[2 * vertex] ?? 0;
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
  const at = (a: number, b: number): number => grid[b * side + a] ?? 0;
  const low = at(x0, y0) + tx * (at(x0 + 1, y0) - at(x0, y0));
  const high = at(x0, y0 + 1) + tx * (at(x0 + 1, y0 + 1) - at(x0, y0 + 1));
  return low + ty * (high - low);
}

/** A held bake's own-level height at (x, y) in quads of its patch. */
function heightOf(held: HeldBake, x: number, y: number): number {
  const heights = held.heights;
  if (heights === null) {
    return 0;
  }
  return held.standing === "edge"
    ? bilinear(heights, PATCH_VERTICES_PER_SIDE, x, y)
    : bilinear(heights, COARSE_SIDE, x / COARSE_HEIGHT_STRIDE, y / COARSE_HEIGHT_STRIDE);
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
function facePlaces(direction: Vec3): FacePlace[] {
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

/** The patch columns (or rows) of `n` along a face holding `s`, its neighbour too on its side. */
function columnsAt(s: number, n: number): number[] {
  const at = s * n;
  const i = Math.min(n - 1, Math.floor(at));
  const fraction = (at - i) * QUADS;
  const columns = [i];
  if (fraction <= BOUNDARY_QUADS && i > 0) {
    columns.push(i - 1);
  }
  if (fraction >= QUADS - BOUNDARY_QUADS && i < n - 1) {
    columns.push(i + 1);
  }
  return columns;
}

/** Held bakes by level, then by `patchKeyIndex`, read finest first. */
class LevelIndex {
  readonly #levels = new Map<number, Map<number, HeldBake>>();
  /** The levels held, finest first. */
  #order: number[] = [];

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
      this.#order = [...this.#levels.keys()].toSorted((a, b) => b - a);
    }
    level.set(patchKeyIndex(held.key), held);
  }

  delete(key: PatchKey): boolean {
    const level = this.#levels.get(key.level);
    if (level === undefined || !level.delete(patchKeyIndex(key))) {
      return false;
    }
    if (level.size === 0) {
      this.#levels.delete(key.level);
      this.#order = this.#order.filter((each) => each !== key.level);
    }
    return true;
  }

  clear(): void {
    this.#levels.clear();
    this.#order = [];
  }

  /** The finest held bake over any of `places`, with its point in it, or `null`. */
  finest(places: ReadonlyArray<FacePlace>): Found | null {
    for (const levelNumber of this.#order) {
      const level = this.#levels.get(levelNumber);
      if (level === undefined) {
        continue;
      }
      const n = 2 ** levelNumber;
      for (const { face, s, t } of places) {
        for (const i of columnsAt(s, n)) {
          for (const j of columnsAt(t, n)) {
            const held = level.get(patchKeyIndex({ face, level: levelNumber, i, j }));
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

  /** The bytes the copies hold, for R10.T14's memory line. */
  get bytes(): number {
    let bytes = 0;
    for (const held of this.#all.bakes()) {
      bytes += (held.heights?.byteLength ?? 0) + (held.mask?.byteLength ?? 0);
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
          mask: null,
          topM: bake.heightRangeM[1],
        };
        break;
      case "bare":
        held = { key: bake.key, standing, heights: null, mask: null, topM: 0 };
        break;
    }
    this.#all.set(held);
    if (standing === "edge") {
      this.#edges.set(held);
    } else {
      this.#edges.delete(bake.key);
    }
    this.#ceilingM = null;
  }

  /** Forgets the bake of the patch keyed `keyString` (a `patchKeyString`), if one is held. */
  forget(keyString: string): void {
    const key = parsePatchKeyString(keyString);
    if (key === null || !this.#all.delete(key)) {
      return;
    }
    this.#edges.delete(key);
    this.#ceilingM = null;
  }

  /** Forgets every bake: a device rebuilt, or the pass disposed. */
  clear(): void {
    this.#all.clear();
    this.#edges.clear();
    this.#ceilingM = 0;
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
