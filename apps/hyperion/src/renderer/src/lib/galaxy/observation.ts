/**
 * How the readings of a system seen in observed mode are written (plan 12, P12.T11): its light's
 * age and the time the light left it, and the stated bound on where it appears.
 */
import {
  AU_PER_LY,
  type BodyDistanceUnit,
  DISTANCE_HYSTERESIS,
  formatBodyDistance,
  formatNumber,
  formatSignificant,
  KM_PER_AU,
  SCALE_AU_BELOW_LY,
} from "../format";

/** Kilometres in a light-year: `AU_PER_LY` astronomical units of `KM_PER_AU` each, both exact. */
const KM_PER_LY = AU_PER_LY * KM_PER_AU;

/**
 * Decimals of a light age in `yr`: universe time's two (`formatUniverseTimeYr`), since the light age
 * is the chart's time less the emitted time and the three are read side by side.
 */
const LIGHT_AGE_DECIMALS = 2;

/**
 * The width of a light age's value, in characters: `228,000.00`, room for the oldest light any
 * observer in the root cube receives, its diagonal light time of 227,023 years (131,072 ly × √3).
 */
export const LIGHT_AGE_WIDTH_CH = 10;

/**
 * The width of an emitted time's value, in characters: `-228,000.00`, room for the earliest, the
 * source horizon, the clock window's −1,000 years less that diagonal light time.
 */
export const EMITTED_WIDTH_CH = 11;

/**
 * Writes a light's age in Julian years, without its unit, to universe time's 0.01 yr, digits
 * grouped from five: `4210.00`, `26,000.00`.
 */
export function formatLightAgeYr(lightAgeYr: number): string {
  return formatNumber(lightAgeYr, LIGHT_AGE_DECIMALS);
}

/** The unit a curvature bound is written in: a planetary system's distance units, then `ly`. */
export type BoundUnit = BodyDistanceUnit | "ly";

/** A curvature bound split into its digits and unit, so that the unit can be set apart. */
export interface FormattedBound {
  readonly value: string;
  readonly unit: BoundUnit;
}

/**
 * Writes the stated curvature bound on an apparent position in the guide's distance ladder, to
 * three significant figures: a planetary system's units, `km`, `Mm`, `Gm` and `AU`
 * (`formatBodyDistance`), below 0.01 ly, and `ly` from there, switching unit with hysteresis.
 *
 * @remarks
 * The bound is a stated limit, not a standard uncertainty, so it is a reading of its own and never
 * a `±`, which the guide keeps for one standard deviation. Plan 12's Design note 3 gives 0.53 ly
 * for a disc star across the root cube, 227,000 years out; scaled as the light age squared, that
 * is about 1.0E-5 ly, 0.65 AU, a thousand years out, so the bound is scaled rather than printed as
 * a long number. The switch to `ly` is at 0.01 ly, where the chart's scale
 * bar leaves `AU` (`SCALE_AU_BELOW_LY`): a bound shown in `ly` keeps it down to 5% under, and one
 * shown in a system's unit keeps that unit up to 5% over, as {@link DISTANCE_HYSTERESIS} has every
 * distance do. A bound of exactly zero, for an orbit followed in full, reads `0 ly`.
 *
 * @param previous - The unit this bound was last shown in, which the caller keeps; `null` for the
 *   first showing.
 */
export function formatCurvatureBound(boundLy: number, previous: BoundUnit | null): FormattedBound {
  if (boundLy === 0) {
    return { value: "0", unit: "ly" };
  }
  const magnitudeLy = Math.abs(boundLy);
  let edgeLy = SCALE_AU_BELOW_LY;
  if (previous === "ly") {
    edgeLy = SCALE_AU_BELOW_LY * (1 - DISTANCE_HYSTERESIS);
  } else if (previous !== null) {
    edgeLy = SCALE_AU_BELOW_LY * (1 + DISTANCE_HYSTERESIS);
  }
  if (magnitudeLy >= edgeLy) {
    return { value: formatSignificant(boundLy), unit: "ly" };
  }
  const inSystem = formatBodyDistance(boundLy * KM_PER_LY, previous === "ly" ? null : previous);
  return { value: inSystem.value, unit: inSystem.unit };
}

/**
 * Writes the curvature bound as the angle it subtends at the observer, in arcseconds, without the
 * `″` set against it: three significant figures, in E notation below 0.001″ (`formatSignificant`),
 * and `0` for an orbit followed in full.
 */
export function formatCurvatureBoundArcsec(boundArcsec: number): string {
  return boundArcsec === 0 ? "0" : formatSignificant(boundArcsec);
}
