import type { SizeClass, SymbolShape } from "./marks";

/** A point of a unit outline, in screen orientation: +x right, +y down. */
export interface OutlinePoint {
  readonly x: number;
  readonly y: number;
}

/**
 * A symbol's outline at unit radius: a circle, or a closed polygon whose last point repeats its
 * first.
 */
export type SymbolOutline =
  | { readonly kind: "circle" }
  | { readonly kind: "polygon"; readonly points: ReadonlyArray<OutlinePoint> };

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

// A record over every shape, so that adding a shape to `SymbolShape` is a compile error here until
// it is drawn.
const OUTLINES: Readonly<Record<SymbolShape, SymbolOutline>> = {
  circle: { kind: "circle" },
  diamond: { kind: "polygon", points: DIAMOND },
  square: { kind: "polygon", points: SQUARE },
  triangle: { kind: "polygon", points: TRIANGLE },
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
