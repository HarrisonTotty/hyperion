import type { ColourToken, DrawList, DrawOp, ReticleOp, ScreenPoint, SymbolOp } from "./drawList";
import { symbolOutline } from "./symbols";

/**
 * The colours a spatial view is painted in, read from the stylesheet's tokens: one CSS colour for
 * each {@link ColourToken} a draw op may name, and the background.
 */
export interface ColourTokens extends Readonly<Record<ColourToken, string>> {
  /** `--surface-0`, which the canvas is cleared to. */
  readonly surface0: string;
  /** `--text-muted`, which a stale view is drawn in ({@link staleTokens}). */
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

/** How much of each side of its square a reticle's corner arm covers. */
const RETICLE_ARM_SHARE = 1 / 3;

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

function paintSymbol(context: CanvasRenderingContext2D, op: SymbolOp, tokens: ColourTokens): void {
  const outline = symbolOutline(op.shape);
  const { xPx, yPx } = op.centre;
  context.beginPath();
  switch (outline.kind) {
    case "circle":
      context.arc(xPx, yPx, op.radiusPx, 0, 2 * Math.PI);
      break;
    case "polygon":
      tracePoints(
        context,
        outline.points.map((point) => ({
          xPx: xPx + point.x * op.radiusPx,
          yPx: yPx + point.y * op.radiusPx,
        })),
      );
      context.closePath();
      break;
  }
  // Filled above the reference plane and open below it, with the same outline (plan 05, D14).
  if (op.fill !== null) {
    context.fillStyle = tokens[op.fill];
    context.fill();
  }
  strokeWith(context, tokens, op.stroke, op.widthPx);
}

/** Four corner brackets of the square of half-width `halfSizePx` about the reticle's centre. */
function paintReticle(
  context: CanvasRenderingContext2D,
  op: ReticleOp,
  tokens: ColourTokens,
): void {
  const { xPx, yPx } = op.centre;
  const half = op.halfSizePx;
  const arm = 2 * half * RETICLE_ARM_SHARE;
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
  strokeWith(context, tokens, op.stroke, op.widthPx);
}

function paintOp(context: CanvasRenderingContext2D, op: DrawOp, tokens: ColourTokens): void {
  switch (op.kind) {
    case "line":
      context.beginPath();
      tracePoints(context, [op.from, op.to]);
      strokeWith(context, tokens, op.stroke, op.widthPx);
      break;
    case "polyline":
      context.beginPath();
      tracePoints(context, op.points);
      strokeWith(context, tokens, op.stroke, op.widthPx);
      break;
    case "circle":
      context.beginPath();
      context.arc(op.centre.xPx, op.centre.yPx, op.radiusPx, 0, 2 * Math.PI);
      strokeWith(context, tokens, op.stroke, op.widthPx);
      break;
    case "symbol":
      paintSymbol(context, op, tokens);
      break;
    case "reticle":
      paintReticle(context, op, tokens);
      break;
    case "ticks":
      context.beginPath();
      for (const segment of op.segments) {
        tracePoints(context, [segment.from, segment.to]);
      }
      strokeWith(context, tokens, op.stroke, op.widthPx);
      break;
  }
}

/**
 * Paints a draw list onto a canvas: clears it to `--surface-0`, then executes each op in order.
 *
 * @remarks
 * The whole backing store is cleared, whatever its size; the ops, in CSS pixels, are then drawn
 * scaled by `pixelRatio` onto a backing store that many times larger. Circles are arcs, symbols a
 * path scaled from their unit outline, filled and stroked above the plane and only stroked below,
 * and reticles four corner brackets. There is no text on the canvas (plan 05, D15), and nothing is
 * translucent, shadowed or graded.
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
  for (const op of drawList.ops) {
    paintOp(context, op, tokens);
  }
}
