import { describe, expect, it } from "vitest";

import { COMPACT_BELOW_REM } from "./chartLayout";
import { mapLayout, mapPictureWidth, mapRows, overlayHasRoom } from "./mapLayout";

/** A page of `widthPx` by `heightPx` at a root font size of `remPx`. */
function page(widthPx: number, heightPx: number, remPx = 16) {
  return { widthPx, heightPx, devicePixelRatio: 1, remPx };
}

/**
 * The `GALAXY MAP` page at each size P07.T11.b's hidden runs measured (plan 07's record), in CSS
 * pixels: in the header strip as it is, and with a mode banner shown, which takes the page 2 px.
 */
const MEASURED = {
  "1280 × 720": { plain: page(798, 520), banner: page(798, 518) },
  "1536 × 864 (1920 × 1080 at 125%)": { plain: page(816.3, 664), banner: page(816.3, 662) },
  "1920 × 1080": { plain: page(1_035.7, 880), banner: page(1_035.7, 878) },
  "1280 × 1024": { plain: page(798, 824), banner: page(798, 822) },
} as const;

describe("mapLayout", () => {
  it("stacks the page until it is measured", () => {
    expect(mapLayout(null)).toBe("stacked");
  });

  it("lays out the 32.5 rem page of 1280 × 720 compact", () => {
    expect(mapLayout(MEASURED["1280 × 720"].plain)).toBe("compact");
  });

  it("stacks the 55 rem page of 1920 × 1080", () => {
    expect(mapLayout(MEASURED["1920 × 1080"].plain)).toBe("stacked");
  });

  it("changes layout where the local chart does, in rem whatever the interface scale", () => {
    expect(mapLayout(page(1_000, COMPACT_BELOW_REM * 16))).toBe("stacked");
    expect(mapLayout(page(1_000, COMPACT_BELOW_REM * 16 - 1))).toBe("compact");
    // 1920 × 1080 at 125%: the same pixels hold fewer rem.
    expect(mapLayout(MEASURED["1536 × 864 (1920 × 1080 at 125%)"].plain)).toBe("compact");
    expect(mapLayout(MEASURED["1280 × 1024"].plain)).toBe("stacked");
  });
});

describe("mapPictureWidth", () => {
  it("gives a stacked page's pictures the width beside the 24 rem words and the axes", () => {
    // 1,035.7 − 29.5 × 16 = 563.7 px across; (880 − 3 × 16) × 2 ÷ 3 = 554.7 px down.
    expect(mapPictureWidth(MEASURED["1920 × 1080"].plain, "stacked")).toBe(554);
    // 798 − 472 = 326 px across, less than the 517 px the height allows.
    expect(mapPictureWidth(MEASURED["1280 × 1024"].plain, "stacked")).toBe(326);
  });

  it("leaves a compact page's edge-on row its tallest plain words, 8.5 rem", () => {
    // 520 − 8 × 16 = 392 px for the rows: two thirds of it, 261.3 px, would leave the edge-on
    // picture 130 px, under its words' 136; so the face-on picture takes 392 − 136 = 256 px.
    expect(mapPictureWidth(MEASURED["1280 × 720"].plain, "compact")).toBe(256);
    expect(mapPictureWidth(MEASURED["1280 × 720"].banner, "compact")).toBe(254);
  });

  it("gives a compact page as wide as its height allows the width beside 27 rem of words", () => {
    // 816.3 − 32.5 × 16 = 296.3 px across, under the 357 px the rows' height allows.
    expect(mapPictureWidth(MEASURED["1536 × 864 (1920 × 1080 at 125%)"].plain, "compact")).toBe(
      296,
    );
  });

  it("leaves a short compact page's face-on row its tallest plain words, 9.875 rem", () => {
    // 300 − 128 = 172 px for the rows; the face-on row keeps 158 px, and the edge-on picture,
    // half the face-on's, the 14 px left: 2 × 14 = 28.
    expect(mapPictureWidth(page(1_200, 300), "compact")).toBe(28);
  });

  it("keeps the width even, so that the edge-on picture is whole pixels tall", () => {
    expect(mapPictureWidth(page(793, 800), "stacked") % 2).toBe(0);
    expect(mapPictureWidth(page(798, 521), "compact") % 2).toBe(0);
  });

  it("gives no width to a page too small for any picture", () => {
    expect(mapPictureWidth(page(400, 100), "compact")).toBe(0);
  });
});

describe("mapRows", () => {
  it("gives a stacked view its picture's height where the picture is the taller", () => {
    // The face-on view's 554 px picture starts 2.5 rem down; the edge-on view's is 277 px.
    expect(mapRows(MEASURED["1920 × 1080"].plain, "stacked")).toEqual({
      faceOnPx: 594,
      edgeOnPx: 277,
    });
  });

  it("gives a narrow stacked page's face-on view the height of its tallest plain words", () => {
    // At 1280 × 1024 the 326 px picture is under the controls' and words' 26.375 rem, 422 px.
    expect(mapRows(MEASURED["1280 × 1024"].plain, "stacked")).toEqual({
      faceOnPx: 462,
      edgeOnPx: 163,
    });
  });

  it("keeps a stacked page's views inside it at every size measured", () => {
    for (const { plain, banner } of [MEASURED["1920 × 1080"], MEASURED["1280 × 1024"]]) {
      for (const size of [plain, banner]) {
        const rows = mapRows(size, "stacked");
        expect(rows.faceOnPx + 8 + rows.edgeOnPx).toBeLessThanOrEqual(size.heightPx);
      }
    }
  });

  it("keeps a compact page's views and its controls inside it at every size measured", () => {
    for (const { plain, banner } of [
      MEASURED["1280 × 720"],
      MEASURED["1536 × 864 (1920 × 1080 at 125%)"],
    ]) {
      for (const size of [plain, banner]) {
        const rows = mapRows(size, "compact");
        // The controls' row is 7.5 rem, the gap between the views 0.5 rem.
        expect(120 + rows.faceOnPx + 8 + rows.edgeOnPx).toBeLessThanOrEqual(size.heightPx);
      }
    }
  });
});

describe("overlayHasRoom", () => {
  it.each([
    ["1280 × 720", "compact", false],
    ["1536 × 864 (1920 × 1080 at 125%)", "compact", false],
    ["1920 × 1080", "stacked", true],
    ["1280 × 1024", "stacked", false],
  ] as const)(
    "finds room at %s, laid out %s, %s, in the header strip as it is and under a banner",
    (name, layout, room) => {
      const { plain, banner } = MEASURED[name];
      expect(mapLayout(plain)).toBe(layout);
      expect(mapLayout(banner)).toBe(layout);
      expect(overlayHasRoom(plain, layout)).toBe(room);
      expect(overlayHasRoom(banner, layout)).toBe(room);
    },
  );

  it("needs the face-on view as tall as its words and the controls under the overlay, 31.125 rem stacked", () => {
    // Stacked pages whose pictures are 496 px, one step short of the words' 498, and 498 px. The
    // edge-on words, 14.125 rem, then fit too: half a 498 px picture is 249 px, over their 226.
    expect(overlayHasRoom(page(968, 800), "stacked")).toBe(false);
    expect(overlayHasRoom(page(970, 800), "stacked")).toBe(true);
  });

  it("has no room before the page has room for any picture", () => {
    expect(overlayHasRoom(page(400, 100), "compact")).toBe(false);
  });
});
