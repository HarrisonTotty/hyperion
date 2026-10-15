/**
 * Galaxy parameters as the `PARAMETERS` panel reads them: a label, the value's digits, its unit
 * and where it came from.
 *
 * @remarks
 * Nothing is converted: the server sends every value in a unit the guide allows (plan 04, design
 * note 11; plan 05, design note D8a), so this module only chooses a precision for each unit.
 */
import type { Parameter, ParameterOrigin, Unit } from "@hyperion/protocol";

import { formatMassMsun, formatNumber, formatSci } from "../../lib/format";
import { parameterLabel } from "./parameterLabels";

/** Where a parameter's value came from, in words. */
export type OriginLabel = "DRAWN" | "DERIVED" | "FIXED";

/** One parameter, ready for a row of the table. */
export interface ParameterReading {
  readonly key: string;
  readonly label: string;
  /**
   * The value's digits, or its text in upper case; `null` when the value is Missing, a number in a
   * unit this build does not know.
   */
  readonly text: string | null;
  /** The unit to set beside a number, or `null` for a text value or a Missing one. */
  readonly unit: Unit | null;
  readonly origin: OriginLabel;
}

/** Masses and counts from this value up read in E notation, since their digits would not fit. */
const E_NOTATION_FROM = 1e7;

/** Masses from this value up read as whole, grouped numbers. */
const WHOLE_MASS_FROM = 1e4;

/** Below this, a dimensionless value's three significant figures would need a long string. */
const SMALLEST_PLAIN = 1e-3;

const significant = new Intl.NumberFormat("en-US", {
  minimumSignificantDigits: 3,
  maximumSignificantDigits: 3,
  useGrouping: "min2",
  signDisplay: "negative",
});

/** Three significant figures, in E notation outside [0.001, 10⁷). */
function threeFigures(value: number): string {
  const magnitude = Math.abs(value);
  if (magnitude !== 0 && (magnitude < SMALLEST_PLAIN || magnitude >= E_NOTATION_FROM)) {
    return formatSci(value);
  }
  return significant.format(value);
}

function massText(valueMsun: number): string {
  const magnitude = Math.abs(valueMsun);
  if (magnitude >= E_NOTATION_FROM) {
    return formatSci(valueMsun);
  }
  return magnitude >= WHOLE_MASS_FROM ? formatNumber(valueMsun, 0) : formatMassMsun(valueMsun);
}

function countText(count: number): string {
  return Math.abs(count) >= E_NOTATION_FROM ? formatSci(count) : formatNumber(count, 0);
}

/**
 * How a number in each unit is written, at the precision the operator can use.
 *
 * @remarks
 * Masses and counts from 10⁷ up in E notation (`5.20E10`), whole and grouped from five digits
 * below that, and masses under 10⁴ as elsewhere on the ship; lengths grouped to one decimal;
 * angles and rates to two decimals; speeds to one; times, shares and other dimensionless values to
 * three significant figures; densities in E notation; pressures to three significant figures;
 * magnitudes to two decimals. A unit added to the protocol without an entry here is a type error.
 */
const NUMBER_TEXT: Readonly<Record<Unit, (value: number) => string>> = {
  msun: massText,
  count: countText,
  ly: (value) => formatNumber(value, 1),
  deg: (value) => formatNumber(value, 2),
  deg_per_myr: (value) => formatNumber(value, 2),
  km_per_s: (value) => formatNumber(value, 1),
  myr: threeFigures,
  gyr: threeFigures,
  none: threeFigures,
  per_ly3: formatSci,
  per_cm3: formatSci,
  k_per_cm3: threeFigures,
  mag: (value) => formatNumber(value, 2),
};

/**
 * The digits of a number in `unit`, or `null` when this build does not know the unit.
 *
 * @remarks
 * `Unit` is one of the enums galaxy plan 04 names as growing, which a newer server may send a new
 * string of without a protocol bump (`decision-gg-port-d0.md` §1). The value then reads Missing,
 * never as an empty cell or as a number in a unit the operator is not shown.
 */
function numberText(value: number, unit: Unit): string | null {
  // The decoder trusts the generated types, so an unknown string arrives typed as a `Unit`.
  return Object.hasOwn(NUMBER_TEXT, unit) ? NUMBER_TEXT[unit](value) : null;
}

function originLabel(origin: ParameterOrigin): OriginLabel {
  let label: OriginLabel;
  switch (origin) {
    case "drawn":
      label = "DRAWN";
      break;
    case "derived":
      label = "DERIVED";
      break;
    case "fixed":
      label = "FIXED";
      break;
  }
  return label;
}

/**
 * Reads a parameter for the table: its label, formatted value, unit and origin in words.
 *
 * @remarks
 * A number in a unit this build does not know reads Missing, with no unit.
 */
export function toReading(parameter: Parameter): ParameterReading {
  const { key, origin, value } = parameter;
  const common = { key, label: parameterLabel(key), origin: originLabel(origin) };
  if (value.type === "text") {
    return { ...common, text: value.value.toUpperCase(), unit: null };
  }
  const text = numberText(value.value, value.unit);
  return { ...common, text, unit: text === null ? null : value.unit };
}
