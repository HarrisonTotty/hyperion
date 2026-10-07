import { RETICLE_SHIFTS, type ReticleStrokesCss } from "../lib/strokes";
import type { Viewport } from "./camera";
import { type Anchor, destinationGapPx, reticleHalfSizePx } from "./drawList";
import type { PointMark } from "./marks";
import { reticleReachPx } from "./symbols";

/** A box on the screen, in CSS pixels from the view's top left. */
export interface BoxPx {
  readonly leftPx: number;
  readonly topPx: number;
  readonly widthPx: number;
  readonly heightPx: number;
}

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
// 0.62 em a character in upper case, and a line is about 1.25 em tall.
const CHARACTER_EM = 0.62;
const LINE_EM = 1.25;
/**
 * How far outside the outer edge of the outermost reticle about its mark a label's text starts,
 * rem: 0.125, on every display (decision-r07-quality-and-destination, Q3; R07.T16.h). A spatial
 * display's label has no plate, so its box is where its text starts.
 */
const LABEL_TEXT_CLEARANCE_REM = 0.125;

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

/**
 * How far from its mark's centre a label's box starts, CSS px: {@link LABEL_TEXT_CLEARANCE_REM}
 * beyond the outer edge, the line and half the mark stroke, of the outermost reticle that can stand
 * about the mark (`reticleReachPx`): the selection's bracket, moved out by four times the outline
 * shift, whether or not the mark is selected, or while it is the destination, its chevrons.
 *
 * @remarks
 * For a bracket it is the mark's radius, 0.375 rem and 0.75 CSS px + 5δ: 3.40, 2.00, 0.75 and 0.75
 * px past 0.375 rem at ratios 0.78125, 1, 2 and 3.
 */
function labelOffsetPx(
  anchor: Anchor,
  remPx: number,
  reticles: ReticleStrokesCss,
  isDestination: boolean,
): number {
  const bracketPx = reticleHalfSizePx(anchor.radiusPx, remPx) + RETICLE_SHIFTS * reticles.shiftPx;
  const reachPx = reticleReachPx(
    bracketPx,
    isDestination ? bracketPx + destinationGapPx(remPx, reticles.minGapPx) : null,
  );
  return reachPx + reticles.markStrokePx / 2 + LABEL_TEXT_CLEARANCE_REM * remPx;
}

/**
 * The CSS transform that places a mark's label, from the view's top left, by its near edge, the one
 * towards its mark: its left edge where it stands to the right, and its right edge where it is
 * flipped to the left, moved back by its own rendered width.
 *
 * @remarks
 * {@link placeLabels} estimates a label's width from its length. A flipped label placed by its left
 * edge would, where its text is wider than the estimate, run in towards its mark past the 0.125 rem
 * its text keeps from every reticle about it (decision-r07-quality-and-destination, Q3; R07.T16.h,
 * after the UX review). Placed by its near edge, its text keeps that clearance whatever its width.
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
 * A label's text starts 0.125 rem outside the outer edge of the outermost reticle that can stand
 * about its mark, as on every display ({@link labelOffsetPx}; decision-r07-quality-and-destination,
 * Q3; R07.T16.h): the selection's bracket, counted whether or not the mark is selected, so that
 * selecting a mark never moves its label, and while the mark is the destination, its chevrons, so
 * that the destination's report moves it out.
 *
 * @param reticles - The marks' strokes at the display's ratio, CSS px (`reticleStrokesCssPx`).
 * @param destinationId - The destination, whose label stands beyond its chevrons, or `null`.
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
  const placed: PlacedLabel[] = [];
  for (const mark of chosen) {
    const anchor = anchorOf.get(mark.id);
    if (anchor === undefined) {
      continue;
    }
    const pinned = pinnedIds.includes(mark.id);
    const size = textSizeRem(mark.label);
    const widthPx = size.widthRem * viewport.remPx;
    const heightPx = size.heightRem * viewport.remPx;
    const offsetPx = labelOffsetPx(anchor, viewport.remPx, reticles, mark.id === destinationId);
    const rightLeftPx = anchor.xPx + offsetPx;
    const flips = rightLeftPx + widthPx > viewport.widthPx;
    const sideLeftPx = flips ? anchor.xPx - offsetPx - widthPx : rightLeftPx;
    const label: PlacedLabel = {
      id: mark.id,
      text: mark.label,
      // Where neither side has the room, as in a narrow view, the label is held inside the view
      // rather than cut off by its edge (the orchestrator's ruling 149.1).
      leftPx: clampInto(sideLeftPx, widthPx, viewport.widthPx),
      topPx: clampInto(anchor.yPx - heightPx / 2, heightPx, viewport.heightPx),
      widthPx,
      heightPx,
      side: flips ? "left" : "right",
    };
    const blocked =
      !inView(anchor, viewport) ||
      placed.some((other) => overlaps(label, other)) ||
      obstacles.some((other) => overlaps(label, other));
    if (pinned || !blocked) {
      placed.push(label);
    }
  }
  return placed;
}
