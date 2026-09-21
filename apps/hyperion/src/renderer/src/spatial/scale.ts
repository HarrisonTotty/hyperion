/**
 * 1-2-5 steps for scale bars, grids and radius selectors.
 *
 * @remarks
 * Every step is built from an integer mantissa and an integer power of ten, dividing for negative
 * exponents, so that 0.01, 0.02 and 0.05 are the same doubles as their literals.
 */

const MANTISSAS = [1, 2, 5] as const;

// A value within this relative distance of a step counts as that step, so that a computed 0.02
// that lands a bit off the double closest to 0.02 is not demoted to 0.01.
const STEP_TOLERANCE = 1e-12;

function step(mantissa: number, exponent: number): number {
  return exponent >= 0 ? mantissa * 10 ** exponent : mantissa / 10 ** -exponent;
}

function requirePositive(x: number): void {
  if (!(x > 0) || !Number.isFinite(x)) {
    throw new RangeError(`a 1-2-5 step needs a positive finite value, got ${String(x)}`);
  }
}

function candidates(x: number): number[] {
  const exponent = Math.floor(Math.log10(x));
  const values: number[] = [];
  for (let e = exponent - 1; e <= exponent + 1; e += 1) {
    for (const mantissa of MANTISSAS) {
      values.push(step(mantissa, e));
    }
  }
  return values;
}

function floorStep(x: number, tolerance: number): number {
  requirePositive(x);
  const limit = x * (1 + tolerance);
  let best = 0;
  for (const value of candidates(x)) {
    if (value <= limit && value > best) {
      best = value;
    }
  }
  return best;
}

/**
 * The largest value of {1, 2, 5} × 10ⁿ at or below `x`.
 *
 * @throws RangeError when `x` is not positive and finite.
 */
export function floor125(x: number): number {
  return floorStep(x, STEP_TOLERANCE);
}

/**
 * The smallest value of {1, 2, 5} × 10ⁿ at or above `x`.
 *
 * @throws RangeError when `x` is not positive and finite.
 */
export function ceil125(x: number): number {
  requirePositive(x);
  const limit = x * (1 - STEP_TOLERANCE);
  let best = Number.POSITIVE_INFINITY;
  for (const value of candidates(x)) {
    if (value >= limit && value < best) {
      best = value;
    }
  }
  return best;
}

/**
 * A unit a scale bar may be read in, as a rung of a unit ladder.
 *
 * @remarks
 * A ladder lists units from the largest down. A bar is read in the first unit whose 1-2-5 length
 * is at least that unit's `minSceneLength`, so that a light-year ladder can step to `500 AU`
 * below 0.01 ly instead of showing 0.005 ly.
 */
export interface ScaleUnit {
  /** How many of this unit make one scene unit. */
  readonly perSceneUnit: number;
  /** The shortest bar, in scene units, read in this unit; ignored for the last unit. */
  readonly minSceneLength: number;
}

/** The ladder of a scene read in its own unit only. */
export const SCENE_UNIT_ONLY: ReadonlyArray<ScaleUnit> = [{ perSceneUnit: 1, minSceneLength: 0 }];

/** A scale bar's length in scene units and on screen. */
export interface ScaleBarLength {
  readonly length: number;
  readonly lengthPx: number;
}

/**
 * The longest bar of a 1-2-5 length, in the ladder's units, that fits in `maxBarPx`.
 *
 * @remarks
 * The bar is never longer than `maxBarPx` and, within one unit, never shorter than 40% of it.
 *
 * @param pxPerUnit - The view's scale, pixels per scene unit.
 * @param units - The unit ladder, largest first; defaults to the scene unit alone.
 */
export function scaleBar(
  pxPerUnit: number,
  maxBarPx: number,
  units: ReadonlyArray<ScaleUnit> = SCENE_UNIT_ONLY,
): ScaleBarLength {
  const longest = maxBarPx / pxPerUnit;
  let length = 0;
  for (const [index, unit] of units.entries()) {
    length = floorStep(longest * unit.perSceneUnit, 0) / unit.perSceneUnit;
    const isLast = index === units.length - 1;
    if (isLast || length >= unit.minSceneLength * (1 - STEP_TOLERANCE)) {
      break;
    }
  }
  if (!(length > 0)) {
    throw new RangeError("a scale bar needs a non-empty unit ladder");
  }
  // Converting to a smaller unit and back can overshoot the maximum in the last bit.
  return { length, lengthPx: Math.min(maxBarPx, length * pxPerUnit) };
}

/**
 * The spacing of the reference grid and its rings for a query radius: 2.5 to 5 rings out to it.
 */
export function gridSpacing(queryRadius: number): number {
  return floor125(queryRadius / 2.5);
}

function radiusSteps(): number[] {
  const steps: number[] = [];
  for (let exponent = -2; exponent <= 2; exponent += 1) {
    for (const mantissa of MANTISSAS) {
      steps.push(step(mantissa, exponent));
    }
  }
  return steps;
}

/**
 * The query radii a chart offers, 0.01 ly to 500 ly in 1-2-5 steps.
 *
 * @remarks
 * Down to a few hundredths of a light-year, the scale near the central black hole (brainstorm,
 * "The local chart in 3D", Controls).
 */
export const RADIUS_STEPS_LY: ReadonlyArray<number> = radiusSteps();
