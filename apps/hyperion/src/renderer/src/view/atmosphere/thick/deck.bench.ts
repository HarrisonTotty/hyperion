/**
 * One cloud-deck split's bake (plan R08, R08.T15.a.1's timing, against `BAKE_CEILING_S`): a
 * benchmark, run by hand only, never by `vitest run`, on one thread:
 *
 *     timeout 3600 nice -n 10 pnpm --filter hyperion exec vitest bench --run \
 *       --project "logic (bench)" --reporter=verbose --maxWorkers=1 view/atmosphere/thick/deck
 *
 * The verbose reporter prints each phase's table. Each fixture's phases are timed as the optics
 * worker would run them for one light on the default grids: the preparation (the column's optics
 * in the 15 bins, the layers and the exponential sums over the whole column), one band's deck bake
 * (every bin and term, the deck's solve and the upper medium's, m = 0 and 1, and under a deck over a
 * surface the whole column's for the table below), and, on the giant, one band of the polarisation
 * correction over the deck. A phase's name carries its solves, its terms and its bytes.
 */
import { describe, expect, test } from "vitest";

import { planckBakeSpectrum, SUN_TEFF_K } from "../../../test/atmosphereFixtures";
import { lightOf, venusClassFixture } from "../../../test/bakeFixtures";
import { nh3DeckFixture, saturnBakeBody } from "../../../test/deckFixtures";
import type { DensityProfile } from "../medium";
import type { ThickBakeBody, ThickBakeLight } from "./bake";
import {
  bakeDeck,
  bakeDeckPolarisation,
  type DeckSplit,
  prepareDeck,
  prepareDeckPolarisation,
} from "./deck";
import type { SpectralColumn } from "./spectralColumn";

/** Each phase's runs: a few, since one takes seconds. */
const RUNS = { iterations: 3, time: 0, warmup: false } as const;

/** Long enough for the slowest fixture's phases on a loaded machine, ms. */
const TIMEOUT_MS = 2_400_000;

/** A tabulated profile's top: the first level above its last of positive density. */
function profileTopM(density: DensityProfile): number {
  let top: number;
  switch (density.kind) {
    case "tabulated": {
      let last = 0;
      density.relative.forEach((value, i) => {
        if (value > 0) {
          last = i;
        }
      });
      top = density.altitudesM[Math.min(last + 1, density.altitudesM.length - 1)] ?? Number.NaN;
      break;
    }
    case "tent":
      top = density.topM;
      break;
    case "exponential":
      top = Number.NaN;
      break;
  }
  return top;
}

/** A fixture as the bench takes it. */
interface DeckCase {
  readonly column: SpectralColumn;
  readonly light: ThickBakeLight;
  readonly body: ThickBakeBody;
  readonly split: DeckSplit;
  /** Whether the polarisation correction over the deck is timed too. */
  readonly polarisation: boolean;
}

/** The fixtures, by name. */
const FIXTURES: ReadonlyArray<readonly [string, () => Promise<DeckCase>]> = [
  [
    "the Venus-class column, split at its sulphuric acid cloud's top, the deck down to the ground",
    async () => {
      const fixture = await venusClassFixture();
      const cloud = fixture.column.terms.find((term) => term.name === "aerosol:H2SO4");
      if (cloud === undefined) {
        throw new Error("the Venus-class column has its cloud");
      }
      return {
        column: fixture.column,
        light: lightOf(fixture),
        body: fixture.body,
        split: { altitudeM: profileTopM(cloud.density), floor: "ground" },
        polarisation: false,
      };
    },
  ],
  [
    "the Saturn-class air over its NH₃-like deck, an envelope, one of four bands",
    async () => {
      const fixture = nh3DeckFixture();
      return Promise.resolve({
        column: fixture.column,
        light: { spectrum: planckBakeSpectrum(SUN_TEFF_K), absorbers: [] },
        body: saturnBakeBody(),
        split: { altitudeM: fixture.deckAltitudeM, floor: "envelope" },
        polarisation: true,
      });
    },
  ],
];

describe("one deck bake, one band and one light", () => {
  test.for(FIXTURES)("%s", { timeout: TIMEOUT_MS }, async ([, make], { bench }) => {
    const fixture = await make();
    const prepared = prepareDeck(fixture.column, [fixture.light], fixture.split);
    const tables = bakeDeck(prepared, fixture.body, 0);
    const slot = tables.above.slots[0];
    expect(slot).toBeDefined();
    const terms = slot?.bins.map((bin) => bin.terms).join(" ") ?? "";
    const solves = slot?.solves ?? 0;
    const below = tables.below?.slots[0];
    const bytes =
      (slot?.values.byteLength ?? 0) +
      (tables.slots[0]?.reflectance.byteLength ?? 0) +
      (tables.slots[0]?.diffuse.byteLength ?? 0) +
      (tables.slots[0]?.beamReflection.byteLength ?? 0) +
      (below?.radiance.byteLength ?? 0) +
      (below?.irradiance.byteLength ?? 0) +
      (below?.direct.byteLength ?? 0);

    await bench("the preparation: the optics, the layers and the exponential sums", () => {
      prepareDeck(fixture.column, [fixture.light], fixture.split);
    }).run(RUNS);
    await bench(
      `one band's deck bake${tables.below === null ? "" : " with the table below"}: ${solves} solves of each, the bins' terms ${terms}, ${bytes} B as f32`,
      () => {
        bakeDeck(prepared, fixture.body, 0);
      },
    ).run(RUNS);
    if (fixture.polarisation) {
      const polarisation = prepareDeckPolarisation(prepared, [fixture.light]);
      await bench("one band of the polarisation correction over the deck", () => {
        bakeDeckPolarisation(prepared, polarisation, fixture.body, 0);
      }).run(RUNS);
    }
  });
});
