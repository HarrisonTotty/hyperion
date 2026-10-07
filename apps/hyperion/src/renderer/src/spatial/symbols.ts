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
 * A mark's label's clearance, rem, on every display: 0.25, the layout's base unit
 * (decision-r07-quality-and-destination, addenda A and B). A label's text starts at least this far
 * outside the outer edge of the selection's bracket, its line plus half the mark stroke, counted
 * whether or not the mark is selected; and while the mark is the destination, its label's box
 * stands at least this far above, or below, the ink of the whole chevron set
 * ({@link destinationLabelRisePx}).
 */
export const LABEL_TEXT_CLEARANCE_REM = 0.25;

/**
 * How far a destination's label's box stands clear of every other mark's symbol, reticles and
 * label, rem: 0.5, twice its {@link LABEL_TEXT_CLEARANCE_REM} from its own chevron set, so that
 * nearness names the right mark (decision-r07-quality-and-destination, addendum B).
 */
export const LABEL_NEIGHBOUR_CLEARANCE_REM = 0.5;

/**
 * How far above a mark's centre a destination's label's box stands, px, its bottom edge above the
 * whole chevron set, or its top edge as far below it (decision-r07-quality-and-destination, addendum
 * B): the upper chevron's apex distance, an arm's run outward (`armPx` ÷ √2, 0.47 of the bracket's
 * half-size), half the mark stroke, and {@link LABEL_TEXT_CLEARANCE_REM}.
 *
 * @remarks
 * A chevron that stands on a label's line reads as one of its characters: the right-hand one, its
 * open side towards the text, as a `<` before the designation (addendum A), and the upper one as a
 * `v` (addendum B), and on a stale display each is the label's colour. Between the side chevron's
 * ink and the upper chevron's there is no room for a line of text and its clearance, so the label
 * stands above the whole set, keeping its place beside the bracket; the chevrons never push it
 * outward. Both drawers take it: the view's `markLabelRisePx` and the spatial displays'
 * `placeLabels`. At a ratio of 1 and 100% it is 22.24, 25.19 and 28.13 px about a class-0 mark, a
 * craft and a class-4 mark: for a craft, 15 + 5.19 + 1 + 4.
 *
 * @param bracketHalfSizePx - The selection's bracket's half-size about the mark, as drawn, whose arm
 *   the chevrons' arms are.
 * @param destinationApexPx - The chevrons' apices' distance from the mark's centre, as drawn.
 * @param markStrokePx - The marks' outline width, px.
 * @param remPx - The interface's rem, px.
 */
export function destinationLabelRisePx(
  bracketHalfSizePx: number,
  destinationApexPx: number,
  markStrokePx: number,
  remPx: number,
): number {
  return (
    destinationSetReachPx(bracketHalfSizePx, destinationApexPx, markStrokePx) +
    LABEL_TEXT_CLEARANCE_REM * remPx
  );
}

/**
 * How far along a screen axis from a mark's centre the destination's chevron set's ink reaches, px:
 * the apex distance, an arm's run outward and half the mark stroke, the half-size of the square that
 * holds the whole set (decision-r07-quality-and-destination, addendum B). Another mark's label stands
 * clear of it on the spatial displays, as the destination's own label stands 0.25 rem beyond it.
 *
 * @param bracketHalfSizePx - The selection's bracket's half-size about the mark, as drawn.
 * @param destinationApexPx - The chevrons' apices' distance from the mark's centre, as drawn.
 * @param markStrokePx - The marks' outline width, px.
 */
export function destinationSetReachPx(
  bracketHalfSizePx: number,
  destinationApexPx: number,
  markStrokePx: number,
): number {
  return destinationApexPx + bracketArmPx(bracketHalfSizePx) * Math.SQRT1_2 + markStrokePx / 2;
}

/**
 * The places a destination's label tries, in order: above its whole chevron set at the right and
 * the left, then below it (decision-r07-quality-and-destination, addendum B). The right is the
 * cartographer's first place for a point's name (Imhof 1975), here the set's.
 */
export const DESTINATION_LABEL_PLACES = [
  "upper-right",
  "upper-left",
  "lower-right",
  "lower-left",
] as const;

/** One of a destination's label's places ({@link DESTINATION_LABEL_PLACES}). */
export type DestinationLabelPlace = (typeof DESTINATION_LABEL_PLACES)[number];

// A record over every place, so that adding one is a compile error here until it is placed.
const PLACE_SIDES: Readonly<
  Record<DestinationLabelPlace, { readonly right: boolean; readonly upper: boolean }>
> = {
  "upper-right": { right: true, upper: true },
  "upper-left": { right: false, upper: true },
  "lower-right": { right: true, upper: false },
  "lower-left": { right: false, upper: false },
};

/** Whether a destination's label's place stands to its mark's right, the side a label reads from. */
export function placeIsRight(place: DestinationLabelPlace): boolean {
  return PLACE_SIDES[place].right;
}

/** Whether a destination's label's place stands above its chevron set. */
export function placeIsUpper(place: DestinationLabelPlace): boolean {
  return PLACE_SIDES[place].upper;
}

/** A box on a display, px from its top left: a label's, or a mark's footprint. */
export interface ScreenBoxPx {
  readonly leftPx: number;
  readonly topPx: number;
  readonly widthPx: number;
  readonly heightPx: number;
}

/**
 * A destination's label's box at one of its places, px: its near edge, the one towards its mark,
 * `nearPx` right or left of the mark's centre, and its bottom or top edge `risePx` above or below
 * it ({@link destinationLabelRisePx}).
 *
 * @param centre - The mark's centre, px from the display's top left.
 * @param nearPx - The label's place beside the bracket: how far from the centre its near edge stands.
 */
export function destinationLabelBoxPx(
  place: DestinationLabelPlace,
  centre: OffsetPx,
  nearPx: number,
  risePx: number,
  widthPx: number,
  heightPx: number,
): ScreenBoxPx {
  return {
    leftPx: placeIsRight(place) ? centre.xPx + nearPx : centre.xPx - nearPx - widthPx,
    topPx: placeIsUpper(place) ? centre.yPx - risePx - heightPx : centre.yPx + risePx,
    widthPx,
    heightPx,
  };
}

/** The square of half-size `halfSizePx` about `centre`, px: a mark's footprint. */
export function squareBoxPx(centre: OffsetPx, halfSizePx: number): ScreenBoxPx {
  return {
    leftPx: centre.xPx - halfSizePx,
    topPx: centre.yPx - halfSizePx,
    widthPx: 2 * halfSizePx,
    heightPx: 2 * halfSizePx,
  };
}

/** The least distance between two boxes, px: 0 where they touch or overlap. */
export function boxGapPx(a: ScreenBoxPx, b: ScreenBoxPx): number {
  const across = Math.max(0, a.leftPx - (b.leftPx + b.widthPx), b.leftPx - (a.leftPx + a.widthPx));
  const down = Math.max(0, a.topPx - (b.topPx + b.heightPx), b.topPx - (a.topPx + a.heightPx));
  return Math.hypot(across, down);
}

/**
 * The place a destination's label takes (decision-r07-quality-and-destination, addendum B): the
 * first of {@link DESTINATION_LABEL_PLACES} whose box lies inside the display and is clear, at least
 * {@link LABEL_NEIGHBOUR_CLEARANCE_REM} from every other mark and label; else the first inside the
 * display; else the upper right. The label is never dropped.
 *
 * @remarks
 * Both drawers take it with their own boxes and neighbours: the spatial displays' `placeLabels`,
 * with estimated boxes, and the view's `markLabelPlaces`, with its plates as laid out. A place out
 * of the display is no place, so that where the upper right runs past the display's right or top
 * edge the label takes the upper left or the lower right, as a label flips at the edge.
 *
 * @param boxOf - The label's box at a place.
 * @param inside - Whether a box lies wholly inside the display.
 * @param clear - Whether a box stands clear of every other mark and label.
 */
export function destinationLabelPlace(
  boxOf: (place: DestinationLabelPlace) => ScreenBoxPx,
  inside: (box: ScreenBoxPx) => boolean,
  clear: (box: ScreenBoxPx) => boolean,
): DestinationLabelPlace {
  const shown = DESTINATION_LABEL_PLACES.filter((place) => inside(boxOf(place)));
  return shown.find((place) => clear(boxOf(place))) ?? shown[0] ?? "upper-right";
}
