import { describe, expect, it } from "vitest";

import type { Viewport } from "./camera";
import { type Anchor, destinationGapPx, reticleHalfSizePx } from "./drawList";
import { RETICLE_SHIFTS, type ReticleStrokesCss, reticleStrokesCssPx } from "../lib/strokes";
import {
  type BoxPx,
  chooseLabels,
  markLabelTransform,
  type PlacedLabel,
  placeLabels,
  textSizeRem,
} from "./labels";
import type { PointMark, SizeClass } from "./marks";
import {
  boxGapPx,
  bracketArmPx,
  DESTINATION_LABEL_PLACES,
  type DestinationLabelPlace,
  destinationChevrons,
  type OffsetPx,
  placeIsRight,
  placeIsUpper,
  SIZE_CLASS_REM,
  squareBoxPx,
} from "./symbols";
import { vec3 } from "../geometry/vec3";
import { stylesheetRule } from "../test/stylesheet";

const VIEWPORT: Viewport = { widthPx: 400, heightPx: 300, remPx: 16 };

/** The ratios in use: the development machine's and the UHD 620's, 100%, a Retina display, and 3. */
const RATIOS = [0.78125, 1, 2, 3] as const;

/** The marks' strokes at a ratio of 2, where no outline moves out. */
const AT_TWO = reticleStrokesCssPx(2);

/**
 * How far a label's box stands beyond its symbol's 6 px radius at a ratio of 2 and 100%: 0.5 rem
 * and 0.75 px, its text 0.25 rem outside the bracket's outer edge (R07.T16.h and its follow-up).
 */
const OFFSET_AT_TWO_PX = 0.5 * 16 + 0.75;

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

function anchor(id: string, xPx: number, yPx: number, radiusPx = 6): Anchor {
  return { id, xPx, yPx, depth: 0, radiusPx };
}

/** The selection's bracket's half-size about `at` as `paint` draws it, CSS px. */
function bracketPx(strokes: ReticleStrokesCss, at: Anchor, remPx = VIEWPORT.remPx): number {
  return reticleHalfSizePx(at.radiusPx, remPx) + RETICLE_SHIFTS * strokes.shiftPx;
}

/** A reticle stroke about a mark, CSS px from the view's top left. */
type Stroke = readonly [OffsetPx, OffsetPx];

/** The selection's bracket's eight arms about `at`, as `paint` draws them, CSS px. */
function bracketStrokes(at: Anchor, strokes: ReticleStrokesCss, remPx: number): Stroke[] {
  const halfPx = bracketPx(strokes, at, remPx);
  const armPx = bracketArmPx(halfPx);
  const point = (x: number, y: number): OffsetPx => ({ xPx: at.xPx + x, yPx: at.yPx + y });
  return [-1, 1].flatMap((sx) =>
    [-1, 1].flatMap((sy): Stroke[] => [
      [point(sx * halfPx, sy * halfPx), point(sx * (halfPx - armPx), sy * halfPx)],
      [point(sx * halfPx, sy * halfPx), point(sx * halfPx, sy * (halfPx - armPx))],
    ]),
  );
}

/** The destination's four chevrons' eight arms about `at`, as `paint` draws them, CSS px. */
function chevronStrokes(at: Anchor, strokes: ReticleStrokesCss, remPx: number): Stroke[] {
  const halfPx = bracketPx(strokes, at, remPx);
  const point = (x: number, y: number): OffsetPx => ({ xPx: at.xPx + x, yPx: at.yPx + y });
  return destinationChevrons(
    halfPx + destinationGapPx(remPx, strokes.minGapPx),
    bracketArmPx(halfPx),
  ).flatMap(([end, apex, other]): Stroke[] => [
    [point(end.xPx, end.yPx), point(apex.xPx, apex.yPx)],
    [point(apex.xPx, apex.yPx), point(other.xPx, other.yPx)],
  ]);
}

/** The least distance from a stroke's ink, its segment and `halfWidthPx` either side, to a box. */
function boxInkDistancePx(box: BoxPx, [from, to]: Stroke, halfWidthPx: number): number {
  const at = (t: number): number => {
    const xPx = from.xPx + (to.xPx - from.xPx) * t;
    const yPx = from.yPx + (to.yPx - from.yPx) * t;
    return Math.hypot(
      Math.max(0, box.leftPx - xPx, xPx - box.leftPx - box.widthPx),
      Math.max(0, box.topPx - yPx, yPx - box.topPx - box.heightPx),
    );
  };
  // A distance to a box is convex along a segment, so a ternary search finds its least.
  let low = 0;
  let high = 1;
  for (let step = 0; step < 100; step += 1) {
    const a = low + (high - low) / 3;
    const b = high - (high - low) / 3;
    if (at(a) <= at(b)) {
      high = b;
    } else {
      low = a;
    }
  }
  return Math.min(at(0), at(1), at((low + high) / 2)) - halfWidthPx;
}

/**
 * The destination's four places, size classes, interface scales and ratios its label is tested at:
 * classes 0 to 4, their radii 0.25 to 0.5 rem, at 80%, 100% and 150% (a rem of 12.8, 16 and 24 px)
 * and the ratios in use.
 */
const DESTINATION_CASES = DESTINATION_LABEL_PLACES.flatMap((place) =>
  ([0, 1, 2, 3, 4] as const).flatMap((sizeClass) =>
    [12.8, 16, 24].flatMap((remPx) =>
      RATIOS.map((ratio) => [place, sizeClass, remPx, ratio] as const),
    ),
  ),
);

/**
 * The destination's mark of a size class at a rem, placed so that its label takes `place`: near the
 * view's right edge for a left-hand place, near its top for a lower one, so that the places before
 * it run out of the view.
 */
function destinationAnchor(
  place: DestinationLabelPlace,
  sizeClass: SizeClass,
  remPx: number,
): Anchor {
  return anchor(
    "a",
    placeIsRight(place) ? 100 : 390,
    placeIsUpper(place) ? 150 : 15,
    (SIZE_CLASS_REM[sizeClass] * remPx) / 2,
  );
}

/** The destination's label, "HD 140283", placed about `at` with no other mark in the view. */
function placedDestination(
  at: Anchor,
  strokes: ReticleStrokesCss,
  remPx: number,
): PlacedLabel | undefined {
  return placeLabels([mark("a", 1, "HD 140283")], [at], { ...VIEWPORT, remPx }, strokes, "a")[0];
}

/** Which of its four places a destination's label stands at about `at`, from its box. */
function placeOf(label: PlacedLabel | undefined, at: Anchor): DestinationLabelPlace | null {
  if (label === undefined) {
    return null;
  }
  const upper = label.topPx + label.heightPx <= at.yPx;
  if (!upper && label.topPx < at.yPx) {
    return null;
  }
  if (label.side === "right") {
    return upper ? "upper-right" : "lower-right";
  }
  return upper ? "upper-left" : "lower-left";
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

  it("draws no label for a mark in a view too narrow for it on either side, rather than hold it back across the mark", () => {
    // "9FG 567Z04 B-3", 121.52 px wide, its box 14.75 px from its mark's centre: at the right it
    // would end at 216.27 px, past 156, and at the left start at -56.27, short of 4 (P12.T11.e).
    const narrow: Viewport = { widthPx: 160, heightPx: 100, remPx: 16 };

    expect(
      placeLabels([mark("a", 1, "9FG 567Z04 B-3")], [anchor("a", 80, 50)], narrow, AT_TWO, null),
    ).toEqual([]);
  });

  it("holds a label 0.25 rem inside the top edge beside its mark, where one side has room", () => {
    const narrow: Viewport = { widthPx: 160, heightPx: 100, remPx: 16 };
    const [label] = placeLabels(
      [mark("a", 1, "9FG 567Z04 B-3")],
      [anchor("a", 10, 4)],
      narrow,
      AT_TWO,
      null,
    );

    expect([label?.side, label?.leftPx, label?.topPx]).toEqual([
      "right",
      10 + 6 + OFFSET_AT_TWO_PX,
      4,
    ]);
    expect((label?.leftPx ?? 0) + (label?.widthPx ?? 0)).toBeLessThanOrEqual(156);
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

    const placed = placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null, {
      pinnedIds: ["picked"],
      obstacles: [furniture],
    });

    expect(placed.map((label) => label.id)).toEqual(["picked", "clear"]);
  });

  it("never drops the selected label", () => {
    const chosen = chooseLabels([mark("heavy", 9), mark("picked", 1)], "picked", null);
    const anchors = [anchor("heavy", 100, 100), anchor("picked", 102, 101)];

    const placed = placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null, { pinnedIds: ["picked"] });

    expect(placed.map((label) => label.id)).toEqual(["picked"]);
  });

  it("keeps the selected label even when its mark is out of view", () => {
    const placed = placeLabels(
      [mark("picked", 1)],
      [anchor("picked", -50, 100)],
      VIEWPORT,
      AT_TWO,
      null,
      { pinnedIds: ["picked"] },
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
    "starts a label's box on the %s 0.5 rem and 3.40, 2.00, 0.75 and 0.75 px beyond its symbol",
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
        return [label?.side, nearPx - 6 - 0.5 * VIEWPORT.remPx];
      });

      expect(gaps.map(([at, gap]) => [at, Math.round(Number(gap) * 100) / 100])).toEqual(
        [3.4, 2, 0.75, 0.75].map((gap) => [side, gap]),
      );
    },
  );

  it.each(DESTINATION_CASES)(
    "stands the destination's label's box at the %s wholly beyond all four chevrons' ink by 0.25 rem, about class %s at a rem of %s px and a ratio of %s",
    (place, sizeClass, remPx, ratio) => {
      const strokes = reticleStrokesCssPx(ratio);
      const at = destinationAnchor(place, sizeClass, remPx);
      const label = placedDestination(at, strokes, remPx);
      const ys = chevronStrokes(at, strokes, remPx).flatMap(([from, to]) => [from.yPx, to.yPx]);
      const inkTopPx = Math.min(...ys) - strokes.markStrokePx / 2;
      const inkBottomPx = Math.max(...ys) + strokes.markStrokePx / 2;
      const topPx = label?.topPx ?? Number.NaN;
      const bottomPx = topPx + (label?.heightPx ?? Number.NaN);
      // An upper box lies wholly above the upper chevron's ink, a lower one wholly below the lower's.
      const gapPx = placeIsUpper(place) ? inkTopPx - bottomPx : topPx - inkBottomPx;

      expect([placeOf(label, at), gapPx >= 0.25 * remPx - 1e-9]).toEqual([place, true]);
    },
  );

  it.each(DESTINATION_CASES)(
    "stands the destination's label's box at the %s 0.25 rem clear of the selection's bracket, about class %s at a rem of %s px and a ratio of %s",
    (place, sizeClass, remPx, ratio) => {
      const strokes = reticleStrokesCssPx(ratio);
      const at = destinationAnchor(place, sizeClass, remPx);
      const label = placedDestination(at, strokes, remPx);

      expect(
        bracketStrokes(at, strokes, remPx).every(
          (stroke) =>
            label !== undefined &&
            boxInkDistancePx(label, stroke, strokes.markStrokePx / 2) >= 0.25 * remPx - 1e-9,
        ),
      ).toBe(true);
    },
  );

  it.each(RATIOS)("keeps the destination's label's box at its bracket place, at %s", (ratio) => {
    const strokes = reticleStrokesCssPx(ratio);
    const [alone] = placeLabels([mark("a", 1)], [anchor("a", 100, 100)], VIEWPORT, strokes, null);
    const [label] = placeLabels([mark("a", 1)], [anchor("a", 100, 100)], VIEWPORT, strokes, "a");

    expect(label?.leftPx).toBe(alone?.leftPx);
  });

  it.each(RATIOS)(
    "stands the destination's label's bottom edge 0.25 rem above the upper chevron's ink, at %s",
    (ratio) => {
      const strokes = reticleStrokesCssPx(ratio);
      const at = anchor("a", 100, 100);
      const [label] = placeLabels([mark("a", 1)], [at], VIEWPORT, strokes, "a");
      // The upper chevron's apex, the least gap or 0.25 rem outside the bracket, and its arm ends
      // an arm's run above it; its ink half the mark stroke beyond.
      const inkPx =
        bracketPx(strokes, at) +
        Math.max(4, strokes.minGapPx) +
        bracketArmPx(bracketPx(strokes, at)) * Math.SQRT1_2 +
        strokes.markStrokePx / 2;

      expect(100 - inkPx - ((label?.topPx ?? 0) + (label?.heightPx ?? 0))).toBeCloseTo(4, 9);
    },
  );

  it.each([
    ["class 0", 4, 14, 22.24],
    ["a craft", 6, 16, 25.19],
    ["class 4", 8, 18, 28.13],
  ] as const)(
    "stands %s's destination box, of radius %s px, its near edge %s px and its bottom edge %s px from the mark's centre, at a ratio of 1 and 100%",
    (_, radiusPx, nearPx, risePx) => {
      const [label] = placeLabels(
        [mark("a", 1)],
        [anchor("a", 100, 100, radiusPx)],
        VIEWPORT,
        reticleStrokesCssPx(1),
        "a",
      );

      expect([
        Math.round(((label?.leftPx ?? 0) - 100) * 100) / 100,
        Math.round((100 - (label?.topPx ?? 0) - (label?.heightPx ?? 0)) * 100) / 100,
      ]).toEqual([nearPx, risePx]);
    },
  );

  it("sends the destination's label to the upper left where a neighbour's mark stands within 0.5 rem of the upper right", () => {
    const strokes = reticleStrokesCssPx(1);
    // The upper-right box: 116 to 124.7 px across, 57.3 to 74.8 px down. The neighbour's bracket
    // place, 12 px about (130, 45), reaches 57 px down over its columns; the upper left, 75.3 to 84
    // px across, is 34 px off it.
    const at = anchor("a", 100, 100);
    const [label] = placeLabels(
      [mark("a", 1)],
      [at, anchor("near", 130, 45)],
      VIEWPORT,
      strokes,
      "a",
    );

    expect(placeOf(label, at)).toBe("upper-left");
  });

  it("sends the destination's label to the upper left where the selection's label stands within 0.5 rem of the upper right", () => {
    const strokes = reticleStrokesCssPx(1);
    // The selection's label, placed first, on its line from 35.25 to 52.75 px down over the
    // upper-right box's columns, 4.6 px above it; its mark's bracket place stands 8.3 px off.
    const chosen = [mark("picked", 9), mark("a", 1)];
    const at = anchor("a", 100, 100);
    const [, label] = placeLabels(
      chosen,
      [anchor("picked", 106, 44, 0), at],
      VIEWPORT,
      strokes,
      "a",
      { pinnedIds: ["picked", "a"] },
    );

    expect(placeOf(label, at)).toBe("upper-left");
  });

  it("sends the destination's label to the upper left where furniture covers the upper right", () => {
    const strokes = reticleStrokesCssPx(1);
    const at = anchor("a", 100, 100);
    const furniture = { leftPx: 110, topPx: 50, widthPx: 40, heightPx: 20 };
    const [label] = placeLabels([mark("a", 1)], [at], VIEWPORT, strokes, "a", {
      obstacles: [furniture],
    });

    expect(placeOf(label, at)).toBe("upper-left");
  });

  it("keeps the destination's label at the upper right where every place is blocked", () => {
    const strokes = reticleStrokesCssPx(1);
    // A neighbour's bracket place, 12 px about it, within 0.5 rem of each of the four boxes.
    const around = [
      [130, 45],
      [70, 45],
      [130, 155],
      [70, 155],
    ].map(([xPx = 0, yPx = 0], i) => anchor(`near-${String(i)}`, xPx, yPx));
    const at = anchor("a", 100, 100);
    const [label] = placeLabels([mark("a", 1)], [at, ...around], VIEWPORT, strokes, "a");

    expect(placeOf(label, at)).toBe("upper-right");
  });

  it("keeps the destination's label at the upper left, the first place inside the view, where both left-hand places are blocked at the right edge", () => {
    const strokes = reticleStrokesCssPx(1);
    // At 390 px the right-hand places run past the view's edge; a neighbour's bracket place within
    // 0.5 rem of each left-hand box, 365.3 to 374 px across.
    const at = anchor("a", 390, 100);
    const around = [
      [380, 45],
      [380, 155],
    ].map(([xPx = 0, yPx = 0], i) => anchor(`near-${String(i)}`, xPx, yPx));
    const [label] = placeLabels([mark("a", 1)], [at, ...around], VIEWPORT, strokes, "a");

    expect(placeOf(label, at)).toBe("upper-left");
  });

  it("drops a lesser label within 0.5 rem of the destination's where every place is blocked", () => {
    const strokes = reticleStrokesCssPx(1);
    // Unlabelled neighbours block every place, so the destination keeps the upper right; the lesser
    // label, on its line from 35.25 to 52.75 px down over that box's columns, is 4.6 px above it.
    const chosen = [mark("a", 1), mark("lesser", 0), mark("apart", 0)];
    const around = [
      [130, 45],
      [70, 45],
      [130, 155],
      [70, 155],
    ].map(([xPx = 0, yPx = 0], i) => anchor(`near-${String(i)}`, xPx, yPx));
    const anchors = [
      anchor("a", 100, 100),
      anchor("lesser", 100, 44, 0),
      anchor("apart", 100, 250),
      ...around,
    ];

    expect(placeLabels(chosen, anchors, VIEWPORT, strokes, "a").map((label) => label.id)).toEqual([
      "a",
      "apart",
    ]);
  });

  it("drops a lesser label whose box comes within 0.25 rem of the destination's chevron set", () => {
    const strokes = reticleStrokesCssPx(1);
    // The lesser label, on its line from 50 to 102 px across, runs into the set, 21.2 px about the
    // destination's centre.
    const chosen = [mark("a", 1), mark("lesser", 0), mark("apart", 0)];
    const anchors = [
      anchor("a", 100, 100),
      anchor("lesser", 40, 100, 0),
      anchor("apart", 100, 250),
    ];

    expect(placeLabels(chosen, anchors, VIEWPORT, strokes, "a").map((label) => label.id)).toEqual([
      "a",
      "apart",
    ]);
  });

  it("keeps the destination's label where it stands whether or not a labelled neighbour is selected", () => {
    const strokes = reticleStrokesCssPx(1);
    // The neighbour's label, near the upper-right box, sends it to the upper left either way: the
    // destination takes every chosen label at its place, placed before it or not.
    const anchors = [anchor("a", 100, 100), anchor("n", 100, 44, 0)];
    const unselected = placeLabels([mark("a", 1), mark("n", 0)], anchors, VIEWPORT, strokes, "a", {
      pinnedIds: ["a"],
    });
    const selected = placeLabels([mark("n", 0), mark("a", 1)], anchors, VIEWPORT, strokes, "a", {
      pinnedIds: ["n", "a"],
    });

    expect(selected.find((label) => label.id === "a")).toEqual(
      unselected.find((label) => label.id === "a"),
    );
  });

  it("takes no account of a chosen label whose mark is out of view, which is never drawn", () => {
    const strokes = reticleStrokesCssPx(1);
    // Held inside the view, the out-of-view mark's label would lie over both upper places, from 5 to
    // 144 px across and 57.25 to 74.75 px down; it is dropped, and the destination keeps the upper
    // right.
    const chosen = [mark("far", 9, "OUTSIDE THE VIEW"), mark("a", 1)];
    const at = anchor("a", 100, 100);
    const placed = placeLabels(chosen, [anchor("far", -5, 66, 0), at], VIEWPORT, strokes, "a", {
      pinnedIds: ["a"],
    });

    expect(
      placeOf(
        placed.find((label) => label.id === "a"),
        at,
      ),
    ).toBe("upper-right");
  });

  it("moves the destination's label above its chevrons on the destination's report, and no other label", () => {
    const strokes = reticleStrokesCssPx(1);
    const chosen = [mark("a", 1), mark("b", 0)];
    const anchors = [anchor("a", 100, 100), anchor("b", 100, 220)];
    const places = (destinationId: string | null): Array<[number, number]> =>
      placeLabels(chosen, anchors, VIEWPORT, strokes, destinationId).map((label) => [
        label.leftPx,
        label.topPx,
      ]);

    const [[aLeft, aTop] = [0, 0], other] = places(null);
    const [[destinationLeft, destinationTop] = [0, 0], unmoved] = places("a");

    expect([destinationLeft === aLeft, destinationTop < aTop, unmoved]).toEqual([
      true,
      true,
      other,
    ]);
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

  it("estimates a label's height as the stylesheet lays it out, one line of 0.875 rem at 1.25", () => {
    // A raised destination's label is placed by its top, so that its bottom edge stands where the
    // box's does only if the box is as tall as the line.
    const rule = stylesheetRule(".spatial-label");
    expect([
      rule.includes("font-size: 0.875rem;"),
      rule.includes("line-height: 1.25;"),
      textSizeRem("PGG 5H0001 A-1").heightRem,
    ]).toEqual([true, true, 0.875 * 1.25]);
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

/**
 * A chart's marks about a destination "d" at (200, 150), labelled `DEST`: eight of the highest
 * priority along the foot of the view, which `chooseLabels` labels, and a mark "s" of `priority`
 * at `at`, which it labels only while it is selected where its priority is the lowest.
 */
function chartAbout(at: readonly [number, number], priority: number) {
  const heavy = Array.from({ length: 8 }, (_, index) => mark(`h${String(index)}`, 10 + index));
  const points = [mark("d", 0, "DEST"), mark("s", priority, "S1"), ...heavy];
  const anchors = [
    anchor("d", 200, 150),
    anchor("s", at[0], at[1]),
    ...heavy.map((point, index) => anchor(point.id, 20 + index * 45, 285)),
  ];
  /** The labels placed with `selectedId` selected, as `SpatialView` places them. */
  const placed = (selectedId: string | null): ReadonlyArray<PlacedLabel> => {
    const chosen = chooseLabels(points, selectedId, "d");
    const unselected = new Set(chooseLabels(points, null, "d").map((point) => point.id));
    const pinned = [selectedId, "d"].filter((id): id is string => id !== null);
    return placeLabels(chosen, anchors, VIEWPORT, reticleStrokesCssPx(1), "d", {
      pinnedIds: pinned,
      unselectedIds: unselected,
    });
  };
  return { placed, destination: anchors[0] ?? anchor("d", 200, 150) };
}

/** The destination's chevron set about `at` at a ratio of 1 and 100%, CSS px: 21.19 px about it. */
function chevronSetAbout(at: Anchor): BoxPx {
  const strokes = reticleStrokesCssPx(1);
  const halfPx = bracketPx(strokes, at);
  const reachPx =
    halfPx + destinationGapPx(16, strokes.minGapPx) + bracketArmPx(halfPx) * Math.SQRT1_2 + 1;
  return {
    leftPx: at.xPx - reachPx,
    topPx: at.yPx - reachPx,
    widthPx: 2 * reachPx,
    heightPx: 2 * reachPx,
  };
}

/** The label of `id` among those placed, which must be there. */
function labelOf(placed: ReadonlyArray<PlacedLabel>, id: string): PlacedLabel {
  const label = placed.find((candidate) => candidate.id === id);
  if (label === undefined) {
    throw new Error(`no label is placed for ${id}`);
  }
  return label;
}

describe("placeLabels about the destination, as though nothing were selected (R07.T16.j)", () => {
  it("leaves the destination's label at the upper right on selecting a mark outside chooseLabels' count beside it", () => {
    // The selection's label at the right, from 196 px across and 91.25 to 108.75 px down, would
    // run into the destination's at the upper right, from 216 px across and 107.31 to 124.81 px down.
    const chart = chartAbout([180, 100], -1);

    const unselected = labelOf(chart.placed(null), "d");
    const selected = labelOf(chart.placed("s"), "d");

    expect([selected, placeOf(selected, chart.destination)]).toEqual([unselected, "upper-right"]);
  });

  it("flips the label of a selection outside chooseLabels' count beside the destination to its left, 0.5 rem clear of the destination's label and 0.25 rem of its chevrons", () => {
    const chart = chartAbout([180, 100], -1);

    const placed = chart.placed("s");
    const selection = labelOf(placed, "s");

    expect([
      selection.side,
      boxGapPx(selection, labelOf(placed, "d")) >= 8,
      boxGapPx(selection, chevronSetAbout(chart.destination)) >= 4,
    ]).toEqual(["left", true, true]);
  });

  it.each([
    ["outside chooseLabels' count", -1],
    ["among chooseLabels' count", 30],
  ] as const)(
    "draws no label for a selection %s whose label would run into the destination's chevrons on either side",
    (_, priority) => {
      // Just below the destination: either side of its line runs into the chevron set, 21.19 px
      // about the destination's centre.
      const chart = chartAbout([200, 175], priority);

      const unselected = chart.placed(null).find((label) => label.id === "d");
      const selected = chart.placed("s");

      expect([
        selected.find((label) => label.id === "d"),
        selected.some((label) => label.id === "s"),
      ]).toEqual([unselected, false]);
    },
  );

  it("draws no label for a selection at the view's right edge whose other side would be held back across its own mark", () => {
    // The selection 0.5 rem inside the right edge has its label flipped to the left, from 358.64 to
    // 376 px across, which runs into the destination's chevron set below it. Its right-hand side
    // runs off the view, and held inside it would stand over the selection's own bracket.
    const placed = placeLabels(
      [mark("s", 1, "S1"), mark("d", 0, "DEST")],
      [anchor("s", 392, 100), anchor("d", 350, 130)],
      VIEWPORT,
      reticleStrokesCssPx(1),
      "d",
      { pinnedIds: ["s", "d"] },
    );

    expect(placed.map((label) => label.id)).toEqual(["d"]);
  });

  it("draws no label for a destination at a corner of a view too small for any place, and none on its chevrons", () => {
    // A view 60 px square: every place about a destination 8 px from its top left runs out of it,
    // or within 0.25 rem of its edges. A neighbour's label, flipped to the left, runs into the set.
    const small: Viewport = { widthPx: 60, heightPx: 60, remPx: 16 };
    const at = anchor("d", 8, 8);
    const placed = placeLabels(
      [mark("d", 1, "DEST"), mark("n", 0, "N")],
      [at, anchor("n", 40, 30)],
      small,
      reticleStrokesCssPx(1),
      "d",
      { pinnedIds: ["d"] },
    );

    expect([
      placed.some((label) => label.id === "d"),
      placed.every((label) => boxGapPx(label, chevronSetAbout(at)) >= 4),
    ]).toEqual([false, true]);
  });

  it("draws no label for a destination whose only place inside the view lies within 0.25 rem of its edge", () => {
    // About a destination at (8, 8) in a view 62 px wide the lower right ends 58.72 px across,
    // 3.28 px inside the right edge; 70 px wide, it stands 0.25 rem inside.
    const placed = (widthPx: number) =>
      placeLabels(
        [mark("d", 1, "DEST")],
        [anchor("d", 8, 8)],
        { widthPx, heightPx: 120, remPx: 16 },
        reticleStrokesCssPx(1),
        "d",
        { pinnedIds: ["d"] },
      ).map((label) => label.id);

    expect([placed(62), placed(70)]).toEqual([[], ["d"]]);
  });
});

describe("placeLabels apart and off the edges (R07.T16.j; addendum D, D4 and D5)", () => {
  it.each([
    ["0.375 rem apart, drops the lower-priority one", 6, ["upper"]],
    ["0.625 rem apart, draws both", 10, ["upper", "lower"]],
  ] as const)("of two lesser labels stacked %s", (_, apartPx, drawn) => {
    // Each label is one line, 17.5 px tall, centred on its mark.
    const placed = placeLabels(
      [mark("upper", 1), mark("lower", 0)],
      [anchor("upper", 100, 100), anchor("lower", 100, 100 + 17.5 + apartPx)],
      VIEWPORT,
      AT_TWO,
      null,
    );

    expect(placed.map((label) => label.id)).toEqual(drawn);
  });

  it.each([
    ["0.375 rem off, is not drawn", 6, []],
    ["0.625 rem off, is drawn", 10, ["lesser"]],
  ] as const)(
    "of a lesser label below other text over the view, a curve label or the core arrow's, one %s",
    (_, apartPx, drawn) => {
      // The label, centred on its mark at 100 px down, runs from 91.25 to 108.75 px.
      const text: BoxPx = {
        leftPx: 90,
        topPx: 91.25 - apartPx - 17.5,
        widthPx: 120,
        heightPx: 17.5,
      };
      const placed = placeLabels(
        [mark("lesser", 1)],
        [anchor("lesser", 100, 100)],
        VIEWPORT,
        AT_TWO,
        null,
        {
          texts: [text],
        },
      );

      expect(placed.map((label) => label.id)).toEqual(drawn);
    },
  );

  it("draws no destination label where every place inside the view lies over furniture or other text", () => {
    // Furniture across the destination's two upper places, and a curve label across its lower ones.
    const at = anchor("a", 100, 100);
    const placed = placeLabels([mark("a", 1)], [at], VIEWPORT, reticleStrokesCssPx(1), "a", {
      pinnedIds: ["a"],
      obstacles: [{ leftPx: 20, topPx: 50, widthPx: 160, heightPx: 30 }],
      texts: [{ leftPx: 20, topPx: 120, widthPx: 160, heightPx: 30 }],
    });

    expect(placed).toEqual([]);
  });

  it("takes the destination's first place over no furniture where none is 0.5 rem clear of the marks", () => {
    // Neighbours within 0.5 rem of all four places; furniture over the two upper ones.
    const at = anchor("a", 100, 100);
    const around = [
      [130, 45],
      [70, 45],
      [130, 155],
      [70, 155],
    ].map(([xPx = 0, yPx = 0], i) => anchor(`near-${String(i)}`, xPx, yPx));
    const [label] = placeLabels(
      [mark("a", 1)],
      [at, ...around],
      VIEWPORT,
      reticleStrokesCssPx(1),
      "a",
      {
        pinnedIds: ["a"],
        obstacles: [{ leftPx: 20, topPx: 50, widthPx: 160, heightPx: 30 }],
      },
    );

    expect(placeOf(label, at)).toBe("lower-right");
  });

  it("starts a pinned label held inside at the left 0.25 rem in", () => {
    const [label] = placeLabels(
      [mark("picked", 1)],
      [anchor("picked", -50, 100)],
      VIEWPORT,
      AT_TWO,
      null,
      { pinnedIds: ["picked"] },
    );

    expect(label?.leftPx).toBe(4);
  });

  it.each([
    ["0.125 rem", 2, "left"],
    ["0.375 rem", 6, "right"],
  ] as const)(
    "flips a label whose right place would end %s inside the right edge, only within 0.25 rem of it",
    (_, insidePx, side) => {
      // "HD 140283", 78.12 px wide, its box 14.75 px from its mark's centre at a ratio of 2.
      const xPx = 400 - insidePx - 78.12 - OFFSET_AT_TWO_PX - 6;
      const [label] = placeLabels(
        [mark("a", 1, "HD 140283")],
        [anchor("a", xPx, 100)],
        VIEWPORT,
        AT_TWO,
        null,
      );

      expect(label?.side).toBe(side);
    },
  );
});

describe("placeLabels off the observer's eye (plan 12, P12.T11.c)", () => {
  /** The eye's 1 rem box about a point, as `observerBoxPx` gives it. */
  function eyeAt(xPx: number, yPx: number): BoxPx {
    const halfPx = VIEWPORT.remPx / 2;
    return { leftPx: xPx - halfPx, topPx: yPx - halfPx, widthPx: 2 * halfPx, heightPx: 2 * halfPx };
  }

  // `SOL-1` beside a mark at (100, 100) runs from 114.75 to 158.15 px across at a ratio of 2.
  const COVERED_EYE = eyeAt(136, 100);

  it("drops a label that would cover the eye, as one over the core arrow is", () => {
    const placed = placeLabels(
      [mark("a", 1, "SOL-1"), mark("b", 0, "SOL-2")],
      [anchor("a", 100, 100), anchor("b", 100, 200)],
      VIEWPORT,
      AT_TWO,
      null,
      { observer: COVERED_EYE },
    );

    expect(placed.map((label) => label.id)).toEqual(["b"]);
  });

  it("keeps the label of the mark the eye stands on, which starts outside the eye's box", () => {
    const chosen = [mark("a", 1, "SOL-1")];
    const anchors = [anchor("a", 100, 100)];

    expect(
      placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null, { observer: eyeAt(100, 100) }),
    ).toEqual(placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null));
  });

  it("puts the selection's label on its other side where it would cover the eye", () => {
    const [label] = placeLabels(
      [mark("a", 1, "SOL-1")],
      [anchor("a", 100, 100)],
      VIEWPORT,
      AT_TWO,
      null,
      { pinnedIds: ["a"], observer: COVERED_EYE },
    );

    expect(label?.side).toBe("left");
    expect((label?.leftPx ?? 0) + (label?.widthPx ?? 0)).toBeCloseTo(100 - 6 - OFFSET_AT_TWO_PX, 9);
  });

  it("draws no label for the selection where its other side is not clear either", () => {
    // At 20 px across, the left side would be held inside the view back across the bracket.
    const placed = placeLabels(
      [mark("a", 1, "SOL-1")],
      [anchor("a", 20, 100)],
      VIEWPORT,
      AT_TWO,
      null,
      { pinnedIds: ["a"], observer: eyeAt(56, 100) },
    );

    expect(placed).toEqual([]);
  });

  it("moves the destination's label off the eye to another of its places", () => {
    const at = anchor("d", 200, 150);
    const chosen = [mark("d", 1, "DEST")];
    const [first] = placeLabels(chosen, [at], VIEWPORT, AT_TWO, "d", { pinnedIds: ["d"] });
    if (first === undefined) {
      throw new Error("the destination has no label to move");
    }
    const eye = eyeAt(first.leftPx + first.widthPx / 2, first.topPx + first.heightPx / 2);

    const [moved] = placeLabels(chosen, [at], VIEWPORT, AT_TWO, "d", {
      pinnedIds: ["d"],
      observer: eye,
    });

    expect(moved).toBeDefined();
    expect(boxGapPx(moved ?? first, eye)).toBeGreaterThan(0);
  });

  it("places every label as before where no eye is drawn", () => {
    const chosen = [mark("a", 2, "SOL-1"), mark("b", 1, "SOL-2"), mark("c", 0, "SOL-3")];
    const anchors = [anchor("a", 100, 100), anchor("b", 130, 108), anchor("c", 380, 200)];

    expect(
      placeLabels(chosen, anchors, VIEWPORT, AT_TWO, "b", { pinnedIds: ["a"], observer: null }),
    ).toEqual(placeLabels(chosen, anchors, VIEWPORT, AT_TWO, "b", { pinnedIds: ["a"] }));
  });
});

describe("placeLabels on the compact chart's 256 px stage (plan 12, P12.T11.e)", () => {
  /** The compact `LOCAL CHART`'s stage at 1280 × 720 and 100%. */
  const STAGE: Viewport = { widthPx: 256, heightPx: 262, remPx: 16 };
  const AT_ONE = reticleStrokesCssPx(1);
  /** A 16-character designation, 138.88 px wide at 0.62 em a character. */
  const DESIGNATION = "GDG 0E0V00 E-141";
  /** A class-4 symbol's radius, 0.5 rem: its label's box starts 18 px from its centre at 1. */
  const RADIUS_PX = 8;

  /** The selection's bracket's box about `at` at a ratio of 1, to its line's outer edge. */
  function bracketBoxAbout(at: Anchor): BoxPx {
    return squareBoxPx(at, bracketPx(AT_ONE, at) + AT_ONE.markStrokePx / 2);
  }

  it("draws no label for a centred selection whose label fits on neither side, and none over its bracket", () => {
    // Held inside the stage its label would run from 4 to 142.88 px across, over the mark at 128.
    const at = anchor("s", 128, 131, RADIUS_PX);
    const placed = placeLabels(
      [mark("s", 1, DESIGNATION), mark("n", 0, "N1")],
      [at, anchor("n", 40, 40)],
      STAGE,
      AT_ONE,
      null,
      { pinnedIds: ["s"] },
    );

    expect([
      placed.map((label) => label.id),
      placed.every((label) => boxGapPx(label, bracketBoxAbout(at)) > 0),
    ]).toEqual([["n"], true]);
  });

  it("draws no label for an unselected mark at the stage's middle", () => {
    expect(
      placeLabels(
        [mark("m", 1, DESIGNATION)],
        [anchor("m", 128, 131, RADIUS_PX)],
        STAGE,
        AT_ONE,
        null,
      ),
    ).toEqual([]);
  });

  it.each([
    [1, 95, "right"],
    [1, 96, null],
    [1, 128, null],
    [1, 160, null],
    [1, 161, "left"],
    [1.5, 96, "right"],
    [1.5, 97, null],
    [1.5, 159, null],
    [1.5, 160, "left"],
  ] as const)(
    "at a ratio of %s labels a 16-character designation about a class-4 mark %s px across on its side with room: %s",
    (ratio, xPx, side) => {
      // Its box starts 18 px out at 1 and 16.75 at 1.5, so it has no side from 95.12 to 160.88 px
      // across at 1 and from 96.37 to 159.63 at 1.5.
      const [label] = placeLabels(
        [mark("m", 1, DESIGNATION)],
        [anchor("m", xPx, 131, RADIUS_PX)],
        STAGE,
        reticleStrokesCssPx(ratio),
        null,
      );

      expect(label?.side ?? null).toBe(side);
    },
  );

  it.each([
    ["top", -2],
    ["top", -13],
    ["bottom", 263],
    ["bottom", 275],
  ] as const)(
    "draws no label for a selection just past the %s edge, at %s px, whose held label would stand within 0.25 rem of what is drawn of its bracket",
    (_, yPx) => {
      // At (128, -2) the held label would stand from 4 to 142.88 px across and 4 to 21.5 px down,
      // over the bracket's box, from 114 to 142 px across and -16 to 12 px down, and the symbol;
      // at -13 and 275 the bracket's box still reaches 1 px into the view, 3 px from the label.
      expect(
        placeLabels(
          [mark("s", 1, DESIGNATION)],
          [anchor("s", 128, yPx, RADIUS_PX)],
          STAGE,
          AT_ONE,
          null,
          { pinnedIds: ["s"] },
        ),
      ).toEqual([]);
    },
  );

  it.each([
    ["top", -14, 4],
    ["top", -15, 4],
    ["bottom", 276, 262 - 4 - 17.5],
    ["bottom", 277, 262 - 4 - 17.5],
  ] as const)(
    "holds the selection's label 0.25 rem inside the %s edge once its bracket lies wholly outside the view, at %s px",
    (_, yPx, topPx) => {
      // The bracket reaches 14 px from the mark's centre, so 14 px past the edge its box meets the
      // view's edge, the held label 0.25 rem clear of it, and 15 px past it 5 px clear.
      const placed = placeLabels(
        [mark("s", 1, DESIGNATION)],
        [anchor("s", 128, yPx, RADIUS_PX)],
        STAGE,
        AT_ONE,
        null,
        { pinnedIds: ["s"] },
      );

      expect(placed.map((label) => [label.leftPx, label.topPx])).toEqual([[4, topPx]]);
    },
  );

  it("keeps the selection's label on the right 2 rem from the left edge, as before", () => {
    const [label] = placeLabels(
      [mark("s", 1, DESIGNATION)],
      [anchor("s", 32, 131, RADIUS_PX)],
      STAGE,
      AT_ONE,
      null,
      { pinnedIds: ["s"] },
    );

    expect([label?.side, label?.leftPx]).toEqual(["right", 32 + 18]);
  });

  it("draws no label for a selection held back on both sides, and places the destination's as with no selection", () => {
    // The selection's label, held inside, would stand far from the destination's, so only the
    // hold's refusal drops it.
    const points = [mark("d", 0, "DEST"), mark("s", -1, DESIGNATION)];
    const anchors = [anchor("d", 60, 60), anchor("s", 128, 131, RADIUS_PX)];
    const placed = (selectedId: string | null): ReadonlyArray<PlacedLabel> =>
      placeLabels(chooseLabels(points, selectedId, "d", 0), anchors, STAGE, AT_ONE, "d", {
        pinnedIds: [selectedId, "d"].filter((id): id is string => id !== null),
        unselectedIds: new Set(chooseLabels(points, null, "d", 0).map((point) => point.id)),
      });

    const selected = placed("s");

    expect([selected.map((label) => label.id), labelOf(selected, "d")]).toEqual([
      ["d"],
      labelOf(placed(null), "d"),
    ]);
  });

  it("takes no account of a chosen label that would be held back across its own mark, which is never drawn", () => {
    // Held inside, the label of the mark at (140, 100) would run from 4 to 142.88 px across and
    // 91.25 to 108.75 px down, over both of the destination's upper places, from 97.31 to 114.81 px
    // down; it is not drawn, and the destination keeps the upper right.
    const at = anchor("a", 60, 140);
    const placed = placeLabels(
      [mark("m", 9, DESIGNATION), mark("a", 1, "DEST")],
      [anchor("m", 140, 100, 0), at],
      STAGE,
      AT_ONE,
      "a",
      { pinnedIds: ["a"] },
    );

    expect([placed.map((label) => label.id), placeOf(labelOf(placed, "a"), at)]).toEqual([
      ["a"],
      "upper-right",
    ]);
  });
});

/** A text box over the view from its edges, as `SpatialView` gives `texts`. */
function textBox(leftPx: number, topPx: number, rightPx: number, bottomPx: number): BoxPx {
  return { leftPx, topPx, widthPx: rightPx - leftPx, heightPx: bottomPx - topPx };
}

describe("placeLabels: the selection's label off the other text over the view (plan 12, P12.T11.e)", () => {
  it("draws no label for a selection whose one side lies over the core arrow's label, on the compact stage at 150%", () => {
    // Main's landing look at #79 to #81, 1920 x 1080 at 150%, OBSERVED: the 256.67 x 232 px stage,
    // `GDG 75DY08 A-3` selected at (145.5, 47.7) beside `CORE 8914.7 ly` (4 to 128.6 px across,
    // 37 to 54.5 px down). Its right side has no room, and its left ran across the core label.
    const stage: Viewport = { widthPx: 256.66668701171875, heightPx: 232, remPx: 16 };
    const strokes = reticleStrokesCssPx(1.5);
    const chosen = [mark("s", 1, "GDG 75DY08 A-3")];
    const anchors = [anchor("s", 145.5, 47.7, 4)];
    const core = textBox(4, 37, 128.6, 54.5);

    const alone = placeLabels(chosen, anchors, stage, strokes, null, { pinnedIds: ["s"] });
    const beside = placeLabels(chosen, anchors, stage, strokes, null, {
      pinnedIds: ["s"],
      texts: [core],
    });

    expect([alone.map((label) => label.side), beside]).toEqual([["left"], []]);
  });

  it("puts the selection's label on its other side where it would lie over other text", () => {
    // `SOL-1` beside a mark at (200, 150) runs from 214.75 to 258.15 px across on its right.
    const [label] = placeLabels(
      [mark("a", 1, "SOL-1")],
      [anchor("a", 200, 150)],
      VIEWPORT,
      AT_TWO,
      null,
      { pinnedIds: ["a"], texts: [textBox(230, 140, 300, 160)] },
    );

    expect(label?.side).toBe("left");
    expect((label?.leftPx ?? 0) + (label?.widthPx ?? 0)).toBeCloseTo(200 - 6 - OFFSET_AT_TWO_PX, 9);
  });

  it("keeps the selection's label between two lines whose boxes it overlaps by their leading alone", () => {
    // Main's landing look, the orbit map at 1280 x 720 (787.33 x 184 px): `GDG 75DX08 A-7 /256`
    // selected at (393.8, 92.1), its line box from 83.35 to 100.85 px down, between
    // `HABITABLE ZONE` (70.7 to 88.2) and `SNOW LINE` (96.3 to 113.8), with `BELT 1` and
    // `OPTIMISTIC` to its left. The boxes overlap by about 5 px; the capitals' ink does not.
    const orbit: Viewport = { widthPx: 787.328125, heightPx: 184, remPx: 16 };
    const texts = [
      textBox(401.9, 70.7, 538.4, 88.2),
      textBox(402.2, 96.3, 492.5, 113.8),
      textBox(338.8, 63.7, 392.7, 81.2),
      textBox(285.4, 95.9, 377.1, 113.4),
    ];
    const [label] = placeLabels(
      [mark("p", 1, "GDG 75DX08 A-7 /256")],
      [anchor("p", 393.8, 92.1, 5)],
      orbit,
      reticleStrokesCssPx(1),
      null,
      { pinnedIds: ["p"], texts },
    );

    expect([label?.side, label?.leftPx]).toEqual(["right", 393.8 + 5 + 0.5 * 16 + 2]);
  });

  it.each([
    ["7.6 px, within their leading, keeps its side", 7.6, "right"],
    ["7.8 px, across their capitals, takes its other side", 7.8, "left"],
  ] as const)(
    "of the selection's label under other text whose box it overlaps by %s",
    (_, overlapPx, side) => {
      // `SOL-1`'s line box at (200, 150) runs from 141.25 to 158.75 px down; the ink band of each
      // box lies 0.275 em, 3.85 px, inside its top and bottom at 0.875 rem.
      const above = textBox(210, 141.25 + overlapPx - 17.5, 300, 141.25 + overlapPx);
      const [label] = placeLabels(
        [mark("a", 1, "SOL-1")],
        [anchor("a", 200, 150)],
        VIEWPORT,
        AT_TWO,
        null,
        { pinnedIds: ["a"], texts: [above] },
      );

      expect(label?.side).toBe(side);
    },
  );

  it("draws no label for the selection where both of its sides lie over other text", () => {
    const chosen = [mark("a", 1, "SOL-1")];
    const anchors = [anchor("a", 200, 150)];
    const texts = [textBox(230, 140, 300, 160), textBox(100, 140, 170, 160)];

    const [free] = placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null, { pinnedIds: ["a"] });

    expect([
      free?.side,
      placeLabels(chosen, anchors, VIEWPORT, AT_TWO, null, { pinnedIds: ["a"], texts }),
    ]).toEqual(["right", []]);
  });
});
