/**
 * IEEE 754 half-precision encoding, for the band's `rgba16float` upload (plan R06, T13.d).
 *
 * @remarks
 * Rounded to nearest, ties to even, as a GPU stores a `f32` into `rgba16float`; values above the
 * largest half, 65,504, become infinity unless clamped first, and below 2⁻²⁴ ÷ 2 become zero.
 * NaN stays NaN. Written out because the renderer cannot rely on `Float16Array` in every runtime
 * it is tested under.
 */

/** The largest finite half: 65,504. */
export const HALF_MAX = 65_504;

const F32 = new Float32Array(1);
const U32 = new Uint32Array(F32.buffer);

/** One `f32` value's half-precision bits. */
export function toHalfBits(value: number): number {
  F32[0] = value;
  const bits = U32[0] ?? 0;
  const sign = (bits >>> 16) & 0x8000;
  const exponent = (bits >>> 23) & 0xff;
  const mantissa = bits & 0x7f_ffff;
  if (exponent === 0xff) {
    // Infinity or NaN; a NaN keeps a mantissa bit set.
    return sign | 0x7c00 | (mantissa === 0 ? 0 : 0x200);
  }
  const halfExponent = exponent - 127 + 15;
  if (halfExponent >= 0x1f) {
    return sign | 0x7c00;
  }
  if (halfExponent <= 0) {
    // Subnormal half: shift the mantissa with its implicit one in, rounding to nearest even.
    if (halfExponent < -10) {
      return sign;
    }
    const full = mantissa | 0x80_0000;
    const shift = 14 - halfExponent;
    const truncated = full >>> shift;
    const remainder = full & ((1 << shift) - 1);
    const half = 1 << (shift - 1);
    const roundUp = remainder > half || (remainder === half && (truncated & 1) === 1);
    return sign | (truncated + (roundUp ? 1 : 0));
  }
  const truncated = (halfExponent << 10) | (mantissa >>> 13);
  const remainder = mantissa & 0x1fff;
  const roundUp = remainder > 0x1000 || (remainder === 0x1000 && (truncated & 1) === 1);
  // A carry out of the mantissa raises the exponent, which is the correct rounding (up to infinity).
  return sign | (truncated + (roundUp ? 1 : 0));
}

/** Half-precision bits of every value, in order. */
export function toHalfArray(values: Float32Array): Uint16Array {
  const halves = new Uint16Array(values.length);
  for (let i = 0; i < values.length; i += 1) {
    halves[i] = toHalfBits(values[i] ?? 0);
  }
  return halves;
}

/** The value of half-precision bits, for tests and readbacks. */
export function fromHalfBits(bits: number): number {
  const sign = (bits & 0x8000) === 0 ? 1 : -1;
  const exponent = (bits >>> 10) & 0x1f;
  const mantissa = bits & 0x3ff;
  if (exponent === 0) {
    return sign * mantissa * 2 ** -24;
  }
  if (exponent === 0x1f) {
    return mantissa === 0 ? sign * Number.POSITIVE_INFINITY : Number.NaN;
  }
  return sign * (1 + mantissa / 1024) * 2 ** (exponent - 15);
}
