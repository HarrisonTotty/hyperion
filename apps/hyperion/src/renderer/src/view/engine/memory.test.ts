import { describe, expect, it } from "vitest";

import { TEXTURE_USAGE } from "./gpuFlags";
import { extentOf, type TextureSpec, textureBytes } from "./memory";

function spec(overrides: Partial<TextureSpec>): TextureSpec {
  return {
    name: "t",
    size: [4, 4],
    dimension: "2d",
    format: "rgba8unorm",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING,
    category: "other",
    ...overrides,
  };
}

describe("a texture's bytes", () => {
  it("are texels times their size for one 2D level", () => {
    expect(textureBytes(spec({ size: [4, 4] }))).toBe(4 * 4 * 4);
  });

  it("count each mip, halving to no less than one texel", () => {
    // 8 × 2, 4 × 1, 2 × 1, 1 × 1 of rgba16float (8 bytes).
    expect(textureBytes(spec({ size: [8, 2], format: "rgba16float", mips: 4 }))).toBe(
      (16 + 4 + 2 + 1) * 8,
    );
  });

  it("count a 2D texture's layers at every level", () => {
    expect(textureBytes(spec({ size: [4, 4, 3], mips: 2 }))).toBe((16 + 4) * 3 * 4);
  });

  it("halve a 3D texture's depth with its mips", () => {
    // 4 × 4 × 4, then 2 × 2 × 2, of r32float.
    expect(
      textureBytes(spec({ size: [4, 4, 4], dimension: "3d", format: "r32float", mips: 2 })),
    ).toBe((64 + 8) * 4);
  });

  it("count a cube's six faces", () => {
    // A 4-texel cube of rgb9e5ufloat with three mips: (16 + 4 + 1) texels a face.
    expect(
      textureBytes(spec({ size: [4, 4, 6], dimension: "cube", format: "rgb9e5ufloat", mips: 3 })),
    ).toBe((16 + 4 + 1) * 6 * 4);
  });

  it("refuse a compressed format", () => {
    expect(() => textureBytes(spec({ format: "bc7-rgba-unorm" }))).toThrow(/bc7-rgba-unorm/u);
  });
});

describe("an extent", () => {
  it("reads either form, the absent axes 1", () => {
    expect(extentOf([8])).toEqual({ width: 8, height: 1, depthOrArrayLayers: 1 });
    expect(extentOf({ width: 8, height: 2 })).toEqual({
      width: 8,
      height: 2,
      depthOrArrayLayers: 1,
    });
  });
});
