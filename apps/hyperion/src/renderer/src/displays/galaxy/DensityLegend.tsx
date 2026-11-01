import type { MapPopulation } from "@hyperion/protocol";

import { formatSci } from "../../lib/format";
import { RasterLegend, tenToThe } from "./RasterLegend";
import { RemnantsCount } from "./RemnantsCount";

interface DensityLegendProps {
  /** The systems the map counts. */
  readonly population: MapPopulation;
  /**
   * Whether the legend's words state what a `REMNANTS` map counts, which the page states in one
   * view's legend; the bar's accessible name states it in both.
   */
  readonly statesCount: boolean;
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
 * (`1E-4`), since B612 has no superscript minus. The `REMNANTS` map's legend states what it counts,
 * `DISPLACED · INIT MASS ABOVE 8 M☉`: layer E's displaced remnants alone, not the remnants retained
 * in their populations (plan 08, P08.T15.b; the guide's `REMNANTS`), in words where `statesCount`
 * holds and in the bar's name always.
 */
export function DensityLegend({
  population,
  statesCount,
  ramp,
  floorLog10PerLy2,
  ceilingLog10PerLy2,
}: DensityLegendProps) {
  const remnants = population === "layer_e_remnants";
  // The bar's name says what the map counts too, for the line's drawn sun sign has a name of its
  // own.
  const counted = remnants
    ? ", displaced remnants of stars of initial mass above 8 solar masses"
    : "";
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
        `${tenToThe(ceilingLog10PerLy2)} systems per square light-year${counted}`
      }
      emptyName={`Column density legend, log scale: no systems in the map${counted}`}
      floorText={`FLOOR ${formatSci(10 ** floorLog10PerLy2)}: AT OR BELOW SHOWN AS BACKGROUND`}
      emptyText="NO SYSTEMS IN MAP"
      counted={remnants && statesCount ? <RemnantsCount /> : null}
    />
  );
}
