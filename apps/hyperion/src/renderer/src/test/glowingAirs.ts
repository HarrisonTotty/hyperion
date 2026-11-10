/**
 * Airs whose disc tables the thermal emission's tests read (plan R08, R08.T18): the ultra-hot
 * giant's medium, a grey column, and disc tables baked whole from a medium with its multiple
 * scattering held at 0, as `thermalDisc.test.ts` bakes its grey columns, glowing where given a
 * temperature.
 */

import type { DiscAir } from "../view/atmosphere/discReflectance";
import type { DiscReflectanceTable } from "../view/atmosphere/discTable";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import { tablesPlanOf } from "../view/atmosphere/tables";
import { type MediumTemperature, thermalSourceOf } from "../view/atmosphere/thermalSource";
import { bakeDiscReflectance } from "../view/atmosphere/thick/discBake";
import { packThickTable } from "../view/atmosphere/thick/read";
import { patternedThickBake } from "./thickTables";

// WASP-121b, its figures of the order L. Delrez et al. give (MNRAS 458 (2016) 4025; not re-read in
// the pod, which reached no source): an F6V star of about 6,460 K and 1.46 R☉, 0.0254 au away, the
// planet of about 1.8 R_J. Its dayside is taken isothermal at 2,700 K, the order of its dayside
// brightness temperature (T. M. Evans et al., Nature 548 (2017) 58), its continuum and molecular
// opacity as a grey absorber, opaque at τ 20, under H₂'s Rayleigh, and its scale height about
// kT ÷ (μ m_H g) with μ 2.3 and g about 9 m s⁻², some 1,000 km. The conclusion holds wherever its
// red reflectance is under about 0.075 at 2,700 K (the science check), as ultra-hot Jupiters'
// optical albedos are.

/** The ultra-hot giant's star's effective temperature, K. */
export const HOT_GIANT_STAR_TEMPERATURE_K = 6_460;
/** Its star's radius, m. */
export const HOT_GIANT_STAR_RADIUS_M = 1.458 * 6.957e8;
/** Its orbit's radius, m. */
export const HOT_GIANT_ORBIT_M = 0.025_44 * 1.495_978_707e11;
/** Its radius, m. */
export const HOT_GIANT_RADIUS_M = 1.807 * 7.1492e7;
/** Its dayside's temperature, K: its brightness temperature's order. */
export const HOT_GIANT_DAYSIDE_K = 2_700;
const GIANT_SCALE_HEIGHT_M = 1.0e6;
/** The top of its air, m: ten scale heights. */
export const HOT_GIANT_TOP_M = 10 * GIANT_SCALE_HEIGHT_M;

/** A grey term of vertical optical depth `scattering` and `absorption` per channel. */
function greyTerm(
  name: string,
  scattering: readonly [number, number, number],
  absorption: number,
): AtmosphereMedium["terms"][number] {
  const column = GIANT_SCALE_HEIGHT_M * -Math.expm1(-HOT_GIANT_TOP_M / GIANT_SCALE_HEIGHT_M);
  const a = absorption / column;
  return {
    name,
    density: { kind: "exponential", scaleHeightM: GIANT_SCALE_HEIGHT_M },
    scattering: [scattering[0] / column, scattering[1] / column, scattering[2] / column],
    absorption: [a, a, a],
    phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
  };
}

/** The ultra-hot giant's air (the figures above). */
export const HOT_GIANT: AtmosphereMedium = {
  name: "ultra-hot giant",
  topHeightM: HOT_GIANT_TOP_M,
  groundAlbedo: [0, 0, 0],
  terms: [greyTerm("H2 Rayleigh", [0.03, 0.07, 0.17], 0), greyTerm("continuum", [0, 0, 0], 20)],
};

/** A grey column over 100 km of vertical optical depth `tau` and single-scattering albedo `omega`. */
export function greyColumn(tau: number, omega: number): AtmosphereMedium {
  const scaleHeightM = 8_000;
  const extinction = tau / (scaleHeightM * -Math.expm1(-100_000 / scaleHeightM));
  return {
    name: "grey",
    topHeightM: 100_000,
    groundAlbedo: [0, 0, 0],
    terms: [
      {
        name: "grey",
        density: { kind: "exponential", scaleHeightM },
        scattering: [omega * extinction, omega * extinction, omega * extinction],
        absorption: [(1 - omega) * extinction, (1 - omega) * extinction, (1 - omega) * extinction],
        phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
      },
    ],
  };
}

/** A column isothermal at `temperatureK` up to `topM`. */
export function isothermal(temperatureK: number, topM: number): MediumTemperature {
  return {
    heightsM: Float64Array.of(0, topM),
    temperaturesK: Float64Array.of(temperatureK, temperatureK),
  };
}

/**
 * The disc table of `medium` on a sphere of `radiusM`, one band and light slot, its J held at 0
 * (its single scattering and transmittance alone), and its own light where `temperatureK` is given,
 * the column isothermal at it.
 */
export function bakedDiscTable(
  medium: AtmosphereMedium,
  radiusM: number,
  temperatureK: number | null,
): DiscReflectanceTable {
  const { bands } = tablesPlanOf(null, radiusM);
  const zero = patternedThickBake(bands, medium, 1);
  for (const band of zero) {
    for (const slot of band.slots) {
      slot.values.fill(0);
    }
  }
  return bakeDiscReflectance({
    medium,
    curves: [],
    bands,
    source: packThickTable(zero, medium),
    ...(temperatureK === null
      ? {}
      : {
          thermal: thermalSourceOf(isothermal(temperatureK, medium.topHeightM), medium.topHeightM),
        }),
  });
}

/** A table's air as a disc record carries it: one band at s = 1, with no spheroid. */
export function discAirOf(table: DiscReflectanceTable): DiscAir {
  return { table, texture: { kind: "texture", name: "disc table" }, lnBands: [0], gravity: null };
}
