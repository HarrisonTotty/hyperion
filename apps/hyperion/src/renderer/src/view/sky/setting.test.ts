import { describe, expect, it } from "vitest";

import { MAX_SKY_STARS } from "@hyperion/protocol";

import { SETTINGS } from "../quality/qualitySetting";
import { mipSizes } from "./mips";
import { SKY_LAYERS } from "./setting";

describe("the sky's settings", () => {
  it("are Design note 22's figures in R05's one list", () => {
    expect(SETTINGS.high.sky).toEqual({
      faceSizePx: 3_072,
      spriteBudget: 4_096,
      nMax: 300_000,
      rebakeShiftPx: 0.1,
    });
    expect(SETTINGS.low.sky).toEqual({
      faceSizePx: 1_024,
      spriteBudget: 2_048,
      nMax: 100_000,
      rebakeShiftPx: 0.1,
    });
  });

  it("ask no more stars than the protocol lists", () => {
    expect(SETTINGS.high.sky.nMax).toBeLessThanOrEqual(MAX_SKY_STARS);
    expect(SETTINGS.low.sky.nMax).toBeLessThanOrEqual(MAX_SKY_STARS);
  });

  it("bake faces whose mip chains reach one texel", () => {
    expect(mipSizes(SETTINGS.high.sky.faceSizePx).at(-1)).toBe(1);
    expect(mipSizes(SETTINGS.low.sky.faceSizePx).at(-1)).toBe(1);
  });

  it("draw the band and the discs only in the photorealistic style", () => {
    expect(SKY_LAYERS.wireframe).toEqual({ sprites: true, cube: true, band: false, discs: false });
    expect(SKY_LAYERS.photorealistic).toEqual({
      sprites: true,
      cube: true,
      band: true,
      discs: true,
    });
  });
});
