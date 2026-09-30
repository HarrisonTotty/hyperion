import type { Unit } from "@hyperion/protocol";
import type { ReactElement } from "react";

import { SolarMassUnit } from "./SolarMassUnit";

interface UnitLabelProps {
  readonly unit: Unit;
}

function unitSymbol(unit: Unit): ReactElement | null {
  let symbol: ReactElement | null;
  switch (unit) {
    case "none":
    case "count":
      symbol = null;
      break;
    case "msun":
      symbol = <SolarMassUnit />;
      break;
    case "ly":
      symbol = <span className="unit">ly</span>;
      break;
    case "myr":
      symbol = <span className="unit">Myr</span>;
      break;
    case "gyr":
      symbol = <span className="unit">Gyr</span>;
      break;
    case "km_per_s":
      symbol = <span className="unit">km/s</span>;
      break;
    case "deg_per_myr":
      symbol = <span className="unit">°/Myr</span>;
      break;
    case "deg":
      symbol = <span className="unit">°</span>;
      break;
    case "per_ly3":
      symbol = <span className="unit">/ly³</span>;
      break;
  }
  return symbol;
}

/**
 * The symbol of one of plan 04's units, as the guide writes it.
 *
 * @remarks
 * Every character is in B612, except the sun sign of `M☉`, which `SolarMassUnit` draws.
 * Dimensionless values and counts have no symbol, so nothing is rendered for them. A unit added to
 * the protocol without a case here is a type error.
 */
export function UnitLabel({ unit }: UnitLabelProps) {
  return unitSymbol(unit);
}
