import type { SymbolShape } from "./marks";
import { symbolOutline } from "./symbols";

/** The unit box every legend mark is drawn in, its centre at the origin. */
const MARK_BOX = "-5 -5 10 10";

/** Radius of a legend symbol in its box, leaving room for the outline drawn inside the diameter. */
const MARK_RADIUS = 4.25;

/** The SVG path of a circle about the box's centre, as two half-turn arcs. */
function circle(radius: number): string {
  return `M ${-radius} 0 A ${radius} ${radius} 0 1 0 ${radius} 0 A ${radius} ${radius} 0 1 0 ${-radius} 0 Z`;
}

/** The SVG path of a symbol's outline at the legend's radius, from the shared outlines. */
function outlinePath(shape: SymbolShape): { readonly d: string; readonly discD: string | null } {
  const outline = symbolOutline(shape);
  let d: string;
  let discD: string | null = null;
  switch (outline.kind) {
    case "circle":
      d = circle(MARK_RADIUS);
      break;
    case "ringed-circle":
      d = circle(MARK_RADIUS);
      discD = circle(MARK_RADIUS * outline.discRadius);
      break;
    case "polygon":
      d = `${outline.points
        .map(
          (point, index) =>
            `${index === 0 ? "M" : "L"} ${point.x * MARK_RADIUS} ${point.y * MARK_RADIUS}`,
        )
        .join(" ")} Z`;
      break;
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
 * Its outline is 1.5 px at every scale, as a mark's is. A ringed circle fills its inner disc alone,
 * as the painter does (plan 06, design note 17).
 */
export function LegendSymbol({
  shape,
  diameterRem,
  filled,
  available = false,
  name,
}: LegendSymbolProps) {
  const { d, discD } = outlinePath(shape);
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
