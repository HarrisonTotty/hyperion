/**
 * A patch's horizon map as the lit terrain reads it (plan R10, R10.T8.b; Design note 10;
 * decision-r10-t8a §1): the TypeScript twin of `shadows/horizon.wgsl`'s lookup and blend, and of
 * the map's encoding, `pack2x16snorm` and `unpack2x16snorm` over e ÷ 90°.
 *
 * @remarks
 * The map is R10.T8.a's (`hyperion_surface::bake::horizon`): four `u32` words a texel, the near
 * field's 65 × 65 vertices row by row and the far field's 9 × 9 points, point (a, b) over vertex
 * (8a, 8b), azimuth 2w in word w's low half and 2w + 1 in its high half, azimuth k at k × 45° of
 * grid azimuth (`gridAzimuth.ts`). At a texel's vertex coordinates (x, y) in [0, 64] the lookup is
 * the near field bilinear between the four vertices about it and the far field bilinear at
 * (x ÷ 8, y ÷ 8) between the four points about it, each azimuth's elevation linear between the two
 * azimuths about the direction's, and the larger of the two fields: at a vertex,
 * `HorizonMap::elevation_at`. The blend mixes the patch's map with its parent's by the morph
 * factor k, as the geometry is mixed, the parent read at its own vertex coordinates where k > 0.
 */

import { HORIZON_FAR_SIDE, HORIZON_WORDS_PER_TEXEL, PATCH_VERTICES_PER_SIDE } from "../slotLayout";

/** The azimuths a horizon map holds. */
export const HORIZON_AZIMUTHS = 8;

/** The largest code, 2¹⁵ − 1, that `pack2x16snorm` scales by. */
const SNORM16_MAX = 32_767;

/** A patch's horizon map, as a bake hands it over (`TerrainLayers.horizonNear` and `horizonFar`). */
export interface HorizonWords {
  /** Four words a vertex, 65 × 65, row by row. */
  readonly near: Uint32Array;
  /** Four words a point, 9 × 9, row by row. */
  readonly far: Uint32Array;
}

/**
 * The code of an `f32` value `x` as WGSL's `pack2x16snorm` gives it, ⌊½ + 32,767 clamp(x, −1, 1)⌋:
 * exact for every `f32`, whose product with 32,767 an `f64` holds exactly.
 */
export function snorm16(x: number): number {
  return Math.floor(0.5 + SNORM16_MAX * Math.min(Math.max(x, -1), 1));
}

/** The elevation of a code as `unpack2x16snorm` reads it, max(c ÷ 32,767, −1) × 90°, radians. */
export function decodeHorizonRad(code: number): number {
  return Math.max(code / SNORM16_MAX, -1) * (Math.PI / 2);
}

/** A 16-bit half word as a signed code, two's complement. */
function signed(half: number): number {
  return half >= 0x8000 ? half - 0x1_0000 : half;
}

/** The two signed 16-bit codes of a word, its low half first. */
export function unpackCodes(word: number): readonly [number, number] {
  return [signed(word & 0xffff), signed((word >>> 16) & 0xffff)];
}

/** The word of two codes, the first in the low half, as `pack2x16snorm` packs them. */
export function packCodes(low: number, high: number): number {
  return ((low & 0xffff) | ((high & 0xffff) << 16)) >>> 0;
}

/** Azimuth `k`'s elevation at texel `texel` of `words`, radians. */
export function horizonAt(words: Uint32Array, texel: number, k: number): number {
  const word = words[texel * HORIZON_WORDS_PER_TEXEL + (k >> 1)];
  if (word === undefined) {
    throw new RangeError(`texel ${String(texel)} is past the map's ${String(words.length)} words`);
  }
  const [low, high] = unpackCodes(word);
  return decodeHorizonRad((k & 1) === 0 ? low : high);
}

/** The two azimuths about a grid azimuth and its fraction of the way from the first to the second. */
interface Knots {
  readonly k0: number;
  readonly k1: number;
  readonly between: number;
}

/** The knots of the grid azimuth `azimuthRad`, any finite angle. */
function knotsOf(azimuthRad: number): Knots {
  const turns = azimuthRad / (Math.PI / 4);
  const q = turns - HORIZON_AZIMUTHS * Math.floor(turns / HORIZON_AZIMUTHS);
  const whole = Math.floor(q);
  const k0 = whole % HORIZON_AZIMUTHS;
  return { k0, k1: (k0 + 1) % HORIZON_AZIMUTHS, between: q - whole };
}

/** a + (b − a) f, the bake's and the shader's form. */
function lerp(a: number, b: number, f: number): number {
  return a + (b - a) * f;
}

/** Texel `texel`'s horizon towards `knots`, linear in azimuth. */
function towards(words: Uint32Array, texel: number, knots: Knots): number {
  return lerp(horizonAt(words, texel, knots.k0), horizonAt(words, texel, knots.k1), knots.between);
}

/** A field of `side` texels a row, bilinear at (x, y) in its texels' units. */
function bilinear(words: Uint32Array, side: number, x: number, y: number, knots: Knots): number {
  const bx = Math.min(Math.floor(Math.max(x, 0)), side - 2);
  const by = Math.min(Math.floor(Math.max(y, 0)), side - 2);
  const fx = Math.min(Math.max(x - bx, 0), 1);
  const fy = Math.min(Math.max(y - by, 0), 1);
  const first = by * side + bx;
  const e00 = towards(words, first, knots);
  const e10 = towards(words, first + 1, knots);
  const e01 = towards(words, first + side, knots);
  const e11 = towards(words, first + side + 1, knots);
  return lerp(lerp(e00, e10, fx), lerp(e01, e11, fx), fy);
}

/**
 * The map's horizon at vertex coordinates (`x`, `y`), each in [0, 64], towards the grid azimuth
 * `azimuthRad`, radians: the larger of the near field and the far field, as the shader takes it.
 */
export function horizonElevationRad(
  map: HorizonWords,
  x: number,
  y: number,
  azimuthRad: number,
): number {
  const knots = knotsOf(azimuthRad);
  const near = bilinear(map.near, PATCH_VERTICES_PER_SIDE, x, y, knots);
  const far = bilinear(map.far, HORIZON_FAR_SIDE, x / 8, y / 8, knots);
  return Math.max(near, far);
}

/**
 * The horizon at (`x`, `y`) of a patch whose map is `own`, blended to its parent's map `parent` by
 * the morph factor `morph` (decision-r10-t8a §1), radians: WGSL's mix(own, parent's, k), the
 * parent's read at its own vertex coordinates ((q_x 64 + x) ÷ 2, (q_y 64 + y) ÷ 2) and only where
 * k > 0; the own map at every k where `parent` is `null`.
 *
 * @param quadrant - The patch's quadrant in its parent, bit 0 i & 1 and bit 1 j & 1.
 * @param morph - k, 0 to 1.
 */
export function blendedHorizonRad(
  own: HorizonWords,
  parent: HorizonWords | null,
  quadrant: number,
  x: number,
  y: number,
  azimuthRad: number,
  morph: number,
): number {
  const e = horizonElevationRad(own, x, y, azimuthRad);
  if (parent === null || !(morph > 0)) {
    return e;
  }
  const px = ((quadrant & 1) * 64 + x) / 2;
  const py = (((quadrant >> 1) & 1) * 64 + y) / 2;
  const p = horizonElevationRad(parent, px, py, azimuthRad);
  return e * (1 - morph) + p * morph;
}
