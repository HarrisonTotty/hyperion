import { formatNumber, formatSci } from "../../lib/format";
import { rampColour, RAMP_LEVELS } from "../../lib/galaxy/ramp";

interface DensityLegendProps {
  /** The ramp the map is painted with, from `buildRamp`, so the legend matches the picture. */
  readonly ramp: Uint8ClampedArray;
  /** log₁₀ of the column density, in systems per square light-year, of code 1 (the floor). */
  readonly floorLog10PerLy2: number;
  /** log₁₀ of the column density, in systems per square light-year, of the largest code. */
  readonly ceilingLog10PerLy2: number;
}

const SEGMENTS = 32;
const BAR_WIDTH = 320;
const SEGMENT_WIDTH = BAR_WIDTH / SEGMENTS;
const BAR_HEIGHT = 12;
const TICK_BOTTOM = 16;
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

function tenToThe(log10: number): string {
  return `ten to the power ${formatNumber(log10, 1)}`;
}

/**
 * The legend of a density map: the ramp as a bar, a tick and label at each whole decade, the
 * unit, the words `LOG SCALE` and the stated floor.
 *
 * @remarks
 * Follows the raster rules under "Graphs, schematics and spatial displays" in
 * `docs/frontend/ux-guidelines.md`. The bar is 32 steps of the picture's own ramp; its accessible
 * name gives the range and unit in words. Tick labels are in E notation (`1E-4`), since B612 has
 * no superscript minus.
 */
export function DensityLegend({ ramp, floorLog10PerLy2, ceilingLog10PerLy2 }: DensityLegendProps) {
  const spanLog10 = ceilingLog10PerLy2 - floorLog10PerLy2;
  // A map with no systems has floor and ceiling equal (plan 04, design note 12).
  const empty = !(spanLog10 > 0);
  const decades = empty ? [] : wholeDecades(floorLog10PerLy2, ceilingLog10PerLy2);
  const position = (log10: number): number => (log10 - floorLog10PerLy2) / spanLog10;
  const name = empty
    ? "Column density legend, log scale: no systems in the map"
    : `Column density legend, log scale from ${tenToThe(floorLog10PerLy2)} to ` +
      `${tenToThe(ceilingLog10PerLy2)} systems per square light-year`;

  return (
    <figure className="density-legend">
      <figcaption className="density-legend__caption">
        <span className="density-legend__title">COLUMN DENSITY</span>
        <span className="density-legend__unit">SYSTEMS/ly²</span>
        <span className="density-legend__scale-type">LOG SCALE</span>
      </figcaption>
      <div className="density-legend__scale">
        <svg
          className="density-legend__bar"
          // The bar is drawn inline from the live ramp; an `img` element would need a data URL.
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
          role="img"
          aria-label={name}
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
              y2={TICK_BOTTOM}
              stroke="currentColor"
              strokeWidth={1}
              vectorEffect="non-scaling-stroke"
            />
          ))}
        </svg>
        <div className="density-legend__ticks" aria-hidden="true">
          {decades.map((decade) => (
            <span
              key={decade}
              className="density-legend__tick"
              style={{ left: `${position(decade) * 100}%` }}
            >
              {formatSci(10 ** decade, "tick")}
            </span>
          ))}
        </div>
      </div>
      <p className="density-legend__floor">
        {empty
          ? "NO SYSTEMS IN MAP"
          : `FLOOR ${formatSci(10 ** floorLog10PerLy2)}: AT OR BELOW SHOWN AS BACKGROUND`}
      </p>
    </figure>
  );
}
