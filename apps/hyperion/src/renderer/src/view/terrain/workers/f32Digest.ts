/**
 * TypeScript twins of the testkit's float digests, `hyperion_testkit::golden::f32_digest` and
 * `f64_digest` (plan R05, T2 and T10.b): FNV-1a 64 over each value's IEEE 754 bits as
 * little-endian bytes, in order, with the offset basis `0xcbf29ce484222325` and the prime
 * `0x100000001b3`.
 *
 * @remarks
 * The hash is kept as two 32-bit halves so that no `BigInt` arithmetic runs per byte: with the
 * prime 2⁴⁰ + 0x1b3, (hi·2³² + lo)·prime mod 2⁶⁴ is lo·0x1b3 + (hi·0x1b3 + lo·2⁸)·2³², every
 * product below 2⁴² and so exact in a `number`. A `-0` and a `0` give different digests.
 */

const PRIME_LOW = 0x1b3;

/** FNV-1a 64 over `bytes` in order. */
export function fnv1a64(bytes: Uint8Array): bigint {
  let hi = 0xcbf29ce4;
  let lo = 0x84222325;
  for (const byte of bytes) {
    lo = (lo ^ byte) >>> 0;
    const low = lo * PRIME_LOW;
    const carry = Math.floor(low / 0x1_0000_0000);
    const nextHi = (hi * PRIME_LOW + carry + ((lo << 8) >>> 0)) >>> 0;
    lo = low >>> 0;
    hi = nextHi;
  }
  return (BigInt(hi) << 32n) | BigInt(lo);
}

function littleEndianBytes(view: ArrayBufferView): Uint8Array {
  // Typed arrays hold their elements in the platform's order; every target of the client is
  // little-endian, which the testkit's `to_le_bytes` assumes too.
  return new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
}

/** The testkit's `f32_digest` over `values`. */
export function f32Digest(values: Float32Array): bigint {
  return fnv1a64(littleEndianBytes(values));
}

/** The testkit's `f64_digest` over `values`. */
export function f64Digest(values: Float64Array): bigint {
  return fnv1a64(littleEndianBytes(values));
}
