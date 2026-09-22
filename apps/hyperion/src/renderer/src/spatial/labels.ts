import type { Viewport } from "./camera";
import type { Anchor } from "./drawList";
import type { PointMark } from "./marks";

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
const GAP_REM = 0.25;

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
 * Each label goes to the right of its symbol and flips to the left at the right edge of the view.
 * Labels are placed in the order given; one whose box overlaps a label already placed or other
 * furniture, or whose mark is out of view, is dropped. The selection's and the destination's
 * labels, which {@link chooseLabels} puts first, are never dropped.
 *
 * @param pinnedIds - Marks whose labels are never dropped: the selection and the destination.
 * @param obstacles - Other text and furniture over the view that labels must not cover.
 */
export function placeLabels(
  chosen: ReadonlyArray<PointMark>,
  anchors: ReadonlyArray<Anchor>,
  viewport: Viewport,
  pinnedIds: ReadonlyArray<string> = [],
  obstacles: ReadonlyArray<BoxPx> = [],
): ReadonlyArray<PlacedLabel> {
  const gapPx = GAP_REM * viewport.remPx;
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
    const rightLeftPx = anchor.xPx + anchor.radiusPx + gapPx;
    const flips = rightLeftPx + widthPx > viewport.widthPx;
    const label: PlacedLabel = {
      id: mark.id,
      text: mark.label,
      leftPx: flips ? anchor.xPx - anchor.radiusPx - gapPx - widthPx : rightLeftPx,
      topPx: anchor.yPx - heightPx / 2,
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
