import { describe, expect, it } from "vitest";

import { itemsInView } from "./itemsInView";

/** Readings two to a 22 px line: the items of `lines` lines, each pair sharing its line's top. */
function pairedLines(lines: number): Array<{ topPx: number; heightPx: number }> {
  return Array.from({ length: lines * 2 }, (_, index) => ({
    topPx: Math.floor(index / 2) * 22,
    heightPx: 18,
  }));
}

describe("itemsInView", () => {
  it("names every item when all are in view", () => {
    expect(itemsInView(pairedLines(4), 0, 200)).toEqual({
      firstVisible: 0,
      lastVisible: 7,
      total: 8,
    });
  });

  it("counts an item in view when any of it is, and not one that only touches the edge", () => {
    // 100 px from the top: lines 0-4 start at 0, 22, 44, 66 and 88; the fifth ends at 106.
    expect(itemsInView(pairedLines(10), 0, 100)).toEqual({
      firstVisible: 0,
      lastVisible: 9,
      total: 20,
    });
    // Scrolled so that line 1 ends exactly at the top, 40 px, it is out of view.
    expect(itemsInView(pairedLines(10), 40, 100)?.firstVisible).toBe(4);
  });

  it("names the first item below the top when the viewport falls between items", () => {
    const items = [
      { topPx: 0, heightPx: 10 },
      { topPx: 50, heightPx: 10 },
    ];

    expect(itemsInView(items, 20, 10)).toEqual({ firstVisible: 1, lastVisible: 1, total: 2 });
  });

  it("names the first item in a viewport of no height, as before the region is laid out", () => {
    expect(itemsInView(pairedLines(3), 0, 0)).toEqual({
      firstVisible: 0,
      lastVisible: 0,
      total: 6,
    });
  });

  it("reports nothing for a region with no items", () => {
    expect(itemsInView([], 0, 100)).toBeNull();
  });
});
