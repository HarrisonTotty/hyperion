/**
 * The cloud deck's bake as the optics worker answers it (plan R08, R08.T15.a.2): a `deck-bake`
 * request through `opticsJobs.ts`'s `OpticsJobs`, in process, for a kept NH₃-like deck on the
 * Saturn-class figure, its lights carried by the request.
 */
import { describe, expect, it } from "vitest";

import { planckBakeSpectrum, SUN_TEFF_K } from "../../../test/atmosphereFixtures";
import { nh3DeckFixture, saturnBakeBody } from "../../../test/deckFixtures";
import { toyLoader } from "../../../test/opticsFakes";
import { OpticsJobs } from "../opticsJobs";
import type { KeptBakeLights, OpticsRequest } from "../opticsMessages";
import { bakeDeck, bakeDeckPolarisation, prepareDeck, prepareDeckPolarisation } from "./deck";

/** A kept body's input: no medium of its own, its lights in the request. */
const KEPT_INPUT = { kind: "kept", atmosphere: { kind: "unset" } } as const;

/** The Sun alone, no absorber: the fixture has no absorber term. */
const LIGHTS: KeptBakeLights = { absorbers: [], spectra: [planckBakeSpectrum(SUN_TEFF_K)] };

/** The NH₃-like deck's bake of its first band, an envelope, as a kept body asks it. */
function deckRequest(altitudeM: number): OpticsRequest {
  const fixture = nh3DeckFixture();
  return {
    kind: "deck-bake",
    id: 11,
    input: KEPT_INPUT,
    slots: [0],
    bands: [0],
    column: fixture.column,
    body: saturnBakeBody(),
    split: { altitudeM, floor: "envelope" },
    kept: LIGHTS,
  };
}

describe("the optics worker's deck (R08.T15.a.2)", () => {
  it(
    "answers a kept deck's bake with its band's tables and the correction over it, as a direct bake makes them",
    // The full grid: some 4 s of deck and 2 s of correction on a pod's thread (2026-10-30).
    { timeout: 120_000 },
    async () => {
      const fixture = nh3DeckFixture();
      const jobs = new OpticsJobs(toyLoader());
      const { reply } = await jobs.answer(deckRequest(fixture.deckAltitudeM));
      if (reply.kind !== "deck-baked") {
        throw new Error(`the deck was answered ${reply.kind}`);
      }
      const lights = [{ spectrum: planckBakeSpectrum(SUN_TEFF_K), absorbers: [] }];
      const split = { altitudeM: fixture.deckAltitudeM, floor: "envelope" } as const;
      const prepared = prepareDeck(fixture.column, lights, split);
      const direct = bakeDeck(prepared, saturnBakeBody(), 0);
      const correction = bakeDeckPolarisation(
        prepared,
        prepareDeckPolarisation(prepared, lights),
        saturnBakeBody(),
        0,
      );
      expect([reply.tables.length, reply.correction?.length]).toEqual([1, 1]);
      // The worker's answer is a structured copy of the same arithmetic: equal to the bit.
      expect(reply.tables[0]?.slots).toEqual(direct.slots);
      expect(reply.tables[0]?.above.slots[0]?.values).toEqual(direct.above.slots[0]?.values);
      expect(reply.correction?.[0]?.slots[0]?.values).toEqual(correction.slots[0]?.values);
      expect(Array.from(reply.correction?.[0]?.altitudesM ?? [])).toEqual(
        Array.from(direct.above.altitudesM),
      );
    },
  );

  it("answers a deck that fails with its own failure, never failed", async () => {
    const jobs = new OpticsJobs(toyLoader());
    expect((await jobs.answer(deckRequest(-1))).reply).toEqual({
      kind: "deck-bake-failed",
      id: 11,
      bands: [0],
      message: expect.stringMatching(/a split lies above its deck's floor/u),
    });
  });
});
