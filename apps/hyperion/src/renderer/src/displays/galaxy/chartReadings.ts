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

/** Room for the longest value of a place's `RADIUS`, in characters. */
export const RADIUS_WIDTH_CH = 9;

/** Room for the longest value of a place's `ANGLE`, in characters. */
export const ANGLE_WIDTH_CH = 6;

/** Room for the longest value of a place's `HEIGHT`, in characters. */
export const HEIGHT_WIDTH_CH = 9;

/** Room for the longest value of the chart time, in characters. */
export const TIME_WIDTH_CH = 9;

/** The precision, in decimals, a place's coordinates are read at, which the core arrow's shares. */
export const CENTRE_DECIMALS = 1;

/**
 * A galactocentric radius as the chart's furniture writes it, to 0.1 ly, without its unit: a
 * place's `RADIUS`, and the core arrow's distance.
 */
export function radiusReading(radiusLy: number): string {
  return formatLengthLy(radiusLy, CENTRE_DECIMALS);
}

/**
 * A point's place in the `GALACTIC` frame as the chart's furniture writes it: `RADIUS`, `ANGLE`
 * (missing on the galactic axis, where it is undefined) and `HEIGHT`, to 0.1 ly, as the chart's
 * centre and its observer are both written.
 */
export function placeReadings(positionLy: CentreLy): ReadonlyArray<SpatialReading> {
  const place = cylindrical(vec3(...positionLy));
  const onAxis = !(place.radiusLy > AXIS_TOLERANCE_LY);
  return [
    {
      label: "RADIUS",
      value: radiusReading(place.radiusLy),
      unit: "ly",
      widthCh: RADIUS_WIDTH_CH,
    },
    {
      label: "ANGLE",
      value: onAxis ? null : formatBearingDeg(place.angleDeg, CENTRE_DECIMALS),
      unit: "",
      widthCh: ANGLE_WIDTH_CH,
    },
    {
      label: "HEIGHT",
      value: formatSigned(place.heightLy, CENTRE_DECIMALS),
      unit: "ly",
      widthCh: HEIGHT_WIDTH_CH,
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
 */
export function modeReading(mode: QueryMode): SpatialMode {
  let reading: SpatialMode;
  switch (mode.kind) {
    case "now":
      reading = { name: "NOW", from: [] };
      break;
    case "observed":
      reading = { name: "OBSERVED FROM", from: placeReadings(mode.observerLy) };
      break;
  }
  return reading;
}
