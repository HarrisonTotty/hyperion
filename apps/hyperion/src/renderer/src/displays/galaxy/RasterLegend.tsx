import type { ReactNode } from "react";

import { formatNumber } from "../../lib/format";
import { rampColour, RAMP_LEVELS } from "../../lib/galaxy/ramp";
import { type ElementSize, useElementSize } from "../../lib/useElementSize";

interface RasterLegendProps {
  /** The ramp the map is painted with, from `buildRamp`, so the legend matches the picture. */
  readonly ramp: Uint8ClampedArray;
  /** log₁₀ of the quantity, in the legend's unit, of code 1 (the floor). */
  readonly floorLog10: number;
  /** log₁₀ of the quantity, in the legend's unit, of the largest code. */
  readonly ceilingLog10: number;
  /** The quantity's name, in upper case: `COLUMN DENSITY`. */
  readonly title: string;
  /** The unit's symbol, as the guide writes it. */
  readonly unit: ReactNode;
  /** The label of the tick at the decade `log10`. */
  readonly tickLabel: (log10: number) => string;
  /** The bar's accessible name, for a map whose ceiling is above its floor. */
  readonly name: string;
  /** The bar's accessible name for a map with nothing above its floor. */
  readonly emptyName: string;
  /** The floor's line: its value, and that values at or below it are the background. */
  readonly floorText: string;
  /** The line in place of the floor's for a map with nothing above its floor. */
  readonly emptyText: string;
}

const SEGMENTS = 32;
const BAR_WIDTH = 320;
const SEGMENT_WIDTH = BAR_WIDTH / SEGMENTS;
const BAR_HEIGHT = 12;
const TICK_BOTTOM = 16;
/** Where an unlabelled decade's shorter tick ends. */
const MINOR_TICK_BOTTOM = 14;
// A decade within this of the floor or the ceiling still gets its tick, despite rounding.
const DECADE_TOLERANCE = 1e-9;

function segmentLevel(segment: number): number {
  // The level at the segment's middle, on the levels above the background (1 to 255).
  return 1 + Math.round(((segment + 0.5) / SEGMENTS) * (RAMP_LEVELS - 2));
}

function wholeDecades(floor: number, ceiling: number): number[] {
  const decades: number[] = [];
  const last = Math.floor(ceiling + DECADE_TOLERANCE);
  for (let decade = Math.ceil(floor - DECADE_TOLERANCE); decade <= last; decade += 1) {
    decades.push(decade);
  }
  return decades;
}

/** B612 Mono's advance, the same for every character, in em (the bundled font's `hmtx`). */
const MONO_ADVANCE_EM = 0.65;
/** The tick labels' size, as `styles.css` sets it on the legend. */
const LABEL_FONT_REM = 0.875;
/** The least space between two tick labels. */
const LABEL_GAP_REM = 0.5;

/**
 * Every how many decades a tick is labelled: 1 unless the labels would run together on the bar
 * as laid out, as the edge-on map's seven decades do in a narrow column.
 *
 * @param ticksWidth - The measured width of the row the labels stand in, the bar's width; `null`
 *   before it is measured, when every decade is labelled.
 */
function labelEvery(
  ticksWidth: ElementSize | null,
  spanLog10: number,
  labels: ReadonlyArray<string>,
): number {
  if (ticksWidth === null || !(ticksWidth.widthPx > 0) || !(spanLog10 > 0)) {
    return 1;
  }
  const widestChars = Math.max(0, ...labels.map((label) => label.length));
  const labelPx =
    (widestChars * MONO_ADVANCE_EM * LABEL_FONT_REM + LABEL_GAP_REM) * ticksWidth.remPx;
  const decadePx = ticksWidth.widthPx / spanLog10;
  return Math.max(1, Math.ceil(labelPx / decadePx));
}

function isMultipleOf(decade: number, step: number): boolean {
  return ((decade % step) + step) % step === 0;
}

/** A decade in words, for an accessible name. */
export function tenToThe(log10: number): string {
  return `ten to the power ${formatNumber(log10, 1)}`;
}

/**
 * The legend of a raster map: the ramp as a bar, a tick and label at each whole decade, the
 * quantity, its unit, the words `LOG SCALE` and the stated floor.
 *
 * @remarks
 * Follows the raster rules under "Graphs, schematics and spatial displays" in
 * `docs/frontend/ux-guidelines.md`, which give each raster quantity a legend of its own with its
 * title, unit, scale and floor: {@link DensityLegend} and `ExtinctionLegend` are this legend with
 * their own words. The bar is 32 steps of the picture's own ramp; its accessible name gives the
 * range and unit in words. Every decade has a tick; where the bar is too narrow for a label at
 * each, only the decades at a multiple of every second (or third) are labelled, with longer ticks.
 * The block's classes are `density-legend`, the first raster's.
 */
export function RasterLegend({
  ramp,
  floorLog10,
  ceilingLog10,
  title,
  unit,
  tickLabel,
  name,
  emptyName,
  floorText,
  emptyText,
}: RasterLegendProps) {
  const { ref: ticksRef, size: ticksWidth } = useElementSize();
  const spanLog10 = ceilingLog10 - floorLog10;
  // A map with nothing above its floor has floor and ceiling equal (plan 04, design note 12).
  const empty = !(spanLog10 > 0);
  const decades = empty ? [] : wholeDecades(floorLog10, ceilingLog10);
  const labels = new Map(decades.map((decade) => [decade, tickLabel(decade)]));
  const step = labelEvery(ticksWidth, spanLog10, [...labels.values()]);
  // Decades at a multiple of the step read best (1E-6, 1E-4, …); failing any, the first decade.
  const anchor = decades.find((decade) => isMultipleOf(decade, step)) ?? decades[0] ?? 0;
  const labelled = (decade: number): boolean => isMultipleOf(decade - anchor, step);
  const position = (log10: number): number => (log10 - floorLog10) / spanLog10;

  return (
    <figure className="density-legend">
      <figcaption className="density-legend__caption">
        <span className="density-legend__title">{title}</span>
        <span className="density-legend__unit">{unit}</span>
        <span className="density-legend__scale-type">LOG SCALE</span>
      </figcaption>
      <div className="density-legend__scale">
        <svg
          className="density-legend__bar"
          // The bar is drawn inline from the live ramp; an `img` element would need a data URL.
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
          role="img"
          aria-label={empty ? emptyName : name}
          viewBox={`0 0 ${BAR_WIDTH} ${TICK_BOTTOM}`}
          preserveAspectRatio="none"
        >
          {Array.from({ length: SEGMENTS }, (_, segment) => (
            <rect
              key={segment}
              x={segment * SEGMENT_WIDTH}
              y={0}
              width={SEGMENT_WIDTH}
              height={BAR_HEIGHT}
              fill={rampColour(ramp, segmentLevel(segment))}
            />
          ))}
          {decades.map((decade) => (
            <line
              key={decade}
              x1={position(decade) * BAR_WIDTH}
              x2={position(decade) * BAR_WIDTH}
              y1={BAR_HEIGHT}
              y2={labelled(decade) ? TICK_BOTTOM : MINOR_TICK_BOTTOM}
              stroke="currentColor"
              strokeWidth={1}
              vectorEffect="non-scaling-stroke"
            />
          ))}
        </svg>
        <div className="density-legend__ticks" ref={ticksRef} aria-hidden="true">
          {decades.filter(labelled).map((decade) => (
            <span
              key={decade}
              className="density-legend__tick"
              style={{ left: `${position(decade) * 100}%` }}
            >
              {labels.get(decade)}
            </span>
          ))}
        </div>
      </div>
      <p className="density-legend__floor">{empty ? emptyText : floorText}</p>
    </figure>
  );
}
