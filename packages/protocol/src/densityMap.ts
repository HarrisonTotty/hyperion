import type { DensityMap } from "./generated/DensityMap";
import type { ExtinctionMap } from "./generated/ExtinctionMap";

/** The standard base64 alphabet, in sextet order. */
const BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/** The sextet each ASCII character stands for, or −1 for one outside the alphabet. */
const SEXTET_OF = ((): Int8Array => {
  const table = new Int8Array(128).fill(-1);
  for (let sextet = 0; sextet < BASE64_ALPHABET.length; sextet += 1) {
    table[BASE64_ALPHABET.charCodeAt(sextet)] = sextet;
  }
  return table;
})();

const PADDING = "=".charCodeAt(0);

/**
 * Decodes standard base64 with padding.
 *
 * @remarks
 * Written out because this package has no DOM types, and so no `atob`.
 *
 * @throws Error if the length is not a multiple of four or a character is not in the alphabet.
 */
function decodeBase64(text: string): Uint8Array {
  if (text.length % 4 !== 0) {
    throw new Error(`base64 text of length ${text.length} is not a whole number of quads`);
  }
  const padding = text.endsWith("==") ? 2 : text.endsWith("=") ? 1 : 0;
  const bytes = new Uint8Array((text.length / 4) * 3 - padding);
  let written = 0;
  for (let quadStart = 0; quadStart < text.length; quadStart += 4) {
    let quad = 0;
    for (let index = quadStart; index < quadStart + 4; index += 1) {
      const charCode = text.charCodeAt(index);
      const isPadding = charCode === PADDING && index >= text.length - padding;
      const sextet = isPadding ? 0 : (SEXTET_OF[charCode] ?? -1);
      if (sextet < 0) {
        throw new Error(`invalid base64 character at offset ${index}`);
      }
      quad = (quad << 6) | sextet;
    }
    for (let shift = 16; shift >= 0 && written < bytes.length; shift -= 8) {
      bytes[written] = (quad >> shift) & 0xff;
      written += 1;
    }
  }
  return bytes;
}

/** Reads `count` little-endian 16-bit codes. */
function readLittleEndianU16(bytes: Uint8Array, count: number): Uint16Array {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const codes = new Uint16Array(count);
  for (let index = 0; index < count; index += 1) {
    codes[index] = view.getUint16(2 * index, true);
  }
  return codes;
}

/** Bytes per code at each bit depth the protocol allows. */
function bytesPerCode(bits: number): 1 | 2 {
  if (bits === 8) {
    return 1;
  }
  if (bits === 16) {
    return 2;
  }
  throw new Error(`unsupported map depth of ${bits} bits`);
}

/**
 * A density map's pixel codes, decoded, with the rule that turns a code back into a density.
 *
 * @remarks
 * Codes run row by row from the top and left to right within a row, as {@link DensityMap}
 * describes along with the map's orientation and extent.
 */
export interface DecodedDensityMap {
  /** Pixels per row. */
  readonly widthPx: number;
  /** Rows. */
  readonly heightPx: number;
  /** One code per pixel: `Uint8Array` at 8 bits, `Uint16Array` at 16 bits. */
  readonly codes: Uint8Array | Uint16Array;
  /** The largest code, 2^bits − 1, which stands for the map's ceiling. */
  readonly maxCode: number;
  /**
   * The column density a code stands for, as log₁₀ of systems per square light-year.
   *
   * @returns `null` for code 0, which means at or below the floor, or empty.
   * @throws RangeError if `code` is not an integer from 0 to {@link DecodedDensityMap.maxCode}.
   */
  log10PerLy2(code: number): number | null;
}

/** What both map payloads carry: the raster's size, its depth and its encoded codes. */
interface QuantisedPayload {
  readonly width_px: number;
  readonly height_px: number;
  readonly bits: number;
  readonly data_base64: string;
}

/** A payload's codes, decoded, and the rule from a code back to log₁₀ of the quantity. */
interface DecodedPayload {
  readonly widthPx: number;
  readonly heightPx: number;
  readonly codes: Uint8Array | Uint16Array;
  readonly maxCode: number;
  readonly log10Of: (code: number) => number | null;
}

/**
 * Decodes a quantised map's payload: the part `decodeDensityMap` and `decodeExtinctionMap` share.
 *
 * @param kind - The map's name, for the error messages.
 * @param floor - log₁₀ of the quantity code 1 stands for.
 * @param ceiling - log₁₀ of the quantity the largest code stands for.
 * @throws Error if the payload is not valid base64, the depth is neither 8 nor 16 bits, or the byte
 *   count is not `width_px × height_px × bits ÷ 8`.
 */
function decodePayload(
  kind: string,
  map: QuantisedPayload,
  floor: number,
  ceiling: number,
): DecodedPayload {
  const codeBytes = bytesPerCode(map.bits);
  const bytes = decodeBase64(map.data_base64);
  const pixels = map.width_px * map.height_px;
  if (bytes.length !== pixels * codeBytes) {
    throw new Error(
      `${kind} holds ${bytes.length} bytes where ${map.width_px} × ${map.height_px} ` +
        `pixels at ${map.bits} bits need ${pixels * codeBytes}`,
    );
  }
  const codes = codeBytes === 1 ? bytes : readLittleEndianU16(bytes, pixels);
  const maxCode = 2 ** map.bits - 1;
  const span = ceiling - floor;
  return {
    widthPx: map.width_px,
    heightPx: map.height_px,
    codes,
    maxCode,
    log10Of: (code: number): number | null => {
      if (!Number.isInteger(code) || code < 0 || code > maxCode) {
        throw new RangeError(`${code} is not a code of a ${map.bits}-bit map`);
      }
      return code === 0 ? null : floor + ((code - 1) * span) / (maxCode - 1);
    },
  };
}

/**
 * Decodes a density map's base64 payload into pixel codes.
 *
 * @throws Error if the payload is not valid base64, the depth is neither 8 nor 16 bits, or the byte
 *   count is not `width_px × height_px × bits ÷ 8`.
 */
export function decodeDensityMap(map: DensityMap): DecodedDensityMap {
  const { log10Of, ...raster } = decodePayload(
    "density map",
    map,
    map.floor_log10_per_ly2,
    map.ceiling_log10_per_ly2,
  );
  return { ...raster, log10PerLy2: log10Of };
}

/**
 * An extinction map's pixel codes, decoded, with the rule that turns a code back into a visual
 * extinction.
 *
 * @remarks
 * The raster is laid out as a density map's is ({@link DecodedDensityMap}); only the quantity
 * differs, so the accessor has another name and the two are not interchangeable.
 */
export interface DecodedExtinctionMap {
  /** Pixels per row. */
  readonly widthPx: number;
  /** Rows. */
  readonly heightPx: number;
  /** One code per pixel: `Uint8Array` at 8 bits, `Uint16Array` at 16 bits. */
  readonly codes: Uint8Array | Uint16Array;
  /** The largest code, 2^bits − 1, which stands for the map's ceiling. */
  readonly maxCode: number;
  /**
   * The visual extinction A(V) a code stands for, as log₁₀ of magnitudes.
   *
   * @returns `null` for code 0, which means at or below the floor of 0.01 mag.
   * @throws RangeError if `code` is not an integer from 0 to {@link DecodedExtinctionMap.maxCode}.
   */
  log10Mag(code: number): number | null;
}

/**
 * Decodes an extinction map's base64 payload into pixel codes.
 *
 * @throws Error if the payload is not valid base64, the depth is neither 8 nor 16 bits, or the byte
 *   count is not `width_px × height_px × bits ÷ 8`.
 */
export function decodeExtinctionMap(map: ExtinctionMap): DecodedExtinctionMap {
  const { log10Of, ...raster } = decodePayload(
    "extinction map",
    map,
    map.floor_log10_mag,
    map.ceiling_log10_mag,
  );
  return { ...raster, log10Mag: log10Of };
}
