import { crc32 } from "../view/stills/pngLabel";

/** A chunk of `type` holding `data`, framed: its length, type, data and CRC. */
export function pngChunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.byteLength);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.byteLength, false);
  for (let i = 0; i < 4; i += 1) {
    out[4 + i] = type.charCodeAt(i);
  }
  out.set(data, 8);
  view.setUint32(8 + data.byteLength, crc32(out.subarray(4, 8 + data.byteLength)), false);
  return out;
}

/** The chunk types of `png`, in order, read without checking anything: a test's view of a file. */
export function chunkTypes(png: Uint8Array): ReadonlyArray<string> {
  const types: string[] = [];
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength);
  for (let offset = 8; offset + 12 <= png.byteLength; offset += 12 + view.getUint32(offset)) {
    types.push(String.fromCharCode(...png.subarray(offset + 4, offset + 8)));
  }
  return types;
}

/** The Adler-32 of `bytes` (RFC 1950 §8.2), the zlib stream's check. */
function adler32(bytes: Uint8Array): number {
  let a = 1;
  let b = 0;
  for (const byte of bytes) {
    a = (a + byte) % 65_521;
    b = (b + a) % 65_521;
  }
  return ((b << 16) | a) >>> 0;
}

/** `raw` as a zlib stream of stored deflate blocks (RFC 1950, RFC 1951 §3.2.4): no compression. */
function storedZlib(raw: Uint8Array): Uint8Array {
  const blocks: Uint8Array[] = [Uint8Array.of(0x78, 0x01)];
  let at = 0;
  do {
    const end = Math.min(at + 65_535, raw.byteLength);
    const piece = raw.subarray(at, end);
    const head = new Uint8Array(5);
    head[0] = end === raw.byteLength ? 1 : 0;
    new DataView(head.buffer).setUint16(1, piece.byteLength, true);
    new DataView(head.buffer).setUint16(3, ~piece.byteLength & 0xffff, true);
    blocks.push(head, piece);
    at = end;
  } while (at < raw.byteLength);
  const check = new Uint8Array(4);
  new DataView(check.buffer).setUint32(0, adler32(raw), false);
  blocks.push(check);
  const out = new Uint8Array(blocks.reduce((sum, block) => sum + block.byteLength, 0));
  let offset = 0;
  for (const block of blocks) {
    out.set(block, offset);
    offset += block.byteLength;
  }
  return out;
}

/**
 * A real PNG of `widthPx` × `heightPx` 8-bit RGBA pixels, each row filtered with type 0, its data
 * stored uncompressed: a file any decoder reads, made without an encoder.
 *
 * @param rgba - The pixels, row by row from the top, 4 bytes each; a pattern of the pixel's
 *   coordinates when absent.
 */
export function rgbaPng(widthPx: number, heightPx: number, rgba?: Uint8Array): Uint8Array {
  const pixels =
    rgba ??
    Uint8Array.from({ length: widthPx * heightPx * 4 }, (_, i) => {
      const pixel = Math.floor(i / 4);
      return (pixel * 37 + (i % 4) * 71) & 0xff;
    });
  const raw = new Uint8Array(heightPx * (1 + 4 * widthPx));
  for (let row = 0; row < heightPx; row += 1) {
    raw[row * (1 + 4 * widthPx)] = 0;
    raw.set(
      pixels.subarray(row * 4 * widthPx, (row + 1) * 4 * widthPx),
      row * (1 + 4 * widthPx) + 1,
    );
  }
  const header = new Uint8Array(13);
  const view = new DataView(header.buffer);
  view.setUint32(0, widthPx, false);
  view.setUint32(4, heightPx, false);
  header.set([8, 6, 0, 0, 0], 8);
  const parts = [
    Uint8Array.of(137, 80, 78, 71, 13, 10, 26, 10),
    pngChunk("IHDR", header),
    pngChunk("IDAT", storedZlib(raw)),
    pngChunk("IEND", new Uint8Array(0)),
  ];
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.byteLength, 0));
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.byteLength;
  }
  return out;
}
