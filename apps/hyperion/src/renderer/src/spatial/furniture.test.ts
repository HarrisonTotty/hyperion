import { describe, expect, it } from "vitest";

import { type CameraAngles, PRESETS, type Viewport } from "./camera";
import type { CircleLabel, CurveLabel, RingLabel } from "./drawList";
import { localFrameAt } from "./frame";
import {
  type BoxPx,
  boxesOverlap,
  coreArrowBoxes,
  coreArrowLayout,
  coreLabelText,
  placeCurveLabels,
  triadFootprintPx,
  triadLayout,
} from "./furniture";
import { textSizeRem } from "./labels";
import { vec3 } from "./vec3";

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

describe("triadLayout", () => {
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
