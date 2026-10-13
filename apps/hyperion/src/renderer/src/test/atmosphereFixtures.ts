/**
 * Media and curves of growth for the atmosphere's tests and the smoke page's checks of R08.T6.c
 * (plan R08, Design notes 5 and 8): R05's Earth with its ozone as an absorber reduced to the linear
 * regime, a medium of four absorbers, one a molecule pair, under real curves, and a Venus-class
 * medium whose slant paths a stored transmittance could not hold. The coefficients of the last two
 * are a test's, not a world's.
 */

import type { Rgb } from "../view/photometry/toneCurve";
import {
  type AbsorberAmount,
  type AbsorberCurve,
  absorberCurve,
  absorberSpecs,
  absorberTerm,
  ABSORBER_CURVE_DECADES,
  ABSORBER_CURVE_NODES_PER_DECADE,
  ABSORBER_TRANSMITTANCE_FLOOR,
  type ColumnAbsorberSpec,
  crossSectionAt,
} from "../view/atmosphere/absorbers";
import {
  type KDistributionTable,
  readKDistribution,
} from "../view/atmosphere/absorbers/kDistribution";
import {
  columnPressureBar,
  dryAdiabatExponent,
  gasProperties,
  hydrostaticColumn,
  type AtmosphereColumn,
} from "../view/atmosphere/column";
import {
  DOBSON_UNIT_PER_M2,
  OZONE_ABSORPTION_PER_M,
  OZONE_COLUMN_DU,
  R05_EARTH_REFERENCE,
} from "../view/atmosphere/earth";
import {
  type AtmosphereMedium,
  BAKE_BIN_COUNT,
  BAKE_BIN_WIDTH_NM,
  BAKE_RANGE_NM,
  columnLengthM,
  type MediumTerm,
  tabulatedDensity,
} from "../view/atmosphere/medium";
import { molecularTerm } from "../view/atmosphere/rayleigh";
import { type BakeSpectrum, bakeSpectrumOf } from "../view/atmosphere/spectralColour";
import type { AbsorberCurveSlots } from "../view/atmosphere/tables";
import { toyPart, toySidecar } from "./kDistributionToys";

/** One Dobson unit, molecules m⁻², `earth.ts`'s (WMO/UNEP 2018, _Twenty Questions_, glossary). */
export { DOBSON_UNIT_PER_M2 };

/**
 * One km-amagat, molecules m⁻²: Loschmidt's number at 273.15 K and 101.325 kPa,
 * 2.686 780 111 × 10²⁵ m⁻³ (CODATA 2018, exact), times 1 km: the unit Karkoschka and Tomasko give
 * methane's k in, and PSG's factor.
 */
export const KM_AMAGAT_PER_M2 = 2.686_780_111e28;

/** An absorber amount's value in its kind's unit: molecules m⁻², or molecule pairs m⁻⁵. */
export function amountValue(amount: AbsorberAmount): number {
  return amount.kind === "column" ? amount.columnPerM2 : amount.pairColumnPerM5;
}

/**
 * A Planck spectrum as bake bins: the mean of B_λ(T) over each bin by a 64-point midpoint rule,
 * scaled to its brightest bin (h, c and k the SI's exact values).
 */
export function planckBakeSpectrum(temperatureK: number): BakeSpectrum {
  const h = 6.626_070_15e-34;
  const c = 299_792_458;
  const k = 1.380_649e-23;
  const values = Array.from({ length: BAKE_BIN_COUNT }, (_, bin) => {
    let sum = 0;
    for (let j = 0; j < 64; j += 1) {
      const m = (BAKE_RANGE_NM[0] + (bin + (j + 0.5) / 64) * BAKE_BIN_WIDTH_NM) * 1e-9;
      sum += 1 / (m ** 5 * Math.expm1((h * c) / (m * k * temperatureK)));
    }
    return sum / 64;
  });
  const peak = Math.max(...values);
  return bakeSpectrumOf(values.map((v) => v / peak));
}

/** The Sun's effective temperature, K: 5,772 (IAU 2015 Resolution B3's nominal T_eff). */
export const SUN_TEFF_K = 5_772;

/**
 * A curve of growth on `absorbers.ts`'s grid about `spec`'s column whose optical depth is σ u per
 * channel: an absorber in the linear regime, clamped at the floor as every curve is.
 *
 * @param crossSectionM2 - σ per channel, m² a molecule.
 */
export function linearAbsorberCurve(spec: ColumnAbsorberSpec, crossSectionM2: Rgb): AbsorberCurve {
  const [first, last] = ABSORBER_CURVE_DECADES;
  const nodes = (last - first) * ABSORBER_CURVE_NODES_PER_DECADE + 1;
  const columns = Float64Array.from(
    { length: nodes },
    (_, j) => spec.columnPerM2 * 10 ** (first + j / ABSORBER_CURVE_NODES_PER_DECADE),
  );
  const transmittance = crossSectionM2.map((sigma) =>
    columns.map((u) => Math.max(Math.exp(-sigma * u), ABSORBER_TRANSMITTANCE_FLOOR)),
  );
  const [red, green, blue] = transmittance;
  if (red === undefined || green === undefined || blue === undefined) {
    throw new Error("a curve has three channels");
  }
  return {
    kind: "column",
    logColumns: columns.map(Math.log),
    logPressures: Float64Array.of(Math.log(spec.basePa)),
    readTopLogPressure: Math.log(spec.basePa),
    transmittance: [red, green, blue],
    opticalDepth: [
      red.map((t) => -Math.log(t)),
      green.map((t) => -Math.log(t)),
      blue.map((t) => -Math.log(t)),
    ],
  };
}

/** A medium and the curves its absorbers are drawn with, light slot 0 a sun's. */
export interface MediumWithCurves {
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
}

/**
 * R05's Earth with its ozone as an absorber (R08.T6.c's regression): the same tent from 10 to 40 km,
 * its 300 DU at n_peak = 300 DU ÷ 15 km, and a curve in the linear regime whose σ are R05's own
 * three coefficients over n_peak (`earth.ts`'s `OZONE_ABSORPTION_PER_M`: Bruneton's and sebh's
 * upward 10 nm bins of Serdyuchenko et al. 2014 at 233 K), so that it reproduces R05's ozone.
 *
 * @remarks
 * Not R08.T4.b's curve: its 1 nm centred bins sit +8.9%, −6.4% and −16.2% from R05's values at
 * 680, 550 and 440 nm (`absorbers.test.ts`), where `decisions-r05.md` item 2's ±5 nm centred bins
 * sit +10%, −6% and −19%.
 */
export function r05EarthWithOzoneAbsorber(): MediumWithCurves {
  const ozone = R05_EARTH_REFERENCE.terms.find((term) => term.name === "ozone");
  if (ozone === undefined) {
    throw new Error("R05's Earth has its ozone");
  }
  const tentLengthM = 15_000;
  const columnPerM2 = OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2;
  const peakPerM3 = columnPerM2 / tentLengthM;
  // The tent is R05's, laid on no column, so the layer's pressures are nominal: the whole air.
  const spec: ColumnAbsorberSpec = {
    kind: "column",
    name: "absorber:O3",
    species: [{ species: "O3", columnPerM2, crossSection: crossSectionAt("O3", 233) }],
    columnPerM2,
    basePa: 101_325,
    topPa: 0,
    temperatureK: 233,
  };
  const term: MediumTerm = {
    name: spec.name,
    density: ozone.density,
    scattering: [0, 0, 0],
    absorption: [0, 0, 0],
    phase: { kind: "none" },
    absorber: { kind: "column", spec, numberDensityPerM3: peakPerM3 },
  };
  const sigma: Rgb = [
    OZONE_ABSORPTION_PER_M[0] / peakPerM3,
    OZONE_ABSORPTION_PER_M[1] / peakPerM3,
    OZONE_ABSORPTION_PER_M[2] / peakPerM3,
  ];
  return {
    medium: {
      ...R05_EARTH_REFERENCE,
      name: "earth, its ozone an absorber",
      terms: R05_EARTH_REFERENCE.terms.map((t) => (t === ozone ? term : t)),
    },
    curves: [[linearAbsorberCurve(spec, sigma)]],
  };
}

/**
 * An Earth-like column of 1 atm, isothermal at 250 K, on WGS 84's mean radius, a test's: its
 * gravity and molar mass are rounded (standard gravity 9.806 65 m s⁻², CGPM 1901; dry air's
 * 28.9644 g mol⁻¹, US Standard Atmosphere 1976), which nothing here depends on.
 */
export function earthLikeColumn(): AtmosphereColumn {
  return hydrostaticColumn({
    surfacePa: 101_325,
    temperature: { kind: "isothermal", temperatureK: 250 },
    meanMolarMassGPerMol: 28.97,
    referenceGravityMS2: 9.806,
    referenceRadiusM: 6_371_008.8,
  });
}

/** Earth's O₂ mole fraction in dry air, 0.209 476 (US Standard Atmosphere 1976, Table 3). */
export const EARTH_OXYGEN_FRACTION = 0.209_476;

/**
 * Four absorbers on an Earth-like column under a 5,772 K Planck sun, by R08.T4.b's and R08.T4.c's
 * own curves, so that the absorber amounts fill the transmittance table's alpha and three channels
 * of the second table (R08.T6.c): 270 DU of stratospheric ozone between 200 and 3 hPa, 1 km-amagat
 * of methane in the whole column, merged with Earth's well-mixed O₂ on the same profile (whose own
 * bands have no cross-sections yet, so add nothing to the curve), the O₂–O₂ pair that O₂ forms,
 * and 30 DU of tropospheric ozone below 200 hPa, a layer of its own; with R05's Rayleigh
 * scattering on the column and R05's aerosol.
 */
export function fourAbsorberMedium(): MediumWithCurves {
  const column = earthLikeColumn();
  const oxygenPerM2 =
    EARTH_OXYGEN_FRACTION *
    column.surfaceNumberDensityPerM3 *
    columnLengthM(column.density, column.topHeightM);
  const { specs } = absorberSpecs(
    [
      { species: "O3", columnPerM2: 270 * DOBSON_UNIT_PER_M2, basePa: 20_000, topPa: 300 },
      { species: "CH4", columnPerM2: KM_AMAGAT_PER_M2, basePa: 101_325, topPa: 0 },
      { species: "O2", columnPerM2: oxygenPerM2, basePa: 101_325, topPa: 0 },
    ],
    column,
  );
  const troposphere: ColumnAbsorberSpec = {
    kind: "column",
    name: "absorber:O3 (troposphere)",
    species: [
      {
        species: "O3",
        columnPerM2: 30 * DOBSON_UNIT_PER_M2,
        crossSection: crossSectionAt("O3", 250),
      },
    ],
    columnPerM2: 30 * DOBSON_UNIT_PER_M2,
    basePa: 101_325,
    topPa: 20_000,
    temperatureK: 250,
  };
  const all = [...specs, troposphere];
  const sun = planckBakeSpectrum(SUN_TEFF_K);
  const [rayleigh, aerosol] = R05_EARTH_REFERENCE.terms;
  if (rayleigh === undefined || aerosol === undefined) {
    throw new Error("R05's Earth has its Rayleigh and aerosol terms");
  }
  return {
    medium: {
      name: "R08.T6.c four absorbers",
      topHeightM: 100_000,
      groundAlbedo: [0.2, 0.15, 0.1],
      terms: [
        { ...rayleigh, density: column.density },
        aerosol,
        ...all.map((spec) => absorberTerm(spec, column)),
      ],
    },
    curves: [all.map((spec) => absorberCurve(spec, sun))],
  };
}

/** The toy line species' broadening pressures, Pa: 4 a decade from 10 Pa to 10 MPa. */
const LINE_TOY_PRESSURES_PA = Array.from({ length: 25 }, (_, k) => 10 * 10 ** (k / 4));

/** The toy line species' temperatures, K. */
const LINE_TOY_TEMPERATURES_K = [150, 200, 250, 300, 350];

/** The toy line species' σ at a node and g-point, m²: its strong core narrowing as 1 ÷ √p past 1 kPa. */
function lineToySigma(t: number, p: number, g: number): number {
  return (g === 0 ? 2e-30 : 6e-27) * (t / 250) * (g === 0 ? 1 : 1 / Math.sqrt(1 + p / 1e3));
}

/**
 * The toy line species as a k-distribution table keyed `species`: three cells at 690–692 nm, each
 * a strong and a weak g-point ({@link lineToySigma}).
 */
function lineToyTable(species: string): KDistributionTable {
  const sigmasM2 = LINE_TOY_TEMPERATURES_K.flatMap((t) =>
    LINE_TOY_PRESSURES_PA.flatMap((p) => [lineToySigma(t, p, 0), lineToySigma(t, p, 1)]),
  );
  const cells = [690, 691, 692].map((nm) => ({ nm, weights: [0.8, 0.2], sigmasM2 }));
  return readKDistribution(
    toySidecar(LINE_TOY_TEMPERATURES_K, LINE_TOY_PRESSURES_PA, cells.length, {
      species,
      measuredRangeK: [150, 296],
      measuredShapeTo: LINE_TOY_TEMPERATURES_K.map(() => LINE_TOY_PRESSURES_PA.length - 1),
    }),
    [
      toyPart(cells, {
        variants: 1,
        temperatures: LINE_TOY_TEMPERATURES_K.length,
        pressures: LINE_TOY_PRESSURES_PA.length,
      }),
    ],
    `the toy line species as ${species}`,
  );
}

/**
 * A medium of five line slots, the tables' most (R08.T4.d): the toy line species keyed as five
 * gases, each on a layer of its own of the Earth-like column, so that the fifth line slot takes
 * pressure channel 4 and the tables `lastAbsorberPressureColumn` at the transmittance's size; with
 * Rayleigh scattering on the column and the column's pressure profile, under a 5,772 K Planck sun.
 */
export function fiveLineSlotMedium(): MediumWithCurves {
  const column = earthLikeColumn();
  const gases = ["O2", "H2O", "CO", "CH4", "N2O"];
  const { specs } = absorberSpecs(
    gases.map((species, k) => ({
      species,
      columnPerM2: 1e25,
      basePa: 101_325 - k * 15_000,
      topPa: k * 1_000,
    })),
    column,
    [],
    [],
    { kDistributions: gases.map(lineToyTable) },
  );
  const sun = planckBakeSpectrum(SUN_TEFF_K);
  const [rayleigh] = R05_EARTH_REFERENCE.terms;
  if (rayleigh === undefined) {
    throw new Error("R05's Earth has its Rayleigh term");
  }
  return {
    medium: {
      name: "R08.T4.d five line slots",
      topHeightM: 100_000,
      groundAlbedo: [0.2, 0.15, 0.1],
      terms: [
        { ...rayleigh, density: column.density },
        ...specs.map((spec) => absorberTerm(spec, column)),
      ],
      pressureBar: columnPressureBar(column),
    },
    curves: [specs.map((spec) => absorberCurve(spec, sun))],
  };
}

/**
 * A medium with a line absorber on an Earth-like column, under a 5,772 K Planck sun (R08.T4.d's
 * GPU read): a toy line species standing as O₂, well mixed in the whole column, three cells at
 * 690–692 nm each with a strong and a weak g-point, the strong one narrowing as 1 ÷ √p past 1 kPa
 * (`lineCurves.test.ts`'s toy), so that its curve has a row a stored pressure up to the ground and
 * one more; the O₂–O₂ pair it forms, put in the first absorber slot so that the line slot, slot 1,
 * reads pressure channel 0 (`tables.ts`'s `pressureChannelOf`); R05's Rayleigh scattering on the
 * column and R05's aerosol; and the column's pressure profile (`columnPressureBar`), from which the
 * tables carry each line slot's pressure-weighted columns.
 */
export function lineAbsorberMedium(): MediumWithCurves {
  const column = earthLikeColumn();
  const table = lineToyTable("O2");
  const oxygenPerM2 =
    EARTH_OXYGEN_FRACTION *
    column.surfaceNumberDensityPerM3 *
    columnLengthM(column.density, column.topHeightM);
  const { specs } = absorberSpecs(
    [{ species: "O2", columnPerM2: oxygenPerM2, basePa: 101_325, topPa: 0 }],
    column,
    undefined,
    undefined,
    { kDistributions: [table] },
  );
  const sun = planckBakeSpectrum(SUN_TEFF_K);
  const [rayleigh, aerosol] = R05_EARTH_REFERENCE.terms;
  if (rayleigh === undefined || aerosol === undefined) {
    throw new Error("R05's Earth has its Rayleigh and aerosol terms");
  }
  const ordered = specs.toReversed();
  return {
    medium: {
      name: "R08.T4.d a line absorber",
      topHeightM: 100_000,
      groundAlbedo: [0.2, 0.15, 0.1],
      terms: [
        { ...rayleigh, density: column.density },
        aerosol,
        ...ordered.map((spec) => absorberTerm(spec, column)),
      ],
      pressureBar: columnPressureBar(column),
    },
    curves: [ordered.map((spec) => absorberCurve(spec, sun))],
  };
}

/** A medium with the radius of the ground its tables stand on. */
export interface MediumOnRadius {
  readonly medium: AtmosphereMedium;
  /** The tables' ground radius, m. */
  readonly radiusM: number;
}

/** The Venus-class column's radius, m: Venus's 6,051.8 km. */
export const VENUS_CLASS_RADIUS_M = 6_051_800;

/**
 * The Venus-class column of {@link venusClassMedium}: 92 bar of CO₂ on `column.test.ts`'s
 * radiative-convective profile at 8.87 m s⁻².
 */
export function venusClassColumn(): AtmosphereColumn {
  const co2 = gasProperties("CO2");
  return hydrostaticColumn({
    surfacePa: 92e5,
    temperature: {
      kind: "radiativeConvective",
      surfaceK: 730,
      surfacePa: 92e5,
      beta: 0.8 * dryAdiabatExponent([{ properties: co2, moleFraction: 1 }]),
      skinK: 2 ** -0.25 * 226.6,
    },
    meanMolarMassGPerMol: co2.molarMassGPerMol,
    referenceGravityMS2: 8.87,
    referenceRadiusM: VENUS_CLASS_RADIUS_M,
  });
}

/**
 * A Venus-class medium (Design note 8's case), a test's: 92 bar of CO₂ on `column.test.ts`'s
 * radiative-convective profile (T_s 730 K, α 0.8, where Design note 3's refit on c_p(T) has since
 * taken 0.88; the skin at 2^(−1/4) × 226.6 K, the NSSDCA Venus fact sheet's black-body
 * temperature), on its 6,051.8 km radius at 8.87 m s⁻², under R08.T3.c's molecular term, and a
 * cloud deck from 48 to 70 km of vertical optical depth 30 in every channel, single-scattering
 * albedo 0.995 and Cornette–Shanks g = 0.7. Its slant paths' transmittance falls far below
 * rgba16float's least value.
 *
 * @remarks
 * The deck's τ, ω and g are chosen for the test, of Venus's order but not fitted to it; T_s is
 * `column.test.ts`'s 730 K (the NSSDCA fact sheet's mean surface temperature is 737 K).
 */
export function venusClassMedium(): MediumOnRadius {
  const radiusM = VENUS_CLASS_RADIUS_M;
  const column = venusClassColumn();
  const { term: molecular } = molecularTerm(column, [{ species: "CO2", moleFraction: 1 }]);
  const deck = tabulatedDensity(
    Float64Array.of(45_000, 48_000, 70_000, 73_000),
    Float64Array.of(0, 1, 1, 0),
  );
  // The deck's ∫ρ dz: 22 km at 1 and two 3 km ramps at ½.
  const extinctionPerM = 30 / 25_000;
  const albedo = 0.995;
  return {
    radiusM,
    medium: {
      name: "R08.T6.c Venus-class",
      topHeightM: column.topHeightM,
      groundAlbedo: [0.15, 0.15, 0.15],
      terms: [
        molecular,
        {
          name: "cloud",
          density: deck,
          scattering: [albedo * extinctionPerM, albedo * extinctionPerM, albedo * extinctionPerM],
          absorption: [
            (1 - albedo) * extinctionPerM,
            (1 - albedo) * extinctionPerM,
            (1 - albedo) * extinctionPerM,
          ],
          phase: { kind: "cornette-shanks", asymmetry: 0.7 },
        },
      ],
    },
  };
}
