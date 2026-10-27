/**
 * R08.T11's per-planet rows of the giants (plan R08, Design note 11; R08.T11's fourth item): the κ
 * slices, the latitude bands and the bytes {@link AtmosphereTables} allocates for the Jupiter- and
 * Saturn-class figures, an ice giant and a giant at plan 14's cap, as built and after Design note
 * 11's two widenings in its order, `KAPPA_STEP` to 0.3 and then `BAND_STEP` to 0.15, set against
 * the 2 MB row ({@link PER_PLANET_ROW_BYTES}).
 *
 * @remarks
 * A record of the figures as they stand, on two airs: a thin H₂–He one with no absorber
 * (`saturnClassMedium`, Design note 11's thin worlds) and the ice giant's methane-absorbing one
 * (`iceGiantMethaneBody`'s medium and curves, one light slot), each on every figure, so that the
 * row's dependence on the figure is seen apart from the air's. The widened slicings span the
 * built ones' extremes by `oblate.ts`'s own rule, 1 + ⌈ln(max ÷ min) ÷ step⌉ evenly in ln, one
 * below `ONE_SLICE_BELOW` and `ONE_BAND_BELOW`; nothing in the client widens today (Design note
 * 11's rule is unbuilt), so the widened rows are what that rule would give, not what is drawn.
 */

import { describe, expect, it } from "vitest";

import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../../lib/system/constants";
import {
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
  ONE_BAND_BELOW,
  ONE_SLICE_BELOW,
  type OblateSlicing,
} from "./oblate";
import {
  type AbsorberCurveSlots,
  AtmosphereTables,
  NO_ABSORBER_CURVES,
  PER_PLANET_ROW_BYTES,
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

const STEPS: ReadonlyArray<Steps> = [
  AS_BUILT,
  { name: "κ 0.3", kappa: 0.3, band: BAND_STEP },
  { name: "κ 0.3, s 0.15", kappa: 0.3, band: 0.15 },
];

/** `count` values from `low` to `high`, evenly in ln, as `oblate.ts` spaces its slices. */
function logSpaced(low: number, high: number, count: number): Float64Array {
  return Float64Array.from({ length: count }, (_, i) =>
    count === 1 ? low : low * (high / low) ** (i / (count - 1)),
  );
}

/** One axis of a slicing respaced at `step` over its own extremes, by `oblate.ts`'s rule. */
function respaced(values: Float64Array, step: number, oneBelow: number): Float64Array {
  const low = values[0];
  const high = values.at(-1);
  if (low === undefined || high === undefined) {
    throw new Error("a slicing holds at least one value an axis");
  }
  const span = Math.log(high / low);
  return span <= oneBelow ? Float64Array.of(1) : logSpaced(low, high, 1 + Math.ceil(span / step));
}

/** A slicing at Design note 11's `steps`. */
function widened(slicing: OblateSlicing, steps: Steps): OblateSlicing {
  return {
    kappa: respaced(slicing.kappa, steps.kappa, ONE_SLICE_BELOW),
    bandGravityRatio: respaced(slicing.bandGravityRatio, steps.band, ONE_BAND_BELOW),
  };
}

/** A giant's row on an air at a slicing's steps: slices, bands and the tables' bytes. */
async function rowOf(giant: Giant, air: Air, steps: Steps): Promise<readonly number[]> {
  const gravity = { ...giant.gravity, slicing: widened(giant.gravity.slicing, steps) };
  const body = { figure: giant.figure, gravity };
  const radiusM = tableRadiusM(giant.figure);
  const plan = tablesPlanOf(body, radiusM);
  const engine = await countingRenderEngine();
  const tables = new AtmosphereTables(engine, air.medium, radiusM, air.curves, body);
  try {
    return [plan.kappa.length, plan.bands.length, tables.rowBytes];
  } finally {
    tables.dispose();
  }
}

/** Every giant's rows on `air`, by name and step: slices, bands and bytes. */
async function rowsOn(air: Air): Promise<Record<string, Record<string, readonly number[]>>> {
  const entries = await Promise.all(
    GIANTS.map(async (giant) => {
      const rows = await Promise.all(STEPS.map(async (steps) => rowOf(giant, air, steps)));
      return [giant.name, Object.fromEntries(STEPS.map((steps, i) => [steps.name, rows[i]]))];
    }),
  );
  return Object.fromEntries(entries);
}

describe("R08.T11's per-planet rows of the giants (Design note 11)", () => {
  // Recorded 2026-10-27 (plan 08, "Deviations in T11, as built"): [slices, bands, bytes]. Design
  // note 11 put a thin Jupiter-class world at 0.57 MB, a thin Saturn-class one at 0.72 MB, a thin ice
  // giant at 0.29 MB and the cap at 1.3–1.4 MB thin; as built every slice's depth, absorber-column
  // and pressure-column tables are allocated at full size whatever the air reads (393,224 B a slice,
  // R08.T8.e's record), so a thin Saturn-class world is over the 2 MB row, and the cap is over it
  // after both widenings.
  it("records the thin air's slices, bands and bytes on each giant, as built and widened", async () => {
    expect(await rowsOn(THIN_AIR)).toEqual({
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

  it("records the methane air's slices, bands and bytes on each giant, as built and widened", async () => {
    expect(await rowsOn(METHANE_AIR)).toEqual({
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

  it("measures the rows against the 2 MB row", () => {
    expect(PER_PLANET_ROW_BYTES).toBe(2_000_000);
  });

  it("respaces no slicing at the built steps", () => {
    for (const giant of GIANTS) {
      const same = widened(giant.gravity.slicing, AS_BUILT);
      expect([same.kappa.length, same.bandGravityRatio.length]).toEqual([
        giant.gravity.slicing.kappa.length,
        giant.gravity.slicing.bandGravityRatio.length,
      ]);
    }
  });
});
