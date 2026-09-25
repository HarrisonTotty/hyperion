/**
 * Number formats shared by every console, following "Numbers, units and time" in
 * `docs/frontend/ux-guidelines.md`.
 *
 * @remarks
 * Each function returns the value without its unit, except where it chooses one, when it returns
 * the unit beside the value or with it ({@link formatScaleLength}, {@link formatAge},
 * {@link formatPeriod}, {@link formatBodyDistance}, {@link formatPressure}), or where the unit is
 * written against the digits (the degree sign, the `yr` inside {@link formatUniverseTimeDhms}). A
 * missing value is the caller's em dash: every formatter here requires a finite number and throws
 * on anything else, since a `NaN` reaching the screen is a bug upstream.
 */

import { SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";

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
 * Formats a number in E notation, with three significant figures by default: `5.20E10`, `1.00E-4`.
 *
 * @remarks
 * For values outside a unit's ladder, where B612's lack of superscripts rules out `10⁻⁴`. In
 * `tick` form trailing zeros of the mantissa are dropped, so an exact power of ten reads `1E-4`.
 *
 * @param significantFigures - 1 to 21; three by the guide, two where a quantity's own precision is
 *   two figures, as a small planetary mass's is ({@link formatMassMearth}).
 */
export function formatSci(
  value: number,
  form: SciForm = "reading",
  significantFigures = 3,
): string {
  requireFinite(value, "value");
  const [mantissa, exponent] = value.toExponential(significantFigures - 1).split("e");
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

const MASS_MEARTH_TWO_DECIMALS_BELOW = 10;
const MASS_MEARTH_ONE_DECIMAL_BELOW = 100;
/** Below this many Earth masses two decimals would drop a figure, so two significant ones are kept. */
const MASS_MEARTH_TWO_FIGURES_BELOW = 0.1;
/**
 * Below this many Earth masses two significant figures need a run of zeros: E notation, in the
 * guide's three figures.
 */
const MASS_MEARTH_SCI_BELOW = 0.001;
/** The significant figures of a decimal mass from 0.001 up to {@link MASS_MEARTH_TWO_FIGURES_BELOW}. */
const MASS_MEARTH_SMALL_FIGURES = 2;

/**
 * Formats a mass in Earth masses without its unit: two significant figures below 0.1, two decimals
 * below 10, one below 100, and whole numbers from there, digits grouped from five as every number
 * is (plan 13, P13.T8.b): `0.012`, `0.33`, `17.1`, `4131`, `12,480`.
 *
 * @remarks
 * Every planetary mass on the ship is in M⊕, from a moon to a 13 Jupiter-mass giant (plan 14,
 * D19); the unit is drawn by `EarthMassUnit`, never typed. Each step is decided on the rounded
 * text, so that 9.996 reads `10.0` and never `10.00`, and 0.0996 reads `0.10` on either side.
 * Below 0.1 M⊕ two decimals would keep one figure or none, so the Moon would read `0.01`; there the
 * mass keeps two significant figures instead, `0.012` (the orchestrator's ruling 35), and below
 * 0.001 M⊕ it is in the guide's E notation with its three, `1.57E-4` for Ceres (ruling 44.1, which
 * keeps the two figures to the decimals from 0.001 to 0.1 M⊕).
 */
export function formatMassMearth(massMearth: number): string {
  requireFinite(massMearth, "mass");
  const twoFigures = Math.abs(roundSignificant(massMearth, MASS_MEARTH_SMALL_FIGURES));
  if (massMearth !== 0 && twoFigures < MASS_MEARTH_SCI_BELOW) {
    return formatSci(massMearth);
  }
  if (massMearth !== 0 && twoFigures < MASS_MEARTH_TWO_FIGURES_BELOW) {
    return numberFormat(`significant:${MASS_MEARTH_SMALL_FIGURES}`, {
      minimumSignificantDigits: MASS_MEARTH_SMALL_FIGURES,
      maximumSignificantDigits: MASS_MEARTH_SMALL_FIGURES,
      signDisplay: "negative",
    }).format(massMearth);
  }
  const twoDecimals = formatNumber(massMearth, 2);
  if (Math.abs(Number(twoDecimals.replaceAll(",", ""))) < MASS_MEARTH_TWO_DECIMALS_BELOW) {
    return twoDecimals;
  }
  const oneDecimal = formatNumber(massMearth, 1);
  if (Math.abs(Number(oneDecimal.replaceAll(",", ""))) < MASS_MEARTH_ONE_DECIMAL_BELOW) {
    return oneDecimal;
  }
  return formatNumber(massMearth, 0);
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

// Three significant figures, digits grouped from five, as each quantity below is read.
function significant(value: number): string {
  return numberFormat("significant:3", {
    minimumSignificantDigits: 3,
    maximumSignificantDigits: 3,
    useGrouping: "min2",
    signDisplay: "negative",
  }).format(value);
}

/**
 * A value to three significant figures, or in E notation where it lies outside `[low, high)` in
 * magnitude and is not zero: the guide's form for a value outside its unit's ladder, which three
 * significant figures in the unit could only show with a run of zeros.
 */
function significantOrSci(value: number, low: number, high: number): string {
  const magnitude = Math.abs(roundSignificant(value, 3));
  return value !== 0 && (magnitude < low || magnitude >= high)
    ? formatSci(value)
    : significant(value);
}

/**
 * Formats a value to the guide's three significant figures, without its unit, digits grouped from
 * five: `0.0893`, `265`, `12,500`; below 0.001 and from 1,000,000 in magnitude in E notation.
 *
 * @remarks
 * For a quantity with no ladder of its own, such as a pulsar's spin period in seconds or a kick in
 * km/s.
 */
export function formatSignificant(value: number): string {
  requireFinite(value, "value");
  return significantOrSci(value, 0.001, 1_000_000);
}

/** Days in a Julian year, the year of every `yr` on the wire. */
const DAYS_PER_JULIAN_YEAR = 365.25;

/** Periods below this many days, after rounding, are read in days. */
const PERIOD_DAYS_BELOW = 1_000;

/** Unit of a period as {@link formatPeriod} writes it: days, or Julian years. */
export type PeriodUnit = "d" | "yr";

/** A period split into its digits and unit, so that the unit can be set apart. */
export interface FormattedPeriod {
  readonly value: string;
  readonly unit: PeriodUnit;
}

/**
 * Formats an orbital or rotation period to three significant figures: in `d` below 1,000 days and
 * in `yr` from there (plan 14, T38.b).
 *
 * @remarks
 * The switch is made after rounding, so 999.6 d reads `2.74 yr`, never `1,000 d`. A body's period
 * does not change as the display time does, so, as for an age, no hysteresis is needed. The year is
 * the Julian year of every `yr` on the wire. A negative period, as a retrograde rotation may be
 * given, keeps its `-`. Below 0.01 d, a quarter of an hour, and from 10,000 yr, the value is in E
 * notation: `5.00E-3 d`, `1.20E6 yr`.
 *
 * @param periodDays - Days of 86,400 s.
 */
export function formatPeriod(periodDays: number): FormattedPeriod {
  requireFinite(periodDays, "period");
  if (Math.abs(roundSignificant(periodDays, 3)) < PERIOD_DAYS_BELOW) {
    return { value: significantOrSci(periodDays, 0.01, PERIOD_DAYS_BELOW), unit: "d" };
  }
  return {
    value: significantOrSci(periodDays / DAYS_PER_JULIAN_YEAR, 0, 10_000),
    unit: "yr",
  };
}

/**
 * Kilometres in one astronomical unit, 149,597,870.7.
 *
 * @remarks
 * The au is 149,597,870,700 m exactly (IAU 2012 Resolution B2).
 */
export const KM_PER_AU = 149_597_870.7;

/** Unit of a distance in a planetary system as {@link formatBodyDistance} writes it. */
export type BodyDistanceUnit = "km" | "Mm" | "Gm" | "AU";

/** A distance split into its digits and unit, so that the unit can be set apart. */
export interface FormattedDistance {
  readonly value: string;
  readonly unit: BodyDistanceUnit;
}

/**
 * How far past either edge of its unit's band, as a share of that edge's distance, a distance may
 * go before {@link formatBodyDistance} leaves the unit it was shown in.
 */
export const DISTANCE_HYSTERESIS = 0.05;

interface DistanceBand {
  readonly unit: BodyDistanceUnit;
  readonly kmPerUnit: number;
  /** Where the band ends and the next begins, in km. */
  readonly belowKm: number;
}

// Each band ends at 1,000 of its unit, but for Gm, which gives way to AU at 0.1 AU (15.0 Gm) so
// that every planet but the closest-in reads in AU and a giant's moons in Mm and Gm.
const DISTANCE_BANDS: ReadonlyArray<DistanceBand> = [
  { unit: "km", kmPerUnit: 1, belowKm: 1_000 },
  { unit: "Mm", kmPerUnit: 1_000, belowKm: 1_000_000 },
  { unit: "Gm", kmPerUnit: 1_000_000, belowKm: 0.1 * KM_PER_AU },
  { unit: "AU", kmPerUnit: KM_PER_AU, belowKm: Number.POSITIVE_INFINITY },
];

function bandOf(unit: BodyDistanceUnit): DistanceBand {
  const band = DISTANCE_BANDS.find((candidate) => candidate.unit === unit);
  if (band === undefined) {
    throw new Error(`no distance band for ${unit}`);
  }
  return band;
}

/**
 * Formats a distance in a planetary system to three significant figures in the guide's scaled
 * units, `km`, `Mm`, `Gm`, then `AU`, switching unit with hysteresis (plan 14, T38.b).
 *
 * @remarks
 * Each unit holds a band of distances: km up to 1,000 km, Mm up to 1,000 Mm, Gm up to 0.1 AU, and
 * AU from there. Without a previous unit, or once the distance has left the previous unit's band by
 * more than {@link DISTANCE_HYSTERESIS} of either edge, the band the distance falls in is taken,
 * decided after rounding so that 999.96 km reads `1.00 Mm`. Inside that margin the previous unit is
 * kept, so that a distance moving across a boundary with the display time does not flicker between
 * `999 Mm` and `1.00 Gm`. Below 0.01 km and from 1,000,000 AU the value is in E notation.
 *
 * @param previous - The unit this distance was last shown in, which the caller keeps; `null` for
 *   the first showing.
 */
export function formatBodyDistance(
  distanceKm: number,
  previous: BodyDistanceUnit | null,
): FormattedDistance {
  requireFinite(distanceKm, "distance");
  const magnitudeKm = Math.abs(distanceKm);
  const nominal =
    DISTANCE_BANDS.find(
      (band) => roundSignificant(magnitudeKm / band.kmPerUnit, 3) * band.kmPerUnit < band.belowKm,
    ) ?? bandOf("AU");
  let chosen = nominal;
  if (previous !== null) {
    const kept = bandOf(previous);
    const index = DISTANCE_BANDS.indexOf(kept);
    const aboveKm = index > 0 ? (DISTANCE_BANDS[index - 1]?.belowKm ?? 0) : 0;
    const inMargin =
      magnitudeKm >= aboveKm * (1 - DISTANCE_HYSTERESIS) &&
      magnitudeKm < kept.belowKm * (1 + DISTANCE_HYSTERESIS);
    chosen = inMargin ? kept : nominal;
  }
  const inUnit = distanceKm / chosen.kmPerUnit;
  const value =
    chosen.unit === "km"
      ? significantOrSci(inUnit, 0.01, Number.POSITIVE_INFINITY)
      : significantOrSci(inUnit, 0, chosen.unit === "AU" ? 1_000_000 : Number.POSITIVE_INFINITY);
  return { value, unit: chosen.unit };
}

/** Decimals every eccentricity is read to, on every display that shows one. */
export const ECCENTRICITY_DECIMALS = 4;

/** Metres in a kilometre. */
const M_PER_KM = 1_000;

/** Seconds in a day of 86,400 s, the day of every `d`. */
const SECONDS_IN_DAY = 86_400;

/** An orbit's period, semi-major axis and eccentricity, each as it is read (plan 11, P11.T14). */
export interface FormattedOrbit {
  readonly period: FormattedPeriod;
  readonly semiMajorAxis: FormattedDistance;
  /** Plain, to {@link ECCENTRICITY_DECIMALS} decimals: `0.0167`. */
  readonly eccentricity: string;
}

/**
 * Formats an orbit's period, semi-major axis and eccentricity as every orbit is read on the ship: the
 * period by {@link formatPeriod}, the axis by {@link formatBodyDistance} with no unit held from
 * before, since an orbit's elements do not move, and the eccentricity to four decimals (plan 11,
 * P11.T14).
 *
 * @param periodS - Seconds.
 * @param semiMajorAxisM - Metres.
 * @throws RangeError for a value that is not finite, or an eccentricity outside [0, 1).
 */
export function formatOrbit(
  periodS: number,
  semiMajorAxisM: number,
  eccentricity: number,
): FormattedOrbit {
  requireFinite(eccentricity, "eccentricity");
  if (eccentricity < 0 || eccentricity >= 1) {
    throw new RangeError(`a bound orbit's eccentricity cannot be ${eccentricity}`);
  }
  return {
    period: formatPeriod(periodS / SECONDS_IN_DAY),
    semiMajorAxis: formatBodyDistance(semiMajorAxisM / M_PER_KM, null),
    eccentricity: formatNumber(eccentricity, ECCENTRICITY_DECIMALS),
  };
}

/**
 * Formats a star's luminosity in solar luminosities to three significant figures, without the unit:
 * `1.00`, `0.0850`, `25,400`; below 0.001 L☉, as a white dwarf's or a brown dwarf's, and from
 * 1,000,000 L☉ in E notation: `1.08E-4`.
 *
 * @remarks
 * The unit, `L☉`, is drawn by `SolarUnit`, never typed (the owner's draft of the guide's units).
 *
 * @throws RangeError for a negative luminosity.
 */
export function formatLuminosityLsun(luminosityLsun: number): string {
  requireFinite(luminosityLsun, "luminosity");
  if (luminosityLsun < 0) {
    throw new RangeError(`a luminosity cannot be ${luminosityLsun} L☉`);
  }
  return significantOrSci(luminosityLsun, 0.001, 1_000_000);
}

/**
 * Formats a star's radius in solar radii to three significant figures, without the unit: `1.00`,
 * `0.0115` for a white dwarf, `1500` for a red supergiant; below 0.001 R☉ and from 100,000 R☉ in E
 * notation.
 *
 * @remarks
 * Neutron stars and black holes are read in kilometres instead ({@link formatRadiusKm}), where
 * `1.75E-5 R☉` would read as a failure of the unit (the orchestrator's ruling 36). The unit, `R☉`, is
 * drawn by `SolarUnit`, never typed.
 *
 * @throws RangeError for a negative radius.
 */
export function formatRadiusRsun(radiusRsun: number): string {
  requireFinite(radiusRsun, "radius");
  if (radiusRsun < 0) {
    throw new RangeError(`a radius cannot be ${radiusRsun} R☉`);
  }
  return significantOrSci(radiusRsun, 0.001, 100_000);
}

/**
 * Kilometres in the nominal solar radius, 695,700 (IAU 2015 Resolution B3), the value of
 * `units::consts::SOLAR_RADIUS_M` in `hyperion-sim`.
 */
export const KM_PER_RSUN = 695_700;

/**
 * Formats a radius in kilometres to three significant figures, without the unit, digits grouped
 * from five: `12.2` for a neutron star, `36.9` for a black hole's horizon; below 0.01 km in E
 * notation.
 *
 * @throws RangeError for a negative radius.
 */
export function formatRadiusKm(radiusKm: number): string {
  requireFinite(radiusKm, "radius");
  if (radiusKm < 0) {
    throw new RangeError(`a radius cannot be ${radiusKm} km`);
  }
  return significantOrSci(radiusKm, 0.01, Number.POSITIVE_INFINITY);
}

/**
 * Formats a temperature in whole kelvins, without the unit: `288`, `1400`, `12,480`.
 *
 * @throws RangeError for a temperature below absolute zero, which no body has.
 */
export function formatTemperatureK(temperatureK: number): string {
  requireFinite(temperatureK, "temperature");
  if (temperatureK < 0) {
    throw new RangeError(`a temperature cannot be ${temperatureK} K`);
  }
  return formatNumber(temperatureK, 0);
}

/** Unit of a pressure as {@link formatPressure} writes it. */
export type PressureUnit = "Pa" | "kPa" | "MPa";

/** A pressure split into its digits and unit, so that the unit can be set apart. */
export interface FormattedPressure {
  readonly value: string;
  readonly unit: PressureUnit;
}

/**
 * Formats a pressure to three significant figures in `Pa` below 1,000 Pa, `kPa` below 1,000 kPa and
 * `MPa` from there (plan 14, T38.b): `610 Pa`, `101 kPa`, `9.20 MPa`.
 *
 * @remarks
 * The switch is made after rounding, so 999.9 Pa reads `1.00 kPa`. A surface pressure is a body's
 * own and does not change with the display time, so no hysteresis is needed. Below 0.01 Pa, as an
 * exosphere's, and from 10,000 MPa the value is in E notation: `3.00E-10 Pa`.
 *
 * @throws RangeError for a negative pressure.
 */
export function formatPressure(pressurePa: number): FormattedPressure {
  requireFinite(pressurePa, "pressure");
  if (pressurePa < 0) {
    throw new RangeError(`a pressure cannot be ${pressurePa} Pa`);
  }
  if (roundSignificant(pressurePa, 3) < 1_000) {
    return { value: significantOrSci(pressurePa, 0.01, 1_000), unit: "Pa" };
  }
  if (roundSignificant(pressurePa / 1_000, 3) < 1_000) {
    return { value: significant(pressurePa / 1_000), unit: "kPa" };
  }
  return { value: significantOrSci(pressurePa / 1_000_000, 0, 10_000), unit: "MPa" };
}

/**
 * Formats a surface gravity in m/s² to three significant figures, without the unit: `9.81`,
 * `24.8`, `0.284`; below 0.01 m/s², as a small moon's, in E notation: `5.70E-3`.
 *
 * @throws RangeError for a negative gravity.
 */
export function formatGravity(gravityMPerS2: number): string {
  requireFinite(gravityMPerS2, "gravity");
  if (gravityMPerS2 < 0) {
    throw new RangeError(`a surface gravity cannot be ${gravityMPerS2} m/s²`);
  }
  return significantOrSci(gravityMPerS2, 0.01, Number.POSITIVE_INFINITY);
}

const SECONDS_PER_DAY = 86_400;
const SECONDS_PER_HOUR = 3_600;
const SECONDS_PER_MINUTE = 60;

/** `whole` split into the count of `unit` it holds and what is left, exactly, for integers. */
function divide(whole: number, unit: number): readonly [number, number] {
  const left = whole % unit;
  return [(whole - left) / unit, left];
}

function twoDigits(value: number): string {
  return String(value).padStart(2, "0");
}

/**
 * Formats a universe time as signed Julian years, then days and the time of day in the guide's
 * `MET` form, without the time system's label: `+12 yr 183/14:08:33` (plan 14, D24).
 *
 * @remarks
 * **Built to plan 14's design note D24, which needs the owner's confirmation**, as plan 05's `UT`
 * did; the guide's entry for it is the `doc` lane's draft for the owner. It is for the `SYSTEM`
 * display's time, whose steps reach down to an hour, which {@link formatUniverseTimeYr} cannot
 * show.
 *
 * The time is taken from the instant's whole seconds, rounded towards negative infinity as the wire
 * gives them, and split with integer arithmetic alone, never through a float of years or days; the
 * nanoseconds are not shown. The sign is the whole time's, as a countdown's `T-` is, so one second
 * before the epoch reads `-0 yr 000/00:00:01` and the epoch itself `+0 yr 000/00:00:00`. The years
 * are grouped from five digits, as every number is; the days, 0 to 365 in a Julian year, are three
 * digits and the hours, minutes and seconds two, so that the field keeps its width.
 *
 * @throws RangeError when `seconds` is not a whole number that a JavaScript number holds exactly.
 */
export function formatUniverseTimeDhms(time: UniverseTime): string {
  const { seconds } = time;
  if (!Number.isSafeInteger(seconds)) {
    throw new RangeError(`${String(seconds)} is not a whole number of seconds`);
  }
  const [years, inYear] = divide(Math.abs(seconds), SECONDS_PER_JULIAN_YEAR);
  const [days, inDay] = divide(inYear, SECONDS_PER_DAY);
  const [hours, inHour] = divide(inDay, SECONDS_PER_HOUR);
  const [minutes, secondsLeft] = divide(inHour, SECONDS_PER_MINUTE);
  const sign = seconds < 0 ? "-" : "+";
  const clock = [hours, minutes, secondsLeft].map(twoDigits).join(":");
  return `${sign}${formatNumber(years, 0)} yr ${String(days).padStart(3, "0")}/${clock}`;
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
