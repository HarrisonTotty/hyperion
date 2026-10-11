import { describe, expect, it } from "vitest";

import bakeSpectra from "../../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import {
  amountValue,
  DOBSON_UNIT_PER_M2,
  EARTH_OXYGEN_FRACTION,
  KM_AMAGAT_PER_M2,
  planckBakeSpectrum,
} from "../../test/atmosphereFixtures";
import { blessMode, expectCommittedTable } from "../../test/bless";
import type { Rgb } from "../photometry/toneCurve";
import {
  ABSORBER_COLUMNS,
  ABSORBER_CROSS_SECTIONS,
  ABSORBER_PAIR_CROSS_SECTIONS,
  ABSORBER_CURVE_DECADES,
  ABSORBER_CURVE_NODES_PER_DECADE,
  ABSORBER_TRANSMITTANCE_FLOOR,
  type AbsorberAmount,
  absorberBinCurves,
  absorberBinTransmittance,
  absorberCurve,
  absorberCurves,
  type AbsorberLayer,
  absorberOptics,
  type AbsorberOptics,
  absorberRankingDepth,
  type AbsorberSpec,
  absorberSpecs,
  absorberTerm,
  absorberTransmittance,
  absorberVerticalAmount,
  channelTransmittance,
  type ColumnAbsorberSpec,
  crossSectionAt,
  type CrossSectionsFile,
  type CrossSectionTable,
  type PairAbsorberSpec,
  pairCrossSectionAt,
  type PairCrossSectionsFile,
  readCrossSections,
  readPairCrossSections,
  scaledAmount,
  type SpeciesCrossSection,
  THOMSON_CROSS_SECTION_M2,
  thomsonTerm,
} from "./absorbers";
import recorded from "./absorbers/curves.json" with { type: "json" };
import { hMinusBoundFreeM2, hMinusFreeFreeM5 } from "./absorbers/hMinus";
import { type AtmosphereColumn, type ColumnInput, hydrostaticColumn } from "./column";
import { EARTH_REFERENCE, OZONE_ABSORPTION_PER_M } from "./earth";
import {
  type AtmosphereMedium,
  BAKE_BIN_COUNT,
  BAKE_BIN_WIDTH_NM,
  BAKE_RANGE_NM,
  columnLengthM,
  densityAt,
  extinction,
} from "./medium";
import { type BakeSpectrum, bakeSpectrumOf, deltaUv } from "./spectralColour";
import { packMedium } from "./tables";
import { transmittanceTwin } from "./tablesCpu";

/** Ozone's reference column: 300 DU, Earth's global mean (WMO/UNEP 2018, Q3). */
const OZONE_COLUMN_PER_M2 = 300 * DOBSON_UNIT_PER_M2;

/**
 * Methane's reference column: 3 km-amagat, about an ice giant's column a bar above its deck and
 * Titan's whole column (2–3; `decision-r08-t4b-t12c.md` §1.3).
 */
const METHANE_COLUMN_PER_M2 = 3 * KM_AMAGAT_PER_M2;

/** The Sun: R06's `solar_colour()`, the 5,772 K main-sequence row of `bake_spectra.json`. */
const THE_SUN: BakeSpectrum = (() => {
  const row = bakeSpectra.suns.find((s) => s.teff_k === 5_772 && s.grid === "MainSequence");
  if (row === undefined) {
    throw new Error("bake_spectra.json holds no 5,772 K main-sequence sun");
  }
  return bakeSpectrumOf(row.bake_spectrum);
})();

const PLANCK_2500 = planckBakeSpectrum(2_500);
const PLANCK_6500 = planckBakeSpectrum(6_500);
const PLANCK_30000 = planckBakeSpectrum(30_000);

/** The suns the record holds, by name. */
const SUNS: ReadonlyArray<{ readonly name: string; readonly spectrum: BakeSpectrum }> = [
  { name: "the Sun (bake_spectra.json, 5772 K, log g 4.438)", spectrum: THE_SUN },
  { name: "Planck 2500 K", spectrum: PLANCK_2500 },
  { name: "Planck 6500 K", spectrum: PLANCK_6500 },
  { name: "Planck 30000 K", spectrum: PLANCK_30000 },
];

/** The suns the plan's colour checks gate: the Sun and a 6,500 K Planck spectrum. */
const GATED_SUNS = new Set(["the Sun (bake_spectra.json, 5772 K, log g 4.438)", "Planck 6500 K"]);

/** A spec of one species at a temperature, laid on no column (its layer is for terms only). */
function singleSpec(
  species: string,
  temperatureK: number,
  columnPerM2: number,
): ColumnAbsorberSpec {
  const crossSection = crossSectionAt(species, temperatureK);
  if (crossSection === undefined) {
    throw new Error(`the client holds no cross-sections for ${species}`);
  }
  return {
    kind: "column",
    name: `absorber:${species}`,
    species: [{ species, columnPerM2, crossSection }],
    columnPerM2,
    basePa: 101_325,
    topPa: 0,
    temperatureK,
  };
}

/** A spec of one molecule pair at a temperature, laid on no column (R08.T4.c). */
function singlePairSpec(
  pair: string,
  temperatureK: number,
  pairColumnPerM5: number,
): PairAbsorberSpec {
  const crossSection = pairCrossSectionAt(pair, temperatureK);
  if (crossSection === undefined) {
    throw new Error(`the client holds no cross-sections for ${pair}`);
  }
  return {
    kind: "pair",
    name: `pair:${pair}`,
    pairs: [{ pair, pairColumnPerM5, crossSection }],
    pairColumnPerM5,
    basePa: 101_325,
    topPa: 0,
    temperatureK,
  };
}

/**
 * SO₂'s reference column: 0.1 km-amagat, about Venus's whole column (150 ppm of a 92-bar CO₂ air,
 * p ÷ (μ m_u g) = 9.2 × 10⁶ Pa ÷ (44 × 1.6605 × 10⁻²⁷ kg × 8.87 m s⁻²) = 1.4 × 10³¹ m⁻², times
 * 1.5 × 10⁻⁴: 2.1 × 10²⁷ m⁻²).
 */
const SULPHUR_DIOXIDE_COLUMN_PER_M2 = 0.1 * KM_AMAGAT_PER_M2;

/** O₂–O₂'s reference pair column: Earth's, 1.2 × 10⁴³ cm⁻⁵ (`decision-r08-t4b-t12c.md`, follow-up 2). */
const OXYGEN_PAIR_COLUMN_PER_M5 = 1.2e53;

/**
 * H₂–He's reference pair column: a hot giant's above 100 bar at 2,000 K, x_H₂ x_He n₀² H ÷ 2 with
 * x_H₂ 0.84, x_He 0.16, n₀ = p ÷ kT = 3.6 × 10²⁶ m⁻³ and H = kT ÷ (2.3 m_u × 20 m s⁻²) = 3.6 × 10⁵ m:
 * 3 × 10⁵⁷ m⁻⁵.
 */
const HYDROGEN_HELIUM_PAIR_COLUMN_PER_M5 = 3e57;

/** H⁻'s reference column: 10²⁰ m⁻², a bound–free optical depth of 0.3 at 550 nm. */
const H_MINUS_COLUMN_PER_M2 = 1e20;

/** The H–e⁻ pair's reference column: 2 × 10⁴⁸ m⁻⁵, a free–free optical depth of 0.3 at 550 nm at 2,500 K. */
const HYDROGEN_ELECTRON_PAIR_COLUMN_PER_M5 = 2e48;

/** The He–e⁻ pair's reference column: 10⁴⁹ m⁻⁵, a free–free optical depth of 0.36 at 600 nm at 2,500 K. */
const HELIUM_ELECTRON_PAIR_COLUMN_PER_M5 = 1e49;

/**
 * O₂–N₂'s reference pair column: Earth's, O₂–O₂'s times x_N₂ ÷ x_O₂ = 0.780 84 ÷ 0.209 476 (US
 * Standard Atmosphere 1976, Table 3): 4.5 × 10⁵³ m⁻⁵.
 */
const OXYGEN_NITROGEN_PAIR_COLUMN_PER_M5 = 4.5e53;

/**
 * H₂–H₂'s reference pair column: Jupiter's above 1 bar, x_H₂² n₀² H ÷ 2 with x_H₂ 0.86, n₀ = p ÷ kT
 * at 1 bar and 165 K = 4.4 × 10²⁵ m⁻³ and H = kT ÷ (2.3 m_u × 24.79 m s⁻²) = 2.4 × 10⁴ m:
 * 1.7 × 10⁵⁵ m⁻⁵.
 */
const HYDROGEN_PAIR_COLUMN_PER_M5 = 1.7e55;

/**
 * The absorbers the record and the colour checks hold, at their reference columns, each with the
 * multiples of it the record covers: ozone to 30 ×, methane to 300 km-amagat
 * (`decision-r08-t4b-t12c.md` §1.3). R08.T4.c's continua and pairs are recorded beside them, with
 * their clamp places (`decision-r08-cmf-licence.md` §5), and not held to T4.b's colour gate.
 */
const ABSORBERS: ReadonlyArray<{
  readonly name: string;
  readonly spec: AbsorberSpec;
  readonly recordedMultiples: readonly number[];
  /** Whether the plan's Δu′v′ gate holds it (T4.b's three) or it is recorded only (T4.c's). */
  readonly gated: boolean;
}> = [
  {
    name: "O3 at 233 K, 300 DU",
    spec: singleSpec("O3", 233, OZONE_COLUMN_PER_M2),
    recordedMultiples: [0.1, 0.3, 1, 3, 10, 30],
    gated: true,
  },
  {
    name: "CH4 at 100 K, 3 km-am",
    spec: singleSpec("CH4", 100, METHANE_COLUMN_PER_M2),
    recordedMultiples: [0.1, 0.3, 1, 3, 10, 30, 100],
    gated: true,
  },
  {
    name: "CH4 at 296 K, 3 km-am",
    spec: singleSpec("CH4", 296, METHANE_COLUMN_PER_M2),
    recordedMultiples: [0.1, 0.3, 1, 3, 10, 30, 100],
    gated: true,
  },
  {
    name: "SO2 at 298 K, 0.1 km-am",
    spec: singleSpec("SO2", 298, SULPHUR_DIOXIDE_COLUMN_PER_M2),
    recordedMultiples: [0.01, 0.1, 1, 10, 100],
    gated: false,
  },
  {
    name: "O2+O2 at 273 K, Earth's 1.2e43 cm-5",
    spec: singlePairSpec("O2+O2", 273, OXYGEN_PAIR_COLUMN_PER_M5),
    recordedMultiples: [1, 10, 100, 1000],
    gated: false,
  },
  {
    name: "H2+He at 2000 K, 3e57 m-5",
    spec: singlePairSpec("H2+He", 2_000, HYDROGEN_HELIUM_PAIR_COLUMN_PER_M5),
    recordedMultiples: [0.01, 0.1, 1, 10, 100],
    gated: false,
  },
  {
    name: "H- bound-free at 2500 K, 1e20 m-2",
    spec: singleSpec("H-", 2_500, H_MINUS_COLUMN_PER_M2),
    recordedMultiples: [0.1, 1, 10, 100],
    gated: false,
  },
  {
    name: "H+e- free-free at 2500 K, 2e48 m-5",
    spec: singlePairSpec("H+e-", 2_500, HYDROGEN_ELECTRON_PAIR_COLUMN_PER_M5),
    recordedMultiples: [0.1, 1, 10, 100],
    gated: false,
  },
  {
    name: "He+e- free-free at 2500 K, 1e49 m-5",
    spec: singlePairSpec("He+e-", 2_500, HELIUM_ELECTRON_PAIR_COLUMN_PER_M5),
    recordedMultiples: [0.1, 1, 10, 100],
    gated: false,
  },
  {
    name: "O2+N2 at 296 K, Earth's 4.5e53 m-5",
    spec: singlePairSpec("O2+N2", 296, OXYGEN_NITROGEN_PAIR_COLUMN_PER_M5),
    recordedMultiples: [1, 10, 100, 1000],
    gated: false,
  },
  {
    name: "O2+CO2 at 296 K, 4.5e53 m-5",
    spec: singlePairSpec("O2+CO2", 296, OXYGEN_NITROGEN_PAIR_COLUMN_PER_M5),
    recordedMultiples: [1, 10, 100, 1000],
    gated: false,
  },
  {
    name: "H2+H2 at 165 K, Jupiter's above 1 bar, 1.7e55 m-5",
    spec: singlePairSpec("H2+H2", 165, HYDROGEN_PAIR_COLUMN_PER_M5),
    recordedMultiples: [1, 10, 100, 1000],
    gated: false,
  },
];

/** The plan's gated span of column multiples: 0.1–10 × the reference, six to the decade. */
const GATED_MULTIPLES = Array.from({ length: 13 }, (_, i) => 10 ** (-1 + i / 6));

/** An absorber of the list by its position, for tests that read one. */
function absorberAt(index: number): AbsorberSpec {
  const entry = ABSORBERS[index];
  if (entry === undefined) {
    throw new Error(`the absorbers list has no entry ${index}`);
  }
  return entry.spec;
}

/** An optics' mixed σ, in its kind's unit: m² a molecule, or m⁵ a molecule pair. */
function sigmaOf(optics: AbsorberOptics): Float64Array {
  return optics.kind === "column" ? optics.crossSectionM2 : optics.crossSectionM5;
}

/** An amount of a spec's kind, its value given in that kind's unit. */
function amountLike(spec: AbsorberSpec, value: number): AbsorberAmount {
  return spec.kind === "column"
    ? { kind: "column", columnPerM2: value }
    : { kind: "pair", pairColumnPerM5: value };
}

/** The signed sum of a channel's light through a column over its star colour, before any clamp. */
function signedRatio(optics: AbsorberOptics, channel: 0 | 1 | 2, u: number): number {
  let passed = 0;
  let total = 0;
  for (const [i, w] of optics.weights[channel].entries()) {
    passed += w * Math.exp(-(sigmaOf(optics)[i] ?? Number.NaN) * u);
    total += w;
  }
  return passed / total;
}

/** The sun's colour and the spectral colour of its light through a column, both on σ's grid. */
function spectralColours(optics: AbsorberOptics, u: number): { star: Rgb; through: Rgb } {
  const star = [0, 0, 0];
  const through = [0, 0, 0];
  for (const [c, weights] of optics.weights.entries()) {
    for (const [i, w] of weights.entries()) {
      star[c] = (star[c] ?? 0) + w;
      through[c] = (through[c] ?? 0) + w * Math.exp(-(sigmaOf(optics)[i] ?? Number.NaN) * u);
    }
  }
  return {
    star: [star[0] ?? 0, star[1] ?? 0, star[2] ?? 0],
    through: [through[0] ?? 0, through[1] ?? 0, through[2] ?? 0],
  };
}

/** The Δu′v′ of the sun's colour times T_c, read from the curve, against the spectral colour. */
function channelColourError(spec: AbsorberSpec, spectrum: BakeSpectrum, multiple: number): number {
  const optics = absorberOptics(spec, spectrum);
  const curve = absorberCurve(spec, spectrum);
  const amount = scaledAmount(absorberVerticalAmount(spec), multiple);
  const { star, through } = spectralColours(optics, amountValue(amount));
  const t = absorberTransmittance(curve, amount);
  return deltaUv([star[0] * t[0], star[1] * t[1], star[2] * t[2]], through);
}

/**
 * The Δu′v′ of the bins' colour against the spectral colour: each bin's curve, read at the column,
 * times the bin's colour Σ c̄ S Δλ over its points, summed over the bins (the bakes' reduction to
 * the channels on storage).
 */
function binColourError(spec: AbsorberSpec, spectrum: BakeSpectrum, multiple: number): number {
  const optics = absorberOptics(spec, spectrum);
  const curves = absorberBinCurves(spec, spectrum);
  const amount = scaledAmount(absorberVerticalAmount(spec), multiple);
  const bins = absorberBinTransmittance(curves, amount);
  const colour = [0, 0, 0];
  for (const [c, weights] of optics.weights.entries()) {
    for (const [i, w] of weights.entries()) {
      colour[c] = (colour[c] ?? 0) + w * (bins[optics.bins[i] ?? 0] ?? Number.NaN);
    }
  }
  const { through } = spectralColours(optics, amountValue(amount));
  return deltaUv([colour[0] ?? 0, colour[1] ?? 0, colour[2] ?? 0], through);
}

/** The largest of a colour error over the plan's gated span. */
function worstOverGatedSpan(
  error: (spec: AbsorberSpec, spectrum: BakeSpectrum, multiple: number) => number,
  spec: AbsorberSpec,
  spectrum: BakeSpectrum,
): number {
  return Math.max(...GATED_MULTIPLES.map((m) => error(spec, spectrum, m)));
}

/** A value rounded to 10⁻⁶. */
function roundMicro(value: number): number {
  return Math.round(value * 1e6) / 1e6;
}

/** A value rounded up to 10⁻⁶, so that the record is an upper bound robust to the last bits. */
function ceilMicro(value: number): number {
  return Math.ceil(value * 1e6) / 1e6;
}

/** A node's column as its decade about the reference, log₁₀(u ÷ U), to 10⁻⁶. */
function nodeDecade(k: number): number {
  return roundMicro(ABSORBER_CURVE_DECADES[0] + k / ABSORBER_CURVE_NODES_PER_DECADE);
}

/** A channel's clamp record without its own gamut exit, which the curve's record holds once. */
function withoutExit(record: {
  readonly gamutExit: number | null;
  readonly aboveOne: number | null;
  readonly atFloor: number | null;
  readonly atFloorByDepth: number | null;
  readonly runningMinimumFrom: number | null;
  readonly runningMinimum: number;
}) {
  return {
    aboveOne: record.aboveOne,
    atFloor: record.atFloor,
    atFloorByDepth: record.atFloorByDepth,
    runningMinimumFrom: record.runningMinimumFrom,
    runningMinimum: record.runningMinimum,
  };
}

/**
 * A curve's node columns in the spec's unit, as `absorbers.ts` makes them: U × 10^(first + k ÷ N).
 */
function nodeColumn(spec: AbsorberSpec, k: number): number {
  return (
    amountValue(absorberVerticalAmount(spec)) *
    10 ** (ABSORBER_CURVE_DECADES[0] + k / ABSORBER_CURVE_NODES_PER_DECADE)
  );
}

/**
 * Where a curve leaves Rec. 709 and where its clamp and running minimum act, as node decades
 * about the reference: the first node at which some channel's signed sum is negative (the gamut
 * exit), and per channel the first node above 1, the first at the floor, the first and the number
 * of nodes
 * the running minimum lowered. `null` where it never happens on the grid.
 */
function clampRecord(spec: AbsorberSpec, spectrum: BakeSpectrum) {
  const optics = absorberOptics(spec, spectrum);
  const curve = absorberCurve(spec, spectrum);
  const channel = (c: 0 | 1 | 2) => {
    let gamutExit: number | null = null;
    let aboveOne: number | null = null;
    let atFloor: number | null = null;
    let atFloorByDepth: number | null = null;
    let runningMinimumFrom: number | null = null;
    let runningMinimum = 0;
    for (const k of curve.logColumns.keys()) {
      const ratio = signedRatio(optics, c, nodeColumn(spec, k));
      gamutExit ??= ratio < 0 ? nodeDecade(k) : null;
      aboveOne ??= ratio > 1 ? nodeDecade(k) : null;
      atFloor ??= ratio < ABSORBER_TRANSMITTANCE_FLOOR ? nodeDecade(k) : null;
      atFloorByDepth ??= ratio >= 0 && ratio < ABSORBER_TRANSMITTANCE_FLOOR ? nodeDecade(k) : null;
      const clamped = Math.min(Math.max(ratio, ABSORBER_TRANSMITTANCE_FLOOR), 1);
      // The node's own column, and a tolerance past the sums' rounding: lowered, not re-rounded.
      const lowered = (curve.transmittance[c][k] ?? Number.NaN) < clamped * (1 - 1e-9);
      runningMinimumFrom ??= lowered ? nodeDecade(k) : null;
      runningMinimum += lowered ? 1 : 0;
    }
    return { gamutExit, aboveOne, atFloor, atFloorByDepth, runningMinimumFrom, runningMinimum };
  };
  const [red, green, blue] = [channel(0), channel(1), channel(2)];
  const exits = [red.gamutExit, green.gamutExit, blue.gamutExit].flatMap((e) =>
    e === null ? [] : [e],
  );
  return {
    gamutExit: exits.length > 0 ? Math.min(...exits) : null,
    red: withoutExit(red),
    green: withoutExit(green),
    blue: withoutExit(blue),
  };
}

/**
 * `curves.json`'s table: each absorber's optical depths under each sun, its channels' and bins'
 * colour errors, and where it leaves the gamut and the clamp acts.
 */
function curvesTable() {
  return {
    description:
      "Curves of growth of the client's absorbers (rendering plan R08, Design note 5; R08.T4.b " +
      "and R08.T4.c; " +
      "decision-r08-t4b-t12c.md), written by absorbers.test.ts under HYPERION_BLESS=1 and " +
      "checked unchanged otherwise. Per absorber and sun, at each of its recordedMultiples of the " +
      "reference column: -ln T_c per channel (r, g, b) read back from the sun's curve (clamps and " +
      "running minimum included), rounded to 1e-6; the CIE 1976 delta " +
      "u'v' between the three-channel colour (the sun's colour times T_c) and the spectral one, " +
      "and between the 15 bins' colour and the spectral one, rounded up to 1e-6. Then, as " +
      "decades log10(u/U) of the curve's nodes, where the light leaves Rec. 709 (gamutExit) and " +
      "per channel where the signed sum passes 1, where it reaches the 1e-6 floor, where it " +
      "reaches the floor by depth alone (the sum still at least 0), and where the running " +
      "minimum first lowers a node and how many it lowers; null where it never happens on the " +
      "grid. R08.T4.c's continua and pairs (SO2, O2+O2, H2+He, H-, H+e-, He+e-, O2+N2, O2+CO2 " +
      "and H2+H2) are recorded beside T4.b's ozone and methane; a pair's reference column is in " +
      "molecule pairs m-5.",
    absorbers: ABSORBERS.map(({ name, spec, recordedMultiples }) => ({
      name,
      ...(spec.kind === "column"
        ? { referenceColumnPerM2: spec.columnPerM2 }
        : { referenceColumnPerM5: spec.pairColumnPerM5 }),
      recordedMultiples,
      suns: SUNS.map((sun) => {
        const curve = absorberCurve(spec, sun.spectrum);
        return {
          sun: sun.name,
          opticalDepth: recordedMultiples.map((m) =>
            absorberTransmittance(curve, scaledAmount(absorberVerticalAmount(spec), m)).map((t) =>
              roundMicro(-Math.log(t)),
            ),
          ),
          deltaUv: recordedMultiples.map((m) =>
            ceilMicro(channelColourError(spec, sun.spectrum, m)),
          ),
          binDeltaUv: recordedMultiples.map((m) =>
            ceilMicro(binColourError(spec, sun.spectrum, m)),
          ),
          ...clampRecord(spec, sun.spectrum),
        };
      }),
    })),
  };
}

/** A cross-section flat over the bake range at 1 nm. */
function flatCrossSection(species: string, sigmaM2: number): SpeciesCrossSection {
  return {
    species,
    provenance: "measured",
    temperatureK: 250,
    firstNm: BAKE_RANGE_NM[0],
    stepNm: 1,
    crossSectionsM2: new Float64Array(BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0] + 1).fill(sigmaM2),
    source: "a test's flat cross-section",
  };
}

/** A spec of one test species of a column with a cross-section. */
function testSpec(crossSection: SpeciesCrossSection, columnPerM2: number): ColumnAbsorberSpec {
  return {
    kind: "column",
    name: `absorber:${crossSection.species}`,
    species: [{ species: crossSection.species, columnPerM2, crossSection }],
    columnPerM2,
    basePa: 100_000,
    topPa: 0,
    temperatureK: 250,
  };
}

/** Earth's column for the layers' tests: Design note 3's Earth, 288 K, 1 bar, β 0.171, 214.4 K. */
const EARTH_COLUMN_INPUT: ColumnInput = {
  surfacePa: 100_000,
  temperature: {
    kind: "radiativeConvective",
    surfaceK: 288,
    surfacePa: 100_000,
    beta: 0.171,
    skinK: 214.4,
  },
  meanMolarMassGPerMol: 28.97,
  referenceGravityMS2: 9.806,
  referenceRadiusM: 6_371_000,
};
const EARTH_COLUMN = hydrostaticColumn(EARTH_COLUMN_INPUT);

/** The same air isothermal at 250 K. */
const ISOTHERMAL_COLUMN = hydrostaticColumn({
  ...EARTH_COLUMN_INPUT,
  temperature: { kind: "isothermal", temperatureK: 250 },
});

/** The column's top pressure, Pa. */
const TOP_PA = EARTH_COLUMN.pressuresPa[EARTH_COLUMN.pressuresPa.length - 1] ?? Number.NaN;

describe("a curve of growth", () => {
  it("is e^(−σu) in every channel for a flat cross-section, under any sun", () => {
    const sigma = 3e-25;
    const spec = testSpec(flatCrossSection("flat", sigma), 1e24);
    for (const sun of SUNS) {
      const curve = absorberCurve(spec, sun.spectrum);
      for (const [k, x] of curve.logColumns.entries()) {
        const expected = Math.max(sigma * Math.exp(x), 0);
        for (const channel of curve.opticalDepth) {
          const depth = channel[k] ?? Number.NaN;
          // Past the floor's 13.8 the curve holds the floor.
          expect(
            Math.abs(depth - Math.min(expected, -Math.log(ABSORBER_TRANSMITTANCE_FLOOR))),
          ).toBeLessThan(1e-12 * Math.max(expected, 1));
        }
      }
    }
  });

  it("spans 10⁻³ to 10³ times the absorber's column, evenly in ln u", () => {
    const spec = absorberAt(0);
    const curve = absorberCurve(spec, THE_SUN);
    const [first, last] = ABSORBER_CURVE_DECADES;
    expect(curve.logColumns.length).toBe((last - first) * ABSORBER_CURVE_NODES_PER_DECADE + 1);
    expect(curve.logColumns[0]).toBeCloseTo(
      Math.log(amountValue(absorberVerticalAmount(spec)) * 10 ** first),
      12,
    );
    expect(curve.logColumns.at(-1)).toBeCloseTo(
      Math.log(amountValue(absorberVerticalAmount(spec)) * 10 ** last),
      12,
    );
  });

  it.each([
    ["2,500 K", PLANCK_2500],
    ["30,000 K", PLANCK_30000],
  ])(
    "keeps every T_c in [10⁻⁶, 1] and non-increasing in u under a %s Planck sun",
    (_, spectrum) => {
      for (const { spec } of ABSORBERS) {
        const curve = absorberCurve(spec, spectrum);
        for (const channel of curve.transmittance) {
          for (const [k, t] of channel.entries()) {
            expect(t).toBeGreaterThanOrEqual(ABSORBER_TRANSMITTANCE_FLOOR);
            expect(t).toBeLessThanOrEqual(1);
            expect(t).toBeLessThanOrEqual(channel[k - 1] ?? 1);
          }
        }
      }
    },
  );

  it("clamps blue at 1 under a 2,500 K sun, where the signed sum passes it", () => {
    // Rec. 709's b̄ is negative over 500–620 nm, which holds ozone's Chappuis band.
    const spec = absorberAt(0);
    const optics = absorberOptics(spec, PLANCK_2500);
    const amount = scaledAmount(absorberVerticalAmount(spec), 10);
    expect(signedRatio(optics, 2, amountValue(amount))).toBeGreaterThan(1.1);
    expect(channelTransmittance(optics, amount)[2]).toBe(1);
  });

  it("clamps methane's red at the floor once its light leaves Rec. 709", () => {
    const spec = absorberAt(2);
    const optics = absorberOptics(spec, THE_SUN);
    const amount = scaledAmount(absorberVerticalAmount(spec), 100);
    expect(signedRatio(optics, 0, amountValue(amount))).toBeLessThan(0);
    expect(channelTransmittance(optics, amount)[0]).toBe(ABSORBER_TRANSMITTANCE_FLOOR);
  });

  it("reaches the floor only past the gamut exit for ozone and methane: every node at the floor has a negative sum", () => {
    // decision-r08-cmf-licence.md §5: neither gas reaches the floor by depth alone. R08.T4.c's
    // continua can, and their records say where (atFloorByDepth), so the claim is scoped to T4.b's.
    const [first, last] = ABSORBER_CURVE_DECADES;
    const nodes = (last - first) * ABSORBER_CURVE_NODES_PER_DECADE + 1;
    let floored = 0;
    for (const { spec } of ABSORBERS.filter((a) => a.gated)) {
      for (const sun of SUNS) {
        const optics = absorberOptics(spec, sun.spectrum);
        for (let k = 0; k < nodes; k += 1) {
          for (const c of [0, 1, 2] as const) {
            const ratio = signedRatio(optics, c, nodeColumn(spec, k));
            const atFloor = ratio < ABSORBER_TRANSMITTANCE_FLOOR;
            floored += atFloor ? 1 : 0;
            expect(atFloor ? ratio : -1).toBeLessThan(0);
          }
        }
      }
    }
    expect(floored).toBeGreaterThan(0);
  });

  it("weighs a channel by max(c̄, 0) where the sun's own colour in it is not positive", () => {
    // A Planck star's Rec. 709 blue is negative below about 1,900 K (decision-r08-t4b-t12c.md).
    const cool = planckBakeSpectrum(1_500);
    const spec = absorberAt(0);
    const optics = absorberOptics(spec, cool);
    expect(optics.weights[2].reduce((sum, w) => sum + w, 0)).toBeLessThanOrEqual(0);
    const amount = scaledAmount(absorberVerticalAmount(spec), 10);
    let passed = 0;
    let total = 0;
    for (const [i, w] of optics.weights[2].entries()) {
      const clipped = Math.max(w, 0);
      passed += clipped * Math.exp(-(sigmaOf(optics)[i] ?? Number.NaN) * amountValue(amount));
      total += clipped;
    }
    expect(channelTransmittance(optics, amount)[2]).toBeCloseTo(passed / total, 12);
  });

  it("is the same for the same sun", () => {
    for (const { spec } of ABSORBERS) {
      expect(absorberCurve(spec, PLANCK_2500)).toEqual(absorberCurve(spec, PLANCK_2500));
    }
  });

  it("differs between a 2,500 K and a 30,000 K sun in every channel", () => {
    for (const { spec } of ABSORBERS.filter((a) => a.gated)) {
      const cool = absorberCurve(spec, PLANCK_2500);
      const hot = absorberCurve(spec, PLANCK_30000);
      for (let c = 0; c < 3; c += 1) {
        const a = cool.opticalDepth[c] ?? new Float64Array(0);
        const b = hot.opticalDepth[c] ?? new Float64Array(0);
        const differences = Array.from(a, (value, k) => Math.abs(value - (b[k] ?? Number.NaN)));
        expect(Math.max(...differences)).toBeGreaterThan(1e-3);
      }
    }
  });

  it("reads back between its nodes within 5 × 10⁻⁴ in T where neither node is clamped", () => {
    const floorDepth = -Math.log(ABSORBER_TRANSMITTANCE_FLOOR);
    let compared = 0;
    for (const { spec } of ABSORBERS) {
      for (const sun of SUNS) {
        const optics = absorberOptics(spec, sun.spectrum);
        const curve = absorberCurve(spec, sun.spectrum);
        // Each node's exact transmittance, once: an interval reads both of its ends.
        const atNodes: ReadonlyArray<Rgb> = Array.from(curve.logColumns, (logColumn) =>
          channelTransmittance(optics, amountLike(spec, Math.exp(logColumn))),
        );
        const atNode = (j: number): Rgb => {
          const node = atNodes[j];
          if (node === undefined) {
            throw new Error(`no node ${String(j)}`);
          }
          return node;
        };
        for (let k = 0; k + 1 < curve.logColumns.length; k += 1) {
          const u = amountLike(
            spec,
            Math.exp(0.5 * ((curve.logColumns[k] ?? 0) + (curve.logColumns[k + 1] ?? 0))),
          );
          const exact = channelTransmittance(optics, u);
          const read = absorberTransmittance(curve, u);
          const [here, next] = [atNode(k), atNode(k + 1)];
          for (const c of [0, 1, 2] as const) {
            // Above the floor at both nodes, and neither lowered by the running minimum.
            const unclamped =
              (curve.opticalDepth[c][k + 1] ?? Number.NaN) < floorDepth &&
              curve.transmittance[c][k] === here[c] &&
              curve.transmittance[c][k + 1] === next[c];
            compared += unclamped ? 1 : 0;
            expect(unclamped ? Math.abs(read[c] - exact[c]) : 0).toBeLessThan(5e-4);
          }
        }
      }
    }
    expect(compared).toBeGreaterThan(4_000);
  });

  it("reads the linear regime below its first node", () => {
    const spec = absorberAt(0);
    const optics = absorberOptics(spec, THE_SUN);
    const curve = absorberCurve(spec, THE_SUN);
    const u = scaledAmount(absorberVerticalAmount(spec), 1e-5);
    const exact = channelTransmittance(optics, u);
    const read = absorberTransmittance(curve, u);
    // Red and green; blue is clamped at 1 there under the Sun.
    expect(
      [0, 1].map((c) => Math.abs(Math.log(read[c] ?? 1) / Math.log(exact[c] ?? 1) - 1)),
    ).toEqual([expect.closeTo(0, 3), expect.closeTo(0, 3)]);
  });

  it("reads nothing at no column", () => {
    expect(
      absorberTransmittance(absorberCurve(absorberAt(0), THE_SUN), {
        kind: "column",
        columnPerM2: 0,
      }),
    ).toEqual([1, 1, 1]);
  });

  it("refuses a negative column", () => {
    expect(() =>
      absorberTransmittance(absorberCurve(absorberAt(0), THE_SUN), {
        kind: "column",
        columnPerM2: -1,
      }),
    ).toThrow(RangeError);
  });

  it("refuses a pair column on a column absorber's curve", () => {
    expect(() =>
      absorberTransmittance(absorberCurve(absorberAt(0), THE_SUN), {
        kind: "pair",
        pairColumnPerM5: 1e40,
      }),
    ).toThrow(/pair amount is read on a column absorber's/u);
  });

  it("refuses a pair amount on a column absorber's optics", () => {
    expect(() =>
      channelTransmittance(absorberOptics(absorberAt(0), THE_SUN), {
        kind: "pair",
        pairColumnPerM5: 1e40,
      }),
    ).toThrow(/pair amount is read on a column absorber's/u);
  });

  it("refuses a column amount on a pair absorber's bin curves", () => {
    const pair = ABSORBERS.find((a) => a.spec.kind === "pair")?.spec;
    if (pair === undefined) {
      throw new Error("the absorbers list holds a pair");
    }
    expect(() =>
      absorberBinTransmittance(absorberBinCurves(pair, THE_SUN), {
        kind: "column",
        columnPerM2: 1e20,
      }),
    ).toThrow(/column amount is read on a pair absorber's/u);
  });

  it("names its absorber's kind", () => {
    const pair = ABSORBERS.find((a) => a.spec.kind === "pair")?.spec;
    if (pair === undefined) {
      throw new Error("the absorbers list holds a pair");
    }
    expect([absorberCurve(absorberAt(0), THE_SUN).kind, absorberCurve(pair, THE_SUN).kind]).toEqual(
      ["column", "pair"],
    );
  });

  it.each([
    ["the Sun", THE_SUN],
    ["a 6,500 K Planck sun", PLANCK_6500],
  ])(
    "gives %s's colour through 0.1–10 × each reference column within 0.002 in u′v′ of the spectral colour",
    (_, spectrum) => {
      // T4.b's gate; T4.c's continua and pairs are recorded, not gated (curves.json).
      for (const { spec } of ABSORBERS.filter((a) => a.gated)) {
        expect(worstOverGatedSpan(channelColourError, spec, spectrum)).toBeLessThanOrEqual(0.002);
      }
    },
  );

  it("gives a colour exactly the spectral one for a flat cross-section", () => {
    const spec = testSpec(flatCrossSection("flat", 3e-25), 1e24);
    for (const sun of SUNS) {
      expect(worstOverGatedSpan(channelColourError, spec, sun.spectrum)).toBeLessThan(1e-12);
    }
  });

  it("gives 300 DU of ozone a green Chappuis optical depth of 0.02–0.04 under the Sun", () => {
    const spec = absorberAt(0);
    const [, green] = channelTransmittance(absorberOptics(spec, THE_SUN), {
      kind: "column",
      columnPerM2: OZONE_COLUMN_PER_M2,
    });
    expect(-Math.log(green)).toBeGreaterThanOrEqual(0.02);
    expect(-Math.log(green)).toBeLessThanOrEqual(0.04);
  });

  it("holds the floor where every point a channel weighs is saturated", () => {
    const spec = testSpec(flatCrossSection("flat", 1e-25), 1e28);
    const t = channelTransmittance(absorberOptics(spec, THE_SUN), {
      kind: "column",
      columnPerM2: 1e28,
    });
    expect(t).toEqual([
      ABSORBER_TRANSMITTANCE_FLOOR,
      ABSORBER_TRANSMITTANCE_FLOOR,
      ABSORBER_TRANSMITTANCE_FLOOR,
    ]);
  });

  it("refuses cross-sections that do not cover the bake range", () => {
    const short = flatCrossSection("short", 1e-25);
    const spec = testSpec(
      { ...short, crossSectionsM2: short.crossSectionsM2.subarray(0, 300) },
      1e24,
    );
    expect(() => absorberOptics(spec, THE_SUN)).toThrow(/bake range/u);
  });

  it("refuses cross-sections that are not finite", () => {
    const flat = flatCrossSection("nan", 1e-25);
    const spec = testSpec(
      { ...flat, crossSectionsM2: flat.crossSectionsM2.map((v, i) => (i === 7 ? Number.NaN : v)) },
      1e24,
    );
    expect(() => absorberOptics(spec, THE_SUN)).toThrow(/not finite/u);
  });

  it("refuses a sun with no light", () => {
    const spec = absorberAt(0);
    expect(() => absorberCurve(spec, bakeSpectrumOf(Array.from({ length: 15 }, () => 0)))).toThrow(
      RangeError,
    );
  });
});

describe("the bake bins' curves", () => {
  it("are each bin's mean of e^(−σu) under a flat spectrum", () => {
    const flat = bakeSpectrumOf(Array.from({ length: 15 }, () => 1));
    const spec = absorberAt(1);
    const optics = absorberOptics(spec, flat);
    const curves = absorberBinCurves(spec, flat);
    expect(curves.transmittance).toHaveLength(BAKE_BIN_COUNT);
    for (const k of [0, 120, 200]) {
      const u = Math.exp(curves.logColumns[k] ?? Number.NaN);
      for (let b = 0; b < BAKE_BIN_COUNT; b += 1) {
        let passed = 0;
        let total = 0;
        for (const [i, s] of optics.light.entries()) {
          if (optics.bins[i] === b) {
            passed += s * Math.exp(-(sigmaOf(optics)[i] ?? Number.NaN) * u);
            total += s;
          }
        }
        expect(curves.transmittance[b]?.[k]).toBeCloseTo(passed / total, 12);
      }
    }
  });

  it("refuse a sun with no light in a bin", () => {
    // S is linear between the bins' centres, so bin 7 is dark only with its neighbours.
    const dark = bakeSpectrumOf(Array.from({ length: 15 }, (_, b) => (b >= 6 && b <= 8 ? 0 : 1)));
    expect(() => absorberBinCurves(absorberAt(0), dark)).toThrow(/bake bin/u);
  });

  it("come with the channel curve from one pass, the same as each alone", () => {
    const spec = absorberAt(1);
    expect(absorberCurves(spec, THE_SUN)).toEqual({
      channels: absorberCurve(spec, THE_SUN),
      bins: absorberBinCurves(spec, THE_SUN),
    });
  });

  it("put each grid point in its 25.33 nm bin, 760 nm in the last", () => {
    const optics = absorberOptics(absorberAt(1), THE_SUN);
    for (const [i, nm] of optics.wavelengthsNm.entries()) {
      const expected = Math.min(Math.floor((nm - 380) / BAKE_BIN_WIDTH_NM), BAKE_BIN_COUNT - 1);
      expect(optics.bins[i]).toBe(expected);
    }
  });

  it.each([
    ["the Sun", THE_SUN],
    ["a 6,500 K Planck sun", PLANCK_6500],
  ])(
    "give %s's colour through 0.1–10 × each reference column within 0.003 in u′v′, reduced on storage",
    (_, spectrum) => {
      for (const { spec } of ABSORBERS.filter((a) => a.gated)) {
        expect(worstOverGatedSpan(binColourError, spec, spectrum)).toBeLessThanOrEqual(0.003);
      }
    },
  );

  it("stay true means, in (0, 1] and non-increasing, however deep", () => {
    for (const { spec } of ABSORBERS) {
      const curves = absorberBinCurves(spec, PLANCK_2500);
      for (const bin of curves.opticalDepth) {
        for (const [k, depth] of bin.entries()) {
          expect(Number.isFinite(depth) && depth >= 0).toBe(true);
          expect(depth).toBeGreaterThanOrEqual(bin[k - 1] ?? 0);
        }
      }
    }
  });
});

describe("the committed curves", () => {
  const table = curvesTable();

  it("are written to absorbers/curves.json under a bless, and checked unchanged otherwise", async () => {
    // Checks that must stop a bless run first: vitest writes the file whatever fails later. The
    // colour gate holds T4.b's absorbers; T4.c's are recorded only.
    for (const absorber of table.absorbers.filter((_, i) => ABSORBERS[i]?.gated === true)) {
      const gated = absorber.recordedMultiples.map((m) => m <= 10);
      for (const sun of absorber.suns.filter((s) => GATED_SUNS.has(s.sun))) {
        expect(Math.max(...sun.deltaUv.filter((_, i) => gated[i] === true))).toBeLessThanOrEqual(
          0.002,
        );
        expect(Math.max(...sun.binDeltaUv.filter((_, i) => gated[i] === true))).toBeLessThanOrEqual(
          0.003,
        );
      }
    }
    await expect(
      expectCommittedTable(table, recorded, "./absorbers/curves.json"),
    ).resolves.toBeUndefined();
  });

  // Under a bless the changed table would be written, so this runs only when comparing.
  it.runIf(blessMode() === "compare")("fail on a changed table, naming the command", async () => {
    const changed = { ...table, description: `${table.description} Changed.` };
    await expect(
      expectCommittedTable(changed, recorded, "./absorbers/curves.json"),
    ).rejects.toThrow(
      /HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run src\/renderer\/src\/view\/atmosphere\/absorbers\.test\.ts/u,
    );
  });
});

/** A layer from the datum to the top, as a well-mixed gas's is. */
function mixed(species: string, columnPerM2: number): AbsorberLayer {
  return { species, columnPerM2, basePa: 1e6, topPa: 0 };
}

/** Six test species. */
const SIX = ["A", "B", "C", "D", "E", "F"] as const;

/** Flat cross-sections of 10⁻²⁶ m² for test species, the same at every temperature. */
function flatTables(species: ReadonlyArray<string>): CrossSectionTable[] {
  const flat = new Float64Array(BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0] + 1).fill(1e-26);
  return species.map((key) => ({
    species: key,
    source: "a test's flat cross-section",
    firstNm: BAKE_RANGE_NM[0],
    stepNm: 1,
    temperaturesK: [100, 300],
    temperatureLaw: "linear",
    measuredRangeK: [100, 300],
    crossSectionsM2: [flat, flat],
  }));
}

/** Six layers of {@link SIX}, each on a profile of its own, of the columns given. */
function sixProfiles(columnOf: (species: string) => number): AbsorberLayer[] {
  return SIX.map((species, i) => ({
    species,
    columnPerM2: columnOf(species),
    basePa: 100_000,
    topPa: 1_000 * (i + 1),
  }));
}

describe("absorbers on one profile", () => {
  it("give one curve, equal to the curve of their mixed cross-section", () => {
    const ozone = 1e23;
    const methane = 1e27;
    const { specs, approximations } = absorberSpecs(
      [mixed("O3", ozone), mixed("CH4", methane)],
      ISOTHERMAL_COLUMN,
    );
    expect(approximations).toEqual([]);
    expect(specs.map((s) => s.name)).toEqual(["absorber:O3+CH4"]);
    const [spec] = specs;
    if (spec?.kind !== "column") {
      throw new Error("two absorbers on one profile gave no column spec");
    }
    const o3 = crossSectionAt("O3", 250);
    const ch4 = crossSectionAt("CH4", 250);
    if (o3 === undefined || ch4 === undefined) {
      throw new Error("the client holds ozone's and methane's cross-sections");
    }
    // The mixture on methane's 0.25 nm grid, ozone's 1 nm bins interpolated between their centres.
    const total = ozone + methane;
    const count = (BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0]) / 0.25 + 1;
    const offset = (BAKE_RANGE_NM[0] - ch4.firstNm) / ch4.stepNm;
    const mixedSigma = Float64Array.from({ length: count }, (_, i) => {
      const x = i / 4;
      const k = Math.floor(x);
      const t = x - k;
      const o = (o3.crossSectionsM2[k] ?? 0) * (1 - t) + (o3.crossSectionsM2[k + 1] ?? 0) * t;
      return (ozone / total) * o + (methane / total) * (ch4.crossSectionsM2[offset + i] ?? 0);
    });
    const single: ColumnAbsorberSpec = {
      ...spec,
      name: "absorber:mixed",
      species: [
        {
          species: "mixed",
          columnPerM2: total,
          crossSection: {
            ...ch4,
            species: "mixed",
            firstNm: BAKE_RANGE_NM[0],
            crossSectionsM2: mixedSigma,
          },
        },
      ],
    };
    for (const sun of SUNS) {
      const merged = absorberCurve(spec, sun.spectrum);
      const reference = absorberCurve(single, sun.spectrum);
      expect(merged.logColumns).toEqual(reference.logColumns);
      for (let c = 0; c < 3; c += 1) {
        const a = merged.opticalDepth[c] ?? new Float64Array(0);
        const b = reference.opticalDepth[c] ?? new Float64Array(0);
        for (const [k, value] of a.entries()) {
          const expected = b[k] ?? Number.NaN;
          expect(Math.abs(value - expected)).toBeLessThanOrEqual(1e-12 * Math.max(expected, 1));
        }
      }
    }
  });

  it("give a profile none of whose species the client holds no column, and report each", () => {
    const layers: AbsorberLayer[] = [
      { species: "O3", columnPerM2: 8e22, basePa: 2_000, topPa: 500 },
      { species: "H2O", columnPerM2: 1e27, basePa: 100_000, topPa: 20_000 },
      mixed("CH4", 1e25),
      mixed("NH3", 1e24),
    ];
    const { specs, approximations } = absorberSpecs(layers, EARTH_COLUMN);
    expect(specs.map((s) => s.name)).toEqual(["absorber:O3", "absorber:CH4+NH3"]);
    expect(approximations).toEqual([
      { species: "H2O", provenance: "none", reason: "noCrossSections" },
      { species: "NH3", provenance: "none", reason: "noCrossSections" },
    ]);
  });

  it("fit six absorbers on five profiles into the five columns", () => {
    const layers = SIX.map((species, i): AbsorberLayer => ({
      species,
      columnPerM2: 1e24,
      basePa: 100_000,
      topPa: species === "F" ? 1_000 : 1_000 * (i + 1),
    }));
    const { specs, approximations } = absorberSpecs(layers, EARTH_COLUMN, flatTables(SIX));
    expect([specs.map((s) => s.name), approximations]).toEqual([
      ["absorber:A+F", "absorber:B", "absorber:C", "absorber:D", "absorber:E"],
      [],
    ]);
  });

  it.each([
    ["first", "A", ["B", "C", "D", "E", "F"]],
    ["sixth", "F", ["A", "B", "C", "D", "E"]],
  ])(
    "keep the five profiles of greatest optical depth when the least is listed %s",
    (_, least, kept) => {
      const { specs, approximations } = absorberSpecs(
        sixProfiles((species) => (species === least ? 1e23 : 1e24)),
        EARTH_COLUMN,
        flatTables(SIX),
      );
      expect([specs.map((s) => s.name), approximations]).toEqual([
        kept.map((species) => `absorber:${species}`),
        [{ species: least, provenance: "none", reason: "noColumnFree" }],
      ]);
    },
  );

  it("keep the earlier listed of two profiles of equal depth", () => {
    const { approximations } = absorberSpecs(
      sixProfiles(() => 1e24),
      EARTH_COLUMN,
      flatTables(SIX),
    );
    expect(approximations).toEqual([{ species: "F", provenance: "none", reason: "noColumnFree" }]);
  });

  it("never take more than the stored columns", () => {
    const { specs } = absorberSpecs(
      sixProfiles(() => 1e24),
      EARTH_COLUMN,
      flatTables(SIX),
    );
    expect(specs).toHaveLength(ABSORBER_COLUMNS);
  });

  it("give the kept profiles their columns in the order listed, not by depth", () => {
    const columns: Readonly<Record<string, number>> = {
      A: 3e24,
      B: 1e24,
      C: 5e24,
      D: 1e23,
      E: 2e24,
      F: 4e24,
    };
    const { specs } = absorberSpecs(
      sixProfiles((species) => columns[species] ?? Number.NaN),
      EARTH_COLUMN,
      flatTables(SIX),
    );
    expect(specs.map((s) => s.name)).toEqual([
      "absorber:A",
      "absorber:B",
      "absorber:C",
      "absorber:E",
      "absorber:F",
    ]);
  });

  it("report every species of a dropped profile noColumnFree, with or without cross-sections", () => {
    const layers = sixProfiles((species) => (species === "F" ? 1e23 : 1e24));
    layers.push({ species: "G", columnPerM2: 1e24, basePa: 100_000, topPa: 6_000 });
    const { approximations } = absorberSpecs(layers, EARTH_COLUMN, flatTables(SIX));
    expect(approximations).toEqual([
      { species: "F", provenance: "none", reason: "noColumnFree" },
      { species: "G", provenance: "none", reason: "noColumnFree" },
    ]);
  });

  it("rank by clipped weights, the deepest channel's depth, where the signed would differ", () => {
    // σ only over 470–550 nm, r̄'s negative lobe: the clipped red weighs none of it.
    const sigma = new Float64Array(BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0] + 1).map((_, i) =>
      i + BAKE_RANGE_NM[0] >= 470 && i + BAKE_RANGE_NM[0] <= 550 ? 1e-24 : 0,
    );
    const crossSection: SpeciesCrossSection = {
      ...flatCrossSection("lobe", 0),
      crossSectionsM2: sigma,
    };
    const spec = testSpec(crossSection, 1e24);
    const optics = absorberOptics(spec, bakeSpectrumOf(Array.from({ length: 15 }, () => 1)));
    const clippedDepth = (channel: Float64Array): number => {
      let passed = 0;
      let total = 0;
      for (const [i, w] of channel.entries()) {
        const clipped = Math.max(w, 0);
        passed += clipped * Math.exp(-(sigmaOf(optics)[i] ?? Number.NaN) * 1e24);
        total += clipped;
      }
      return -Math.log(passed / total);
    };
    const [red, green, blue] = optics.weights.map(clippedDepth);
    const signedRed = -Math.log(signedRatio(optics, 0, 1e24));
    expect([
      absorberRankingDepth(spec),
      Math.max(red ?? 0, green ?? 0, blue ?? 0) === green,
      signedRed < 0,
    ]).toEqual([expect.closeTo(green ?? Number.NaN, 12), true, true]);
  });

  it("rank a profile by its clipped-weight depth, σU for a flat cross-section", () => {
    const [spec] = absorberSpecs(
      sixProfiles(() => 1e24).slice(0, 1),
      EARTH_COLUMN,
      flatTables(SIX),
    ).specs;
    expect(spec === undefined ? Number.NaN : absorberRankingDepth(spec)).toBeCloseTo(0.01, 12);
  });
});

/** σ over the bake range at 1 nm: one value at point 100 (480 nm), another everywhere else. */
function pointAt100(atPoint: number, elsewhere: number): Float64Array {
  return Float64Array.from({ length: BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0] + 1 }, (_, i) =>
    i === 100 ? atPoint : elsewhere,
  );
}

describe("an absorber's spec", () => {
  it("takes its layer's column-weighted mean temperature", () => {
    const { specs } = absorberSpecs(
      [{ species: "O3", columnPerM2: 8e22, basePa: 1e6, topPa: 0 }],
      ISOTHERMAL_COLUMN,
    );
    expect(specs[0]?.temperatureK).toBeCloseTo(250, 9);
    // Above Earth's tropopause the profile is its 214.4 K skin.
    const { specs: high } = absorberSpecs(
      [{ species: "O3", columnPerM2: 8e22, basePa: 3_000, topPa: 1_000 }],
      EARTH_COLUMN,
    );
    expect(high[0]?.temperatureK).toBeCloseTo(214.4, 6);
  });

  it("takes methane's cross-sections between its temperatures as a quadratic in ln σ", () => {
    // z = (149 − 198) ÷ 98 = −½: ln σ = ⅜ ln σ₁₀₀ + ¾ ln σ₁₉₈ − ⅛ ln σ₂₉₆ (NEMESIS's tkark.f form).
    const [s100, s198, s296] = [100, 198, 296].map((t) => crossSectionAt("CH4", t));
    const between = crossSectionAt("CH4", 149);
    if (s100 === undefined || s198 === undefined || s296 === undefined || between === undefined) {
      throw new Error("the client holds methane's cross-sections");
    }
    expect(between.provenance).toBe("measured");
    let quadratic = 0;
    let linear = 0;
    for (const [i, value] of between.crossSectionsM2.entries()) {
      const a = s100.crossSectionsM2[i] ?? Number.NaN;
      const b = s198.crossSectionsM2[i] ?? Number.NaN;
      const c = s296.crossSectionsM2[i] ?? Number.NaN;
      const positive = a > 0 && b > 0 && c > 0;
      quadratic += positive ? 1 : 0;
      linear += positive ? 0 : 1;
      const expected = positive
        ? Math.exp(0.375 * Math.log(a) + 0.75 * Math.log(b) - 0.125 * Math.log(c))
        : 0.5 * (a + b);
      expect(Math.abs(value - expected)).toBeLessThanOrEqual(1e-12 * expected);
    }
    // Every point of 380–400 nm is 0 at every temperature, so linear.
    expect([quadratic > 1_000, linear >= 81]).toEqual([true, true]);
  });

  it("takes a quadratic-law point with a 0 linear in σ, floored at 0 below the first temperature", () => {
    const table: CrossSectionTable = {
      species: "Z",
      source: "a test",
      firstNm: BAKE_RANGE_NM[0],
      stepNm: 1,
      temperaturesK: [100, 198, 296],
      temperatureLaw: "lnQuadratic",
      measuredRangeK: [50, 300],
      // Point 100 is 0 at 100 K; every other point is positive at all three.
      crossSectionsM2: [pointAt100(0, 1e-26), pointAt100(2e-26, 2e-26), pointAt100(4e-26, 4e-26)],
    };
    const cold = crossSectionAt("Z", 60, [table]);
    const hot = crossSectionAt("Z", 299, [table]);
    expect([cold?.crossSectionsM2[100], hot?.crossSectionsM2[100], cold?.provenance]).toEqual([
      0,
      expect.closeTo(2e-26 + (2e-26 * 101) / 98, 40),
      "measured",
    ]);
  });

  /** A two-temperature `lnLinear` table whose point 100 is 0 at 100 K. */
  const lnLinearTable: CrossSectionTable = {
    species: "Z",
    source: "a test",
    firstNm: BAKE_RANGE_NM[0],
    stepNm: 1,
    temperaturesK: [100, 300],
    temperatureLaw: "lnLinear",
    measuredRangeK: [100, 300],
    crossSectionsM2: [pointAt100(0, 1e-26), pointAt100(4e-26, 4e-26)],
  };

  it("takes an lnLinear-law point ln σ linear in T between its temperatures", () => {
    // Midway in T: the geometric mean of 1e-26 and 4e-26, not their arithmetic 2.5e-26.
    expect(crossSectionAt("Z", 200, [lnLinearTable])?.crossSectionsM2[0]).toBeCloseTo(2e-26, 40);
  });

  it("takes an lnLinear-law point with a 0 at a temperature linear in σ", () => {
    expect(crossSectionAt("Z", 200, [lnLinearTable])?.crossSectionsM2[100]).toBeCloseTo(2e-26, 40);
  });

  it("takes an lnLinear-law table's own values at its temperatures", () => {
    expect(crossSectionAt("Z", 300, [lnLinearTable])?.crossSectionsM2).toEqual(
      lnLinearTable.crossSectionsM2[1],
    );
  });

  it("takes ozone's cross-sections linear in T between its 10 K steps", () => {
    const [s233, s243, between] = [233, 243, 238].map((t) => crossSectionAt("O3", t));
    for (const [i, value] of (between?.crossSectionsM2 ?? new Float64Array(0)).entries()) {
      const expected =
        0.5 * ((s233?.crossSectionsM2[i] ?? Number.NaN) + (s243?.crossSectionsM2[i] ?? Number.NaN));
      expect(Math.abs(value - expected)).toBeLessThanOrEqual(1e-15 * expected);
    }
    expect(between?.crossSectionsM2).toHaveLength(421);
  });

  it("takes methane's Eq. 8 past its tabulated temperatures, measured at 70 K", () => {
    // z = (70 − 198) ÷ 98: Lagrange's weights ½z(z − 1), 1 − z² and ½z(z + 1).
    const z = (70 - 198) / 98;
    const weights = [0.5 * z * (z - 1), 1 - z * z, 0.5 * z * (z + 1)];
    const [s100, s198, s296] = [100, 198, 296].map((t) => crossSectionAt("CH4", t));
    const cold = crossSectionAt("CH4", 70);
    expect(cold?.provenance).toBe("measured");
    const i = Math.round((727 - 380) / 0.25);
    const ln = [s100, s198, s296].map((s) => Math.log(s?.crossSectionsM2[i] ?? Number.NaN));
    const expected = Math.exp(
      (weights[0] ?? 0) * (ln[0] ?? 0) +
        (weights[1] ?? 0) * (ln[1] ?? 0) +
        (weights[2] ?? 0) * (ln[2] ?? 0),
    );
    expect(Math.abs((cold?.crossSectionsM2[i] ?? Number.NaN) / expected - 1)).toBeLessThan(1e-12);
  });

  it("leaves a Titan-class column near 90 K unlabelled", () => {
    const titan = absorberSpecs(
      [mixed("CH4", 1e26)],
      hydrostaticColumn({
        ...EARTH_COLUMN_INPUT,
        temperature: { kind: "isothermal", temperatureK: 90 },
      }),
    );
    expect(titan.approximations).toEqual([]);
  });

  it("labels methane below 50 K estimated, and holds its 50 K values", () => {
    const cold = crossSectionAt("CH4", 40);
    const at50 = crossSectionAt("CH4", 50);
    expect([cold?.provenance, cold?.crossSectionsM2]).toEqual(["estimated", at50?.crossSectionsM2]);
  });

  it("labels ozone above 293 K estimated", () => {
    expect(crossSectionAt("O3", 300)?.provenance).toBe("estimated");
  });

  it("reports a profile beyond its cross-sections' measured range", () => {
    const { approximations } = absorberSpecs(
      [mixed("CH4", 1e26)],
      hydrostaticColumn({
        ...EARTH_COLUMN_INPUT,
        temperature: { kind: "isothermal", temperatureK: 40 },
      }),
    );
    expect(approximations).toEqual([
      { species: "CH4", provenance: "estimated", reason: "temperatureOutsideTable" },
    ]);
  });

  it("holds no cross-sections for a key the client does not know", () => {
    expect(crossSectionAt("XeF2", 250)).toBeUndefined();
  });

  it.each([
    [
      "listed twice",
      [
        { species: "O3", columnPerM2: 1, basePa: 2_000, topPa: 500 },
        { species: "O3", columnPerM2: 1, basePa: 1_000, topPa: 500 },
      ],
    ],
    ["a negative column", [{ species: "O3", columnPerM2: -1, basePa: 2_000, topPa: 500 }]],
    ["a NaN column", [{ species: "O3", columnPerM2: Number.NaN, basePa: 2_000, topPa: 500 }]],
    ["a base above its top", [{ species: "O3", columnPerM2: 1, basePa: 500, topPa: 2_000 }]],
    ["a layer above the column", [{ species: "O3", columnPerM2: 1, basePa: 1e-6, topPa: 1e-7 }]],
  ])("refuses a species %s", (_, layers: AbsorberLayer[]) => {
    expect(() => absorberSpecs(layers, EARTH_COLUMN)).toThrow(RangeError);
  });

  it("leaves out a layer of no column", () => {
    expect(
      absorberSpecs([{ species: "O3", columnPerM2: 0, basePa: 2_000, topPa: 500 }], EARTH_COLUMN),
    ).toEqual({ specs: [], approximations: [] });
  });

  it.each([Number.NaN, 0, -10])("refuses a temperature of %s K", (temperatureK) => {
    expect(() => crossSectionAt("O3", temperatureK)).toThrow(RangeError);
  });
});

describe("an absorber's term", () => {
  const layer: AbsorberLayer = { species: "O3", columnPerM2: 8e22, basePa: 12_000, topPa: 500 };
  const [spec] = absorberSpecs([layer], EARTH_COLUMN).specs;
  if (spec?.kind !== "column") {
    throw new Error("the ozone layer gave no column spec");
  }
  const term = absorberTerm(spec, EARTH_COLUMN);

  it("carries the spec's vertical column", () => {
    const numberDensity =
      term.absorber?.kind === "column" ? term.absorber.numberDensityPerM3 : Number.NaN;
    const column = numberDensity * columnLengthM(term.density, EARTH_COLUMN.topHeightM);
    expect(Math.abs(column / amountValue(absorberVerticalAmount(spec)) - 1)).toBeLessThan(1e-12);
    expect(term.absorber?.spec).toBe(spec);
  });

  it("absorbs and scatters nothing but by its curves", () => {
    expect([term.scattering, term.absorption, term.phase]).toEqual([
      [0, 0, 0],
      [0, 0, 0],
      { kind: "none" },
    ]);
  });

  it("follows the column's density inside its layer and holds none outside it", () => {
    const heightAt = (pressurePa: number): number => {
      const i = EARTH_COLUMN.pressuresPa.findIndex((p) => p < pressurePa);
      return EARTH_COLUMN.altitudesM[i] ?? Number.NaN;
    };
    const inside = heightAt(3_000);
    expect(densityAt(term.density, inside)).toBeCloseTo(
      densityAt(EARTH_COLUMN.density, inside),
      12,
    );
    expect(densityAt(term.density, heightAt(20_000))).toBe(0);
    expect(densityAt(term.density, heightAt(100))).toBe(0);
    expect(densityAt(term.density, 0)).toBe(0);
  });

  it("reaches the column's top for a layer with no top", () => {
    const [methane] = absorberSpecs([mixed("CH4", 1e26)], EARTH_COLUMN).specs;
    if (methane === undefined) {
      throw new Error("the methane layer gave no spec");
    }
    const whole = absorberTerm(methane, EARTH_COLUMN);
    expect(methane.topPa).toBe(TOP_PA);
    if (whole.density.kind !== "tabulated") {
      throw new Error("an absorber's term has a tabulated density");
    }
    expect(whole.density.altitudesM.at(-1)).toBe(EARTH_COLUMN.topHeightM);
    expect(densityAt(whole.density, 0)).toBe(1);
  });

  it("refuses a spec of no column", () => {
    expect(() => absorberTerm({ ...spec, columnPerM2: 0 }, EARTH_COLUMN)).toThrow(RangeError);
  });

  /** R05's Earth with the absorber term added. */
  const withAbsorber: AtmosphereMedium = {
    ...EARTH_REFERENCE,
    terms: [...EARTH_REFERENCE.terms, term],
  };

  it("is packed by the kernels' packer in absorber slot 0, with its n ÷ U (R08.T6.c)", () => {
    const packed = packMedium(withAbsorber).subarray(3 * 16, 4 * 16);
    const numberDensity =
      term.absorber?.kind === "column" ? term.absorber.numberDensityPerM3 : Number.NaN;
    const perM = numberDensity / spec.columnPerM2;
    expect([packed[3], packed[7]]).toEqual([Math.fround(perM), 1]);
  });

  it("is read by the CPU twin, its vertical column from the ground u ÷ U = 1 (R08.T6.c)", () => {
    // Texel (0, 0): the ground, straight up; its alpha is the absorber's column over its own. At
    // 20,000 midpoint steps, since the layer's edges are sharp (the kernel's 256 give 0.974).
    const size = { widthTexels: 8, heightTexels: 4 };
    const table = transmittanceTwin(withAbsorber, 6_371_008.8, size, 20_000);
    expect(table.texels[3]).toBeCloseTo(1, 3);
  });

  it("has extinction coefficients of 0: it absorbs by its curves alone", () => {
    expect(extinction(term)).toEqual([0, 0, 0]);
  });
});

/** A committed table by its species. */
function committedTable(species: string): CrossSectionTable {
  const found = ABSORBER_CROSS_SECTIONS.find((t) => t.species === species);
  if (found === undefined) {
    throw new Error(`crossSections.json holds no ${species}`);
  }
  return found;
}

/** A minimal valid cross-sections file: one species at two temperatures, 380–760 nm at 190 nm. */
const TOY_FILE: CrossSectionsFile = {
  tables: [
    {
      species: "X",
      source: "a test",
      reduction: "none",
      firstNm: 380,
      stepNm: 190,
      temperaturesK: [100, 200],
      temperatureLaw: "linear",
      measuredRangeK: [100, 200],
      rows: [
        [380, 1e-25, 2e-25],
        [570, 1e-25, 2e-25],
        [760, 1e-25, 2e-25],
      ],
    },
  ],
};

/** {@link TOY_FILE} with its one table changed. */
function toyWith(change: Partial<CrossSectionsFile["tables"][number]>): CrossSectionsFile {
  const [table] = TOY_FILE.tables;
  if (table === undefined) {
    throw new Error("the toy file has a table");
  }
  return { tables: [{ ...table, ...change }] };
}

describe("reading a cross-sections file", () => {
  it("reads a valid file", () => {
    expect(readCrossSections(TOY_FILE).map((t) => [t.species, t.measuredRangeK])).toEqual([
      ["X", [100, 200]],
    ]);
  });

  it("refuses a species listed twice", () => {
    expect(() => readCrossSections({ tables: [...TOY_FILE.tables, ...TOY_FILE.tables] })).toThrow(
      /listed twice/u,
    );
  });

  it.each([
    ["temperatures that do not ascend", toyWith({ temperaturesK: [200, 100] }), /ascend/u],
    ["an unknown law", toyWith({ temperatureLaw: "cubic" }), /temperature law/u],
    [
      "a quadratic law on two temperatures",
      toyWith({ temperatureLaw: "lnQuadratic" }),
      /temperature law/u,
    ],
    [
      "a measured range past a linear table",
      toyWith({ measuredRangeK: [50, 200] }),
      /measured range/u,
    ],
    [
      "a measured range past an lnLinear table",
      toyWith({ temperatureLaw: "lnLinear", measuredRangeK: [100, 400] }),
      /measured range/u,
    ],
    [
      "a row off the grid",
      toyWith({
        rows: [
          [380, 1, 1],
          [571, 1, 1],
          [760, 1, 1],
        ],
      }),
      /row 1/u,
    ],
    [
      "a negative σ",
      toyWith({
        rows: [
          [380, 1, 1],
          [570, -1, 1],
          [760, 1, 1],
        ],
      }),
      /σ at 570/u,
    ],
    [
      "a grid short of 760 nm",
      toyWith({
        rows: [
          [380, 1, 1],
          [570, 1, 1],
        ],
      }),
      /cover/u,
    ],
  ])("refuses %s", (_, file, message) => {
    expect(() => readCrossSections(file)).toThrow(message);
  });
});

describe("the committed cross-sections", () => {
  it("hold ozone at 193–293 K at 1 nm over 380–800 nm", () => {
    const ozone = committedTable("O3");
    expect([
      ozone.firstNm,
      ozone.stepNm,
      ozone.crossSectionsM2[0]?.length,
      ozone.temperaturesK,
    ]).toEqual([380, 1, 421, [193, 203, 213, 223, 233, 243, 253, 263, 273, 283, 293]]);
  });

  it("hold methane at 100, 198 and 296 K at 0.25 nm over 380–800 nm", () => {
    const methane = committedTable("CH4");
    expect([
      methane.firstNm,
      methane.stepNm,
      methane.crossSectionsM2[0]?.length,
      methane.temperaturesK,
    ]).toEqual([380, 0.25, 1_681, [100, 198, 296]]);
  });

  it("bin ozone centred: +8.9%, −6.4% and −16.2% from R05's upward 10 nm bins at 680, 550 and 440 nm", () => {
    // decisions-r05.md item 2, from Serdyuchenko et al.'s 233 K column; R05's coefficients are
    // σ × 300 DU ÷ 15 km.
    const peakPerM3 = (300 * DOBSON_UNIT_PER_M2) / 15_000;
    const ozone = crossSectionAt("O3", 233);
    if (ozone === undefined) {
      throw new Error("the client holds ozone's cross-sections");
    }
    const expected = [0.089, -0.064, -0.162];
    for (const [c, nm] of [680, 550, 440].entries()) {
      const upward = (OZONE_ABSORPTION_PER_M[c] ?? Number.NaN) / peakPerM3;
      const centred = ozone.crossSectionsM2[nm - 380] ?? Number.NaN;
      expect(Math.abs(centred / upward - 1 - (expected[c] ?? Number.NaN))).toBeLessThan(0.003);
    }
  });

  it("keep methane at 100 K within 7% of Karkoschka 1998's coefficients at four band centres", () => {
    // Karkoschka 1998 (Icarus 133, 134), PDS GBAT_0001 1995low.tab (DOI 10.17189/2bp8-k793, CC0),
    // derived from the giant planets' spectra, km⁻¹ amagat⁻¹ at the vacuum wavelength: 619.2 nm
    // 0.7366, 702.0 nm 0.3450, 727.2 nm 4.5742, 780.0 nm 0.5618. Each against the 1 nm mean of
    // the 0.25 nm grid, Karkoschka 1998's resolution.
    const methane = crossSectionAt("CH4", 100);
    if (methane === undefined) {
      throw new Error("the client holds methane's cross-sections");
    }
    for (const [nm, k] of [
      [619.2, 0.7366],
      [702.0, 0.345],
      [727.2, 4.5742],
      [780.0, 0.5618],
    ] as const) {
      let sum = 0;
      for (let j = -2; j <= 2; j += 1) {
        const at = Math.round((nm - 380) / 0.25) + j;
        sum += (methane.crossSectionsM2[at] ?? Number.NaN) * (j === -2 || j === 2 ? 0.5 : 1);
      }
      const ours = (sum / 4) * KM_AMAGAT_PER_M2;
      expect(Math.abs(ours / k - 1)).toBeLessThan(0.07);
    }
  });

  it("hold SO₂'s near-ultraviolet tail at 298–358 K at 1 nm, 0 from 417 nm", () => {
    const so2 = committedTable("SO2");
    const at298 = crossSectionAt("SO2", 298);
    expect([so2.temperaturesK, so2.measuredRangeK, so2.crossSectionsM2[0]?.length]).toEqual([
      [298, 318, 338, 358],
      [298, 358],
      421,
    ]);
    // Hermans et al. 2009's ã–X̃ band near 383 nm, 2.4 × 10⁻²² cm² at 298 K (the snapshot's file).
    expect(at298?.crossSectionsM2[383 - 380]).toBeCloseTo(2.448e-26, 29);
    expect(Math.max(...(at298?.crossSectionsM2.subarray(417 - 380) ?? [1]))).toBe(0);
  });

  it("label SO₂ at Venus's cloud tops, 240 K, estimated", () => {
    expect(crossSectionAt("SO2", 240)?.provenance).toBe("estimated");
  });
});

/** H₂–He's σ at 700 nm at a temperature, m⁵. */
function hydrogenHeliumAt700(temperatureK: number): number {
  return pairCrossSectionAt("H2+He", temperatureK)?.crossSectionsM5[700 - 380] ?? Number.NaN;
}

/** A committed pair table by its key. */
function committedPair(pair: string) {
  const found = ABSORBER_PAIR_CROSS_SECTIONS.find((t) => t.pair === pair);
  if (found === undefined) {
    throw new Error(`crossSections.json holds no pair ${pair}`);
  }
  return found;
}

describe("the committed pairs", () => {
  it("hold O₂–O₂ at 203–293 K, measured over 203–287 K", () => {
    const pair = committedPair("O2+O2");
    expect([pair.members, pair.temperaturesK, pair.measuredRangeK, pair.temperatureLaw]).toEqual([
      ["O2", "O2"],
      [203, 223, 233, 253, 263, 273, 287, 293],
      [203, 287],
      "linear",
    ]);
  });

  // The sets' own peaks (HITRAN2024's file, at 1 cm-1): 380.3, 446.5, 477.0, 532.8, 577.7 and
  // 630.1 nm; the plan's "near 380, 446, 477, 532, 577 and 630 nm".
  it.each([
    [380.3, 375, 385],
    [446.5, 440, 452],
    [477.0, 470, 485],
    [532.8, 525, 540],
    [577.7, 570, 585],
    [630.1, 620, 640],
  ])(
    "put O₂–O₂'s double transition near %s nm in the 1 nm bin beside it, its peak",
    (peakNm, fromNm, toNm) => {
      const sigma = pairCrossSectionAt("O2+O2", 273)?.crossSectionsM5 ?? new Float64Array(0);
      let best = fromNm;
      for (let nm = fromNm; nm <= toNm; nm += 1) {
        best = (sigma[nm - 380] ?? 0) > (sigma[best - 380] ?? 0) ? nm : best;
      }
      expect(Math.abs(best - peakNm)).toBeLessThanOrEqual(0.5);
    },
  );

  it("hold O₂–N₂'s A band at 296 K alone, measured there", () => {
    const pair = committedPair("O2+N2");
    expect([pair.temperaturesK, pair.measuredRangeK]).toEqual([[296], [296, 296]]);
  });

  it("hold O₂–N₂ at 0 to 723 nm, before its set starts at 722.6 nm (13,839.6 cm⁻¹)", () => {
    const sigma = committedPair("O2+N2").crossSectionsM5[0] ?? new Float64Array(0);
    expect(Math.max(...sigma.subarray(0, 723 - 380 + 1))).toBe(0);
  });

  it("put O₂–N₂'s largest σ in the range at 760 nm", () => {
    const sigma = committedPair("O2+N2").crossSectionsM5[0] ?? new Float64Array(0);
    const inRange = sigma.subarray(0, 760 - 380 + 1);
    expect(inRange.indexOf(Math.max(...inRange)) + 380).toBe(760);
  });

  it("label O₂–N₂ off its one temperature estimated", () => {
    expect(pairCrossSectionAt("O2+N2", 280)?.provenance).toBe("estimated");
  });

  it("hold H₂–He at 26 temperatures, ln σ linear in T, measured over 200–9,900 K", () => {
    const pair = committedPair("H2+He");
    expect([pair.temperaturesK.length, pair.temperatureLaw, pair.measuredRangeK]).toEqual([
      26,
      "lnLinear",
      [200, 9_900],
    ]);
  });

  it("hold no H₂–He set between 925 and 1,200 K", () => {
    expect(committedPair("H2+He").temperaturesK.some((t) => t > 925 && t < 1_200)).toBe(false);
  });

  it("hold H₂–He at 0 to 500 nm, where HITRAN's file ends, and above 0 from 501 nm", () => {
    const sigma = committedPair("H2+He").crossSectionsM5.at(-1) ?? new Float64Array(0);
    expect([Math.max(...sigma.subarray(0, 500 - 380 + 1)), (sigma[501 - 380] ?? 0) > 0]).toEqual([
      0,
      true,
    ]);
  });

  it("take H₂–He ln-linear in T between its 925 and 1,200 K sets", () => {
    expect(hydrogenHeliumAt700(1_062.5)).toBeCloseTo(
      Math.sqrt(hydrogenHeliumAt700(925) * hydrogenHeliumAt700(1_200)),
      70,
    );
  });

  it("hold H₂–H₂ at Borysow's 20 temperatures, 60–7,000 K, ln σ linear in T", () => {
    const pair = committedPair("H2+H2");
    expect([pair.members, pair.temperatureLaw, pair.measuredRangeK]).toEqual([
      ["H2", "H2"],
      "lnLinear",
      [60, 7_000],
    ]);
    expect(pair.temperaturesK).toEqual([
      60, 100, 150, 200, 250, 300, 350, 400, 500, 600, 700, 800, 900, 1_000, 2_000, 3_000, 4_000,
      5_000, 6_000, 7_000,
    ]);
  });

  it.each([
    [60, 607],
    [350, 607],
    [400, 589],
    [600, 597],
    [1_000, 589],
    [2_000, 501],
    [7_000, 501],
  ])("hold H₂–H₂ at %s K above 0 from %s nm, where its table reaches, and 0 below", (t, fromNm) => {
    const pair = committedPair("H2+H2");
    const sigma = pair.crossSectionsM5[pair.temperaturesK.indexOf(t)] ?? new Float64Array(0);
    expect([Math.max(...sigma.subarray(0, fromNm - 380)), (sigma[fromNm - 380] ?? 0) > 0]).toEqual([
      0,
      true,
    ]);
  });

  it("put H₂–H₂'s largest σ in the range at 760 nm, the second overtone's blue wing", () => {
    const sigma = pairCrossSectionAt("H2+H2", 300)?.crossSectionsM5 ?? new Float64Array(0);
    const inRange = sigma.subarray(0, 760 - 380 + 1);
    expect(inRange.indexOf(Math.max(...inRange)) + 380).toBe(760);
  });

  it("give H⁻'s free–free in closed form, John 1988's Eq. 6 times k_B T", () => {
    expect(pairCrossSectionAt("H+e-", 2_500)?.crossSectionsM5[600 - 380]).toBeCloseTo(
      hMinusFreeFreeM5(600, 2_500),
      70,
    );
  });

  it("measure H⁻'s free–free over 1,400–10,080 K and hold it, estimated, beyond", () => {
    const cold = pairCrossSectionAt("H+e-", 1_000);
    const atEnd = pairCrossSectionAt("H+e-", 5_040 / 3.6);
    expect([pairCrossSectionAt("H+e-", 2_500)?.provenance, cold?.provenance]).toEqual([
      "measured",
      "estimated",
    ]);
    expect(cold?.crossSectionsM5).toEqual(atEnd?.crossSectionsM5);
  });

  it("give He⁻'s free–free from John 1994's Table 2, on He and e⁻", () => {
    const he = pairCrossSectionAt("He+e-", 2_520);
    expect([he?.provenance, he?.members]).toEqual(["measured", ["He", "e-"]]);
    // John 1994, Table 2, θ 2.0 at 650.9 nm: 0.117 × 10⁻²⁶ cm⁴ dyn⁻¹, times k_B T, in m⁵.
    const k = 1.380_649e-16;
    // 651 nm lies 0.1 nm from the row, within 10⁻³ of it.
    const expected = 0.117e-26 * k * 2_520 * 1e-10;
    expect(Math.abs((he?.crossSectionsM5[651 - 380] ?? 0) / expected - 1)).toBeLessThan(1e-3);
  });

  it("give H⁻'s bound–free in closed form, per H⁻, measured at any temperature", () => {
    const hMinus = crossSectionAt("H-", 2_500);
    expect([
      hMinus?.provenance,
      hMinus?.firstNm,
      hMinus?.stepNm,
      hMinus?.crossSectionsM2.length,
    ]).toEqual(["measured", 380, 1, 421]);
    expect(hMinus?.crossSectionsM2[550 - 380]).toBe(hMinusBoundFreeM2(550, 2_500));
  });
});

/** A well-mixed gas's layer on a column: x n_s ∫ ρ dz from the datum to the top. */
function wellMixed(species: string, moleFraction: number, column: AtmosphereColumn): AbsorberLayer {
  return {
    species,
    columnPerM2:
      moleFraction *
      column.surfaceNumberDensityPerM3 *
      columnLengthM(column.density, column.topHeightM),
    basePa: column.pressuresPa[0] ?? Number.NaN,
    topPa: 0,
  };
}

/** ∫ n_A n_B dz by the trapezoid rule on a column's levels 0 to `last`, n = c ρ. */
function pairQuadrature(column: AtmosphereColumn, ca: number, cb: number, last: number): number {
  let sum = 0;
  for (let i = 1; i <= last; i += 1) {
    const dz = (column.altitudesM[i] ?? Number.NaN) - (column.altitudesM[i - 1] ?? Number.NaN);
    const r0 = column.density.relative[i - 1] ?? Number.NaN;
    const r1 = column.density.relative[i] ?? Number.NaN;
    sum += 0.5 * (ca * cb * r0 * r0 + ca * cb * r1 * r1) * dz;
  }
  return sum;
}

/** The pair specs among a body's specs. */
function pairSpecs(specs: ReadonlyArray<AbsorberSpec>): PairAbsorberSpec[] {
  return specs.flatMap((s) => (s.kind === "pair" ? [s] : []));
}

describe("pair absorbers", () => {
  /** Earth's O₂ on Design note 3's column, and the specs it forms. */
  const earthOxygen = absorberSpecs(
    [wellMixed("O2", EARTH_OXYGEN_FRACTION, EARTH_COLUMN)],
    EARTH_COLUMN,
  );
  /** Earth's O₂–O₂ spec. */
  function earthOxygenPair(): PairAbsorberSpec {
    const [pair] = pairSpecs(earthOxygen.specs);
    if (pair === undefined) {
      throw new Error("Earth's O₂ formed no pair");
    }
    return pair;
  }

  it("form Earth's O₂–O₂ as its one spec", () => {
    expect(earthOxygen.specs.map((s) => s.name)).toEqual(["pair:O2+O2"]);
  });

  it("label Earth's O₂ for its own bands, drawn as nothing until R08.T4.d", () => {
    expect(earthOxygen.approximations).toEqual([
      { species: "O2", provenance: "none", reason: "noCrossSections" },
    ]);
  });

  it("give Earth a vertical O₂–O₂ column of about 1.2 × 10⁴³ cm⁻⁵ (1.0–1.4)", () => {
    const columnPerCm5 = earthOxygenPair().pairColumnPerM5 * 1e-10;
    expect([columnPerCm5 > 1.0e43, columnPerCm5 < 1.4e43]).toEqual([true, true]);
  });

  it("give Earth's vertical O₂–O₂ optical depth at 477 nm in 0.004–0.012", () => {
    const pair = earthOxygenPair();
    const tau =
      (pair.pairs[0]?.crossSection.crossSectionsM5[477 - 380] ?? 0) * pair.pairColumnPerM5;
    expect([tau > 0.004, tau < 0.012]).toEqual([true, true]);
  });

  it("give a pair the column ∫ n_A n_B dz, the trapezoid of the members' densities to 10⁻⁹", () => {
    const column = ISOTHERMAL_COLUMN;
    const ns = column.surfaceNumberDensityPerM3;
    const last = column.altitudesM.length - 1;
    const whole = absorberSpecs(
      [wellMixed("H2", 0.84, column), wellMixed("He", 0.16, column)],
      column,
    );
    const hydrogenHelium = (specs: ReadonlyArray<AbsorberSpec>) =>
      pairSpecs(specs)
        .flatMap((s) => s.pairs)
        .find((p) => p.pair === "H2+He");
    const expected = pairQuadrature(column, 0.84 * ns, 0.16 * ns, last);
    expect(
      Math.abs((hydrogenHelium(whole.specs)?.pairColumnPerM5 ?? Number.NaN) / expected - 1),
    ).toBeLessThan(1e-9);
    // He only up to a level's pressure: the overlap ends there, c_He from its own layer.
    const k = 300;
    const topPa = column.pressuresPa[k] ?? Number.NaN;
    const heliumColumn = 1e27;
    const partialSpecs = absorberSpecs(
      [
        wellMixed("H2", 0.84, column),
        { species: "He", columnPerM2: heliumColumn, basePa: 1e6, topPa },
      ],
      column,
    ).specs;
    const partial = pairSpecs(partialSpecs).find((s) => s.pairs.some((p) => p.pair === "H2+He"));
    let heLength = 0;
    for (let i = 1; i <= k; i += 1) {
      heLength +=
        0.5 *
        ((column.density.relative[i - 1] ?? Number.NaN) +
          (column.density.relative[i] ?? Number.NaN)) *
        ((column.altitudesM[i] ?? Number.NaN) - (column.altitudesM[i - 1] ?? Number.NaN));
    }
    const partialExpected = pairQuadrature(column, 0.84 * ns, heliumColumn / heLength, k);
    expect(Math.abs((partial?.pairColumnPerM5 ?? Number.NaN) / partialExpected - 1)).toBeLessThan(
      1e-9,
    );
    expect(partial?.topPa).toBe(topPa);
  });

  it("form no pair with an absent member", () => {
    expect(
      pairSpecs(absorberSpecs([wellMixed("He", 0.15, EARTH_COLUMN)], EARTH_COLUMN).specs),
    ).toEqual([]);
  });

  it("form no pair from layers that do not overlap", () => {
    const formed = pairSpecs(
      absorberSpecs(
        [
          { species: "H2", columnPerM2: 1e27, basePa: 100_000, topPa: 50_000 },
          { species: "He", columnPerM2: 1e26, basePa: 40_000, topPa: 10_000 },
        ],
        EARTH_COLUMN,
      ).specs,
    ).flatMap((s) => s.pairs.map((p) => p.pair));
    // H₂ pairs with itself on its own layer; H₂–He has no overlap to form on.
    expect(formed).toEqual(["H2+H2"]);
  });

  it("form H₂–H₂ from H₂ alone, on its own layer", () => {
    const [pair] = pairSpecs(
      absorberSpecs([wellMixed("H2", 0.9, EARTH_COLUMN)], EARTH_COLUMN).specs,
    );
    expect(pair?.name).toBe("pair:H2+H2");
  });

  it("merge pairs on one overlap into one spec, and leave transparent members unlabelled", () => {
    const { specs, approximations } = absorberSpecs(
      [
        wellMixed("O2", 0.01, ISOTHERMAL_COLUMN),
        wellMixed("H2", 0.84, ISOTHERMAL_COLUMN),
        wellMixed("He", 0.15, ISOTHERMAL_COLUMN),
      ],
      ISOTHERMAL_COLUMN,
    );
    expect(specs.map((s) => s.name)).toEqual(["pair:O2+O2,H2+He,H2+H2"]);
    // O₂ and H₂ are pending (their own lines); He is transparent and takes no label.
    expect(approximations).toEqual([
      { species: "O2", provenance: "none", reason: "noCrossSections" },
      { species: "H2", provenance: "none", reason: "noCrossSections" },
    ]);
  });

  it("form H⁻'s free–free from H and e⁻, estimated below 1,400 K", () => {
    const at = (temperatureK: number) =>
      absorberSpecs(
        [
          wellMixed("H", 1e-3, isothermal(temperatureK)),
          wellMixed("e-", 1e-7, isothermal(temperatureK)),
        ],
        isothermal(temperatureK),
      );
    const hot = at(3_000);
    const cool = at(1_000);
    expect(hot.specs.map((s) => s.name)).toEqual(["pair:H+e-"]);
    expect(cool.approximations).toEqual([
      { species: "H", provenance: "none", reason: "noCrossSections" },
      { species: "H+e-", provenance: "estimated", reason: "temperatureOutsideTable" },
    ]);
  });

  it("take a pair profile's pair-weighted mean temperature, warmer than its column-weighted one", () => {
    const [pair] = pairSpecs(
      absorberSpecs([wellMixed("O2", EARTH_OXYGEN_FRACTION, EARTH_COLUMN)], EARTH_COLUMN).specs,
    );
    const [ozone] = absorberSpecs([wellMixed("O3", 1e-6, EARTH_COLUMN)], EARTH_COLUMN).specs;
    expect(pair?.temperatureK).toBeGreaterThan(ozone?.temperatureK ?? Number.POSITIVE_INFINITY);
    const [isothermalPair] = pairSpecs(
      absorberSpecs([wellMixed("O2", 0.2, ISOTHERMAL_COLUMN)], ISOTHERMAL_COLUMN).specs,
    );
    expect(isothermalPair?.temperatureK).toBeCloseTo(250, 9);
  });

  it("count a pair profile among the stored columns, and report a dropped pair by its key", () => {
    const layers = sixProfiles(() => 1e24).slice(0, 5);
    layers.push({ species: "O2", columnPerM2: 1e20, basePa: 100_000, topPa: 0 });
    const tables = [...flatTables(SIX)];
    const { specs, approximations } = absorberSpecs(layers, EARTH_COLUMN, tables);
    expect(specs).toHaveLength(ABSORBER_COLUMNS);
    expect(approximations).toEqual([
      { species: "O2", provenance: "none", reason: "noCrossSections" },
      { species: "O2+O2", provenance: "none", reason: "noColumnFree" },
    ]);
  });

  it("lay a pair's term with its pair column, U ÷ ∫ρ² dz at relative density 1", () => {
    const pair = earthOxygenPair();
    const term = absorberTerm(pair, EARTH_COLUMN);
    const density = term.absorber?.kind === "pair" ? term.absorber.pairDensityPerM6 : Number.NaN;
    expect(
      Math.abs(
        (density * columnLengthM(term.density, EARTH_COLUMN.topHeightM)) / pair.pairColumnPerM5 - 1,
      ),
    ).toBeLessThan(1e-12);
  });

  it("lay a pair's term on the square of the column's relative density", () => {
    const term = absorberTerm(earthOxygenPair(), EARTH_COLUMN);
    const h = EARTH_COLUMN.altitudesM[200] ?? Number.NaN;
    expect(densityAt(term.density, h)).toBeCloseTo(densityAt(EARTH_COLUMN.density, h) ** 2, 12);
  });

  it("name a pair's term by its pairs, and scatter and absorb nothing but by its curves", () => {
    const term = absorberTerm(earthOxygenPair(), EARTH_COLUMN);
    expect([term.name, term.scattering, term.absorption, term.phase]).toEqual([
      "pair:O2+O2",
      [0, 0, 0],
      [0, 0, 0],
      { kind: "none" },
    ]);
  });

  it("give a pair's term extinction coefficients of 0: it absorbs by its curves alone (R08.T6.c)", () => {
    expect(extinction(absorberTerm(earthOxygenPair(), EARTH_COLUMN))).toEqual([0, 0, 0]);
  });
});

/** Earth's air isothermal at a temperature. */
function isothermal(temperatureK: number): AtmosphereColumn {
  return hydrostaticColumn({
    ...EARTH_COLUMN_INPUT,
    temperature: { kind: "isothermal", temperatureK },
  });
}

describe("Thomson scattering", () => {
  const electrons = wellMixed("e-", 1e-9, EARTH_COLUMN);
  const term = thomsonTerm(electrons, EARTH_COLUMN);

  it("scatters n_e σ_T in every channel, absorbs nothing, with Rayleigh's phase at ρ = 0", () => {
    const ne = 1e-9 * EARTH_COLUMN.surfaceNumberDensityPerM3;
    for (const s of term.scattering) {
      expect(Math.abs(s / (ne * THOMSON_CROSS_SECTION_M2) - 1)).toBeLessThan(1e-12);
    }
    expect([term.name, term.absorption, term.phase, term.absorber]).toEqual([
      "thomson",
      [0, 0, 0],
      { kind: "rayleigh", depolarisation: [0, 0, 0] },
      undefined,
    ]);
    expect(extinction(term)).toEqual(term.scattering);
  });

  it("takes CODATA 2018's σ_T, 6.652 458 7321 × 10⁻²⁹ m²", () => {
    expect(THOMSON_CROSS_SECTION_M2).toBe(6.652_458_7321e-29);
  });

  it("refuses a layer that is not e⁻'s", () => {
    expect(() => thomsonTerm({ ...electrons, species: "H-" }, EARTH_COLUMN)).toThrow(/by e-/u);
  });

  it("refuses a layer of no column", () => {
    expect(() => thomsonTerm({ ...electrons, columnPerM2: 0 }, EARTH_COLUMN)).toThrow(
      /column is finite and above 0/u,
    );
  });
});

/** A minimal valid pair file: one pair at one temperature, 380–760 nm at 190 nm. */
const TOY_PAIRS: PairCrossSectionsFile = {
  pairs: [
    {
      pair: "X+Y",
      members: ["X", "Y"],
      source: "a test",
      reduction: "none",
      firstNm: 380,
      stepNm: 190,
      temperaturesK: [300],
      temperatureLaw: "linear",
      measuredRangeK: [300, 300],
      rows: [
        [380, 1e-60],
        [570, 1e-60],
        [760, 1e-60],
      ],
    },
  ],
};

describe("reading a file's pairs", () => {
  it("reads a valid pair, measured at its one temperature", () => {
    expect(
      readPairCrossSections(TOY_PAIRS).map((t) => [t.pair, t.members, t.measuredRangeK]),
    ).toEqual([["X+Y", ["X", "Y"], [300, 300]]]);
  });

  it.each([
    ["a key not its members'", { pair: "Y+X" }, /joined by \+/u],
    ["three members", { members: ["X", "Y", "Z"] }, /joined by \+/u],
    ["an unknown law", { temperatureLaw: "cubic" }, /temperature law/u],
  ])("refuses %s", (_, change, message) => {
    const [pair] = TOY_PAIRS.pairs;
    if (pair === undefined) {
      throw new Error("the toy file has a pair");
    }
    expect(() => readPairCrossSections({ pairs: [{ ...pair, ...change }] })).toThrow(message);
  });

  it("refuses a pair listed twice", () => {
    expect(() =>
      readPairCrossSections({ pairs: [...TOY_PAIRS.pairs, ...TOY_PAIRS.pairs] }),
    ).toThrow(/listed twice/u);
  });
});
