/**
 * A deck's channels at their own wavelengths (plan R08, R08.T15.b.2's route; `deck.ts`'s remarks,
 * "Own wavelengths"): over a surface whose lights allow all three, the channels' own points solved
 * beside the bins, the share with bin 0 holding them, and each channel's light that point's alone;
 * none over an envelope.
 */
import { describe, expect, it } from "vitest";

import { planckBakeSpectrum, SUN_TEFF_K } from "../../../test/atmosphereFixtures";
import { nh3DeckFixture, saturnBakeBody } from "../../../test/deckFixtures";
import { BAKE_BIN_COUNT } from "../medium";
import { THICK_BAKE_GRID, type ThickBakeGrid, type ThickBakeLight } from "./bake";
import {
  bakeDeck,
  bakeDeckBins,
  type DeckSplit,
  deckOwnChannels,
  gatherDeckBins,
  prepareDeck,
  type PreparedDeck,
} from "./deck";

/** A small grid, cheap to bake. */
const GRID: ThickBakeGrid = {
  layers: 16,
  altitudeStride: 2,
  sunCosines: Float64Array.of(0.3, 0.7, 1),
  upwardCosines: THICK_BAKE_GRID.upwardCosines,
};

/** The deck's layers on {@link GRID}. */
const DECK_LAYERS = 8;

/**
 * The Sun with its gases' channel curves, of which the fixture's column, with no gas, has none: each
 * channel may take its own wavelength.
 */
const OWN_SUN: ThickBakeLight = {
  spectrum: planckBakeSpectrum(SUN_TEFF_K),
  absorbers: [],
  channels: [],
};

/** The Sun without channel curves, under which no channel may. */
const BINS_SUN: ThickBakeLight = { spectrum: planckBakeSpectrum(SUN_TEFF_K), absorbers: [] };

const body = saturnBakeBody();

/** The NH₃ deck fixture's split over `floor`. */
function splitOf(floor: DeckSplit["floor"]): DeckSplit {
  return { altitudeM: nh3DeckFixture().deckAltitudeM, floor };
}

/** The NH₃ deck fixture's column prepared over `floor` under `light`, `bins` its share where given. */
function preparedOf(
  floor: DeckSplit["floor"],
  light: ThickBakeLight,
  bins?: ReadonlyArray<number>,
): PreparedDeck {
  return prepareDeck(nh3DeckFixture().column, [light], splitOf(floor), {
    grid: GRID,
    deckLayers: DECK_LAYERS,
    ...(bins === undefined ? {} : { bins }),
  });
}

describe("a deck's channels at their own wavelengths (R08.T15.b.2)", () => {
  it("takes all three over a surface whose lights allow them, and none over an envelope or under lights without channel curves", () => {
    expect([
      deckOwnChannels(splitOf("ground"), [OWN_SUN]),
      deckOwnChannels(splitOf("envelope"), [OWN_SUN]),
      deckOwnChannels(splitOf("ground"), [BINS_SUN]),
    ]).toEqual([
      [true, true, true],
      [false, false, false],
      [false, false, false],
    ]);
  });

  it("solves the channels' points beside the bins over a surface, each channel of the table below its own point's light alone", () => {
    const deck = preparedOf("ground", OWN_SUN);
    const share = bakeDeckBins(deck, body, 0);
    const below = bakeDeck(deck, body, 0).below?.slots[0]?.radiance;
    const own = share.bins.slice(BAKE_BIN_COUNT).map((part) => part.below?.[0]?.radiance);
    if (below === undefined || own.some((point) => point === undefined)) {
      throw new Error("a table below and its own points");
    }
    const misses = [0, 1, 2].map((c) => {
      let worst = 0;
      for (let i = 0; i < below.length / 3; i += 1) {
        const expected = Math.fround(own[c]?.[i] ?? Number.NaN);
        worst = Math.max(worst, Math.abs((below[i * 3 + c] ?? Number.NaN) - expected));
      }
      return worst;
    });
    expect([share.bins.map((part) => part.bin), misses]).toEqual([
      Array.from({ length: BAKE_BIN_COUNT + 3 }, (_, b) => b),
      [0, 0, 0],
    ]);
  });

  it("solves the bins alone over an envelope", () => {
    const share = bakeDeckBins(preparedOf("envelope", OWN_SUN), body, 0);
    expect(share.bins.map((part) => part.bin)).toEqual(
      Array.from({ length: BAKE_BIN_COUNT }, (_, b) => b),
    );
  });

  it("gathers its bin shares, the channels' points with bin 0's, to the whole band's bake to the bit", () => {
    const evens = Array.from({ length: 8 }, (_, k) => 2 * k);
    const odds = Array.from({ length: 7 }, (_, k) => 2 * k + 1);
    const shares = [evens, odds].map((bins) =>
      bakeDeckBins(preparedOf("ground", OWN_SUN, bins), body, 0),
    );
    expect([
      shares.map((share) => share.bins.map((part) => part.bin)),
      gatherDeckBins(shares),
    ]).toEqual([[[...evens, 15, 16, 17], odds], bakeDeck(preparedOf("ground", OWN_SUN), body, 0)]);
  });
});
