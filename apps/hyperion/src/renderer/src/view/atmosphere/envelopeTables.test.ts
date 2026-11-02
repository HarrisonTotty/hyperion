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
import { type AbsorberCurveSlots, kernelMedium } from "./tables";
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
  it("draw each of the hand Jupiter's decks above its floor at its own column, though its levels alias their sheets", () => {
    const assembled = assembleMedium(recordInput(detailOf(handJupiterDetail())), {
      envelopes: true,
    });
    if (assembled.medium === undefined || assembled.regime.kind !== "cloudDeck") {
      throw new Error("the hand Jupiter draws a medium split at its floor");
    }
    const upper = upperMediumOf(assembled.medium, assembled.regime.deckAltitudeM);
    const top = upper.topHeightM;
    const drawn = kernelMedium(upper);
    // By its levels' densities the kernels kept 1.7% of its H₂O and NH₄SH decks' columns and 51% of
    // its NH₃ deck's (2026-11-02).
    const kept = upper.terms.flatMap((term, i) => {
      const resampled = drawn.terms[i];
      return term.name.startsWith("aerosol:") && resampled !== undefined
        ? [columnLengthM(resampled.density, top) / columnLengthM(term.density, top)]
        : [];
    });
    expect(kept).toHaveLength(4);
    for (const share of kept) {
      expect(share).toBeCloseTo(1, 9);
    }
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
    expect([
      largestMultiScattering(assembled.medium, radiusM, curves) > HALF_FLOAT_MAX,
      largestMultiScattering(upperMediumOf(assembled.medium, floorM), radiusM + floorM, curves),
    ]).toEqual([true, expect.toSatisfy((most: number) => most > 0 && most < 1)]);
  }, 180_000);
});
