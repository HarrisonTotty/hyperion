/**
 * The disc's normal map (plan R10, T10.g; decision-r10-t16-leans §1, remedy 1): the field's own
 * shading normals over its surveyed cells, at the field's level, packed as R07's disc reads them, its
 * texture's construction and its CPU twin.
 *
 * @remarks
 * A worker answers each surveyed cell of the body's field (level L, 5 to 8) with the shading normal
 * the coarsest terrain gives that cell's centre (the surface crate's `WorkerFields::normal_map`: R09's
 * height and gradient at band max(L − 6, 0), the doubled bake's odd–odd sample), as the octahedral
 * pair of a body-fixed unit vector, narrowed to half floats as a bake's normals are (`packNormals`).
 *
 * - **The layout** is the class map's (`discSurface.ts`) at N = 2^L texels a face side: a 2D array of
 *   six layers, one a face, each (N + 2) × (N + 2) texels of {@link NORMAL_MAP_FORMAT}, face texel
 *   (i, j), the cell (f, L, i, j), at (i + 1, j + 1), and a one-texel gutter about it holding the
 *   neighbouring faces' edge texels ({@link forEachGutterTexel}, shared with the class map). The
 *   pair is in the body-fixed axes, which every face shares, so a gutter texel is its neighbour's
 *   pair as it stands. The gutter's corners and every unsurveyed texel hold {@link NO_NORMAL}.
 * - **The reconstruction** ({@link normalMapNormalAt}, the twin of the shader's) is the class map's
 *   survey-masked bilinear one, on the decoded vectors: nothing where the texel holding the point is
 *   unsurveyed (the datum's normal shades there); otherwise each of the four texels whose centres
 *   are nearest, the surveyed ones alone, decoded (`decodeOctahedral`, the terrain's own), weighted
 *   by its bilinear weight, summed and renormalised. Filtering the vectors rather than the pairs
 *   keeps the octahedron's fold out of the mean, as the terrain's `patchNormalAt` does.
 * - **The cost** is 4 bytes a texel: 6 (N + 2)² × 4 bytes, 27,744 B at level 5, 104,544 B at 6,
 *   405,600 B at 7 and 1,597,536 B (1.52 MiB) at 8, filed under `coarse-field-gpu` with the class map.
 */
import { type Vec3, vec3 } from "../../geometry/vec3";
import type { DiscNormalMap } from "../appearance/bodyAppearance";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { TextureSpec } from "../engine/memory";
import type { RenderEngine } from "../engine/types";
import { fieldCellAt, MAX_FIELD_LEVEL, MIN_FIELD_LEVEL } from "../terrain/coverage";
import { uvToSt, type Xyz, xyzToFaceUv } from "../terrain/cube";
import type { Face } from "../terrain/patchKey";
import { decodeOctahedral } from "../terrain/shadingNormal";
import { classMapSide, forEachGutterTexel } from "./discSurface";

/** The normal map's texel format: an octahedral pair as two half floats, as the terrain's normals. */
export const NORMAL_MAP_FORMAT = "rg16float" as const;

/**
 * The pair an unsurveyed texel and the gutter's corners hold: (2, 2), outside the [−1, 1]² every
 * octahedral pair lies in, and exact in a half float. A texel whose first coordinate's magnitude is
 * above {@link NO_NORMAL_ABOVE} holds no normal.
 */
export const NO_NORMAL: readonly [number, number] = [2, 2];

/**
 * The magnitude above which a texel's first coordinate reads as {@link NO_NORMAL}: 1.5, halfway
 * between a real pair's largest, 1, and the sentinel's 2, far past a half float's rounding of either.
 */
export const NO_NORMAL_ABOVE = 1.5;

/** The cube sphere's faces: a layer each. */
const FACES = 6;

/** The values a texel holds: the pair's two coordinates. */
const PAIR = 2;

/** A normal map's texels as they are uploaded. */
export interface NormalMapTexels {
  /** N = 2^L, the texels along each face's side, the gutter apart: 32 to 256. */
  readonly faceTexels: number;
  /**
   * Layer after layer, one a face, each N + 2 rows y of N + 2 texels x ({@link classMapSide}), two
   * half floats a texel, the octahedral pair (x, y) of a body-fixed unit normal: face texel (i, j) at
   * (i + 1, j + 1), the gutter's sides the neighbouring faces' edge texels, its corners and every
   * unsurveyed texel {@link NO_NORMAL}.
   */
  readonly data: Float16Array;
}

/** The index in `map.data` of the first coordinate of stored texel (x, y) of `face`'s layer. */
function pairOffset(map: NormalMapTexels, face: Face, x: number, y: number): number {
  const side = classMapSide(map.faceTexels);
  return ((face * side + y) * side + x) * PAIR;
}

/** Whether a texel's first coordinate `x` is {@link NO_NORMAL}'s. */
export function isNoNormal(x: number): boolean {
  return Math.abs(x) > NO_NORMAL_ABOVE;
}

/**
 * Packs a worker's normal map in the layout the disc reads, its gutter from each face's neighbours.
 *
 * @param level - The field's level L, 5 to 8, whose cells `cells` names: N = 2^L.
 * @param cells - The cell index at `level` (`fieldCellIndex`'s) of each cell answered, each once.
 * @param pairs - Each cell's octahedral pair, two values a cell in `cells`' order, each in [−1, 1].
 * @throws Error for a level outside 5 to 8, pairs that are not two a cell, a cell index past the
 *   level's cells, or a coordinate outside [−1, 1].
 */
export function packNormalMap(
  level: number,
  cells: ArrayLike<number>,
  pairs: ArrayLike<number>,
): NormalMapTexels {
  if (!(Number.isInteger(level) && level >= MIN_FIELD_LEVEL && level <= MAX_FIELD_LEVEL)) {
    throw new Error(
      `a normal map is at a field's level, ${String(MIN_FIELD_LEVEL)} to ${String(MAX_FIELD_LEVEL)}, not ${String(level)}`,
    );
  }
  if (pairs.length !== PAIR * cells.length) {
    throw new Error(
      `a normal map of ${String(cells.length)} cells has ${String(pairs.length)} pair values`,
    );
  }
  const faceTexels = 2 ** level;
  const side = classMapSide(faceTexels);
  const map: NormalMapTexels = {
    faceTexels,
    data: new Float16Array(FACES * side * side * PAIR).fill(NO_NORMAL[0]),
  };
  const count = FACES * 4 ** level;
  for (let k = 0; k < cells.length; k += 1) {
    const index = cells[k] ?? -1;
    if (!(Number.isInteger(index) && index >= 0 && index < count)) {
      throw new Error(`a normal map at level ${String(level)} has no cell ${String(index)}`);
    }
    const x = pairs[PAIR * k] ?? NaN;
    const y = pairs[PAIR * k + 1] ?? NaN;
    if (!(Math.abs(x) <= 1 && Math.abs(y) <= 1)) {
      throw new Error(
        `cell ${String(index)}'s pair (${String(x)}, ${String(y)}) has a coordinate outside [-1, 1]`,
      );
    }
    const cell = fieldCellAt(level, index);
    const at = pairOffset(map, cell.face, cell.i + 1, cell.j + 1);
    map.data[at] = x;
    map.data[at + 1] = y;
  }
  forEachGutterTexel(faceTexels, (face, x, y, from) => {
    const to = pairOffset(map, face, x, y);
    const source = pairOffset(map, from.face, from.i + 1, from.j + 1);
    map.data[to] = map.data[source] ?? NO_NORMAL[0];
    map.data[to + 1] = map.data[source + 1] ?? NO_NORMAL[1];
  });
  return map;
}

/**
 * The specification of a normal map's texture: (N + 2) × (N + 2) texels a layer, the gutter
 * included, six layers, filed under `coarse-field-gpu` beside the class map (R12 Design note 6's
 * coarse field's GPU copy; R10.T10.g).
 */
export function normalMapTextureSpec(name: string, faceTexels: number): TextureSpec {
  const side = classMapSide(faceTexels);
  return {
    name,
    size: { width: side, height: side, depthOrArrayLayers: FACES },
    dimension: "2d",
    format: NORMAL_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "coarse-field-gpu",
  };
}

/**
 * A disc's normal map: `texels` uploaded as a texture named `name`, carried with them.
 *
 * @remarks
 * The texture is the caller's, as a class map's is: made again after a device loss
 * (`onRestored`), and released with `releaseTexture` when the map is dropped.
 */
export function normalMapTexture(
  engine: Pick<RenderEngine, "createTexture" | "writeTexture">,
  name: string,
  texels: NormalMapTexels,
): DiscNormalMap {
  const texture = engine.createTexture(normalMapTextureSpec(name, texels.faceTexels));
  const side = classMapSide(texels.faceTexels);
  engine.writeTexture(
    texture,
    { x: 0, y: 0, z: 0 },
    { width: side, height: side, depthOrArrayLayers: FACES },
    texels.data,
  );
  return { texture, texels };
}

/** The decoded normal at stored texel (x, y) of `face`'s layer, or `null` where it holds none. */
function storedNormal(map: NormalMapTexels, face: Face, x: number, y: number): Vec3 | null {
  const at = pairOffset(map, face, x, y);
  const px = map.data[at] ?? NO_NORMAL[0];
  if (isNoNormal(px)) {
    return null;
  }
  return decodeOctahedral(px, map.data[at + 1] ?? NO_NORMAL[1]);
}

/**
 * The normal map's shading normal at a body-fixed direction, by the survey-masked bilinear
 * reconstruction of the decoded vectors (the shader's twin, R10.T10.g), or `null` where the texel
 * holding the direction is unsurveyed.
 *
 * @remarks
 * With (s, t) the direction's cell coordinates on its face and n = N, as `classMapSharesAt` takes
 * them: `null` where the texel holding it, (⌊n s⌋, ⌊n t⌋), holds no normal; otherwise each of the
 * four texels whose centres are nearest, from (⌊n s − ½⌋, ⌊n t − ½⌋), the gutter's among them at a
 * face's edge, by its bilinear weight in (n s − ½, n t − ½), those holding a normal alone, each its
 * decoded unit vector, summed and renormalised to unit length. The texel holding the point is one of
 * the four, at a bilinear weight of at least ¼; at a surveyed texel's centre the normal is its own.
 * Should the weighted sum vanish (normals that cancel, which no ground's do), the holding texel's
 * own normal stands.
 *
 * @param direction - Along the body-fixed axes, z the pole; it need not be unit.
 * @returns A body-fixed unit vector, or `null` for the datum's.
 */
export function normalMapNormalAt(map: NormalMapTexels, direction: Xyz): Vec3 | null {
  const n = map.faceTexels;
  const { face, u, v } = xyzToFaceUv(direction);
  const sN = uvToSt(u) * n;
  const tN = uvToSt(v) * n;
  const hi = Math.min(Math.max(Math.floor(sN), 0), n - 1);
  const hj = Math.min(Math.max(Math.floor(tN), 0), n - 1);
  const holding = storedNormal(map, face, hi + 1, hj + 1);
  if (holding === null) {
    return null;
  }
  const x = sN - 0.5;
  const y = tN - 0.5;
  const i0 = Math.min(Math.max(Math.floor(x), -1), n - 1);
  const j0 = Math.min(Math.max(Math.floor(y), -1), n - 1);
  const fx = x - i0;
  const fy = y - j0;
  let sx = 0;
  let sy = 0;
  let sz = 0;
  for (let corner = 0; corner < 4; corner += 1) {
    const di = corner % 2;
    const dj = Math.floor(corner / 2);
    const normal = storedNormal(map, face, i0 + di + 1, j0 + dj + 1);
    if (normal === null) {
      continue;
    }
    const b = (di === 0 ? 1 - fx : fx) * (dj === 0 ? 1 - fy : fy);
    sx += b * normal.x;
    sy += b * normal.y;
    sz += b * normal.z;
  }
  const length = Math.hypot(sx, sy, sz);
  return length > 0 ? vec3(sx / length, sy / length, sz / length) : holding;
}
