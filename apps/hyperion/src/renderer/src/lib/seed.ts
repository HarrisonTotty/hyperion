/**
 * The screen form of seeds and IDs, and the reading of a typed seed.
 *
 * @remarks
 * Every `u64` is 16 lower-case hex digits on the wire (plan 04, design note 8) and upper case on
 * screen, the guide's annunciator convention; a body's ID is its system's with a full stop and its
 * index in 4 more hex digits. These functions are the only places that change case (plan 05,
 * design note D8).
 */
import { type BodyIdHex, isHex64, type SeedHex } from "@hyperion/protocol";

/** What {@link parseSeedHex} made of the operator's text. */
export type ParsedSeed = { readonly ok: true; readonly seed: SeedHex } | { readonly ok: false };

const TYPED_SEED = /^[0-9a-f]{1,16}$/;

/**
 * Reads a seed as the operator typed it: 1 to 16 hex digits in either case, with surrounding
 * spaces ignored and leading zeros supplied.
 *
 * @returns The seed in wire form, or `{ ok: false }` for anything else, including an empty field,
 *   a `0x` prefix and more than 16 digits.
 */
export function parseSeedHex(text: string): ParsedSeed {
  const digits = text.trim().toLowerCase();
  if (!TYPED_SEED.test(digits)) {
    return { ok: false };
  }
  const seed = digits.padStart(16, "0");
  return isHex64(seed) ? { ok: true, seed } : { ok: false };
}

/** Writes a seed or an ID for the screen: its 16 hex digits in upper case. */
export function formatHex64(hex: string): string {
  return hex.toUpperCase();
}

/**
 * Writes a body's ID for the screen: its system's 16 hex digits, the full stop and its 4-digit
 * index, in upper case, as {@link formatHex64} writes a system's (`0200080020000000.010A`).
 */
export function formatBodyIdHex(id: BodyIdHex): string {
  return id.toUpperCase();
}
