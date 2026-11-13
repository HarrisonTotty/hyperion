/**
 * The `INPUTS` column's layout by its height (plan R15, T6): compact at the floor, full at
 * 1920 × 1080, by `rem` so that it follows the interface scale.
 */
import { describe, expect, it } from "vitest";

import type { ElementSize } from "../../lib/useElementSize";
import {
  DEFAULT_INPUTS_FOLD,
  FULL_MIN_HEIGHT_REM,
  inputsLayout,
  toggledInputsFold,
} from "./inputsLayout";

/** A column `heightPx` tall at `remPx` to the `rem`, as the client measures it. */
function column(heightPx: number, remPx = 16): ElementSize {
  return { widthPx: 26 * remPx, heightPx, devicePixelRatio: 1, remPx };
}

describe("inputsLayout", () => {
  it("lays out a 1280 × 720 window's column compact and a 1920 × 1080 one's full", () => {
    expect([inputsLayout(column(599)), inputsLayout(column(959))]).toEqual(["compact", "full"]);
  });

  it("lays out 1920 × 1080 at 150% compact, its column as tall in rem as the floor's", () => {
    expect(inputsLayout(column(599 * 1.5, 24))).toBe("compact");
  });

  it("switches at the full layout's tallest state", () => {
    const remPx = 16;
    expect([
      inputsLayout(column(FULL_MIN_HEIGHT_REM * remPx - 1, remPx)),
      inputsLayout(column(FULL_MIN_HEIGHT_REM * remPx, remPx)),
    ]).toEqual(["compact", "full"]);
  });

  it("lays out full before the column is measured", () => {
    expect(inputsLayout(null)).toBe("full");
  });
});

describe("toggledInputsFold", () => {
  it("opens the part pressed, folding the one open, and opens STAR again as the open one folds", () => {
    expect([
      toggledInputsFold("star", "volatiles"),
      toggledInputsFold("volatiles", "volatiles"),
      toggledInputsFold("star", "star"),
    ]).toEqual(["volatiles", DEFAULT_INPUTS_FOLD, "star"]);
  });
});
