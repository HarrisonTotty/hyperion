import { RETICLE_SHIFTS, type ReticleStrokesCss } from "../lib/strokes";
import type { Viewport } from "./camera";
import { type Anchor, destinationGapPx, reticleHalfSizePx } from "./drawList";
import type { PointMark } from "./marks";
import {
  boxGapPx,
  type DestinationLabelPlace,
  destinationLabelBoxPx,
  destinationLabelPlace,
  destinationLabelRisePx,
  destinationSetReachPx,
  LABEL_EDGE_CLEARANCE_REM,
  LABEL_NEIGHBOUR_CLEARANCE_REM,
  LABEL_TEXT_CLEARANCE_REM,
  placeIsRight,
  type ScreenBoxPx,
  squareBoxPx,
} from "./symbols";

/** A box on the screen, in CSS pixels from the view's top left (`spatial/symbols.ts`'s). */
export type BoxPx = ScreenBoxPx;

/** A mark label placed over the view, in CSS pixels from the view's top left. */
export interface PlacedLabel {
  readonly id: string;
  readonly text: string;
  readonly leftPx: number;
  readonly topPx: number;
  readonly widthPx: number;
  readonly heightPx: number;
  /** Which side of its symbol the label sits on. */
  readonly side: "right" | "left";
}

/** Size of mark labels, and of all text over a spatial view, in `rem`. */
export const LABEL_FONT_REM = 0.875;
// jsdom cannot measure text, so a label's box is estimated from its length: B612 averages about
// 0.62 em a character in upper case. Its height is exact: one line at the stylesheet's
// `.spatial-label` `line-height` of 1.25, so that a raised destination's label stands by its
// bottom edge where it is placed.
const CHARACTER_EM = 0.62;
const LINE_EM = 1.25;

/**
 * How far inside a line of text's box, above and below, the ink of its capitals and figures lies,
 * in ems of its font size: the band in which the selection's label and the other text over the
 * view are tested for overlap, so that glyphs over glyphs are refused and lines that only share
 * their leading are not (plan 12, P12.T11.e).
 *
 * @remarks
 * From the bundled B612 (`@fontsource/b612` 5.3.0, regular and bold) and B612 Mono
 * (`@fontsource/b612-mono` 5.3.0), whose `head`, `hhea` and `OS/2` tables give 2,000 units per em,
 * an ascender of 1,930, a descender of 500, no line gap and a cap height of 1,500. In a line 1.25 em
 * high ({@link LINE_EM}) the 1.215 em content area stands 0.0175 em below the box's top, so the
 * baseline lies 0.9825 em down and the capitals' tops 0.2325 em down: the capitals and figures span
 * 0.2325 to 0.9825 em of the box. Taking 0.275 em off each edge keeps 0.7 em inside that band. Two
 * lines set one under the other, whose boxes overlap by about 5 px of their 17.5 px at 100% (the
 * orbit map's `HABITABLE ZONE`, a selected planet's label and `SNOW LINE`, at main's landing look
 * at #79 to #81), keep their ink apart and are not overlapping text; a label 1.9 px below the core
 * arrow's (15.6 px of overlap) is. A lower-case unit's descender, the `y` of `ly`, lies below the
 * band.
 */
const INK_INSET_EM = 0.275;

/** The estimated size of a line of text over a spatial view. */
export interface TextSizeRem {
  readonly widthRem: number;
  readonly heightRem: number;
}

/**
 * The estimated box of one line of text set at {@link LABEL_FONT_REM}, from its length.
 *
 * @remarks
 * An estimate, since text cannot be measured before it is laid out (nor at all in jsdom), used to
 * keep labels apart and inside the view.
 *
 * @param letterSpacingEm - The letter spacing the text is set with, in `em`.
 */
export function textSizeRem(text: string, letterSpacingEm = 0): TextSizeRem {
  return {
    widthRem: text.length * (CHARACTER_EM + letterSpacingEm) * LABEL_FONT_REM,
    heightRem: LINE_EM * LABEL_FONT_REM,
  };
}

/**
 * How far along a screen direction the centre of a box must be from a point for the box to reach
 * no nearer the point than its edge: the half-extent of the box along that direction.
 *
 * @param direction - A unit vector on the screen, y downwards.
 */
export function halfExtentRem(
  direction: { readonly x: number; readonly y: number },
  size: TextSizeRem,
): number {
  const across = Math.abs(direction.x) > 0 ? size.widthRem / 2 / Math.abs(direction.x) : Infinity;
  const down = Math.abs(direction.y) > 0 ? size.heightRem / 2 / Math.abs(direction.y) : Infinity;
  return Math.min(across, down);
}

function byPriority(a: PointMark, b: PointMark): number {
  if (a.labelPriority !== b.labelPriority) {
    return b.labelPriority - a.labelPriority;
  }
  if (a.id === b.id) {
    return 0;
  }
  return a.id < b.id ? -1 : 1;
}

/**
 * The marks to label, most important first: the selection, the destination, then the `count`
 * marks of highest `labelPriority` (ties by ID).
 *
 * @remarks
 * The selection and the destination come on top of `count`, so the chart shows at most
 * `count + 2` labels, and `count + 1` while it has no destination.
 */
export function chooseLabels(
  points: ReadonlyArray<PointMark>,
  selectedId: string | null,
  destinationId: string | null,
  count = 8,
): ReadonlyArray<PointMark> {
  const pinnedIds = [selectedId, destinationId].filter((id): id is string => id !== null);
  const pinned: PointMark[] = [];
  for (const id of pinnedIds) {
    const found = points.find((point) => point.id === id);
    if (found !== undefined && !pinned.includes(found)) {
      pinned.push(found);
    }
  }
  const rest = points
    .filter((point) => !pinned.includes(point))
    .toSorted(byPriority)
    .slice(0, Math.max(0, count));
  return [...pinned, ...rest];
}

function overlaps(a: BoxPx, b: BoxPx): boolean {
  return (
    a.leftPx < b.leftPx + b.widthPx &&
    b.leftPx < a.leftPx + a.widthPx &&
    a.topPx < b.topPx + b.heightPx &&
    b.topPx < a.topPx + a.heightPx
  );
}

/**
 * `startPx` moved as little as it takes for a box `sizePx` long to lie `edgePx` inside each end of
 * `0`–`extentPx`, or to `edgePx` where the box is longer than that leaves room for
 * (decision-r07-quality-and-destination, addendum D, D5).
 */
function clampInto(startPx: number, sizePx: number, extentPx: number, edgePx: number): number {
  return Math.max(edgePx, Math.min(startPx, extentPx - edgePx - sizePx));
}

/**
 * A text box narrowed to the ink of its capitals and figures ({@link INK_INSET_EM}), at the labels'
 * size ({@link LABEL_FONT_REM}).
 */
function inkBandPx(box: BoxPx, remPx: number): BoxPx {
  const insetPx = INK_INSET_EM * LABEL_FONT_REM * remPx;
  return {
    leftPx: box.leftPx,
    topPx: box.topPx + insetPx,
    widthPx: box.widthPx,
    heightPx: Math.max(0, box.heightPx - 2 * insetPx),
  };
}

/** How far a label stands inside the view's edges, CSS px: {@link LABEL_EDGE_CLEARANCE_REM}. */
function edgeClearancePx(viewport: Viewport): number {
  return LABEL_EDGE_CLEARANCE_REM * viewport.remPx;
}

/** The selection's bracket's half-size about a mark as `paint` draws it, moved out by 4δ, CSS px. */
function bracketDrawnPx(anchor: Anchor, remPx: number, reticles: ReticleStrokesCss): number {
  return reticleHalfSizePx(anchor.radiusPx, remPx) + RETICLE_SHIFTS * reticles.shiftPx;
}

/**
 * The box of the selection's bracket's place about a mark, CSS px: the bracket as `paint` draws it,
 * moved out by four times the outline shift, to its line's outer edge, half the mark stroke beyond,
 * whether or not the mark is selected.
 *
 * @remarks
 * A label held inside the view about a mark out of it stands {@link LABEL_TEXT_CLEARANCE_REM}
 * outside it (plan 12, P12.T11.e), and so does an annulus's label about every mark (plan 14,
 * P14.T42.d), where a mark's own label starts.
 *
 * @param reticles - The marks' strokes at the display's ratio, CSS px (`reticleStrokesCssPx`).
 */
export function bracketBoxPx(anchor: Anchor, remPx: number, reticles: ReticleStrokesCss): BoxPx {
  return squareBoxPx(anchor, bracketDrawnPx(anchor, remPx, reticles) + reticles.markStrokePx / 2);
}

/**
 * How far from its mark's centre a label's box starts, CSS px: `LABEL_TEXT_CLEARANCE_REM` beyond
 * the outer edge, the line and half the mark stroke, of the selection's bracket, moved out by four
 * times the outline shift, whether or not the mark is selected or the destination
 * (decision-r07-quality-and-destination, Q3 and addendum A). A spatial display's label has no
 * plate, so its box is where its text starts.
 *
 * @remarks
 * It is the mark's radius, 0.5 rem and 0.75 CSS px + 5δ: 3.40, 2.00, 0.75 and 0.75 px past 0.5 rem
 * at ratios 0.78125, 1, 2 and 3.
 */
function labelOffsetPx(anchor: Anchor, remPx: number, reticles: ReticleStrokesCss): number {
  return (
    bracketDrawnPx(anchor, remPx, reticles) +
    reticles.markStrokePx / 2 +
    LABEL_TEXT_CLEARANCE_REM * remPx
  );
}

/** A label's place beside its bracket and its estimated box's size, CSS px. */
interface LabelSizePx {
  /** How far from its mark's centre its near edge stands ({@link labelOffsetPx}). */
  readonly offsetPx: number;
  readonly widthPx: number;
  readonly heightPx: number;
}

/** A mark's label's place beside its bracket and its estimated box's size, CSS px. */
function labelSizePx(
  mark: PointMark,
  anchor: Anchor,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
): LabelSizePx {
  const size = textSizeRem(mark.label);
  return {
    offsetPx: labelOffsetPx(anchor, viewport.remPx, reticles),
    widthPx: size.widthRem * viewport.remPx,
    heightPx: size.heightRem * viewport.remPx,
  };
}

/**
 * A mark's label on its mark's line: to the right of its symbol, flipped to the left where it would
 * end within 0.25 rem of the right edge of the view, and held 0.25 rem inside the view where it has
 * room on neither side, or at the top or bottom edge (the orchestrator's ruling 149.1;
 * decision-r07-quality-and-destination, addendum D, D5).
 *
 * @remarks
 * Held inside on neither side, the box runs back across its own mark and bracket.
 * {@link placeLabels} draws it only for a pinned mark out of the view whose bracket it stands
 * 0.25 rem clear of, so that an off-view selection's name stays on show, and never for a mark in
 * view (plan 12, P12.T11.e; {@link besideItsMark}, {@link clearOfItsBracket}). The hold at the top
 * and bottom edges leaves the label beside its mark, and stands.
 *
 * @param side - The side to stand on, as a label made by the selection takes its other side to
 *   yield to the destination's (addendum C, C1); the right, flipped at the edge, when absent.
 */
function lineLabel(
  mark: PointMark,
  anchor: Anchor,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
  side?: PlacedLabel["side"],
): PlacedLabel {
  const { offsetPx, widthPx, heightPx } = labelSizePx(mark, anchor, viewport, reticles);
  const edgePx = edgeClearancePx(viewport);
  const rightLeftPx = anchor.xPx + offsetPx;
  const standsLeft =
    side === undefined ? rightLeftPx + widthPx > viewport.widthPx - edgePx : side === "left";
  const sideLeftPx = standsLeft ? anchor.xPx - offsetPx - widthPx : rightLeftPx;
  return {
    id: mark.id,
    text: mark.label,
    leftPx: clampInto(sideLeftPx, widthPx, viewport.widthPx, edgePx),
    topPx: clampInto(anchor.yPx - heightPx / 2, heightPx, viewport.heightPx, edgePx),
    widthPx,
    heightPx,
    side: standsLeft ? "left" : "right",
  };
}

/**
 * Whether a label on its mark's line stands at its own place beside its mark, its near edge
 * {@link labelOffsetPx} out, and has not been held inside the view back across its bracket.
 *
 * @remarks
 * Only the left and right hold moves the near edge; one held at the top or bottom edge still
 * stands beside its mark. A label whose mark is in view is drawn only where this holds, on either
 * side (plan 12, P12.T11.e).
 */
function besideItsMark(
  label: PlacedLabel,
  anchor: Anchor,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
): boolean {
  const offsetPx = labelOffsetPx(anchor, viewport.remPx, reticles);
  const nearEdgePx = label.side === "right" ? label.leftPx : label.leftPx + label.widthPx;
  const placePx = label.side === "right" ? anchor.xPx + offsetPx : anchor.xPx - offsetPx;
  return Math.abs(nearEdgePx - placePx) < 1e-9;
}

/**
 * Whether a label held inside the view stands at least {@link LABEL_TEXT_CLEARANCE_REM} clear of
 * its mark's bracket's box, to the outer edge of its line, as `paint` draws it.
 *
 * @remarks
 * The canvas draws every mark and the selection's bracket wherever they stand, so a mark whose
 * centre lies just past the view's top or bottom edge still shows part of both. A label held
 * inside the view about a mark out of it is drawn only where it stands this far clear of the
 * bracket, and never where it would run across what is drawn of them (plan 12, P12.T11.e; main's
 * answer on #80). Where the held label spans the bracket across, as a label with room on neither
 * side does, that is once the bracket lies wholly outside the view.
 */
function clearOfItsBracket(
  label: BoxPx,
  anchor: Anchor,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
): boolean {
  const bracket = bracketBoxPx(anchor, viewport.remPx, reticles);
  return boxGapPx(label, bracket) > LABEL_TEXT_CLEARANCE_REM * viewport.remPx - 1e-9;
}

/** The destination's chevron set's apex distance about its mark, as `paint` draws it, CSS px. */
function destinationApexPx(anchor: Anchor, remPx: number, reticles: ReticleStrokesCss): number {
  return bracketDrawnPx(anchor, remPx, reticles) + destinationGapPx(remPx, reticles.minGapPx);
}

/**
 * A destination's label at its place (decision-r07-quality-and-destination, addenda B and C): above
 * or below its whole chevron set, at the right or the left, with its near edge at its place beside
 * the bracket; the first of its four places inside the view, 0.25 rem inside its edges and over no
 * furniture or other text, and at least `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of every other mark,
 * of every other label it counts at its place on its line and of the other text; else the first
 * such place inside the view (`destinationLabelPlace`), P05's furniture and text counting as the
 * chrome of addendum C's second tier. Where none is, it is `null`, not drawn, and is not held
 * inside, where it would stand on its own chevrons (ruling 149.1; addendum C, C3, and D, D5;
 * R07.T16.j).
 *
 * @remarks
 * Every other mark is taken at its bracket's place, its outer edge, which holds its symbol and its
 * reticles, whether or not it is selected; and every label it counts at its place on its line, as
 * the view takes every other plate. Selection is no input to any of them, so selecting never moves
 * the destination's label (addendum C, C1).
 *
 * @param others - The labels it is placed against, each at its place on its mark's line: those
 *   `chooseLabels` would choose with no selection, and of them only the ones a chart with no
 *   selection draws: none whose mark is out of view, and none that would be held back across its
 *   own mark (P12.T11.e).
 * @param obstacles - Furniture it never covers.
 * @param texts - Other text over the view: 0.5 rem off at its first choice, never covered.
 */
function destinationLabel(
  mark: PointMark,
  anchor: Anchor,
  anchors: ReadonlyArray<Anchor>,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
  others: ReadonlyArray<BoxPx>,
  obstacles: ReadonlyArray<BoxPx>,
  texts: ReadonlyArray<BoxPx>,
): PlacedLabel | null {
  const { remPx } = viewport;
  const { offsetPx, widthPx, heightPx } = labelSizePx(mark, anchor, viewport, reticles);
  const risePx = destinationLabelRisePx(
    bracketDrawnPx(anchor, remPx, reticles),
    destinationApexPx(anchor, remPx, reticles),
    reticles.markStrokePx,
    remPx,
  );
  const boxOf = (place: DestinationLabelPlace): BoxPx =>
    destinationLabelBoxPx(place, anchor, offsetPx, risePx, widthPx, heightPx);
  const neighbours = [
    ...anchors
      .filter((other) => other.id !== anchor.id)
      .map((other) => bracketBoxPx(other, remPx, reticles)),
    ...others,
    ...texts,
  ];
  const clearPx = LABEL_NEIGHBOUR_CLEARANCE_REM * remPx;
  const edgePx = edgeClearancePx(viewport);
  // A place is 0.25 rem inside the view's edges (addendum D, D5) and over no furniture or text,
  // which P05 holds as its chrome (addendum C, C3's second tier; R07.T16.j).
  const place = destinationLabelPlace(
    boxOf,
    (box) =>
      box.leftPx >= edgePx &&
      box.topPx >= edgePx &&
      box.leftPx + box.widthPx <= viewport.widthPx - edgePx &&
      box.topPx + box.heightPx <= viewport.heightPx - edgePx &&
      ![...obstacles, ...texts].some((other) => overlaps(box, other)),
    (box) => neighbours.every((other) => boxGapPx(box, other) >= clearPx),
  );
  if (place === null) {
    return null;
  }
  const box = boxOf(place);
  return {
    id: mark.id,
    text: mark.label,
    leftPx: box.leftPx,
    topPx: box.topPx,
    widthPx,
    heightPx,
    side: placeIsRight(place) ? "right" : "left",
  };
}

/**
 * The CSS transform that places a mark's label, from the view's top left, by its near edge, the one
 * towards its mark: its left edge where it stands to the right, and its right edge where it is
 * flipped to the left, moved back by its own rendered width.
 *
 * @remarks
 * {@link placeLabels} estimates a label's width from its length. A flipped label placed by its left
 * edge would, where its text is wider than the estimate, run in towards its mark past the 0.25 rem
 * its text keeps from the bracket about it (decision-r07-quality-and-destination, Q3 and addendum
 * A; R07.T16.h, after the UX review). Placed by its near edge, its text keeps that clearance
 * whatever its width. Its height is exact, one line at the stylesheet's `line-height`, so that its
 * top places a destination's label above its chevron set by its bottom edge as well.
 */
export function markLabelTransform(label: PlacedLabel, remPx: number): string {
  const top = `${String(label.topPx / remPx)}rem`;
  return label.side === "right"
    ? `translate(${String(label.leftPx / remPx)}rem, ${top})`
    : `translate(calc(${String((label.leftPx + label.widthPx) / remPx)}rem - 100%), ${top})`;
}

function inView(anchor: Anchor, viewport: Viewport): boolean {
  return (
    anchor.xPx >= 0 &&
    anchor.xPx <= viewport.widthPx &&
    anchor.yPx >= 0 &&
    anchor.yPx <= viewport.heightPx
  );
}

/** What a spatial display's mark labels are placed among, besides the marks ({@link placeLabels}). */
export interface LabelSurroundings {
  /**
   * Marks whose labels are never dropped for a neighbour, furniture or a mark out of view: the
   * selection and the destination. The selection's still yields to the destination's, keeps off
   * the observer's eye (`observer`) and off the other text over the view (`texts`), whose glyphs
   * its own never overlap, and is never held back across its own mark or bracket (plan 12,
   * P12.T11.e).
   */
  readonly pinnedIds?: ReadonlyArray<string>;
  /** Furniture over the view that a label must not cover: the core arrow's mark. */
  readonly obstacles?: ReadonlyArray<BoxPx>;
  /**
   * Other text over the view, which a label that may be dropped stands 0.5 rem from, as from
   * another mark's label (decision-r07-quality-and-destination, addendum D, D4; R07.T16.j), and
   * whose glyphs the selection's label never overlaps (plan 12, P12.T11.e): the triad's footprint,
   * the core arrow's label and the curve labels.
   */
  readonly texts?: ReadonlyArray<BoxPx>;
  /**
   * The marks `chooseLabels` would label with no selection, whose labels alone the destination's
   * is placed against (addendum C, C1); where absent, every chosen label that can be drawn, as when
   * nothing is selected.
   */
  readonly unselectedIds?: ReadonlySet<string>;
  /**
   * The observer's eye's 1 rem box where it is drawn (`observerBoxPx`; plan 12, P12.T11.c), which
   * every mark's label keeps off, the selection's included; `null` or absent where it is not drawn.
   */
  readonly observer?: BoxPx | null;
}

/**
 * Places the chosen labels beside their symbols, dropping those that would collide.
 *
 * @remarks
 * Each label goes to the right of its symbol and flips to the left where it would end within 0.25
 * rem of the right edge of the view; at the top or bottom edge it is moved 0.25 rem inside the
 * view, so that no label stands on the canvas's focus ring (decision-r07-quality-and-destination,
 * addendum D, D5). Where it has room on neither side, held inside the view it would run back across
 * its own mark and bracket, so while its mark is in view it is not drawn, the selection's too
 * (below): the bracket, the readout and the list still name the mark (plan 12, P12.T11.e). Only the
 * label of a pinned mark out of the view, the selection's, is still moved 0.25 rem inside it (the
 * orchestrator's ruling 149.1), so that its name stays on show, and only where it then stands
 * 0.25 rem clear of the mark's bracket, which the canvas draws in part while the mark's centre is
 * just past the top or bottom edge; else it is not drawn either. One whose box overlaps furniture,
 * or stands within 0.5 rem of a label already placed or of other text over the view (D4), or whose
 * mark is out of view, is dropped. The selection's label is never dropped for those (`pinnedIds`),
 * but yields to the destination's and keeps off the observer's eye and the other text (below).
 *
 * A label's text starts 0.25 rem outside the outer edge of the selection's bracket about its mark,
 * as on every display ({@link labelOffsetPx}; decision-r07-quality-and-destination, Q3 and addendum
 * A), counted whether or not the mark is selected, so that selecting a mark never moves its label.
 * While the mark is the destination its label keeps that place beside the bracket and stands above
 * or below the whole chevron set, at the upper right, the upper left, the lower right or the lower
 * left, the first inside the view and 0.5 rem clear of other marks, labels and text, else the first
 * inside the view over no furniture or text (addendum B; `destinationLabel`), so that no chevron
 * stands on its line or beside its text, where it would read as one of its characters. So the
 * destination's report moves it, and the chevrons never move it outward. Where no place lies 0.25
 * rem inside the view's edges over no furniture or text, it is not drawn, and is not held inside
 * (addendum C, C3, and D, D5).
 *
 * The destination's label is placed first, as though no mark were selected (addendum C, C1;
 * R07.T16.j): against the other marks and against the labels `chooseLabels` would choose with no
 * selection (`unselectedIds`), at their places on their lines, so that selecting never moves it;
 * of those, only the labels a chart with no selection draws, none whose mark is out of view and
 * none that would be held back across its own mark or bracket (P12.T11.e). The selection's label
 * comes next. Where it stands within 0.5 rem of the destination's label or 0.25 rem of its chevron
 * set, overlaps other text over the view by its glyphs (`texts`: the triad's, the core arrow's
 * label, a curve label; their ink bands, {@link INK_INSET_EM}), or would be held back across its
 * own mark or bracket, it takes its other side, and where that is not clear either, or not beside
 * its mark, it is not drawn: the bracket, the readout and the list still name the selection
 * (P12.T11.e: text over text is a wrong picture, so the selection's label yields to the other text
 * where a lesser label is dropped 0.5 rem from it). The rest follow in order. A lesser label within 0.5 rem of
 * the destination's, or within 0.25 rem of its chevron set, is dropped, so that no other name
 * stands nearer the destination's than its own, or runs through its chevrons. The labels are
 * returned in the order given.
 *
 * In observed mode the observer's eye (`observer`) is furniture to every label, the selection's
 * too (plan 12, P12.T11.c): a label over it is dropped, as one over the core arrow is, and the
 * destination's takes the next of its places; the selection's takes its other side where that is
 * clear, and where it is not, it is not drawn, as where it yields to the destination's. A label
 * beside the mark the eye stands on starts outside the eye's box, so it is kept: both are true
 * positions, and the eye is drawn after every mark.
 *
 * @param reticles - The marks' strokes at the display's ratio, CSS px (`reticleStrokesCssPx`).
 * @param destinationId - The destination, whose label stands above or below its chevrons, or `null`.
 * @param surroundings - The pinned marks, the furniture, the observer's eye and the other text the
 *   labels are placed among, and the labels chosen with no selection; none of them when absent.
 */
export function placeLabels(
  chosen: ReadonlyArray<PointMark>,
  anchors: ReadonlyArray<Anchor>,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
  destinationId: string | null,
  surroundings: LabelSurroundings = {},
): ReadonlyArray<PlacedLabel> {
  const {
    pinnedIds = [],
    obstacles: furniture = [],
    texts = [],
    unselectedIds,
    observer = null,
  } = surroundings;
  // The eye is furniture to every label; the selection's, which furniture does not move, keeps off
  // it alone (`coversEye`), and off the other text over the view, so that no glyph stands over
  // another: their ink bands, not their line boxes, are tested (`coversText`; P12.T11.e).
  const obstacles = observer === null ? furniture : [...furniture, observer];
  const coversEye = (label: BoxPx): boolean => observer !== null && overlaps(label, observer);
  const textInk = texts.map((other) => inkBandPx(other, viewport.remPx));
  const coversText = (label: BoxPx): boolean => {
    const ink = inkBandPx(label, viewport.remPx);
    return textInk.some((other) => overlaps(ink, other));
  };
  const anchorOf = new Map(anchors.map((anchor) => [anchor.id, anchor]));
  const isInView = (id: string): boolean => {
    const anchor = anchorOf.get(id);
    return anchor !== undefined && inView(anchor, viewport);
  };
  // Whether a label on its mark's line may be drawn where it stands: beside its mark, or held
  // inside the view for a mark out of it, as a pinned one's is, 0.25 rem clear of its bracket;
  // never held back across its own mark or bracket (P12.T11.e).
  const inPlace = (label: PlacedLabel, anchor: Anchor): boolean =>
    besideItsMark(label, anchor, viewport, reticles) ||
    (!inView(anchor, viewport) && clearOfItsBracket(label, anchor, viewport, reticles));
  // Every other chosen label that can be drawn, at its place on its line: beside its mark, or held
  // inside the view for a pinned mark out of it, 0.25 rem clear of its bracket. Another mark out of
  // view has none, and a label held back across its own mark or bracket is not drawn, so the
  // destination's avoids neither.
  const lineLabels = new Map<string, PlacedLabel>();
  for (const mark of chosen) {
    const anchor = anchorOf.get(mark.id);
    if (
      anchor !== undefined &&
      mark.id !== destinationId &&
      (inView(anchor, viewport) || pinnedIds.includes(mark.id))
    ) {
      const label = lineLabel(mark, anchor, viewport, reticles);
      if (inPlace(label, anchor)) {
        lineLabels.set(mark.id, label);
      }
    }
  }
  // Those the destination's label is placed against: with no selection, a label is drawn only where
  // its mark is in view.
  const unselectedLabels = [...lineLabels.values()].filter(
    (label) => unselectedIds === undefined || (unselectedIds.has(label.id) && isInView(label.id)),
  );
  const destinationAnchor = destinationId === null ? undefined : anchorOf.get(destinationId);
  const destinationMark =
    destinationId === null ? undefined : chosen.find((mark) => mark.id === destinationId);
  const destination =
    destinationMark === undefined || destinationAnchor === undefined
      ? null
      : destinationLabel(
          destinationMark,
          destinationAnchor,
          anchors,
          viewport,
          reticles,
          unselectedLabels,
          obstacles,
          texts,
        );
  const destinationSet =
    destinationAnchor === undefined
      ? null
      : squareBoxPx(
          destinationAnchor,
          destinationSetReachPx(
            bracketDrawnPx(destinationAnchor, viewport.remPx, reticles),
            destinationApexPx(destinationAnchor, viewport.remPx, reticles),
            reticles.markStrokePx,
          ),
        );
  const neighbourPx = LABEL_NEIGHBOUR_CLEARANCE_REM * viewport.remPx;
  const setPx = LABEL_TEXT_CLEARANCE_REM * viewport.remPx;
  // Whether a label stands too near the destination's, or its chevrons, to be drawn there.
  const nearDestination = (label: BoxPx): boolean =>
    (destination !== null && boxGapPx(label, destination) < neighbourPx) ||
    (destinationSet !== null && boxGapPx(label, destinationSet) < setPx);

  const placed = new Map<string, PlacedLabel>();
  if (destination !== null) {
    placed.set(destination.id, destination);
  }
  // Whether the selection's label must leave its place: for the destination's, off the eye, or off
  // the other text over the view.
  const yields = (label: BoxPx): boolean =>
    nearDestination(label) || coversEye(label) || coversText(label);
  // The selection's label, which yields to the destination's and keeps off the observer's eye and
  // the other text, and is never held back across its own mark or bracket: its other side, or none.
  for (const mark of chosen) {
    const anchor = anchorOf.get(mark.id);
    if (anchor === undefined || mark.id === destinationId || !pinnedIds.includes(mark.id)) {
      continue;
    }
    const label = lineLabels.get(mark.id) ?? lineLabel(mark, anchor, viewport, reticles);
    if (inPlace(label, anchor) && !yields(label)) {
      placed.set(mark.id, label);
      continue;
    }
    const other = lineLabel(
      mark,
      anchor,
      viewport,
      reticles,
      label.side === "right" ? "left" : "right",
    );
    // Its other side only where it stands there in full, not held inside the view back across its
    // own bracket (the UX review's must-fix).
    if (!yields(other) && besideItsMark(other, anchor, viewport, reticles)) {
      placed.set(mark.id, other);
    }
  }
  // The rest, each dropped where it would collide.
  for (const mark of chosen) {
    const anchor = anchorOf.get(mark.id);
    if (anchor === undefined || mark.id === destinationId || pinnedIds.includes(mark.id)) {
      continue;
    }
    const label = lineLabels.get(mark.id) ?? lineLabel(mark, anchor, viewport, reticles);
    // In view and beside its mark, and apart from every label placed and every other text by the
    // destination's clearance, since touching labels read as one (addendum D, D4).
    const blocked =
      !inView(anchor, viewport) ||
      !besideItsMark(label, anchor, viewport, reticles) ||
      [...placed.values(), ...texts].some(
        (other) => overlaps(label, other) || boxGapPx(label, other) < neighbourPx,
      ) ||
      obstacles.some((other) => overlaps(label, other)) ||
      nearDestination(label);
    if (!blocked) {
      placed.set(mark.id, label);
    }
  }
  return chosen.flatMap((mark) => placed.get(mark.id) ?? []);
}
