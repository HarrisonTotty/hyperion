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
  bracketArmPx,
  DESTINATION_LABEL_PLACES,
  type DestinationLabelPlace,
  destinationChevrons,
  type OffsetPx,
  placeIsRight,
  placeIsUpper,
  SIZE_CLASS_REM,
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
      ["picked", "a"],
    );

    expect(placeOf(label, at)).toBe("upper-left");
  });

  it("sends the destination's label to the upper left where furniture covers the upper right", () => {
    const strokes = reticleStrokesCssPx(1);
    const at = anchor("a", 100, 100);
    const furniture = { leftPx: 110, topPx: 50, widthPx: 40, heightPx: 20 };
    const [label] = placeLabels([mark("a", 1)], [at], VIEWPORT, strokes, "a", [], [furniture]);

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
    const unselected = placeLabels([mark("a", 1), mark("n", 0)], anchors, VIEWPORT, strokes, "a", [
      "a",
    ]);
    const selected = placeLabels([mark("n", 0), mark("a", 1)], anchors, VIEWPORT, strokes, "a", [
      "n",
      "a",
    ]);

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
    const placed = placeLabels(chosen, [anchor("far", -5, 66, 0), at], VIEWPORT, strokes, "a", [
      "a",
    ]);

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
