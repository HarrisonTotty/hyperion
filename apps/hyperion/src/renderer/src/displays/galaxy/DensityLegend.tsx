import { formatSci } from "../../lib/format";
import { RasterLegend, tenToThe } from "./RasterLegend";

interface DensityLegendProps {
  /** The ramp the map is painted with, from `buildRamp`, so the legend matches the picture. */
  readonly ramp: Uint8ClampedArray;
  /** log₁₀ of the column density, in systems per square light-year, of code 1 (the floor). */
  readonly floorLog10PerLy2: number;
  /** log₁₀ of the column density, in systems per square light-year, of the largest code. */
  readonly ceilingLog10PerLy2: number;
}

function tick(log10: number): string {
  return formatSci(10 ** log10, "tick");
}

/**
 * The legend of a density map: the ramp as a bar, a tick and label at each whole decade, the
 * unit, the words `LOG SCALE` and the stated floor.
 *
 * @remarks
 * A {@link RasterLegend} titled `COLUMN DENSITY` in `SYSTEMS/ly²`. Tick labels are in E notation
 * (`1E-4`), since B612 has no superscript minus.
 */
export function DensityLegend({ ramp, floorLog10PerLy2, ceilingLog10PerLy2 }: DensityLegendProps) {
  return (
    <RasterLegend
      ramp={ramp}
      floorLog10={floorLog10PerLy2}
      ceilingLog10={ceilingLog10PerLy2}
      title="COLUMN DENSITY"
      unit="SYSTEMS/ly²"
      tickLabel={tick}
      name={
        `Column density legend, log scale from ${tenToThe(floorLog10PerLy2)} to ` +
        `${tenToThe(ceilingLog10PerLy2)} systems per square light-year`
      }
      emptyName="Column density legend, log scale: no systems in the map"
      floorText={`FLOOR ${formatSci(10 ** floorLog10PerLy2)}: AT OR BELOW SHOWN AS BACKGROUND`}
      emptyText="NO SYSTEMS IN MAP"
    />
  );
}
