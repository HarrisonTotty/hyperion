import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { FIXTURE_MOON, FIXTURE_PLANET } from "../../test/viewFixtures";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION, quaternionFromAxisAngle } from "../camera/quaternion";
import type { CameraTarget } from "../camera/state";
import { markShiftDevicePx, markStrokeDevicePx } from "../../lib/strokes";
import { SIZE_CLASS_REM } from "../../spatial/symbols";
import { bodyKindSymbol } from "../scene/model";
import {
  bodySymbolMark,
  bracketHalfSizePx,
  bracketReticle,
  contactRadiusPx,
  destinationHalfSizePx,
  destinationReticle,
  flightPathMarker,
  type HullOnView,
  lineOfSightClearancePx,
  lineOfSightGivesWay,
  type LineOfSightStrokes,
  lineOfSightTripod,
  markBoxPx,
  markLabelOffsetPx,
  markLabelRisePx,
  type ScreenMark,
  type ScreenSegment,
  segmentDistancePx,
  symbologyMarks,
  symbolRadiusPx,
  targetMark,
} from "./symbology";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = {
  orientation: quaternionFromAxisAngle(vec3(0, 1, 0), 0.2),
  fovXRad: Math.PI / 3,
};
const REM_PX = 16;
const PLANET: CameraTarget = { kind: "body", body: FIXTURE_PLANET };
const MOON: CameraTarget = { kind: "body", body: FIXTURE_MOON };

/**
 * The apices of the destination's chevrons, in their order (above, below, left, right): each
 * chevron is two segments, an arm's outer end to the apex and the apex to the other arm's end.
 */
function apices(mark: ScreenMark): Array<{ xPx: number; yPx: number }> {
  return mark.segments.filter((_, i) => i % 2 === 0).map(([, apex]) => apex);
}

/** The mark's strokes' bounding box. */
function bounds(mark: ScreenMark): { left: number; right: number; top: number; bottom: number } {
  const xs = mark.segments.flatMap(([a, b]) => [a.xPx, b.xPx]);
  const ys = mark.segments.flatMap(([a, b]) => [a.yPx, b.yPx]);
  return {
    left: Math.min(...xs),
    right: Math.max(...xs),
    top: Math.min(...ys),
    bottom: Math.max(...ys),
  };
}

describe("the bracket reticle", () => {
  it("encloses the selected mark's projected position", () => {
    const p = project(vec3(1e6, 2e5, -4e6), CAMERA, VIEWPORT);
    const box = bounds(bracketReticle(PLANET, { xPx: p.xPx, yPx: p.yPx }, 5, REM_PX, 0));
    expect([box.left < p.xPx, box.right > p.xPx, box.top < p.yPx, box.bottom > p.yPx]).toEqual([
      true,
      true,
      true,
      true,
    ]);
  });

  it("is drawn in --accent as four corner brackets, a margin outside the mark", () => {
    const reticle = bracketReticle(PLANET, { xPx: 100, yPx: 100 }, 5, REM_PX, 0);
    expect([reticle.token, reticle.segments.length, bounds(reticle).left]).toEqual([
      "accent",
      8,
      100 - 5 - 0.25 * REM_PX,
    ]);
  });
});

describe("the destination's chevrons (R07.T16.h; decision-r07-quality-and-destination, Q2)", () => {
  it("is drawn in --target as four chevrons on the mark's axes, pointing at it, outside the selection's bracket", () => {
    const at = { xPx: 100, yPx: 100 };
    const destination = destinationReticle(PLANET, at, 5, REM_PX, 0, 0);
    // The bracket's half-size 9 px, the apices 4 px, a margin, outside it; each arm the bracket's,
    // 6 px, run out from its apex at 45°.
    const run = 6 * Math.SQRT1_2;
    const arms = destination.segments.map(([a, b]) => [
      px(Math.hypot(b.xPx - a.xPx, b.yPx - a.yPx)),
      px(Math.abs(b.xPx - a.xPx)),
    ]);
    expect([destination.token, destination.segments.length, apices(destination), arms]).toEqual([
      "target",
      8,
      [
        { xPx: 100, yPx: 87 },
        { xPx: 100, yPx: 113 },
        { xPx: 87, yPx: 100 },
        { xPx: 113, yPx: 100 },
      ],
      Array.from({ length: 8 }, () => [6, px(run)]),
    ]);
  });

  it("runs each arm away from the mark, so that every chevron points at it", () => {
    const at = { xPx: 100, yPx: 100 };
    const destination = destinationReticle(PLANET, at, 5, REM_PX, 0, 0);
    const fromMark = (p: { xPx: number; yPx: number }): number =>
      Math.hypot(p.xPx - at.xPx, p.yPx - at.yPx);
    // An arm's outer end lies farther from the mark than its apex, on both arms of each chevron.
    expect(
      destination.segments.map(([a, b], i) =>
        i % 2 === 0 ? fromMark(a) > fromMark(b) : fromMark(b) > fromMark(a),
      ),
    ).toEqual(Array.from({ length: 8 }, () => true));
  });

  it("stands the larger of a margin and the least gap outside the selection's bracket", () => {
    expect([
      destinationHalfSizePx(5, REM_PX, 0, 0) - bracketHalfSizePx(5, REM_PX, 0),
      destinationHalfSizePx(5, 10, 0, 4) - bracketHalfSizePx(5, 10, 0),
    ]).toEqual([0.25 * REM_PX, 4]);
  });
});

describe("a mark's label's offset (R07.T16.g; decision-r07-t16d-followups, item 1)", () => {
  // At a ratio of 1 and 100%: δ = 0.25 px, the least gap 4 px.
  const shift = markShiftDevicePx(1);

  it("stands a craft's label at T16.d's place, 0.75 rem and five shifts", () => {
    expect(markLabelOffsetPx(0.375 * REM_PX, REM_PX, shift)).toBe(0.75 * REM_PX + 5 * shift);
  });

  it("stands a class-4 symbol's label 0.125 rem and a shift beyond its bracket", () => {
    const radius = 0.5 * REM_PX;
    expect(markLabelOffsetPx(radius, REM_PX, shift)).toBe(
      bracketHalfSizePx(radius, REM_PX, shift) + 0.125 * REM_PX + shift,
    );
  });
});

describe("a destination's label above its chevron set (R07.T16.h's follow-up; decision-r07-quality-and-destination, addenda A and B)", () => {
  // At a ratio of 1 and 100%: δ = 0.25 px, the least gap 4 px, the mark stroke 2 px.
  const shift = markShiftDevicePx(1);
  const gapPx = 4;
  const strokePx = 2;

  it("stands its plate 0.25 rem above the upper chevron's ink: its apex, an arm's run and half the mark stroke", () => {
    const radius = 0.5 * REM_PX;
    // The chevrons' arms are the bracket's, a third of its side, run out at 45°.
    const runPx = ((2 * bracketHalfSizePx(radius, REM_PX, shift)) / 3) * Math.SQRT1_2;
    expect(markLabelRisePx(radius, REM_PX, shift, gapPx, strokePx)).toBeCloseTo(
      destinationHalfSizePx(radius, REM_PX, shift, gapPx) + runPx + strokePx / 2 + 0.25 * REM_PX,
      9,
    );
  });

  it.each([
    ["class 0", 0.25, 13.25, 22.24],
    ["a craft", 0.375, 13.25, 25.19],
    ["class 4", 0.5, 15.25, 28.13],
  ] as const)(
    "stands %s's destination plate's near edge at its bracket place and its bottom edge above the set, at a ratio of 1 and 100%: %s rem, %s and %s px",
    (_, radiusRem, nearPx, risePx) => {
      // A craft: the bracket 11 px out, the apices 15, the upper chevron's arm ends 7.33 ÷ √2 = 5.19
      // px above them, its ink 1 px more; the plate 4 px above that.
      const radius = radiusRem * REM_PX;
      expect(
        [
          markLabelOffsetPx(radius, REM_PX, shift),
          markLabelRisePx(radius, REM_PX, shift, gapPx, strokePx),
        ].map((valuePx) => Math.round(valuePx * 100) / 100),
      ).toEqual([nearPx, risePx]);
    },
  );
});

describe("a target's mark", () => {
  it("is four open ticks on the cardinal directions, outside the mark, unlike the brackets", () => {
    const at = { xPx: 100, yPx: 100 };
    const mark = targetMark(MOON, at, 6, vec3(0, 0, -1e3), null, 0);
    // Each tick is one axis-aligned stroke along a ray from the centre, starting at the radius.
    const ticks = mark.segments.map(([a, b]) => {
      const inner = Math.hypot(a.xPx - at.xPx, a.yPx - at.yPx);
      const outer = Math.hypot(b.xPx - at.xPx, b.yPx - at.yPx);
      const radial = (a.xPx === b.xPx && a.xPx === at.xPx) || (a.yPx === b.yPx && a.yPx === at.yPx);
      return [radial, inner, outer > inner];
    });
    expect([mark.token, ticks]).toEqual(["text", Array.from({ length: 4 }, () => [true, 6, true])]);
  });

  it("carry its range and closure rate, closing positive", () => {
    const bracket = targetMark(
      MOON,
      { xPx: 10, yPx: 10 },
      5,
      vec3(3e3, 4e3, 0),
      vec3(-3, -4, 0),
      0,
    );
    expect([bracket.rangeM, bracket.closureMPerS]).toEqual([5e3, 5]);
  });

  it("carry no closure rate with no own ship", () => {
    expect(
      targetMark(MOON, { xPx: 10, yPx: 10 }, 5, vec3(0, 0, -1e3), null, 0).closureMPerS,
    ).toBeNull();
  });
});

describe("the flight path marker", () => {
  it("projects the own ship's velocity direction", () => {
    const velocity = vec3(100, 50, -2000);
    const marker = flightPathMarker(velocity, CAMERA, VIEWPORT, REM_PX, 0);
    const p = project(velocity, CAMERA, VIEWPORT);
    expect([marker?.anchor.xPx, marker?.anchor.yPx].map((v) => Math.round((v ?? 0) * 1e6))).toEqual(
      [p.xPx, p.yPx].map((v) => Math.round(v * 1e6)),
    );
  });

  it("shows at a docking speed of a centimetre a second", () => {
    const straight: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    expect(flightPathMarker(vec3(0, 0, -0.01), straight, VIEWPORT, REM_PX, 0)?.anchor).toEqual({
      xPx: 960,
      yPx: 540,
    });
  });

  it("is absent with no own ship", () => {
    expect(flightPathMarker(null, CAMERA, VIEWPORT, REM_PX, 0)).toBeNull();
  });

  it("is absent where the velocity points behind the camera", () => {
    const straight: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    expect(flightPathMarker(vec3(0, 0, 10), straight, VIEWPORT, REM_PX, 0)).toBeNull();
  });
});

describe("a body's symbol", () => {
  it("marks a body 2 px across with its symbol", () => {
    const mark = bodySymbolMark(MOON, bodyKindSymbol("moon"), { xPx: 50, yPx: 50 }, 2, REM_PX, 0);
    // The moon's pentagon: five strokes.
    expect([mark?.kind, mark?.segments.length]).toEqual(["body_symbol", 5]);
  });

  it("leaves a body 4 px across to its limb", () => {
    expect(
      bodySymbolMark(PLANET, bodyKindSymbol("planet"), { xPx: 50, yPx: 50 }, 4, REM_PX, 0),
    ).toBeNull();
  });

  it("draws a ringed circle's ring and its disc", () => {
    const mark = bodySymbolMark(
      PLANET,
      { shape: "ringed-circle", sizeClass: 3 },
      { xPx: 50, yPx: 50 },
      1,
      REM_PX,
      0,
    );
    expect(mark?.segments.length).toBe(24 + 16);
  });
});

/** The symbology of one small body, selected or not, and the destination or not. */
function marksOf(selection: CameraTarget | null, destination: CameraTarget | null): ScreenMark[] {
  return symbologyMarks(
    {
      anchors: [
        {
          target: MOON,
          at: { xPx: 300, yPx: 300 },
          body: { symbol: bodyKindSymbol("moon"), diameterPx: 1 },
          craft: null,
        },
      ],
      selection,
      destination,
      ownVelocityMPerS: null,
      remPx: 10,
      markShiftPx: 0,
      minReticleGapPx: 4,
    },
    { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
    VIEWPORT,
  );
}

describe("symbologyMarks", () => {
  it("stands a destination that is not the selection the least gap outside a bracket's place", () => {
    // The moon's bracket at 10 px to the rem stands its radius and 2.5 px out; the destination's
    // left apex 4 px beyond, the least gap, more than a margin.
    const bracketPx = symbolRadiusPx(bodyKindSymbol("moon"), 10) + 2.5;
    const destination = marksOf(null, MOON).filter((mark) => mark.kind === "destination");
    expect([
      destination.map((mark) => apices(mark)[2]?.xPx),
      marksOf(MOON, null)
        .filter((mark) => mark.kind === "selection")
        .map((mark) => bounds(mark).left),
    ]).toEqual([[300 - bracketPx - 4], [300 - bracketPx]]);
  });

  it("stands the destination in one place whether or not it is the selection", () => {
    const destination = (marks: ScreenMark[]): unknown[] =>
      marks.filter((mark) => mark.kind === "destination").map(bounds);
    expect(destination(marksOf(null, MOON))).toEqual(destination(marksOf(MOON, MOON)));
  });

  it("marks a small body, brackets the selection, the destination and a craft, and adds the flight path marker", () => {
    const marks = symbologyMarks(
      {
        anchors: [
          {
            target: MOON,
            at: { xPx: 300, yPx: 300 },
            body: { symbol: bodyKindSymbol("moon"), diameterPx: 1 },
            craft: null,
          },
          {
            target: { kind: "craft", craft: "other" },
            at: { xPx: 500, yPx: 300 },
            body: null,
            craft: { relativeM: vec3(0, 0, -1e3), relativeVelocityMPerS: null },
          },
        ],
        selection: MOON,
        destination: MOON,
        ownVelocityMPerS: vec3(0, 0, -1),
        remPx: REM_PX,
        markShiftPx: 0,
        minReticleGapPx: 0,
      },
      { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
      VIEWPORT,
    );
    expect(marks.map((m) => [m.kind, m.token])).toEqual([
      ["body_symbol", "text"],
      ["selection", "accent"],
      ["destination", "target"],
      ["target", "text"],
      ["flight_path", "text"],
    ]);
  });
});

/** The display ratios the outlines' shift is tested at, and the shift δ at each, device px. */
const RATIOS = [0.78125, 1, 2, 3] as const;

/** The distances from `at` of a mark's stroke ends, px. */
function radii(mark: ScreenMark | null, at: { xPx: number; yPx: number }): number[] {
  return (mark?.segments ?? []).flatMap(([a, b]) => [
    Math.hypot(a.xPx - at.xPx, a.yPx - at.yPx),
    Math.hypot(b.xPx - at.xPx, b.yPx - at.yPx),
  ]);
}

/** Rounded to 1e-9 px, so that a shift compares exactly, and with no negative zero. */
function px(value: number): number {
  return Math.round(value * 1e9) / 1e9 + 0;
}

describe("the outlines' outward shift (R07.T16.d; decision-thin-line-contrast, item 2)", () => {
  const at = { xPx: 200, yPx: 200 };

  it("moves a class-0 circle's line out by the shift: 0.41, 0.25, 0 and 0 px", () => {
    const out = RATIOS.map((ratio) => {
      const remPx = REM_PX * ratio;
      const circle = { shape: "circle", sizeClass: 0 } as const;
      const mark = bodySymbolMark(PLANET, circle, at, 1, remPx, markShiftDevicePx(ratio));
      const asBuilt = (SIZE_CLASS_REM[0] * remPx) / 2;
      return px(Math.max(...radii(mark, at)) - asBuilt);
    });
    expect(out).toEqual([0.4140625, 0.25, 0, 0]);
  });

  it("moves each side of a polygon out by the shift, so that its hole stays", () => {
    // The triangle's unit inradius is a half: its corners move out by twice the shift.
    const triangle = { shape: "triangle", sizeClass: 0 } as const;
    const out = RATIOS.map((ratio) => {
      const remPx = REM_PX * ratio;
      const shift = markShiftDevicePx(ratio);
      const mark = bodySymbolMark(PLANET, triangle, at, 1, remPx, shift);
      const asBuilt = (SIZE_CLASS_REM[0] * remPx) / 2;
      return px(Math.max(...radii(mark, at)) * 0.5 - asBuilt * 0.5);
    });
    expect(out).toEqual(RATIOS.map((ratio) => px(markShiftDevicePx(ratio))));
  });

  it("moves a ringed circle's disc out by the shift and its ring by three times it", () => {
    const ringed = { shape: "ringed-circle", sizeClass: 2 } as const;
    const out = RATIOS.map((ratio) => {
      const remPx = REM_PX * ratio;
      const mark = bodySymbolMark(PLANET, ringed, at, 1, remPx, markShiftDevicePx(ratio));
      const r = radii(mark, at);
      const ring = symbolRadiusPx(ringed, remPx);
      return [px(Math.max(...r) - ring), px(Math.min(...r) - ring / 3)];
    });
    expect(out).toEqual(
      RATIOS.map((ratio) => [px(3 * markShiftDevicePx(ratio)), px(markShiftDevicePx(ratio))]),
    );
  });

  it("moves the selection's and the destination's reticles out by four times the shift", () => {
    const out = RATIOS.map((ratio) => {
      const remPx = REM_PX * ratio;
      const shift = markShiftDevicePx(ratio);
      return [
        px(at.xPx - bounds(bracketReticle(PLANET, at, 5, remPx, shift)).left - (5 + 0.25 * remPx)),
        px(
          at.xPx -
            (apices(destinationReticle(PLANET, at, 5, remPx, shift, 0))[2]?.xPx ?? Number.NaN) -
            (5 + 0.5 * remPx),
        ),
      ];
    });
    expect(out).toEqual(RATIOS.map((ratio) => [0, 0].map(() => px(4 * markShiftDevicePx(ratio)))));
  });

  it("moves a target's ticks and the flight path marker's circle out by the shift", () => {
    const straight: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    const out = RATIOS.map((ratio) => {
      const remPx = REM_PX * ratio;
      const shift = markShiftDevicePx(ratio);
      const ticks = targetMark(MOON, at, 6, vec3(0, 0, -1e3), null, shift);
      const marker = flightPathMarker(vec3(0, 0, -1), straight, VIEWPORT, remPx, shift);
      const centre = marker?.anchor ?? at;
      return [
        px(Math.min(...radii(ticks, at)) - 6),
        // The circle's 24 sides end on its radius; the wings and fin start there.
        px(Math.min(...radii(marker, centre)) - 0.375 * remPx),
      ];
    });
    expect(out).toEqual(RATIOS.map((ratio) => [0, 0].map(() => px(markShiftDevicePx(ratio)))));
  });
});

/** The view's strokes for the line-of-sight mark at a device-pixel ratio, device px. */
function tripodStrokesAt(ratio: number): LineOfSightStrokes {
  return {
    markStrokePx: markStrokeDevicePx(ratio),
    markShiftPx: markShiftDevicePx(ratio),
  };
}

/** Fails the test where a mark that should have been made was not. */
function absent(what: string): never {
  throw new Error(`${what} was not made`);
}

/** The view's centre, where the line of sight falls. */
const CENTRE = { xPx: 960, yPx: 540 };

/**
 * Each tick's inner and outer ends' distances from `at`, px, its direction out from `at`, degrees
 * clockwise from straight up, and how far its line passes from `at`, px.
 */
function ticksAbout(
  mark: ScreenMark,
  at: { xPx: number; yPx: number },
): Array<{ innerPx: number; outerPx: number; angleDeg: number; offLinePx: number }> {
  return mark.segments.map(([inner, outer]) => {
    const dx = outer.xPx - inner.xPx;
    const dy = outer.yPx - inner.yPx;
    return {
      innerPx: Math.hypot(inner.xPx - at.xPx, inner.yPx - at.yPx),
      outerPx: Math.hypot(outer.xPx - at.xPx, outer.yPx - at.yPx),
      angleDeg: (Math.atan2(dx, -dy) * 180) / Math.PI,
      offLinePx:
        Math.abs(dx * (at.yPx - inner.yPx) - dy * (at.xPx - inner.xPx)) / Math.hypot(dx, dy),
    };
  });
}

/** The distance, px, from `p` to the segment `s`, by its foot on the segment's line, clamped. */
function pointToSegmentPx(p: { xPx: number; yPx: number }, s: ScreenSegment): number {
  const [a, b] = s;
  const dx = b.xPx - a.xPx;
  const dy = b.yPx - a.yPx;
  const t = Math.min(
    1,
    Math.max(0, ((p.xPx - a.xPx) * dx + (p.yPx - a.yPx) * dy) / (dx * dx + dy * dy)),
  );
  return Math.hypot(a.xPx + t * dx - p.xPx, a.yPx + t * dy - p.yPx);
}

/** How far clockwise the direction `aDeg` stands from `bDeg`, whole degrees from 0 to 359. */
function degreesApart(aDeg: number, bDeg: number): number {
  return Math.round((((aDeg - bDeg) % 360) + 360) % 360);
}

/**
 * How far from its centreline a tick's coloured stroke lights a pixel, device px, as `lines.wgsl`
 * lights it: half the stroke and the fringe, its casing apart (decision-r10-t12-commit3 §3).
 */
function strokeReachPx(strokes: LineOfSightStrokes): number {
  return strokes.markStrokePx / 2 + 0.5;
}

/** A point on the view, px. */
function pointPx(xPx: number, yPx: number): { xPx: number; yPx: number } {
  return { xPx, yPx };
}

describe("the line-of-sight mark, the tripod (R10.T12)", () => {
  it("draws three --text ticks pointing at the point, one from below and two from above at 60° to the vertical", () => {
    // At a ratio of 2 and 100%, where neither the shift nor the open centre moves it.
    const mark = lineOfSightTripod(CENTRE, 32, tripodStrokesAt(2));
    const ticks = ticksAbout(mark, CENTRE);
    expect({
      kind: mark.kind,
      token: mark.token,
      target: mark.target,
      anchor: mark.anchor,
      angles: ticks.map((tick) => Math.round(tick.angleDeg * 1e9) / 1e9),
      onTheirLines: ticks.every((tick) => tick.offLinePx < 1e-9),
    }).toEqual({
      kind: "line_of_sight",
      token: "text",
      target: null,
      anchor: CENTRE,
      angles: [180, -60, 60],
      onTheirLines: true,
    });
  });

  it("sets its ticks 120° apart", () => {
    const [below, left, right] = ticksAbout(
      lineOfSightTripod(CENTRE, 32, tripodStrokesAt(2)),
      CENTRE,
    );
    expect([
      degreesApart(below?.angleDeg ?? 0, right?.angleDeg ?? 0),
      degreesApart(right?.angleDeg ?? 0, left?.angleDeg ?? 0),
      degreesApart(left?.angleDeg ?? 0, below?.angleDeg ?? 0),
    ]).toEqual([120, 120, 120]);
  });

  it("starts its ticks 0.1875 rem from the point and ends them at 0.4375 rem", () => {
    const ticks = ticksAbout(lineOfSightTripod(CENTRE, 32, tripodStrokesAt(2)), CENTRE);
    expect(ticks.map((tick) => [px(tick.innerPx), px(tick.outerPx)])).toEqual([
      [6, 14],
      [6, 14],
      [6, 14],
    ]);
  });

  it("moves its ticks out by the outline's shift, keeping their length, as a craft's are", () => {
    // At a ratio of 1 and the 200% interface scale the open centre does not bind: δ is 0.25 px.
    const ticks = ticksAbout(lineOfSightTripod(CENTRE, 32, tripodStrokesAt(1)), CENTRE);
    expect(ticks.map((tick) => [px(tick.innerPx), px(tick.outerPx)])).toEqual([
      [6.25, 14.25],
      [6.25, 14.25],
      [6.25, 14.25],
    ]);
  });

  it.each([
    // [ratio, the interface's rem in CSS px (80% and 100%), the ticks' start, device px]: 0.1875 rem
    // and δ, the casings free to meet over the point (decision-r10-t12-commit3 §3).
    [1, 12.8, 0.1875 * 12.8 + 0.25],
    [1, 16, 0.1875 * 16 + 0.25],
    [2, 12.8, 0.1875 * 25.6],
    [2, 16, 0.1875 * 32],
  ])(
    "keeps the point open at a ratio of %d and a rem of %d px, its ticks 0.25 rem long",
    (ratio, remCssPx, innerPx) => {
      const remPx = remCssPx * ratio;
      const strokes = tripodStrokesAt(ratio);
      const mark = lineOfSightTripod(CENTRE, remPx, strokes);
      // The pixel that holds the point, wherever the point falls on the grid: at a corner of four
      // pixels, at a pixel's centre and between.
      const unlit = [0, 0.5, 0.3].every((fraction) => {
        const at = { xPx: 960 + fraction, yPx: 540 + (fraction === 0.3 ? 0.8 : fraction) };
        const tripod = lineOfSightTripod(at, remPx, strokes);
        const pixel = { xPx: Math.floor(at.xPx) + 0.5, yPx: Math.floor(at.yPx) + 0.5 };
        // At a corner, every one of the four pixels about it.
        const pixels =
          fraction === 0
            ? [-1, 0].flatMap((i) =>
                [-1, 0].map((j) => ({ xPx: 960 + i + 0.5, yPx: 540 + j + 0.5 })),
              )
            : [pixel];
        return pixels.every((p) =>
          tripod.segments.every(
            (segment) => pointToSegmentPx(p, segment) >= strokeReachPx(strokes),
          ),
        );
      });
      const ticks = ticksAbout(mark, CENTRE);
      expect({
        inner: ticks.map((tick) => px(tick.innerPx)),
        lengthRem: ticks.map((tick) => px((tick.outerPx - tick.innerPx) / remPx)),
        unlit,
      }).toEqual({
        inner: [innerPx, innerPx, innerPx].map(px),
        lengthRem: [0.25, 0.25, 0.25],
        unlit: true,
      });
    },
  );

  it.each([0.78125, 1, 1.25, 2, 3])(
    "never needs its guard at a ratio of %d from the 80% interface scale up, 0.1875 rem and the shift keeping the point open",
    (ratio) => {
      const strokes = tripodStrokesAt(ratio);
      // The guard: the stroke's reach and half a pixel's diagonal (decision-r10-t12-commit3 §3).
      const guardPx = strokeReachPx(strokes) + Math.SQRT1_2;
      const starts = [12.8, 16, 20, 24].map(
        (remCssPx) => 0.1875 * remCssPx * ratio + markShiftDevicePx(ratio),
      );
      expect(starts.every((startPx) => startPx >= guardPx)).toBe(true);
    },
  );

  it("gives its box: its ticks' ends widened by half its stroke, its casing and the fringe", () => {
    const mark = lineOfSightTripod(CENTRE, 32, tripodStrokesAt(2));
    const reach = 1.5 + 2 + 0.5;
    const across = 14 * Math.sin(Math.PI / 3);
    const box = markBoxPx(mark, reach);
    expect([box.leftPx, box.topPx, box.widthPx, box.heightPx].map(px)).toEqual(
      [960 - across - reach, 540 - 7 - reach, 2 * (across + reach), 21 + 2 * reach].map(px),
    );
  });
});

describe("the least distance between two strokes on the view (R10.T12)", () => {
  it("is 0 where they cross", () => {
    expect(
      segmentDistancePx([pointPx(0, 0), pointPx(10, 10)], [pointPx(0, 10), pointPx(10, 0)]),
    ).toBe(0);
  });

  it("is the gap between two parallel strokes", () => {
    expect(segmentDistancePx([pointPx(0, 0), pointPx(10, 0)], [pointPx(2, 3), pointPx(8, 3)])).toBe(
      3,
    );
  });

  it("is an end's distance from the middle of the other", () => {
    expect(segmentDistancePx([pointPx(0, 0), pointPx(10, 0)], [pointPx(5, 2), pointPx(5, 9)])).toBe(
      2,
    );
  });

  it("is the gap between the near ends of two strokes on one line", () => {
    expect(segmentDistancePx([pointPx(0, 0), pointPx(4, 0)], [pointPx(7, 0), pointPx(12, 0)])).toBe(
      3,
    );
  });
});

describe("the line-of-sight mark's give-way (R10.T12; the guide's tripod, beside other marks)", () => {
  // At a ratio of 2 and 100%: the tripod's ticks from 6 to 14 px, the outlines not moved.
  const REM = 32;
  const strokes = tripodStrokesAt(2);
  const tripod = lineOfSightTripod(CENTRE, REM, strokes);
  const CRAFT: CameraTarget = { kind: "craft", craft: "other" };
  const straight: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

  /**
   * Each kind of mark that makes it give way, directly below the point, its nearest stroke
   * `clearRem` from the end of the tripod's lower tick (14 px down): a craft's ticks, its upper
   * tick ending 12 + 8 px above its anchor; a body's circle, its top 12 px above its centre; and
   * the flight path marker, its fin's tip 12 + 10 px above its centre.
   */
  const below: ReadonlyArray<readonly [string, (clearRem: number) => ScreenMark]> = [
    [
      "a craft's ticks",
      (clearRem) =>
        targetMark(
          CRAFT,
          { xPx: 960, yPx: 540 + 14 + 20 + clearRem * REM },
          contactRadiusPx(REM),
          vec3(1e3, 0, 0),
          null,
          0,
        ),
    ],
    [
      "a body's symbol",
      (clearRem) =>
        bodySymbolMark(
          PLANET,
          bodyKindSymbol("star"),
          { xPx: 960, yPx: 540 + 14 + 12 + clearRem * REM },
          1,
          REM,
          0,
        ) ?? absent("the body's symbol"),
    ],
    [
      "the flight path marker",
      (clearRem) => {
        // Below the view's centre by y px: along (0, −y ÷ (s W ÷ 2), −1), s = 1 ÷ tan 30°.
        const yPx = 14 + 22 + clearRem * REM;
        const t = yPx / (Math.sqrt(3) * 960);
        return (
          flightPathMarker(vec3(0, -t, -1), straight, VIEWPORT, REM, 0) ??
          absent("the flight path marker")
        );
      },
    ],
  ];

  it.each(below)("measures %s's clearance from its ticks, centreline to centreline", (_, at) => {
    for (const clearRem of [0.3, 0.6, 1]) {
      expect(lineOfSightClearancePx(tripod.segments, [at(clearRem)]) / REM).toBeCloseTo(
        clearRem,
        9,
      );
    }
  });

  it.each(below)(
    "leaves for %s within 0.5 rem, and returns only once it is 0.75 rem clear",
    (_, at) => {
      const givesWay = (clearRem: number, yielded: boolean): boolean =>
        lineOfSightGivesWay(tripod.segments, [at(clearRem)], REM, yielded);
      expect({
        leavesWithin: givesWay(0.49, false),
        staysBeyond: givesWay(0.51, false),
        heldInTheBand: [givesWay(0.6, false), givesWay(0.6, true)],
        stillLeftAt: givesWay(0.74, true),
        returnsAt: givesWay(0.76, true),
      }).toEqual({
        leavesWithin: true,
        staysBeyond: false,
        heldInTheBand: [false, true],
        stillLeftAt: true,
        returnsAt: false,
      });
    },
  );

  it("never gives way to the reticles that stand round it", () => {
    const reticles = [
      bracketReticle(PLANET, CENTRE, symbolRadiusPx(bodyKindSymbol("planet"), REM), REM, 0),
      destinationReticle(PLANET, CENTRE, symbolRadiusPx(bodyKindSymbol("planet"), REM), REM, 0, 5),
    ];
    expect([
      lineOfSightGivesWay(tripod.segments, reticles, REM, false),
      lineOfSightGivesWay(tripod.segments, reticles, REM, true),
    ]).toEqual([false, false]);
  });

  it("returns at once where no mark is near it", () => {
    expect(lineOfSightGivesWay(tripod.segments, [], REM, true)).toBe(false);
  });

  /**
   * A craft's hull as the tripod gives way to it (decision-r10-t12-commit3 §1): one edge across
   * the view below the point, `clearRem` from the end of the lower tick (14 px down), and whether
   * one of its opaque faces stands over the point.
   */
  function hullBelow(clearRem: number, coversPoint: boolean): HullOnView {
    const yPx = 540 + 14 + clearRem * REM;
    return {
      edges: [
        [
          { xPx: 800, yPx },
          { xPx: 1120, yPx },
        ],
      ],
      coversPoint,
    };
  }

  it("stands 0 clear of a hull whose opaque face covers the point, its edges 2 rem away", () => {
    const clearRem = (hull: HullOnView): number =>
      Math.round((lineOfSightClearancePx(tripod.segments, [], [hull]) / REM) * 1e9) / 1e9;
    expect([clearRem(hullBelow(2, true)), clearRem(hullBelow(2, false))]).toEqual([0, 2]);
  });

  it.each(below)("measures a hull's edges as it measures %s's strokes", (_, at) => {
    const mark = at(0.6);
    expect(
      lineOfSightClearancePx(tripod.segments, [], [{ edges: mark.segments, coversPoint: false }]),
    ).toBe(lineOfSightClearancePx(tripod.segments, [mark]));
  });

  it("takes the nearer of the marks and the hulls", () => {
    const [, craftTicks] = below[0] ?? absent("the craft's ticks");
    expect(
      lineOfSightClearancePx(tripod.segments, [craftTicks(1)], [hullBelow(0.3, false)]) / REM,
    ).toBeCloseTo(0.3, 9);
  });

  it("leaves for a hull's edge within 0.5 rem, and returns only once it is 0.75 rem clear", () => {
    const givesWay = (clearRem: number, yielded: boolean): boolean =>
      lineOfSightGivesWay(tripod.segments, [], REM, yielded, [hullBelow(clearRem, false)]);
    expect({
      leavesWithin: givesWay(0.4, false),
      heldInTheBand: [givesWay(0.6, false), givesWay(0.6, true)],
      returnsAt: givesWay(0.8, true),
    }).toEqual({ leavesWithin: true, heldInTheBand: [false, true], returnsAt: false });
  });
});
