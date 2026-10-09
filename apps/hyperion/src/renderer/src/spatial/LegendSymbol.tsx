import { markShiftDevicePx, RING_SHIFTS, useStrokeMetrics } from "../lib/strokes";
import type { SymbolShape } from "./marks";
import { symbolOutline, unitInradius } from "./symbols";

/** The width of the box every legend mark is drawn in, in its own units. */
const MARK_BOX_UNITS = 10;

/** The unit box every legend mark is drawn in, its centre at the origin. */
const MARK_BOX = "-5 -5 10 10";

/** Radius of a legend symbol in its box, leaving room for the outline drawn inside the diameter. */
const MARK_RADIUS = 4.25;

/** The SVG path of a circle about the box's centre, as two half-turn arcs. */
function circle(radius: number): string {
  return `M ${-radius} 0 A ${radius} ${radius} 0 1 0 ${radius} 0 A ${radius} ${radius} 0 1 0 ${-radius} 0 Z`;
}

/**
 * The SVG path of a symbol's outline at the legend's radius, from the shared outlines, moved out
 * by `shiftUnits`, the outline shift δ in the box's units, as `paint` moves a mark's: a circle's
 * radius by δ, a polygon's sides each by δ (its corners by δ over its unit inradius), and a ringed
 * circle's disc by δ and its ring by 3δ.
 */
function outlinePath(
  shape: SymbolShape,
  shiftUnits: number,
): { readonly d: string; readonly discD: string | null } {
  const outline = symbolOutline(shape);
  let d: string;
  let discD: string | null = null;
  switch (outline.kind) {
    case "circle":
      d = circle(MARK_RADIUS + shiftUnits);
      break;
    case "ringed-circle":
      d = circle(MARK_RADIUS + RING_SHIFTS * shiftUnits);
      discD = circle(MARK_RADIUS * outline.discRadius + shiftUnits);
      break;
    case "polygon": {
      const corner = MARK_RADIUS + shiftUnits / unitInradius(outline.points);
      d = `${outline.points
        .map((point, index) => `${index === 0 ? "M" : "L"} ${point.x * corner} ${point.y * corner}`)
        .join(" ")} Z`;
      break;
    }
  }
  return { d, discD };
}

/** Props of {@link LegendSymbol}. */
export interface LegendSymbolProps {
  readonly shape: SymbolShape;
  /** Diameter in `rem`, from the size class it stands for. */
  readonly diameterRem: number;
  readonly filled: boolean;
  /** Whether it stands for what is available, drawn in `--accent`; `false` when absent. */
  readonly available?: boolean | undefined;
  /**
   * The shape's accessible name, such as "Ringed circle", where the legend names shapes; absent,
   * the symbol is hidden from assistive technology, as a sample of size or fill is.
   */
  readonly name?: string | undefined;
}

/**
 * One symbol of a legend, drawn as an inline SVG from the outline the spatial view paints it with,
 * so that the legend and the picture cannot disagree.
 *
 * @remarks
 * Its outline is a mark's, `--mark-stroke`: the larger of 1.5 CSS px and 2 device px. Where that
 * is wider than 1.5 px, the outline widens outward as the painter's does (R07.T16.f;
 * decision-thin-line-contrast, item 2), by the outline shift δ, `markShiftDevicePx` of the ratio,
 * taken into the box's units at its diameter and the root's rem, so that its hole stays as built. A
 * ringed circle fills its inner disc alone, as the painter does (plan 06, design note 17).
 */
export function LegendSymbol({
  shape,
  diameterRem,
  filled,
  available = false,
  name,
}: LegendSymbolProps) {
  const { devicePixelRatio, remPx } = useStrokeMetrics();
  // δ in CSS px, then in the box's units, whose width spans the symbol's diameter.
  const shiftCssPx = markShiftDevicePx(devicePixelRatio) / devicePixelRatio;
  const { d, discD } = outlinePath(shape, (shiftCssPx * MARK_BOX_UNITS) / (diameterRem * remPx));
  const fillsRing = filled && discD === null;
  return (
    <svg
      className={
        available ? "symbol-legend__mark symbol-legend__mark--available" : "symbol-legend__mark"
      }
      style={{ width: `${diameterRem}rem`, height: `${diameterRem}rem` }}
      viewBox={MARK_BOX}
      role={name === undefined ? undefined : "img"}
      aria-label={name}
      aria-hidden={name === undefined ? "true" : undefined}
      focusable="false"
    >
      <path d={d} className={fillsRing ? "symbol-legend__filled" : undefined} />
      {discD === null ? null : (
        <path d={discD} className={filled ? "symbol-legend__filled" : undefined} />
      )}
    </svg>
  );
}
