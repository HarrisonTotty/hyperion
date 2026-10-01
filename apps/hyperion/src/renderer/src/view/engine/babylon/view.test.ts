import { describe, expect, it } from "vitest";

import { paddedBytesPerRow } from "./readback";
import { srgbViewFormat, unpadRows } from "./view";

describe("a view's canvas", () => {
  it("renders through the sRGB twin of its 8-bit format", () => {
    expect(srgbViewFormat("bgra8unorm")).toBe("bgra8unorm-srgb");
    expect(srgbViewFormat("rgba8unorm")).toBe("rgba8unorm-srgb");
  });

  it("refuses a format with no sRGB view", () => {
    expect(() => srgbViewFormat("rgba16float")).toThrow(/rgba16float/u);
  });
});

describe("a readback's texels", () => {
  it("lose their row padding", () => {
    const padded = new Uint8Array(512);
    padded.set([1, 2, 3, 4, 5, 6, 7, 8], 0);
    padded.set([9, 10, 11, 12, 13, 14, 15, 16], 256);
    expect([...unpadRows(padded, 2, 2, 256, false)]).toEqual([
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
    ]);
  });

  it("come out in RGBA order from a BGRA canvas", () => {
    const padded = new Uint8Array(256);
    padded.set([30, 20, 10, 255], 0);
    expect([...unpadRows(padded, 1, 1, 256, true)]).toEqual([10, 20, 30, 255]);
  });
});

describe("a readback's rows", () => {
  it("are padded to 256 bytes", () => {
    expect(paddedBytesPerRow(64, 4)).toBe(256);
    expect(paddedBytesPerRow(65, 4)).toBe(512);
    expect(paddedBytesPerRow(3, 8)).toBe(256);
    expect(paddedBytesPerRow(128, 8)).toBe(1024);
  });
});
