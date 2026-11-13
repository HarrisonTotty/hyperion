/**
 * A kept scene's thick bake made directly (plan R08, R08.T14.c): `TEST EARTH` (its aerosol from its
 * channel optics, its ozone under the reference Sun's curves) and `TEST MARS` (its dust on its own
 * TAMUdust optics) each bake one band, finite, with J⁰ at least 0 (below the horizontal, at least
 * −10⁻⁶ of the largest, R08.T14.d.2); and TEST EARTH's m = 2 packs into
 * G₂ to its word's bound against its `f32` values (R08.T14.d.1).
 */
import { describe, expect, it } from "vitest";

import { leastZerothModes } from "../../../test/thickTables";
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
import { kernelMedium } from "../tables";
import { type MsSourceTable, THICK_BAKE_MODES, thickBakeBands } from "./bake";
import { bakeKeptCase, keptColumnOf, sphereBakeBody } from "./keptBake";
import { packThickTable, thickCurveChannel } from "./read";

/**
 * Whether a table's J⁰ is at least 0 above the horizontal, and at least −10⁻⁶ of its largest below
 * it (`test/thickTables.ts`'s `leastZerothModes`).
 */
function expectZerothModesAtLeastZero(table: MsSourceTable): void {
  const slot = table.slots[0];
  if (slot === undefined || table.slots.length !== 1) {
    throw new Error("the kept bake has one light slot");
  }
  const { above, belowShare } = leastZerothModes(slot.values, table.sunCosines);
  expect(above).toBeGreaterThanOrEqual(0);
  expect(belowShare).toBeGreaterThanOrEqual(-1e-6);
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

  it(
    "bakes TEST EARTH's one band finite, J⁰ at least 0 above the horizontal and at least −10⁻⁶ of its largest below it",
    { timeout: KEPT_BAKE_TIMEOUT_MS },
    () => {
      const table = bakedOf(EARTH_CASE, EARTH_COLUMN.referenceGravityMS2, EARTH_REFERENCE_RADIUS_M);
      expect(table.slots[0]?.values.every(Number.isFinite)).toBe(true);
      expectZerothModesAtLeastZero(table);
    },
  );

  it(
    "bakes TEST MARS's one band finite, J⁰ at least 0 above the horizontal and at least −10⁻⁶ of its largest below it",
    { timeout: KEPT_BAKE_TIMEOUT_MS },
    () => {
      const table = bakedOf(
        MARS_CASE,
        MARS_FIT_COLUMN.referenceGravityMS2,
        MARS_REFERENCE_RADIUS_M,
      );
      expect(table.slots[0]?.values.every(Number.isFinite)).toBe(true);
      expectZerothModesAtLeastZero(table);
    },
  );

  it("refuses a kept medium with no sources", () => {
    expect(() => keptColumnOf({ medium: EARTH_CASE.medium })).toThrow(/no sources/u);
  });

  it(
    "packs TEST EARTH's m = 2 into G₂ to 1 in 255 of each texel's largest, against its f32 values",
    { timeout: KEPT_BAKE_TIMEOUT_MS },
    () => {
      const table = bakedOf(EARTH_CASE, EARTH_COLUMN.referenceGravityMS2, EARTH_REFERENCE_RADIUS_M);
      const packed = packThickTable([table], kernelMedium(EARTH_CASE.medium));
      const values = table.slots[0]?.values ?? new Float32Array(0);
      const views = table.travelCosines.length;
      let worst = 0;
      let curveMost = 0;
      let levelMost = 0;
      for (let a = 0; a < table.altitudesM.length; a += 1) {
        for (const [s, mu0] of table.sunCosines.entries()) {
          for (const [v, muV] of table.travelCosines.entries()) {
            const cell = (a * table.sunCosines.length + s) * views + v;
            const at = cell * THICK_BAKE_MODES * 3;
            // The f32 control: the bake's J² over the cell's sines, unrounded past f32.
            const sines = (1 - mu0 * mu0) * (1 - muV * muV);
            const control = [0, 1, 2].map((c) => (values[at + 6 + c] ?? Number.NaN) / sines);
            const largest = Math.max(...control.map(Math.abs));
            const word = packed.texels[cell * 4 + 3] ?? 0;
            for (let c = 0; c < 3; c += 1) {
              levelMost = Math.max(levelMost, values[at + c] ?? Number.NaN);
              curveMost = Math.max(curveMost, Math.abs(values[at + 6 + c] ?? Number.NaN));
              if (largest > 0) {
                // The word's bound: 1 in 255 of the texel's largest, or 2⁻²⁵ where that is larger.
                const bound = Math.max(largest / 255, 2 ** -25);
                const read = thickCurveChannel(word, c);
                worst = Math.max(worst, Math.abs(read - (control[c] ?? Number.NaN)) / bound);
              }
            }
          }
        }
      }
      expect([curveMost > 0.1 * levelMost, worst <= 1 + 1e-6]).toEqual([true, true]);
    },
  );
});
