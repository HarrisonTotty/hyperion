import { SolarMassUnit } from "../../components/SolarMassUnit";

/**
 * What the `REMNANTS` map counts, in the words the page states it in (plan 08, P08.T15.b; the
 * guide's `REMNANTS`): `DISPLACED · INIT MASS ABOVE 8 M☉`, layer E's displaced remnants alone, not
 * the remnants retained in their populations.
 */
export function RemnantsCount() {
  return (
    <>
      DISPLACED · INIT MASS ABOVE 8 <SolarMassUnit />
    </>
  );
}
