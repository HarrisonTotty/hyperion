import { describe, expect, it } from "vitest";

import { windowRange } from "./windowRange";

const ROW_PX = 32;
const VIEWPORT_PX = 20 * ROW_PX;

describe("windowRange", () => {
  it.each([
    ["the top", 0, 3_000, { first: 0, last: 27, firstVisible: 0, lastVisible: 19 }],
    ["a partly scrolled top", 16, 3_000, { first: 0, last: 28, firstVisible: 0, lastVisible: 20 }],
    [
      "the middle",
      1_000 * ROW_PX,
      3_000,
      { first: 992, last: 1_027, firstVisible: 1_000, lastVisible: 1_019 },
    ],
    [
      "the end",
      2_980 * ROW_PX,
      3_000,
      { first: 2_972, last: 2_999, firstVisible: 2_980, lastVisible: 2_999 },
    ],
    ["fewer rows than the window", 0, 5, { first: 0, last: 4, firstVisible: 0, lastVisible: 4 }],
    [
      "a scroll offset past the end",
      1e9,
      3_000,
      { first: 2_972, last: 2_999, firstVisible: 2_980, lastVisible: 2_999 },
    ],
    [
      "a scroll offset past a short list",
      500,
      5,
      { first: 0, last: 4, firstVisible: 0, lastVisible: 4 },
    ],
    [
      "a negative scroll offset",
      -40,
      3_000,
      { first: 0, last: 27, firstVisible: 0, lastVisible: 19 },
    ],
  ])("covers %s", (_, scrollTopPx, total, expected) => {
    expect(windowRange(scrollTopPx, ROW_PX, VIEWPORT_PX, total, 8)).toEqual(expected);
  });

  it("is null for a list with no rows", () => {
    expect(windowRange(0, ROW_PX, VIEWPORT_PX, 0, 8)).toBeNull();
  });

  it("keeps row edges exact at a fractional row height", () => {
    const rowPx = 28.8;
    const range = windowRange(2_980 * rowPx, rowPx, 20 * rowPx, 3_000, 0);

    expect(range).toEqual({ first: 2_980, last: 2_999, firstVisible: 2_980, lastVisible: 2_999 });
  });

  it("names the top row when the viewport has no height", () => {
    expect(windowRange(0, ROW_PX, 0, 10, 2)).toEqual({
      first: 0,
      last: 2,
      firstVisible: 0,
      lastVisible: 0,
    });
  });

  it.each([
    [0, 10, 0],
    [ROW_PX, 2.5, 0],
    [ROW_PX, 10, -1],
  ])("refuses a row height of %f with %f rows and %i overscan", (rowPx, total, overscan) => {
    expect(() => windowRange(0, rowPx, VIEWPORT_PX, total, overscan)).toThrow(RangeError);
  });

  it.each([
    [Number.NaN, VIEWPORT_PX],
    [0, Number.NaN],
    [0, Number.POSITIVE_INFINITY],
  ])("refuses a scroll offset of %f or a viewport of %f px", (scrollTopPx, viewportPx) => {
    expect(() => windowRange(scrollTopPx, ROW_PX, viewportPx, 10, 0)).toThrow(RangeError);
  });
});
