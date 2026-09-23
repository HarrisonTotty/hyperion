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
 * px, so a caller should give a ringed circle size class 2 or more: the smallest a giant is
 * expected to take, which plan 06's record of this task puts to the orchestrator. Nothing here
 * enforces it.
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
