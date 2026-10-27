/**
 * R08.T11's per-planet rows of the giants (plan R08, Design note 11; R08.T11's fourth item, and
 * R08.T11.e): the κ slices, the latitude bands, the bytes {@link AtmosphereTables} allocates and the
 * widening it takes for the Jupiter- and Saturn-class figures, an ice giant and a giant at plan 14's
 * cap, set against the 2 MB row ({@link PER_PLANET_ROW_BYTES}), and what Design note 11's two
 * widenings in its order, `KAPPA_STEP` to 0.3 and then `BAND_STEP` to 0.15, would give each.
 *
 * @remarks
 * A record of the figures as they stand, on two airs: a thin H₂–He one with no absorber
 * (`saturnClassMedium`, Design note 11's thin worlds) and the ice giant's methane-absorbing one
 * (`iceGiantMethaneBody`'s medium and curves, one light slot), each on every figure, so that the
 * row's dependence on the figure is seen apart from the air's. Since R08.T11.e the column tables a
 * medium does not read are one texel a slice (`tables.ts`'s `columnTableSizes`), and a row still
 * over 2 MB takes Design note 11's widenings in the client (`tables.ts`'s `rowBodyOf`): the rows
 * "as built" are the tables' own, the widened ones the slicing respaced at those steps over the
 * built one's extremes by `oblate.ts`'s `respacedSlicing`, and R08.T11.a's rows, every column
 * table at full size, are kept beside them (`every`).
 */

import { describe, expect, it } from "vitest";

import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../../lib/system/constants";
import {
  fourAbsorberMedium,
  gravityOfSpheroid,
  iceGiantMethaneBody,
  SATURN_SPHEROID,
  saturnClassMedium,
  URANUS_SPHEROID,
} from "../../test/atmosphereFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { tableRadiusM } from "./hillaire";
import type { AtmosphereMedium } from "./medium";
import {
  BAND_STEP,
  type BodyGravity,
  bodyGravity,
  KAPPA_STEP,
  type LevelSpheroid,
  respacedSlicing,
  WIDE_BAND_STEP,
  WIDE_KAPPA_STEP,
} from "./oblate";
import {
  type AbsorberCurveSlots,
  type AtmosphereBody,
  AtmosphereTables,
  NO_ABSORBER_CURVES,
  PER_PLANET_ROW_BYTES,
  perPlanetRowOf,
  rowBodyOf,
  type RowWidening,
  tablesPlanOf,
} from "./tables";

/**
 * Jupiter at 1 bar: a and c of Archinal et al. 2018 (Celest. Mech. Dyn. Astron. 130, 22), the
 * nominal GM of IAU 2015 Resolution B3 (the simulation's `GM_JUPITER`), and System III's
 * 870.536° d⁻¹ (Archinal et al. 2018), 9.925 h: `oblate.test.ts`'s `JUPITER`.
 */
const JUPITER: LevelSpheroid = {
  equatorialRadiusM: 71_492e3,
  polarRadiusM: 66_854e3,
  gmM3S2: 1.266_865_3e17,
  angularVelocityRadS: (870.536 * (Math.PI / 180)) / 86_400,
};

/** Plan 14's flattening cap (`FLATTENING_CAP`, `planetary/params.rs`). */
const FLATTENING_CAP = 0.2;

/** The capped giant's volumetric radius, m, and mean density, kg m⁻³: `oblate.test.ts`'s. */
const CAPPED_RADIUS_M = 7e7;
const CAPPED_DENSITY_KG_M3 = 690;

/**
 * The capped giant's true spin, rad s⁻¹: `oblate.test.ts`'s `CAPPED_OMEGA_RAD_S`, 1.1 break-up
 * periods, past which its free flattening exceeds the cap; the capped law reads none of it but its
 * presence (a rotation section).
 */
const CAPPED_OMEGA_RAD_S =
  1 /
  (1.1 *
    Math.sqrt(
      CAPPED_RADIUS_M ** 3 /
        (GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 *
          ((4 / 3) * Math.PI * CAPPED_RADIUS_M ** 3 * CAPPED_DENSITY_KG_M3)),
    ));

/** The capped giant's C ÷ Ma²: `oblate.test.ts`'s `CAPPED_C`. */
const CAPPED_MOMENT_OF_INERTIA = 0.21;

/** A giant's datum and gravity. */
interface Giant {
  readonly name: string;
  readonly figure: LevelSpheroid;
  readonly gravity: BodyGravity;
}

/**
 * A giant at plan 14's cap (Design note 17, `capped`): `oblate.test.ts`'s capped giant, drawn at
 * f = 0.2 as plan 14's `Spheroid::from_volumetric` lays it, a = R (1 − f)^(−⅓) and c = a (1 − f),
 * its gravity at the Darwin–Radau spin ω_fig that levels that figure (its true spin has no level
 * f = 0.2 figure, so the capped law takes none).
 */
function cappedGiant(): Giant {
  const massKg = (4 / 3) * Math.PI * CAPPED_RADIUS_M ** 3 * CAPPED_DENSITY_KG_M3;
  const equatorialRadiusM = CAPPED_RADIUS_M / Math.cbrt(1 - FLATTENING_CAP);
  const datum = {
    equatorialRadiusM,
    polarRadiusM: equatorialRadiusM * (1 - FLATTENING_CAP),
  };
  const gravity = bodyGravity(
    {
      figure: datum,
      massKg,
      angularVelocityRadS: CAPPED_OMEGA_RAD_S,
      bulkGravityMS2: Number.NaN,
      figureLaw: { law: "capped", momentOfInertiaFactor: CAPPED_MOMENT_OF_INERTIA },
    },
    tableRadiusM(datum),
  );
  const spheroid = gravity.spheroid;
  if (spheroid === null) {
    throw new Error("a capped figure's gravity has its level spheroid");
  }
  return { name: "the cap", figure: spheroid, gravity };
}

const GIANTS: ReadonlyArray<Giant> = [
  { name: "Jupiter", figure: JUPITER, gravity: gravityOfSpheroid(JUPITER) },
  { name: "Saturn", figure: SATURN_SPHEROID, gravity: gravityOfSpheroid(SATURN_SPHEROID) },
  { name: "Uranus", figure: URANUS_SPHEROID, gravity: gravityOfSpheroid(URANUS_SPHEROID) },
  cappedGiant(),
];

/** An air and its lights' curves. */
interface Air {
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
}

const METHANE = iceGiantMethaneBody();

/** The thin H₂–He air with no absorber: Design note 11's thin worlds. */
const THIN_AIR: Air = { name: "thin", medium: saturnClassMedium(), curves: NO_ABSORBER_CURVES };

/** The ice giant's methane-absorbing air under one star's curves. */
const METHANE_AIR: Air = { name: "methane", medium: METHANE.medium, curves: METHANE.curves };

/** A slicing's steps: as built, κ widened to 0.3, then the bands to 0.15 as well. */
interface Steps {
  readonly name: string;
  readonly kappa: number;
  readonly band: number;
}

/** The built steps. */
const AS_BUILT: Steps = { name: "as built", kappa: KAPPA_STEP, band: BAND_STEP };

/** Design note 11's widenings, each respaced from the built slicing's extremes. */
const WIDENED: ReadonlyArray<Steps> = [
  { name: "κ 0.3", kappa: WIDE_KAPPA_STEP, band: BAND_STEP },
  { name: "κ 0.3, s 0.15", kappa: WIDE_KAPPA_STEP, band: WIDE_BAND_STEP },
];

/** The giant at plan 14's cap. */
function theCap(): Giant {
  const cap = GIANTS.find(({ name }) => name === "the cap");
  if (cap === undefined) {
    throw new Error("the giants hold the cap");
  }
  return cap;
}

/** A giant's body at a slicing's steps. */
function bodyAt(giant: Giant, steps: Steps): AtmosphereBody {
  const slicing = respacedSlicing(giant.gravity.slicing, steps.kappa, steps.band);
  return { figure: giant.figure, gravity: { ...giant.gravity, slicing } };
}

/** A giant's row: slices, bands, bytes and the widening its tables take. */
type GiantRow = readonly [number, number, number, RowWidening];

/**
 * A giant's row on an air at a slicing's steps as its tables build it: slices, bands, bytes and the
 * widening they take.
 */
async function rowOf(giant: Giant, air: Air, steps: Steps): Promise<GiantRow> {
  const radiusM = tableRadiusM(giant.figure);
  const engine = await countingRenderEngine();
  const tables = new AtmosphereTables(
    engine,
    air.medium,
    radiusM,
    air.curves,
    bodyAt(giant, steps),
  );
  try {
    return [tables.plan.kappa.length, tables.plan.bands.length, tables.rowBytes, tables.widening];
  } finally {
    tables.dispose();
  }
}

/** Every giant's rows on `air`, by name: as built, then at each widening. */
async function rowsOn(air: Air): Promise<Record<string, Record<string, GiantRow>>> {
  const entries = await Promise.all(
    GIANTS.map(async (giant) => {
      const byStep = await Promise.all(
        [AS_BUILT, ...WIDENED].map(
          async (steps) => [steps.name, await rowOf(giant, air, steps)] as const,
        ),
      );
      return [giant.name, Object.fromEntries(byStep)] as const;
    }),
  );
  return Object.fromEntries(entries);
}

/**
 * Every giant's row on `air` as R08.T11.a recorded it, every column table at full size and no
 * widening (`perPlanetRowOf` under `every`, at the built slicing and the widened ones): slices,
 * bands and bytes. No giant here takes the slot layers, so the row's base is its bytes.
 */
function rowsBeforeOn(air: Air): Record<string, Record<string, readonly number[]>> {
  return Object.fromEntries(
    GIANTS.map((giant) => [
      giant.name,
      Object.fromEntries(
        [AS_BUILT, ...WIDENED].map((steps) => {
          const plan = tablesPlanOf(bodyAt(giant, steps), tableRadiusM(giant.figure));
          const row = perPlanetRowOf(air.medium, air.curves, plan, "every");
          return [steps.name, [plan.kappa.length, plan.bands.length, row.baseBytes]];
        }),
      ),
    ]),
  );
}

describe("R08.T11's per-planet rows of the giants (Design note 11)", () => {
  // Recorded 2026-10-27 by R08.T11.a (plan 08, "Deviations in T11, as built"): [slices, bands,
  // bytes]. Every slice's depth, absorber-column and pressure-column tables at full size whatever
  // the air reads (393,224 B a slice, R08.T8.e's record), so a thin Saturn-class world stood over
  // the 2 MB row, and the cap over it after both widenings, against Design note 11's 0.57 MB thin
  // Jupiter-class, 0.72 MB Saturn-class, 0.29 MB ice giant and 1.3–1.4 MB thin cap.
  it("keeps R08.T11.a's rows of the thin air, every column table at full size", () => {
    expect(rowsBeforeOn(THIN_AIR)).toEqual({
      Jupiter: {
        "as built": [4, 3, 1_648_036],
        "κ 0.3": [3, 3, 1_254_812],
        "κ 0.3, s 0.15": [3, 3, 1_254_812],
      },
      Saturn: {
        "as built": [5, 4, 2_065_836],
        "κ 0.3": [3, 4, 1_279_388],
        "κ 0.3, s 0.15": [3, 3, 1_254_812],
      },
      Uranus: {
        "as built": [2, 2, 837_012],
        "κ 0.3": [2, 2, 837_012],
        "κ 0.3, s 0.15": [2, 2, 837_012],
      },
      "the cap": {
        "as built": [10, 7, 4_105_684],
        "κ 0.3": [6, 7, 2_532_788],
        "κ 0.3, s 0.15": [6, 5, 2_483_636],
      },
    });
  });

  it("keeps R08.T11.a's rows of the methane air, every column table at full size", () => {
    expect(rowsBeforeOn(METHANE_AIR)).toEqual({
      Jupiter: {
        "as built": [4, 3, 1_663_920],
        "κ 0.3": [3, 3, 1_270_696],
        "κ 0.3, s 0.15": [3, 3, 1_270_696],
      },
      Saturn: {
        "as built": [5, 4, 2_081_720],
        "κ 0.3": [3, 4, 1_295_272],
        "κ 0.3, s 0.15": [3, 3, 1_270_696],
      },
      Uranus: {
        "as built": [2, 2, 852_896],
        "κ 0.3": [2, 2, 852_896],
        "κ 0.3, s 0.15": [2, 2, 852_896],
      },
      "the cap": {
        "as built": [10, 7, 4_121_568],
        "κ 0.3": [6, 7, 2_548_672],
        "κ 0.3, s 0.15": [6, 5, 2_499_520],
      },
    });
  });

  // Since R08.T11.e (2026-10-27): [slices, bands, bytes, the widening the tables take]. With no
  // absorber both column tables are one texel a slice, 131,096 B a slice with the depth's, so every
  // thin giant is under 2 MB as built and none widens: Jupiter 0.60 MB, Saturn 0.76, Uranus 0.31
  // and the cap 1.48, against Design note 11's 0.57, 0.72, 0.29 and 1.3–1.4 (its estimate leaves
  // out the terms buffer, the body's table and the curves' texel).
  it("records the thin air's slices, bands, bytes and widening on each giant, as built and widened (R08.T11.e)", async () => {
    expect(await rowsOn(THIN_AIR)).toEqual({
      Jupiter: {
        "as built": [4, 3, 599_524, "none"],
        "κ 0.3": [3, 3, 468_428, "none"],
        "κ 0.3, s 0.15": [3, 3, 468_428, "none"],
      },
      Saturn: {
        "as built": [5, 4, 755_196, "none"],
        "κ 0.3": [3, 4, 493_004, "none"],
        "κ 0.3, s 0.15": [3, 3, 468_428, "none"],
      },
      Uranus: {
        "as built": [2, 2, 312_756, "none"],
        "κ 0.3": [2, 2, 312_756, "none"],
        "κ 0.3, s 0.15": [2, 2, 312_756, "none"],
      },
      "the cap": {
        "as built": [10, 7, 1_484_404, "none"],
        "κ 0.3": [6, 7, 960_020, "none"],
        "κ 0.3, s 0.15": [6, 5, 910_868, "none"],
      },
    });
  });

  // One absorber reads the pressure-column table and not the absorber-column one, 262,160 B a
  // slice: the cap, 2.81 MB at its ten slices, is the one row over 2 MB, and its tables widen κ to
  // 0.3, six slices at 1.76 MB, so that the bands keep their step and fewer bands are never taken.
  it("records the methane air's slices, bands, bytes and widening on each giant, as built and widened (R08.T11.e)", async () => {
    expect(await rowsOn(METHANE_AIR)).toEqual({
      Jupiter: {
        "as built": [4, 3, 1_139_664, "none"],
        "κ 0.3": [3, 3, 877_504, "none"],
        "κ 0.3, s 0.15": [3, 3, 877_504, "none"],
      },
      Saturn: {
        "as built": [5, 4, 1_426_400, "none"],
        "κ 0.3": [3, 4, 902_080, "none"],
        "κ 0.3, s 0.15": [3, 3, 877_504, "none"],
      },
      Uranus: {
        "as built": [2, 2, 590_768, "none"],
        "κ 0.3": [2, 2, 590_768, "none"],
        "κ 0.3, s 0.15": [2, 2, 590_768, "none"],
      },
      "the cap": {
        "as built": [6, 7, 1_762_288, "kappa"],
        "κ 0.3": [6, 7, 1_762_288, "none"],
        "κ 0.3, s 0.15": [6, 5, 1_713_136, "none"],
      },
    });
  });

  it("would have put the methane air's cap at 2.81 MB unwidened, its ten slices at one texel where unread", () => {
    const cap = theCap();
    const plan = tablesPlanOf(bodyAt(cap, AS_BUILT), tableRadiusM(cap.figure));
    const row = perPlanetRowOf(METHANE_AIR.medium, METHANE_AIR.curves, plan);
    expect([plan.kappa.length, plan.bands.length, row.baseBytes]).toEqual([10, 7, 2_810_928]);
  });

  it("builds the methane air's cap in its widened slicing, its figure and gravity its own", async () => {
    const cap = theCap();
    const given = bodyAt(cap, AS_BUILT);
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(
      engine,
      METHANE_AIR.medium,
      tableRadiusM(cap.figure),
      METHANE_AIR.curves,
      given,
    );
    try {
      const built = tables.body;
      expect([
        built === given,
        built?.gravity.slicing.kappa.length,
        built?.gravity.slicing.bandGravityRatio.length,
        built?.figure,
        built?.gravity.referenceGravityMS2,
        tables.plan.kappa.length,
      ]).toEqual([false, 6, 7, given.figure, given.gravity.referenceGravityMS2, 6]);
    } finally {
      tables.dispose();
    }
  });

  // The ruling's last step (`decision-r08-t11-frame.md` §5): fewer bands cannot take a slices'
  // excess, so a row over by its slices is left over rather than its picture coarsened.
  it("leaves the cap over 2 MB after both widenings where its slices alone pass it, with four absorbers", () => {
    const cap = theCap();
    const four = fourAbsorberMedium();
    const radiusM = tableRadiusM(cap.figure);
    const sliced = rowBodyOf(four.medium, four.curves, bodyAt(cap, AS_BUILT), radiusM);
    const plan = tablesPlanOf(sliced.body, radiusM);
    const row = perPlanetRowOf(four.medium, four.curves, plan);
    expect([sliced.widening, sliced.over, plan.kappa.length, plan.bands.length]).toEqual([
      "kappaAndBands",
      true,
      6,
      5,
    ]);
    expect(row.transmittanceBytes).toBe(6 * 393_224);
    expect(row.transmittanceBytes).toBeGreaterThan(PER_PLANET_ROW_BYTES);
  });

  it("measures the rows against the 2 MB row", () => {
    expect(PER_PLANET_ROW_BYTES).toBe(2_000_000);
  });

  it("respaces no slicing at the built steps", () => {
    for (const giant of GIANTS) {
      const same = respacedSlicing(giant.gravity.slicing, AS_BUILT.kappa, AS_BUILT.band);
      expect([same.kappa.length, same.bandGravityRatio.length]).toEqual([
        giant.gravity.slicing.kappa.length,
        giant.gravity.slicing.bandGravityRatio.length,
      ]);
    }
  });
});
