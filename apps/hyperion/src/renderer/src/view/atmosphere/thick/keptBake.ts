/**
 * A kept scene's thick bake, polarisation correction and deck made directly, with no optics worker
 * (plan R08, R08.T14.c, R08.T14.f and R08.T15.a.2; `decision-r08-t13c2-classes.md` §6): for the
 * tests, the recorded runs and the vector gates, from a kept medium and its sources, as
 * `opticsJobs.ts` answers a kept body's `thick-bake`, `polarisation-bake` and `deck-bake`.
 *
 * @remarks
 * A kept medium is handed to the atmosphere cache whole and never assembled, so its terms' sources
 * come beside it (`AtmosphereCache.ts`'s `BuiltAtmosphere.sources`, a reference case's `sources`),
 * and its absorbers' specs and its lights' spectra with its bake's lights
 * (`opticsMessages.ts`'s `KeptBakeLights`). Like `bake.ts`, it runs the optics' Mie, MMF and phase
 * files, so it belongs in the optics worker or a test, never on the render thread.
 */

import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../../../lib/system/constants";
import type { AtmosphereMedium } from "../medium";
import { bodyGravity } from "../oblate";
import type { KeptBakeLights } from "../opticsMessages";
import {
  bakeMultipleScattering,
  type MsSourceTable,
  prepareBake,
  THICK_BAKE_GRID,
  type ThickBakeBody,
  thickBakeBands,
  type ThickBakeGrid,
  thickBakeLights,
} from "./bake";
import {
  bakeDeck,
  bakeDeckPolarisation,
  type DeckBandOptions,
  type DeckOptions,
  type DeckSplit,
  type DeckTables,
  prepareDeck,
  prepareDeckPolarisation,
  type PreparedDeck,
} from "./deck";
import {
  bakePolarisationCorrection,
  needsPolarisationCorrection,
  type PolarisationTable,
  preparePolarisation,
} from "./polarisation";
import { type SpectralColumn, type SpectralSource, spectralColumnOf } from "./spectralColumn";

/**
 * A kept medium with its terms' sources, as `AtmosphereCache.ts`'s `BuiltAtmosphere` and
 * `reference/cases.ts`'s `ReferenceCase` carry them; `sources` absent for one that is never baked.
 */
export interface KeptMedium {
  readonly medium: AtmosphereMedium;
  readonly sources?: ReadonlyArray<SpectralSource>;
}

/**
 * A kept medium's spectral column: `spectralColumnOf` of its medium and sources.
 *
 * @throws RangeError for a kept medium with no sources, or as `spectralColumnOf` refuses them.
 */
export function keptColumnOf(kept: KeptMedium): SpectralColumn {
  if (kept.sources === undefined) {
    throw new RangeError(`the kept medium ${kept.medium.name} has no sources to bake from`);
  }
  return spectralColumnOf(kept.medium, kept.sources);
}

/** What a direct kept bake takes beside its medium, lights and body. */
export interface KeptBakeOptions {
  /** The bands to bake, indices of `bake.ts`'s `thickBakeBands(body)`; every band by default. */
  readonly bands?: ReadonlyArray<number>;
  /**
   * The bake's grid; `bake.ts`'s `THICK_BAKE_GRID` by default, with its 14 rows below the horizontal
   * (R08.T14.d.2), for the bake and the correction alike.
   */
  readonly grid?: ThickBakeGrid;
}

/**
 * A kept medium's thick tables, one a band asked in its order, each with a slot a light of
 * `lights` in theirs: `prepareBake` of its column under the lights' bin curves, as the optics
 * worker forms them (`bake.ts`'s `thickBakeLights`), then `bakeMultipleScattering` a band.
 *
 * @throws RangeError as {@link keptColumnOf}, `thickBakeLights`, `prepareBake` and
 *   `bakeMultipleScattering` refuse their inputs (a band the body does not have).
 */
export function bakeKeptCase(
  kept: KeptMedium,
  lights: KeptBakeLights,
  body: ThickBakeBody,
  options: KeptBakeOptions = {},
): ReadonlyArray<MsSourceTable> {
  const column = keptColumnOf(kept);
  const prepared = prepareBake(
    column,
    thickBakeLights(column, lights.absorbers, lights.spectra),
    options.grid ?? THICK_BAKE_GRID,
  );
  const bands = options.bands ?? thickBakeBands(body).map((_, band) => band);
  return bands.map((band) => bakeMultipleScattering(prepared, body, band));
}

/**
 * A kept medium's polarisation correction (R08.T14.f), one table a band asked in its order, each with
 * a slot a light of `lights` in theirs: `preparePolarisation` of its column under the lights' bin
 * curves, formed as {@link bakeKeptCase} forms them, then `bakePolarisationCorrection` a band, as the
 * optics worker answers a kept body's `polarisation-bake`.
 *
 * @throws RangeError as {@link keptColumnOf}, `thickBakeLights`, `preparePolarisation` and
 *   `bakePolarisationCorrection` refuse their inputs (a band the body does not have).
 */
export function bakeKeptCorrection(
  kept: KeptMedium,
  lights: KeptBakeLights,
  body: ThickBakeBody,
  options: KeptBakeOptions = {},
): ReadonlyArray<PolarisationTable> {
  const column = keptColumnOf(kept);
  const prepared = preparePolarisation(
    column,
    thickBakeLights(column, lights.absorbers, lights.spectra),
    { grid: options.grid ?? THICK_BAKE_GRID },
  );
  const bands = options.bands ?? thickBakeBands(body).map((_, band) => band);
  return bands.map((band) => bakePolarisationCorrection(prepared, body, band));
}

/** A kept medium's deck as a direct bake makes it (R08.T15.a.2). */
export interface KeptDeck {
  /** The deck's preparation, for a caller's records of its layers and floor. */
  readonly prepared: PreparedDeck;
  /** One a band asked, in its order, each with a slot a light. */
  readonly tables: ReadonlyArray<DeckTables>;
  /** The correction over the deck, one a band; `null` where the column needs none. */
  readonly correction: ReadonlyArray<PolarisationTable> | null;
}

/** What a direct deck's bake takes beside a kept bake's options: the deck's and its bands'. */
export interface KeptDeckOptions extends KeptBakeOptions {
  /** The deck's preparation's options, the grid's from {@link KeptBakeOptions.grid}. */
  readonly deck?: Omit<DeckOptions, "grid">;
  /** Each band's options (`thick/deck.ts`'s `DeckBandOptions`). */
  readonly band?: DeckBandOptions;
}

/**
 * A kept medium's deck split at `split` (R08.T15.a.2), as `opticsJobs.ts` answers a kept body's
 * `deck-bake`: `prepareDeck` of its column under the lights' bin curves, formed as
 * {@link bakeKeptCase} forms them, each band asked's `bakeDeck`, and where the column needs it the
 * correction over the deck a band.
 *
 * @throws RangeError as {@link keptColumnOf}, `thickBakeLights`, `prepareDeck`, `bakeDeck` and the
 *   correction refuse their inputs (a band the body does not have).
 */
export function bakeKeptDeck(
  kept: KeptMedium,
  lights: KeptBakeLights,
  body: ThickBakeBody,
  split: DeckSplit,
  options: KeptDeckOptions = {},
): KeptDeck {
  const column = keptColumnOf(kept);
  const bakeLights = thickBakeLights(column, lights.absorbers, lights.spectra);
  const prepared = prepareDeck(column, bakeLights, split, {
    ...options.deck,
    grid: options.grid ?? THICK_BAKE_GRID,
  });
  const bands = options.bands ?? thickBakeBands(body).map((_, band) => band);
  const tables = bands.map((band) => bakeDeck(prepared, body, band, options.band));
  let correction: ReadonlyArray<PolarisationTable> | null = null;
  if (needsPolarisationCorrection(column)) {
    const polarisation = prepareDeckPolarisation(prepared, bakeLights);
    correction = bands.map((band) => bakeDeckPolarisation(prepared, polarisation, body, band));
  }
  return { prepared, tables, correction };
}

/**
 * The bake's body of a kept case on a sphere (`BuiltAtmosphere.body` `null`, one slice and one
 * band): a sphere of the tables' ground radius with no rotation section, whose one band stands at
 * s = 1 on R_ref, its gravity the bulk gravity everywhere (`oblate.ts`'s `bodyGravity`).
 *
 * @param radiusM - R_ref, m: the sphere's radius and the tables' ground.
 * @param surfaceGravityMS2 - Its gravity, m s⁻², finite and above 0: its column's g_ref. The bake
 *   reads only the body's one band, so it moves nothing in the table.
 * @throws RangeError for a gravity that is not finite and above 0.
 */
export function sphereBakeBody(radiusM: number, surfaceGravityMS2: number): ThickBakeBody {
  const figure = { equatorialRadiusM: radiusM, polarRadiusM: radiusM };
  return {
    figure,
    gravity: bodyGravity(
      {
        figure,
        massKg: (surfaceGravityMS2 * radiusM * radiusM) / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
        angularVelocityRadS: null,
        bulkGravityMS2: surfaceGravityMS2,
        figureLaw: null,
      },
      radiusM,
    ),
    bottomRadiusM: radiusM,
  };
}
