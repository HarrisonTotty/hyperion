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
 * The decimals the furniture reads a place's lengths at on a chart of this query radius: `RADIUS`
 * and `HEIGHT`, the core arrow's distance, and the observer's place.
 *
 * @remarks
 * One decimal from a radius of 10 ly, as before, and one more for each decade narrower, to five
 * below 0.01 ly, so that a step of the place is at most a hundredth of the chart's radius: on the
 * finest chart, 0.001 ly or 63.2 AU in radius, 0.00001 ly or 0.63 AU, the step the `CURSOR`
 * panel's coordinates keep, which places it against the central black hole (plan 09, P09.T42).
 * `ANGLE` takes its own decimals, by its step along the arc ({@link placePrecisionFor}). The
 * radius is the answer's, and its steps are exact 1-2-5 values; a tolerance keeps a computed
 * 0.01 ly from falling a decade.
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

/** The decimals one answer's places are read at, the chart's centre's and its observer's alike. */
export interface PlacePrecision {
  /** `RADIUS`, `HEIGHT` and the core arrow's distance: {@link placeDecimalsFor} of the radius. */
  readonly place: number;
  /** `ANGLE`: the fewest whose step along the arc is within the place's, at most `place`. */
  readonly angle: number;
}

/**
 * The decimals the furniture reads one answer's places at: its lengths by the query radius, and its
 * angle by the step that angle takes along the arc at the answer's centre (plan 09, P09.T42).
 *
 * @remarks
 * A step of 10⁻ᵃ degrees at a distance R from the galactic axis is R × π ÷ 180 × 10⁻ᵃ along the
 * arc, so the fewest decimals a whose step is no coarser than the place's 10⁻ᵈ ly is
 * ⌈d + log₁₀(π R ÷ 180)⌉, held between one and d, with the tolerance `placeDecimalsFor` uses at a
 * decade's edge. Near the central black hole that is one decimal, since five, the place's, would
 * step 10⁻¹⁰ ly along the arc there, digits the operator cannot act on (§ Numbers, units and time).
 * The cap at d keeps the slot's width and every chart from 10 ly as it was, at one decimal. It
 * leaves a stated limit, older than the fine steps: farther than 180 ÷ π ≈ 57.3 ly from the axis
 * the angle's step is coarser than the place's (at the Sun's 26,000 ly, 0.0045 ly on a 0.001 ly
 * chart, and 45 ly at 0.1°), so there `RADIUS`, `HEIGHT` and the `CURSOR` panel's coordinates place
 * the chart. Within {@link AXIS_TOLERANCE_LY} of the axis the angle is missing, and takes one.
 * The observer's `ANGLE` takes the centre's decimals, one precision for the quantity in one answer,
 * as its `RADIUS` takes the centre's.
 *
 * @param queryRadiusLy - The answer's query radius.
 * @param centreLy - The answer's centre, in the `GALACTIC` frame.
 */
export function placePrecisionFor(queryRadiusLy: number, centreLy: CentreLy): PlacePrecision {
  const place = placeDecimalsFor(queryRadiusLy);
  const { radiusLy } = cylindrical(vec3(...centreLy));
  if (!(radiusLy > AXIS_TOLERANCE_LY)) {
    return { place, angle: CENTRE_DECIMALS };
  }
  const wanted = Math.ceil(place + Math.log10((Math.PI * radiusLy) / 180) - 1e-9);
  return { place, angle: Math.min(place, Math.max(CENTRE_DECIMALS, wanted)) };
}

/**
 * A galactocentric radius as the chart's furniture writes it, without its unit: a place's
 * `RADIUS`, and the core arrow's distance.
 *
 * @param decimals - The chart's place decimals ({@link placeDecimalsFor}), 1 from a 10 ly chart.
 */
export function radiusReading(radiusLy: number, decimals: number): string {
  return formatLengthLy(radiusLy, decimals);
}

/**
 * A point's place in the `GALACTIC` frame as the chart's furniture writes it: `RADIUS`, `ANGLE`
 * (missing on the galactic axis, where it is undefined) and `HEIGHT`, at the answer's precision,
 * as the chart's centre and its observer are both written. Each slot holds its reading's longest
 * value at the place's decimals (`-65,536.00000` at five), so it is fixed for the chart; the
 * angle's, which take no more, keep the same slot.
 *
 * @param precision - The answer's decimals ({@link placePrecisionFor}), 1 and 1 from a 10 ly chart.
 */
export function placeReadings(
  positionLy: CentreLy,
  precision: PlacePrecision,
): ReadonlyArray<SpatialReading> {
  const place = cylindrical(vec3(...positionLy));
  const onAxis = !(place.radiusLy > AXIS_TOLERANCE_LY);
  const extra = precision.place - CENTRE_DECIMALS;
  return [
    {
      label: "RADIUS",
      value: radiusReading(place.radiusLy, precision.place),
      unit: "ly",
      widthCh: RADIUS_WIDTH_CH + extra,
    },
    {
      label: "ANGLE",
      value: onAxis ? null : formatBearingDeg(place.angleDeg, precision.angle),
      unit: "",
      widthCh: ANGLE_WIDTH_CH + extra,
    },
    {
      label: "HEIGHT",
      value: formatSigned(place.heightLy, precision.place),
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
 * @param precision - The answer's decimals ({@link placePrecisionFor}), the observer taking the
 * centre's.
 */
export function modeReading(mode: QueryMode, precision: PlacePrecision): SpatialMode {
  let reading: SpatialMode;
  switch (mode.kind) {
    case "now":
      reading = { name: "NOW", from: [] };
      break;
    case "observed":
      reading = { name: "OBSERVED FROM", from: placeReadings(mode.observerLy, precision) };
      break;
  }
  return reading;
}
