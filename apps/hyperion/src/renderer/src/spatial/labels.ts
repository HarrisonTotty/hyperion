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
 * `startPx` moved as little as it takes for a box `sizePx` long to lie inside `0`–`extentPx`, or
 * to `0` where the box is longer than that.
 */
function clampInto(startPx: number, sizePx: number, extentPx: number): number {
  return Math.max(0, Math.min(startPx, extentPx - sizePx));
}

/** The selection's bracket's half-size about a mark as `paint` draws it, moved out by 4δ, CSS px. */
function bracketDrawnPx(anchor: Anchor, remPx: number, reticles: ReticleStrokesCss): number {
  return reticleHalfSizePx(anchor.radiusPx, remPx) + RETICLE_SHIFTS * reticles.shiftPx;
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
 * A mark's label on its mark's line: to the right of its symbol, flipped to the left at the right
 * edge of the view, and held inside the view where it has room on neither side, or at the top or
 * bottom edge (the orchestrator's ruling 149.1).
 */
function lineLabel(
  mark: PointMark,
  anchor: Anchor,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
): PlacedLabel {
  const { offsetPx, widthPx, heightPx } = labelSizePx(mark, anchor, viewport, reticles);
  const rightLeftPx = anchor.xPx + offsetPx;
  const flips = rightLeftPx + widthPx > viewport.widthPx;
  const sideLeftPx = flips ? anchor.xPx - offsetPx - widthPx : rightLeftPx;
  return {
    id: mark.id,
    text: mark.label,
    leftPx: clampInto(sideLeftPx, widthPx, viewport.widthPx),
    topPx: clampInto(anchor.yPx - heightPx / 2, heightPx, viewport.heightPx),
    widthPx,
    heightPx,
    side: flips ? "left" : "right",
  };
}

/** The destination's chevron set's apex distance about its mark, as `paint` draws it, CSS px. */
function destinationApexPx(anchor: Anchor, remPx: number, reticles: ReticleStrokesCss): number {
  return bracketDrawnPx(anchor, remPx, reticles) + destinationGapPx(remPx, reticles.minGapPx);
}

/**
 * A destination's label at its place (decision-r07-quality-and-destination, addendum B): above or
 * below its whole chevron set, at the right or the left, with its near edge at its place beside the
 * bracket; the first of its four places inside the view and at least
 * `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of every other mark and of every other chosen label at its
 * place on its line, and not over furniture (`destinationLabelPlace`). It is held inside the view
 * where no place fits (ruling 149.1).
 *
 * @remarks
 * Every other mark is taken at its bracket's place, its outer edge, which holds its symbol and its
 * reticles, whether or not it is selected; and every other chosen label at its place, whether or not
 * it is placed before this one, as the view takes every other plate. So selecting a mark that has a
 * label never moves the destination's label.
 *
 * @param others - Every other chosen label that can be drawn, at its place on its mark's line.
 */
function destinationLabel(
  mark: PointMark,
  anchor: Anchor,
  anchors: ReadonlyArray<Anchor>,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
  others: ReadonlyArray<BoxPx>,
  obstacles: ReadonlyArray<BoxPx>,
): PlacedLabel {
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
      .map((other) =>
        squareBoxPx(other, bracketDrawnPx(other, remPx, reticles) + reticles.markStrokePx / 2),
      ),
    ...others,
  ];
  const clearPx = LABEL_NEIGHBOUR_CLEARANCE_REM * remPx;
  const place = destinationLabelPlace(
    boxOf,
    (box) =>
      box.leftPx >= 0 &&
      box.topPx >= 0 &&
      box.leftPx + box.widthPx <= viewport.widthPx &&
      box.topPx + box.heightPx <= viewport.heightPx,
    (box) =>
      neighbours.every((other) => boxGapPx(box, other) >= clearPx) &&
      !obstacles.some((other) => overlaps(box, other)),
  );
  const box = boxOf(place);
  return {
    id: mark.id,
    text: mark.label,
    leftPx: clampInto(box.leftPx, widthPx, viewport.widthPx),
    topPx: clampInto(box.topPx, heightPx, viewport.heightPx),
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

/**
 * Places the chosen labels beside their symbols, dropping those that would collide.
 *
 * @remarks
 * Each label goes to the right of its symbol and flips to the left at the right edge of the view;
 * where it has room on neither side, or at the top or bottom edge, it is moved just inside the view.
 * Labels are placed in the order given; one whose box overlaps a label already placed or other
 * furniture, or whose mark is out of view, is dropped. The selection's and the destination's
 * labels, which {@link chooseLabels} puts first, are never dropped.
 *
 * A label's text starts 0.25 rem outside the outer edge of the selection's bracket about its mark,
 * as on every display ({@link labelOffsetPx}; decision-r07-quality-and-destination, Q3 and addendum
 * A), counted whether or not the mark is selected, so that selecting a mark never moves its label.
 * While the mark is the destination its label keeps that place beside the bracket and stands above
 * or below the whole chevron set, at the upper right, the upper left, the lower right or the lower
 * left, the first 0.5 rem clear of other marks and of the other chosen labels (addendum B;
 * `destinationLabel`), so that no chevron stands on its line or beside its text, where it would
 * read as one of its characters. So the destination's report moves it, and the chevrons never move
 * it outward. A lesser label within 0.5 rem of the destination's, or within 0.25 rem of its chevron
 * set, is dropped, so that no other name stands nearer the destination's than its own, or runs
 * through its chevrons.
 *
 * @param reticles - The marks' strokes at the display's ratio, CSS px (`reticleStrokesCssPx`).
 * @param destinationId - The destination, whose label stands above or below its chevrons, or `null`.
 * @param pinnedIds - Marks whose labels are never dropped: the selection and the destination.
 * @param obstacles - Other text and furniture over the view that labels must not cover.
 */
export function placeLabels(
  chosen: ReadonlyArray<PointMark>,
  anchors: ReadonlyArray<Anchor>,
  viewport: Viewport,
  reticles: ReticleStrokesCss,
  destinationId: string | null,
  pinnedIds: ReadonlyArray<string> = [],
  obstacles: ReadonlyArray<BoxPx> = [],
): ReadonlyArray<PlacedLabel> {
  const anchorOf = new Map(anchors.map((anchor) => [anchor.id, anchor]));
  // Every other chosen label that can be drawn, at its place on its line: its mark in view, or
  // pinned. A label whose mark is out of view is dropped, so the destination's never avoids it.
  const lineLabels = new Map<string, PlacedLabel>();
  for (const mark of chosen) {
    const anchor = anchorOf.get(mark.id);
    if (
      anchor !== undefined &&
      mark.id !== destinationId &&
      (inView(anchor, viewport) || pinnedIds.includes(mark.id))
    ) {
      lineLabels.set(mark.id, lineLabel(mark, anchor, viewport, reticles));
    }
  }
  const destinationAnchor = destinationId === null ? undefined : anchorOf.get(destinationId);
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
  const placed: PlacedLabel[] = [];
  let destination: PlacedLabel | null = null;
  for (const mark of chosen) {
    const anchor = anchorOf.get(mark.id);
    if (anchor === undefined) {
      continue;
    }
    const pinned = pinnedIds.includes(mark.id);
    const isDestination = mark.id === destinationId;
    const label = isDestination
      ? destinationLabel(
          mark,
          anchor,
          anchors,
          viewport,
          reticles,
          [...lineLabels.values()],
          obstacles,
        )
      : (lineLabels.get(mark.id) ?? lineLabel(mark, anchor, viewport, reticles));
    // A lesser label gives way to the destination's, and to its chevrons.
    const nearDestination =
      !isDestination &&
      ((destination !== null && boxGapPx(label, destination) < neighbourPx) ||
        (destinationSet !== null && boxGapPx(label, destinationSet) < setPx));
    const blocked =
      !inView(anchor, viewport) ||
      placed.some((other) => overlaps(label, other)) ||
      obstacles.some((other) => overlaps(label, other)) ||
      nearDestination;
    if (pinned || !blocked) {
      placed.push(label);
      if (isDestination) {
        destination = label;
      }
    }
  }
  return placed;
}
