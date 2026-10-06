import { describe, expect, it } from "vitest";

import { lineScale, markShiftDevicePx, markStrokeDevicePx, MIN_STROKE_DEVICE_PX } from "./strokes";

/** The ratios in use: the development machine's and the UHD 620's, 100%, a Retina display, and 3. */
const RATIOS = [0.78125, 1, 2, 3] as const;

describe("the console's stroke widths (decision-thin-line-contrast, item 2)", () => {
  it("draws no line or outline under 2 device px", () => {
    expect(MIN_STROKE_DEVICE_PX).toBe(2);
  });

  it("scales a line by the larger of the ratio and 2: 2, 2, 2 and 3", () => {
    expect(RATIOS.map(lineScale)).toEqual([2, 2, 2, 3]);
  });

  it("draws a mark's outline at the larger of 1.5 times the ratio and 2: 2, 2, 3 and 4.5", () => {
    expect(RATIOS.map(markStrokeDevicePx)).toEqual([2, 2, 3, 4.5]);
  });

  it("moves a mark's outline out by half what the floor adds: 0.41, 0.25, 0 and 0 px", () => {
    expect(RATIOS.map(markShiftDevicePx)).toEqual([0.4140625, 0.25, 0, 0]);
  });

  it("moves an outline 0.06 px at a ratio of 1.25 and none from 4/3 up", () => {
    expect([
      markShiftDevicePx(1.25),
      Math.abs(markShiftDevicePx(4 / 3)) < 1e-12,
      markShiftDevicePx(1.5),
    ]).toEqual([0.0625, true, 0]);
  });

  it("takes a ratio that is not a positive number as 1", () => {
    expect(
      [Number.NaN, 0, -2, Number.POSITIVE_INFINITY].map((ratio) => [
        lineScale(ratio),
        markStrokeDevicePx(ratio),
        markShiftDevicePx(ratio),
      ]),
    ).toEqual(Array.from({ length: 4 }, () => [2, 2, 0.25]));
  });
});
