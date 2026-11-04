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
  it("draw each of the hand Jupiter's decks above its floor at its own column, though its levels alias their sheets, and its chromophore within the resampling's tolerance", () => {
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
    // its NH₃ deck's (2026-11-02), so each is projected, its column kept. Its chromophore, the
    // fixture's, is a smooth layer, which from generator version 27 its levels keep to 0.64%
    // (measured 2026-11-04; 97.4% at 26, projected): within the tolerance, it keeps its levels'
    // densities. Its τ(550), 0.117 at the disc's column (P14.T24.h.b part 6) where 26's was 0.307,
    // lowers the floor 3.7 km, which moves the levels about the layer.
    const kept = new Map(
      upper.terms.flatMap((term, i) => {
        const resampled = drawn.terms[i];
        return term.name.startsWith("aerosol:") && resampled !== undefined
          ? [[term.name, columnLengthM(resampled.density, top) / columnLengthM(term.density, top)]]
          : [];
      }),
    );
    expect([...kept.keys()]).toEqual([
      "aerosol:H2O",
      "aerosol:NH4SH",
      "aerosol:NH3",
      "aerosol:chromophore",
    ]);
    for (const deck of ["aerosol:H2O", "aerosol:NH4SH", "aerosol:NH3"]) {
      expect(kept.get(deck)).toBeCloseTo(1, 9);
    }
    const chromophore = kept.get("aerosol:chromophore") ?? Number.NaN;
    expect([
      Math.abs(chromophore - 1) <= RESAMPLED_COLUMN_TOLERANCE,
      Math.round(chromophore * 1e4) / 1e4,
    ]).toEqual([true, 0.9936]);
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
    // Measured 2026-11-04: at most 1.216 above the floor, its chromophore at generator version 27's
    // disc column (τ(550) 0.117, P14.T24.h.b part 6) absorbing less than 26's (0.307), under which
    // it stayed under 1.
    expect([
      largestMultiScattering(assembled.medium, radiusM, curves) > HALF_FLOAT_MAX,
      above > 0 && above < HALF_FLOAT_MAX,
      Math.round(above * 1000) / 1000,
    ]).toEqual([true, true, 1.216]);
  }, 180_000);
});
