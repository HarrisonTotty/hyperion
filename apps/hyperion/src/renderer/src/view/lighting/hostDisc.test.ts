import { describe, expect, it } from "vitest";

import { solarFixtureRow } from "../../test/litFixtures";
import {
  frozenHostDisc,
  SUN_BAKE_SPECTRUM,
  SUN_COLOUR_RGB,
  SUN_LUX_PER_V0,
  sunLikeHostDisc,
} from "./hostDisc";
import { CHANNEL_LUMINANCE, photopicIlluminance } from "./illuminance";

describe("sunLikeHostDisc's defaults (R06's Sun, plan R08, R08.T6.j)", () => {
  it("carry the chroma, lux_per_v0 and bake spectrum of the Sun's row of R06's fixture", () => {
    const row = solarFixtureRow();
    const disc = sunLikeHostDisc();
    expect(disc.chroma).toEqual(row.chroma);
    expect(disc.lux_per_v0).toBe(row.lux_per_v0);
    expect(disc.bake_spectrum).toEqual(row.bake_spectrum);
    expect([SUN_COLOUR_RGB[0], SUN_COLOUR_RGB[1]]).toEqual(row.chroma);
    expect(SUN_LUX_PER_V0).toBe(row.lux_per_v0);
    expect([...SUN_BAKE_SPECTRUM]).toEqual(row.bake_spectrum);
  });

  it("take the blue from R06's luminance row, as R06's blue() does", () => {
    const [r, g, b] = SUN_COLOUR_RGB;
    const [yr, yg, yb] = CHANNEL_LUMINANCE;
    expect(b).toBe((1 - yr * r - yg * g) / yb);
    expect(photopicIlluminance(SUN_COLOUR_RGB)).toBeCloseTo(1, 12);
    // R06's blue() from the unrounded red and green is 0.9340062696; the wire's f32 chroma moves
    // it by 4.4 × 10⁻⁷.
    expect(Math.abs(b - 0.934_006_269_558_386_1)).toBeLessThan(5e-7);
  });

  it("split the photopic mean by R06's colour, B, V and R as R06 orders them", () => {
    const disc = sunLikeHostDisc();
    const [lb, lv, lr] = disc.mean_luminance_cd_m2;
    const mean = photopicIlluminance([lr, lv, lb]);
    expect(lr / mean).toBeCloseTo(SUN_COLOUR_RGB[0], 12);
    expect(lv / mean).toBeCloseTo(SUN_COLOUR_RGB[1], 12);
    expect(lb / mean).toBeCloseTo(SUN_COLOUR_RGB[2], 12);
  });

  it("give way to an override", () => {
    const grey = sunLikeHostDisc({ lux_per_v0: 1, colourRgb: [1, 1, 1] });
    expect(grey.lux_per_v0).toBe(1);
    expect(grey.chroma).toEqual([1, 1]);
    const [gb, gv, gr] = grey.mean_luminance_cd_m2;
    expect(gb).toBe(gv);
    expect(gr).toBe(gv);
    const [lb, lv, lr] = sunLikeHostDisc().mean_luminance_cd_m2;
    expect(gv / photopicIlluminance([lr, lv, lb])).toBeCloseTo(1 / SUN_LUX_PER_V0, 12);
  });

  it("copy the bake spectrum, so that a disc's edit cannot reach the constant", () => {
    const disc = sunLikeHostDisc();
    expect(disc.bake_spectrum).not.toBe(SUN_BAKE_SPECTRUM);
    expect(Object.isFrozen(SUN_BAKE_SPECTRUM)).toBe(true);
  });
});

describe("frozenHostDisc", () => {
  it("refuses an edit of a held disc's limb law in place", () => {
    const disc = frozenHostDisc(sunLikeHostDisc());
    expect(() => {
      disc.limb[2].c = 0.5;
    }).toThrow(TypeError);
  });

  it("refuses an edit of a held disc's luminance in place", () => {
    const disc = frozenHostDisc(sunLikeHostDisc());
    expect(() => {
      disc.mean_luminance_cd_m2[0] = 1;
    }).toThrow(TypeError);
  });
});
