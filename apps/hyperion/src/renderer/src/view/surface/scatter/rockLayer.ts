/**
 * A finest patch's authoritative rocks as its bake carries them (plan R11, T4.a): the layer R11's
 * `RockBakeHook` fills through R10's `BakeHook` (`hyperion_surface::scatter::bake`), transferred
 * with the patch's other layers and held with it while it is resident.
 *
 * @remarks
 * - **The layer**: {@link ROCK_WORDS} `f64` a rock (`src/wasm.rs`'s `BakedTerrain.rocks()`, the
 *   crate's `rock_words`): its base centre, body-fixed metres; its frame's a, b and c axes,
 *   body-fixed unit vectors, the c axis the datum's normal at its axis; its semi-axes a, b and c,
 *   metres; its superquadric exponent n, so that the rock is |x ÷ a|ⁿ + |y ÷ b|ⁿ + |z ÷ c|ⁿ ≤ 1 in
 *   its frame about its base centre; the ground's height at its axis, metres above the datum; and
 *   its axis's unit direction, body-fixed, which names the cell, and so the patch, it lies in.
 *   The rocks are those whose axis the patch holds, octave by octave, as `rocks_near` orders them,
 *   and only a patch at the finest level carries the layer.
 * - **Placement**: a rock is placed relative to the patch's `f64` origin, as every patch vertex
 *   is: its base centre less the origin in `f64`, narrowed once to `f32`
 *   ({@link rockOffsetsF32}).
 * - It imports nothing that needs the engine, so that the height worker may build it
 *   (`tsconfig.worker.json`).
 */

import { type Vec3, vec3 } from "../../../geometry/vec3";

/** The `f64` words a rock is handed over in: the surface crate's `ROCK_WORDS`. */
export const ROCK_WORDS = 20;

/** A finest patch's rocks as its bake hands them over (see the module's remarks). */
export interface RockLayer {
  /** How many rocks. */
  readonly count: number;
  /** {@link ROCK_WORDS} `f64` a rock, rock r at `20 r`. */
  readonly words: Float64Array;
}

/** One rock of a {@link RockLayer}, unpacked. */
export interface Rock {
  /** The centre of its base outline, on the ground at its axis: body-fixed, m. */
  readonly baseCentreM: Vec3;
  /**
   * Its frame, body-fixed unit vectors: the a axis along its yaw, the b axis, and the c axis, the
   * datum's normal at its axis; a right-handed set.
   */
  readonly axes: readonly [Vec3, Vec3, Vec3];
  /** Its semi-axes a, b and c, m: a and b horizontal, c its exposed height. */
  readonly semiAxesM: readonly [number, number, number];
  /** Its superquadric exponent, 2 (an ellipsoid) to 3. */
  readonly exponent: number;
  /** The ground's height at its axis, m above the datum. */
  readonly groundHeightM: number;
  /** Its axis's unit direction, body-fixed: its foot on the datum is the spheroid's point there. */
  readonly dir: Vec3;
}

/**
 * The layer of `words` as the module hands them over.
 *
 * @throws RangeError where `words` is not a whole number of rocks.
 */
export function rockLayerOf(words: Float64Array): RockLayer {
  if (words.length % ROCK_WORDS !== 0) {
    throw new RangeError(
      `a rock layer of ${String(words.length)} words is not ${String(ROCK_WORDS)} a rock`,
    );
  }
  return { count: words.length / ROCK_WORDS, words };
}

/**
 * Rock `n` of `layer`.
 *
 * @throws RangeError where `n` is not one of its rocks.
 */
export function rockAt(layer: RockLayer, n: number): Rock {
  if (!Number.isInteger(n) || n < 0 || n >= layer.count) {
    throw new RangeError(`rock ${String(n)} is not one of the layer's ${String(layer.count)}`);
  }
  const at = ROCK_WORDS * n;
  const w = (k: number): number => layer.words[at + k] ?? Number.NaN;
  const v = (k: number): Vec3 => vec3(w(k), w(k + 1), w(k + 2));
  return {
    baseCentreM: v(0),
    axes: [v(3), v(6), v(9)],
    semiAxesM: [w(12), w(13), w(14)],
    exponent: w(15),
    groundHeightM: w(16),
    dir: v(17),
  };
}

/** Every rock of `layer`, in its order. */
export function rocksOf(layer: RockLayer): Rock[] {
  return Array.from({ length: layer.count }, (_, n) => rockAt(layer, n));
}

/**
 * Each rock's base centre less the patch's origin `originM`, body-fixed, the difference taken in
 * `f64` and narrowed once to `f32`, three a rock: where its mesh is placed, as the patch's own
 * vertices are.
 */
export function rockOffsetsF32(layer: RockLayer, originM: Vec3): Float32Array {
  const out = new Float32Array(3 * layer.count);
  for (let n = 0; n < layer.count; n += 1) {
    const at = ROCK_WORDS * n;
    out[3 * n] = (layer.words[at] ?? 0) - originM.x;
    out[3 * n + 1] = (layer.words[at + 1] ?? 0) - originM.y;
    out[3 * n + 2] = (layer.words[at + 2] ?? 0) - originM.z;
  }
  return out;
}

/** The bytes `layer` holds on the heap, for the patch cache's memory line. */
export function rockLayerBytes(layer: RockLayer): number {
  return layer.words.byteLength;
}
