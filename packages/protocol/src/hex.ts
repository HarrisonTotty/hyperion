/** Exactly 16 lowercase hexadecimal digits: the wire form of every `u64`. */
const HEX64 = /^[0-9a-f]{16}$/;

/** The largest `u64`, 2⁶⁴ − 1. */
const U64_MAX = 0xffff_ffff_ffff_ffffn;

/**
 * Whether `text` is the wire form of a `u64`: exactly 16 lowercase hexadecimal digits.
 *
 * @remarks
 * Seeds, universe IDs and system IDs all take this form. Upper case, a `0x` prefix and any other
 * length are refused, so that one value has one string.
 */
export function isHex64(text: string): boolean {
  return HEX64.test(text);
}

/**
 * Formats a `u64` in its wire form, 16 lowercase hexadecimal digits.
 *
 * @throws RangeError if `value` is negative or above 2⁶⁴ − 1.
 */
export function u64ToHex(value: bigint): string {
  if (value < 0n || value > U64_MAX) {
    throw new RangeError(`${value} is not an unsigned 64-bit integer`);
  }
  return value.toString(16).padStart(16, "0");
}

/**
 * Parses the wire form of a `u64`.
 *
 * @remarks
 * A JavaScript number holds integers exactly only up to 2⁵³, so the value is a `bigint`.
 *
 * @throws SyntaxError if `hex` is not exactly 16 lowercase hexadecimal digits.
 */
export function hexToU64(hex: string): bigint {
  if (!isHex64(hex)) {
    throw new SyntaxError(`expected 16 lowercase hexadecimal digits, got ${JSON.stringify(hex)}`);
  }
  return BigInt(`0x${hex}`);
}
