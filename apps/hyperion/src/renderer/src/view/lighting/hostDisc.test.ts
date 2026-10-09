import { describe, expect, it } from "vitest";

import { frozenHostDisc, sunLikeHostDisc } from "./hostDisc";

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
