import { formatNumber } from "../../lib/format";
import { RasterLegend } from "./RasterLegend";

interface ExtinctionLegendProps {
  /** The ramp the map is painted with, from `buildRamp`, so the legend matches the picture. */
  readonly ramp: Uint8ClampedArray;
  /** log₁₀ of the visual extinction, in magnitudes, of code 1 (the floor). */
  readonly floorLog10Mag: number;
  /** log₁₀ of the visual extinction, in magnitudes, of the largest code. */
  readonly ceilingLog10Mag: number;
}

/** A magnitude to as many decimals as its decade needs, and no more: `0.01`, `0.1`, `1`, `10`. */
function magnitudeText(log10: number): string {
  return formatNumber(10 ** log10, Math.max(0, -Math.floor(log10 + 1e-9)));
}

/** A magnitude in words, for an accessible name, to the two decimals every A(V) reading has. */
function magnitudeWords(log10: number): string {
  return `${formatNumber(10 ** log10, 2)} magnitudes`;
}

/**
 * The legend of an extinction map: the ramp as a bar, a tick and label at each whole decade of
 * magnitudes, the title `EXTINCTION A(V)`, the unit `mag`, the words `LOG SCALE` and the stated
 * floor (plan 07, P07.T11.b).
 *
 * @remarks
 * A {@link RasterLegend}, as the density map's is. Its ticks are plain decimals, `0.01` to `10`,
 * since every decade the map can span, from the floor of 0.01 mag to tens of magnitudes, is inside
 * the magnitude's own ladder; the floor is stated to the same precision.
 */
export function ExtinctionLegend({ ramp, floorLog10Mag, ceilingLog10Mag }: ExtinctionLegendProps) {
  return (
    <RasterLegend
      ramp={ramp}
      floorLog10={floorLog10Mag}
      ceilingLog10={ceilingLog10Mag}
      title="EXTINCTION A(V)"
      unit="mag"
      tickLabel={magnitudeText}
      name={`Extinction legend, log scale from ${magnitudeWords(floorLog10Mag)} to ${magnitudeWords(ceilingLog10Mag)}`}
      emptyName="Extinction legend, log scale: no extinction above the floor"
      floorText={`FLOOR ${magnitudeText(floorLog10Mag)} mag: AT OR BELOW SHOWN AS BACKGROUND`}
      emptyText={`NOTHING ABOVE FLOOR ${magnitudeText(floorLog10Mag)} mag`}
    />
  );
}
