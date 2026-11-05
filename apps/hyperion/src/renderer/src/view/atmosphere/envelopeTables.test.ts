/**
 * A gas envelope's tables as the analytic term draws them while its route is closed (plan R08,
 * R08.T16.a.2): its medium above its floor, whose multiple scattering stays within the tables'
 * `rgba16float`, where Hillaire's series over its whole column diverges.
 */
import { describe, expect, it } from "vitest";

import { detailOf, recordInput } from "../../test/atmosphereRecords";
import { handJupiterDetail } from "../../test/giantFixtures";
import { absorberCurveSlots, assembleMedium } from "./assemble";
import { type AtmosphereMedium, columnLengthM } from "./medium";
import { type AbsorberCurveSlots, kernelMedium, RESAMPLED_COLUMN_TOLERANCE } from "./tables";
import { planetTwin } from "./tablesCpu";
import { upperMediumOf } from "./thick/deckRead";

/** The largest finite value `rgba16float` holds: (2 − 2⁻¹⁰) × 2¹⁵ (IEEE 754-2019, binary16). */
const HALF_FLOAT_MAX = 65_504;

/** The largest value of a medium's multiple-scattering table, light slot 0, on a sphere of `radiusM`. */
function largestMultiScattering(
  medium: AtmosphereMedium,
  radiusM: number,
  curves: AbsorberCurveSlots,
): number {
  const planet = planetTwin(medium, radiusM, curves);
  const table = planet.multiScatteringBySlot?.[0] ?? planet.multiScattering;
  return table.texels.reduce((most, value) => Math.max(most, value), 0);
}

describe("a gas envelope's tables (R08.T16.a.2)", () => {
  it("draw each of the hand Jupiter's decks above its floor at its own column, though its NH₃ deck's levels alias its sheet, and its haze and chromophore within the resampling's tolerance", () => {
    const assembled = assembleMedium(recordInput(detailOf(handJupiterDetail())), {
      envelopes: true,
    });
    if (assembled.medium === undefined || assembled.regime.kind !== "cloudDeck") {
      throw new Error("the hand Jupiter draws a medium split at its floor");
    }
    const upper = upperMediumOf(assembled.medium, assembled.regime.deckAltitudeM);
    const top = upper.topHeightM;
    const drawn = kernelMedium(upper);
    // By its levels' densities the kernels kept 51% of its NH₃ deck's column (2026-11-02), so it is
    // projected, its column kept. Under 30's NH₃ haze (`giantFixtures.ts`'s `coldGiantHazes`) the
    // floor rises to 2.12 bar: its water deck lies below it, and its NH₄SH deck, cut there, keeps
    // 100.28% by its levels (measured 2026-11-05), the haze 100.03% and the chromophore 100.00%,
    // each within the tolerance and kept on its levels' densities.
    const kept = new Map(
      upper.terms.flatMap((term, i) => {
        const resampled = drawn.terms[i];
        return term.name.startsWith("aerosol:") && resampled !== undefined
          ? [[term.name, columnLengthM(resampled.density, top) / columnLengthM(term.density, top)]]
          : [];
      }),
    );
    expect([...kept.keys()]).toEqual([
      "aerosol:NH4SH",
      "aerosol:NH3",
      "aerosol:NH3:2",
      "aerosol:chromophore",
    ]);
    expect(kept.get("aerosol:NH3")).toBeCloseTo(1, 9);
    expect(
      [...kept.values()].map((share) => [
        Math.abs(share - 1) <= RESAMPLED_COLUMN_TOLERANCE,
        Math.round(share * 1e4) / 1e4,
      ]),
    ).toEqual([
      [true, 1.0028],
      [true, 1],
      [true, 1.0003],
      [true, 1],
    ]);
  }, 180_000);

  it("hold the hand Jupiter's multiple scattering within rgba16float above its floor, where its whole column's diverges", () => {
    const assembled = assembleMedium(recordInput(detailOf(handJupiterDetail())), {
      envelopes: true,
    });
    if (assembled.medium === undefined || assembled.regime.kind !== "cloudDeck") {
      throw new Error("the hand Jupiter draws a medium split at its floor");
    }
    const curves = absorberCurveSlots(assembled.drawn);
    const radiusM = assembled.drawn.bottomRadiusM;
    const floorM = assembled.regime.deckAltitudeM;
    const above = largestMultiScattering(
      upperMediumOf(assembled.medium, floorM),
      radiusM + floorM,
      curves,
    );
    // Measured 2026-11-05: at most 0.265 above the floor, which 30's NH₃ haze lifts to 2.12 bar
    // (1.216 at 27's chromophore with no haze, under 1 at 26's).
    expect([
      largestMultiScattering(assembled.medium, radiusM, curves) > HALF_FLOAT_MAX,
      above > 0 && above < HALF_FLOAT_MAX,
      Math.round(above * 1000) / 1000,
    ]).toEqual([true, true, 0.265]);
  }, 180_000);
});
