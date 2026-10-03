import { describe, expect, it } from "vitest";

import { illuminanceLx } from "../photometry/magnitude";
import {
  REC709_LUMA,
  starIlluminanceRgbLx,
  starPixelLuminanceRgb,
  unitLuminanceRgb,
} from "./photometry";

/** Rec. 709 luminance of a linear colour. */
function luma(rgb: readonly [number, number, number]): number {
  return REC709_LUMA[0] * rgb[0] + REC709_LUMA[1] * rgb[1] + REC709_LUMA[2] * rgb[2];
}

describe("a sky star's display light", () => {
  it("takes a white chromaticity to white of unit luminance", () => {
    const [r, g, b] = unitLuminanceRgb(1 / 3, 1 / 3);
    expect(r).toBeCloseTo(1, 12);
    expect(g).toBeCloseTo(1, 12);
    expect(b).toBeCloseTo(1, 12);
  });

  it("keeps the star's photopic illuminance in its channels' luminance, whatever its colour", () => {
    for (const [cr, cg] of [
      [0.5, 0.35],
      [0.2, 0.3],
      [1 / 3, 1 / 3],
    ] as const) {
      expect(luma(starIlluminanceRgbLx(1.5, cr, cg)) / illuminanceLx(1.5)).toBeCloseTo(1, 12);
    }
  });

  it("is 2.54 µlx at V 0 and a hundredth of it at V 5", () => {
    expect(luma(starIlluminanceRgbLx(0, 1 / 3, 1 / 3))).toBeCloseTo(2.54e-6, 15);
    expect(luma(starIlluminanceRgbLx(5, 1 / 3, 1 / 3))).toBeCloseTo(2.54e-8, 17);
  });

  it("spreads a pixel's share over its solid angle per channel", () => {
    const [r] = starPixelLuminanceRgb([1e-6, 2e-6, 3e-6], 0.5, 1e-7);
    expect(r).toBeCloseTo(5, 12);
  });
});
