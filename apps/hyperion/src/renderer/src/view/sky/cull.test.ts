import type { SkyStars } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { cubeTexelOf } from "./cube";
import { cullSky, starIsSeen } from "./cull";
import { starIlluminanceRgbLx } from "./photometry";

/** Stars along fixed directions with the given V, white, no offsets unless given. */
function starsOf(
  vMag: readonly number[],
  options: { readonly eyeOffset?: number; readonly cameraBand?: number } = {},
): SkyStars {
  const count = vMag.length;
  const directions = new Float32Array(count * 3);
  for (let i = 0; i < count; i += 1) {
    const angle = (i * 2 * Math.PI) / count;
    directions.set([Math.cos(angle), Math.sin(angle), 0.3], i * 3);
  }
  return {
    count,
    directions,
    distanceLy: new Float32Array(count).fill(100),
    vMag: Float32Array.from(vMag),
    chroma: new Float32Array(count * 2).fill(1 / 3),
    eyeOffsetMag: new Float32Array(count).fill(options.eyeOffset ?? 0),
    cameraBandMag: new Float32Array(count).fill(options.cameraBand ?? 0),
  };
}

describe("cullSky", () => {
  it("keeps a camera's stars brighter than its limit, by V plus the band term", () => {
    const stars = starsOf([5, 7, 9, 9.4], { cameraBand: 0.2 });
    const culled = cullSky(stars, { kind: "camera", limitV: 9.5 }, 8);
    expect([...culled.kept]).toEqual([0, 1, 2]);
    expect(culled.culledCount).toBe(1);
  });

  it("keeps a red star that its band term lifts over a camera's limit", () => {
    const stars = starsOf([9.6], { cameraBand: -0.25 });
    expect(starIsSeen(stars, 0, { kind: "camera", limitV: 9.5 })).toBe(true);
  });

  it("puts a culled star's light in the band texel of its direction, and nowhere else", () => {
    const stars = starsOf([8]);
    stars.directions.set([0, 0, -1]);
    stars.chroma.set([0.5, 0.3]);
    const culled = cullSky(stars, { kind: "camera", limitV: 7 }, 4);
    const { face, column, row } = cubeTexelOf(0, 0, -1, 4);
    const at = ((face * 4 + row) * 4 + column) * 3;
    const light = starIlluminanceRgbLx(8, Math.fround(0.5), Math.fround(0.3));
    expect([...culled.bandIlluminanceLx.subarray(at, at + 3)]).toEqual([...light]);
    const elsewhere = culled.bandIlluminanceLx.reduce(
      (total, value, index) => (index >= at && index < at + 3 ? total : total + value),
      0,
    );
    expect(elsewhere).toBe(0);
  });

  it("keeps an eye's stars by the limit in their direction, moved by their colour offset", () => {
    const stars = starsOf([6, 6.7], { eyeOffset: 0.2 });
    const limit = { kind: "eye" as const, limitAt: (): number => 6.6 };
    expect(starIsSeen(stars, 0, limit)).toBe(true);
    expect(starIsSeen(stars, 1, limit)).toBe(true);
    expect(starIsSeen(stars, 1, { kind: "eye", limitAt: () => 6.4 })).toBe(false);
    expect(starIsSeen(stars, 1, { kind: "eye", limitAt: () => Number.NaN })).toBe(true);
  });

  it("hands every culled star's light to the band, to 10⁻⁶", () => {
    const vMag = Array.from({ length: 2_000 }, (_, i) => 4 + (i % 60) / 10);
    const stars = starsOf(vMag);
    const culled = cullSky(stars, { kind: "camera", limitV: 7 }, 16);
    let expected = 0;
    for (let i = 0; i < stars.count; i += 1) {
      if (!culled.kept.includes(i)) {
        expected += starIlluminanceRgbLx(stars.vMag[i] ?? 0, 1 / 3, 1 / 3)[1];
      }
    }
    let banded = 0;
    for (let texel = 1; texel < culled.bandIlluminanceLx.length; texel += 3) {
      banded += culled.bandIlluminanceLx[texel] ?? 0;
    }
    expect(culled.culledCount).toBeGreaterThan(0);
    expect(Math.abs(banded / expected - 1)).toBeLessThan(1e-6);
  });
});
