import type { BodyIdHex } from "./generated/BodyIdHex";
import type { SystemIdHex } from "./generated/SystemIdHex";

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

/** A system's 16 digits, a full stop and 4 more: the wire form of a body ID. */
const BODY_ID = /^([0-9a-f]{16})\.([0-9a-f]{4})$/;

/** The largest body index, 2¹⁶ − 1. */
const BODY_INDEX_MAX = 0xffff;

/**
 * A body ID taken apart: its system and its index within the system.
 *
 * @remarks
 * The index's layout is plan 14's: stars at 0–15 (`0x0000`–`0x000f`), then each planet at
 * `slot << 8` from slot 1, its moons and rings in the low byte.
 */
export interface BodyIdParts {
  /** The system's ID in its wire form, 16 lowercase hexadecimal digits. */
  readonly system: SystemIdHex;
  /** The body's index within its system, an integer from 0 to 65,535. */
  readonly bodyIndex: number;
}

/**
 * Whether `text` is the wire form of a body ID: a system's 16 lowercase hexadecimal digits, a full
 * stop and the body index as 4 lowercase hexadecimal digits, `0200080020000000.0100`.
 */
export function isBodyId(text: string): boolean {
  return BODY_ID.test(text);
}

/**
 * Takes a body ID's wire form apart into its system and its body index.
 *
 * @remarks
 * Upper case, a `0x` prefix, whitespace and any other length are refused, as the server refuses
 * them, so that one body has one string.
 *
 * @throws SyntaxError if `text` is not a body ID's wire form.
 */
export function parseBodyId(text: BodyIdHex): BodyIdParts {
  const match = BODY_ID.exec(text);
  const system = match?.[1];
  const index = match?.[2];
  if (system === undefined || index === undefined) {
    throw new SyntaxError(
      `expected 16 lowercase hexadecimal digits, a full stop and 4 lowercase hexadecimal digits, got ${JSON.stringify(text)}`,
    );
  }
  return { system, bodyIndex: Number.parseInt(index, 16) };
}

/**
 * Writes a body ID in its wire form, as the server writes it: the inverse of {@link parseBodyId}.
 *
 * @throws SyntaxError if `system` is not a system ID's wire form.
 * @throws RangeError if `bodyIndex` is not an integer from 0 to 65,535.
 */
export function formatBodyId({ system, bodyIndex }: BodyIdParts): BodyIdHex {
  if (!isHex64(system)) {
    throw new SyntaxError(
      `expected 16 lowercase hexadecimal digits for the system, got ${JSON.stringify(system)}`,
    );
  }
  if (!(Number.isInteger(bodyIndex) && bodyIndex >= 0 && bodyIndex <= BODY_INDEX_MAX)) {
    throw new RangeError(`${String(bodyIndex)} is not a body index, an integer from 0 to 65,535`);
  }
  return `${system}.${bodyIndex.toString(16).padStart(4, "0")}`;
}
