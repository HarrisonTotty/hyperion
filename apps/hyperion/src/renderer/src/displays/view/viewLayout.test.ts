import { describe, expect, it } from "vitest";

import type { ElementSize } from "../../lib/useElementSize";
import {
  DEFAULT_FOLD,
  FULL_MIN_HEIGHT_REM,
  FULL_MIN_WIDTH_REM,
  toggledFold,
  viewLayout,
} from "./viewLayout";

/** A `.view` box of `widthPx` by `heightPx` CSS px at `remPx` to the rem. */
function box(widthPx: number, heightPx: number, remPx = 16): ElementSize {
  return { widthPx, heightPx, devicePixelRatio: 1, remPx };
}

describe("VIEW's layout (R07.T19.b)", () => {
  it("is full before the box is measured", () => {
    expect(viewLayout(null)).toBe("full");
  });

  it("is full for the box of a 1920 × 1080 window at 100%", () => {
    expect(viewLayout(box(1888, 923))).toBe("full");
  });

  it("is compact for 1280 × 720, and for 1920 × 1080 at 125% and 150%", () => {
    // The interface scale enlarges the rem: the same window holds fewer of them.
    expect([
      viewLayout(box(1248, 563)),
      viewLayout(box(1888, 923, 20)),
      viewLayout(box(1888, 923, 24)),
    ]).toEqual(["compact", "compact", "compact"]);
  });

  it("is full from its least width and height, and compact just below either", () => {
    const widthPx = FULL_MIN_WIDTH_REM * 16;
    const heightPx = FULL_MIN_HEIGHT_REM * 16;
    expect([
      viewLayout(box(widthPx, heightPx)),
      viewLayout(box(widthPx - 1, heightPx)),
      viewLayout(box(widthPx, heightPx - 1)),
    ]).toEqual(["full", "compact", "compact"]);
  });
});

describe("VIEW's folding panels (R07.T19.b)", () => {
  it("opens CAMERA by default", () => {
    expect(DEFAULT_FOLD).toBe("camera");
  });

  it("folds the open panel when another opens, and opens CAMERA when the open one folds", () => {
    expect([
      toggledFold("camera", "style"),
      toggledFold("style", "exposure"),
      toggledFold("exposure", "exposure"),
      toggledFold("camera", "camera"),
      toggledFold("meter", "instruments"),
    ]).toEqual(["style", "exposure", "camera", "camera", "instruments"]);
  });
});
