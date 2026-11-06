import { describe, expect, it } from "vitest";

import {
  DEFAULT_VSYNC_MS,
  drawsNow,
  VSYNC_SEEN_FRAMES,
  VSYNC_WINDOW_FRAMES,
  VsyncClock,
  vsyncPeriodOf,
  vsyncsPerDraw,
} from "./vsyncPacing";

/** A display's vsync period at `hz`, ms. */
function periodAt(hz: number): number {
  return 1000 / hz;
}

describe("vsyncPeriodOf (R07.T19)", () => {
  it("measures nothing until 30 intervals are seen", () => {
    expect([
      vsyncPeriodOf(Array.from({ length: VSYNC_SEEN_FRAMES - 1 }, () => 8.3)),
      vsyncPeriodOf(Array.from({ length: VSYNC_SEEN_FRAMES }, () => 8.3)),
    ]).toEqual([null, 8.3]);
  });

  it("takes the 10th percentile, so that skipped callbacks leave it at the panel's period", () => {
    // 108 callbacks two vsyncs apart and 12 on time: a tenth of 120 on time still reads the panel.
    const intervals = [
      ...Array.from({ length: 108 }, () => 16.7),
      ...Array.from({ length: 12 }, () => 8.3),
    ];
    const fewer = [
      ...Array.from({ length: 109 }, () => 16.7),
      ...Array.from({ length: 11 }, () => 8.3),
    ];
    expect([vsyncPeriodOf(intervals), vsyncPeriodOf(fewer)]).toEqual([8.3, 16.7]);
  });

  it("reads the last 120 intervals, leaving zeros and gaps out", () => {
    const old = Array.from({ length: 200 }, () => 8.3);
    const recent = Array.from({ length: VSYNC_WINDOW_FRAMES }, () => 16.7);
    expect([
      vsyncPeriodOf([...old, ...recent]),
      vsyncPeriodOf([0, Number.NaN, ...recent.slice(0, VSYNC_SEEN_FRAMES)]),
    ]).toEqual([16.7, 16.7]);
  });
});

describe("VsyncClock (R07.T19)", () => {
  it("gives a 60 Hz display's period until 30 intervals are seen, then the measured one", () => {
    const clock = new VsyncClock();
    const seen: number[] = [];
    for (let frame = 0; frame <= VSYNC_SEEN_FRAMES; frame += 1) {
      clock.note(frame * periodAt(120));
      seen.push(clock.periodMs);
    }
    expect(seen.at(-2)).toBe(DEFAULT_VSYNC_MS);
    expect(seen.at(-1)).toBeCloseTo(periodAt(120), 9);
  });

  it("ignores a timestamp that does not move on", () => {
    const clock = new VsyncClock();
    for (let frame = 0; frame <= VSYNC_SEEN_FRAMES; frame += 1) {
      clock.note(frame * 10);
      clock.note(frame * 10);
    }
    expect(clock.periodMs).toBe(10);
  });
});

describe("vsyncsPerDraw (R07.T19)", () => {
  it.each([
    [60, 1, 2],
    [59.94, 1, 2],
    [100, 1, 3],
    [120, 2, 4],
    [144, 2, 5],
  ])("draws 60 and 30 Hz views on a %s Hz display every %s and %s vsyncs", (hz, high, low) => {
    expect([vsyncsPerDraw(60, periodAt(hz)), vsyncsPerDraw(30, periodAt(hz))]).toEqual([high, low]);
  });

  it("draws in every vsync of a display slower than the view's rate", () => {
    expect(vsyncsPerDraw(60, periodAt(50))).toBe(1);
  });

  it.each([
    [120, 2, 4],
    [144, 2, 4],
    [165, 2, 4],
    [60, 1, 2],
  ])(
    "keeps a 30 Hz view asked in a 60 Hz view's draws on its grid on a %s Hz display: every %s vsyncs' multiple %s",
    (hz, within, low) => {
      expect([vsyncsPerDraw(60, periodAt(hz)), vsyncsPerDraw(30, periodAt(hz), within)]).toEqual([
        within,
        low,
      ]);
    },
  );
});

describe("drawsNow (R07.T19)", () => {
  it("draws once half a vsync short of k vsyncs has passed since the last draw", () => {
    const p = periodAt(120);
    expect([
      drawsNow(Number.POSITIVE_INFINITY, 30, p),
      drawsNow(3 * p, 30, p),
      drawsNow(3.4 * p, 30, p),
      drawsNow(3.5 * p, 30, p),
      drawsNow(4 * p, 30, p),
      drawsNow(5 * p, 30, p),
    ]).toEqual([true, false, false, true, true, true]);
  });

  it("draws a view asked every second vsync once half of that short of its k has passed", () => {
    // 144 Hz, asked every 2 vsyncs: k 4, so once 3 vsyncs have passed.
    const p = periodAt(144);
    expect([
      drawsNow(2 * p, 30, p, 2),
      drawsNow(2.9 * p, 30, p, 2),
      drawsNow(4 * p, 30, p, 2),
    ]).toEqual([false, false, true]);
  });
});
