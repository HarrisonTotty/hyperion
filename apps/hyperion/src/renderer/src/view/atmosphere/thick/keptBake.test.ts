/**
 * A kept scene's thick bake made directly (plan R08, R08.T14.c): `TEST EARTH` (its aerosol from its
 * channel optics, its ozone under the reference Sun's curves) and `TEST MARS` (its dust on its own
 * TAMUdust optics) each bake one band, finite, with E⁰ at least 0.
 */
import { describe, expect, it } from "vitest";

import { MARS_FIT_COLUMN } from "../channels";
import { EARTH_COLUMN, EARTH_OZONE, R06_SOLAR_BAKE_SPECTRUM, REFERENCE_SUN } from "../earth";
import {
  EARTH_CASE,
  EARTH_REFERENCE_RADIUS_M,
  MARS_CASE,
  MARS_REFERENCE_RADIUS_M,
  referenceCaseBakeLights,
  type ReferenceCase,
} from "../reference/cases";
import { type MsSourceTable, THICK_BAKE_MODES, thickBakeBands } from "./bake";
import { bakeKeptCase, keptColumnOf, sphereBakeBody } from "./keptBake";

/** A table's E⁰ entries (the m = 0 mode of `bake.ts`'s layout), every channel, of its one slot. */
function zerothMode(table: MsSourceTable): ReadonlyArray<number> {
  const values = table.slots[0]?.values;
  if (values === undefined || table.slots.length !== 1) {
    throw new Error("the kept bake has one light slot");
  }
  return Array.from(values).filter((_, i) => Math.floor(i / 3) % THICK_BAKE_MODES === 0);
}

/** The least of many values. */
function leastOf(values: ReadonlyArray<number>): number {
  return values.reduce((least, value) => Math.min(least, value), Number.POSITIVE_INFINITY);
}

/** A kept case's one table: its one band under its one light. */
function bakedOf(referenceCase: ReferenceCase, gravityMS2: number, radiusM: number): MsSourceTable {
  const tables = bakeKeptCase(
    referenceCase,
    referenceCaseBakeLights(referenceCase),
    sphereBakeBody(radiusM, gravityMS2),
  );
  const [table] = tables;
  if (table === undefined || tables.length !== 1) {
    throw new Error(`case ${referenceCase.name} bakes one band`);
  }
  return table;
}

// The full grid, one band and one light, preparation and bake: TEST EARTH's 1.4 s (its ozone's
// fits in 15 bins) and TEST MARS's 0.9 s (its TAMUdust optics in 15 bins), at load 2.4
// (2026-10-27). Forty times the slower, rounded up, for a loaded machine.
const KEPT_BAKE_TIMEOUT_MS = 60_000;

describe("a kept case's bake lights", () => {
  it("are the reference Sun's spectrum and the medium's absorbers' specs", () => {
    expect(REFERENCE_SUN.bake_spectrum).toEqual([...R06_SOLAR_BAKE_SPECTRUM]);
    expect(referenceCaseBakeLights(EARTH_CASE)).toEqual({
      absorbers: [EARTH_OZONE],
      spectra: [R06_SOLAR_BAKE_SPECTRUM],
    });
    expect(referenceCaseBakeLights(MARS_CASE).absorbers).toEqual([]);
  });
});

describe("a kept case's direct bake", () => {
  it("takes one band on a sphere, at s = 1 on R_ref", () => {
    expect(
      thickBakeBands(sphereBakeBody(EARTH_REFERENCE_RADIUS_M, EARTH_COLUMN.referenceGravityMS2)),
    ).toEqual([
      {
        gravityRatio: 1,
        latitudeRad: 0,
        bottomRadiusM: EARTH_REFERENCE_RADIUS_M,
        coefficientFactor: 1,
      },
    ]);
  });

  it("bakes TEST EARTH's one band finite, E⁰ at least 0", { timeout: KEPT_BAKE_TIMEOUT_MS }, () => {
    const table = bakedOf(EARTH_CASE, EARTH_COLUMN.referenceGravityMS2, EARTH_REFERENCE_RADIUS_M);
    expect(table.slots[0]?.values.every(Number.isFinite)).toBe(true);
    expect(leastOf(zerothMode(table))).toBeGreaterThanOrEqual(0);
  });

  it("bakes TEST MARS's one band finite, E⁰ at least 0", { timeout: KEPT_BAKE_TIMEOUT_MS }, () => {
    const table = bakedOf(MARS_CASE, MARS_FIT_COLUMN.referenceGravityMS2, MARS_REFERENCE_RADIUS_M);
    expect(table.slots[0]?.values.every(Number.isFinite)).toBe(true);
    expect(leastOf(zerothMode(table))).toBeGreaterThanOrEqual(0);
  });

  it("refuses a kept medium with no sources", () => {
    expect(() => keptColumnOf({ medium: EARTH_CASE.medium })).toThrow(/no sources/u);
  });
});
