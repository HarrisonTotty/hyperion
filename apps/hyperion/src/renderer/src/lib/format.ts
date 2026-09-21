/**
 * Number formats shared by every console, following "Numbers, units and time" in
 * `docs/frontend/ux-guidelines.md`.
 *
 * @remarks
 * Each function returns the value without its unit, except where its name says it chooses one
 * ({@link formatScaleLength}, {@link formatAge}) or the unit is written against the digits (the
 * degree sign). A missing value is the caller's em dash: every formatter here requires a finite
 * number and throws on anything else, since a `NaN` reaching the screen is a bug upstream.
 */

/** Name of the universe time system, shown before every universe time (`UT +12.50 yr`). */
export const TIME_SYSTEM_LABEL = "UT";

/**
 * Astronomical units in one light-year, 63,241.077.
 *
 * @remarks
 * The au is 149,597,870,700 m exactly (IAU 2012 Resolution B2). The light-year is the distance
 * light travels in a Julian year of 365.25 days of 86,400 s at 299,792,458 m/s, which is
 * 9,460,730,472,580,800 m (IAU Style Manual, with the Julian year of the IAU 1976 system of
 * astronomical constants).
 */
export const AU_PER_LY = 9_460_730_472_580_800 / 149_597_870_700;

/**
 * Scale lengths below this many light-years are read in astronomical units.
 *
 * @remarks
 * A scale bar's unit ladder uses the same threshold, so that its 1-2-5 steps and
 * {@link formatScaleLength} agree: `0.01 ly`, then `500 AU`.
 */
export const SCALE_AU_BELOW_LY = 0.01;

type SignStyle = "negative" | "always";

const numberFormats = new Map<string, Intl.NumberFormat>();

// Building an `Intl.NumberFormat` costs far more than formatting with one, and lists and readouts
// format many values a render, so each distinct format is built once.
function numberFormat(key: string, options: Intl.NumberFormatOptions): Intl.NumberFormat {
  const cached = numberFormats.get(key);
  if (cached !== undefined) {
    return cached;
  }
  const created = new Intl.NumberFormat("en-US", options);
  numberFormats.set(key, created);
  return created;
}

function requireFinite(value: number, what: string): void {
  if (!Number.isFinite(value)) {
    throw new RangeError(`${what} must be a finite number, got ${String(value)}`);
  }
}

function fixed(value: number, decimals: number, sign: SignStyle, integerDigits = 1): string {
  requireFinite(value, "value");
  // "negative" leaves out the sign of a value that rounds to zero, so -0.004 never reads -0.00.
  // "exceptZero" does the same for "always", and the plus is added back for zero below.
  const format = numberFormat(`fixed:${decimals}:${sign}:${integerDigits}`, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
    minimumIntegerDigits: integerDigits,
    useGrouping: "min2",
    signDisplay: sign === "always" ? "exceptZero" : "negative",
  });
  const text = format.format(value);
  return sign === "always" && !text.startsWith("-") && !text.startsWith("+") ? `+${text}` : text;
}

/**
 * Formats a number with a fixed count of decimals, grouping digits in threes from five digits.
 *
 * @remarks
 * Negative values carry `-`; a value that rounds to zero carries no sign.
 *
 * @param decimals - Digits after the decimal point, 0 to 20.
 * @example formatNumber(12480, 0) === "12,480"; formatNumber(1234.56, 1) === "1234.6"
 */
export function formatNumber(value: number, decimals: number): string {
  return fixed(value, decimals, "negative");
}

/**
 * Formats a number like {@link formatNumber} but always with a sign, for quantities whose
 * direction matters; zero reads `+0.00`.
 */
export function formatSigned(value: number, decimals: number): string {
  return fixed(value, decimals, "always");
}

/** Whether {@link formatSci} writes a reading or a legend tick. */
export type SciForm = "reading" | "tick";

/**
 * Formats a number in E notation with three significant figures: `5.20E10`, `1.00E-4`.
 *
 * @remarks
 * For values outside a unit's ladder, where B612's lack of superscripts rules out `10⁻⁴`. In
 * `tick` form trailing zeros of the mantissa are dropped, so an exact power of ten reads `1E-4`.
 */
export function formatSci(value: number, form: SciForm = "reading"): string {
  requireFinite(value, "value");
  const [mantissa, exponent] = value.toExponential(2).split("e");
  if (mantissa === undefined || exponent === undefined) {
    throw new Error(`toExponential gave no exponent for ${String(value)}`);
  }
  const shownMantissa = form === "tick" ? mantissa.replace(/\.?0+$/, "") : mantissa;
  return `${shownMantissa}E${exponent.replace("+", "")}`;
}

/**
 * Formats a length in light-years with a fixed count of decimals, without the unit.
 *
 * @param decimals - Kept the same for every distance on one chart (see plan 05, note D21).
 */
export function formatLengthLy(ly: number, decimals: number): string {
  return formatNumber(ly, decimals);
}

/**
 * Formats a scale-bar length, with its unit: `20 ly`, `0.05 ly`, or `500 AU` below 0.01 ly.
 *
 * @remarks
 * Scale lengths are 1-2-5 values in their unit, so three significant figures show them exactly.
 */
export function formatScaleLength(ly: number): string {
  requireFinite(ly, "length");
  const sig = numberFormat("scale", { maximumSignificantDigits: 3, useGrouping: "min2" });
  // A tolerance keeps a computed 0.01 ly from falling to AU through rounding in the last bit.
  if (Math.abs(ly) >= SCALE_AU_BELOW_LY * (1 - 1e-9)) {
    return `${sig.format(ly)} ly`;
  }
  return `${sig.format(ly * AU_PER_LY)} AU`;
}

/**
 * Formats a bearing as three zero-padded digits and a degree sign, `000°` to `359°`.
 *
 * @remarks
 * The angle is wrapped into [0, 360) after rounding, so 359.6° reads `000°`.
 *
 * @param decimals - Digits after the decimal point (`045.0°` with 1).
 */
export function formatBearingDeg(angleDeg: number, decimals = 0): string {
  requireFinite(angleDeg, "angle");
  const step = 10 ** decimals;
  const rounded = Math.round(angleDeg * step) / step;
  const wrapped = ((rounded % 360) + 360) % 360;
  return `${fixed(wrapped, decimals, "negative", 3)}°`;
}

/** Formats a signed whole angle with at least two digits: `+30°`, `-05°`, `+00°`. */
export function formatSignedDeg(angleDeg: number): string {
  return `${fixed(angleDeg, 0, "always", 2)}°`;
}

/** Unit of an age as {@link formatAge} writes it. */
export type AgeUnit = "Myr" | "Gyr";

/** An age split into its digits and unit, so that the unit can be set apart. */
export interface FormattedAge {
  readonly value: string;
  readonly unit: AgeUnit;
}

const MYR_PER_GYR = 1_000;

function roundSignificant(value: number, digits: number): number {
  return value === 0 ? 0 : Number(value.toPrecision(digits));
}

/**
 * Formats an age to three significant figures, in `Myr` below 1,000 Myr and `Gyr` from there.
 *
 * @remarks
 * The switch is made after rounding, so 999.9 Myr reads `1.00 Gyr`, never `1,000 Myr`. Ages are
 * static on a chart, so no hysteresis is needed.
 */
export function formatAge(ageMyr: number): FormattedAge {
  requireFinite(ageMyr, "age");
  const sig = numberFormat("age", {
    minimumSignificantDigits: 3,
    maximumSignificantDigits: 3,
    useGrouping: false,
    signDisplay: "negative",
  });
  if (Math.abs(roundSignificant(ageMyr, 3)) < MYR_PER_GYR) {
    return { value: sig.format(ageMyr), unit: "Myr" };
  }
  return { value: sig.format(ageMyr / MYR_PER_GYR), unit: "Gyr" };
}

const MASS_TWO_DECIMALS_BELOW_MSUN = 10;

/**
 * Formats a mass in solar masses without its unit: two decimals below 10, one from there.
 *
 * @remarks
 * The unit is drawn by `SolarMassUnit`, never typed.
 */
export function formatMassMsun(massMsun: number): string {
  const twoDecimals = formatNumber(massMsun, 2);
  // Decided on the rounded text, so that 9.996 reads 10.0 and never 10.00.
  const shown = Math.abs(Number(twoDecimals.replaceAll(",", "")));
  return shown < MASS_TWO_DECIMALS_BELOW_MSUN ? twoDecimals : formatNumber(massMsun, 1);
}

/**
 * Formats a universe time as signed years with two decimals, without the unit: `+12.50`.
 *
 * @remarks
 * Shown after {@link TIME_SYSTEM_LABEL} and before `yr`.
 */
export function formatUniverseTimeYr(timeYr: number): string {
  return formatSigned(timeYr, 2);
}

/**
 * Formats the rows of a scrolling list that are in view and the total: `12-24 of 87`.
 *
 * @remarks
 * Numbers are grouped from five digits like every other: `9980-10,000 of 12,480`.
 *
 * @param firstIndex - 0-based index of the first row in view, as `windowRange` reports it.
 * @param lastIndex - 0-based index of the last row in view, inclusive.
 * @throws RangeError when the indexes do not describe rows of a list of `total` rows.
 */
export function formatListPosition(firstIndex: number, lastIndex: number, total: number): string {
  const valid =
    Number.isInteger(firstIndex) &&
    Number.isInteger(lastIndex) &&
    Number.isInteger(total) &&
    firstIndex >= 0 &&
    firstIndex <= lastIndex &&
    lastIndex < total;
  if (!valid) {
    throw new RangeError(`rows ${firstIndex}-${lastIndex} are not in a list of ${total}`);
  }
  return `${formatNumber(firstIndex + 1, 0)}-${formatNumber(lastIndex + 1, 0)} of ${formatNumber(total, 0)}`;
}
