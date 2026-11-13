/**
 * A still's label in its PNG (plan R11, R11.T10.c, Design note 17): an `iTXt` chunk whose keyword
 * is `hyperion-still` and whose text is the label's one text (`stillLabel.ts`'s `stillLabelText`),
 * the same as the JSON sidecar's, so that the PNG carries its own record wherever it is copied.
 *
 * @remarks
 * The PNG specification (Portable Network Graphics (PNG) Specification, Third Edition, W3C
 * Recommendation of 24 June 2025, re-checked 2026-11-13) is followed for what is read and written
 * here: the eight-byte signature (§5.2); each chunk a four-byte big-endian length of at most
 * 2³¹ − 1, a type of four ASCII letters, its data and a CRC-32 of its type and data (§5.3, §5.4,
 * §5.5); `IHDR` first, its width and height from 1 to 2³¹ − 1 (§11.2.1), and `IEND` last (§5.6). An
 * `iTXt` chunk (§11.3.3.4) holds a keyword of 1 to 79 bytes and a null, a compression flag and
 * method (0 and 0 here: uncompressed), a language tag and a null, a translated keyword and a null,
 * then the text in UTF-8 with no null. The chunk goes straight after `IHDR`: §5.6 orders a text
 * chunk against no other chunk.
 *
 * A PNG is read in one pass ({@link scanPng}), which keeps nothing of a chunk but its frame unless
 * it is `IHDR` or a still's label, and checks the CRC of only those and `IEND`: the image's own
 * chunks are the decoder's to check, and a file of millions of tiny chunks then costs one pass and
 * no memory. Pure, with no platform API: the main process checks a still's chunk against its
 * sidecar (`main/stills.ts`), which is why this file is in `tsconfig.node.json`'s list.
 */

import type { StillLabel, StillSize } from "../../../../preload/api";
import { parseStillLabel, STILL_LABEL_MAX_BYTES, stillLabelText } from "./stillLabel";

/** The keyword of a still's `iTXt` chunk (Design note 17). */
export const STILL_PNG_KEYWORD = "hyperion-still";

/** The PNG signature (§5.2): 137 80 78 71 13 10 26 10. */
const SIGNATURE = Uint8Array.of(137, 80, 78, 71, 13, 10, 26, 10);

/** The largest chunk length and image side the specification allows: 2³¹ − 1 (§5.3, §11.2.1). */
const PNG_MAX_UINT = 0x7fff_ffff;

/** The bytes of a chunk around its data: its length, its type and its CRC. */
const CHUNK_FRAME_BYTES = 12;

/** `IHDR`'s data length (§11.2.1). */
const IHDR_BYTES = 13;

/** The CRC-32 table of §5.5's polynomial, reflected (0xEDB88320), as Annex D's sample code builds it. */
const CRC_TABLE: Uint32Array = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) {
      c = (c & 1) === 1 ? 0xedb8_8320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

/** The CRC-32 of `bytes` (§5.5): the register starts at all ones and is inverted at the end. */
export function crc32(bytes: Uint8Array): number {
  let c = 0xffff_ffff;
  for (const byte of bytes) {
    c = (CRC_TABLE[(c ^ byte) & 0xff] ?? 0) ^ (c >>> 8);
  }
  return (c ^ 0xffff_ffff) >>> 0;
}

/** What one pass over a PNG gave: its size and still labels, or why it is not a PNG. */
export type PngScan =
  | {
      readonly kind: "png";
      readonly size: StillSize;
      /** The offset of the chunk after `IHDR`, where a label goes. */
      readonly afterHeader: number;
      /** The data of each `iTXt` chunk with the still's keyword, in order. */
      readonly stillChunks: ReadonlyArray<Uint8Array>;
    }
  | { readonly kind: "not-png"; readonly reason: string };

/** Bytes that are not a PNG, and why. */
function notPng(reason: string): PngScan {
  return { kind: "not-png", reason };
}

/** A big-endian u32 at `offset`, which the caller has checked lies within `bytes`. */
function u32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, false);
}

/** Whether `code` is a chunk type's byte: an ASCII letter (§5.4). */
function isTypeByte(code: number | undefined): boolean {
  return code !== undefined && ((code >= 65 && code <= 90) || (code >= 97 && code <= 122));
}

/** The keyword's bytes, Latin-1 and here ASCII. */
const KEYWORD_BYTES = Uint8Array.from(STILL_PNG_KEYWORD, (ch) => ch.charCodeAt(0));

/** Whether `data`, an `iTXt` chunk's, has a still's keyword. */
function isStillChunk(data: Uint8Array): boolean {
  return (
    data.byteLength > KEYWORD_BYTES.byteLength &&
    KEYWORD_BYTES.every((byte, i) => data[i] === byte) &&
    data[KEYWORD_BYTES.byteLength] === 0
  );
}

/** Whether the chunk at `offset` of `length` data bytes has the CRC its frame says. */
function crcHolds(png: Uint8Array, offset: number, length: number): boolean {
  return crc32(png.subarray(offset + 4, offset + 8 + length)) === u32(png, offset + 8 + length);
}

/** Whether the four bytes at `offset` spell `type`. */
function typeIs(png: Uint8Array, offset: number, type: string): boolean {
  return (
    png[offset] === type.charCodeAt(0) &&
    png[offset + 1] === type.charCodeAt(1) &&
    png[offset + 2] === type.charCodeAt(2) &&
    png[offset + 3] === type.charCodeAt(3)
  );
}

/**
 * `png` read in one pass: its signature; every chunk's frame within the file and its type four
 * letters; `IHDR` first, of 13 bytes, its sides in range and its CRC holding; each still's label
 * chunk, its CRC holding; and `IEND` last, empty, at the file's end, its CRC holding.
 */
export function scanPng(png: Uint8Array): PngScan {
  if (png.byteLength < SIGNATURE.byteLength || !SIGNATURE.every((byte, i) => png[i] === byte)) {
    return notPng("no PNG signature");
  }
  let offset = SIGNATURE.byteLength;
  let size: StillSize | null = null;
  let afterHeader = 0;
  let ended = false;
  const stillChunks: Uint8Array[] = [];
  while (offset < png.byteLength && !ended) {
    if (png.byteLength - offset < CHUNK_FRAME_BYTES) {
      return notPng(`a chunk cut short at byte ${String(offset)}`);
    }
    const length = u32(png, offset);
    if (length > PNG_MAX_UINT || length > png.byteLength - offset - CHUNK_FRAME_BYTES) {
      return notPng(`a chunk's length runs past the file at byte ${String(offset)}`);
    }
    const at = offset + 4;
    if (![0, 1, 2, 3].every((k) => isTypeByte(png[at + k]))) {
      return notPng(`a chunk type that is not four letters at byte ${String(offset)}`);
    }
    if (size === null) {
      if (!typeIs(png, at, "IHDR") || length !== IHDR_BYTES || !crcHolds(png, offset, length)) {
        return notPng("IHDR is not the first chunk, of 13 bytes, its CRC holding");
      }
      const widthPx = u32(png, offset + 8);
      const heightPx = u32(png, offset + 12);
      if (widthPx < 1 || heightPx < 1 || widthPx > PNG_MAX_UINT || heightPx > PNG_MAX_UINT) {
        return notPng("IHDR's sides are out of range");
      }
      size = { widthPx, heightPx };
      afterHeader = offset + CHUNK_FRAME_BYTES + length;
    } else if (typeIs(png, at, "iTXt")) {
      const data = png.subarray(offset + 8, offset + 8 + length);
      if (isStillChunk(data)) {
        if (!crcHolds(png, offset, length)) {
          return notPng(`the ${STILL_PNG_KEYWORD} chunk at byte ${String(offset)} fails its CRC`);
        }
        stillChunks.push(data);
      }
    } else if (typeIs(png, at, "IEND")) {
      if (length !== 0 || !crcHolds(png, offset, length)) {
        return notPng("IEND is not empty, its CRC holding");
      }
      ended = true;
    }
    offset += CHUNK_FRAME_BYTES + length;
  }
  if (size === null || !ended || offset !== png.byteLength) {
    return notPng("IEND is not the last chunk, at the file's end");
  }
  return { kind: "png", size, afterHeader, stillChunks };
}

/** The image's size from `IHDR`, or `null` where `png` is not a PNG. */
export function pngImageSize(png: Uint8Array): StillSize | null {
  const scan = scanPng(png);
  return scan.kind === "png" ? scan.size : null;
}

/** A chunk of `type` holding `data`, framed: length, type, data, CRC. */
function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(CHUNK_FRAME_BYTES + data.byteLength);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.byteLength, false);
  for (let i = 0; i < 4; i += 1) {
    out[4 + i] = type.charCodeAt(i);
  }
  out.set(data, 8);
  view.setUint32(8 + data.byteLength, crc32(out.subarray(4, 8 + data.byteLength)), false);
  return out;
}

/**
 * The `iTXt` chunk's data for `label`: the keyword and its null, the compression flag and method
 * (0, 0: uncompressed), an empty language tag and translated keyword, each with its null, then the
 * label's text in UTF-8.
 */
function labelChunkData(label: StillLabel): Uint8Array {
  const words = new TextEncoder().encode(stillLabelText(label));
  const head = [...KEYWORD_BYTES, 0, 0, 0, 0, 0];
  const data = new Uint8Array(head.length + words.byteLength);
  data.set(head, 0);
  data.set(words, head.length);
  return data;
}

/**
 * `png` with `label` in an `iTXt` chunk `hyperion-still` straight after its `IHDR`.
 *
 * @throws RangeError where `png` is not a PNG ({@link scanPng}), already holds a `hyperion-still`
 *   chunk, or is not of the label's size; or where the label is not one of the schema's.
 */
export function withStillLabel(png: Uint8Array, label: StillLabel): Uint8Array {
  const scan = scanPng(png);
  if (scan.kind === "not-png") {
    throw new RangeError(`a still's label goes into a PNG: ${scan.reason}`);
  }
  if (scan.stillChunks.length > 0) {
    throw new RangeError(`the PNG already holds a ${STILL_PNG_KEYWORD} chunk`);
  }
  if (scan.size.widthPx !== label.size.widthPx || scan.size.heightPx !== label.size.heightPx) {
    throw new RangeError("the PNG is not of the label's size");
  }
  const added = chunk("iTXt", labelChunkData(label));
  const at = scan.afterHeader;
  const out = new Uint8Array(png.byteLength + added.byteLength);
  out.set(png.subarray(0, at), 0);
  out.set(added, at);
  out.set(png.subarray(at), at + added.byteLength);
  return out;
}

/** What a PNG's still label read back as. */
export type PngLabelRead =
  | {
      readonly kind: "label";
      readonly label: StillLabel;
      /** The chunk's text as written, the label's one text. */
      readonly text: string;
    }
  /** No `hyperion-still` chunk. */
  | { readonly kind: "none" }
  /** The bytes are not a PNG, or its chunk is not a label of the schema, or there are two. */
  | { readonly kind: "malformed"; readonly reason: string };

/**
 * The label in a scanned PNG's `hyperion-still` chunk, read back through the schema.
 *
 * @remarks
 * The chunk's text must be the label's one text: a text whose JSON is a label but written otherwise
 * (another key order, spacing) is `malformed`, so that a chunk that reads back is byte for byte the
 * sidecar the same label writes.
 */
export function labelOfScan(scan: PngScan): PngLabelRead {
  if (scan.kind === "not-png") {
    return { kind: "malformed", reason: scan.reason };
  }
  const [only, another] = scan.stillChunks;
  if (only === undefined) {
    return { kind: "none" };
  }
  if (another !== undefined) {
    return { kind: "malformed", reason: `more than one ${STILL_PNG_KEYWORD} chunk` };
  }
  const at = KEYWORD_BYTES.byteLength + 1;
  // Uncompressed (flag 0, method 0), then an empty language tag and translated keyword.
  if (only[at] !== 0 || only[at + 1] !== 0 || only[at + 2] !== 0 || only[at + 3] !== 0) {
    return { kind: "malformed", reason: "the chunk is compressed or has a language tag" };
  }
  const words = only.subarray(at + 4);
  if (words.byteLength > STILL_LABEL_MAX_BYTES) {
    return { kind: "malformed", reason: "the chunk's text is over the label's bound" };
  }
  let text: string;
  let parsed: unknown;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(words);
    parsed = JSON.parse(text);
  } catch (error: unknown) {
    const why = error instanceof Error ? error.message : String(error);
    return { kind: "malformed", reason: `the chunk's text is not JSON in UTF-8: ${why}` };
  }
  const label = parseStillLabel(parsed);
  if (label === null) {
    return { kind: "malformed", reason: "the chunk's text is not a label of the schema" };
  }
  if (stillLabelText(label) !== text) {
    return { kind: "malformed", reason: "the chunk's text is not the label's one text" };
  }
  return { kind: "label", label, text };
}

/** The label in `png`'s `hyperion-still` chunk, read back through the schema ({@link labelOfScan}). */
export function readStillLabel(png: Uint8Array): PngLabelRead {
  return labelOfScan(scanPng(png));
}
