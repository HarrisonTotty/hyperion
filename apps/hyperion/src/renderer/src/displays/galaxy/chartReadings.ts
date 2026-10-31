/**
 * The readings the `GALAXY` display writes about the chart on show, from one writer: a place in the
 * `GALACTIC` frame, the chart time and the mode (plan 12, P12.T11.b).
 *
 * @remarks
 * The `LOCAL CHART`'s furniture and the `HR DIAGRAM` page both read the answer on show through
 * these, so that the two pages cannot write its mode, its observer or its time with different
 * labels, slots or precisions.
 */
import {
  formatBearingDeg,
  formatLengthLy,
  formatSigned,
  formatUniverseTimeYr,
  TIME_SYSTEM_LABEL,
} from "../../lib/format";
import type { CentreLy, QueryMode } from "../../lib/galaxy/model";
import { AXIS_TOLERANCE_LY, cylindrical } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import type { SpatialReading } from "../../spatial/Reading";
import type { SpatialMode } from "../../spatial/ModeReading";

/** The frame the chart's places are given in, as the furniture's `FRAME` names it. */
export const CHART_FRAME_NAME = "GALACTIC";

/** Room for the longest value of a place's `RADIUS` at one decimal, in characters. */
export const RADIUS_WIDTH_CH = 9;

/** Room for the longest value of a place's `ANGLE` at one decimal, in characters. */
export const ANGLE_WIDTH_CH = 6;

/** Room for the longest value of a place's `HEIGHT` at one decimal, in characters. */
export const HEIGHT_WIDTH_CH = 9;

/** Room for the longest value of the chart time, in characters. */
export const TIME_WIDTH_CH = 9;

/**
 * The precision, in decimals, a place's coordinates are read at on a chart from 10 ly, which the
 * core arrow's shares.
 */
export const CENTRE_DECIMALS = 1;

/** The most decimals a place is read at: on the charts below 0.01 ly, the centre's finest. */
const FINEST_PLACE_DECIMALS = 5;

/**
 * The decimals the furniture reads a place at on a chart of this query radius: `RADIUS`, `ANGLE`
 * and `HEIGHT`, the core arrow's distance, and the observer's place.
 *
 * @remarks
 * One decimal from a radius of 10 ly, as before, and one more for each decade narrower, to five
 * below 0.01 ly, so that a step of the place is at most a hundredth of the chart's radius: on the
 * finest chart, 0.001 ly or 63.2 AU in radius, 0.00001 ly or 0.63 AU, the step the `CURSOR`
 * panel's coordinates keep, which places it against the central black hole (plan 09, P09.T42). The angle takes the same
 * decimals, which near the axis resolve the place finer still. The radius is the answer's, and its
 * steps are exact 1-2-5 values; a tolerance keeps a computed 0.01 ly from falling a decade.
 */
export function placeDecimalsFor(queryRadiusLy: number): number {
  let decimals = CENTRE_DECIMALS;
  let decade = 10;
  while (decimals < FINEST_PLACE_DECIMALS && queryRadiusLy < decade * (1 - 1e-9)) {
    decimals += 1;
    decade /= 10;
  }
  return decimals;
}

/**
 * A galactocentric radius as the chart's furniture writes it, without its unit: a place's
 * `RADIUS`, and the core arrow's distance.
 *
 * @param decimals - The chart's place decimals, {@link placeDecimalsFor}: 0.1 ly from a 10 ly chart.
 */
export function radiusReading(radiusLy: number, decimals: number): string {
  return formatLengthLy(radiusLy, decimals);
}

/**
 * A point's place in the `GALACTIC` frame as the chart's furniture writes it: `RADIUS`, `ANGLE`
 * (missing on the galactic axis, where it is undefined) and `HEIGHT`, at the chart's place
 * decimals, as the chart's centre and its observer are both written. Each slot holds its reading's
 * longest value at those decimals (`-65,536.00000` at five), so it is fixed for the chart.
 *
 * @param decimals - The chart's place decimals, {@link placeDecimalsFor}: 0.1 ly from a 10 ly chart.
 */
export function placeReadings(
  positionLy: CentreLy,
  decimals: number,
): ReadonlyArray<SpatialReading> {
  const place = cylindrical(vec3(...positionLy));
  const onAxis = !(place.radiusLy > AXIS_TOLERANCE_LY);
  const extra = decimals - CENTRE_DECIMALS;
  return [
    {
      label: "RADIUS",
      value: radiusReading(place.radiusLy, decimals),
      unit: "ly",
      widthCh: RADIUS_WIDTH_CH + extra,
    },
    {
      label: "ANGLE",
      value: onAxis ? null : formatBearingDeg(place.angleDeg, decimals),
      unit: "",
      widthCh: ANGLE_WIDTH_CH + extra,
    },
    {
      label: "HEIGHT",
      value: formatSigned(place.heightLy, decimals),
      unit: "ly",
      widthCh: HEIGHT_WIDTH_CH + extra,
    },
  ];
}

/**
 * The chart time as the chart's furniture writes it, in universe time to 0.01 yr (`UT +12.50 yr`).
 *
 * @param timeYr - The answer's chart time, in years from the epoch.
 */
export function timeReading(timeYr: number): SpatialReading {
  return {
    label: TIME_SYSTEM_LABEL,
    value: formatUniverseTimeYr(timeYr),
    unit: "yr",
    widthCh: TIME_WIDTH_CH,
  };
}

/**
 * The mode of the chart on show, as its furniture writes it after the frame, the time and the scale
 * bar (plan 12, P12.T11 and P12.T11.d): `NOW`, or `OBSERVED FROM` and the observer, written as the
 * centre is.
 *
 * @param mode - The mode the answer on show was given in, as `answeredMode` reads it, never the
 * mode being chosen.
 * @param decimals - The answer's place decimals, {@link placeDecimalsFor} of its radius.
 */
export function modeReading(mode: QueryMode, decimals: number): SpatialMode {
  let reading: SpatialMode;
  switch (mode.kind) {
    case "now":
      reading = { name: "NOW", from: [] };
      break;
    case "observed":
      reading = { name: "OBSERVED FROM", from: placeReadings(mode.observerLy, decimals) };
      break;
  }
  return reading;
}
