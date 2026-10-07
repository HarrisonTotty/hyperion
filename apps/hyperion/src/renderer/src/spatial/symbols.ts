import type { SizeClass, SymbolShape } from "./marks";

/** A point of a unit outline, in screen orientation: +x right, +y down. */
export interface OutlinePoint {
  readonly x: number;
  readonly y: number;
}

/**
 * A symbol's outline at unit radius: a circle; a closed polygon whose last point repeats its
 * first; or a ringed circle, a disc of `discRadius` inside a ring at unit radius, of which the disc
 * alone takes the fill.
 */
export type SymbolOutline =
  | { readonly kind: "circle" }
  | { readonly kind: "polygon"; readonly points: ReadonlyArray<OutlinePoint> }
  | { readonly kind: "ringed-circle"; readonly discRadius: number };

/**
 * Diameter of a mark of each size class, in `rem`, so that marks follow the interface scale.
 *
 * @remarks
 * The smallest is large enough that an open outline still reads as open at 80% scale: with the
 * 1.5 px outline drawn inside the diameter it leaves a 5 px hole at 100% (plan 05, D14).
 */
export const SIZE_CLASS_REM: Readonly<Record<SizeClass, number>> = {
  0: 0.5,
  1: 0.625,
  2: 0.75,
  3: 0.875,
  4: 1,
};

/** Width of a symbol's outline, drawn inside its diameter. */
export const SYMBOL_STROKE_PX = 1.5;

const SQRT3_2 = Math.sqrt(3) / 2;
const HALF_SQRT2 = Math.SQRT1_2;

function closed(points: ReadonlyArray<OutlinePoint>): ReadonlyArray<OutlinePoint> {
  const first = points[0];
  return first === undefined ? points : [...points, first];
}

const DIAMOND = closed([
  { x: 0, y: -1 },
  { x: 1, y: 0 },
  { x: 0, y: 1 },
  { x: -1, y: 0 },
]);

const SQUARE = closed([
  { x: -HALF_SQRT2, y: -HALF_SQRT2 },
  { x: HALF_SQRT2, y: -HALF_SQRT2 },
  { x: HALF_SQRT2, y: HALF_SQRT2 },
  { x: -HALF_SQRT2, y: HALF_SQRT2 },
]);

// Point up, with its centroid on the centre so that it sits on its stalk like the others.
const TRIANGLE = closed([
  { x: 0, y: -1 },
  { x: SQRT3_2, y: 0.5 },
  { x: -SQRT3_2, y: 0.5 },
]);

// The triangle turned over, point down, with the same centroid and so the same hit area (plan 13,
// P13.T8.c).
const TRIANGLE_DOWN = closed([
  { x: 0, y: 1 },
  { x: -SQRT3_2, y: -0.5 },
  { x: SQRT3_2, y: -0.5 },
]);

/**
 * The corners of a regular polygon on the unit circle, the first at `firstDeg` measured clockwise
 * on the screen from straight up, and each next one clockwise from it.
 */
function regularPolygon(corners: number, firstDeg: number): ReadonlyArray<OutlinePoint> {
  return closed(
    Array.from({ length: corners }, (_, index) => {
      const angle = ((firstDeg + (360 * index) / corners) * Math.PI) / 180;
      return { x: Math.sin(angle), y: -Math.cos(angle) };
    }),
  );
}

// Point up, as the triangle is, so that at a small size it reads apart from the hexagon, whose
// sides are flat at the top and bottom (plan 14, T40.a).
const PENTAGON = regularPolygon(5, 0);

// Corners at the left and right and flat at the top and bottom, apart from the pentagon's point.
const HEXAGON = regularPolygon(6, 30);

/**
 * How far out the ringed circle's disc reaches, as a share of its ring's radius.
 *
 * @remarks
 * A third splits what the ring leaves inside it evenly between the gap round the disc, which tells
 * a filled ringed circle from a filled circle, and the hole in the open disc, which tells it open
 * from filled (plan 06, D17). At size class 2 both are 2 px across at 100% and 1.2 px at 80%. At
 * the smallest class no split leaves both a pixel wide, since two outlines already take 6 of its 8
 * px, so a ringed circle is drawn at size class 2 or more: `starSizeClass` in
 * `lib/galaxy/starSymbols.ts` raises it there (the orchestrator's ruling 35.4).
 */
const RINGED_DISC_SHARE = 1 / 3;

// A record over every shape, so that adding a shape to `SymbolShape` is a compile error here until
// it is drawn.
const OUTLINES: Readonly<Record<SymbolShape, SymbolOutline>> = {
  circle: { kind: "circle" },
  diamond: { kind: "polygon", points: DIAMOND },
  square: { kind: "polygon", points: SQUARE },
  triangle: { kind: "polygon", points: TRIANGLE },
  "ringed-circle": { kind: "ringed-circle", discRadius: RINGED_DISC_SHARE },
  "triangle-down": { kind: "polygon", points: TRIANGLE_DOWN },
  pentagon: { kind: "polygon", points: PENTAGON },
  hexagon: { kind: "polygon", points: HEXAGON },
};

/**
 * The unit outline of a symbol shape, centred on the origin and inside the unit circle.
 *
 * @remarks
 * The painter scales it to the mark's radius.
 */
export function symbolOutline(shape: SymbolShape): SymbolOutline {
  return OUTLINES[shape];
}

/**
 * The distance from the centre of a closed unit outline to its nearest side: the unit polygon's
 * inradius, `cos(π ÷ n)` for a regular n-gon (0.5 for the triangles, 0.707 for the square and the
 * diamond, 0.809 for the pentagon and 0.866 for the hexagon).
 *
 * @remarks
 * Both drawers move a polygon's outline out by an outline shift δ on every side by moving its
 * corners δ over this (decision-r07-t16d-followups, (a)): the view's `bodySymbolMark` and the
 * spatial displays' `paint`, so that the legend, the picture and the view agree. That is exact only
 * for an outline whose sides all lie this far from its centre, as every polygon here does (each is
 * regular about its centre, the triangles about their centroids). An outline added later whose
 * sides do not, such as a four-point star, needs each side offset instead.
 */
export function unitInradius(points: ReadonlyArray<OutlinePoint>): number {
  let nearest = Infinity;
  for (let i = 1; i < points.length; i += 1) {
    const a = points[i - 1];
    const b = points[i];
    if (a !== undefined && b !== undefined) {
      const length = Math.hypot(b.x - a.x, b.y - a.y);
      if (length > 0) {
        nearest = Math.min(nearest, Math.abs(a.x * b.y - a.y * b.x) / length);
      }
    }
  }
  return nearest;
}

/** How much of each side of its square the selection's bracket's corner arm covers: a third. */
export const BRACKET_ARM_SHARE = 1 / 3;

/**
 * The length of each corner arm of a bracket of half-size `halfSizePx`, px: a third of its side.
 * The view's bracket and the spatial displays' both take it, and so do the destination's chevrons,
 * whose arms are as long as the bracket's about the same mark.
 */
export function bracketArmPx(halfSizePx: number): number {
  return 2 * halfSizePx * BRACKET_ARM_SHARE;
}

/** A point px from a mark's centre, in screen orientation: +x right, +y down. */
export interface OffsetPx {
  readonly xPx: number;
  readonly yPx: number;
}

/**
 * One of the destination's chevrons: the outer end of one arm, the apex, and the outer end of the
 * other arm, from the mark's centre, traced in that order as one open stroke.
 */
export type Chevron = readonly [OffsetPx, OffsetPx, OffsetPx];

/** The screen axes from a mark outward, in the order the chevrons are given: up, down, left, right. */
const CHEVRON_AXES: ReadonlyArray<OutlinePoint> = [
  { x: 0, y: -1 },
  { x: 0, y: 1 },
  { x: -1, y: 0 },
  { x: 1, y: 0 },
];

/**
 * The mark of a commanded destination: four open chevrons, one on each screen axis of the mark,
 * above, below, left and right, each pointing at it (decision-r07-quality-and-destination, Q2;
 * plan R07, R07.T16.h).
 *
 * @remarks
 * Each chevron's apex lies on its axis `apexPx` from the mark's centre, and its two arms run away
 * from the mark at 45° either side of the axis, each `armPx` long, so that it points in at what the
 * ship is to reach. The arms are open strokes, never filled, closed or dashed. They stand on the
 * cardinal points outside the selection's bracket, where its corners leave the middle of each side
 * open, so that the destination is told from the selection by its shape, in every colour state.
 * Both drawers take it: the view's symbology and the spatial displays' painter.
 *
 * @param apexPx - The apices' distance from the mark's centre: the destination's half-size, the
 *   least gap outside the bracket's place.
 * @param armPx - Each arm's length: the bracket's arm about the same mark ({@link bracketArmPx}).
 */
export function destinationChevrons(apexPx: number, armPx: number): ReadonlyArray<Chevron> {
  const run = armPx * Math.SQRT1_2;
  return CHEVRON_AXES.map((axis): Chevron => {
    // The axis turned a quarter, across it; each arm runs out along the axis and across it equally.
    const across = { x: -axis.y, y: axis.x };
    const apex = { xPx: axis.x * apexPx, yPx: axis.y * apexPx };
    const end = (side: number): OffsetPx => ({
      xPx: apex.xPx + (axis.x + side * across.x) * run,
      yPx: apex.yPx + (axis.y + side * across.y) * run,
    });
    return [end(-1), apex, end(1)];
  });
}

/**
 * How far along a screen axis from a mark's centre the outermost reticle that can stand about it
 * reaches, to the reticle's line, px: the selection's bracket's half-size, or while the mark is the
 * destination, its chevrons' reach, the apex distance and an arm's run outward, `armPx` ÷ √2
 * (decision-r07-quality-and-destination, Q3).
 *
 * @remarks
 * The bracket is counted whether or not the mark is selected, so that selecting a mark never moves
 * its label. Each display stands its labels clear of it: a label's text starts at least 0.125 rem
 * outside the reticle's outer edge, its line plus half the mark stroke. The view's
 * `markLabelOffsetPx` and the spatial displays' `placeLabels` both take it.
 *
 * @param bracketHalfSizePx - The selection's bracket's half-size about the mark, as drawn.
 * @param destinationApexPx - While the mark is the destination, its chevrons' apex distance, as
 *   drawn; `null` while it is not.
 */
export function reticleReachPx(
  bracketHalfSizePx: number,
  destinationApexPx: number | null,
): number {
  return destinationApexPx === null
    ? bracketHalfSizePx
    : destinationApexPx + bracketArmPx(bracketHalfSizePx) * Math.SQRT1_2;
}
