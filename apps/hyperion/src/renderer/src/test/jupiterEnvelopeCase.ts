/**
 * `jupiter-envelope`, the reference tracer's absorbing envelope case (plan R08, R08.T16.a.2): the
 * hand Jupiter's medium (`giantFixtures.ts`'s `handJupiterDetail`), whose chromophore absorbs above
 * its envelope's floor, for `hyperion-fit atmosphere-reference` to trace and the envelope's gate to
 * hold the route's twin to as `giant-deck`'s envelope variant is held (`reference/vectorGate.test.ts`).
 *
 * @remarks
 * - **The medium** is the hand Jupiter's as the envelope assembles it, from
 *   {@link JUPITER_ENVELOPE_BASE_DEPTH} below its top down (`thick/deckRead.ts`'s `upperMediumOf`),
 *   over a black ground there: `giant-deck`'s shape, a deck read to its column's base over black
 *   beneath an envelope's floor at τ 10 (`thick/deck.ts`'s `DeckFloor`), here its H₂O, NH₄SH and NH₃
 *   decks and its chromophore haze above the floor and its gas beneath. Its terms are the drawn
 *   medium's, their sources and their densities on the drawn column's scale, its absorbers' curves
 *   under the reference Sun (`earth.ts`'s `R06_SOLAR_BAKE_SPECTRUM`), and its tabulated phases'
 *   mean cosines their modes' own. The column's levels at its floor and its decks' bases stand
 *   0.16–0.74 mm apart, which the case's 9 figures would merge at a millimetre; each is held
 *   {@link JUPITER_ENVELOPE_LEAST_LEVEL_SPACING_M} above the one below it, in the medium the gate's
 *   twin draws as in the file: the H₂O sheet, some 5 m deep, loses 0.08% of its column (τ 0.0014),
 *   the other terms under 0.015%.
 * - **The figure** is a sphere through the cut, the drawn tables' ground radius
 *   (`assemble.ts`'s `bottomRadiusM`) raised by it, as `giant-deck` stands on Saturn's.
 * - **The geometries** are Design note 10's disc and limb from orbit, the views the gate takes of an
 *   envelope, which has no view from below; no ground, aerial or flux aggregate is traced. The limb
 *   keeps the tangents below the case's top: the medium's scale height at its ground is 151 km
 *   (`medium.ts`'s `localScaleHeightM`, its decks' τ over its gas's extinction), so that 10 H
 *   stands past the top and the orbit.
 *
 * Its Mie runs for a few seconds; the case is built once a module and held.
 */

import {
  type AtmosphereMedium,
  columnLengthM,
  type DensityProfile,
  extinction,
  localScaleHeightM,
  type MediumTerm,
} from "../view/atmosphere/medium";
import { assembleMedium } from "../view/atmosphere/assemble";
import { absorberCurve } from "../view/atmosphere/absorbers";
import { aerosolTermOptics } from "../view/atmosphere/aerosol";
import { R06_SOLAR_BAKE_SPECTRUM } from "../view/atmosphere/earth";
import {
  type ReferenceCase,
  SPHERE_ORBIT_HEIGHT_M,
  sphereCase,
} from "../view/atmosphere/reference/cases";
import {
  discSet,
  nameLabel,
  SPHERE_LIMB_SCALE_HEIGHTS,
  sphereLimbImage,
} from "../view/atmosphere/reference/geometries";
import { THICK_CASE_SIGNIFICANT_FIGURES } from "../view/atmosphere/reference/thickCases";
import type { DeckSplit } from "../view/atmosphere/thick/deck";
import { upperMediumOf } from "../view/atmosphere/thick/deckRead";
import { ENVELOPE_FLOOR_CHANNEL } from "../view/atmosphere/thick/regime";
import type { Rgb } from "../view/photometry/toneCurve";
import { type DetailAnswer, detailOf, recordInput } from "./atmosphereRecords";
import { handJupiterDetail } from "./giantFixtures";

/** The case's name: its files are `jupiter-envelope.case.json` and `.reference.json`. */
export const JUPITER_ENVELOPE_CASE = "jupiter-envelope";

/**
 * How deep the case's column reaches: its green vertical optical depth from the top, of every term
 * but the absorbers (`thick/regime.ts`'s `envelopeFloorM`'s reading), 20, about `giant-deck`'s
 * whole column's (its deck's 20 and its gas's), twice the envelope's floor's 10.
 */
export const JUPITER_ENVELOPE_BASE_DEPTH = 20;

/**
 * The least spacing of a term's levels in the case, m: 1 cm, ten of the millimetres the case's 9
 * figures keep of heights under 1,000 km.
 */
export const JUPITER_ENVELOPE_LEAST_LEVEL_SPACING_M = 0.01;

/** An envelope's case and the split its gate reads it at. */
export interface EnvelopeCase {
  readonly referenceCase: ReferenceCase;
  /**
   * The giant's envelope floor in the case's heights, the split its route takes
   * (`assemble.ts`'s `DrawnEnvelope.floorAltitudeM`, the cover's floor), over a black floor.
   */
  readonly split: DeckSplit;
  /** Where the case's ground lies in the drawn medium's heights, m. */
  readonly baseAltitudeM: number;
}

/** The highest height of `medium` at which its non-absorbing terms' green τ from the top is `depth`. */
function depthHeightM(medium: AtmosphereMedium, depth: number): number {
  const top = medium.topHeightM;
  const terms = medium.terms.filter((term) => term.absorber === undefined);
  const columnsToTop = terms.map((term) => columnLengthM(term.density, top));
  const depthAbove = (heightM: number): number =>
    terms.reduce(
      (sum, term, i) =>
        sum +
        extinction(term)[ENVELOPE_FLOOR_CHANNEL] *
          ((columnsToTop[i] ?? Number.NaN) - columnLengthM(term.density, heightM)),
      0,
    );
  if (!(depthAbove(0) > depth)) {
    throw new RangeError(`the medium's whole column is shallower than τ ${depth}`);
  }
  let low = 0;
  let high = top;
  for (let step = 0; step < 64; step += 1) {
    const middle = 0.5 * (low + high);
    if (depthAbove(middle) >= depth) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return low;
}

/** `density` with each level at least {@link JUPITER_ENVELOPE_LEAST_LEVEL_SPACING_M} above the last. */
function heldApart(density: DensityProfile, topHeightM: number): DensityProfile {
  if (density.kind !== "tabulated") {
    return density;
  }
  const altitudesM = Float64Array.from(density.altitudesM);
  for (let i = 1; i < altitudesM.length; i += 1) {
    const least = (altitudesM[i - 1] ?? Number.NaN) + JUPITER_ENVELOPE_LEAST_LEVEL_SPACING_M;
    altitudesM[i] = Math.max(altitudesM[i] ?? Number.NaN, least);
  }
  if (
    !((altitudesM.at(-1) ?? Number.NaN) <= Math.max(topHeightM, density.altitudesM.at(-1) ?? 0))
  ) {
    throw new RangeError("a profile's levels held apart pass its medium's top");
  }
  return { ...density, altitudesM };
}

/** `term` with its density's levels held apart ({@link heldApart}). */
function termHeldApart(term: MediumTerm, topHeightM: number): MediumTerm {
  return { ...term, density: heldApart(term.density, topHeightM) };
}

/** The case once built, for the module's life. */
let built: EnvelopeCase | undefined;

/**
 * `jupiter-envelope` (the module's remarks), built on the first call and held.
 *
 * @throws Error if the hand Jupiter draws no envelope's medium: a broken fixture.
 */
export function jupiterEnvelopeCase(): EnvelopeCase {
  built ??= handEnvelopeCase(JUPITER_ENVELOPE_CASE, handJupiterDetail());
  return built;
}

/**
 * The absorbing envelope case `name` of a hand giant's `detail`, as `jupiter-envelope` is built
 * (the module's remarks): its medium as its envelope assembles, cut at green τ
 * {@link JUPITER_ENVELOPE_BASE_DEPTH} from the top over a black ground, its levels held
 * {@link JUPITER_ENVELOPE_LEAST_LEVEL_SPACING_M} apart, on a sphere through the cut, under the disc
 * and the limb from orbit (each tangent below its top), split at its envelope floor.
 *
 * @throws Error if the giant draws no envelope's medium, or its floor lies below the cut.
 */
export function handEnvelopeCase(name: string, detail: DetailAnswer): EnvelopeCase {
  const { medium, drawn } = assembleMedium(recordInput(detailOf(detail)), {
    envelopes: true,
  });
  if (medium === undefined || drawn.envelope === undefined) {
    throw new Error(`${name}'s giant draws an envelope's medium`);
  }
  const baseM = depthHeightM(medium, JUPITER_ENVELOPE_BASE_DEPTH);
  const floorM = drawn.envelope.floorAltitudeM;
  if (!(floorM > baseM)) {
    throw new Error(`${name}'s floor at ${floorM} m lies at or below the case's base ${baseM} m`);
  }
  const cut = upperMediumOf(medium, baseM);
  const caseMedium: AtmosphereMedium = {
    ...cut,
    name,
    groundAlbedo: [0, 0, 0],
    terms: cut.terms.map((term) => termHeldApart(term, cut.topHeightM)),
  };
  const phaseAsymmetry = Object.fromEntries(
    drawn.sources.flatMap((source, k): [string, Rgb][] => {
      const term = medium.terms[k];
      if (source.kind !== "aerosol" || term === undefined) {
        return [];
      }
      const [red, green, blue] = aerosolTermOptics(source.particles).channels;
      return [[term.name, [red.asymmetry, green.asymmetry, blue.asymmetry]]];
    }),
  );
  const radiusM = drawn.bottomRadiusM + baseM;
  const scaleHeightM = localScaleHeightM(caseMedium, 0);
  const tangents = SPHERE_LIMB_SCALE_HEIGHTS.filter(
    (n) => n * scaleHeightM < caseMedium.topHeightM,
  ).map((n) => ({ name: `limb-${nameLabel(n)}H`, heightM: n * scaleHeightM }));
  const base = sphereCase(
    name,
    caseMedium,
    drawn.sources,
    radiusM,
    drawn.absorbers.map((absorber) => absorberCurve(absorber, R06_SOLAR_BAKE_SPECTRUM)),
    phaseAsymmetry,
  );
  return {
    referenceCase: {
      ...base,
      geometries: [
        ...discSet(SPHERE_ORBIT_HEIGHT_M),
        ...sphereLimbImage(radiusM, tangents, "limb", SPHERE_ORBIT_HEIGHT_M),
      ],
      aggregates: [],
      significantFigures: THICK_CASE_SIGNIFICANT_FIGURES,
    },
    split: { altitudeM: floorM - baseM, floor: "envelope" },
    baseAltitudeM: baseM,
  };
}
