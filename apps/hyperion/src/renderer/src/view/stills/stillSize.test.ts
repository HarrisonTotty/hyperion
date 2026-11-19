import { describe, expect, it } from "vitest";

import { STILL_MAX_SIDE_PX } from "./stillLabel";
import { STILL_SCALES, stillSizeAt, stillSizeOptions, stillSizeWithin } from "./stillSize";

describe("a still's size (R11.T10.e.1)", () => {
  it("is a whole multiple of the view's output size, its aspect the view's", () => {
    expect(stillSizeAt({ widthPx: 1920, heightPx: 1080 }, 3)).toEqual({
      widthPx: 5760,
      heightPx: 3240,
    });
  });

  it("offers every multiple whose sides are within the largest still's, the output size first", () => {
    expect([
      stillSizeOptions({ widthPx: 1920, heightPx: 1080 }).map(({ scale }) => scale),
      stillSizeOptions({ widthPx: 2560, heightPx: 1440 }).map(({ scale }) => scale),
      stillSizeOptions({ widthPx: 3840, heightPx: 2160 }).map(({ scale }) => scale),
    ]).toEqual([STILL_SCALES, [1, 2, 3], [1, 2]]);
  });

  it("never offers a still below the view's output size, even one past the largest still's side", () => {
    const wide = { widthPx: STILL_MAX_SIDE_PX + 64, heightPx: 1080 };
    expect(stillSizeOptions(wide)).toEqual([{ scale: 1, size: wide }]);
  });

  it("holds a still within a limit on its longer side, its aspect kept to the pixel (main's ruling, 2026-11-19)", () => {
    expect([
      stillSizeWithin({ widthPx: 7680, heightPx: 4320 }, 2048),
      stillSizeWithin({ widthPx: 4098, heightPx: 2304 }, 4096),
      stillSizeWithin({ widthPx: 1080, heightPx: 5760 }, 4096),
      stillSizeWithin({ widthPx: 1920, heightPx: 1080 }, 4096),
      stillSizeWithin({ widthPx: 10_000, heightPx: 1 }, 4096),
    ]).toEqual([
      { widthPx: 2048, heightPx: 1152 },
      // 2,304 × 4,096 ÷ 4,098 is 2,302.9.
      { widthPx: 4096, heightPx: 2303 },
      { widthPx: 768, heightPx: 4096 },
      { widthPx: 1920, heightPx: 1080 },
      // Never under a pixel.
      { widthPx: 4096, heightPx: 1 },
    ]);
  });

  it("offers each size within the limit once, a multiple that would draw the one before it left out", () => {
    expect([
      stillSizeOptions({ widthPx: 1920, heightPx: 1080 }, 2048).map(({ scale, size }) => [
        scale,
        size.widthPx,
      ]),
      stillSizeOptions({ widthPx: 3840, heightPx: 2160 }, 2048).map(({ scale, size }) => [
        scale,
        size.widthPx,
      ]),
    ]).toEqual([
      [
        [1, 1920],
        [2, 2048],
      ],
      [[1, 2048]],
    ]);
  });

  it("refuses a limit that is not a positive integer", () => {
    expect(() => stillSizeWithin({ widthPx: 1920, heightPx: 1080 }, 0)).toThrow(RangeError);
  });
});
