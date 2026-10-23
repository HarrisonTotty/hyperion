import { describe, expect, it } from "vitest";

import { minReticleGapDevicePx, reticleStrokesCssPx } from "../lib/strokes";
import { type CameraAngles, PRESETS, project, viewBasis, type Viewport } from "./camera";
import {
  type Anchor,
  buildDrawList,
  type CircleLabel,
  type CurveLabel,
  type DrawOp,
  type RingLabel,
  type ScreenPoint,
} from "./drawList";
import { type LocalFrame, localFrameAt, planeFrame } from "../geometry/frame";
import {
  type BoxPx,
  boxesOverlap,
  CORE_HEAD_REM,
  coreArrowBoxes,
  type CoreArrowLayout,
  coreArrowLayout,
  coreLabelText,
  curveInksPx,
  type InkPx,
  inkGapPx,
  markInksPx,
  type AnnulusLabelSurroundings,
  type PlacedCurveLabel,
  placeAnnulusLabels,
  placeCurveLabels,
  segmentBoxGapPx,
  tickInksPx,
  TRIAD_BOX_REM,
  TRIAD_HEAD_REM,
  TRIAD_MARKER_REM,
  type TriadAxis,
  triadBoxRem,
  triadFootprintPx,
  triadLayout,
  triadShiftRem,
} from "./furniture";
import { bracketBoxPx, textSizeRem } from "./labels";
import type { SpatialScene } from "./marks";
import { boxGapPx } from "./symbols";
import { add, scale, vec3 } from "../geometry/vec3";

const FRAME = localFrameAt(vec3(26_000, 0, 0));
/** A chart 22.5 rem wide, as the local chart's column is at 1280 × 720. */
const NARROW: Viewport = { widthPx: 360, heightPx: 300, remPx: 16 };
const LABEL = coreLabelText({ value: "26,000.0", unit: "ly" });

/** Whether `box` lies wholly inside `outer`. */
function within(box: BoxPx, outer: BoxPx): boolean {
  return (
    box.leftPx >= outer.leftPx &&
    box.topPx >= outer.topPx &&
    box.leftPx + box.widthPx <= outer.leftPx + outer.widthPx &&
    box.topPx + box.heightPx <= outer.topPx + outer.heightPx
  );
}

function insideView(box: BoxPx, viewport: Viewport): boolean {
  return within(box, {
    leftPx: 0,
    topPx: 0,
    widthPx: viewport.widthPx,
    heightPx: viewport.heightPx,
  });
}

function triadBox(angles: CameraAngles, viewport = NARROW): BoxPx {
  return triadFootprintPx(triadLayout(FRAME, angles), viewport);
}

/** Every camera direction on a 15° grid. */
function everyAngle(): CameraAngles[] {
  const angles: CameraAngles[] = [];
  for (let azimuthDeg = 0; azimuthDeg < 360; azimuthDeg += 15) {
    for (let elevationDeg = -90; elevationDeg <= 90; elevationDeg += 15) {
      angles.push({ azimuthDeg, elevationDeg });
    }
  }
  return angles;
}

describe("triadBoxRem", () => {
  it("keeps its usual box on a view with the room for it", () => {
    expect(triadBoxRem({ widthPx: 1_036, heightPx: 628, remPx: 16 })).toEqual(TRIAD_BOX_REM);
  });

  it("gives a short view a box no taller than the view", () => {
    // The local chart's stage at 1280 × 688 with the census table shown.
    expect(triadBoxRem({ widthPx: 798, heightPx: 74, remPx: 16 })).toEqual({
      width: 15,
      height: 4.625,
    });
  });

  it("follows the interface scale", () => {
    expect(triadBoxRem({ widthPx: 798, heightPx: 170, remPx: 24 })).toEqual({
      width: 15,
      height: 170 / 24,
    });
  });
});

describe("triadLayout", () => {
  it("keeps every axis and label inside a box shorter than the usual one, at every angle", () => {
    // The overlay clips what leaves it, so a short stage must shorten the triad, not cut it off.
    const shortBox = { width: 15, height: 4.625 };
    const inBox: BoxPx = {
      leftPx: -shortBox.width / 2,
      topPx: -shortBox.height / 2,
      widthPx: shortBox.width,
      heightPx: shortBox.height,
    };
    const failures: string[] = [];

    for (const angles of everyAngle()) {
      const axes = triadLayout(FRAME, angles, shortBox);
      const clear = axes.every(
        (axis) =>
          within(
            {
              leftPx: axis.labelAt.x - axis.labelSize.widthRem / 2,
              topPx: axis.labelAt.y - axis.labelSize.heightRem / 2,
              widthPx: axis.labelSize.widthRem,
              heightPx: axis.labelSize.heightRem,
            },
            inBox,
          ) &&
          within(
            { leftPx: axis.tip.x - 0.25, topPx: axis.tip.y - 0.25, widthPx: 0.5, heightPx: 0.5 },
            inBox,
          ),
      );
      if (!clear) {
        failures.push(`${angles.azimuthDeg}°, ${angles.elevationDeg}°`);
      }
    }

    expect(failures).toEqual([]);
  });

  it("keeps the labels from covering each other, and inside the triad's box", () => {
    const failures: string[] = [];
    const inBox: BoxPx = { leftPx: -7.5, topPx: -4, widthPx: 15, heightPx: 8 };

    for (const angles of everyAngle()) {
      const boxes = triadLayout(FRAME, angles).map((axis) => ({
        leftPx: axis.labelAt.x - axis.labelSize.widthRem / 2,
        topPx: axis.labelAt.y - axis.labelSize.heightRem / 2,
        widthPx: axis.labelSize.widthRem,
        heightPx: axis.labelSize.heightRem,
      }));
      const clear = boxes.every(
        (box, index) =>
          within(box, inBox) && boxes.slice(index + 1).every((other) => !boxesOverlap(box, other)),
      );
      if (!clear) {
        failures.push(`${angles.azimuthDeg}°, ${angles.elevationDeg}°`);
      }
    }

    expect(failures).toEqual([]);
  });

  it("sizes each label as the stylesheet sets it: 0.875 rem, spaced 0.1 em", () => {
    const [coreward] = triadLayout(FRAME, PRESETS.oblique);

    expect(coreward?.labelSize).toEqual(textSizeRem("COREWARD", 0.1));
  });
});

/** A triad label's box, in `rem` as px at 1 px a rem. */
function labelBoxRem(axis: TriadAxis): BoxPx {
  return {
    leftPx: axis.labelAt.x - axis.labelSize.widthRem / 2,
    topPx: axis.labelAt.y - axis.labelSize.heightRem / 2,
    widthPx: axis.labelSize.widthRem,
    heightPx: axis.labelSize.heightRem,
  };
}

function remPoint(x: number, y: number): ScreenPoint {
  return { xPx: x, yPx: y };
}

/**
 * An axis's end symbol as `AxisTriad` draws it, its path in `rem`: the away or towards circle of
 * radius `TRIAD_MARKER_REM` + δ, taken as a disc, or the arrowhead's two barbs.
 */
function endSymbolPath(axis: TriadAxis, shiftRem: number): InkPx[] {
  const tip = remPoint(axis.tip.x, axis.tip.y);
  if (axis.end !== "across") {
    return [{ from: tip, to: tip, halfWidthPx: TRIAD_MARKER_REM + shiftRem }];
  }
  const heading = Math.atan2(axis.tip.y, axis.tip.x);
  return [1, -1].map((sign) => {
    const angle = heading + Math.PI + (sign * Math.PI) / 6;
    return {
      from: tip,
      to: remPoint(
        axis.tip.x + TRIAD_HEAD_REM * Math.cos(angle),
        axis.tip.y + TRIAD_HEAD_REM * Math.sin(angle),
      ),
      halfWidthPx: 0,
    };
  });
}

/** An axis's line from the origin as `AxisTriad` draws it, its path in `rem`, to its circle or tip. */
function axisLinePath(axis: TriadAxis, shiftRem: number): InkPx[] {
  const reach = Math.hypot(axis.tip.x, axis.tip.y);
  const lineReach = axis.end === "across" ? reach : reach - (TRIAD_MARKER_REM + shiftRem);
  if (!(lineReach > 0)) {
    return [];
  }
  return [
    {
      from: remPoint(0, 0),
      to: remPoint((axis.tip.x / reach) * lineReach, (axis.tip.y / reach) * lineReach),
      halfWidthPx: 0,
    },
  ];
}

/** Each label's least gap to every axis's paths of one kind, its own included, in `rem`. */
function gapsToAxes(
  axes: ReadonlyArray<TriadAxis>,
  shiftRem: number,
  pathsOf: (axis: TriadAxis, shiftRem: number) => InkPx[],
): number[] {
  return axes.map((axis) =>
    Math.min(
      ...axes
        .flatMap((other) => pathsOf(other, shiftRem))
        .map((path) => inkGapPx(labelBoxRem(axis), path)),
    ),
  );
}

/**
 * The outline shifts the triad is tested at: the projector's 0.78125 at 100% and 80%, a ratio of 1
 * at 100%, and 2, where nothing moves out.
 */
const TRIAD_SHIFTS: ReadonlyArray<readonly [string, number]> = [
  ["0.78125 at 100%", triadShiftRem(0.78125, 16)],
  ["0.78125 at 80%", triadShiftRem(0.78125, 12.8)],
  ["1 at 100%", triadShiftRem(1, 16)],
  ["2 at 100%", triadShiftRem(2, 16)],
];

/** What a triad's labels clear by: 0.125 rem, to within rounding. */
const TRIAD_CLEAR_REM = 0.125 - 1e-9;

/**
 * A triad seen from `FRONT`, whose screen is spinward's normal: `shownCoreward` drawn at `angleDeg`
 * above the right, away from the viewer or across the screen, and spinward and north drawn down
 * and to the left, out of its label's way.
 */
function triadWithCorewardAt(angleDeg: number, end: "away" | "across"): ReadonlyArray<TriadAxis> {
  const basis = { right: scale(FRAME.coreward, -1), up: FRAME.north, forward: FRAME.spinward };
  const angle = (angleDeg * Math.PI) / 180;
  const shown: LocalFrame = {
    coreward: add(
      add(scale(basis.right, Math.cos(angle)), scale(basis.up, Math.sin(angle))),
      scale(basis.forward, end === "away" ? 0.5 : 0),
    ),
    spinward: scale(basis.up, -1),
    north: scale(basis.right, -1),
    onAxis: false,
  };
  return triadLayout(FRAME, PRESETS.front, TRIAD_BOX_REM, shown);
}

/** One axis of the tilted local chart's triad, from `OBLIQUE`, at an outline shift. */
function obliqueAxis(name: TriadAxis["name"], shiftRem: number): TriadAxis {
  const axis = triadLayout(FRAME, PRESETS.oblique, TRIAD_BOX_REM, FRAME, shiftRem).find(
    (candidate) => candidate.name === name,
  );
  if (axis === undefined) {
    throw new Error(`the triad has no ${name} axis`);
  }
  return axis;
}

describe("triadLayout's labels clear of every axis (R07.T16.j)", () => {
  it.each(TRIAD_SHIFTS)(
    "stands COREWARD 0.125 rem clear of NORTH's symbol on the tilted local chart, at a ratio of %s",
    (_, shiftRem) => {
      const axes = triadLayout(FRAME, PRESETS.oblique, TRIAD_BOX_REM, FRAME, shiftRem);
      const coreward = axes.find((axis) => axis.name === "coreward");
      const north = axes.find((axis) => axis.name === "north");
      if (coreward === undefined || north === undefined) {
        throw new Error("the triad has no coreward or north axis");
      }

      const gaps = endSymbolPath(north, shiftRem).map((path) =>
        inkGapPx(labelBoxRem(coreward), path),
      );

      expect(Math.min(...gaps)).toBeGreaterThanOrEqual(TRIAD_CLEAR_REM);
    },
  );

  it.each(TRIAD_SHIFTS)(
    "stands COREWARD left of its own circle on the tilted local chart, at a ratio of %s",
    (_, shiftRem) => {
      const coreward = obliqueAxis("coreward", shiftRem);
      const box = labelBoxRem(coreward);

      expect(box.leftPx + box.widthPx).toBeLessThan(coreward.tip.x - TRIAD_MARKER_REM - shiftRem);
    },
  );

  it.each(TRIAD_SHIFTS)(
    "stands NORTH straight above its tip on the tilted local chart, at a ratio of %s",
    (_, shiftRem) => {
      const north = obliqueAxis("north", shiftRem);
      const box = labelBoxRem(north);

      expect([north.labelAt.x - north.tip.x, box.topPx + box.heightPx < north.tip.y]).toEqual([
        expect.closeTo(0, 9),
        true,
      ]);
    },
  );

  it.each(TRIAD_SHIFTS)(
    "keeps NORTH from stacking over SPINWARD on the tilted local chart, sharing no column with it, at a ratio of %s",
    (_, shiftRem) => {
      const north = labelBoxRem(obliqueAxis("north", shiftRem));
      const spinward = labelBoxRem(obliqueAxis("spinward", shiftRem));

      expect(
        north.leftPx + north.widthPx <= spinward.leftPx ||
          spinward.leftPx + spinward.widthPx <= north.leftPx,
      ).toBe(true);
    },
  );

  it.each(TRIAD_SHIFTS)(
    "stands each label of the tilted local chart's triad nearer its own tip than any other axis's, at a ratio of %s",
    (_, shiftRem) => {
      const axes = triadLayout(FRAME, PRESETS.oblique, TRIAD_BOX_REM, FRAME, shiftRem);
      const tipGap = (axis: TriadAxis, of: TriadAxis): number =>
        segmentBoxGapPx(
          remPoint(of.tip.x, of.tip.y),
          remPoint(of.tip.x, of.tip.y),
          labelBoxRem(axis),
        );

      expect(
        axes.flatMap((axis) =>
          axes
            .filter((other) => other !== axis && tipGap(axis, other) <= tipGap(axis, axis))
            .map((other) => `${axis.name} nearer ${other.name}'s tip`),
        ),
      ).toEqual([]);
    },
  );

  it.each(TRIAD_SHIFTS)(
    "stands no label of the tilted local chart's triad within 0.125 rem of any axis's symbol or line, its own included, at a ratio of %s",
    (_, shiftRem) => {
      const axes = triadLayout(FRAME, PRESETS.oblique, TRIAD_BOX_REM, FRAME, shiftRem);

      expect(
        [endSymbolPath, axisLinePath].map((pathsOf) =>
          gapsToAxes(axes, shiftRem, pathsOf).every((gap) => gap >= TRIAD_CLEAR_REM),
        ),
      ).toEqual([true, true]);
    },
  );

  it.each(TRIAD_SHIFTS)(
    "stands every label 0.125 rem clear of every axis's symbol and line and of the labels before it, from every direction, at a ratio of %s",
    (_, shiftRem) => {
      const failures: string[] = [];

      for (const angles of everyAngle()) {
        const axes = triadLayout(FRAME, angles, TRIAD_BOX_REM, FRAME, shiftRem);
        const boxes = axes.map(labelBoxRem);
        const apart = boxes.every((box, index) =>
          boxes.slice(0, index).every((other) => boxGapPx(box, other) >= TRIAD_CLEAR_REM),
        );
        const clear = [endSymbolPath, axisLinePath].every((pathsOf) =>
          gapsToAxes(axes, shiftRem, pathsOf).every((gap) => gap >= TRIAD_CLEAR_REM),
        );
        if (!apart || !clear) {
          failures.push(`${angles.azimuthDeg}°, ${angles.elevationDeg}°`);
        }
      }

      expect(failures).toEqual([]);
    },
  );

  it.each([
    ["the usual box", TRIAD_BOX_REM],
    ["a stage 12 rem wide and 4.625 rem tall", { width: 12, height: 4.625 }],
  ] as const)(
    "holds every label 0.25 rem inside the stage's left and bottom edges, off the focus ring, from every direction, in %s",
    (_, boxRem) => {
      const failures = everyAngle().filter((angles) =>
        triadLayout(FRAME, angles, boxRem).some((axis) => {
          const box = labelBoxRem(axis);
          return (
            box.leftPx < -boxRem.width / 2 + 0.25 - 1e-9 ||
            box.topPx + box.heightPx > boxRem.height / 2 - 0.25 + 1e-9
          );
        }),
      );

      expect(failures).toEqual([]);
    },
  );

  it.each([15, 21, 30, 45, 70])(
    "stands an away end's unobstructed label 0.5 rem from its tip by true distance, at %s° from the horizontal",
    (angleDeg) => {
      const coreward = triadWithCorewardAt(angleDeg, "away").find(
        (axis) => axis.name === "coreward",
      );
      if (coreward === undefined) {
        throw new Error("the triad has no coreward axis");
      }

      expect(coreward.end).toBe("away");
      expect(
        segmentBoxGapPx(
          remPoint(coreward.tip.x, coreward.tip.y),
          remPoint(coreward.tip.x, coreward.tip.y),
          labelBoxRem(coreward),
        ),
      ).toBeCloseTo(0.5, 9);
    },
  );

  it.each([15, 21, 30, 45, 70])(
    "stands an arrowhead's unobstructed label 0.25 rem from its tip by true distance, at %s° from the horizontal",
    (angleDeg) => {
      const coreward = triadWithCorewardAt(angleDeg, "across").find(
        (axis) => axis.name === "coreward",
      );
      if (coreward === undefined) {
        throw new Error("the triad has no coreward axis");
      }

      expect(coreward.end).toBe("across");
      expect(
        segmentBoxGapPx(
          remPoint(coreward.tip.x, coreward.tip.y),
          remPoint(coreward.tip.x, coreward.tip.y),
          labelBoxRem(coreward),
        ),
      ).toBeCloseTo(0.25, 9);
    },
  );

  it("takes a place across a line, clear of the symbols and the labels, where none clears the lines too", () => {
    // A box too narrow for the triad, 10 rem wide, as a stage 160 px wide gives at 100%: SPINWARD,
    // from 25° and -75°, has no place inside it that is off every line as well.
    const axes = triadLayout(
      FRAME,
      { azimuthDeg: 25, elevationDeg: -75 },
      { width: 10, height: 8 },
    );
    const spinward = axes.findIndex((axis) => axis.name === "spinward");
    const boxes = axes.map(labelBoxRem);

    expect([
      (gapsToAxes(axes, 0, endSymbolPath)[spinward] ?? 0) >= TRIAD_CLEAR_REM,
      boxes.every(
        (box, index) =>
          index === spinward || boxGapPx(box, boxes[spinward] ?? box) >= TRIAD_CLEAR_REM,
      ),
      (gapsToAxes(axes, 0, axisLinePath)[spinward] ?? 1) < TRIAD_CLEAR_REM,
    ]).toEqual([true, true, true]);
  });

  it("keeps a label at its first place, held inside the box, where no place clears the symbols, rather than drop it", () => {
    // In a box 10 rem wide, SPINWARD runs across the screen to the right from 0° and -25°. Every
    // place inside the box stands on an arrowhead, so the label stands at its first place, beyond
    // its tip, held 5 rem less half its width from the box's centre.
    const axes = triadLayout(FRAME, { azimuthDeg: 0, elevationDeg: -25 }, { width: 10, height: 8 });
    const spinward = axes.find((axis) => axis.name === "spinward");
    if (spinward === undefined) {
      throw new Error("the triad has no spinward axis");
    }

    expect([
      spinward.end,
      spinward.labelAt.x,
      spinward.labelAt.y,
      Math.min(...endSymbolPath(spinward, 0).map((path) => inkGapPx(labelBoxRem(spinward), path))) <
        TRIAD_CLEAR_REM,
    ]).toEqual([
      "across",
      expect.closeTo(5 - spinward.labelSize.widthRem / 2, 9),
      expect.closeTo(0, 9),
      true,
    ]);
  });
});

describe("triadFootprintPx", () => {
  it("lies inside the triad's box in the view's bottom left-hand corner", () => {
    const corner: BoxPx = {
      leftPx: 0,
      topPx: NARROW.heightPx - 8 * 16,
      widthPx: 15 * 16,
      heightPx: 8 * 16,
    };
    const failures = everyAngle()
      .filter((angles) => !within(triadBox(angles), corner))
      .map((angles) => `${angles.azimuthDeg}°, ${angles.elevationDeg}°`);

    expect(failures).toEqual([]);
  });
});

describe("coreArrowLayout", () => {
  it("stops the arrow short of the triad when coreward runs down into it", () => {
    // From the top turned to 180°, coreward points down the screen, towards the bottom edge.
    const angles = { azimuthDeg: 180, elevationDeg: 90 };
    const triad = triadBox(angles);

    const layout = coreArrowLayout(FRAME, angles, NARROW, LABEL, [triad]);

    expect(layout.kind).toBe("arrow");
    for (const box of coreArrowBoxes(layout, NARROW.remPx)) {
      expect(boxesOverlap(box, triad)).toBe(false);
    }
  });

  it("keeps the arrow and its label inside the view and off the triad from every direction", () => {
    const failures: string[] = [];

    for (const angles of everyAngle()) {
      const triad = triadBox(angles);
      const layout = coreArrowLayout(FRAME, angles, NARROW, LABEL, [triad]);
      const clear = coreArrowBoxes(layout, NARROW.remPx).every(
        (box) => insideView(box, NARROW) && !boxesOverlap(box, triad),
      );
      if (!clear) {
        failures.push(`${angles.azimuthDeg}°, ${angles.elevationDeg}°`);
      }
    }

    expect(failures).toEqual([]);
  });

  it("puts the arrow's head where coreward leaves the view when nothing is in the way", () => {
    const layout = coreArrowLayout(FRAME, PRESETS.top, NARROW, LABEL, []);

    expect(layout).toMatchObject({ kind: "arrow", tip: { xPx: 180, yPx: 8 } });
  });

  it("has nothing to draw on the galactic axis", () => {
    expect(coreArrowLayout(localFrameAt(vec3(0, 0, 0)), PRESETS.top, NARROW, LABEL, []).kind).toBe(
      "undefined",
    );
  });
});

/**
 * An orbit map's plane, tilted 30° to the galactic one away from coreward, its coreward laid from
 * galactic spinward: from `OBLIQUE`, galactic coreward then runs down and to the left, as on
 * T16.f's orbit map.
 */
const ORBIT_PLANE = planeFrame(
  add(scale(FRAME.north, Math.cos(Math.PI / 6)), scale(FRAME.coreward, -0.5)),
  FRAME.spinward,
);
/** The orbit map's stage at 1920 × 1080 CSS px, beside its list. */
const ORBIT_VIEW: Viewport = { widthPx: 1232, heightPx: 766, remPx: 16 };
/** How far the core arrow's label stands from its segment: 0.625 rem, to within rounding. */
const ARROW_CLEAR_PX = (CORE_HEAD_REM + 0.25) * 16 - 1e-9;

/** The core arrow as laid out, which must be an arrow. */
function arrowOf(layout: CoreArrowLayout): Extract<CoreArrowLayout, { kind: "arrow" }> {
  if (layout.kind !== "arrow") {
    throw new Error(`the core arrow is drawn as ${layout.kind}, not an arrow`);
  }
  return layout;
}

/** The core arrow from the top turned to 45°, which points up and to the left at 45°. */
const AT_45_DEG: CameraAngles = { azimuthDeg: 45, elevationDeg: 90 };
/** A view with room for the core arrow's label beside a 45° arrow at its corner. */
const ROOMY: Viewport = { widthPx: 900, heightPx: 700, remPx: 16 };

/** The outer half of a box beside the core arrow, on the arrow's left or right. */
function outerHalf(box: BoxPx, side: "left" | "right"): BoxPx {
  return {
    ...box,
    leftPx: side === "left" ? box.leftPx : box.leftPx + box.widthPx / 2,
    widthPx: box.widthPx / 2,
  };
}

/** The middle of the core arrow and its label's centre, CSS px. */
function arrowMiddleAndLabelCentre(
  arrow: Extract<CoreArrowLayout, { kind: "arrow" }>,
): readonly [ScreenPoint, ScreenPoint] {
  return [
    { xPx: (arrow.tail.xPx + arrow.tip.xPx) / 2, yPx: (arrow.tail.yPx + arrow.tip.yPx) / 2 },
    {
      xPx: arrow.label.leftPx + arrow.label.widthPx / 2,
      yPx: arrow.label.topPx + arrow.label.heightPx / 2,
    },
  ];
}

describe("coreArrowLayout's label clear of its arrow (R07.T16.j)", () => {
  it.each([
    ["the local chart, narrow", FRAME, NARROW],
    ["the local chart, wide", FRAME, ORBIT_VIEW],
    ["an orbit map, narrow", ORBIT_PLANE, NARROW],
    ["an orbit map, wide", ORBIT_PLANE, ORBIT_VIEW],
  ] as const)(
    "keeps its label 0.625 rem from the arrow's shaft and head by true distance, from every direction, on %s",
    (_, frame, viewport) => {
      const failures: string[] = [];

      for (const angles of everyAngle()) {
        const triad = triadFootprintPx(triadLayout(frame, angles, TRIAD_BOX_REM, FRAME), viewport);
        const layout = coreArrowLayout(frame, angles, viewport, LABEL, [triad], FRAME);
        if (
          layout.kind === "arrow" &&
          !(
            segmentBoxGapPx(layout.tail, layout.tip, layout.label) >= ARROW_CLEAR_PX &&
            insideView(layout.label, viewport)
          )
        ) {
          failures.push(`${angles.azimuthDeg}°, ${angles.elevationDeg}°`);
        }
      }

      expect(failures).toEqual([]);
    },
  );

  it("keeps an orbit map's arrow near the view's edge clear of its own label", () => {
    // The orbit map from OBLIQUE, its plane tilted to the galaxy's: galactic coreward runs down to
    // the bottom left, so that the arrow stops short of the triad near the foot of the view, where
    // T16.f's capture showed its head across its own label's digits.
    const triad = triadFootprintPx(
      triadLayout(ORBIT_PLANE, PRESETS.oblique, TRIAD_BOX_REM, FRAME),
      ORBIT_VIEW,
    );
    const arrow = arrowOf(
      coreArrowLayout(ORBIT_PLANE, PRESETS.oblique, ORBIT_VIEW, LABEL, [triad], FRAME),
    );

    expect([
      arrow.tip.yPx > ORBIT_VIEW.heightPx / 2,
      segmentBoxGapPx(arrow.tail, arrow.tip, arrow.label) >= ARROW_CLEAR_PX,
      boxesOverlap(arrow.label, triad),
    ]).toEqual([true, true, false]);
  });

  it.each([
    ["up and to the left, its label below and to the left", 45, -1],
    ["down and to the right, its label above and to the right", 225, 1],
  ] as const)(
    "stands the label beside a 45° arrow pointing %s, 0.625 rem from its segment, its tail included",
    (_, azimuthDeg, rightward) => {
      // From the top, turned to 45° or 225°; at 225° the place below runs off the view's foot, and
      // the label takes the other side.
      const arrow = arrowOf(
        coreArrowLayout(FRAME, { azimuthDeg, elevationDeg: 90 }, ROOMY, LABEL, []),
      );
      const [middle, centre] = arrowMiddleAndLabelCentre(arrow);
      const along = { x: arrow.tip.xPx - arrow.tail.xPx, y: arrow.tip.yPx - arrow.tail.yPx };

      expect([
        // On the line through the arrow's middle across it, on its side.
        Math.abs((centre.xPx - middle.xPx) * along.x + (centre.yPx - middle.yPx) * along.y) < 1e-6,
        Math.sign(centre.xPx - middle.xPx),
        segmentBoxGapPx(arrow.tail, arrow.tip, arrow.label),
        segmentBoxGapPx(arrow.tail, arrow.tail, arrow.label) >= ARROW_CLEAR_PX,
      ]).toEqual([true, rightward, expect.closeTo(10, 6), true]);
    },
  );

  it("stands the label beyond the tail, 0.625 rem from the tail by true distance, where furniture covers both sides", () => {
    // From the top coreward runs straight up, to the view's top edge. The furniture covers the
    // outer half of each place beside it, so that the arrow does not stop short for it.
    const left = arrowOf(coreArrowLayout(FRAME, PRESETS.top, ROOMY, LABEL, [])).label;
    const furniture = [outerHalf(left, "left")];
    const right = arrowOf(coreArrowLayout(FRAME, PRESETS.top, ROOMY, LABEL, furniture)).label;
    const arrow = arrowOf(
      coreArrowLayout(FRAME, PRESETS.top, ROOMY, LABEL, [...furniture, outerHalf(right, "right")]),
    );

    expect([
      arrow.label.topPx > arrow.tail.yPx,
      segmentBoxGapPx(arrow.tail, arrow.tail, arrow.label),
      segmentBoxGapPx(arrow.tail, arrow.tip, arrow.label),
    ]).toEqual([true, expect.closeTo(10, 9), expect.closeTo(10, 9)]);
  });

  it("gives up the furniture's clearance before the arrow's where furniture covers every place", () => {
    // Furniture over the whole view, which the arrow from the view's centre does not stop for.
    const everywhere: BoxPx = {
      leftPx: 0,
      topPx: 0,
      widthPx: NARROW.widthPx,
      heightPx: NARROW.heightPx,
    };
    const arrow = arrowOf(coreArrowLayout(FRAME, AT_45_DEG, NARROW, LABEL, [everywhere]));

    expect(segmentBoxGapPx(arrow.tail, arrow.tip, arrow.label)).toBeGreaterThanOrEqual(
      ARROW_CLEAR_PX,
    );
  });
});

function circleLabels(
  radiusPx: number,
  texts: ReadonlyArray<string>,
  viewport = NARROW,
): CircleLabel[] {
  const centre = { xPx: viewport.widthPx / 2, yPx: viewport.heightPx / 2 };
  return texts.map((text, stack) => ({
    key: `sphere:${radiusPx}:${stack}`,
    text,
    placement: "circle-top",
    xPx: centre.xPx,
    yPx: centre.yPx - radiusPx,
    stack,
    centre,
    radiusPx,
  }));
}

function ringLabel(xPx: number, yPx: number, text: string): RingLabel {
  return { key: `ring:${text}`, text, placement: "ring", xPx, yPx, stack: 0 };
}

/** The boxes the placed labels take, estimated as the view estimates them. */
function placedBoxes(labels: ReadonlyArray<CurveLabel>, viewport = NARROW): BoxPx[] {
  return placeCurveLabels(labels, viewport, []).map((placed) => {
    const size = textSizeRem(placed.text, 0.1);
    return {
      leftPx: placed.leftPx,
      topPx: placed.topPx,
      widthPx: size.widthRem * viewport.remPx,
      heightPx: size.heightRem * viewport.remPx,
    };
  });
}

describe("placeCurveLabels", () => {
  it("sets a lone sphere's label above the top of its circle, right of the top point", () => {
    const placed = placeCurveLabels(circleLabels(100, ["QUERY EDGE 80 ly"]), NARROW, []);

    // The circle's top is at 50 px; the label's bottom 0.375 rem above it, its left 0.5 rem right.
    expect(placed).toEqual([expect.objectContaining({ leftPx: 188, topPx: 50 - 6 - 17.5 })]);
  });

  it("sets both labels of one circle inside a narrow view, one above the other", () => {
    // The fitted circle of a 300 px tall view: its top 2 rem below the edge.
    const labels = circleLabels(118, ["RANGE 50 ly SET", "QUERY EDGE 50 ly"]);

    const boxes = placedBoxes(labels);

    expect(boxes).toHaveLength(2);
    for (const box of boxes) {
      expect(insideView(box, NARROW)).toBe(true);
    }
    expect(boxes[1]?.topPx).toBeCloseTo((boxes[0]?.topPx ?? 0) + 17.5, 9);
  });

  it("keeps labelling a circle whose top has left the view", () => {
    const boxes = placedBoxes(circleLabels(200, ["RANGE 50 ly SET", "QUERY EDGE 50 ly"]));

    expect(boxes).toHaveLength(2);
    for (const box of boxes) {
      expect(insideView(box, NARROW)).toBe(true);
    }
  });

  it("drops the labels of a circle that the view lies wholly inside", () => {
    expect(placeCurveLabels(circleLabels(1_000, ["QUERY EDGE 50 ly"]), NARROW, [])).toEqual([]);
  });

  it("keeps the labels off the furniture and off each other", () => {
    const labels: CurveLabel[] = [
      ...circleLabels(60, ["RANGE 20 ly SET"]),
      ...circleLabels(118, ["QUERY EDGE 50 ly"]),
      ringLabel(180, 90, "PLANE 20 ly"),
    ];
    const furniture: BoxPx = { leftPx: 150, topPx: 0, widthPx: 60, heightPx: 40 };

    const placed = placeCurveLabels(labels, NARROW, [furniture]);

    const boxes = placed.map((label) => {
      const size = textSizeRem(label.text, 0.1);
      return {
        leftPx: label.leftPx,
        topPx: label.topPx,
        widthPx: size.widthRem * 16,
        heightPx: size.heightRem * 16,
      };
    });
    expect(boxes).toHaveLength(3);
    for (const [index, box] of boxes.entries()) {
      expect(boxesOverlap(box, furniture)).toBe(false);
      for (const other of boxes.slice(index + 1)) {
        expect(boxesOverlap(box, other)).toBe(false);
      }
    }
  });

  it("sets a ring's label below and to the right of its coreward point", () => {
    const placed = placeCurveLabels([ringLabel(100, 100, "PLANE 50 ly")], NARROW, []);

    expect(placed).toEqual([expect.objectContaining({ leftPx: 108, topPx: 104 })]);
  });
});

/** A mark's stalk, 2 CSS px wide as at a ratio of 1, from `top` straight down to `bottom`. */
function stalk(xPx: number, topPx: number, bottomPx: number): InkPx {
  return { from: { xPx, yPx: topPx }, to: { xPx, yPx: bottomPx }, halfWidthPx: 1 };
}

/** Stalks every `everyPx` across the whole of a view, top to bottom. */
function stalksAcross(viewport: Viewport, everyPx: number): InkPx[] {
  return Array.from({ length: Math.floor(viewport.widthPx / everyPx) + 1 }, (_, index) =>
    stalk(index * everyPx, 0, viewport.heightPx),
  );
}

/**
 * A ring about `centre` of `radiusPx` on the screen, labelled at its right-hand point, with the
 * points along it every 15° either way that its label also tries, as the draw list gives them.
 */
function ringLabelAlong(centre: ScreenPoint, radiusPx: number, text: string): RingLabel {
  const turns = Array.from({ length: 12 }, (_, index) => (index + 1) * 15).flatMap((turnDeg) =>
    turnDeg === 180 ? [180] : [turnDeg, -turnDeg],
  );
  const at = (deg: number): ScreenPoint => ({
    xPx: centre.xPx + radiusPx * Math.cos((deg * Math.PI) / 180),
    yPx: centre.yPx - radiusPx * Math.sin((deg * Math.PI) / 180),
  });
  return { ...ringLabel(at(0).xPx, at(0).yPx, text), alongPx: turns.map(at) };
}

/** The one curve label placed, which must have been placed: its box as `placeCurveLabels` estimates it. */
function onlyBox(placed: ReadonlyArray<PlacedCurveLabel>): BoxPx {
  const [label, ...others] = placed;
  if (label === undefined || others.length > 0) {
    throw new Error(`expected one curve label placed, not ${String(placed.length)}`);
  }
  return boxOf(label);
}

/** A curve label's box as `placeCurveLabels` estimates it. */
function boxOf(label: { leftPx: number; topPx: number; text: string }, remPx = 16): BoxPx {
  const size = textSizeRem(label.text, 0.1);
  return {
    leftPx: label.leftPx,
    topPx: label.topPx,
    widthPx: size.widthRem * remPx,
    heightPx: size.heightRem * remPx,
  };
}

/** Whether two lengths are one, to within rounding. */
function near(a: number, b: number): boolean {
  return Math.abs(a - b) < 1e-9;
}

/** Which point a ring label's box stands beside, at one of its four corners' places, or -1. */
function besidePointIndex(box: BoxPx, points: ReadonlyArray<ScreenPoint>): number {
  return points.findIndex(
    (point) =>
      (near(box.leftPx, point.xPx + 8) || near(box.leftPx + box.widthPx, point.xPx - 8)) &&
      (near(box.topPx, point.yPx + 4) || near(box.topPx + box.heightPx, point.yPx - 4)),
  );
}

/** How near any of `inks` comes to a box, CSS px. */
function nearestInkPx(box: BoxPx, inks: ReadonlyArray<InkPx>): number {
  return Math.min(...inks.map((ink) => inkGapPx(box, ink)));
}

describe("placeCurveLabels clear of the marks' stalks and symbols (R07.T16.j)", () => {
  it("moves a ring's label along its ring, clear of a stalk through each of its places beside its coreward point", () => {
    // The ring's label at (180, 150) would stand right of the point, above or below it, across a
    // stalk at 230 px, or left of it across one at 120 px.
    const label = ringLabelAlong({ xPx: 120, yPx: 150 }, 60, "PLANE 20 ly");
    const stalks = [stalk(230, 100, 200), stalk(120, 100, 200)];

    const box = onlyBox(placeCurveLabels([label], NARROW, [], stalks));

    expect([
      besidePointIndex(box, [{ xPx: label.xPx, yPx: label.yPx }]),
      besidePointIndex(box, label.alongPx ?? []) >= 0,
      nearestInkPx(box, stalks) >= 2 - 1e-9,
    ]).toEqual([-1, true, true]);
  });

  it("keeps a ring's label at its first place clear of all but the stalks where stalks hem in every place", () => {
    const label = ringLabelAlong({ xPx: 120, yPx: 150 }, 60, "PLANE 20 ly");

    const placed = placeCurveLabels([label], NARROW, [], stalksAcross(NARROW, 10));

    expect(placed).toEqual([expect.objectContaining({ leftPx: 188, topPx: 154 })]);
  });

  it("keeps a ring's label clear of furniture as well as the stalks, giving up the stalks first", () => {
    // Furniture over the first two places, below and above to the right; stalks everywhere.
    const label = ringLabelAlong({ xPx: 120, yPx: 150 }, 60, "PLANE 20 ly");
    const furniture: BoxPx = { leftPx: 185, topPx: 120, widthPx: 120, heightPx: 60 };

    const box = onlyBox(placeCurveLabels([label], NARROW, [furniture], stalksAcross(NARROW, 10)));

    expect(boxesOverlap(box, furniture)).toBe(false);
  });

  it("moves a sphere's label round its circle off a stalk through its place above the top", () => {
    const labels = circleLabels(100, ["QUERY EDGE 80 ly"]);
    // The label's place above the top runs from 188 px across; a stalk at 220 px crosses it.
    const stalks = [stalk(220, 0, 60)];

    const box = onlyBox(placeCurveLabels(labels, NARROW, [], stalks));

    expect([
      box.leftPx === 188 && box.topPx === 50 - 6 - 17.5,
      nearestInkPx(box, stalks) >= 2 - 1e-9,
    ]).toEqual([false, true]);
  });

  it("never drops a sphere's label for a stalk's sake: hemmed in, it takes its first place", () => {
    const placed = placeCurveLabels(
      circleLabels(100, ["QUERY EDGE 80 ly"]),
      NARROW,
      [],
      stalksAcross(NARROW, 10),
    );

    expect(placed).toEqual([expect.objectContaining({ leftPx: 188, topPx: 50 - 6 - 17.5 })]);
  });

  it("keeps a curve label 0.125 rem clear of a mark's symbol", () => {
    // A class-4 mark's symbol, 8 px about its centre, 1 px below the ring label's first place.
    const symbol: InkPx = {
      from: { xPx: 220, yPx: 180.5 },
      to: { xPx: 220, yPx: 180.5 },
      halfWidthPx: 8,
    };

    const box = onlyBox(
      placeCurveLabels([ringLabel(180, 150, "PLANE 20 ly")], NARROW, [], [symbol]),
    );

    expect(nearestInkPx(box, [symbol])).toBeGreaterThanOrEqual(2 - 1e-9);
  });
});

describe("markInksPx", () => {
  /** A mark's stalk, a grid line and three symbols, as the draw list gives them. */
  const OPS: ReadonlyArray<DrawOp> = [
    {
      kind: "line",
      from: { xPx: 10, yPx: 10 },
      to: { xPx: 10, yPx: 60 },
      stroke: "text",
      widthPx: 1,
      markId: "a",
    },
    {
      kind: "line",
      from: { xPx: 0, yPx: 0 },
      to: { xPx: 100, yPx: 0 },
      stroke: "line",
      widthPx: 1,
      markId: null,
    },
    ...(["circle", "triangle", "ringed-circle"] as const).map((shape): DrawOp => ({
      kind: "symbol",
      id: shape,
      centre: { xPx: 10, yPx: 10 },
      shape,
      radiusPx: 5.25,
      stroke: "text",
      fill: null,
      widthPx: 1.5,
    })),
  ];

  it("takes each stalk at the line scale and each symbol to its outline's outer edge, and no grid line, at 0.78125", () => {
    // At 0.78125 a line is 2.56 CSS px wide and an outline too, moved out by 0.53 px (δ): a circle by
    // δ, a triangle's corners by 2δ and mitred to a whole stroke beyond, a ringed circle's ring by 3δ.
    const shiftPx = (2 - 1.5 * 0.78125) / 2 / 0.78125;
    const halfStrokePx = 1 / 0.78125;

    expect(markInksPx(OPS, 0.78125).map((ink) => ink.halfWidthPx)).toEqual([
      expect.closeTo(halfStrokePx, 9),
      expect.closeTo(5.25 + shiftPx + halfStrokePx, 9),
      expect.closeTo(5.25 + 2 * shiftPx + 2 * halfStrokePx, 9),
      expect.closeTo(5.25 + 3 * shiftPx + halfStrokePx, 9),
    ]);
  });
});

describe("curveInksPx", () => {
  /** A grid line, a plane ring, an orbit, a selected orbit, an eye, a stalk and two tick sets. */
  const OPS: ReadonlyArray<DrawOp> = [
    {
      kind: "line",
      from: { xPx: 0, yPx: 0 },
      to: { xPx: 100, yPx: 0 },
      stroke: "line",
      widthPx: 1,
      markId: null,
    },
    {
      kind: "polyline",
      points: [
        { xPx: 0, yPx: 10 },
        { xPx: 50, yPx: 10 },
      ],
      stroke: "line",
      widthPx: 1,
    },
    {
      kind: "polyline",
      points: [
        { xPx: 0, yPx: 20 },
        { xPx: 50, yPx: 20 },
        { xPx: 50, yPx: 70 },
      ],
      stroke: "textMuted",
      widthPx: 1,
    },
    {
      kind: "polyline",
      points: [
        { xPx: 0, yPx: 30 },
        { xPx: 50, yPx: 30 },
      ],
      stroke: "text",
      widthPx: 2,
    },
    {
      kind: "polyline",
      points: [
        { xPx: 60, yPx: 30 },
        { xPx: 70, yPx: 30 },
      ],
      stroke: "text",
      widthPx: 1.5,
      weight: "mark",
    },
    {
      kind: "line",
      from: { xPx: 10, yPx: 10 },
      to: { xPx: 10, yPx: 60 },
      stroke: "text",
      widthPx: 1,
      markId: "a",
    },
    {
      kind: "ticks",
      segments: [
        { from: { xPx: 0, yPx: 40 }, to: { xPx: 0, yPx: 44 } },
        { from: { xPx: 10, yPx: 40 }, to: { xPx: 10, yPx: 44 } },
      ],
      stroke: "textMuted",
      widthPx: 1,
      weight: "line",
    },
    {
      kind: "ticks",
      segments: [{ from: { xPx: 0, yPx: 50 }, to: { xPx: 4, yPx: 54 } }],
      stroke: "text",
      widthPx: 1.5,
      weight: "mark",
    },
  ];

  it("takes each orbit's segment at the line scale, and no grid line, plane ring, eye, stalk or tick, at 1.5", () => {
    // At 1.5 a line is drawn 2 device px a CSS px, 4/3 CSS px for each of its own: half of a 1 px
    // line's is 2/3 px, and of the selected orbit's 2 px, 4/3 px.
    const onePx = 2 / 1.5 / 2;

    expect(curveInksPx(OPS, 1.5).map((ink) => [ink.from.yPx, ink.to.yPx, ink.halfWidthPx])).toEqual(
      [
        [20, 20, expect.closeTo(onePx, 9)],
        [20, 70, expect.closeTo(onePx, 9)],
        [30, 30, expect.closeTo(2 * onePx, 9)],
      ],
    );
  });

  it("takes each line tick at the line scale, and no mark's tick, at 1.5", () => {
    const onePx = 2 / 1.5 / 2;

    expect(tickInksPx(OPS, 1.5).map((ink) => [ink.from.yPx, ink.to.yPx, ink.halfWidthPx])).toEqual([
      [40, 44, expect.closeTo(onePx, 9)],
      [40, 44, expect.closeTo(onePx, 9)],
    ]);
  });
});

/** What the annuli's labels are placed among: nothing, but what is given. */
function amid(surroundings: Partial<AnnulusLabelSurroundings> = {}): AnnulusLabelSurroundings {
  return {
    standing: [],
    marks: [],
    brackets: [],
    reticledIds: new Set(),
    curves: [],
    ticks: [],
    ...surroundings,
  };
}

/** An annulus's label beside a point, giving way to the marks', with the points it may move to. */
function annulusLabel(
  xPx: number,
  yPx: number,
  text: string,
  alongPx: ReadonlyArray<ScreenPoint> = [],
): RingLabel {
  return { ...ringLabel(xPx, yPx, text), key: `annulus:${text}`, alongPx, yieldsToMarks: true };
}

/**
 * An annulus's label on a ring about `centre` of `radiusPx` on the screen, at the point `fromDeg`
 * round it (counter-clockwise from the right), with the points along it every 15° either way from
 * there, nearest first, as the draw list gives them.
 */
function annulusLabelAlong(
  centre: ScreenPoint,
  radiusPx: number,
  text: string,
  fromDeg = 0,
): RingLabel {
  const turns = Array.from({ length: 12 }, (_, index) => (index + 1) * 15).flatMap((turnDeg) =>
    turnDeg === 180 ? [180] : [turnDeg, -turnDeg],
  );
  const at = (deg: number): ScreenPoint => ({
    xPx: centre.xPx + radiusPx * Math.cos((deg * Math.PI) / 180),
    yPx: centre.yPx - radiusPx * Math.sin((deg * Math.PI) / 180),
  });
  const point = at(fromDeg);
  return annulusLabel(
    point.xPx,
    point.yPx,
    text,
    turns.map((turnDeg) => at(fromDeg + turnDeg)),
  );
}

/** A ring's or an annulus's label's first place: 0.5 rem right of its point and 0.25 rem below. */
function firstPlace(label: RingLabel, remPx = 16): BoxPx {
  return boxOf(
    { leftPx: label.xPx + 0.5 * remPx, topPx: label.yPx + 0.25 * remPx, text: label.text },
    remPx,
  );
}

/** Whether a placed label stands at a box's place. */
function standsAt(label: PlacedCurveLabel | undefined, box: BoxPx): boolean {
  return label !== undefined && near(label.leftPx, box.leftPx) && near(label.topPx, box.topPx);
}

/** Whether a stroke crosses or touches a box at its width. */
function crosses(box: BoxPx, ink: InkPx): boolean {
  return segmentBoxGapPx(ink.from, ink.to, box) <= ink.halfWidthPx;
}

describe("placeAnnulusLabels (plan 14, P14.T42.d)", () => {
  // `SNOW LINE` beside its point at (100, 100): its first place from 108 to 198.72 px across and
  // from 104 to 121.5 px down.
  const SNOW = annulusLabel(100, 100, "SNOW LINE");
  const SNOW_BOX = firstPlace(SNOW);

  /** How a label's first place stands to a box: below it, or end to end beyond either end. */
  type Arrangement = "stacked" | "end to end" | "end to end, before it";

  /**
   * `BELT 1`, its first place `gapPx` below a box, or `gapPx` beyond its right-hand or left-hand
   * end at its height, with a point well below to move to.
   */
  function besideBox(box: BoxPx, arrangement: Arrangement, gapPx: number): RingLabel {
    const widthPx = textSizeRem("BELT 1", 0.1).widthRem * 16;
    let xPx: number;
    switch (arrangement) {
      case "stacked":
        xPx = box.leftPx - 8;
        break;
      case "end to end":
        xPx = box.leftPx + box.widthPx + gapPx - 8;
        break;
      case "end to end, before it":
        xPx = box.leftPx - gapPx - widthPx - 8;
        break;
    }
    const yPx = arrangement === "stacked" ? box.topPx + box.heightPx + gapPx - 4 : box.topPx - 4;
    return annulusLabel(xPx, yPx, "BELT 1", [{ xPx, yPx: yPx + 100 }]);
  }

  it.each([
    ["stacked, 0.375 rem apart, moves the later", "stacked", 6, false],
    ["stacked, 0.625 rem apart, keeps both", "stacked", 10, true],
    ["end to end, 0.375 rem apart, moves the later", "end to end", 6, false],
    ["end to end, 0.625 rem apart, keeps both", "end to end", 10, true],
  ] as const)(
    "sets two annulus labels at least 0.5 rem apart: their first places %s",
    (_, arrangement, gapPx, keeps) => {
      const later = besideBox(SNOW_BOX, arrangement, gapPx);

      const placed = placeAnnulusLabels([SNOW, later], NARROW, amid());

      const [first, second] = placed;
      expect([
        placed.length,
        standsAt(first, SNOW_BOX),
        standsAt(second, firstPlace(later)),
        first !== undefined && second !== undefined && boxGapPx(first, second) >= 8 - 1e-9,
      ]).toEqual([2, true, keeps, true]);
    },
  );

  // A mark's label as `placeLabels` placed it, before the annuli's.
  const MARK_LABEL: BoxPx = { leftPx: 108, topPx: 104, widthPx: 120, heightPx: 17.5 };

  it.each([
    ["stacked, 0.375 rem below it, moves", "stacked", 6, false],
    ["stacked, 0.625 rem below it, stays", "stacked", 10, true],
    ["end to end, 0.375 rem beyond it, moves", "end to end", 6, false],
    ["end to end, 0.625 rem beyond it, stays", "end to end", 10, true],
  ] as const)(
    "sets an annulus label 0.5 rem from a mark's label placed before it: its first place %s",
    (_, arrangement, gapPx, keeps) => {
      const label = besideBox(MARK_LABEL, arrangement, gapPx);

      const [placed] = placeAnnulusLabels([label], NARROW, amid({ standing: [MARK_LABEL] }));

      expect([
        standsAt(placed, firstPlace(label)),
        placed !== undefined && boxGapPx(placed, MARK_LABEL) >= 8 - 1e-9,
      ]).toEqual([keeps, true]);
    },
  );

  it.each([
    ["stacked, 0.375 rem below it, moves", "stacked", 6, false],
    ["stacked, 0.625 rem below it, stays", "stacked", 10, true],
    ["end to end, 0.375 rem before it, moves", "end to end, before it", 6, false],
    ["end to end, 0.625 rem before it, stays", "end to end, before it", 10, true],
  ] as const)(
    "sets an annulus label 0.5 rem from the core arrow and its label: its first place %s",
    (_, arrangement, gapPx, keeps) => {
      // From the top the arrow points up from the view's centre, its label at its left, so the
      // label's free end is its left-hand one.
      const arrow = arrowOf(coreArrowLayout(FRAME, PRESETS.top, ROOMY, LABEL, []));
      const furniture = coreArrowBoxes(arrow, 16);
      const label = besideBox(arrow.label, arrangement, gapPx);

      const [placed] = placeAnnulusLabels([label], ROOMY, amid({ standing: furniture }));

      expect([
        standsAt(placed, firstPlace(label)),
        placed !== undefined && furniture.every((box) => boxGapPx(placed, box) >= 8 - 1e-9),
      ]).toEqual([keeps, true]);
    },
  );

  /**
   * A planet's mark of radius 5 px straight below `SNOW LINE`'s first place, as the draw list
   * gives it at a ratio of 1: its symbol's ink, and the box of the selection's bracket's place
   * about it, 11 px about its centre, whose top stands `gapPx` below the label's first place.
   */
  function planetBelow(gapPx: number): { marks: ReadonlyArray<InkPx>; bracket: BoxPx } {
    const anchor: Anchor = {
      id: "p",
      xPx: SNOW_BOX.leftPx + SNOW_BOX.widthPx / 2,
      yPx: SNOW_BOX.topPx + SNOW_BOX.heightPx + gapPx + 11,
      depth: 0,
      radiusPx: 5,
    };
    const symbol: DrawOp = {
      kind: "symbol",
      id: "p",
      centre: anchor,
      shape: "triangle-down",
      radiusPx: 5 - 0.75,
      stroke: "text",
      fill: null,
      widthPx: 1.5,
    };
    return {
      marks: markInksPx([symbol], 1),
      bracket: bracketBoxPx(anchor, 16, reticleStrokesCssPx(1)),
    };
  }

  it.each([
    ["0.125 rem outside a mark's bracket's box, moves", 2, false],
    ["0.375 rem outside it, stays", 6, true],
  ] as const)(
    "sets an annulus label 0.25 rem outside every mark's bracket's box: its first place %s",
    (_, gapPx, keeps) => {
      const { marks, bracket } = planetBelow(gapPx);

      const [placed] = placeAnnulusLabels(
        [SNOW],
        NARROW,
        amid({ marks, brackets: [{ id: "p", box: bracket }] }),
      );

      expect([
        boxGapPx(SNOW_BOX, bracket),
        standsAt(placed, SNOW_BOX),
        placed !== undefined && boxGapPx(placed, bracket) >= 4 - 1e-9,
      ]).toEqual([expect.closeTo(gapPx, 9), keeps, true]);
    },
  );

  it("leaves a ring's label 0.125 rem outside a mark's bracket's box at its first place, clear of its symbol (R07.T16.j)", () => {
    const { marks } = planetBelow(2);

    const placed = placeCurveLabels(
      [ringLabel(SNOW.xPx, SNOW.yPx, "SNOW LINE")],
      NARROW,
      [],
      marks,
    );

    expect(placed).toEqual([expect.objectContaining({ leftPx: 108, topPx: 104 })]);
  });

  it("leaves out an annulus label that stalks hem in on every place, where a ring's label keeps its first", () => {
    const stalks = stalksAcross(NARROW, 10);
    const annulus = annulusLabelAlong({ xPx: 120, yPx: 150 }, 60, "PLANE 20 ly");
    const ring = ringLabelAlong({ xPx: 120, yPx: 150 }, 60, "PLANE 20 ly");

    expect([
      placeAnnulusLabels([annulus], NARROW, amid({ marks: stalks })),
      placeCurveLabels([ring], NARROW, [], stalks),
    ]).toEqual([[], [expect.objectContaining({ leftPx: 188, topPx: 154 })]]);
  });

  it("leaves the annuli's labels to placeAnnulusLabels and the other curve labels to placeCurveLabels", () => {
    const annulus = annulusLabel(100, 100, "SNOW LINE");
    const ring = ringLabel(100, 200, "PLANE 20 ly");

    expect([
      placeCurveLabels([annulus, ring], NARROW, []).map((label) => label.key),
      placeAnnulusLabels([annulus, ring], NARROW, amid()).map((label) => label.key),
    ]).toEqual([["ring:PLANE 20 ly"], ["annulus:SNOW LINE"]]);
  });

  // `SNOW LINE` on a ring of 60 px about (120, 150), at its right-hand point, (180, 150).
  const ALONG = annulusLabelAlong({ xPx: 120, yPx: 150 }, 60, "SNOW LINE");

  it("moves an annulus label along its edge to the first place that no curve crosses", () => {
    // Two ticks 50 px long, 2 px wide, across its places beside its point and the points 15° on
    // either way, but not above right of the point 30° on.
    const ticks: InkPx[] = [200, 160].map((xPx) => ({
      from: { xPx, yPx: 125 },
      to: { xPx, yPx: 175 },
      halfWidthPx: 1,
    }));

    const box = onlyBox(placeAnnulusLabels([ALONG], NARROW, amid({ curves: ticks })));

    expect([
      besidePointIndex(box, [{ xPx: ALONG.xPx, yPx: ALONG.yPx }]),
      besidePointIndex(box, ALONG.alongPx ?? []),
      ticks.some((tick) => crosses(box, tick)),
    ]).toEqual([-1, 2, false]);
  });

  it("takes an annulus label's first place clear of the marks where curves cross every place", () => {
    // Curves every 10 px across the view, and a stalk across its first place.
    const placed = placeAnnulusLabels(
      [ALONG],
      NARROW,
      amid({ marks: [stalk(200, 150, 180)], curves: stalksAcross(NARROW, 10) }),
    );

    expect(placed).toEqual([expect.objectContaining({ leftPx: 188, topPx: 150 - 4 - 17.5 })]);
  });

  it("leaves an annulus label out where a tick crosses every place, rather than set it across one", () => {
    // Ticks every 10 px across the view, as a belt's lie across the inner system at ALL.
    expect(placeAnnulusLabels([ALONG], NARROW, amid({ ticks: stalksAcross(NARROW, 10) }))).toEqual(
      [],
    );
  });

  it("moves an annulus label along its edge to a place no tick crosses, though an orbit crosses it", () => {
    // Two ticks across its first places, and an orbit across every place.
    const ticks: InkPx[] = [200, 160].map((xPx) => ({
      from: { xPx, yPx: 125 },
      to: { xPx, yPx: 175 },
      halfWidthPx: 1,
    }));

    const box = onlyBox(
      placeAnnulusLabels([ALONG], NARROW, amid({ ticks, curves: stalksAcross(NARROW, 10) })),
    );

    expect(ticks.some((tick) => crosses(box, tick))).toBe(false);
  });

  it("sets every annulus label at its first place where there is room, as built", () => {
    // Rings 100, 175, 220 and 250 px across about the centre of a 1214 x 625 stage, each labelled
    // on a side of its own, and no mark.
    const stage: Viewport = { widthPx: 1214, heightPx: 625, remPx: 16 };
    const centre = { xPx: 607, yPx: 312.5 };
    const labels = [
      annulusLabelAlong(centre, 50, "HABITABLE ZONE", -90),
      annulusLabelAlong(centre, 87.5, "SNOW LINE", 0),
      annulusLabelAlong(centre, 110, "OPTIMISTIC", 180),
      annulusLabelAlong(centre, 125, "BELT 1", 90),
    ];

    const placed = placeAnnulusLabels(labels, stage, amid());

    expect(labels.map((label, index) => standsAt(placed[index], firstPlace(label)))).toEqual([
      true,
      true,
      true,
      true,
    ]);
  });
});

/** One of the orbit map's supported stages: its canvas, CSS px, its ratio and its scale at ALL. */
interface CrowdStage {
  readonly name: string;
  readonly viewport: Viewport;
  readonly ratio: number;
  readonly pxPerAu: number;
}

/** The scale at `ALL` at 1280 x 720 that draws a belt's outer edge 3.4 AU out 40 px across. */
const CROWD_PX_PER_AU = 20 / 3.4;

const CROWD_STAGES: ReadonlyArray<CrowdStage> = [
  {
    name: "787.33 x 184 px at a ratio of 1 (1280 x 720)",
    viewport: { widthPx: 787.33, heightPx: 184, remPx: 16 },
    ratio: 1,
    pxPerAu: CROWD_PX_PER_AU,
  },
  {
    name: "1214 x 625 px at a ratio of 1 (1920 x 1080)",
    viewport: { widthPx: 1214, heightPx: 625, remPx: 16 },
    ratio: 1,
    pxPerAu: 3.4 * CROWD_PX_PER_AU,
  },
  {
    name: "787.33 x 197.33 px at a ratio of 1.5 (1920 x 1080 at 150%)",
    viewport: { widthPx: 787.33, heightPx: 197.33, remPx: 16 },
    ratio: 1.5,
    pxPerAu: CROWD_PX_PER_AU,
  },
];

/**
 * A synthetic crowd like the one look-90 found on the orbit map at `ALL` (GDG 75DX08 A-7), drawn
 * from the default oblique view: a host at the centre; `HABITABLE ZONE` (0.989 to 1.69 AU),
 * `SNOW LINE` (2.26 AU), `OPTIMISTIC` (0.749 to 1.77 AU, its edges ticked into the band) and
 * `BELT 1` (2.5 to 3.4 AU, ticked across every 10°), each labelled at its outer edge's rimward
 * point, the optimistic zone and the belt at their spinward points as the orbit map has them, in
 * the scene's order; three planets on orbits between them (0.87, 1.38 and 2.05 AU), each with a
 * short stalk; and the selection on the habitable zone's outer edge at its rimward point, where
 * that zone's label starts, its orbit drawn as the selected one's and its label placed at its
 * right, `GDG 75DX08 A-7 /256`'s 156.1 x 17.5 px. Each mark's anchor is 5 px in radius, the
 * host's 8.
 */
function orbitCrowd(stage: CrowdStage): {
  readonly labels: ReadonlyArray<RingLabel>;
  readonly ops: ReadonlyArray<DrawOp>;
  readonly anchors: ReadonlyArray<Anchor>;
  readonly selectionLabel: BoxPx;
} {
  const { viewport, ratio, pxPerAu } = stage;
  const camera = { ...PRESETS.oblique, pxPerUnit: pxPerAu };
  const basis = viewBasis(FRAME, PRESETS.oblique);
  const onPlane = (radiusAu: number, angleDeg: number): ScreenPoint => {
    const angle = (angleDeg * Math.PI) / 180;
    const { xPx, yPx } = project(
      add(
        scale(FRAME.coreward, radiusAu * Math.cos(angle)),
        scale(FRAME.spinward, radiusAu * Math.sin(angle)),
      ),
      basis,
      camera,
      viewport,
    );
    return { xPx, yPx };
  };
  const turns = Array.from({ length: 12 }, (_, index) => (index + 1) * 15).flatMap((turnDeg) =>
    turnDeg === 180 ? [180] : [turnDeg, -turnDeg],
  );
  const label = (text: string, radiusAu: number, fromDeg: number): RingLabel => {
    const point = onPlane(radiusAu, fromDeg);
    return annulusLabel(
      point.xPx,
      point.yPx,
      text,
      turns.map((turnDeg) => onPlane(radiusAu, fromDeg + turnDeg)),
    );
  };
  const edge = (radiusAu: number, stroke: "textMuted" | "text" = "textMuted"): DrawOp => ({
    kind: "polyline",
    points: Array.from({ length: 97 }, (_, index) => onPlane(radiusAu, (index % 96) * 3.75)),
    stroke,
    widthPx: stroke === "text" ? 2 : 1,
  });
  const everyTenDeg = Array.from({ length: 36 }, (_, index) => index * 10);
  // A tick from one edge towards another, at most `lengthPx` long, or the whole way across.
  const tick = (fromAu: number, towardsAu: number, angleDeg: number, lengthPx = Infinity) => {
    const from = onPlane(fromAu, angleDeg);
    const aim = onPlane(towardsAu, angleDeg);
    const spanPx = Math.hypot(aim.xPx - from.xPx, aim.yPx - from.yPx);
    const share = spanPx > 0 ? Math.min(1, lengthPx / spanPx) : 0;
    return {
      from,
      to: {
        xPx: from.xPx + (aim.xPx - from.xPx) * share,
        yPx: from.yPx + (aim.yPx - from.yPx) * share,
      },
    };
  };
  const planets = [
    { id: "b", radiusAu: 0.87, angleDeg: 200 },
    { id: "c", radiusAu: 1.38, angleDeg: 160 },
    { id: "d", radiusAu: 2.05, angleDeg: 100 },
    { id: "s", radiusAu: 1.69, angleDeg: 180 },
  ];
  const centre = onPlane(0, 0);
  const anchors: Anchor[] = [
    { id: "host", ...centre, depth: 0, radiusPx: 8 },
    ...planets.map(({ id, radiusAu, angleDeg }): Anchor => {
      const at = onPlane(radiusAu, angleDeg);
      return { id, xPx: at.xPx, yPx: at.yPx, depth: 0, radiusPx: 5 };
    }),
  ];
  const marks = anchors.flatMap((anchor): DrawOp[] => [
    {
      kind: "line",
      from: anchor,
      to: { xPx: anchor.xPx, yPx: anchor.yPx + (anchor.id === "host" ? 0 : 3) },
      stroke: "text",
      widthPx: 1,
      markId: anchor.id,
    },
    {
      kind: "symbol",
      id: anchor.id,
      centre: anchor,
      shape: anchor.id === "host" ? "circle" : "triangle-down",
      radiusPx: anchor.radiusPx - 0.75,
      stroke: "text",
      fill: "text",
      widthPx: 1.5,
    },
  ]);
  const ops: DrawOp[] = [
    ...[0.87, 1.38, 2.05].map((radiusAu) => edge(radiusAu)),
    edge(1.69, "text"),
    ...[0.989, 1.69, 2.26, 0.749, 1.77, 2.5, 3.4].map((radiusAu) => edge(radiusAu)),
    {
      kind: "ticks",
      segments: [
        ...everyTenDeg.map((angleDeg) => tick(0.749, 1.77, angleDeg, 4)),
        ...everyTenDeg.map((angleDeg) => tick(1.77, 0.749, angleDeg, 4)),
      ],
      stroke: "textMuted",
      widthPx: 1,
      weight: "line",
    },
    {
      kind: "ticks",
      segments: everyTenDeg.map((angleDeg) => tick(2.5, 3.4, angleDeg)),
      stroke: "textMuted",
      widthPx: 1,
      weight: "line",
    },
    ...marks,
  ];
  const selection = anchors.at(-1) ?? anchors[0];
  if (selection === undefined) {
    throw new Error("the crowd has a selection");
  }
  const bracket = bracketBoxPx(selection, viewport.remPx, reticleStrokesCssPx(ratio));
  return {
    labels: [
      label("HABITABLE ZONE", 1.69, 180),
      label("SNOW LINE", 2.26, 180),
      label("OPTIMISTIC", 1.77, 90),
      label("BELT 1", 3.4, 90),
    ],
    ops,
    anchors,
    selectionLabel: {
      leftPx: bracket.leftPx + bracket.widthPx + 0.25 * viewport.remPx,
      topPx: selection.yPx - 17.5 / 2,
      widthPx: 156.1,
      heightPx: 17.5,
    },
  };
}

describe("placeAnnulusLabels on the orbit map's crowd at its three stages (plan 14, P14.T42.d; look-90)", () => {
  it.each(CROWD_STAGES)(
    "keeps every annulus label drawn 0.5 rem from every label, 0.25 rem outside every bracket's box, 0.125 rem from every stalk and 0.25 rem inside the stage at $name",
    (stage) => {
      const { labels, ops, anchors, selectionLabel } = orbitCrowd(stage);
      const brackets = anchors.map((anchor) => ({
        id: anchor.id,
        box: bracketBoxPx(anchor, stage.viewport.remPx, reticleStrokesCssPx(stage.ratio)),
      }));
      const stalks = markInksPx(
        ops.filter((op) => op.kind === "line"),
        stage.ratio,
      );

      const placed = placeAnnulusLabels(labels, stage.viewport, {
        standing: [selectionLabel],
        marks: markInksPx(ops, stage.ratio),
        brackets,
        // The selection's bracket is drawn about the last mark.
        reticledIds: new Set(anchors.slice(-1).map((anchor) => anchor.id)),
        curves: curveInksPx(ops, stage.ratio),
        ticks: tickInksPx(ops, stage.ratio),
      });

      const inside: BoxPx = {
        leftPx: 4,
        topPx: 4,
        widthPx: stage.viewport.widthPx - 8,
        heightPx: stage.viewport.heightPx - 8,
      };
      const labelsDrawn = [selectionLabel, ...placed];
      const faults: string[] = [];
      for (const label of placed) {
        if (labelsDrawn.some((other) => other !== label && boxGapPx(label, other) < 8 - 1e-9)) {
          faults.push(`${label.text} within 0.5 rem of a label`);
        }
        if (brackets.some(({ box }) => boxGapPx(label, box) < 4 - 1e-9)) {
          faults.push(`${label.text} within 0.25 rem of a bracket's box`);
        }
        if (nearestInkPx(label, stalks) < 2 - 1e-9) {
          faults.push(`${label.text} within 0.125 rem of a stalk`);
        }
        if (!within(label, inside)) {
          faults.push(`${label.text} within 0.25 rem of the stage's edge`);
        }
      }
      expect(faults).toEqual([]);
    },
  );
});

/**
 * A body's mark of radius `radiusPx` at a point, as the draw list gives it at a ratio of 1: its
 * symbol's ink, and the box of the selection's bracket's place about it with the mark's ID.
 */
function markAt(
  id: string,
  centre: ScreenPoint,
  radiusPx: number,
): { marks: ReadonlyArray<InkPx>; bracket: { id: string; box: BoxPx } } {
  const anchor: Anchor = { id, ...centre, depth: 0, radiusPx };
  const symbol: DrawOp = {
    kind: "symbol",
    id,
    centre: anchor,
    shape: "triangle-down",
    radiusPx: radiusPx - 0.75,
    stroke: "text",
    fill: null,
    widthPx: 1.5,
  };
  return {
    marks: markInksPx([symbol], 1),
    bracket: { id, box: bracketBoxPx(anchor, 16, reticleStrokesCssPx(1)) },
  };
}

/** Every place a ring's or an annulus's label tries, beside its point and each along its edge. */
function everyPlace(label: RingLabel): BoxPx[] {
  const { widthPx, heightPx } = firstPlace(label);
  return [{ xPx: label.xPx, yPx: label.yPx }, ...(label.alongPx ?? [])].flatMap((point) =>
    [point.xPx + 8, point.xPx - 8 - widthPx].flatMap((leftPx) =>
      [point.yPx + 4, point.yPx - 4 - heightPx].map((topPx) => ({
        leftPx,
        topPx,
        widthPx,
        heightPx,
      })),
    ),
  );
}

describe("placeAnnulusLabels about a ring's own planet (plan 14, P14.T42.g)", () => {
  // A class-3 planet's mark, 0.875 rem across, at (120, 150): its bracket's box 13 px about its
  // centre.
  const PLANET = markAt("planet", { xPx: 120, yPx: 150 }, 7);
  // Its ring, 12 px across on the screen, lies inside that box, labelled at its right-hand point.
  const RING = annulusLabelAlong({ xPx: 120, yPx: 150 }, 6, "A j RING 1");
  const ABOUT: RingLabel = { ...RING, aboutMarkId: "planet" };

  it("sets a ring's label about its own planet, every place within 0.25 rem of the planet's bracket's box, 0.125 rem clear of its symbol", () => {
    const places = everyPlace(ABOUT);

    const placed = placeAnnulusLabels(
      [ABOUT],
      NARROW,
      amid({ marks: PLANET.marks, brackets: [PLANET.bracket] }),
    );

    expect([
      places.length,
      places.every((box) => boxGapPx(box, PLANET.bracket.box) < 4 - 1e-9),
      placed.length,
      placed.every((label) => nearestInkPx(boxOf(label), PLANET.marks) >= 2 - 1e-9),
    ]).toEqual([96, true, 1, true]);
  });

  it("leaves out a ring's label with no planet named, every place within 0.25 rem of that bracket's box", () => {
    expect(
      placeAnnulusLabels([RING], NARROW, amid({ marks: PLANET.marks, brackets: [PLANET.bracket] })),
    ).toEqual([]);
  });

  it("sets a ring's label about its own planet 0.25 rem outside a second mark's bracket's box at its first place", () => {
    // A class-2 moon below the first place, its bracket's box, 12 px about its centre, touching
    // that place, its symbol 0.125 rem or more clear of it: only the moon's bracket refuses it.
    const first = firstPlace(ABOUT);
    const moon = markAt(
      "moon",
      { xPx: first.leftPx + first.widthPx / 2, yPx: first.topPx + first.heightPx + 12 },
      6,
    );

    const placed = placeAnnulusLabels(
      [ABOUT],
      NARROW,
      amid({
        marks: [...PLANET.marks, ...moon.marks],
        brackets: [PLANET.bracket, moon.bracket],
      }),
    );

    const [label] = placed;
    expect([
      boxGapPx(first, moon.bracket.box),
      nearestInkPx(first, moon.marks) >= 2 - 1e-9,
      placed.length,
      standsAt(label, first),
      label !== undefined && boxGapPx(boxOf(label), moon.bracket.box) >= 4 - 1e-9,
    ]).toEqual([expect.closeTo(0, 9), true, 1, false, true]);
  });

  it("moves a ring's label about its own planet off the planet's symbol, still inside the planet's bracket's box", () => {
    // The ring labelled at its left-hand point, 6 px left of the planet's centre: its first place,
    // 0.5 rem right of that point and 0.25 rem below it, stands on the planet's symbol.
    const left: RingLabel = {
      ...annulusLabelAlong({ xPx: 120, yPx: 150 }, 6, "A j RING 1", 180),
      aboutMarkId: "planet",
    };

    const placed = placeAnnulusLabels(
      [left],
      NARROW,
      amid({ marks: PLANET.marks, brackets: [PLANET.bracket] }),
    );

    const [label] = placed;
    expect([
      nearestInkPx(firstPlace(left), PLANET.marks) < 2,
      placed.length,
      standsAt(label, firstPlace(left)),
      label !== undefined && nearestInkPx(boxOf(label), PLANET.marks) >= 2 - 1e-9,
      label !== undefined && boxGapPx(boxOf(label), PLANET.bracket.box) < 4,
    ]).toEqual([true, 1, false, true, true]);
  });

  it("leaves out a ring's label about its own planet where a tick crosses every place", () => {
    expect(
      placeAnnulusLabels(
        [ABOUT],
        NARROW,
        amid({
          marks: PLANET.marks,
          brackets: [PLANET.bracket],
          ticks: stalksAcross(NARROW, 10),
        }),
      ),
    ).toEqual([]);
  });
});

describe("placeAnnulusLabels about a ring's planet with a reticle drawn about it (plan 14, P14.T42.g; main's ruling on issue #96)", () => {
  // The scene of the look's probe, as the draw list gives it at a ratio of 1: a class-2 planet at
  // the centre of a 1200 × 900 stage at the default camera's fit, `OBLIQUE` at 8.36 px a unit, and
  // its ring 0.3 to 0.6 units about it, its outer edge 5.02 px or less from the centre, inside the
  // planet's bracket's box, 12 px about its centre.
  const STAGE: Viewport = { widthPx: 1200, heightPx: 900, remPx: 16 };
  const CAMERA = { ...PRESETS.oblique, pxPerUnit: (450 - 32) / 50 };
  const PROBE: SpatialScene = {
    frame: FRAME,
    points: [
      {
        id: "p",
        position: vec3(0, 0, 0),
        shape: "triangle-down",
        sizeClass: 2,
        status: "plain",
        label: "A-11",
        labelPriority: Number.MAX_VALUE,
      },
    ],
    spheres: [],
    plane: { spacing: 1, extent: 0, rings: [] },
    annuli: [
      {
        id: "ring:r",
        innerRadius: 0.3,
        outerRadius: 0.6,
        ticks: true,
        label: "A j RING 1",
        aboutId: "p",
      },
    ],
    selectedId: null,
    destinationId: null,
  };

  /** The ring's label placed as the view places it, among what is given a reticle. */
  function probe(reticledIds: ReadonlySet<string>): {
    readonly placed: ReadonlyArray<PlacedCurveLabel>;
    readonly bracket: BoxPx;
  } {
    const drawList = buildDrawList(PROBE, CAMERA, STAGE, minReticleGapDevicePx(1));
    const brackets = drawList.anchors.map((anchor) => ({
      id: anchor.id,
      box: bracketBoxPx(anchor, 16, reticleStrokesCssPx(1)),
    }));
    const [bracket] = brackets;
    if (bracket === undefined) {
      throw new Error("the probe draws its planet");
    }
    // The planet's label, as `placeLabels` sets it: 0.25 rem right of the bracket's box, on its
    // centre's line.
    const planetLabel: BoxPx = {
      leftPx: bracket.box.leftPx + bracket.box.widthPx + 4,
      topPx: 450 - 17.5 / 2,
      widthPx: textSizeRem("A-11").widthRem * 16,
      heightPx: 17.5,
    };
    const placed = placeAnnulusLabels(drawList.curveLabels, STAGE, {
      standing: [planetLabel],
      marks: markInksPx(drawList.ops, 1),
      brackets,
      reticledIds,
      curves: curveInksPx(drawList.ops, 1),
      ticks: tickInksPx(drawList.ops, 1),
    });
    return { placed, bracket: bracket.box };
  }

  it("keeps a ring's label 0.25 rem outside its planet's bracket's box while a reticle is drawn about the planet: here, every place within it, it is left out", () => {
    expect(probe(new Set(["p"])).placed).toEqual([]);
  });

  it.each([
    ["no reticle is drawn", new Set<string>()],
    ["a reticle is drawn about another mark only", new Set(["moon"])],
  ])(
    "sets a ring's label inside its planet's bracket's box while %s, from 491.2 to 592 px across",
    (_, reticledIds) => {
      const { placed, bracket } = probe(reticledIds);

      const boxes = placed.map((label) => boxOf(label));
      expect(
        boxes.map((box) => [box.leftPx, box.leftPx + box.widthPx, boxGapPx(box, bracket)]),
      ).toEqual([[expect.closeTo(491.2, 6), expect.closeTo(592, 6), 0]]);
    },
  );
});
