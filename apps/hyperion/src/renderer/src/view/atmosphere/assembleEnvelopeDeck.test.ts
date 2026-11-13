/**
 * A gas envelope's medium through T15's deck machinery (plan R08, R08.T16.a.1), by hand under
 * `HYPERION_SLOW_TESTS=1`: the hand Jupiter's bands, the hot Jupiter's deck read to τ 1,000, and
 * the hot Jupiter's reflectance near normal incidence, printed beside Rowe et al. 2008's geometric
 * albedo and not asserted (research-r08-t16a's QUESTION 1).
 *
 * @remarks
 * Each prepares a deck over the drawn medium's spectral column (`thick/deck.ts`' `prepareDeck`),
 * its modes' optics in the 15 bake bins: tens of seconds a fixture on a pod.
 */
import { describe, expect, it } from "vitest";

import { detailOf, recordInput } from "../../test/atmosphereRecords";
import { handJupiterDetail } from "../../test/giantFixtures";
import { memoised } from "../../test/memo";
import { hotJupiterDetail } from "../../test/planetaryFixture";
import { assembleMedium, type DrawnMedium } from "./assemble";
import { type AtmosphereMedium, BAKE_WAVELENGTHS_NM, columnLengthM, extinction } from "./medium";
import { type ThickBakeBody, thickBakeBands, thickBakeLights } from "./thick/bake";
import {
  bakeDeck,
  DECK_FLOOR_OPTICAL_DEPTH,
  deckColumnSolve,
  type PreparedDeck,
  prepareDeck,
} from "./thick/deck";
import { ENVELOPE_FLOOR_CHANNEL } from "./thick/regime";
import { spectralColumnOf } from "./thick/spectralColumn";

/** Whether the deck's checks run: by hand, `HYPERION_SLOW_TESTS=1`. */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

const SLOW = slowTests();

/** The limit of each, ms: some ten times its run on a pod. */
const LIMIT_MS = 1_200_000;

const ANSWERS = { handJupiter: handJupiterDetail, hotJupiter: hotJupiterDetail } as const;

type Fixture = keyof typeof ANSWERS;

/** A fixture's drawn medium, its deck prepared over its floor, and its body as a bake's. */
const decked = memoised(
  (
    fixture: Fixture,
  ): {
    readonly medium: AtmosphereMedium;
    readonly drawn: DrawnMedium;
    readonly prepared: PreparedDeck;
    readonly body: ThickBakeBody;
  } => {
    const result = assembleMedium(recordInput(detailOf(ANSWERS[fixture]())), { envelopes: true });
    const { medium, drawn } = result;
    if (medium === undefined || drawn.envelope === undefined) {
      throw new Error(`the ${fixture} draws an envelope's medium`);
    }
    const column = spectralColumnOf(medium, drawn.sources);
    const lights = thickBakeLights(column, drawn.absorbers, drawn.spectra);
    const prepared = prepareDeck(column, lights, {
      altitudeM: drawn.envelope.floorAltitudeM,
      floor: "envelope",
    });
    return {
      medium,
      drawn,
      prepared,
      body: {
        figure: drawn.body.figure,
        gravity: drawn.body.gravity,
        bottomRadiusM: drawn.bottomRadiusM,
      },
    };
  },
  (fixture) => fixture,
);

/** The vertical τ from the top down to `heightM` in the floor's channel, of every term but the absorbers. */
function depthAboveM(medium: AtmosphereMedium, heightM: number): number {
  return medium.terms
    .filter((term) => term.absorber === undefined)
    .reduce(
      (sum, term) =>
        sum +
        extinction(term)[ENVELOPE_FLOOR_CHANNEL] *
          (columnLengthM(term.density, medium.topHeightM) - columnLengthM(term.density, heightM)),
      0,
    );
}

describe("an envelope's medium through T15's deck (by hand)", () => {
  it.skipIf(!SLOW)(
    "reads the hot Jupiter's deck to τ 1,000 from the top, inside its column, over a black floor",
    () => {
      const { medium, prepared } = decked("hotJupiter");
      expect(prepared.floorAltitudeM).toBeGreaterThan(0);
      // The spectral column's green channel is the drawn medium's to the bit (T15.a.1's record).
      expect(depthAboveM(medium, prepared.floorAltitudeM) / DECK_FLOOR_OPTICAL_DEPTH).toBeCloseTo(
        1,
        3,
      );
      expect([...prepared.floorAlbedo].every((a) => a === 0)).toBe(true);
    },
    LIMIT_MS,
  );

  it.skipIf(!SLOW)(
    "bakes the hand Jupiter's three bands at its floor, the columns in the bands' inverse gravities",
    () => {
      const { prepared, body } = decked("handJupiter");
      const tables = thickBakeBands(body).map((_, band) => bakeDeck(prepared, body, band));
      const [first, ...rest] = tables;
      if (first === undefined) {
        throw new Error("three bands");
      }
      expect(tables).toHaveLength(3);
      let worst = 0;
      for (const other of rest) {
        // The floor is given in h*: one height, so one pressure, in every band.
        expect(other.deckAltitudeM).toBe(first.deckAltitudeM);
        const ratio = other.band.gravityRatio / first.band.gravityRatio;
        for (let b = 0; b < BAKE_WAVELENGTHS_NM.length; b += 1) {
          for (const [one, two] of [
            [first.above.verticalOpticalDepth, other.above.verticalOpticalDepth],
            [first.deckOpticalDepth, other.deckOpticalDepth],
          ] as const) {
            worst = Math.max(worst, Math.abs((one[b] ?? NaN) / (two[b] ?? NaN) / ratio - 1));
          }
        }
      }
      expect(worst).toBeLessThan(1e-12);
    },
    LIMIT_MS,
  );

  it.skipIf(!SLOW)(
    "prints the hot Jupiter's reflectance near normal incidence beside Rowe et al. 2008's A_g",
    () => {
      const { prepared, body } = decked("hotJupiter");
      const [band] = thickBakeBands(body);
      if (band === undefined) {
        throw new Error("one band");
      }
      const light = prepared.upper.lights[0];
      const lines: string[] = [];
      let factorSum = 0;
      let albedoSum = 0;
      let bins = 0;
      for (const [bin, nm] of BAKE_WAVELENGTHS_NM.entries()) {
        if (nm < 400 || nm > 700) {
          continue;
        }
        let factor = 0;
        let albedo = 0;
        for (const term of light?.bins[bin]?.terms ?? []) {
          const { solution } = deckColumnSolve(prepared, band, bin, term);
          const beam = solution.beams.at(-1);
          const users = solution.userCosines.length;
          const mu0 = beam?.beamCosine ?? Number.NaN;
          // The highest sun and view on the grid, μ₀ 0.984 and μ 0.9375: m = 0 alone near normal.
          const up = beam?.modes[0]?.userUp[users - 1] ?? Number.NaN;
          factor += term.weight * ((Math.PI * up) / mu0);
          albedo += term.weight * ((beam?.fluxes.diffuseUp[0] ?? Number.NaN) / mu0);
        }
        lines.push(
          `${nm.toFixed(1)} nm: reflectance factor ${factor.toFixed(4)}, plane albedo ${albedo.toFixed(4)}`,
        );
        factorSum += factor;
        albedoSum += albedo;
        bins += 1;
      }
      lines.push(
        `400–700 nm mean: reflectance factor ${(factorSum / bins).toFixed(4)}, plane albedo ${(albedoSum / bins).toFixed(4)}; Rowe et al. 2008: A_g 0.038 ± 0.045 (3σ under 0.17)`,
      );
      console.warn(`the hot Jupiter fixture as drawn:\n${lines.join("\n")}`);
      expect(bins).toBeGreaterThan(0);
    },
    LIMIT_MS,
  );
});
