import { describe, expect, it } from "vitest";

import type { ElementSize } from "../../lib/useElementSize";
import {
  DEFAULT_FOLD,
  FULL_MIN_HEIGHT_BOUND_REM,
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

  it("is full for the box of a maximised 1920 × 1080 window, 53.5 rem under its bars", () => {
    expect(viewLayout(box(1888, 856))).toBe("full");
  });

  it("is compact for the app's opening window, 1600 × 900", () => {
    expect(viewLayout(box(1568, 742))).toBe("compact");
  });

  it("is compact for 1280 × 720, and for 1920 × 1080 at 125% and 150%", () => {
    // The interface scale enlarges the rem: the same window holds fewer of them.
    expect([
      viewLayout(box(1248, 563)),
      viewLayout(box(1888, 923, 20)),
      viewLayout(box(1888, 923, 24)),
    ]).toEqual(["compact", "compact", "compact"]);
  });

  it("starts at a 99 rem box, so that the 98.5 rem box it began at is compact (R06.T13.j)", () => {
    const heightPx = FULL_MIN_HEIGHT_REM * 16;
    expect([FULL_MIN_WIDTH_REM, viewLayout(box(98.5 * 16, heightPx))]).toEqual([99, "compact"]);
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

describe("VIEW's full layout's least height (R07.T19.b's follow-up)", () => {
  // A task that would pass the bound asks for a ruling rather than raise it.
  it("stays within 52.5 rem, so that a maximised 1920 × 1080 window is full (decision-r07-t19b-exposure-fit, item 2)", () => {
    expect(FULL_MIN_HEIGHT_REM).toBeLessThanOrEqual(FULL_MIN_HEIGHT_BOUND_REM);
  });

  it("is bounded at the 53.5 rem box of a maximised 1920 × 1080 window less 1 rem", () => {
    expect(FULL_MIN_HEIGHT_BOUND_REM).toBe(52.5);
  });

  it("takes the STYLE panel's BASELINE row: 752 px, its 0.5 rem margin and its 2 rem row, 49.5 rem (R10.T12.a; decision-r10-t12a-scope §3.3)", () => {
    expect([FULL_MIN_HEIGHT_REM, (752 + 8 + 32) / 16]).toEqual([49.5, 49.5]);
  });

  it("keeps every named box's layout at 49.5 rem: 1920 × 1080 at 100% and maximised full, 1600 × 900 and 1280 × 720 compact", () => {
    expect([
      viewLayout(box(1888, 923)),
      viewLayout(box(1888, 856)),
      viewLayout(box(1568, 742)),
      viewLayout(box(1248, 563)),
    ]).toEqual(["full", "full", "compact", "compact"]);
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
