import {
  lineScale,
  markShiftDevicePx,
  markStrokeDevicePx,
  RETICLE_SHIFTS,
  RING_SHIFTS,
} from "../lib/strokes";
import type {
  ChevronsOp,
  ColourToken,
  DrawList,
  DrawOp,
  ReticleOp,
  ScreenPoint,
  SymbolOp,
  TicksOp,
} from "./drawList";
import { bracketArmPx, destinationChevrons, symbolOutline, unitInradius } from "./symbols";

/**
 * The colours a spatial view is painted in, read from the stylesheet's tokens: one CSS colour for
 * each {@link ColourToken} a draw op may name, and the background.
 */
export interface ColourTokens extends Readonly<Record<ColourToken, string>> {
  /** `--surface-0`, which the canvas is cleared to. */
  readonly surface0: string;
  /**
   * `--text-muted`, which reference paths and annulus edges are drawn in, and every mark and curve
   * of a stale view ({@link staleTokens}).
   */
  readonly textMuted: string;
}

/** The custom property behind each colour a spatial view uses. */
const TOKEN_PROPERTIES: Readonly<Record<keyof ColourTokens, string>> = {
  text: "--text",
  accent: "--accent",
  target: "--target",
  line: "--line",
  surface0: "--surface-0",
  textMuted: "--text-muted",
};

function isTokenName(name: string): name is keyof ColourTokens {
  return Object.hasOwn(TOKEN_PROPERTIES, name);
}

/** Every colour a spatial view uses, taken from the record that must name each one. */
const TOKEN_NAMES: ReadonlyArray<keyof ColourTokens> =
  Object.keys(TOKEN_PROPERTIES).filter(isTokenName);

/** Whether two colour sets are the same in every token, so that an unchanged set can be kept. */
export function sameTokens(a: ColourTokens, b: ColourTokens): boolean {
  return TOKEN_NAMES.every((name) => a[name] === b[name]);
}

/**
 * Reads the spatial view's colours from the tokens in effect at `element`.
 *
 * @remarks
 * The values are trimmed, since a computed custom property keeps the stylesheet's leading space.
 * The painter never holds a colour of its own (the guide's "always use the tokens").
 *
 * @throws Error naming the token when one is not set, as when the stylesheet is not loaded.
 */
export function readTokens(element: Element): ColourTokens {
  const style = getComputedStyle(element);
  const read = (property: string): string => {
    const value = style.getPropertyValue(property).trim();
    if (value.length === 0) {
      throw new Error(`the colour token ${property} is not set`);
    }
    return value;
  };
  return {
    text: read(TOKEN_PROPERTIES.text),
    accent: read(TOKEN_PROPERTIES.accent),
    target: read(TOKEN_PROPERTIES.target),
    line: read(TOKEN_PROPERTIES.line),
    surface0: read(TOKEN_PROPERTIES.surface0),
    textMuted: read(TOKEN_PROPERTIES.textMuted),
  };
}

/**
 * The same colours for a view whose data is stale: everything a mark or a curve is drawn in reads
 * in `--text-muted`.
 *
 * @remarks
 * The guide shows a stale value in `--text-muted` ("Data states"), and a stale map's picture is
 * already ramped to it (plan 05, T8.d). `--line`, the reference grid, is furniture and not data, so
 * it is left alone, as is the background.
 */
export function staleTokens(tokens: ColourTokens): ColourTokens {
  return {
    ...tokens,
    text: tokens.textMuted,
    accent: tokens.textMuted,
    target: tokens.textMuted,
  };
}

function strokeWith(
  context: CanvasRenderingContext2D,
  tokens: ColourTokens,
  stroke: ColourToken,
  widthPx: number,
): void {
  context.strokeStyle = tokens[stroke];
  context.lineWidth = widthPx;
  context.stroke();
}

function tracePoints(context: CanvasRenderingContext2D, points: ReadonlyArray<ScreenPoint>): void {
  for (const [index, point] of points.entries()) {
    if (index === 0) {
      context.moveTo(point.xPx, point.yPx);
    } else {
      context.lineTo(point.xPx, point.yPx);
    }
  }
}

/**
 * The widths and the outline shift a paint draws at, CSS px, from its pixel ratio
 * (decision-thin-line-contrast, item 2; R07.T16.f).
 */
interface PaintStrokes {
  /** CSS px drawn for each CSS px of a line's width: `lineScale` ÷ the ratio. */
  readonly lineFactor: number;
  /** A symbol's and a reticle's outline: `markStrokeDevicePx` ÷ the ratio. */
  readonly markWidthPx: number;
  /** How far an outline moves out, δ: `markShiftDevicePx` ÷ the ratio. */
  readonly shiftPx: number;
}

function paintStrokesAt(pixelRatio: number): PaintStrokes {
  return {
    lineFactor: lineScale(pixelRatio) / pixelRatio,
    markWidthPx: markStrokeDevicePx(pixelRatio) / pixelRatio,
    shiftPx: markShiftDevicePx(pixelRatio) / pixelRatio,
  };
}

/**
 * A symbol, its outline moved out by the shift on every side, so that its inner edge stays where
 * a 1.5 CSS px outline's would be and every hole it encloses stays as built: a circle's radius by
 * δ, a polygon's corners by δ over its unit inradius (its sides each by δ, as the view's
 * `bodySymbolMark` moves them), and a ringed circle's disc by δ and its ring by 3δ, so that the
 * gap round the disc stays too.
 */
function paintSymbol(
  context: CanvasRenderingContext2D,
  op: SymbolOp,
  tokens: ColourTokens,
  strokes: PaintStrokes,
): void {
  const outline = symbolOutline(op.shape);
  const { xPx, yPx } = op.centre;
  const { shiftPx } = strokes;
  context.beginPath();
  switch (outline.kind) {
    case "circle":
      context.arc(xPx, yPx, op.radiusPx + shiftPx, 0, 2 * Math.PI);
      break;
    case "polygon": {
      const cornerPx = op.radiusPx + shiftPx / unitInradius(outline.points);
      tracePoints(
        context,
        outline.points.map((point) => ({
          xPx: xPx + point.x * cornerPx,
          yPx: yPx + point.y * cornerPx,
        })),
      );
      context.closePath();
      break;
    }
    case "ringed-circle":
      context.arc(xPx, yPx, op.radiusPx * outline.discRadius + shiftPx, 0, 2 * Math.PI);
      break;
  }
  // Filled above the reference plane and open below it, with the same outline (plan 05, D14).
  if (op.fill !== null) {
    context.fillStyle = tokens[op.fill];
    context.fill();
  }
  // The ring joins the path only after the fill, so that the disc alone says which side of the
  // plane the mark is on and the ring still reads round it (plan 06, D17); one stroke draws both.
  if (outline.kind === "ringed-circle") {
    const ringPx = op.radiusPx + RING_SHIFTS * shiftPx;
    context.moveTo(xPx + ringPx, yPx);
    context.arc(xPx, yPx, ringPx, 0, 2 * Math.PI);
  }
  strokeWith(context, tokens, op.stroke, strokes.markWidthPx);
}

/**
 * Four corner brackets of the square of half-width `halfSizePx` about the reticle's centre, moved
 * out by four times the outline shift, as a ringed circle grows.
 */
function paintReticle(
  context: CanvasRenderingContext2D,
  op: ReticleOp,
  tokens: ColourTokens,
  strokes: PaintStrokes,
): void {
  const { xPx, yPx } = op.centre;
  const half = op.halfSizePx + RETICLE_SHIFTS * strokes.shiftPx;
  const arm = bracketArmPx(half);
  context.beginPath();
  for (const [dx, dy] of [
    [-1, -1],
    [1, -1],
    [1, 1],
    [-1, 1],
  ] as const) {
    const cornerX = xPx + dx * half;
    const cornerY = yPx + dy * half;
    context.moveTo(cornerX, cornerY - dy * arm);
    context.lineTo(cornerX, cornerY);
    context.lineTo(cornerX - dx * arm, cornerY);
  }
  strokeWith(context, tokens, op.stroke, strokes.markWidthPx);
}

/**
 * The destination's four chevrons about its mark, each pointing at it: their apices the op's gap
 * outside the bracket's place and each arm the bracket's arm, both moved out with the bracket by
 * four times the outline shift (R07.T16.h). Each is one open stroke, arm, apex, arm, and all four
 * are stroked together.
 */
function paintChevrons(
  context: CanvasRenderingContext2D,
  op: ChevronsOp,
  tokens: ColourTokens,
  strokes: PaintStrokes,
): void {
  const { xPx, yPx } = op.centre;
  const bracketPx = op.bracketHalfSizePx + RETICLE_SHIFTS * strokes.shiftPx;
  context.beginPath();
  for (const chevron of destinationChevrons(bracketPx + op.gapPx, bracketArmPx(bracketPx))) {
    tracePoints(
      context,
      chevron.map((point) => ({ xPx: xPx + point.xPx, yPx: yPx + point.yPx })),
    );
  }
  strokeWith(context, tokens, op.stroke, strokes.markWidthPx);
}

/**
 * The width a `ticks` op is stroked at: a line's at the line scale, or a mark's at the mark stroke,
 * not moved out, since an arrowhead holds nothing to keep (R07.T16.h).
 */
function ticksWidthPx(op: TicksOp, strokes: PaintStrokes): number {
  let widthPx: number;
  switch (op.weight) {
    case "line":
      widthPx = op.widthPx * strokes.lineFactor;
      break;
    case "mark":
      widthPx = strokes.markWidthPx;
      break;
  }
  return widthPx;
}

function paintOp(
  context: CanvasRenderingContext2D,
  op: DrawOp,
  tokens: ColourTokens,
  strokes: PaintStrokes,
): void {
  switch (op.kind) {
    case "line":
      context.beginPath();
      tracePoints(context, [op.from, op.to]);
      strokeWith(context, tokens, op.stroke, op.widthPx * strokes.lineFactor);
      break;
    case "polyline":
      context.beginPath();
      tracePoints(context, op.points);
      strokeWith(context, tokens, op.stroke, op.widthPx * strokes.lineFactor);
      break;
    case "circle":
      context.beginPath();
      context.arc(op.centre.xPx, op.centre.yPx, op.radiusPx, 0, 2 * Math.PI);
      strokeWith(context, tokens, op.stroke, op.widthPx * strokes.lineFactor);
      break;
    case "symbol":
      paintSymbol(context, op, tokens, strokes);
      break;
    case "reticle":
      paintReticle(context, op, tokens, strokes);
      break;
    case "chevrons":
      paintChevrons(context, op, tokens, strokes);
      break;
    case "ticks":
      context.beginPath();
      for (const segment of op.segments) {
        tracePoints(context, [segment.from, segment.to]);
      }
      strokeWith(context, tokens, op.stroke, ticksWidthPx(op, strokes));
      break;
  }
}

/**
 * Paints a draw list onto a canvas: clears it to `--surface-0`, then executes each op in order.
 *
 * @remarks
 * The whole backing store is cleared, whatever its size; the ops, in CSS pixels, are then drawn
 * scaled by `pixelRatio` onto a backing store that many times larger. Circles are arcs, symbols a
 * path scaled from their unit outline, filled and stroked above the plane and only stroked below
 * (a ringed circle's disc alone taking the fill), reticles four corner brackets, and a destination
 * four chevrons pointing at its mark. There is no text on the canvas (plan 05, D15), and nothing is
 * translucent, shadowed or graded.
 *
 * No stroke is narrower than 2 device px (decision-thin-line-contrast, item 2; R07.T16.f): every op
 * but a mark is stroked at its `widthPx` times `lineScale(pixelRatio)` ÷ `pixelRatio`, so that lines
 * keep their ratios to one another, and a mark's outline at `markStrokeDevicePx(pixelRatio)` ÷
 * `pixelRatio`, whatever its op's `widthPx`: a symbol's, a reticle's, the destination's chevrons'
 * and a `ticks` op's whose weight is `mark`, the HR diagram's off-scale arrowhead (R07.T16.h). Where
 * that outline is wider than 1.5 CSS px it widens outward by δ, `markShiftDevicePx(pixelRatio)` ÷
 * `pixelRatio`, so that every hole stays as built: a symbol's outline by δ on every side, a ringed
 * circle's disc by δ and its ring by 3δ, and a reticle's half-size by 4δ, with the chevrons moved
 * out as the bracket is; an arrowhead holds nothing to keep and is not moved. The draw list itself
 * stays in the guide's CSS widths.
 *
 * @param pixelRatio - Backing-store pixels in one CSS pixel, the device pixel ratio. Required, so
 *   that a caller cannot leave a high-density canvas drawn in its top left-hand corner.
 */
export function paint(
  context: CanvasRenderingContext2D,
  drawList: DrawList,
  tokens: ColourTokens,
  pixelRatio: number,
): void {
  context.setTransform(1, 0, 0, 1, 0, 0);
  context.fillStyle = tokens.surface0;
  context.fillRect(0, 0, context.canvas.width, context.canvas.height);
  context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  const strokes = paintStrokesAt(pixelRatio);
  for (const op of drawList.ops) {
    paintOp(context, op, tokens, strokes);
  }
}
