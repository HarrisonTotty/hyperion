import { describe, expect, it } from "vitest";

import type { Viewport } from "./camera";
import type { Anchor } from "./drawList";
import { chooseLabels, placeLabels } from "./labels";
import type { PointMark } from "./marks";
import { vec3 } from "./vec3";

const VIEWPORT: Viewport = { widthPx: 400, heightPx: 300, remPx: 16 };

function mark(id: string, labelPriority: number, label = id.toUpperCase()): PointMark {
  return {
    id,
    position: vec3(0, 0, 0),
    shape: "circle",
    sizeClass: 2,
    status: "plain",
    label,
    labelPriority,
  };
}

function anchor(id: string, xPx: number, yPx: number): Anchor {
  return { id, xPx, yPx, depth: 0, radiusPx: 6 };
}

const MARKS = Array.from({ length: 20 }, (_, index) =>
  mark(`m${String(index).padStart(2, "0")}`, index),
);

describe("chooseLabels", () => {
  it("puts the selection first, then the destination, then the highest priorities", () => {
    const chosen = chooseLabels(MARKS, "m03", "m05", 3).map((point) => point.id);

    expect(chosen).toEqual(["m03", "m05", "m19", "m18", "m17"]);
  });

  it("chooses count marks by priority when nothing is selected", () => {
    expect(chooseLabels(MARKS, null, null)).toHaveLength(8);
    expect(chooseLabels(MARKS, null, null, 4).map((point) => point.id)).toEqual([
      "m19",
      "m18",
      "m17",
      "m16",
    ]);
  });

  it("adds the selection to count, and does not repeat a selection among the priorities", () => {
    expect(chooseLabels(MARKS, "m02", null)).toHaveLength(9);
    expect(chooseLabels(MARKS, "m19", null, 2).map((point) => point.id)).toEqual([
      "m19",
      "m18",
      "m17",
    ]);
  });

  it("labels a selection that is also the destination once", () => {
    expect(chooseLabels(MARKS, "m01", "m01", 1).map((point) => point.id)).toEqual(["m01", "m19"]);
  });

  it("breaks priority ties by ID", () => {
    const tied = [mark("c", 5), mark("a", 5), mark("b", 5)];

    expect(chooseLabels(tied, null, null, 2).map((point) => point.id)).toEqual(["a", "b"]);
  });

  it("ignores a selection that is not in the scene", () => {
    expect(chooseLabels(MARKS, "gone", null, 1).map((point) => point.id)).toEqual(["m19"]);
  });
});

describe("placeLabels", () => {
  it("puts a label to the right of its symbol, centred on it", () => {
    const [label] = placeLabels([mark("a", 1, "SOL-1")], [anchor("a", 100, 100)], VIEWPORT);

    expect(label).toMatchObject({ id: "a", text: "SOL-1", side: "right", leftPx: 100 + 6 + 4 });
    expect((label?.topPx ?? 0) + (label?.heightPx ?? 0) / 2).toBeCloseTo(100, 9);
    expect(label?.widthPx).toBeCloseTo(5 * 0.62 * 14, 9);
  });

  it("flips a label to the left at the right edge", () => {
    const [label] = placeLabels([mark("a", 1, "HD 140283")], [anchor("a", 380, 100)], VIEWPORT);

    expect(label?.side).toBe("left");
    expect((label?.leftPx ?? 0) + (label?.widthPx ?? 0)).toBeCloseTo(380 - 6 - 4, 9);
  });

  it("drops a lesser label that overlaps one already placed", () => {
    const chosen = [mark("big", 9), mark("small", 1), mark("apart", 0)];
    const anchors = [anchor("big", 100, 100), anchor("small", 104, 104), anchor("apart", 100, 200)];

    expect(placeLabels(chosen, anchors, VIEWPORT).map((label) => label.id)).toEqual([
      "big",
      "apart",
    ]);
  });

  it("drops a label that would cover other furniture, but never the selection's", () => {
    const chosen = [mark("picked", 1), mark("under", 9), mark("clear", 0)];
    const anchors = [
      anchor("picked", 100, 50),
      anchor("under", 100, 100),
      anchor("clear", 100, 200),
    ];
    const furniture = { leftPx: 0, topPx: 40, widthPx: 400, heightPx: 70 };

    const placed = placeLabels(chosen, anchors, VIEWPORT, ["picked"], [furniture]);

    expect(placed.map((label) => label.id)).toEqual(["picked", "clear"]);
  });

  it("never drops the selected label", () => {
    const chosen = chooseLabels([mark("heavy", 9), mark("picked", 1)], "picked", null);
    const anchors = [anchor("heavy", 100, 100), anchor("picked", 102, 101)];

    const placed = placeLabels(chosen, anchors, VIEWPORT, ["picked"]);

    expect(placed.map((label) => label.id)).toEqual(["picked"]);
  });

  it("keeps the selected label even when its mark is out of view", () => {
    const placed = placeLabels([mark("picked", 1)], [anchor("picked", -50, 100)], VIEWPORT, [
      "picked",
    ]);

    expect(placed).toHaveLength(1);
  });

  it("drops the label of a mark out of view", () => {
    expect(placeLabels([mark("a", 1)], [anchor("a", 100, 900)], VIEWPORT)).toHaveLength(0);
  });

  it("scales the estimated box with the interface", () => {
    const [small] = placeLabels([mark("a", 1, "ABCD")], [anchor("a", 10, 10)], VIEWPORT);
    const [large] = placeLabels([mark("a", 1, "ABCD")], [anchor("a", 10, 10)], {
      ...VIEWPORT,
      remPx: 24,
    });

    expect(large?.widthPx).toBeCloseTo((small?.widthPx ?? 0) * 1.5, 9);
  });
});
