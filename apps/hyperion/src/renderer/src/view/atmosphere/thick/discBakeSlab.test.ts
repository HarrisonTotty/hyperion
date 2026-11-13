/**
 * The disc's table under a thin opaque slab high in a tall envelope (R08's
 * fix-r08-disc-table-under-slab): the owner's warm sub-Neptune's soot and haze at 0.1–0.27 Pa,
 * as a hand slab, alone and over a layer, its envelope cut at its floor as the router cuts it
 * (`regime.ts`'s `envelopeFloorM`), the table against the routed full passes' twins from 10⁹ m.
 *
 * @remarks
 * Its layers are tabulated as the assembly lays an aerosol's (a box of density 1 with 100 m ramps),
 * so that the march cuts its rays at their edges as it cuts a drawn mode's (`medium.ts`'s
 * `layerEdgesM`). A layer under the slab lowers nothing the slab lets through, but it raises the
 * floor and so moves every one of the table's quadratic steps: before the fix the table's disc read
 * 0.45 stops darker than the march in red at 0° on the slab alone and 1.07 at 90° over the deep
 * layer, and 0.08 over the layer beneath it.
 */
import { beforeAll, describe, expect, it } from "vitest";

import { gravityOfSpheroid } from "../../../test/atmosphereFixtures";
import {
  bakedDiscAir,
  type BakedDiscAir,
  type DiscAirCase,
  fullPassTwins,
  type FullPassTwins,
  integratedRadianceAt,
} from "../../../test/discAirCases";
import { type AtmosphereMedium, type MediumTerm, tabulatedDensity } from "../medium";
import type { LevelSpheroid } from "../oblate";
import { kernelMedium } from "../tables";
import { thickBakeBands } from "./bake";
import { deckDatumOf, upperMediumOf } from "./deckRead";
import { discAirMarcher } from "./discBake";
import { envelopeFloorM } from "./regime";
import { channelSource } from "./spectralColumn";

/** A sphere of the owner's sub-Neptune's 2.86 R⊕ and 11.9 m s⁻², not spinning. */
const SUB_NEPTUNE: LevelSpheroid = {
  equatorialRadiusM: 18_240e3,
  polarRadiusM: 18_240e3,
  gmM3S2: 11.9 * 18_240e3 ** 2,
  angularVelocityRadS: 0,
};

/** The column's top, m above its base, and its gas's scale height, m: about the draw's. */
const TOP_M = 4_700e3;
const SCALE_HEIGHT_M = 130e3;

/** The slab: a scale height at the column's top, as the draw's soot and haze, of τ 5. */
const SLAB_M = [4_200e3, 4_330e3] as const;

/** A tabulated box of density 1 from `bottomM` to `topM`, ramped to 0 over 100 m past each end. */
function box(
  name: string,
  [bottomM, topM]: readonly [number, number],
  tau: number,
  albedo: readonly [number, number, number],
  asymmetry: number,
): MediumTerm {
  const rampM = 100;
  const sigma = tau / (topM - bottomM + rampM);
  return {
    name,
    density: tabulatedDensity(
      Float64Array.of(0, bottomM - rampM, bottomM, topM, topM + rampM, TOP_M),
      Float64Array.of(0, 0, 1, 1, 0, 0),
    ),
    scattering: [sigma * albedo[0], sigma * albedo[1], sigma * albedo[2]],
    absorption: [sigma * (1 - albedo[0]), sigma * (1 - albedo[1]), sigma * (1 - albedo[2])],
    phase: { kind: "cornette-shanks", asymmetry },
  };
}

/** The envelope's gas: a Rayleigh scatterer of τ 60 in green, blue the most. */
function gas(): MediumTerm {
  const sigma = 60 / (SCALE_HEIGHT_M * -Math.expm1(-TOP_M / SCALE_HEIGHT_M));
  return {
    name: "gas",
    density: { kind: "exponential", scaleHeightM: SCALE_HEIGHT_M },
    scattering: [sigma * 0.4, sigma, sigma * 2.2],
    absorption: [0, 0, 0],
    phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
  };
}

/** What lies under the slab. */
type Under = "nothing" | "a layer beneath it" | "a deep layer";

/** The slab over `under`, cut at its floor, on its floor's datum, as a disc air case. */
function slabCase(under: Under): DiscAirCase {
  const terms = [gas(), box("slab", SLAB_M, 5, [0.9, 0.95, 0.98], 0.5)];
  switch (under) {
    case "nothing":
      break;
    case "a layer beneath it":
      terms.push(box("layer", [1_000e3, SLAB_M[0]], 12, [0.99, 0.99, 0.99], 0.7));
      break;
    case "a deep layer":
      terms.push(box("layer", [1_000e3, 2_000e3], 12, [0.99, 0.99, 0.99], 0.7));
      break;
  }
  const medium: AtmosphereMedium = {
    name: `a slab over ${under}`,
    topHeightM: TOP_M,
    groundAlbedo: [0, 0, 0],
    terms,
  };
  const floorM = envelopeFloorM(medium);
  const gravity = gravityOfSpheroid(SUB_NEPTUNE);
  return {
    name: medium.name,
    medium: upperMediumOf(medium, floorM),
    sources: terms.map(channelSource),
    curves: [],
    body: { figure: deckDatumOf(SUB_NEPTUNE, gravity, floorM), gravity },
  };
}

const CASES: ReadonlyArray<Under> = ["nothing", "a layer beneath it", "a deep layer"];

/** A case, its table and its full passes' twins. */
interface BuiltSlab {
  readonly item: DiscAirCase;
  readonly baked: BakedDiscAir;
  readonly twins: FullPassTwins;
}

/** The limit of each case's bake and twins, ms: some ten times their 6 s on a pod. */
const LIMIT_MS = 120_000;

describe("the disc's table under a thin opaque slab (R08)", () => {
  const built = new Map<Under, BuiltSlab>();
  beforeAll(() => {
    for (const under of CASES) {
      const item = slabCase(under);
      const baked = bakedDiscAir(item);
      built.set(under, { item, baked, twins: fullPassTwins(item, baked) });
    }
  }, 3 * LIMIT_MS);
  const builtOf = (under: Under): BuiltSlab => {
    const found = built.get(under);
    if (found === undefined) {
      throw new Error(`the slab over ${under} is baked before the tests`);
    }
    return found;
  };

  it("puts the floor in the layer beneath the slab, and deeper with none, so that the steps move", () => {
    const columnsM = CASES.map((under) => builtOf(under).item.medium.topHeightM);
    // The upper medium's top above its floor: the floor rises with the layer under the slab.
    expect(columnsM[1]).toBeLessThan(columnsM[2] ?? Number.NaN);
    expect(columnsM[2]).toBeLessThan(columnsM[0] ?? Number.NaN);
  });

  for (const under of CASES) {
    it(`marches each node of the slab over ${under} within 5% of 3,072 steps in every channel`, () => {
      const { item, baked } = builtOf(under);
      const input = {
        medium: kernelMedium(item.medium),
        curves: item.curves,
        bands: thickBakeBands(baked.bakeBody),
        source: baked.source,
      };
      const march = discAirMarcher(input, 0);
      const converged = discAirMarcher(input, 0, { viewSteps: 3_072 });
      for (const [muView, muSun, azimuthRad] of [
        [1, 1, 0],
        [0.5, 0.5, Math.PI],
        [0.5, 0.8, 1],
        [0.2, 0.6, 0.5],
      ] as const) {
        const got = march(muView, muSun, azimuthRad, 0);
        const want = converged(muView, muSun, azimuthRad, 0);
        for (const c of [0, 1, 2] as const) {
          expect(Math.abs(got[c] / want[c] - 1)).toBeLessThan(0.05);
        }
      }
    });

    for (const phaseDeg of [0, 90]) {
      it(
        `draws the slab over ${under} within 5% of the routed march inside its disc from 10⁹ m at a phase of ${phaseDeg}°`,
        () => {
          const { item, baked, twins } = builtOf(under);
          const at = integratedRadianceAt(item, baked, twins, phaseDeg, {
            radii: 60,
            azimuths: 16,
          });
          for (const c of [0, 1, 2] as const) {
            // The air beyond the datum's limb is the full passes' alone: the table's disc has none.
            const inside = at.routed[c] - at.limb[c];
            expect(Math.abs(at.disc[c] / inside - 1)).toBeLessThan(0.05);
          }
        },
        LIMIT_MS,
      );
    }
  }
});
