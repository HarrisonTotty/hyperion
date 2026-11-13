/**
 * Toy inputs for the atomic-line tools' tests (plan R08, R08.T4.h): Barklem and Collet's tables in
 * their layout, a Saha data set and a line, their values made up.
 */

import { type KuruczLine, parseBarklemTable, type SahaData, SOLAR_ABUNDANCES } from "./atomicLines";

/** A toy table 8 or 7 in Barklem and Collet's layout: species rows over a few temperatures. */
export function toyTable(
  rows: Readonly<Record<string, ReadonlyArray<number>>>,
  temperaturesK: ReadonlyArray<number>,
): string {
  return [
    "# Partition functions Q",
    `#   T [K]   ${temperaturesK.map((t) => t.toExponential(5)).join("   ")}`,
    "#",
    ...Object.entries(rows).map(
      ([name, values]) =>
        `${name.padEnd(7)}   ${values.map((v) => v.toExponential(5)).join("   ")}`,
    ),
  ].join("\n");
}

/**
 * Toy Saha data: every stage's Q constant (2, the first ions' 1), every element's ionisation
 * energies stated (Na's 5.14 eV, the other metals' 9, 20 and 40 eV), H₂ wholly dissociated.
 */
export function toySahaData(): SahaData {
  const temperaturesK = [100, 1_000, 3_000, 10_000];
  const rows: Record<string, number[]> = {};
  const energies = new Map<string, readonly [number, number, number]>();
  for (const e of SOLAR_ABUNDANCES) {
    for (const stage of ["I", "II", "III"]) {
      rows[`${e.symbol}_${stage}`] = temperaturesK.map(() => (stage === "II" ? 1 : 2));
    }
    energies.set(
      e.symbol,
      e.symbol === "H" ? [13.6, -1, -1] : [e.symbol === "Na" ? 5.14 : 9, 20, 40],
    );
  }
  return {
    ionisationEnergiesEv: energies,
    partitionFunctions: parseBarklemTable(toyTable(rows, temperaturesK), "toy 8"),
    equilibriumConstants: parseBarklemTable(
      toyTable({ H2: temperaturesK.map(() => 30) }, temperaturesK),
      "toy 7",
    ),
  };
}

/** A line at a vacuum wavelength with log gf and lower level, its upper level by default the photon's. */
export function lineAt(
  vacuumNm: number,
  logGf: number,
  lowerPerCm: number,
  upperPerCm = lowerPerCm + 1e7 / vacuumNm,
): KuruczLine {
  return {
    listedNm: vacuumNm,
    vacuumNm,
    logGf,
    lowerPerCm,
    upperPerCm,
    lowerJ: 0.5,
    upperJ: 1.5,
    lowerLabel: "a",
    upperLabel: "b",
    reference: "XX",
    logGammaRadPerS: 0,
    logGammaStarkPerElectron: 0,
    logGammaVanDerWaalsPerHydrogen: 0,
  };
}
