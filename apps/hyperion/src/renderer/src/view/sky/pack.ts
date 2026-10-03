/**
 * The TypeScript `rgb9e5ufloat` packer, the reference the bake's WGSL packer is tested against
 * (plan R06, Design note 21 and T13.b).
 *
 * @remarks
 * Three 9-bit mantissas share one 5-bit exponent, bias 15, with no implicit leading one: a texel is
 * m × 2^(e − 15 − 9). The conversion is the one the OpenGL extension specifies
 * (EXT_texture_shared_exponent, "Encoding of Special Internal Formats"), which WebGPU's
 * `rgb9e5ufloat` takes over (WebGPU §26.1.3, packed formats): clamp each channel to
 * [0, {@link RGB9E5_MAX}], take the shared exponent from the largest, round each channel to
 * nearest, and raise the exponent by one where the largest mantissa rounds up to 2⁹. Rounding is
 * written as q = v ÷ 2^k, then ⌊q⌋ plus one where q − ⌊q⌋ ≥ ½: for an `f32` channel every step is
 * exact in `f32` as in `f64`, whereas ⌊q + ½⌋ would round the sum in `f32` and differ just below a
 * half. The WGSL packer computes it the same way and so matches this one bit for bit.
 */

/** Mantissa bits per channel: N = 9. */
const MANTISSA_BITS = 9;
/** The exponent bias: B = 15. */
const EXPONENT_BIAS = 15;
/** The largest exponent: E_max = 31. */
const EXPONENT_MAX = 31;

/**
 * The largest value a channel holds: (2⁹ − 1) ÷ 2⁹ × 2^(31 − 15) = 65,408.
 *
 * @remarks
 * EXT_texture_shared_exponent's `sharedexp_max`; a brighter channel is clamped to it.
 */
export const RGB9E5_MAX =
  ((2 ** MANTISSA_BITS - 1) / 2 ** MANTISSA_BITS) * 2 ** (EXPONENT_MAX - EXPONENT_BIAS);

/** The smallest positive value a channel holds: 2^(0 − 15 − 9) = 2⁻²⁴; anything under half of it packs to zero. */
export const RGB9E5_MIN_POSITIVE = 2 ** -(EXPONENT_BIAS + MANTISSA_BITS);

/**
 * ⌊log₂ x⌋ for a positive finite x, exactly.
 *
 * @remarks
 * `Math.log2` may round across an integer near a power of two, so the estimate is corrected
 * against exact powers of two.
 */
function floorLog2(x: number): number {
  let exponent = Math.floor(Math.log2(x));
  if (2 ** exponent > x) {
    exponent -= 1;
  } else if (2 ** (exponent + 1) <= x) {
    exponent += 1;
  }
  return exponent;
}

/** q rounded to nearest, halves up, by its fraction: exact for any `f32` q. */
function roundHalfUp(q: number): number {
  const whole = Math.floor(q);
  return q - whole >= 0.5 ? whole + 1 : whole;
}

/** A channel clamped to [0, {@link RGB9E5_MAX}], NaN and negatives taken as 0. */
function clampChannel(value: number): number {
  return value > 0 ? Math.min(value, RGB9E5_MAX) : 0;
}

/**
 * One texel packed: red in bits 0–8, green in 9–17, blue in 18–26 and the exponent in 27–31.
 *
 * @param r - Red, linear, any value (clamped as the extension says).
 * @returns The packed texel as an unsigned 32-bit integer.
 */
export function packRgb9e5(r: number, g: number, b: number): number {
  const red = clampChannel(r);
  const green = clampChannel(g);
  const blue = clampChannel(b);
  const largest = Math.max(red, green, blue);
  const floorExponent = largest > 0 ? floorLog2(largest) : -Infinity;
  const provisional = Math.max(-EXPONENT_BIAS - 1, floorExponent) + 1 + EXPONENT_BIAS;
  const largestMantissa = roundHalfUp(largest / 2 ** (provisional - EXPONENT_BIAS - MANTISSA_BITS));
  const exponent = largestMantissa === 2 ** MANTISSA_BITS ? provisional + 1 : provisional;
  const step = 2 ** (exponent - EXPONENT_BIAS - MANTISSA_BITS);
  const mantissa = (value: number): number => roundHalfUp(value / step);
  return (mantissa(red) | (mantissa(green) << 9) | (mantissa(blue) << 18) | (exponent << 27)) >>> 0;
}

/** One packed texel read back as linear red, green and blue. */
export function unpackRgb9e5(packed: number): readonly [number, number, number] {
  const step = 2 ** ((packed >>> 27) - EXPONENT_BIAS - MANTISSA_BITS);
  return [
    (packed & 0x1ff) * step,
    ((packed >>> 9) & 0x1ff) * step,
    ((packed >>> 18) & 0x1ff) * step,
  ];
}

/**
 * A level of `rgba` texels packed, alpha ignored, in the same order.
 *
 * @param rgba - Four floats a texel; for a cube level, face after face, rows top to bottom, as
 *   `writePackedCubeLevel` takes them.
 * @throws Error when the length is not a multiple of four.
 */
export function packRgb9e5Texels(rgba: Float32Array): Uint32Array {
  if (rgba.length % 4 !== 0) {
    throw new Error(`${rgba.length} floats are not whole rgba texels`);
  }
  const packed = new Uint32Array(rgba.length / 4);
  for (let texel = 0; texel < packed.length; texel += 1) {
    const at = texel * 4;
    packed[texel] = packRgb9e5(rgba[at] ?? 0, rgba[at + 1] ?? 0, rgba[at + 2] ?? 0);
  }
  return packed;
}
