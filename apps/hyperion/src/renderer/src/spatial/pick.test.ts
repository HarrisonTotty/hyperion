import { describe, expect, it } from "vitest";

import type { Anchor } from "./drawList";
import { pick } from "./pick";

function anchor(id: string, xPx: number, yPx: number, depth = 0, radiusPx = 4): Anchor {
  return { id, xPx, yPx, depth, radiusPx };
}

const TOLERANCE_PX = 16;

describe("pick", () => {
  it("picks nothing when no mark is within tolerance", () => {
    expect(pick([anchor("a", 100, 100)], { xPx: 130, yPx: 100 }, TOLERANCE_PX)).toBeNull();
    expect(pick([], { xPx: 0, yPx: 0 }, TOLERANCE_PX)).toBeNull();
  });

  it("picks a mark within tolerance", () => {
    expect(pick([anchor("a", 100, 100)], { xPx: 112, yPx: 100 }, TOLERANCE_PX)).toBe("a");
  });

  it("picks the nearer of two marks on screen", () => {
    const anchors = [anchor("a", 100, 100), anchor("b", 110, 100)];

    expect(pick(anchors, { xPx: 107, yPx: 100 }, TOLERANCE_PX)).toBe("b");
    expect(pick(anchors, { xPx: 103, yPx: 100 }, TOLERANCE_PX)).toBe("a");
  });

  it("picks the mark nearer the viewer of two that coincide", () => {
    const anchors = [anchor("far", 100, 100, 30), anchor("near", 100, 100, -12)];

    expect(pick(anchors, { xPx: 101, yPx: 101 }, TOLERANCE_PX)).toBe("near");
  });

  it("treats marks under half a pixel apart as coincident", () => {
    const anchors = [anchor("far", 100, 100, 30), anchor("near", 100.3, 100, -12)];

    expect(pick(anchors, { xPx: 99, yPx: 100 }, TOLERANCE_PX)).toBe("near");
  });

  it("settles a tie in distance and depth by the lower ID", () => {
    const anchors = [anchor("b", 100, 100, 5), anchor("a", 100, 100, 5)];

    expect(pick(anchors, { xPx: 100, yPx: 100 }, TOLERANCE_PX)).toBe("a");
  });

  it("picks a large symbol at its rim beyond the tolerance", () => {
    const anchors = [anchor("big", 100, 100, 0, 24)];

    expect(pick(anchors, { xPx: 122, yPx: 100 }, TOLERANCE_PX)).toBe("big");
    expect(pick(anchors, { xPx: 126, yPx: 100 }, TOLERANCE_PX)).toBeNull();
  });

  it("does not depend on the order of the anchors", () => {
    const anchors = [
      anchor("a", 100, 100, 3),
      anchor("b", 100.2, 100, -1),
      anchor("c", 100.4, 100, -1),
      anchor("d", 108, 100, -50),
    ];
    const point = { xPx: 100, yPx: 100 };
    const expected = pick(anchors, point, TOLERANCE_PX);

    const orders = [
      [0, 1, 2, 3],
      [3, 2, 1, 0],
      [2, 0, 3, 1],
      [1, 3, 0, 2],
    ];
    for (const order of orders) {
      const shuffled = order.flatMap((index) => anchors[index] ?? []);
      expect(pick(shuffled, point, TOLERANCE_PX)).toBe(expected);
    }
    expect(expected).toBe("b");
  });
});
