import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { FIXTURE_MOON, FIXTURE_PLANET } from "../../test/viewFixtures";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION, quaternionFromAxisAngle } from "../camera/quaternion";
import type { CameraTarget } from "../camera/state";
import { markShiftDevicePx } from "../../lib/strokes";
import { SIZE_CLASS_REM } from "../../spatial/symbols";
import { bodyKindSymbol } from "../scene/model";
import {
  bodySymbolMark,
  bracketHalfSizePx,
  bracketReticle,
  destinationHalfSizePx,
  destinationReticle,
  flightPathMarker,
  markLabelOffsetPx,
  type ScreenMark,
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
    expect(markLabelOffsetPx(0.375 * REM_PX, REM_PX, shift, null)).toBe(0.75 * REM_PX + 5 * shift);
  });

  it("stands a class-4 symbol's label 0.125 rem and a shift beyond its bracket", () => {
    const radius = 0.5 * REM_PX;
    expect(markLabelOffsetPx(radius, REM_PX, shift, null)).toBe(
      bracketHalfSizePx(radius, REM_PX, shift) + 0.125 * REM_PX + shift,
    );
  });

  it("stands a destination's label 0.125 rem and a shift beyond its chevrons' reach", () => {
    const radius = 0.375 * REM_PX;
    // The reach: the apex distance and an arm's run outward, the bracket's arm over √2.
    const armPx = (2 * bracketHalfSizePx(radius, REM_PX, shift)) / 3;
    expect(markLabelOffsetPx(radius, REM_PX, shift, 4)).toBeCloseTo(
      destinationHalfSizePx(radius, REM_PX, shift, 4) +
        armPx * Math.SQRT1_2 +
        0.125 * REM_PX +
        shift,
      9,
    );
  });

  it("stands a craft's label 22.44 px from its anchor while it is the destination, at a ratio of 1 and 100%", () => {
    // The bracket 11 px out, the apices 15, the reach 15 + 7.33 ÷ √2; the plate 2.25 px beyond.
    expect(Math.round(markLabelOffsetPx(0.375 * REM_PX, REM_PX, shift, 4) * 100) / 100).toBe(22.44);
  });
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
