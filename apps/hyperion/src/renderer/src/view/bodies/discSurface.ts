/**
 * The disc's class-map surface (plan R07, T8.b; Design note 24): the layout of R10's coarse
 * class-weights map as `shaders/bodyDisc.wgsl` reads it, its construction and its CPU twin.
 *
 * @remarks
 * A class map holds, for each cell of R05's cube sphere at one level, the weight of each of the
 * body's classes over the cell, surveyed cells only (R10 Design note 8; R10.T10.d fills it). The
 * texture is a 2D array of {@link CLASS_MAP_FORMAT} texels, (N + 2) × (N + 2) per face (R10.T10.d,
 * decision-r07-t8b): the face's N × N texels and a one-texel gutter about them. Face texel (i, j)
 * of face f, stored at (i + 1, j + 1), is the cell s ∈ [i ÷ N, (i + 1) ÷ N),
 * t ∈ [j ÷ N, (j + 1) ÷ N) of R05's face coordinates (u, v) (`xyzToFaceUv`) under S2's quadratic
 * warp (`uvToSt`), so with N = 2^L it is the quadtree's cell (f, L, i, j). The gutter's sides hold
 * the edge texels of the neighbouring faces across each edge, and its four corners no weight. Class
 * k's weight is channel k mod 4 of layer f + 6 ⌊k ÷ 4⌋. An unsurveyed texel holds no weight, and a
 * surveyed one weights that sum to its surveyed share (1 where it is surveyed whole).
 *
 * At a point the disc shades with each class's law by its share and with `elsewhere` by the rest
 * ({@link classMapSharesAt}), through a survey-masked bilinear reconstruction: a point in an
 * unsurveyed texel takes `elsewhere` alone, so the disc shows no pattern past the survey's cells;
 * a point in a surveyed texel takes the share vector (`elsewhere`'s 1 − Σ w and each class's w,
 * {@link surfaceShares}) interpolated over the four nearest texel centres, counting surveyed texels
 * alone and renormalised, so the cells show no blocks; the gutter carries it across a face's edge.
 */
import type { ClassMapDiscSurface, DiscSurface } from "../appearance/bodyAppearance";
import type { PhotometricLaw } from "../appearance/law";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { TextureSpec } from "../engine/memory";
import type { RenderEngine, TextureHandle } from "../engine/types";
import { faceUvToXyz, stToUv, uvToSt, type Xyz, xyzToFaceUv } from "../terrain/cube";
import type { Face } from "../terrain/patchKey";

/** The classes one disc's class map may hold; the record and the shader carry this many laws. */
export const MAX_DISC_CLASSES = 16;

/** The class map's texel format: four classes' weights, 0 to 1 in steps of 1 ÷ 255. */
export const CLASS_MAP_FORMAT = "rgba8unorm" as const;

/** Classes per layer: one in each channel. */
export const CLASSES_PER_LAYER = 4;

/** The cube sphere's faces. */
const FACES = 6;

/** The cube sphere's faces, in order. */
const FACE_INDICES: ReadonlyArray<Face> = [0, 1, 2, 3, 4, 5];

/** The largest weight a texel's byte holds. */
const UNORM_MAX = 255;

/**
 * How far past a face's edge, in face coordinates, the gutter looks for the neighbouring face's
 * texel: a nudge off the edge, where a point lies on both faces, onto the neighbour's side.
 */
const ACROSS_EDGE = 1e-9;

/** The layers of a class map of `classes` classes: six faces for every four classes. */
export function classMapLayers(classes: number): number {
  return FACES * Math.ceil(classes / CLASSES_PER_LAYER);
}

/** The texels along a layer's side for N texels along a face's: N and a gutter texel each side. */
export function classMapSide(faceTexels: number): number {
  return faceTexels + 2;
}

/** A class map's texels as they are uploaded. */
export interface ClassMapTexels {
  /** N, the texels along each face's side, the gutter apart. */
  readonly faceTexels: number;
  /** The body's classes, one law each. */
  readonly classes: number;
  /**
   * Layer after layer ({@link classMapLayers}), each N + 2 rows y of N + 2 texels x
   * ({@link classMapSide}), four bytes a texel: class 4g + c's weight × 255 in channel c of layer
   * f + 6g. Face texel (i, j) is at (i + 1, j + 1); the gutter's sides hold the neighbouring faces'
   * edge texels, and its corners nothing.
   */
  readonly data: Uint8Array;
}

/** One texel of a class map: a face and a cell on it. */
export interface ClassMapTexel {
  readonly face: Face;
  /** The column, from 0 at u = −1. */
  readonly i: number;
  /** The row, from 0 at v = −1. */
  readonly j: number;
}

/** A cell coordinate s ∈ [0, 1] as a texel index of N. */
function texelIndex(s: number, faceTexels: number): number {
  return Math.min(Math.max(Math.floor(s * faceTexels), 0), faceTexels - 1);
}

/**
 * The texel holding a body-fixed direction: R05's face and (u, v) there, warped to (s, t) and
 * scaled by N, as `class_map_texel` does in the shader.
 *
 * @param direction - Along the body-fixed axes, z the pole; it need not be unit.
 */
export function classMapTexelOf(direction: Xyz, faceTexels: number): ClassMapTexel {
  const { face, u, v } = xyzToFaceUv(direction);
  return {
    face,
    i: texelIndex(uvToSt(u), faceTexels),
    j: texelIndex(uvToSt(v), faceTexels),
  };
}

/** The byte offset of class `k`'s weight at stored texel (x, y) of `face`'s layers, gutter included. */
function storedOffset(map: ClassMapTexels, face: Face, x: number, y: number, k: number): number {
  const side = classMapSide(map.faceTexels);
  const layer = face + FACES * Math.floor(k / CLASSES_PER_LAYER);
  return ((layer * side + y) * side + x) * CLASSES_PER_LAYER + (k % CLASSES_PER_LAYER);
}

/** The byte offset of class `k`'s weight at a face texel. */
function weightOffset(map: ClassMapTexels, texel: ClassMapTexel, k: number): number {
  return storedOffset(map, texel.face, texel.i + 1, texel.j + 1, k);
}

/**
 * One texel's weights as bytes that keep their sum: each w × 255 rounded down, and the bytes that
 * leaves short of round(255 Σ w) given to the largest remainders (ties to the lower class).
 */
function weightBytes(weights: ReadonlyArray<number>): number[] {
  const scaled = weights.map((w) => Math.min(Math.max(w, 0), 1) * UNORM_MAX);
  const total = Math.min(UNORM_MAX, Math.round(scaled.reduce((sum, w) => sum + w, 0)));
  const bytes = scaled.map((w) => Math.floor(w));
  let short = total - bytes.reduce((sum, b) => sum + b, 0);
  const order = scaled
    .map((w, k) => ({ k, remainder: w - Math.floor(w) }))
    .toSorted((a, b) => (b.remainder !== a.remainder ? b.remainder - a.remainder : a.k - b.k));
  for (const { k } of order) {
    if (short <= 0) {
      break;
    }
    bytes[k] = (bytes[k] ?? 0) + 1;
    short -= 1;
  }
  return bytes;
}

/**
 * The face texel across the edge from gutter texel (x, y) of `face`, x or y being 0 or N + 1 and
 * the other inside the face's N: the neighbouring face's texel at the point just past the edge
 * beside that edge texel's centre, which is the neighbour's own edge texel.
 */
function acrossEdge(face: Face, x: number, y: number, faceTexels: number): ClassMapTexel {
  const n = faceTexels;
  const past = 1 + ACROSS_EDGE;
  const along = (index: number): number => stToUv((index - 0.5) / n);
  let u: number;
  let v: number;
  if (x === 0 || x === n + 1) {
    u = x === 0 ? -past : past;
    v = along(y);
  } else {
    u = along(x);
    v = y === 0 ? -past : past;
  }
  return classMapTexelOf(faceUvToXyz(face, u, v), n);
}

/**
 * Packs a class map from each texel's weights, in the layout the shader reads, its gutter from
 * each face's neighbours.
 *
 * @remarks
 * A texel's weights are each class's share of it, summing to its surveyed share: 1 for a texel
 * surveyed whole (R10.T2's `ClassWeights`, whose bytes sum to 255, pass as byte ÷ 255), less for one
 * the survey covers in part, whose rest `elsewhere` takes. They are quantised to bytes keeping that
 * sum, so a texel surveyed whole leaves no share to `elsewhere`.
 *
 * @param weightsOf - A texel's weight per class, each in [0, 1], or `null` for an unsurveyed texel.
 * @throws Error if N is not a positive integer, or `classes` is not 1 to {@link MAX_DISC_CLASSES},
 *   or a texel's weights are not `classes` many.
 */
export function packClassMap(
  faceTexels: number,
  classes: number,
  weightsOf: (texel: ClassMapTexel) => ReadonlyArray<number> | null,
): ClassMapTexels {
  if (!(Number.isInteger(faceTexels) && faceTexels > 0)) {
    throw new Error(`a class map has a positive whole number of texels a side, got ${faceTexels}`);
  }
  if (!(Number.isInteger(classes) && classes > 0 && classes <= MAX_DISC_CLASSES)) {
    throw new Error(`a class map holds 1 to ${MAX_DISC_CLASSES} classes, got ${classes}`);
  }
  const side = classMapSide(faceTexels);
  const map: ClassMapTexels = {
    faceTexels,
    classes,
    data: new Uint8Array(classMapLayers(classes) * side * side * CLASSES_PER_LAYER),
  };
  for (const face of FACE_INDICES) {
    for (let j = 0; j < faceTexels; j += 1) {
      for (let i = 0; i < faceTexels; i += 1) {
        const texel = { face, i, j };
        const weights = weightsOf(texel);
        if (weights === null) {
          continue;
        }
        if (weights.length !== classes) {
          throw new Error(
            `texel (${face}, ${i}, ${j}) has ${weights.length} weights for ${classes} classes`,
          );
        }
        weightBytes(weights).forEach((byte, k) => {
          map.data[weightOffset(map, texel, k)] = byte;
        });
      }
    }
  }
  // The gutter: each side's texels from the neighbouring face's edge, the corners left empty.
  for (const face of FACE_INDICES) {
    for (let at = 1; at <= faceTexels; at += 1) {
      for (const [x, y] of [
        [0, at],
        [faceTexels + 1, at],
        [at, 0],
        [at, faceTexels + 1],
      ] as const) {
        const from = acrossEdge(face, x, y, faceTexels);
        for (let k = 0; k < classes; k += 1) {
          map.data[storedOffset(map, face, x, y, k)] = map.data[weightOffset(map, from, k)] ?? 0;
        }
      }
    }
  }
  return map;
}

/** The weight of each class at stored texel (x, y) of `face`'s layers, as the shader reads it. */
function storedWeights(map: ClassMapTexels, face: Face, x: number, y: number): number[] {
  return Array.from(
    { length: map.classes },
    (_, k) => (map.data[storedOffset(map, face, x, y, k)] ?? 0) / UNORM_MAX,
  );
}

/**
 * The weight of each class in the texel holding a body-fixed direction, byte ÷ 255: the texel's own,
 * unfiltered ({@link classMapSharesAt} reconstructs the shares between texels).
 */
export function classWeightsAt(map: ClassMapTexels, direction: Xyz): number[] {
  const texel = classMapTexelOf(direction, map.faceTexels);
  return storedWeights(map, texel.face, texel.i + 1, texel.j + 1);
}

/**
 * The share of each of {@link discSurfaceLaws}' laws at a point, from its classes' weights: the
 * weights, scaled down to sum to 1 where they sum to more, after `elsewhere`'s share 1 − Σ w (0
 * where they sum to 1 or more), as `texel_shares` does.
 */
export function surfaceShares(weights: ReadonlyArray<number>): number[] {
  const total = weights.reduce((sum, w) => sum + w, 0);
  const classes = total > 1 ? weights.map((w) => w / total) : [...weights];
  return [total >= 1 ? 0 : 1 - total, ...classes];
}

/**
 * The share of each of {@link discSurfaceLaws}' laws at a body-fixed direction under a class map,
 * by the survey-masked bilinear reconstruction `surface_shares` computes (R10.T10.d,
 * decision-r07-t8b).
 *
 * @remarks
 * With (s, t) the direction's cell coordinates on its face and n = N: where the texel holding it,
 * (⌊n s⌋, ⌊n t⌋), holds no weight, `elsewhere` alone; otherwise each of the four texels whose
 * centres are nearest, from (⌊n s − ½⌋, ⌊n t − ½⌋), the gutter's among them at a face's edge, by its
 * bilinear weight in (n s − ½, n t − ½), those holding weight alone, each with its own shares
 * ({@link surfaceShares}), summed and divided by their bilinear weights' sum. The texel holding the
 * point is one of the four, with a bilinear weight of at least ¼, so the sum is never 0. At a
 * surveyed texel's centre the shares are its own.
 *
 * @param direction - Along the body-fixed axes, z the pole; it need not be unit.
 */
export function classMapSharesAt(map: ClassMapTexels, direction: Xyz): number[] {
  const n = map.faceTexels;
  const { face, u, v } = xyzToFaceUv(direction);
  const sN = uvToSt(u) * n;
  const tN = uvToSt(v) * n;
  const holding = [
    Math.min(Math.max(Math.floor(sN), 0), n - 1),
    Math.min(Math.max(Math.floor(tN), 0), n - 1),
  ] as const;
  const x = sN - 0.5;
  const y = tN - 0.5;
  const i0 = Math.min(Math.max(Math.floor(x), -1), n - 1);
  const j0 = Math.min(Math.max(Math.floor(y), -1), n - 1);
  const fx = x - i0;
  const fy = y - j0;
  const shares = Array.from({ length: map.classes + 1 }, () => 0);
  let norm = 0;
  for (let corner = 0; corner < 4; corner += 1) {
    const di = corner % 2;
    const dj = Math.floor(corner / 2);
    const i = i0 + di;
    const j = j0 + dj;
    const weights = storedWeights(map, face, i + 1, j + 1);
    const surveyed = weights.some((w) => w > 0);
    if (i === holding[0] && j === holding[1] && !surveyed) {
      return [1, ...Array.from({ length: map.classes }, () => 0)];
    }
    if (!surveyed) {
      continue;
    }
    const b = (di === 0 ? 1 - fx : fx) * (dj === 0 ? 1 - fy : fy);
    surfaceShares(weights).forEach((share, m) => {
      shares[m] = (shares[m] ?? 0) + b * share;
    });
    norm += b;
  }
  return norm > 0 ? shares.map((share) => share / norm) : [1, ...shares.slice(1).map(() => 0)];
}

/** The laws a surface shades with, in the disc record's order: one, or `elsewhere` then each class's. */
export function discSurfaceLaws(surface: DiscSurface): ReadonlyArray<PhotometricLaw> {
  let laws: ReadonlyArray<PhotometricLaw>;
  switch (surface.kind) {
    case "uniform":
      laws = [surface.law];
      break;
    case "class-map":
      laws = [surface.elsewhere, ...surface.laws];
      break;
  }
  return laws;
}

/**
 * The specification of a class map's texture: (N + 2) × (N + 2) texels a layer, the gutter
 * included, filed under `coarse-field-gpu` beside the coverage mask (R12 Design note 6's coarse
 * field's GPU copy; R10.T10.d), 418 KB at R10's 64 texels a face and 16 classes.
 */
export function classMapTextureSpec(
  name: string,
  faceTexels: number,
  classes: number,
): TextureSpec {
  const side = classMapSide(faceTexels);
  return {
    name,
    size: { width: side, height: side, depthOrArrayLayers: classMapLayers(classes) },
    dimension: "2d",
    format: CLASS_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "coarse-field-gpu",
  };
}

/**
 * A disc's class-map surface (R10.T10.d's construction): the map uploaded as its texture, with a law
 * per class and the uniform law for unsurveyed texels.
 *
 * @remarks
 * The texture is the caller's: it is made again, and this called again, after a device loss
 * (`onRestored`), and released with `releaseTexture` when the map is dropped.
 *
 * @param laws - One per class of `map`, in its channel order.
 * @param elsewhere - The uniform `lawFor(p, q, template)`, for unsurveyed texels.
 * @throws Error if `laws` are not one per class of `map`.
 */
export function classMapSurface(
  engine: Pick<RenderEngine, "createTexture" | "writeTexture">,
  name: string,
  map: ClassMapTexels,
  laws: ReadonlyArray<PhotometricLaw>,
  elsewhere: PhotometricLaw,
): ClassMapDiscSurface {
  if (laws.length !== map.classes) {
    throw new Error(`a class map of ${map.classes} classes needs as many laws, got ${laws.length}`);
  }
  const weights: TextureHandle = engine.createTexture(
    classMapTextureSpec(name, map.faceTexels, map.classes),
  );
  const side = classMapSide(map.faceTexels);
  engine.writeTexture(
    weights,
    { x: 0, y: 0, z: 0 },
    { width: side, height: side, depthOrArrayLayers: classMapLayers(map.classes) },
    map.data,
  );
  return { kind: "class-map", weights, laws, elsewhere };
}
