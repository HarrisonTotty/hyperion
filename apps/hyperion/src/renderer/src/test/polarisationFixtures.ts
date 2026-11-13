/**
 * The polarisation correction's fixtures (plan R08, R08.T14.e's tests and benchmark): the generated
 * Earth's column as `bakeJob.test.ts` forms it, and a three-bar N₂ world, beside
 * `bakeFixtures.ts`'s Venus-class and Titan-class columns.
 *
 * @remarks
 * A test fixture, not used at run time, beside `bakeFixtures.ts` in `test/` since R08.T14.c (plan
 * R08's Risks, T14.b's and T14.e's deferred findings "The fixtures' home").
 */

import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../lib/system/constants";
import { planckBakeSpectrum, SUN_TEFF_K, WGS84_SPHEROID } from "./atmosphereFixtures";
import { detailOf, recordInput } from "./atmosphereRecords";
import { toyLoader } from "./opticsFakes";
import { earthDetail } from "./planetaryFixture";
import type { AbsorberSpec } from "../view/atmosphere/absorbers";
import { type AssembledMedium, assembleMedium, lineSpeciesOf } from "../view/atmosphere/assemble";
import type { BodyAtmosphereInput } from "../view/atmosphere/atmosphereInput";
import { gasProperties, hydrostaticColumn, meanMolarMassGPerMol } from "../view/atmosphere/column";
import { type AtmosphereMedium, CHANNEL_WAVELENGTHS_NM } from "../view/atmosphere/medium";
import { bodyGravity } from "../view/atmosphere/oblate";
import { molecularMixture, molecularTerm } from "../view/atmosphere/rayleigh";
import type { ThickBakeBody } from "../view/atmosphere/thick/bake";
import type { ThickFixture } from "./bakeFixtures";
import type { SpectralColumn } from "../view/atmosphere/thick/spectralColumn";

/** The generated Earth's fixture, with the record input and the assembly it was formed from. */
export interface EarthFixture extends ThickFixture {
  readonly input: BodyAtmosphereInput;
  readonly assembled: AssembledMedium;
}

/**
 * The generated Earth (`test/planetaryFixture.ts`'s `earthDetail`) under the Sun, its lines on the
 * toy tables: a column of its molecular term and its first lined absorber by their sources, as
 * `bakeJob.test.ts` forms it (n_s read from the term's green coefficient over σ_mix there). The
 * generated Earth draws no aerosol (its record's inventory is not modelled), so none is added.
 */
export async function earthFixture(): Promise<EarthFixture> {
  const input = recordInput(detailOf(earthDetail()));
  const kDistributions = await toyLoader().loadKDistributions(lineSpeciesOf(input));
  const assembled = assembleMedium(input, { kDistributions });
  if (assembled.medium === undefined || input.kind !== "record") {
    throw new Error("the Earth draws a medium");
  }
  const surface = input.detail.record.surface;
  if (surface.state !== "ok") {
    throw new Error("the Earth has a surface");
  }
  const { medium, drawn } = assembled;
  const rayleigh = medium.terms.find((t) => t.name === "rayleigh");
  const lined = drawn.absorbers.find(
    (spec) => spec.kind === "column" && spec.species.some((s) => s.lines !== undefined),
  );
  const absorber = medium.terms.find((t) => t.name === lined?.name);
  if (rayleigh === undefined || lined === undefined || absorber?.absorber?.kind !== "column") {
    throw new Error("the Earth has a molecular term and a lined absorber");
  }
  const sun = input.lights[0]?.spectrum;
  if (sun === undefined) {
    throw new Error("the Earth has a sun");
  }
  const gases = surface.value.gases;
  const column: SpectralColumn = {
    name: "the Earth's air and one absorber",
    topHeightM: medium.topHeightM,
    groundAlbedo: medium.groundAlbedo,
    terms: [
      {
        name: rayleigh.name,
        density: rayleigh.density,
        source: {
          kind: "molecular",
          fractions: gases,
          numberDensityPerM3:
            rayleigh.scattering[1] /
            molecularMixture(gases).crossSectionM2(CHANNEL_WAVELENGTHS_NM[1]),
        },
      },
      {
        name: absorber.name,
        density: absorber.density,
        source: {
          kind: "absorber",
          amount: { kind: "column", numberDensityPerM3: absorber.absorber.numberDensityPerM3 },
        },
      },
    ],
    ...(medium.pressureBar === undefined ? {} : { pressureBar: medium.pressureBar }),
  };
  const subset: AtmosphereMedium = { ...medium, terms: [rayleigh, absorber] };
  return {
    input,
    assembled,
    medium: subset,
    column,
    body: { ...drawn.body, bottomRadiusM: drawn.bottomRadiusM },
    specs: [lined],
    sun,
  };
}

/**
 * The three-bar world's surface pressure, Pa: three standard atmospheres, 1 atm = 101,325 Pa
 * exactly (a defined value).
 */
export const THREE_BAR_SURFACE_PA = 3 * 101_325;

/**
 * Its isothermal temperature, K: Earth's mean surface temperature (the NSSDCA Earth fact sheet's
 * average temperature, 288 K), a test's choice.
 */
export const THREE_BAR_TEMPERATURE_K = 288;

/** GM of the Earth with its atmosphere, m³ s⁻²: WGS 84's (`test/atmosphereFixtures.ts`). */
const EARTH_GM_M3S2 = WGS84_SPHEROID.gmM3S2;

/**
 * The Earth's mean radius, m: the IUGG's R₁ = 6,371.0088 km (H. Moritz, "Geodetic Reference System
 * 1980", J. Geodesy 74 (2000) 128–133), to the kilometre, a test's sphere.
 */
const EARTH_RADIUS_M = 6.371e6;

/**
 * The three-bar world (decision-r08-vector item 2's "three-bar N₂-like world, 3 × Earth's τ_R, no
 * aerosol"): pure N₂ at {@link THREE_BAR_SURFACE_PA}, isothermal at {@link
 * THREE_BAR_TEMPERATURE_K}, on an Earth-sized sphere of one band, over a black ground as the
 * ruling's measurements are, under the Sun, with no absorber. Its τ_R is a little over the
 * ruling's 3 × Earth's: N₂ is lighter than air, so three atmospheres of it hold more molecules
 * than three of air.
 */
export function threeBarFixture(): ThickFixture {
  const fractions = [{ species: "N2", moleFraction: 1 }] as const;
  const gasFractions = fractions.map(({ species, moleFraction }) => ({
    properties: gasProperties(species),
    moleFraction,
  }));
  const gravityMS2 = EARTH_GM_M3S2 / EARTH_RADIUS_M ** 2;
  const air = hydrostaticColumn({
    surfacePa: THREE_BAR_SURFACE_PA,
    temperature: { kind: "isothermal", temperatureK: THREE_BAR_TEMPERATURE_K },
    meanMolarMassGPerMol: meanMolarMassGPerMol(gasFractions),
    referenceGravityMS2: gravityMS2,
    referenceRadiusM: EARTH_RADIUS_M,
  });
  const medium: AtmosphereMedium = {
    name: "three-bar N2",
    topHeightM: air.topHeightM,
    groundAlbedo: [0, 0, 0],
    terms: [molecularTerm(air, fractions).term],
  };
  const figure = { equatorialRadiusM: EARTH_RADIUS_M, polarRadiusM: EARTH_RADIUS_M };
  const body: ThickBakeBody = {
    figure,
    gravity: bodyGravity(
      {
        figure,
        massKg: EARTH_GM_M3S2 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
        angularVelocityRadS: null,
        bulkGravityMS2: gravityMS2,
        figureLaw: null,
      },
      EARTH_RADIUS_M,
    ),
    bottomRadiusM: EARTH_RADIUS_M,
  };
  const specs: AbsorberSpec[] = [];
  return {
    medium,
    column: {
      name: medium.name,
      topHeightM: medium.topHeightM,
      groundAlbedo: medium.groundAlbedo,
      terms: [
        {
          name: "rayleigh",
          density: air.density,
          source: {
            kind: "molecular",
            fractions,
            numberDensityPerM3: air.surfaceNumberDensityPerM3,
          },
        },
      ],
    },
    body,
    specs,
    sun: planckBakeSpectrum(SUN_TEFF_K),
  };
}
