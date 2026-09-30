import { describe, expect, it } from "vitest";

import { chartLayout, COMPACT_BELOW_REM, controlsGiveWay } from "./chartLayout";

/** A page of `heightRem` at a root font size of `remPx`, 50 rem wide. */
function page(heightRem: number, remPx = 16) {
  return { widthPx: 50 * remPx, heightPx: heightRem * remPx, devicePixelRatio: 1, remPx };
}

describe("chartLayout", () => {
  it("stacks the page until it is measured", () => {
    expect(chartLayout(null)).toBe("stacked");
  });

  it("lays out the 30 rem page of 1280 × 720 compact", () => {
    expect(chartLayout(page(30))).toBe("compact");
  });

  it("stacks the 52.5 rem page of 1920 × 1080, as it always was", () => {
    expect(chartLayout(page(52.5))).toBe("stacked");
  });

  it("changes layout at the threshold, in rem whatever the interface scale", () => {
    expect(chartLayout(page(COMPACT_BELOW_REM))).toBe("stacked");
    expect(chartLayout(page(COMPACT_BELOW_REM - 0.1))).toBe("compact");
    // 1920 × 1080 at 125%: the same pixels hold fewer rem.
    expect(chartLayout(page(840 / 20, 20))).toBe("compact");
  });
});

describe("controlsGiveWay", () => {
  it("hides the query controls on a compact page while the census table is shown", () => {
    expect(controlsGiveWay("compact", true, true)).toBe(true);
    expect(controlsGiveWay("compact", false, true)).toBe(false);
    expect(controlsGiveWay("stacked", true, true)).toBe(false);
  });

  it("brings them back when the answer goes, taking the table and its toggle with it", () => {
    expect(controlsGiveWay("compact", true, false)).toBe(false);
  });
});
