import { describe, expect, it } from "vitest";

import { COMPACT_BELOW_REM } from "./chartLayout";
import { mapLayout, mapPictureWidth } from "./mapLayout";

/** A page of `widthPx` by `heightPx` at a root font size of `remPx`. */
function page(widthPx: number, heightPx: number, remPx = 16) {
  return { widthPx, heightPx, devicePixelRatio: 1, remPx };
}

describe("mapLayout", () => {
  it("stacks the page until it is measured", () => {
    expect(mapLayout(null)).toBe("stacked");
  });

  it("lays out the 32.5 rem page of 1280 × 720 compact", () => {
    expect(mapLayout(page(798, 520))).toBe("compact");
  });

  it("stacks the 55 rem page of 1920 × 1080", () => {
    expect(mapLayout(page(1_036, 880))).toBe("stacked");
  });

  it("changes layout where the local chart does, in rem whatever the interface scale", () => {
    expect(mapLayout(page(1_000, COMPACT_BELOW_REM * 16))).toBe("stacked");
    expect(mapLayout(page(1_000, COMPACT_BELOW_REM * 16 - 1))).toBe("compact");
    // 1920 × 1080 at 125%: the same pixels hold fewer rem.
    expect(mapLayout(page(1_036, 840, 20))).toBe("compact");
  });
});

describe("mapPictureWidth", () => {
  it("gives a stacked page's pictures the width beside the 24 rem words and the axes", () => {
    // 1,036 − 29.5 × 16 = 564 px across; (880 − 3 × 16) × 2 ÷ 3 = 554.7 px down.
    expect(mapPictureWidth(page(1_036, 880), "stacked")).toBe(554);
    // 792 − 472 = 320 px across, less than the 501 px the height allows.
    expect(mapPictureWidth(page(792, 800), "stacked")).toBe(320);
  });

  it("gives a compact page's pictures the height under the controls' row", () => {
    // 798 − 32.5 × 16 = 278 px across; (520 − 8 × 16) × 2 ÷ 3 = 261.3 px down, leaving the words
    // 450 px, 28 rem.
    expect(mapPictureWidth(page(798, 520), "compact")).toBe(260);
  });

  it("keeps the width even, so that the edge-on picture is whole pixels tall", () => {
    expect(mapPictureWidth(page(793, 800), "stacked") % 2).toBe(0);
    expect(mapPictureWidth(page(798, 521), "compact") % 2).toBe(0);
  });

  it("gives no width to a page too small for any picture", () => {
    expect(mapPictureWidth(page(400, 100), "compact")).toBe(0);
  });
});
