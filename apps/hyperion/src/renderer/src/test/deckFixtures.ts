/**
 * The cloud-deck split's fixtures (plan R08, R08.T15.a.1's and T15.b.1's tests and benchmark):
 * synthetic decks on the Saturn-class air, each a drawn medium with its spectral column.
 *
 * @remarks
 * A test fixture, not used at run time. The decks are hand values, not any body's: a channel source
 * of Cornette–Shanks phase (`spectralColumn.ts`'s `channels`), grey across the spectrum, so that
 * every bake bin solves the same deck. R08.T12.b.2's `giant-deck` and Venus media are its own.
 */

import { mediumTableRadiusM } from "../view/atmosphere/assemble";
import {
  type AtmosphereMedium,
  type MediumTerm,
  tabulatedDensity,
} from "../view/atmosphere/medium";
import { SATURN_OBLATE_REFERENCE_SOURCES } from "../view/atmosphere/reference/cases";
import type { ThickBakeBody } from "../view/atmosphere/thick/bake";
import { envelopeFloorM } from "../view/atmosphere/thick/regime";
import {
  channelSource,
  type SpectralColumn,
  spectralColumnOf,
} from "../view/atmosphere/thick/spectralColumn";
import {
  gravityOfSpheroid,
  SATURN_SPHEROID,
  saturnClassMedium,
  saturnScaleHeightM,
} from "./atmosphereFixtures";

/** A deck fixture: its drawn medium, its spectral column, and where its split lies. */
export interface DeckFixture {
  readonly medium: AtmosphereMedium;
  readonly column: SpectralColumn;
  /** The split's altitude, h\* in m: the deck's top. */
  readonly deckAltitudeM: number;
}

/** Saturn's figure and normal gravity as a bake's body, R_ref its ground: four latitude bands. */
export function saturnBakeBody(): ThickBakeBody {
  const figure = {
    equatorialRadiusM: SATURN_SPHEROID.equatorialRadiusM,
    polarRadiusM: SATURN_SPHEROID.polarRadiusM,
  };
  return {
    figure,
    gravity: gravityOfSpheroid(SATURN_SPHEROID),
    bottomRadiusM: mediumTableRadiusM(figure),
  };
}

/** A grey cloud term of Cornette–Shanks phase, its scattering and absorption per m at density 1. */
function cloudTerm(
  density: MediumTerm["density"],
  scatteringPerM: number,
  absorptionPerM: number,
  asymmetry: number,
): MediumTerm {
  return {
    name: "aerosol:deck",
    density,
    scattering: [scatteringPerM, scatteringPerM, scatteringPerM],
    absorption: [absorptionPerM, absorptionPerM, absorptionPerM],
    phase: { kind: "cornette-shanks", asymmetry },
  };
}

/** The Saturn-class air with a deck term beside it, and its column. */
function withDeck(deck: MediumTerm, deckAltitudeM: number, name: string): DeckFixture {
  const air = saturnClassMedium();
  const medium: AtmosphereMedium = { ...air, name, terms: [...air.terms, deck] };
  return {
    medium,
    column: spectralColumnOf(medium, [...SATURN_OBLATE_REFERENCE_SOURCES, channelSource(deck)]),
    deckAltitudeM,
  };
}

/** The pressure the NH₃-like deck's top lies at: 0.5 bar, a hand value above Saturn's 1-bar datum. */
export const NH3_DECK_TOP_BAR = 0.5;

/**
 * An NH₃-like deck under the Saturn-class air, a giant's (R08.T15.a.1's band test): a grey cloud of
 * Cornette–Shanks g 0.75 and ω 0.99, uniform from the 1-bar datum to its top at
 * {@link NH3_DECK_TOP_BAR} (h\* = H ln 2 on the isothermal air), and τ 20 in the green channel, a
 * hand value above the split's 10 (Saturn's ammonia cloud has its top near 0.5–1 bar; the figures
 * are not fitted to it).
 */
export function nh3DeckFixture(): DeckFixture {
  const topM = saturnScaleHeightM() * Math.log(1 / NH3_DECK_TOP_BAR);
  const tau = 20;
  // A box of relative density 1, falling to 0 over the last metre below its top: its column is
  // topM − 0.5 m, so the coefficient that gives τ 20 is 20 ÷ that.
  const density = tabulatedDensity(
    Float64Array.of(0, topM - 1, topM, 2 * topM),
    Float64Array.of(1, 1, 0, 0),
  );
  const extinction = tau / (topM - 0.5);
  return withDeck(
    cloudTerm(density, 0.99 * extinction, 0.01 * extinction, 0.75),
    topM,
    "saturn-class nh3 deck",
  );
}

/** The conservative envelope's vertical optical depth at its datum, in the green channel. */
export const CONSERVATIVE_ENVELOPE_TAU = 3_000;

/**
 * A conservative envelope (R08.T15.a.1's 1% test): the Saturn-class air with a grey cloud of
 * Cornette–Shanks g 0.8 and ω 1, exponential at the air's scale height and τ
 * {@link CONSERVATIVE_ENVELOPE_TAU} to the datum, so that its deck's floor at τ 1,000 lies inside
 * the column and the deck beneath its split (where τ reaches 10) is nearly 990 deep.
 */
export function conservativeEnvelopeFixture(): DeckFixture {
  const air = saturnClassMedium();
  const scaleHeightM = saturnScaleHeightM();
  // τ from the top to h is τ₀ (e^(−h/H) − e^(−top/H)): the coefficient that gives τ₀ at the datum.
  const extinction =
    CONSERVATIVE_ENVELOPE_TAU / (scaleHeightM * (1 - Math.exp(-air.topHeightM / scaleHeightM)));
  const deck = cloudTerm({ kind: "exponential", scaleHeightM }, extinction, 0, 0.8);
  const fixture = withDeck(deck, 0, "conservative envelope");
  // The split is the envelope's floor (`regime.ts`'s `envelopeFloorM`), where τ reaches 10.
  return { ...fixture, deckAltitudeM: envelopeFloorM(fixture.medium) };
}
