import { describe, expect, it } from "vitest";

import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import type { KernelPair } from "../kernels";
import type { TextureSpec } from "../memory";
import { PresentationOnlyReadback } from "../types";
import {
  assertBufferReadable,
  coversBuffer,
  coversTexture,
  paddedBytesPerRow,
  stagingLayout,
  textureRead,
  unpad,
  WriterRecord,
} from "./readback";

const EXACT: KernelPair = { name: "exact", reference: "", subgroup: null, readback: "bit-exact" };
const ROUGH: KernelPair = {
  name: "rough",
  reference: "",
  subgroup: null,
  readback: "presentation-only",
};

describe("the last-writer record", () => {
  it("refuses a resource a presentation-only kernel wrote last, naming the kernel", () => {
    const writers = new WriterRecord();
    const buffer = {};
    writers.wroteBy(buffer, ROUGH);
    expect(() => {
      writers.assertReadable(buffer, "cpu");
    }).toThrow(new PresentationOnlyReadback("rough"));
  });

  it("lets the tolerance access read it", () => {
    const writers = new WriterRecord();
    const buffer = {};
    writers.wroteBy(buffer, ROUGH);
    expect(() => {
      writers.assertReadable(buffer, "tolerance");
    }).not.toThrow();
  });

  it("reads a resource a bit-exact kernel wrote after the presentation-only one", () => {
    const writers = new WriterRecord();
    const buffer = {};
    writers.wroteBy(buffer, ROUGH);
    writers.wroteBy(buffer, EXACT);
    expect(() => {
      writers.assertReadable(buffer, "cpu");
    }).not.toThrow();
  });

  it("reads a resource an upload or a draw wrote after the kernel", () => {
    const writers = new WriterRecord();
    const texture = {};
    writers.wroteBy(texture, ROUGH);
    writers.wroteOtherwise(texture);
    expect(() => {
      writers.assertReadable(texture, "cpu");
    }).not.toThrow();
  });
});

describe("a staging buffer's layout", () => {
  it("pads each row to 256 bytes", () => {
    expect(stagingLayout(3, 2, 8)).toEqual({
      bytesPerRow: 256,
      rowBytes: 24,
      rows: 2,
      layers: 1,
      bytes: 512,
    });
    expect(paddedBytesPerRow(100, 4)).toBe(512);
  });

  it("unpads to tightly packed rows", () => {
    const layout = stagingLayout(1, 2, 4);
    const padded = new Uint8Array(layout.bytes);
    padded.set([1, 2, 3, 4], 0);
    padded.set([5, 6, 7, 8], 256);
    expect([...new Uint8Array(unpad(padded, layout))]).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
  });
});

/** A 2D texture of `format`, `64 × 32`, three mips, readable. */
function textureOf(format: GPUTextureFormat, overrides: Partial<TextureSpec> = {}): TextureSpec {
  return {
    name: "t",
    size: [64, 32],
    dimension: "2d",
    format,
    mips: 3,
    usage: TEXTURE_USAGE.COPY_SRC | TEXTURE_USAGE.TEXTURE_BINDING,
    category: "render-targets",
    ...overrides,
  };
}

describe("a texture read", () => {
  it.each([
    ["rgba16float", 8, "all"],
    ["rg11b10ufloat", 4, "all"],
    ["rgba8unorm", 4, "all"],
    ["depth32float", 4, "depth-only"],
  ] as const)("of %s copies %i bytes a texel through the %s aspect", (format, bytes, aspect) => {
    const read = textureRead(textureOf(format, { mips: 1 }), 0);
    expect(read).toMatchObject({ bytesPerTexel: bytes, aspect, width: 64, height: 32 });
    expect(stagingLayout(read.width, read.height, read.bytesPerTexel).bytesPerRow).toBe(
      paddedBytesPerRow(64, bytes),
    );
  });

  it("covers a level's size", () => {
    expect(textureRead(textureOf("rgba16float"), 2)).toMatchObject({ width: 16, height: 8 });
  });

  it("copies all six faces of a cube", () => {
    const cube = textureOf("rgb9e5ufloat", { size: [4, 4, 6], dimension: "cube" });
    expect(textureRead(cube, 1)).toMatchObject({ width: 2, height: 2, layers: 6 });
  });

  it("refuses a texture without COPY_SRC", () => {
    expect(() =>
      textureRead(textureOf("rgba8unorm", { usage: TEXTURE_USAGE.TEXTURE_BINDING }), 0),
    ).toThrow(/COPY_SRC/u);
  });

  it("refuses a level the texture lacks", () => {
    expect(() => textureRead(textureOf("rgba8unorm"), 3)).toThrow(/no level 3/u);
  });

  it("refuses a region outside the level", () => {
    expect(() =>
      textureRead(textureOf("rgba8unorm"), 1, { x: 30, y: 0, width: 4, height: 1 }),
    ).toThrow(/outside level 1/u);
  });

  it("refuses part of a depth level", () => {
    expect(() =>
      textureRead(textureOf("depth32float", { mips: 1 }), 0, { x: 0, y: 0, width: 1, height: 1 }),
    ).toThrow(/whole level/u);
  });

  it("refuses a depth whose depth cannot be copied", () => {
    expect(() => textureRead(textureOf("depth24plus", { mips: 1 }), 0)).toThrow(
      /cannot be copied/u,
    );
  });
});

describe("a buffer read", () => {
  const spec = { name: "b", bytes: 16, usage: BUFFER_USAGE.COPY_SRC, category: "other" } as const;

  it("needs COPY_SRC", () => {
    expect(() => {
      assertBufferReadable({ ...spec, usage: BUFFER_USAGE.STORAGE });
    }).toThrow(/COPY_SRC/u);
  });

  it("needs a size that is a multiple of 4", () => {
    expect(() => {
      assertBufferReadable({ ...spec, bytes: 6 });
    }).toThrow(/multiple of 4/u);
  });
});

describe("a copy", () => {
  it("carries its source's writer to its destination", () => {
    const writers = new WriterRecord();
    const buffer = {};
    const cube = {};
    writers.wroteBy(buffer, ROUGH);
    writers.copied(buffer, cube, true);
    expect(() => {
      writers.assertReadable(cube, "cpu");
    }).toThrow(PresentationOnlyReadback);
  });

  it("of part of a resource keeps the destination's presentation-only mark", () => {
    const writers = new WriterRecord();
    const buffer = {};
    const cube = {};
    writers.wroteBy(cube, ROUGH);
    writers.copied(buffer, cube, false);
    expect(() => {
      writers.assertReadable(cube, "cpu");
    }).toThrow(PresentationOnlyReadback);
  });
});

describe("a write's coverage", () => {
  const buffer = { name: "b", bytes: 16, usage: BUFFER_USAGE.COPY_DST, category: "other" } as const;

  it("covers a buffer only from its start to its end", () => {
    expect(coversBuffer(buffer, 0, 16)).toBe(true);
    expect(coversBuffer(buffer, 0, 4)).toBe(false);
    expect(coversBuffer(buffer, 4, 12)).toBe(false);
  });

  it("covers a texture only over every texel and layer of its one level", () => {
    const cube = textureOf("rgba8unorm", { mips: 1, size: [8, 8], dimension: "cube" });
    expect(coversTexture(cube, [0, 0, 0], [8, 8, 6], 0)).toBe(true);
    expect(coversTexture(cube, [0, 0, 0], [8, 8, 1], 0)).toBe(false);
    expect(coversTexture(cube, { x: 1 }, [8, 8, 6], 0)).toBe(false);
    const mipped = textureOf("rgba8unorm", { mips: 2, size: [8, 8] });
    expect(coversTexture(mipped, [0, 0], [8, 8], 0)).toBe(false);
  });
});
