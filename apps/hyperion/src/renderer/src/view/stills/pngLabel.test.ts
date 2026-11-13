import { describe, expect, it } from "vitest";

import { chunkTypes, pngChunk, rgbaPng } from "../../test/pngFixtures";
import { testStillLabel } from "../../test/stillFixtures";
import {
  crc32,
  pngImageSize,
  readStillLabel,
  scanPng,
  STILL_PNG_KEYWORD,
  withStillLabel,
} from "./pngLabel";
import { stillLabelText } from "./stillLabel";

/** `png` with an `iTXt` chunk of `data` after `IHDR`, framed by hand. */
function withChunk(png: Uint8Array, data: Uint8Array): Uint8Array {
  const at = 8 + 12 + 13;
  const added = pngChunk("iTXt", data);
  const out = new Uint8Array(png.byteLength + added.byteLength);
  out.set(png.subarray(0, at), 0);
  out.set(added, at);
  out.set(png.subarray(at), at + added.byteLength);
  return out;
}

/** An `iTXt` chunk's data for `text` under the still's keyword, uncompressed, with no language. */
function itxt(text: string): Uint8Array {
  const head = Array.from(STILL_PNG_KEYWORD, (ch) => ch.charCodeAt(0));
  return Uint8Array.from([...head, 0, 0, 0, 0, 0, ...new TextEncoder().encode(text)]);
}

describe("a still's label in its PNG (R11.T10.c)", () => {
  it("takes the CRC-32 the PNG specification gives, as its check value", () => {
    // CRC-32's check value: the CRC of the nine ASCII digits "123456789".
    expect(crc32(new TextEncoder().encode("123456789"))).toBe(0xcbf4_3926);
  });

  it("parses back equal to the sidecar, text for text", () => {
    const label = testStillLabel();
    const read = readStillLabel(withStillLabel(rgbaPng(16, 8), label));
    expect(read).toEqual({ kind: "label", label, text: stillLabelText(label) });
  });

  it("goes in an iTXt chunk straight after IHDR, the image's chunks kept as they were", () => {
    const png = rgbaPng(16, 8);
    const labelled = withStillLabel(png, testStillLabel());
    expect([chunkTypes(png), chunkTypes(labelled), pngImageSize(labelled)]).toEqual([
      ["IHDR", "IDAT", "IEND"],
      ["IHDR", "iTXt", "IDAT", "IEND"],
      { widthPx: 16, heightPx: 8 },
    ]);
  });

  it("leaves every byte of the image's own chunks as they were", () => {
    const png = rgbaPng(16, 8);
    const labelled = withStillLabel(png, testStillLabel());
    const added = labelled.byteLength - png.byteLength;
    const at = 8 + 12 + 13;
    expect([
      labelled.subarray(0, at).every((byte, i) => byte === png[i]),
      labelled.subarray(at + added).every((byte, i) => byte === png[at + i]),
    ]).toEqual([true, true]);
  });

  it("reads none from a PNG without one", () => {
    expect(readStillLabel(rgbaPng(16, 8))).toEqual({ kind: "none" });
  });

  it("refuses a second label in the same PNG", () => {
    const labelled = withStillLabel(rgbaPng(16, 8), testStillLabel());
    expect(() => withStillLabel(labelled, testStillLabel())).toThrow(RangeError);
  });

  it("refuses a label of another size than the PNG's", () => {
    expect(() => withStillLabel(rgbaPng(32, 8), testStillLabel())).toThrow(RangeError);
  });

  it("refuses bytes that are not a PNG", () => {
    expect(() => withStillLabel(new Uint8Array(64), testStillLabel())).toThrow(RangeError);
  });

  it("finds a chunk whose CRC fails, and reads it as no PNG", () => {
    const labelled = withStillLabel(rgbaPng(16, 8), testStillLabel());
    const broken = labelled.slice();
    // A byte of the label's text, inside the iTXt chunk.
    const at = 8 + 25 + 8 + STILL_PNG_KEYWORD.length + 10;
    broken[at] = (broken[at] ?? 0) ^ 0x20;
    expect(readStillLabel(broken)).toMatchObject({ kind: "malformed" });
  });

  it("refuses a chunk whose text is a label written otherwise than its one text", () => {
    const label = testStillLabel();
    const spaced = JSON.stringify(JSON.parse(stillLabelText(label)), null, 2);
    expect(readStillLabel(withChunk(rgbaPng(16, 8), itxt(spaced)))).toEqual({
      kind: "malformed",
      reason: "the chunk's text is not the label's one text",
    });
  });

  it("refuses a chunk whose text is not a label of the schema", () => {
    expect(readStillLabel(withChunk(rgbaPng(16, 8), itxt('{"schema":"other"}')))).toEqual({
      kind: "malformed",
      reason: "the chunk's text is not a label of the schema",
    });
  });

  it("refuses two labels in one PNG", () => {
    const text = stillLabelText(testStillLabel());
    const twice = withChunk(withChunk(rgbaPng(16, 8), itxt(text)), itxt(text));
    expect(readStillLabel(twice)).toMatchObject({ kind: "malformed" });
  });

  it.each([
    ["a file cut short", (png: Uint8Array) => png.subarray(0, png.byteLength - 4)],
    ["bytes after IEND", (png: Uint8Array) => Uint8Array.from([...png, 0])],
    ["no signature", (png: Uint8Array) => Uint8Array.from([0, ...png.subarray(1)])],
  ])("reads %s as no PNG", (_, change) => {
    expect(scanPng(change(rgbaPng(16, 8))).kind).toBe("not-png");
  });
});
