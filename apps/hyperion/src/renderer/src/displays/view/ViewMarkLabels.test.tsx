import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { contrastRatio, tokenLuminance } from "../../smoke/strokeContrast";
import { destinationSetReachPx, type ScreenBoxPx } from "../../spatial/symbols";
import { stylesheetRule } from "../../test/stylesheet";
import type { DrawAnchor } from "../../view/wireframe/drawList";
import {
  bracketHalfSizePx,
  destinationHalfSizePx,
  markLabelRisePx,
} from "../../view/wireframe/symbology";
import { type MarkRow, targetKey } from "./viewRun";
import {
  destinationLabelTransform,
  destinationSetReachCssPx,
  markLabelPlaces,
  type MarkLabelStage,
  type MarkLabelState,
  type MarkLabelSurroundings,
  markLabelTransform,
  PLATE_SIDE_PADDING_REM,
  type PlateSizePx,
  sideLabelTransform,
  ViewMarkLabels,
} from "./ViewMarkLabels";

/** A label's place, or `null` where it is hidden whole. */
type LabelPlace = MarkLabelState["place"];

const ANCHOR: DrawAnchor = {
  target: { kind: "craft", craft: "other" },
  xPx: 200,
  yPx: 100,
  distanceM: 5_000,
  label: { kind: "target", rangeM: 5_000, closureMPerS: null },
  labelOffsetPx: 24,
  labelRisePx: null,
  markReachPx: 12,
};

/** {@link ANCHOR} as the destination: its label stands 20 device px above or below its chevrons. */
const DESTINATION: DrawAnchor = { ...ANCHOR, labelRisePx: 20 };

/** The destination's label's key. */
const KEY = targetKey(DESTINATION.target);

/** Another craft's anchor at a place, its label shown or not, its reach as {@link ANCHOR}'s. */
function aCraftAt(craft: string, xPx: number, yPx: number, labelled = true): DrawAnchor {
  return {
    target: { kind: "craft", craft },
    xPx,
    yPx,
    distanceM: 5_000,
    label: labelled ? { kind: "target", rangeM: 5_000, closureMPerS: null } : null,
    labelOffsetPx: 24,
    labelRisePx: null,
    markReachPx: 12,
  };
}

/** Another craft, well clear of the destination. */
const NEIGHBOUR = aCraftAt("neighbour", 600, 400);

/** A stage of 800 × 600 CSS px at a ratio of 1 and a rem of 16 px: 0.5 rem is 8 px. */
const STAGE = { widthPx: 800, heightPx: 600, devicePixelRatio: 1, remPx: 16 } as const;

/** Each anchor's plate laid out 100 × 18 CSS px, as jsdom, which lays nothing out, cannot. */
function plates(...anchors: ReadonlyArray<DrawAnchor>): ReadonlyMap<string, PlateSizePx> {
  return new Map(
    anchors.map((anchor) => [targetKey(anchor.target), { widthPx: 100, heightPx: 18 }]),
  );
}

/** What a frame's labels are placed among, at the time 0 unless it says another. */
type Among = Partial<MarkLabelSurroundings>;

/** Each labelled anchor's label's place, `null` where it is hidden, in the anchors' order. */
function placesOf(
  anchors: ReadonlyArray<DrawAnchor>,
  laidOut: ReadonlyMap<string, PlateSizePx>,
  stage: MarkLabelStage,
  surroundings: Among = {},
): ReadonlyArray<LabelPlace | undefined> {
  const states = markLabelPlaces(anchors, laidOut, stage, { nowMs: 0, ...surroundings });
  return anchors.map((anchor) => states.get(targetKey(anchor.target))?.place);
}

/** The destination's label's place among `anchors`. */
function destinationPlace(
  anchors: ReadonlyArray<DrawAnchor>,
  laidOut: ReadonlyMap<string, PlateSizePx>,
  surroundings: Among = {},
): LabelPlace | undefined {
  return markLabelPlaces(anchors, laidOut, STAGE, { nowMs: 0, ...surroundings }).get(KEY)?.place;
}

function aRow(overrides: Partial<MarkRow> = {}): MarkRow {
  return {
    key: "craft:other",
    target: { kind: "craft", craft: "other" },
    name: "OTHER · TEST HULL",
    kind: "CRAFT",
    range: "5.00 km",
    unit: "km",
    fromCamera: false,
    closure: { kind: "known", text: "+3.40 m/s" },
    ...overrides,
  };
}

/** The label of `row`, found by its name: the labels are hidden from assistive technology. */
function labelOf(row: MarkRow): HTMLElement {
  render(<ViewMarkLabels anchors={[ANCHOR]} devicePixelRatio={1} rows={[row]} />);
  return screen.getByText(row.name, { exact: false });
}

describe("a mark's label", () => {
  it("reads the range and the closure rate", () => {
    expect(labelOf(aRow())).toHaveTextContent(/^OTHER · TEST HULL 5\.00 km \+3\.40 m\/s$/);
  });

  it("says FROM CAMERA, as the list does, where there is no own ship", () => {
    expect(labelOf(aRow({ fromCamera: true, closure: { kind: "none" } }))).toHaveTextContent(
      /^OTHER · TEST HULL 5\.00 km FROM CAMERA$/,
    );
  });

  it("shows a closure rate it does not have as a muted em dash", () => {
    const label = labelOf(aRow({ closure: { kind: "unknown" } }));
    expect([label.textContent, screen.getByText("—")]).toEqual([
      "OTHER · TEST HULL 5.00 km —",
      expect.objectContaining({ className: "readout__missing" }),
    ]);
  });

  it("hands each label to the drawing loop by its target's key, placed beside its mark", () => {
    const labelRef = vi.fn<(key: string, node: HTMLElement | null) => void>();
    const { unmount } = render(
      <ViewMarkLabels
        anchors={[ANCHOR]}
        devicePixelRatio={2}
        rows={[aRow()]}
        labelRef={labelRef}
      />,
    );
    expect([labelRef.mock.calls[0]?.[0], labelRef.mock.calls[0]?.[1]?.style.transform]).toEqual([
      "craft:other",
      "translate(112px, 50px)",
    ]);
    unmount();
    expect(labelRef).toHaveBeenLastCalledWith("craft:other", null);
  });

  it.each([
    ["a mark's", ANCHOR, "translate(224px, 100px)"],
    ["a destination's", DESTINATION, "translate(224px, calc(80px - 50%))"],
  ] as const)(
    "mounts %s label hidden until the drawing loop places it (R07.T16.i)",
    (_, anchor, transform) => {
      render(<ViewMarkLabels anchors={[anchor]} devicePixelRatio={1} rows={[aRow()]} />);
      const label = screen.getByText("OTHER · TEST HULL", { exact: false });
      expect([label.style.visibility, label.style.transform]).toEqual(["hidden", transform]);
    },
  );
});

describe("a label's place beside its mark (R07.T16.g; R07.T16.i)", () => {
  it.each([
    ["right", "translate(224px, 100px)", "translate(112px, 50px)"],
    ["left", "translate(calc(176px - 100%), 100px)", "translate(calc(88px - 100%), 50px)"],
    [
      "below",
      "translate(calc(200px - 50%), calc(124px + 0.25rem + 50%))",
      "translate(calc(100px - 50%), calc(62px + 0.25rem + 50%))",
    ],
    [
      "above",
      "translate(calc(200px - 50%), calc(76px - 0.25rem - 50%))",
      "translate(calc(100px - 50%), calc(38px - 0.25rem - 50%))",
    ],
  ] as const)(
    "stands its plate at the %s of its mark, the draw list's offset from it, in CSS px",
    (place, atOne, atTwo) => {
      expect([sideLabelTransform(ANCHOR, 1, place), sideLabelTransform(ANCHOR, 2, place)]).toEqual([
        atOne,
        atTwo,
      ]);
    },
  );

  it("centres its plate on its mark's line by the plate's own translate", () => {
    expect(stylesheetRule(".view-marks__label")).toContain("translate: 0 -50%;");
  });

  it("pads its plate 0.25 rem to the left and right alone, its text's clearance from the bracket", () => {
    // The draw list's tests read the text 0.25 rem inside the plate's near edge, and its bottom
    // edge as the text's (R07.T16.h's follow-up). Below and above its mark, the plate stands that
    // padding further out, so that its text is as far from the mark as at the right.
    expect(stylesheetRule(".view-marks__label")).toContain(
      `padding: 0 ${String(PLATE_SIDE_PADDING_REM)}rem;`,
    );
  });
});

// The plate's own `translate: 0 -50%` and the transform's −50% lift it by its height, so that its
// bottom edge stands at the transform's y; the transform's +50% undoes the plate's, so that its top
// edge does; and the transform's −100% moves a left-hand plate back by its width, so that its right
// edge stands at the transform's x.
describe("a destination's label above its chevron set (R07.T16.h's follow-up; decision-r07-quality-and-destination, addenda A to C)", () => {
  it.each([
    ["upper-right", "translate(224px, calc(80px - 50%))", "translate(112px, calc(40px - 50%))"],
    [
      "upper-left",
      "translate(calc(176px - 100%), calc(80px - 50%))",
      "translate(calc(88px - 100%), calc(40px - 50%))",
    ],
    ["lower-right", "translate(224px, calc(120px + 50%))", "translate(112px, calc(60px + 50%))"],
    [
      "lower-left",
      "translate(calc(176px - 100%), calc(120px + 50%))",
      "translate(calc(88px - 100%), calc(60px + 50%))",
    ],
  ] as const)(
    "stands its plate at the %s, its near edges the draw list's offset and rise from its mark",
    (place, atOne, atTwo) => {
      expect([
        destinationLabelTransform(DESTINATION, 1, place, 20),
        destinationLabelTransform(DESTINATION, 2, place, 20),
      ]).toEqual([atOne, atTwo]);
    },
  );

  it("gives a destination's place no transform on a mark that is no longer the destination", () => {
    const placed = { kind: "destination", place: "upper-left", changesMs: [] } as const;
    expect([
      markLabelTransform(DESTINATION, 1, placed),
      markLabelTransform(ANCHOR, 1, placed),
    ]).toEqual(["translate(calc(176px - 100%), calc(80px - 50%))", null]);
  });

  it("gives a hidden label no transform", () => {
    expect(markLabelTransform(ANCHOR, 1, { kind: "mark", place: null, changesMs: [] })).toBeNull();
  });

  it("takes the upper right where it is clear", () => {
    expect(destinationPlace([DESTINATION, NEIGHBOUR], plates(DESTINATION, NEIGHBOUR))).toBe(
      "upper-right",
    );
  });

  it("leaves every other label clear of it at its right", () => {
    expect(placesOf([DESTINATION, NEIGHBOUR], plates(DESTINATION, NEIGHBOUR), STAGE)[1]).toBe(
      "right",
    );
  });

  it("goes to the upper left where a neighbour's mark stands within 0.5 rem of the upper right", () => {
    // Its square reaches 62 px down, the upper-right plate's top; the upper left is 112 px off.
    const near = { ...NEIGHBOUR, xPx: 300, yPx: 50, label: null };
    expect(destinationPlace([DESTINATION, near], plates(DESTINATION))).toBe("upper-left");
  });

  it("goes to the upper left where a neighbour's label stands within 0.5 rem of the upper right", () => {
    // Its plate on its line, 216 to 316 px across and 41 to 59 px down, 3 px above the upper right.
    const near = { ...NEIGHBOUR, xPx: 192, yPx: 50, markReachPx: 0 };
    expect(destinationPlace([DESTINATION, near], plates(DESTINATION, near))).toBe("upper-left");
  });

  /** Four unlabelled marks, each touching one of the destination's places. */
  const around = [
    [274, 50],
    [126, 50],
    [274, 150],
    [126, 150],
  ].map(([xPx = 0, yPx = 0], i) => aCraftAt(`around-${String(i)}`, xPx, yPx, false));

  it("takes the first place inside the stage where none is 0.5 rem clear of every neighbour", () => {
    expect(destinationPlace([DESTINATION, ...around], plates(DESTINATION))).toBe("upper-right");
  });

  it("takes the first place inside the stage and 0.25 rem clear of the chrome where none is 0.5 rem clear", () => {
    // A box 2 px above the upper right, 54 px right of the upper left.
    const chrome = [{ leftPx: 230, topPx: 40, widthPx: 100, heightPx: 20 }];
    expect(destinationPlace([DESTINATION, ...around], plates(DESTINATION), { chrome })).toBe(
      "upper-left",
    );
  });

  it("is hidden whole where no place lies inside the stage, never shown past its edge", () => {
    // Its upper places' tops 3 px above the stage, its lower places' bottoms 3 px below it.
    const small = { widthPx: 140, heightPx: 70, devicePixelRatio: 1, remPx: 16 };
    const middle = { ...DESTINATION, xPx: 70, yPx: 35 };
    expect(markLabelPlaces([middle], plates(middle), small, { nowMs: 0 }).get(KEY)).toEqual({
      kind: "destination",
      place: null,
      changesMs: [],
    });
  });

  it("is hidden whole where every place inside the stage stands within 0.25 rem of the chrome", () => {
    const chrome = [{ leftPx: 0, topPx: 0, widthPx: 800, heightPx: 600 }];
    expect(destinationPlace([DESTINATION], plates(DESTINATION), { chrome })).toBeNull();
  });

  it("takes the upper left at the stage's right edge", () => {
    const right = { ...DESTINATION, xPx: 750 };
    expect(destinationPlace([right], plates(right))).toBe("upper-left");
  });

  it("takes the lower right at the stage's top edge", () => {
    const top = { ...DESTINATION, yPx: 30 };
    expect(destinationPlace([top], plates(top))).toBe("lower-right");
  });

  it("goes to the lower right where the chrome stands within 0.5 rem of both upper places", () => {
    // A label block down to 60 px, 2 px above the upper places' plates.
    const chrome = [{ leftPx: 0, topPx: 0, widthPx: 800, heightPx: 60 }];
    expect(destinationPlace([DESTINATION], plates(DESTINATION), { chrome })).toBe("lower-right");
  });

  it("is hidden until its plate is laid out", () => {
    expect(destinationPlace([DESTINATION], new Map())).toBeNull();
  });

  it("holds the square of its chevron set, its rise less 0.25 rem", () => {
    // A mark of 10 device px at a ratio of 2 and a rem of 16 CSS px, as the draw list places it.
    const risePx = markLabelRisePx(10, 32, 0, 5, 3);
    const reachPx = destinationSetReachPx(
      bracketHalfSizePx(10, 32, 0),
      destinationHalfSizePx(10, 32, 0, 5),
      3,
    );
    const stage = { ...STAGE, devicePixelRatio: 2 };
    expect(destinationSetReachCssPx(risePx, stage) - reachPx / 2).toBeCloseTo(0, 9);
  });
});

/** A craft whose label stands 13.25 px from it, a craft's at a ratio of 1 and 100%. */
function aMarkAt(craft: string, xPx: number, yPx: number, distanceM = 5_000): DrawAnchor {
  return { ...aCraftAt(craft, xPx, yPx), distanceM, labelOffsetPx: 13.25, markReachPx: 11.75 };
}

/** Each anchor's plate as `TEST PLANET`'s laid out at 100%, 108 × 18 CSS px. */
function planetPlates(...anchors: ReadonlyArray<DrawAnchor>): ReadonlyMap<string, PlateSizePx> {
  return new Map(
    anchors.map((anchor) => [targetKey(anchor.target), { widthPx: 108, heightPx: 18 }]),
  );
}

/**
 * The compact layout's stage at 1280 × 720 and 100%, and its label block, as T16.i's hidden
 * captures measured them in the wireframe (`page/run-compact720.log`): the block's foot 252 px
 * down, its right edge 492.27 px across.
 */
const COMPACT_STAGE = { widthPx: 816, heightPx: 540, devicePixelRatio: 1, remPx: 16 } as const;
const COMPACT_BLOCK: ScreenBoxPx = { leftPx: 8, topPx: 8, widthPx: 484.27, heightPx: 244 };

/** The full layout's stage at 1920 × 1080, and an open instrument slot at its top right. */
const FULL_STAGE = { widthPx: 1280, heightPx: 720, devicePixelRatio: 1, remPx: 16 } as const;
const SLOT: ScreenBoxPx = { leftPx: 717, topPx: 8, widthPx: 555, heightPx: 254 };

/** Two marks whose labels meet at their right: the nearer at 1 km, the other 5 px lower. */
const nearer = aMarkAt("nearer", 400, 300, 1_000);
const farther = aMarkAt("farther", 420, 305, 2_000);

/** One mark's label's place in the compact layout, by the block. */
function compactPlace(xPx: number, yPx: number): LabelPlace | undefined {
  const mark = aMarkAt("planet", xPx, yPx);
  return placesOf([mark], planetPlates(mark), COMPACT_STAGE, { chrome: [COMPACT_BLOCK] })[0];
}

/** One mark's label's place in the full layout, by the slot. */
function slotPlace(xPx: number, yPx: number): LabelPlace | undefined {
  const mark = aMarkAt("planet", xPx, yPx);
  return placesOf([mark], planetPlates(mark), FULL_STAGE, { chrome: [SLOT] })[0];
}

describe("a label clear of the view's chrome, its edges and other labels (R07.T16.i; decision-r07-quality-and-destination, Q6 (a) and addendum C)", () => {
  it("in the compact layout, takes a label below the label block where its mark stands at the block's foot", () => {
    // Its plate on its line, at the right or the left, runs under the block.
    expect(compactPlace(405, 250)).toBe("below");
  });

  it("in the compact layout, keeps the right for a mark under the label block's right edge, past the block", () => {
    // Its plate starts 6.98 px right of the block.
    expect(compactPlace(486, 120)).toBe("right");
  });

  it("in the compact layout, hides the label whole of a mark wholly under the label block", () => {
    // As TEST PLANET's from CHASE: every place lies under the block.
    expect(compactPlace(398, 157)).toBeNull();
  });

  it("takes its label to the left of a mark under an instrument slot's left edge", () => {
    expect(slotPlace(720, 100)).toBe("left");
  });

  it("hides the label whole of a mark wholly under an instrument slot", () => {
    expect(slotPlace(1000, 100)).toBeNull();
  });

  it.each([
    ["the stage's right edge", 1275, 400, "left"],
    ["the stage's foot", 640, 712, "above"],
    ["the stage's top right corner", 1278, 2, null],
  ] as const)("never stands past %s", (_, xPx, yPx, place) => {
    const mark = aMarkAt("planet", xPx, yPx);
    expect(placesOf([mark], planetPlates(mark), FULL_STAGE)[0]).toBe(place);
  });

  it("stands no plate within 0.25 rem of the label block", () => {
    // Its plate at the right and the left 3.5 px under the block's foot, then 4.5 px.
    expect([compactPlace(300, 264.5), compactPlace(300, 265.5)]).toEqual(["below", "right"]);
  });

  it("stands no plate within 0.25 rem of an instrument slot", () => {
    // Its plate at the right 3.75 px left of the slot, then 5.75 px.
    expect([slotPlace(592, 100), slotPlace(590, 100)]).toEqual(["left", "right"]);
  });

  it("places the nearer label first, and takes the other's to its next place, never under it", () => {
    expect(placesOf([farther, nearer], planetPlates(nearer, farther), FULL_STAGE)).toEqual([
      "left",
      "right",
    ]);
  });

  it("places the selection's label before a neighbour's, which takes its next place", () => {
    expect(
      placesOf([farther, nearer], planetPlates(nearer, farther), FULL_STAGE, {
        selection: targetKey(farther.target),
      }),
    ).toEqual(["right", "left"]);
  });

  it("moves no label when a mark is selected", () => {
    const laidOut = planetPlates(nearer, farther);
    const before = markLabelPlaces([farther, nearer], laidOut, FULL_STAGE, { nowMs: 0 });
    expect(
      placesOf([farther, nearer], laidOut, FULL_STAGE, {
        selection: targetKey(farther.target),
        previous: before,
      }),
    ).toEqual(["left", "right"]);
  });

  /**
   * The destination's upper-right plate, 224 to 324 px across and 62 to 80 down, and a mark 454 px
   * across whose plate at its right runs under a box over the stage, and whose plate at its left
   * stands 6 px right of the destination's.
   */
  const beside = aCraftAt("beside", 454, 71);
  const overBeside: ScreenBoxPx = { leftPx: 470, topPx: 50, widthPx: 200, heightPx: 40 };

  it("takes another label to its next place where it stands within 0.5 rem of the destination's plate", () => {
    expect(
      placesOf([DESTINATION, beside], plates(DESTINATION, beside), STAGE, {
        chrome: [overBeside],
      }),
    ).toEqual(["upper-right", "below"]);
  });

  it("hides another label whole where every place it has left stands too near", () => {
    // Above it and below it, the chrome as well.
    const chrome = [
      overBeside,
      { leftPx: 380, topPx: 0, widthPx: 200, heightPx: 50 },
      { leftPx: 380, topPx: 95, widthPx: 200, heightPx: 30 },
    ];
    expect(
      placesOf([DESTINATION, beside], plates(DESTINATION, beside), STAGE, { chrome })[1],
    ).toBeNull();
  });

  it("never moves the destination's label for the selection's, which yields to it", () => {
    const laidOut = plates(DESTINATION, beside);
    const surroundings = { chrome: [overBeside] };
    expect([
      placesOf([DESTINATION, beside], laidOut, STAGE, surroundings),
      placesOf([DESTINATION, beside], laidOut, STAGE, {
        ...surroundings,
        selection: targetKey(beside.target),
      }),
    ]).toEqual([
      ["upper-right", "below"],
      ["upper-right", "below"],
    ]);
  });

  it("takes another label to its next place within 0.25 rem of the destination's chevron set", () => {
    // The set's square, 184 to 216 px across and 84 to 116 down, the destination's label hidden;
    // a plate at the right 3 px above it, then 5 px.
    const near = aCraftAt("near", 100, 72);
    const clear = aCraftAt("clear", 100, 70);
    expect([
      placesOf([DESTINATION, near], plates(near), STAGE)[1],
      placesOf([DESTINATION, clear], plates(clear), STAGE)[1],
    ]).toEqual(["below", "right"]);
  });

  it("hides a label whose plate is not laid out", () => {
    expect(placesOf([ANCHOR], new Map(), STAGE)).toEqual([null]);
  });
});

/**
 * A label's places over frames `stepMs` apart, each from the state the frame before left: a second
 * apart by default, so that no label reaches its limit of changes.
 */
function framesOf(
  marks: ReadonlyArray<DrawAnchor>,
  stage: MarkLabelStage,
  chrome: ReadonlyArray<ScreenBoxPx>,
  stepMs = 1_000,
): ReadonlyArray<LabelPlace | undefined> {
  let previous: ReadonlyMap<string, MarkLabelState> = new Map();
  return marks.map((mark, frame) => {
    previous = markLabelPlaces([mark], planetPlates(mark), stage, {
      chrome,
      previous,
      nowMs: frame * stepMs,
    });
    return previous.get(targetKey(mark.target))?.place;
  });
}

describe("a label's hysteresis (R07.T16.i)", () => {
  it("leaves its label where it is as its mark moves 0.2 rem back and forth across an obstacle's edge", () => {
    // At 593.35 px its plate at the right ends 2.4 px from the slot; at 590.15 px, 5.6 px.
    const xs = [593.35, 590.15, 593.35, 590.15, 593.35, 590.15];
    expect(
      framesOf(
        xs.map((x) => aMarkAt("planet", x, 100)),
        FULL_STAGE,
        [SLOT],
      ),
    ).toEqual(xs.map(() => "left"));
  });

  it("moves its label back once the place it moves to is clear by 0.25 rem", () => {
    // At 586 px its plate at the right ends 9.75 px from the slot.
    const marks = [593.35, 590.15, 586].map((x) => aMarkAt("planet", x, 100));
    expect(framesOf(marks, FULL_STAGE, [SLOT])).toEqual(["left", "left", "right"]);
  });

  it("keeps a hidden label hidden as its mark moves 0.2 rem back and forth, until a place is clear by 0.25 rem", () => {
    // Under the block, its plate at the right 2.4, 5.6, then 8.25 px right of the block's edge.
    const xs = [481.42, 484.62, 481.42, 484.62, 487.27];
    expect(
      framesOf(
        xs.map((x) => aMarkAt("planet", x, 120)),
        COMPACT_STAGE,
        [COMPACT_BLOCK],
      ),
    ).toEqual([null, null, null, null, "right"]);
  });

  it("leaves a place that no longer fits in the frame it stops fitting", () => {
    // Its plate at the right 5.75 px from the slot, then over it.
    const marks = [590, 600].map((x) => aMarkAt("planet", x, 100));
    expect(framesOf(marks, FULL_STAGE, [SLOT])).toEqual(["right", "left"]);
  });

  it("moves a label at once when its mark is reported as the destination, and when it no longer is", () => {
    const line = markLabelPlaces([ANCHOR], plates(ANCHOR), STAGE, { nowMs: 0 });
    const raised = markLabelPlaces([DESTINATION], plates(DESTINATION), STAGE, {
      previous: line,
      nowMs: 1_000,
    });
    const back = markLabelPlaces([ANCHOR], plates(ANCHOR), STAGE, {
      previous: raised,
      nowMs: 2_000,
    });
    expect([line, raised, back].map((states) => states.get(KEY))).toEqual([
      { kind: "mark", place: "right", changesMs: [0] },
      { kind: "destination", place: "upper-right", changesMs: [1_000] },
      { kind: "mark", place: "right", changesMs: [2_000] },
    ]);
  });

  it("takes a farther label's place where its own no longer fits, the farther label giving way", () => {
    // The nearer at its right and the farther at its left, then a box over the nearer's right.
    const laidOut = planetPlates(nearer, farther);
    const before = markLabelPlaces([farther, nearer], laidOut, FULL_STAGE, { nowMs: 0 });
    const chrome = [{ leftPx: 410, topPx: 285, widthPx: 120, heightPx: 28 }];
    expect([
      placesOf([farther, nearer], laidOut, FULL_STAGE),
      placesOf([farther, nearer], laidOut, FULL_STAGE, { chrome, previous: before, nowMs: 1_000 }),
    ]).toEqual([
      ["left", "right"],
      ["below", "left"],
    ]);
  });

  it("changes a label at most three times in a second, the last of them hiding it whole where its place stops fitting", () => {
    // Every 100 ms its mark swings between 586 px, where only its right fits, and 600 px, where only
    // its left does, between a box at the left and the slot.
    const left: ScreenBoxPx = { leftPx: 8, topPx: 8, widthPx: 462, heightPx: 254 };
    const marks = Array.from({ length: 15 }, (_, frame) =>
      aMarkAt("planet", frame % 2 === 0 ? 586 : 600, 100),
    );
    expect(framesOf(marks, FULL_STAGE, [left, SLOT], 100)).toEqual([
      "right",
      "left",
      ...Array.from({ length: 9 }, () => null),
      "left",
      "right",
      null,
      null,
    ]);
  });
});

/** A colour token's value, as the stylesheet's `:root` rule declares it. */
function tokenOf(name: string): string {
  return (
    new RegExp("--" + name + ": (#[0-9a-f]{6});", "u").exec(stylesheetRule(":root"))?.[1] ?? ""
  );
}

// The plate over a neighbouring mark is checked in the stylesheet, not in pixels: the DOM over the
// canvas is not captured by `just test-render`, so what of the covered mark shows beside the plate's
// edge is a stated limit, looked at in the by-hand page captures (R07.T16.g, T16.f).
describe("a label's plate over a neighbouring mark (R07.T16.g; decision-r07-t16d-followups)", () => {
  it("is opaque --surface-0 where it lies over a mark that crowds its own", () => {
    // A second craft 2 rem to the right of the first, at a ratio of 1: beyond the first one's
    // plate's near edge, under the plate, which runs on for the label's text.
    const neighbour: DrawAnchor = {
      ...ANCHOR,
      target: { kind: "craft", craft: "neighbour" },
      xPx: ANCHOR.xPx + 32,
    };
    render(
      <ViewMarkLabels
        anchors={[ANCHOR, neighbour]}
        devicePixelRatio={1}
        rows={[aRow(), aRow({ key: "craft:neighbour", name: "NEIGHBOUR · TEST HULL" })]}
      />,
    );
    const plate = screen.getByText("OTHER · TEST HULL", { exact: false });
    const rules = [stylesheetRule(".view-marks"), stylesheetRule(".view-marks__label")].join("\n");
    expect({
      over: neighbour.xPx > ANCHOR.xPx + ANCHOR.labelOffsetPx,
      plated: plate.classList.contains("view-marks__label"),
      paints: stylesheetRule(".view-marks__label").includes("background: var(--surface-0);"),
      surface: /^#[0-9a-f]{6}$/u.test(tokenOf("surface-0")),
      seeThrough: /opacity|filter|mix-blend-mode|rgba|hsla|transparent|color-mix/u.test(rules),
    }).toEqual({ over: true, plated: true, paints: true, surface: true, seeThrough: false });
  });

  it("holds its text and its stale readings at 6:1 against the plate", () => {
    const surface = tokenLuminance(tokenOf("surface-0"));
    expect(
      ["text", "text-muted"].map(
        (token) => contrastRatio(tokenLuminance(tokenOf(token)), surface) >= 6,
      ),
    ).toEqual([true, true]);
  });

  it("cuts to its place, with no transition or animation", () => {
    // An eased move would carry the plate over a reported destination's reticle for up to 150 ms.
    expect(/transition|animation/u.test(stylesheetRule(".view-marks__label"))).toBe(false);
  });
});
