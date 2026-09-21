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
  /** The value's digits, or its text in upper case. */
  readonly text: string;
  /** The unit to set beside a number, or `null` for a text value. */
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
 * The digits of a number in `unit`, at the precision the operator can use.
 *
 * @remarks
 * Masses and counts from 10⁷ up in E notation (`5.20E10`), whole and grouped from five digits
 * below that, and masses under 10⁴ as elsewhere on the ship; lengths grouped to one decimal;
 * angles and rates to two decimals; speeds to one; times, shares and other dimensionless values to
 * three significant figures; densities in E notation.
 */
function numberText(value: number, unit: Unit): string {
  let text: string;
  switch (unit) {
    case "msun":
      text = massText(value);
      break;
    case "count":
      text = countText(value);
      break;
    case "ly":
      text = formatNumber(value, 1);
      break;
    case "deg":
    case "deg_per_myr":
      text = formatNumber(value, 2);
      break;
    case "km_per_s":
      text = formatNumber(value, 1);
      break;
    case "myr":
    case "gyr":
    case "none":
      text = threeFigures(value);
      break;
    case "per_ly3":
      text = formatSci(value);
      break;
  }
  return text;
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

/** Reads a parameter for the table: its label, formatted value, unit and origin in words. */
export function toReading(parameter: Parameter): ParameterReading {
  const { key, origin, value } = parameter;
  const common = { key, label: parameterLabel(key), origin: originLabel(origin) };
  return value.type === "number"
    ? { ...common, text: numberText(value.value, value.unit), unit: value.unit }
    : { ...common, text: value.value.toUpperCase(), unit: null };
}
