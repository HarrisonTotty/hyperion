/**
 * A kept scene's thick bake made directly, with no optics worker (plan R08, R08.T14.c;
 * `decision-r08-t13c2-classes.md` §6): for the tests and the recorded runs, from a kept medium and
 * its sources, as `opticsJobs.ts` answers a kept body's `thick-bake`.
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
  /** The bake's grid; `bake.ts`'s `THICK_BAKE_GRID` by default. */
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
