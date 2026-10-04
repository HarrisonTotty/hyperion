/**
 * The disc's class-map surface (plan R07, T8.b; Design note 24): the layout of R10's coarse
 * class-weights map as `shaders/bodyDisc.wgsl` reads it, its construction and its CPU twin.
 *
 * @remarks
 * A class map holds, for each cell of R05's cube sphere at one level, the weight of each of the
 * body's classes over the cell, surveyed cells only (R10 Design note 8; R10.T10.d fills it). The
 * texture is a 2D array of {@link CLASS_MAP_FORMAT} texels, N × N per face: texel (i, j) of face f
 * is the cell s ∈ [i ÷ N, (i + 1) ÷ N), t ∈ [j ÷ N, (j + 1) ÷ N) of R05's face coordinates (u, v)
 * (`xyzToFaceUv`) under S2's quadratic warp (`uvToSt`), so with N = 2^L it is the quadtree's cell
 * (f, L, i, j). Class k's weight is channel k mod 4 of layer f + 6 ⌊k ÷ 4⌋. An unsurveyed texel
 * holds no weight. At a point the disc shades with each class's law by its weight and with
 * `elsewhere` by what the weights leave, 1 − Σ w ({@link surfaceShares}): a surveyed texel's
 * weights sum to 1 and leave none, an unsurveyed one leaves all, so the disc shows no pattern the
 * ship has not seen. The shader reads the texel the point falls in, unfiltered, so the survey's
 * edge stays where the survey put it.
 */
import type { ClassMapDiscSurface, DiscSurface } from "../appearance/bodyAppearance";
import type { PhotometricLaw } from "../appearance/law";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { TextureSpec } from "../engine/memory";
import type { RenderEngine, TextureHandle } from "../engine/types";
import { uvToSt, type Xyz, xyzToFaceUv } from "../terrain/cube";
import type { Face } from "../terrain/patchKey";

/** The classes one disc's class map may hold; the record and the shader carry this many laws. */
export const MAX_DISC_CLASSES = 16;

/** The class map's texel format: four classes' weights, 0 to 1 in steps of 1 ÷ 255. */
export const CLASS_MAP_FORMAT = "rgba8unorm" as const;

/** Classes per layer: one in each channel. */
export const CLASSES_PER_LAYER = 4;

/** The cube sphere's faces. */
const FACES = 6;

/** The largest weight a texel's byte holds. */
const UNORM_MAX = 255;

/** The layers of a class map of `classes` classes: six faces for every four classes. */
export function classMapLayers(classes: number): number {
  return FACES * Math.ceil(classes / CLASSES_PER_LAYER);
}

/** A class map's texels as they are uploaded. */
export interface ClassMapTexels {
  /** N, the texels along each face's side. */
  readonly faceTexels: number;
  /** The body's classes, one law each. */
  readonly classes: number;
  /**
   * Layer after layer ({@link classMapLayers}), each N rows j of N texels i, four bytes a texel:
   * class 4g + c's weight × 255 in channel c of layer f + 6g.
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

/** The byte offset of class `k`'s weight at a texel. */
function weightOffset(map: ClassMapTexels, texel: ClassMapTexel, k: number): number {
  const n = map.faceTexels;
  const layer = texel.face + FACES * Math.floor(k / CLASSES_PER_LAYER);
  return ((layer * n + texel.j) * n + texel.i) * CLASSES_PER_LAYER + (k % CLASSES_PER_LAYER);
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
 * Packs a class map from each texel's weights, in the layout the shader reads.
 *
 * @remarks
 * A surveyed texel's weights sum to 1 (R10.T2's `ClassWeights`, whose bytes sum to 255, pass as
 * byte ÷ 255); they are quantised to bytes keeping that sum, so a surveyed texel leaves no share
 * to `elsewhere`.
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
  const map: ClassMapTexels = {
    faceTexels,
    classes,
    data: new Uint8Array(classMapLayers(classes) * faceTexels * faceTexels * CLASSES_PER_LAYER),
  };
  for (const face of [0, 1, 2, 3, 4, 5] as const) {
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
  return map;
}

/** The weight of each class at a body-fixed direction, as the shader reads it: byte ÷ 255. */
export function classWeightsAt(map: ClassMapTexels, direction: Xyz): number[] {
  const texel = classMapTexelOf(direction, map.faceTexels);
  return Array.from(
    { length: map.classes },
    (_, k) => (map.data[weightOffset(map, texel, k)] ?? 0) / UNORM_MAX,
  );
}

/**
 * The share of each of {@link discSurfaceLaws}' laws at a point, from its classes' weights: the
 * weights, scaled down to sum to 1 where they sum to more, after `elsewhere`'s share 1 − Σ w (0
 * where they sum to 1 or more), as `surface_shares` does.
 */
export function surfaceShares(weights: ReadonlyArray<number>): number[] {
  const total = weights.reduce((sum, w) => sum + w, 0);
  const classes = total > 1 ? weights.map((w) => w / total) : [...weights];
  return [total >= 1 ? 0 : 1 - total, ...classes];
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

/** The specification of a class map's texture. */
export function classMapTextureSpec(
  name: string,
  faceTexels: number,
  classes: number,
): TextureSpec {
  return {
    name,
    size: { width: faceTexels, height: faceTexels, depthOrArrayLayers: classMapLayers(classes) },
    dimension: "2d",
    format: CLASS_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
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
  engine.writeTexture(
    weights,
    { x: 0, y: 0, z: 0 },
    {
      width: map.faceTexels,
      height: map.faceTexels,
      depthOrArrayLayers: classMapLayers(map.classes),
    },
    map.data,
  );
  return { kind: "class-map", weights, laws, elsewhere };
}
