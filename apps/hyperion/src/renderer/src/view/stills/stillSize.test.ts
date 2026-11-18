import { describe, expect, it } from "vitest";

import { STILL_MAX_SIDE_PX } from "./stillLabel";
import { STILL_SCALES, stillSizeAt, stillSizeOptions } from "./stillSize";

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
});
