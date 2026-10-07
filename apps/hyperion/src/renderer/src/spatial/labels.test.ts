import { describe, expect, it } from "vitest";

import type { Viewport } from "./camera";
import type { Anchor } from "./drawList";
import { reticleStrokesCssPx } from "../lib/strokes";
import { chooseLabels, markLabelTransform, placeLabels } from "./labels";
import type { PointMark } from "./marks";
import { vec3 } from "../geometry/vec3";

const VIEWPORT: Viewport = { widthPx: 400, heightPx: 300, remPx: 16 };

/** The ratios in use: the development machine's and the UHD 620's, 100%, a Retina display, and 3. */
const RATIOS = [0.78125, 1, 2, 3] as const;

/** The marks' strokes at a ratio of 2, where no outline moves out. */
const AT_TWO = reticleStrokesCssPx(2);

/**
 * How far a label's box stands beyond its symbol's 6 px radius at a ratio of 2 and 100%: 0.375 rem
 * and 0.75 px, its text 0.125 rem outside the bracket's outer edge (R07.T16.h).
 */
const OFFSET_AT_TWO_PX = 0.375 * 16 + 0.75;

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
    const [label] = placeLabels(
      [mark("a", 1, "SOL-1")],
      [anchor("a", 100, 100)],
      VIEWPORT,
      AT_TWO,
      null,
    );

    expect(label).toMatchObject({
      id: "a",
      text: "SOL-1",
      side: "right",
      leftPx: 100 + 6 + OFFSET_AT_TWO_PX,
    });
    expect((label?.topPx ?? 0) + (label?.heightPx ?? 0) / 2).toBeCloseTo(100, 9);
    expect(label?.widthPx).toBeCloseTo(5 * 0.62 * 14, 9);
  });

  it("flips a label to the left at the right edge", () => {
    const [label] = placeLabels(
      [mark("a", 1, "HD 140283")],
      [anchor("a", 380, 100)],
      VIEWPORT,
      AT_TWO,
      null,
    );

    expect(label?.side).toBe("left");
    expect((label?.leftPx ?? 0) + (label?.widthPx ?? 0)).toBeCloseTo(380 - 6 - OFFSET_AT_TWO_PX, 9);
  });

  it("keeps a label whole inside a view too narrow for it on either side of its mark", () => {
    const narrow: Viewport = { widthPx: 160, heightPx: 100, remPx: 16 };
    const [label] = placeLabels(
      [mark("a", 1, "9FG 567Z04 B-3")],
      [anchor("a", 80, 4)],
      narrow,
      AT_TWO,
      null,
    );

    expect(label?.leftPx).toBeGreaterThanOrEqual(0);
    expect((label?.leftPx ?? 0) + (label?.widthPx ?? 0)).toBeLessThanOrEqual(160);
    expect(label?.topPx).toBe(0);
  });

  it("drops a lesser label that overlaps one already placed", () => {
    const chosen = [mark("big", 9), mark("small", 1), mark("apart", 0)];
    const anchors = [anchor("big", 100, 100), anchor("small", 104, 104), anchor("apart", 100, 200)];

    expect(placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null).map((label) => label.id)).toEqual([
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

    const placed = placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null, ["picked"], [furniture]);

    expect(placed.map((label) => label.id)).toEqual(["picked", "clear"]);
  });

  it("never drops the selected label", () => {
    const chosen = chooseLabels([mark("heavy", 9), mark("picked", 1)], "picked", null);
    const anchors = [anchor("heavy", 100, 100), anchor("picked", 102, 101)];

    const placed = placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null, ["picked"]);

    expect(placed.map((label) => label.id)).toEqual(["picked"]);
  });

  it("keeps the selected label even when its mark is out of view", () => {
    const placed = placeLabels(
      [mark("picked", 1)],
      [anchor("picked", -50, 100)],
      VIEWPORT,
      AT_TWO,
      null,
      ["picked"],
    );

    expect(placed).toHaveLength(1);
  });

  it("drops the label of a mark out of view", () => {
    expect(
      placeLabels([mark("a", 1)], [anchor("a", 100, 900)], VIEWPORT, AT_TWO, null),
    ).toHaveLength(0);
  });

  it.each([
    ["right", 100],
    ["left", 380],
  ] as const)(
    "starts a label's box on the %s 0.375 rem and 3.40, 2.00, 0.75 and 0.75 px beyond its symbol",
    (side, xPx) => {
      const gaps = RATIOS.map((ratio) => {
        const [label] = placeLabels(
          [mark("a", 1, "HD 140283")],
          [anchor("a", xPx, 100)],
          VIEWPORT,
          reticleStrokesCssPx(ratio),
          null,
        );
        const nearPx =
          side === "right"
            ? (label?.leftPx ?? 0) - xPx
            : xPx - (label?.leftPx ?? 0) - (label?.widthPx ?? 0);
        return [label?.side, nearPx - 6 - 0.375 * VIEWPORT.remPx];
      });

      expect(gaps.map(([at, gap]) => [at, Math.round(Number(gap) * 100) / 100])).toEqual(
        [3.4, 2, 0.75, 0.75].map((gap) => [side, gap]),
      );
    },
  );

  it.each(RATIOS)(
    "starts the destination's label's box 0.125 rem beyond its chevrons' reach and half the mark stroke, at %s",
    (ratio) => {
      const strokes = reticleStrokesCssPx(ratio);
      const [label] = placeLabels([mark("a", 1)], [anchor("a", 100, 100)], VIEWPORT, strokes, "a");
      // The bracket about the 6 px symbol, moved out by four shifts; the apices the least gap or
      // 0.25 rem outside it; each arm the bracket's, a third of its side, run out at 45°.
      const bracketPx = 6 + 4 + 4 * strokes.shiftPx;
      const reachPx =
        bracketPx + Math.max(4, strokes.minGapPx) + ((2 * bracketPx) / 3) * Math.SQRT1_2;

      expect((label?.leftPx ?? 0) - 100).toBeCloseTo(
        reachPx + strokes.markStrokePx / 2 + 0.125 * VIEWPORT.remPx,
        9,
      );
    },
  );

  it("moves the destination's label out beyond its chevrons, and no other label", () => {
    const strokes = reticleStrokesCssPx(1);
    const chosen = [mark("a", 1), mark("b", 0)];
    const anchors = [anchor("a", 100, 100), anchor("b", 100, 200)];
    const left = (destinationId: string | null): Array<number | undefined> =>
      placeLabels(chosen, anchors, VIEWPORT, strokes, destinationId).map((label) => label.leftPx);

    const [alone = 0, other] = left(null);
    const [destination = 0, unmoved] = left("a");

    expect([destination > alone, unmoved]).toEqual([true, other]);
  });

  it("places a label that stands to the right by its left edge", () => {
    const [label] = placeLabels(
      [mark("a", 1, "SOL-1")],
      [anchor("a", 100, 100)],
      VIEWPORT,
      AT_TWO,
      null,
    );

    expect(label === undefined ? null : markLabelTransform(label, 16)).toBe(
      `translate(${String((label?.leftPx ?? 0) / 16)}rem, ${String((label?.topPx ?? 0) / 16)}rem)`,
    );
  });

  it("places a label flipped to the left by its right edge, moved back by its own width", () => {
    const [label] = placeLabels(
      [mark("a", 1, "HD 140283")],
      [anchor("a", 380, 100)],
      VIEWPORT,
      AT_TWO,
      null,
    );
    // Its near edge, towards the mark: the box's right edge, wherever its text really ends.
    const nearRem = ((label?.leftPx ?? 0) + (label?.widthPx ?? 0)) / 16;

    expect([label?.side, label === undefined ? null : markLabelTransform(label, 16)]).toEqual([
      "left",
      `translate(calc(${String(nearRem)}rem - 100%), ${String((label?.topPx ?? 0) / 16)}rem)`,
    ]);
  });

  it("scales the estimated box with the interface", () => {
    const [small] = placeLabels(
      [mark("a", 1, "ABCD")],
      [anchor("a", 10, 10)],
      VIEWPORT,
      AT_TWO,
      null,
    );
    const [large] = placeLabels(
      [mark("a", 1, "ABCD")],
      [anchor("a", 10, 10)],
      {
        ...VIEWPORT,
        remPx: 24,
      },
      AT_TWO,
      null,
    );

    expect(large?.widthPx).toBeCloseTo((small?.widthPx ?? 0) * 1.5, 9);
  });
});
