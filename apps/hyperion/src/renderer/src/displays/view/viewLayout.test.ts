import { describe, expect, it } from "vitest";

import type { ElementSize } from "../../lib/useElementSize";
import {
  DEFAULT_FOLD,
  FULL_MIN_HEIGHT_BOUND_REM,
  FULL_MIN_HEIGHT_REM,
  FULL_MIN_WIDTH_REM,
  FULL_WHOLE_MIN_HEIGHT_REM,
  inSecondColumnFold,
  SECOND_COLUMN_FOLD,
  secondColumnFolds,
  toggledFold,
  toggledSecondFold,
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

/** Column B's tallest, px, with its Style panel `stylePx` tall: the exposure, the meter and two gaps. */
function columnPx(stylePx: number): number {
  return stylePx + 425.5 + 168 + 2 * 8;
}

describe("VIEW's full layout's least height (R07.T19.b's follow-up)", () => {
  // A task that would pass the bound asks for a ruling rather than raise it.
  it("stays within 52.5 rem, so that a maximised 1920 × 1080 window is full (decision-r07-t19b-exposure-fit, item 2)", () => {
    expect(FULL_MIN_HEIGHT_REM).toBeLessThanOrEqual(FULL_MIN_HEIGHT_BOUND_REM);
  });

  it("is bounded at the 53.5 rem box of a maximised 1920 × 1080 window less 1 rem", () => {
    expect(FULL_MIN_HEIGHT_BOUND_REM).toBe(52.5);
  });

  it("takes the STYLE panel's BASELINE row: 752 px, its 0.5 rem margin and its 2 rem row, 49.5 rem in the second column (R10.T12.a; decision-r10-t12a-scope §3.3)", () => {
    expect((752 + 8 + 32) / 16).toBe(49.5);
  });

  it("keeps the second column at 49.5 rem with the BASELINE row beside an instrument's QUALITY LOW reason, the GRID row never beside it (R10.T12.d)", () => {
    // The Style panel at 99.5 px: with the two-line reason (43 px) and BASELINE (40 px), or GRID and
    // BASELINE (40 px each).
    expect([columnPx(99.5 + 43 + 40) / 16, columnPx(99.5 + 40 + 40) / 16 <= 49.5]).toEqual([
      49.5,
      true,
    ]);
  });

  it("takes the first column with STILLS sharing CAMERA's place, 813.5 px measured, to 51 rem (plan R11, R11.T10.b)", () => {
    // Hidden and offscreen, 2026-11-18: Instruments 202.5, Targets at two rows 173, the CAMERA
    // STILLS row 32, the stills' two-line line 35 and Camera at its worst 339 px, four gaps.
    expect([
      202.5 + 173 + 32 + 35 + 339 + 4 * 8,
      FULL_MIN_HEIGHT_REM,
      Math.ceil(813.5 / 16 / 0.25) * 0.25,
    ]).toEqual([813.5, 51, 51]);
  });

  it("holds the second column folded within it, 763 px measured (R11.T10.b's follow-up)", () => {
    // Style 182.5, the EXPOSURE row 32 and the exposure at its worst 532.5 px, two gaps.
    const foldedPx = 182.5 + 32 + 532.5 + 2 * 8;
    expect(foldedPx).toBe(763);
    expect(foldedPx).toBeLessThanOrEqual(FULL_MIN_HEIGHT_REM * 16);
  });

  it("keeps every named box's layout at 51 rem: 1920 × 1080 at 100% and maximised full, 1600 × 900 and 1280 × 720 compact", () => {
    expect([
      viewLayout(box(1888, 923)),
      viewLayout(box(1888, 856)),
      viewLayout(box(1568, 742)),
      viewLayout(box(1248, 563)),
    ]).toEqual(["full", "full", "compact", "compact"]);
  });
});

describe("VIEW's second column's whole height (R11.T10.b's follow-up; the UX ruling, 2026-11-18)", () => {
  it("is the column's tallest state whole, 899 px measured, rounded up to 56.25 rem", () => {
    // Style 182.5, the exposure at its worst with its white balance 532.5 and the meter 168 px, two
    // gaps; measured 899.05 px.
    expect([
      182.5 + 532.5 + 168 + 2 * 8,
      FULL_WHOLE_MIN_HEIGHT_REM,
      Math.ceil(899.05 / 16 / 0.25) * 0.25,
    ]).toEqual([899, 56.25, 56.25]);
  });

  it("stays under 57.25 rem, so that a 1920 × 1080 window's 919 px box stands whole", () => {
    expect(FULL_WHOLE_MIN_HEIGHT_REM).toBeLessThanOrEqual(57.25);
    expect(secondColumnFolds(box(1888, 919))).toBe(false);
  });

  it("folds in a maximised 1920 × 1080 window's 853 px box and down to the full layout's least height", () => {
    expect([
      secondColumnFolds(box(1888, 853)),
      secondColumnFolds(box(1888, FULL_MIN_HEIGHT_REM * 16)),
      secondColumnFolds(box(1888, FULL_WHOLE_MIN_HEIGHT_REM * 16 - 1)),
      secondColumnFolds(box(1888, FULL_WHOLE_MIN_HEIGHT_REM * 16)),
    ]).toEqual([true, true, true, false]);
  });

  it("never folds before the box is measured, nor in the compact layout, whose own fold holds", () => {
    expect([
      secondColumnFolds(null),
      secondColumnFolds(box(1568, 739)),
      secondColumnFolds(box(1248, 559)),
    ]).toEqual([false, false, false]);
  });

  it("follows the interface scale, reckoned in rem", () => {
    expect([
      secondColumnFolds(box(2360, 1149, 20)),
      secondColumnFolds(box(2360, 1100, 20)),
    ]).toEqual([false, true]);
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

  it("opens the second column's EXPOSURE again when its open panel folds", () => {
    expect([
      SECOND_COLUMN_FOLD,
      toggledSecondFold("exposure", "meter"),
      toggledSecondFold("meter", "meter"),
      toggledSecondFold("exposure", "exposure"),
    ]).toEqual(["exposure", "meter", "exposure", "exposure"]);
  });

  it("folds EXPOSURE and EXPOSURE METER alone in the second column", () => {
    expect(
      (["camera", "stills", "style", "exposure", "balance", "meter", null] as const).map((panel) =>
        inSecondColumnFold(panel),
      ),
    ).toEqual([false, false, false, true, false, true, false]);
  });
});
