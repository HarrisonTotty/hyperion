/**
 * A patch's shading normal as the lit terrain reads it (plan R10, R10.T8.c; R05's Design note 5;
 * decision-r10-t16-leans §16): the TypeScript twin of `shaders/terrainPatch.wgsl`'s
 * `decodeOctahedral`, `patchNormalAt`, `parentPatchUv` and `shadingNormalAt`, operation for
 * operation in `f64`.
 *
 * @remarks
 * A patch's normals are its bake's octahedral pairs (`hyperion_surface::patch::normals`),
 * body-fixed, at the setting's resolution (65 or 129 samples a side over the patch), rounded to
 * half floats by the worker (`packNormals`) and filtered by hand on the decoded vectors, bilinearly
 * between the four samples about the point, then renormalised. The shading normal morphs with the
 * height (CDLOD): the own tile's normal is blended to the parent's by the morph factor k,
 * normalize(mix(own, parent's, k)), the parent's tile read at the patch's place in it,
 * ((q_x, q_y) + (u, v)) ÷ 2, and only where k > 0. So at k = 1 a fine patch shades with its
 * parent's normals, as its geometry is its parent's, and along a coarse–fine edge it shades as its
 * coarse neighbour, whose own tile holds the same samples of the parent's level there.
 */

import { type Vec3, vec3 } from "../../geometry/vec3";

/** One patch's normals as the atlas holds them, without the gutter. */
export interface NormalsTile {
  /** Octahedral pairs, sample (x, y) at 2 (y N + x), row by row: the bake's, as half floats. */
  readonly pairs: ArrayLike<number>;
  /** N, the samples a side: 65 at the mesh's resolution, 129 at twice it. */
  readonly samplesPerSide: number;
}

/** +1 for x ≥ 0 (a zero of either sign included), −1 otherwise: the shader's `signNotZero`. */
function signNotZero(x: number): number {
  return x < 0 ? -1 : 1;
}

/** The unit vector of the octahedral pair (`x`, `y`): the shader's `decodeOctahedral`. */
export function decodeOctahedral(x: number, y: number): Vec3 {
  const z = 1 - Math.abs(x) - Math.abs(y);
  let a = x;
  let b = y;
  if (z < 0) {
    a = (1 - Math.abs(y)) * signNotZero(x);
    b = (1 - Math.abs(x)) * signNotZero(y);
  }
  const n = Math.hypot(a, b, z);
  return vec3(a / n, b / n, z / n);
}

/** Sample (`x`, `y`) of `tile`, decoded. */
function sampleAt(tile: NormalsTile, x: number, y: number): Vec3 {
  const at = 2 * (y * tile.samplesPerSide + x);
  return decodeOctahedral(tile.pairs[at] ?? NaN, tile.pairs[at + 1] ?? NaN);
}

/** a + (b − a) f, component by component, as WGSL's `mix`. */
function mixVec(a: Vec3, b: Vec3, f: number): Vec3 {
  return vec3(a.x * (1 - f) + b.x * f, a.y * (1 - f) + b.y * f, a.z * (1 - f) + b.z * f);
}

/** `v` over its length. */
function unit(v: Vec3): Vec3 {
  const n = Math.hypot(v.x, v.y, v.z);
  return vec3(v.x / n, v.y / n, v.z / n);
}

/**
 * The terrain's unit normal at (`u`, `v`), the patch's place (x, y) ÷ 64, of `tile`: the four
 * decoded samples about it mixed bilinearly and renormalised (the shader's `patchNormalAt`).
 */
export function tileNormalAt(tile: NormalsTile, u: number, v: number): Vec3 {
  const n = tile.samplesPerSide;
  const sx = u * (n - 1);
  const sy = v * (n - 1);
  const bx = Math.min(Math.floor(sx), n - 2);
  const by = Math.min(Math.floor(sy), n - 2);
  const fx = sx - bx;
  const fy = sy - by;
  const n00 = sampleAt(tile, bx, by);
  const n10 = sampleAt(tile, bx + 1, by);
  const n01 = sampleAt(tile, bx, by + 1);
  const n11 = sampleAt(tile, bx + 1, by + 1);
  return unit(mixVec(mixVec(n00, n10, fx), mixVec(n01, n11, fx), fy));
}

/**
 * The place (`u`, `v`) of a patch whose quadrant in its parent is `quadrant` (`gpu/uniforms.ts`'
 * `parentQuadrant`: bit 0 i & 1, bit 1 j & 1) as its parent's place, ((q_x, q_y) + (u, v)) ÷ 2
 * (the shader's `parentPatchUv`).
 */
export function parentPatchUv(quadrant: number, u: number, v: number): readonly [number, number] {
  return [((quadrant & 1) + u) * 0.5, (((quadrant >> 1) & 1) + v) * 0.5];
}

/**
 * The shading normal at (`u`, `v`) of a patch whose normals are `own`, blended to its parent's
 * `parent` by the morph factor `morph` (the shader's `shadingNormalAt`): normalize(mix(own,
 * parent's, min(k, 1))), the parent's read at the patch's place in it and only where k > 0; the own
 * tile's at every k where `parent` is `null` (no parent resident).
 *
 * @param quadrant - The patch's quadrant in its parent (`parentQuadrant`), 0 to 3.
 * @param morph - k, 0 to 1.
 */
export function shadingNormal(
  own: NormalsTile,
  parent: NormalsTile | null,
  quadrant: number,
  u: number,
  v: number,
  morph: number,
): Vec3 {
  const mine = tileNormalAt(own, u, v);
  if (parent === null || !(morph > 0)) {
    return mine;
  }
  const [pu, pv] = parentPatchUv(quadrant, u, v);
  return unit(mixVec(mine, tileNormalAt(parent, pu, pv), Math.min(morph, 1)));
}
