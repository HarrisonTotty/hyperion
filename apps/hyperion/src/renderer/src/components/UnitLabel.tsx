import type { Unit } from "@hyperion/protocol";
import type { ReactElement } from "react";

import { SolarMassUnit } from "./SolarMassUnit";

interface UnitLabelProps {
  readonly unit: Unit;
}

/**
 * Each unit's symbol, or `null` for none. A unit added to the protocol without an entry here is a
 * type error.
 */
const SYMBOLS: Readonly<Record<Unit, ReactElement | null>> = {
  none: null,
  count: null,
  msun: <SolarMassUnit />,
  ly: <span className="unit">ly</span>,
  myr: <span className="unit">Myr</span>,
  gyr: <span className="unit">Gyr</span>,
  km_per_s: <span className="unit">km/s</span>,
  deg_per_myr: <span className="unit">°/Myr</span>,
  deg: <span className="unit">°</span>,
  per_ly3: <span className="unit">/ly³</span>,
  // The gas's astronomers' units (plan 07, P07.T10.b), composed as `/ly³` is, since B612 has no
  // superscript minus.
  per_cm3: <span className="unit">/cm³</span>,
  k_per_cm3: <span className="unit">K/cm³</span>,
  mag: <span className="unit">mag</span>,
};

function unitSymbol(unit: Unit): ReactElement | null {
  // `Unit` grows without a protocol bump (galaxy plan 04's reserved list), and the decoder trusts
  // the generated types, so a newer server's unit arrives typed as one of these: it has no symbol.
  return Object.hasOwn(SYMBOLS, unit) ? SYMBOLS[unit] : null;
}

/**
 * The symbol of one of plan 04's units, as the guide writes it.
 *
 * @remarks
 * Every character is in B612, except the sun sign of `M☉`, which `SolarMassUnit` draws.
 * Dimensionless values and counts have no symbol, so nothing is rendered for them, and nor is it
 * for a unit this build does not know (`decision-gg-port-d0.md` §1), whose value reads Missing.
 */
export function UnitLabel({ unit }: UnitLabelProps) {
  return unitSymbol(unit);
}
