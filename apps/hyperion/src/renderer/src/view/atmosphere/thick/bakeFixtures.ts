/**
 * The thick bake's fixtures (plan R08, R08.T14.b's tests and benchmark): a Venus-class and a
 * Titan-class column with their drawn media, the spectral column of each built from the same
 * sources as its drawn terms.
 *
 * @remarks
 * A test fixture, not used at run time. It sits here rather than in `test/`, the shared fixtures'
 * home, because R08.T14.b's lane may not edit `test/`; it moves there when a later task can (plan
 * R08's Risks, "Deferred findings (2026-10-13 rule)" of R08.T14.b).
 *
 * The sources a drawn medium's terms come from are `assemble.ts`'s own and private (R08.T14.c
 * exposes them). Here they are the inputs each term was built from: for the record fixture, the
 * record's gases and mode, the drawn terms' densities and amounts, and the molecular term's n_s read
 * from its green coefficient over σ_mix there (to rounding: the hand-off gives n_s itself); for the
 * hand column, the column and the specs the terms were built on.
 */

import type {
  AerosolInventory,
  AerosolMode as RecordAerosolMode,
  BodyDetail,
} from "../../../lib/system/model";
import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../../../lib/system/constants";
import {
  planckBakeSpectrum,
  SUN_TEFF_K,
  TITAN_FRACTIONS,
  TITAN_GM_M3S2,
  TITAN_RADIUS_M,
  TITAN_SURFACE,
  titanClassMethaneMedium,
} from "../../../test/atmosphereFixtures";
import { recordInput, venus92Detail } from "../../../test/atmosphereRecords";
import { inlinePart } from "../../../test/kDistributionFixtures";
import type { AbsorberBinCurves, AbsorberSpec } from "../absorbers";
import { absorberBinCurves } from "../absorbers";
import { loadKDistributions } from "../absorbers/kDistribution";
import { type AerosolModeSpec, type AerosolParticles, aerosolTerm } from "../aerosol";
import { assembleMedium, lineSpeciesOf } from "../assemble";
import type { BodyAtmosphereInput } from "../atmosphereInput";
import {
  type AtmosphereColumn,
  gasProperties,
  hydrostaticColumn,
  meanMolarMassGPerMol,
} from "../column";
import {
  type AtmosphereMedium,
  CHANNEL_WAVELENGTHS_NM,
  columnLengthM,
  type MediumTerm,
} from "../medium";
import { bodyGravity } from "../oblate";
import { molecularMixture } from "../rayleigh";
import type { BakeSpectrum } from "../spectralColour";
import type { ThickBakeBody, ThickBakeLight } from "./bake";
import { type SpectralColumn, type SpectralSource, spectralColumnOf } from "./spectralColumn";

/** A fixture: its drawn medium, its spectral column, its body and its light. */
export interface ThickFixture {
  readonly medium: AtmosphereMedium;
  readonly column: SpectralColumn;
  readonly body: ThickBakeBody;
  /** The absorber terms' specs, in the column's order. */
  readonly specs: ReadonlyArray<AbsorberSpec>;
  readonly sun: BakeSpectrum;
}

/** The Venus-class fixture, with the record input it was assembled from. */
export interface VenusFixture extends ThickFixture {
  readonly input: BodyAtmosphereInput;
}

/** The fixture's one light: the sun's bin curves of its absorbers. */
export function lightOf(fixture: ThickFixture): ThickBakeLight {
  return {
    spectrum: fixture.sun,
    absorbers: fixture.specs.map((spec): AbsorberBinCurves => absorberBinCurves(spec, fixture.sun)),
  };
}

/** A cloud of one mode of gamma-distributed droplets, as {@link VENUS_CLOUD} gives it. */
export interface VenusCloud {
  /** r_eff, m. */
  readonly effectiveRadiusM: number;
  /** v_eff, dimensionless. */
  readonly effectiveVariance: number;
  /** Its base's pressure, Pa. */
  readonly basePa: number;
  /** Its top's pressure, Pa. */
  readonly topPa: number;
  /** Its vertical optical depth at 550 nm. */
  readonly tau550: number;
}

/**
 * The Venus-class cloud: H₂SO₄ droplets of 75 wt% (`materials/sulphuric-acid-75.json`) on a
 * gamma distribution of r_eff 1.05 µm and v_eff 0.07, Hansen and Hovenier 1974's Venus cloud
 * droplets (J. Atmos. Sci. 31 (1974) 1137–1160: r_eff 1.05 ± 0.10 µm, v_eff 0.07 ± 0.02), well mixed
 * from 1 bar (50.4 km in the Venus International Reference Atmosphere, A. Seiff et al., Adv. Space
 * Res. 5(11) (1985) 3–58) to 0.04 bar (69.6 km there); the measured base, 47.9 km (M. G. Tomasko
 * et al., JGR 85 (1980) 8167–8186), lies near 1.37 bar, so the lower cloud is left out. Its vertical
 * optical depth at 550 nm is 30, a test's choice inside the clouds' measured range: about 25 at
 * 0.63 µm (Tomasko et al. 1980) and 25–40 across the Pioneer Venus probes (the rendering
 * brainstorm), the droplets extinguishing nearly alike at 550 and 630 nm.
 */
export const VENUS_CLOUD: VenusCloud = {
  effectiveRadiusM: 1.05e-6,
  effectiveVariance: 0.07,
  basePa: 1e5,
  topPa: 4e3,
  tau550: 30,
};

/** {@link venus92Detail} with {@link VENUS_CLOUD} as its inventory's one mode. */
export function venusCloudDetail(): BodyDetail {
  const detail = venus92Detail();
  const surface = detail.record.surface;
  if (surface.state !== "ok") {
    throw new Error("the Venus-class record's surface is not ok");
  }
  const mode: RecordAerosolMode = {
    material: "H2SO4",
    phase: "liquid",
    deck: null,
    shape: "sphere",
    sizeDistribution: {
      kind: "value",
      value: {
        kind: "gamma",
        effectiveRadiusM: VENUS_CLOUD.effectiveRadiusM,
        effectiveVariance: VENUS_CLOUD.effectiveVariance,
      },
    },
    columnMassKgM2: { kind: "value", value: 1e-3 },
    opticalDepth: { kind: "value", value: { tau550: VENUS_CLOUD.tau550, basis: "sphere" } },
    basePa: VENUS_CLOUD.basePa,
    topPa: VENUS_CLOUD.topPa,
    fSed: 0,
    coverage: 1,
    temperatureK: { kind: "value", value: 300 },
    notModelled: null,
  };
  const inventory: AerosolInventory = { modes: [mode], decks: [], absorbers: [], notFormed: [] };
  return {
    ...detail,
    record: {
      ...detail.record,
      surface: {
        state: "ok",
        value: { ...surface.value, aerosols: { kind: "value", value: inventory } },
      },
    },
  };
}

/** The particles `assemble.ts` draws {@link VENUS_CLOUD} with: its distribution of spheres. */
export const VENUS_CLOUD_PARTICLES: AerosolParticles = {
  kind: "distribution",
  mode: {
    material: "H2SO4",
    form: { phase: "liquid" },
    shape: "sphere",
    sizes: {
      kind: "gamma",
      effectiveRadiusUm: VENUS_CLOUD.effectiveRadiusM * 1e6,
      effectiveVariance: VENUS_CLOUD.effectiveVariance,
    },
  },
};

/** The term of a medium with a name, or a test failure. */
function termOf(medium: AtmosphereMedium, name: string): MediumTerm {
  const term = medium.terms.find((t) => t.name === name);
  if (term === undefined) {
    throw new Error(`medium ${medium.name} has no term ${name}`);
  }
  return term;
}

/**
 * The Venus-class fixture: {@link venusCloudDetail} under the Sun (a 5,772 K Planck sun) as the
 * assembly draws it, its CO₂'s lines on the committed tables, and its spectral column.
 */
export async function venusClassFixture(): Promise<VenusFixture> {
  const sun = planckBakeSpectrum(SUN_TEFF_K);
  const input = recordInput(venusCloudDetail(), [{ star: "the Sun", spectrum: sun }]);
  const kDistributions = await loadKDistributions(lineSpeciesOf(input), inlinePart);
  const assembled = assembleMedium(input, { kDistributions });
  if (assembled.medium === undefined) {
    throw new Error("the Venus-class record draws no medium");
  }
  const { medium, drawn } = assembled;
  const surface =
    input.kind === "record" && input.detail.record.surface.state === "ok"
      ? input.detail.record.surface.value
      : undefined;
  if (surface === undefined) {
    throw new Error("the Venus-class record has no surface");
  }
  const rayleigh = termOf(medium, "rayleigh");
  const green = CHANNEL_WAVELENGTHS_NM[1];
  const sources = medium.terms.map((term): SpectralSource => {
    if (term.absorber !== undefined) {
      return {
        kind: "absorber",
        amount:
          term.absorber.kind === "column"
            ? { kind: "column", numberDensityPerM3: term.absorber.numberDensityPerM3 }
            : { kind: "pair", pairDensityPerM6: term.absorber.pairDensityPerM6 },
      };
    }
    if (term === rayleigh) {
      return {
        kind: "molecular",
        fractions: surface.gases,
        numberDensityPerM3:
          rayleigh.scattering[1] / molecularMixture(surface.gases).crossSectionM2(green),
      };
    }
    if (term.name === "aerosol:H2SO4") {
      return {
        kind: "aerosol",
        particles: VENUS_CLOUD_PARTICLES,
        extinction550PerM: VENUS_CLOUD.tau550 / columnLengthM(term.density, medium.topHeightM),
      };
    }
    throw new Error(`the Venus-class fixture does not expect term ${term.name}`);
  });
  return {
    input,
    medium,
    column: spectralColumnOf(medium, sources),
    body: { ...drawn.body, bottomRadiusM: drawn.bottomRadiusM },
    specs: drawn.absorbers,
    sun,
  };
}

/**
 * The Titan-class haze: tholin (`materials/tholin.json`) as aggregates of 3,000 monomers of
 * 0.05 µm at D_f 2, `aerosol.test.ts`'s Titan-like aggregate after Tomasko et al. 2008's model of
 * Titan's haze (M. G. Tomasko et al., Planet. Space Sci. 56 (2008) 669–707, not read here: S. M.
 * Hörst, JGR Planets 122 (2017) 432, quotes it as 4,000 monomers of 0.04 µm), well mixed through
 * the column, of vertical optical depth 8.1 at 550 nm: the column Huygens' DISR measured (Tomasko
 * et al. 2008, Fig. 47, as M. Bazzon, H. M. Schmid and E. Buenzli, A&A 572 (2014) A6, eqs.
 * A.8–A.10, fit it: 3.94 above 80 km, 2.79 from 30 to 80 km and 1.37 below), as `hyperion-sim`'s
 * Titan haze takes it.
 */
export const TITAN_HAZE: AerosolModeSpec = {
  name: "aerosol:tholin",
  particles: {
    kind: "aggregate",
    mode: {
      material: "tholin",
      form: { phase: "solid" },
      monomerRadiusUm: 0.05,
      monomerCount: 3_000,
      fractalDimension: 2,
    },
  },
  opticalDepth550: 8.1,
  basePa: TITAN_SURFACE.pressurePa,
  topPa: 1,
  mixingExponent: 0,
};

/**
 * The Titan-class column `atmosphereFixtures.ts`'s `titanClassMethaneMedium` is built on: the same
 * inputs, to the bit.
 */
export function titanClassColumn(): AtmosphereColumn {
  const fractions = TITAN_FRACTIONS.map(({ species, moleFraction }) => ({
    properties: gasProperties(species),
    moleFraction,
  }));
  return hydrostaticColumn({
    surfacePa: TITAN_SURFACE.pressurePa,
    temperature: { kind: "isothermal", temperatureK: TITAN_SURFACE.temperatureK },
    meanMolarMassGPerMol: meanMolarMassGPerMol(fractions),
    referenceGravityMS2: TITAN_GM_M3S2 / TITAN_RADIUS_M ** 2,
    referenceRadiusM: TITAN_RADIUS_M,
  });
}

/** A sphere of Titan's radius with no rotation section: one latitude band at s = 1. */
export function titanBody(): ThickBakeBody {
  const figure = { equatorialRadiusM: TITAN_RADIUS_M, polarRadiusM: TITAN_RADIUS_M };
  return {
    figure,
    gravity: bodyGravity(
      {
        figure,
        massKg: TITAN_GM_M3S2 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
        angularVelocityRadS: null,
        bulkGravityMS2: TITAN_GM_M3S2 / TITAN_RADIUS_M ** 2,
        figureLaw: null,
      },
      TITAN_RADIUS_M,
    ),
    bottomRadiusM: TITAN_RADIUS_M,
  };
}

/**
 * The Titan-class fixture: `titanClassMethaneMedium`'s air (N₂ with 5.65% CH₄, its methane one
 * absorber) with {@link TITAN_HAZE} added on the same column, under the Sun.
 */
export function titanClassFixture(): ThickFixture {
  const column = titanClassColumn();
  const base = titanClassMethaneMedium();
  const haze = aerosolTerm(TITAN_HAZE, column);
  const medium: AtmosphereMedium = { ...base.medium, terms: [...base.medium.terms, haze.term] };
  const specs: AbsorberSpec[] = [];
  const sources = medium.terms.map((term): SpectralSource => {
    if (term.absorber !== undefined) {
      if (term.absorber.kind !== "column") {
        throw new Error("the Titan-class fixture's absorber is a column absorber");
      }
      specs.push(term.absorber.spec);
      return {
        kind: "absorber",
        amount: { kind: "column", numberDensityPerM3: term.absorber.numberDensityPerM3 },
      };
    }
    if (term.name === "rayleigh") {
      return {
        kind: "molecular",
        fractions: TITAN_FRACTIONS,
        numberDensityPerM3: column.surfaceNumberDensityPerM3,
      };
    }
    if (term.name === TITAN_HAZE.name) {
      return {
        kind: "aerosol",
        particles: TITAN_HAZE.particles,
        extinction550PerM:
          TITAN_HAZE.opticalDepth550 / columnLengthM(term.density, column.topHeightM),
      };
    }
    throw new Error(`the Titan-class fixture does not expect term ${term.name}`);
  });
  return {
    medium,
    column: spectralColumnOf(medium, sources),
    body: titanBody(),
    specs,
    sun: planckBakeSpectrum(SUN_TEFF_K),
  };
}
