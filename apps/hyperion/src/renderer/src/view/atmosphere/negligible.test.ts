import { beforeAll, describe, expect, it } from "vitest";

import { EARTH_H2O_COLUMN_PER_M2, EARTH_H2O_TOP_PA } from "../../test/earthLinePaths";
import { inlinePart, toyPart, toySidecar } from "../../test/kDistributionFixtures";
import { RANGE_SUNS, THE_SUN } from "../../test/lineRanges";
import {
  type AbsorberLayer,
  absorberOptics,
  absorberSpecs,
  absorberTerm,
  channelTransmittance,
  type ColumnAbsorberSpec,
  layerMoments,
  type PairAbsorberSpec,
  pairCrossSectionAt,
} from "./absorbers";
import {
  cellMeanSigma,
  cellSigmasAt,
  type KDistributionTable,
  kDistributionAt,
  loadKDistributions,
  readKDistribution,
} from "./absorbers/kDistribution";
import {
  type AbsorberBound,
  PENDING_ABSORBERS,
  PENDING_CELL_COUNT,
  type PendingAbsorber,
} from "./absorbers/transparency";
import { MARS_AIR, MARS_FIT_COLUMN } from "./channels";
import {
  type AtmosphereColumn,
  gasProperties,
  hydrostaticColumn,
  meanMolarMassGPerMol,
} from "./column";
import {
  DOBSON_UNIT_PER_M2,
  EARTH_COLUMN,
  EARTH_DRY_AIR,
  EARTH_OZONE_PROFILE,
  OZONE_COLUMN_DU,
} from "./earth";
import { tableRadiusM } from "./hillaire";
import {
  type AbsorptionVerdict,
  boundCellMeansAt,
  boundCellWeights,
  type BroadenerVerdict,
  channelChanges,
  type FixedGMixVerdict,
  type HeldChiVerdict,
  largestChordAmount,
  lorentzHalfWidthPerCm,
  NEGLIGIBLE_CHANGE,
  type NegligibleBody,
  negligibleAbsorbers,
  type NegligibleVerdict,
} from "./negligible";
import {
  SATURN_OBLATE_AIR,
  SATURN_OBLATE_GRAVITY,
  SATURN_OBLATE_SPHEROID,
  SATURN_OBLATE_TEMPERATURE_K,
} from "./reference/cases";

/** The test suns: R06's Sun and the 2,500 K and 30,000 K Planck suns, T4.b's three. */
const SUNS = RANGE_SUNS.map(([, sun]) => sun);

/** Avogadro's number, mol⁻¹ (SI 2019, exact). */
const AVOGADRO_PER_MOL = 6.022_140_76e23;

/** H₂O's molar mass, kg mol⁻¹ (IUPAC 2013 standard atomic weights). */
const WATER_KG_PER_MOL = 0.018_015_28;

/** A column's air above its datum, molecules m⁻². */
function airColumnPerM2(column: AtmosphereColumn): number {
  const datumPa = column.pressuresPa[0] ?? Number.NaN;
  return column.surfaceNumberDensityPerM3 * layerMoments(column, datumPa, 0, 1).integralM;
}

/** Gases mixed from the datum to the top at their mole fractions, as the record's bulk gases are. */
function wellMixed(
  column: AtmosphereColumn,
  gases: ReadonlyArray<readonly [string, number]>,
): AbsorberLayer[] {
  const datumPa = column.pressuresPa[0] ?? Number.NaN;
  const air = airColumnPerM2(column);
  return gases.map(([species, x]) => ({
    species,
    columnPerM2: x * air,
    basePa: datumPa,
    topPa: 0,
  }));
}

/** A layer's column times a factor. */
function scaled(
  layers: ReadonlyArray<AbsorberLayer>,
  species: string,
  factor: number,
): AbsorberLayer[] {
  return layers.map((l) =>
    l.species === species ? { ...l, columnPerM2: l.columnPerM2 * factor } : l,
  );
}

/**
 * A Lorentz line's share 3–25 cm⁻¹ from its centre at half-width γ, cm⁻¹, (2 ÷ π) arctan(22γ ÷
 * (γ² + 75)), to two figures as the reduction writes it (`src/tools/kDistributions.ts`).
 */
function nearWingShareOf(gammaPerCm: number): number {
  return Number(
    ((2 / Math.PI) * Math.atan((22 * gammaPerCm) / (gammaPerCm ** 2 + 75))).toPrecision(2),
  );
}

function absorptionOf(
  verdicts: ReadonlyArray<NegligibleVerdict>,
  key: string,
): AbsorptionVerdict | undefined {
  return verdicts.find((v): v is AbsorptionVerdict => v.test === "absorption" && v.key === key);
}

function heldChiOf(
  verdicts: ReadonlyArray<NegligibleVerdict>,
  key: string,
): HeldChiVerdict | undefined {
  return verdicts.find((v): v is HeldChiVerdict => v.test === "heldChi" && v.key === key);
}

function fixedGMixOf(
  verdicts: ReadonlyArray<NegligibleVerdict>,
  key: string,
): FixedGMixVerdict | undefined {
  return verdicts.find((v): v is FixedGMixVerdict => v.test === "fixedGMix" && v.key === key);
}

function broadenerOf(
  verdicts: ReadonlyArray<NegligibleVerdict>,
  key: string,
): BroadenerVerdict | undefined {
  return verdicts.find((v): v is BroadenerVerdict => v.test === "broadener" && v.key === key);
}

let tables: ReadonlyArray<KDistributionTable> = [];

beforeAll(async () => {
  tables = await loadKDistributions(["CO", "CO2", "H2O", "N2O", "O2", "H2", "K"], inlinePart);
}, 120_000);

/** Earth's dry air's mole fractions (`earth.ts`'s {@link EARTH_DRY_AIR}: N₂, O₂, Ar and CO₂ at 0.033%). */
const EARTH_BULK = new Map(EARTH_DRY_AIR.map((g) => [g.species, g.moleFraction] as const));

/** N₂O's tropospheric mole fraction, 332 ppb in 2019 (IPCC AR6 WG1, 2021, Ch. 2, Table 2.1). */
const EARTH_N2O_FRACTION = 3.32e-7;

/**
 * An Earth-class body: {@link EARTH_COLUMN} with its dry air and N₂O well mixed, AFGL's 1.42 g cm⁻²
 * of water to 300 hPa (`earthLinePaths.ts`) and the measured ozone profile's 300 DU.
 */
function earthBody(): NegligibleBody {
  const ozoneBasePa = EARTH_OZONE_PROFILE[0]?.pressurePa ?? Number.NaN;
  const ozoneTopPa = EARTH_OZONE_PROFILE.at(-1)?.pressurePa ?? Number.NaN;
  return {
    column: EARTH_COLUMN,
    layers: [
      ...wellMixed(EARTH_COLUMN, [...EARTH_BULK, ["N2O", EARTH_N2O_FRACTION]]),
      {
        species: "H2O",
        columnPerM2: EARTH_H2O_COLUMN_PER_M2,
        basePa: 101_325,
        topPa: EARTH_H2O_TOP_PA,
      },
      {
        species: "O3",
        columnPerM2: OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2,
        basePa: ozoneBasePa,
        topPa: ozoneTopPa,
        profile: EARTH_OZONE_PROFILE,
      },
    ],
    bulkMoleFractions: EARTH_BULK,
    suns: SUNS,
    kDistributions: tables,
  };
}

/** Mars's air (`channels.ts`'s {@link MARS_AIR}). */
const MARS_BULK = new Map(MARS_AIR.map((g) => [g.species, g.moleFraction] as const));

/**
 * A Mars-class body: {@link MARS_FIT_COLUMN}, its air well mixed, and 10 precipitable µm of water,
 * `decision-r08-t4d-kdist.md` §6.5's Mars-class case.
 */
function marsBody(): NegligibleBody {
  return {
    column: MARS_FIT_COLUMN,
    layers: [
      ...wellMixed(MARS_FIT_COLUMN, [...MARS_BULK]),
      {
        species: "H2O",
        columnPerM2: (1e-5 * 1_000 * AVOGADRO_PER_MOL) / WATER_KG_PER_MOL,
        basePa: MARS_FIT_COLUMN.pressuresPa[0] ?? Number.NaN,
        topPa: 0,
      },
    ],
    bulkMoleFractions: MARS_BULK,
    suns: SUNS,
    kDistributions: tables,
  };
}

/** A body of well-mixed gases on an isothermal column. */
function isothermalBody(input: {
  readonly surfacePa: number;
  readonly temperatureK: number;
  readonly molarMassGPerMol: number;
  readonly gravityMS2: number;
  readonly radiusM: number;
  readonly gases: ReadonlyArray<readonly [string, number]>;
  readonly bulk: ReadonlyArray<readonly [string, number]>;
}): NegligibleBody {
  const column = hydrostaticColumn({
    surfacePa: input.surfacePa,
    temperature: { kind: "isothermal", temperatureK: input.temperatureK },
    meanMolarMassGPerMol: input.molarMassGPerMol,
    referenceGravityMS2: input.gravityMS2,
    referenceRadiusM: input.radiusM,
  });
  return {
    column,
    layers: wellMixed(column, input.gases),
    bulkMoleFractions: new Map(input.bulk),
    suns: SUNS,
    kDistributions: tables,
  };
}

/**
 * A Venus-class body at 92 bar: CO₂ 96.5% and N₂ 3.5%, 43.45 g mol⁻¹, g 8.87 m s⁻², R 6,051.8 km
 * (NSSDCA's Venus fact sheet), isothermal at 700 K, a column-weighted stand-in for VIRA's 737 K
 * surface and cooler air above.
 */
function venusBody(): NegligibleBody {
  const gases = [
    ["CO2", 0.965],
    ["N2", 0.035],
  ] as const;
  return isothermalBody({
    surfacePa: 9.2e6,
    temperatureK: 700,
    molarMassGPerMol: 43.45,
    gravityMS2: 8.87,
    radiusM: 6_051_800,
    gases,
    bulk: gases,
  });
}

/**
 * A Uranus-class body of H₂ and He alone (83% and 15% by NSSDCA's Uranus fact sheet, renormalised;
 * its 2.3% CH₄ left out, whose H₂ widths the table does not hold), isothermal at 76 K, 2.64 g mol⁻¹,
 * g 8.69 m s⁻², R 25,559 km: at 3.7 bar about 300 km-amagat of H₂, follow-up 3's column to its deck.
 */
function uranusBody(surfacePa: number): NegligibleBody {
  const gases = [
    ["H2", 0.847],
    ["He", 0.153],
  ] as const;
  return isothermalBody({
    surfacePa,
    temperatureK: 76,
    molarMassGPerMol: 2.64,
    gravityMS2: 8.69,
    radiusM: 25_559_000,
    gases,
    bulk: gases,
  });
}

/**
 * A Saturn-class body (R08.T10.c): `reference/cases.ts`'s {@link SATURN_OBLATE_AIR}, the NSSDCA
 * Saturn fact sheet's 96.3% H₂, 3.25% He and 4,500 ppm CH₄, well mixed and isothermal at
 * {@link SATURN_OBLATE_TEMPERATURE_K} (134 K), its datum at 1 bar on Saturn's g_ref
 * ({@link SATURN_OBLATE_GRAVITY}) and its figure's table radius, (2a + c) ÷ 3, as the
 * `saturn-oblate` reference's column is.
 */
function saturnBody(): NegligibleBody {
  const gases = SATURN_OBLATE_AIR.map(
    ({ species, moleFraction }) => [species, moleFraction] as const,
  );
  return isothermalBody({
    surfacePa: 1e5,
    temperatureK: SATURN_OBLATE_TEMPERATURE_K,
    molarMassGPerMol: meanMolarMassGPerMol(
      SATURN_OBLATE_AIR.map(({ species, moleFraction }) => ({
        properties: gasProperties(species),
        moleFraction,
      })),
    ),
    gravityMS2: SATURN_OBLATE_GRAVITY.referenceGravityMS2,
    radiusM: tableRadiusM(SATURN_OBLATE_SPHEROID),
    gases,
    bulk: gases,
  });
}

/**
 * An Earth-class body ({@link earthBody}) with a bulk gas's mole fraction set: its well-mixed layer,
 * in its place or after the others, and its share of the broadening mixture, the other gases'
 * fractions as they are.
 */
function earthWith(species: string, moleFraction: number): NegligibleBody {
  const body = earthBody();
  const bulk = new Map(EARTH_BULK);
  bulk.set(species, moleFraction);
  const [layer] = wellMixed(EARTH_COLUMN, [[species, moleFraction]]);
  if (layer === undefined) {
    throw new Error("a gas mixed over the column is one layer");
  }
  const held = body.layers.some((l) => l.species === species);
  return {
    ...body,
    layers: held
      ? body.layers.map((l) => (l.species === species ? layer : l))
      : [...body.layers, layer],
    bulkMoleFractions: bulk,
  };
}

/**
 * K's mole fraction in a solar H₂–He envelope: twice H₂'s 0.85 times solar K/H, 10^(5.03 − 12) =
 * 1.07 × 10⁻⁷ (Asplund et al. 2009, ARA&A 47, 481, log ε_K 5.03).
 */
const SOLAR_K_FRACTION = 2 * 0.85 * 1.07e-7;

/**
 * A hot giant's upper envelope: H₂ 85% and He 15% with K, isothermal at 1,500 K, 2.3 g mol⁻¹,
 * g 25 m s⁻², R 70,000 km (hand values for a hot Jupiter), its datum at `surfacePa`, K's column
 * `kColumnPerM2` (its solar mole fraction if absent), and e⁻ where given.
 */
function hotGiantBody(
  surfacePa: number,
  kColumnPerM2?: number,
  electronFraction?: number,
): NegligibleBody {
  const bulk = [
    ["H2", 0.85],
    ["He", 0.15],
  ] as const;
  const body = isothermalBody({
    surfacePa,
    temperatureK: 1_500,
    molarMassGPerMol: 2.3,
    gravityMS2: 25,
    radiusM: 7e7,
    gases: [
      ...bulk,
      ["K", SOLAR_K_FRACTION],
      ...(electronFraction === undefined ? [] : [["e-", electronFraction] as const]),
    ],
    bulk,
  });
  const solarColumnPerM2 = SOLAR_K_FRACTION * airColumnPerM2(body.column);
  return kColumnPerM2 === undefined
    ? body
    : { ...body, layers: scaled(body.layers, "K", kColumnPerM2 / solarColumnPerM2) };
}

/**
 * A snowball-class body (P14.T13.c's snowballs keep their carbon airborne,
 * `decision-r08-t4d-kdist-chain.md` §16): CO₂ and N₂ at an even share, isothermal at 250 K, where CO₂
 * is gaseous to 17.9 bar (Span and Wagner 1996), 36 g mol⁻¹, on Earth's g and radius.
 */
function snowballBody(surfacePa: number): NegligibleBody {
  const gases = [
    ["CO2", 0.5],
    ["N2", 0.5],
  ] as const;
  return isothermalBody({
    surfacePa,
    temperatureK: 250,
    molarMassGPerMol: 36,
    gravityMS2: 9.8,
    radiusM: 6_371_000,
    gases,
    bulk: gases,
  });
}

describe("the bound's pieces", () => {
  it("interpolates a line from a single lower level as the Boltzmann law, to 10⁻⁹", () => {
    // σ = A e^(−E ÷ kT) for lower levels 0.5, 2 and 6 eV up, at nodes 1,000, 2,000 and 3,000 K.
    const kelvinPerEv = 11_604.518;
    const levels = [0.5, 2, 6].map((ev) => ev * kelvinPerEv);
    const law = (t: number): Float64Array =>
      Float64Array.from({ length: PENDING_CELL_COUNT }, (_, c) => {
        const e = levels[c % levels.length] ?? 0;
        return 1e-20 * Math.exp(-e / t);
      });
    const nodes = [1_000, 2_000, 3_000];
    const bound: AbsorberBound = {
      kind: "species",
      key: "Xx",
      task: "R08.T4.e",
      part: undefined,
      basis: "measured",
      temperaturesK: nodes,
      source: "the Boltzmann law",
      method: "exact",
      cellMeansM2: nodes.map(law),
    };
    const worst = Math.max(
      ...[1_250, 1_500, 2_200, 2_900].map((t) => {
        const read = boundCellMeansAt(bound, t).cellMeans;
        const exact = law(t);
        return Math.max(...read.map((v, c) => Math.abs(v / (exact[c] ?? Number.NaN) - 1)));
      }),
    );
    expect(worst).toBeLessThan(1e-9);
  });

  it("holds an entry's end nodes beyond them, and says so", () => {
    const co = PENDING_ABSORBERS.find((p) => p.key === "CO" && p.part === undefined)?.bound;
    if (co === undefined) {
      throw new Error("CO's hot list is pending");
    }
    const below = boundCellMeansAt(co, 500);
    const atFirst = boundCellMeansAt(co, co.temperaturesK[0] ?? Number.NaN);
    expect([below.pastNodes, atFirst.pastNodes, Array.from(below.cellMeans)]).toEqual([
      true,
      false,
      Array.from(atFirst.cellMeans),
    ]);
  });

  it("takes the whole grazing chord tangent at the datum, about √(2πR ÷ H) vertical columns", () => {
    const body = isothermalBody({
      surfacePa: 1e5,
      temperatureK: 250,
      molarMassGPerMol: 28.96,
      gravityMS2: 9.8,
      radiusM: 6_371_000,
      gases: [["CO2", 1e-3]],
      bulk: [["N2", 1]],
    });
    const [layer] = body.layers;
    if (layer === undefined) {
      throw new Error("the body lists CO2");
    }
    const spec: ColumnAbsorberSpec = {
      kind: "column",
      name: "absorber:CO2",
      species: [{ species: "CO2", columnPerM2: layer.columnPerM2, crossSection: undefined }],
      columnPerM2: layer.columnPerM2,
      basePa: 1e5,
      topPa: body.column.pressuresPa.at(-1) ?? Number.NaN,
      temperatureK: 250,
    };
    const amount = largestChordAmount(absorberTerm(spec, body.column), body.column);
    const scaleHeightM = (1.380_649e-23 * 250) / (28.96 * 1.660_539_068_92e-27 * 9.8);
    const chapman = Math.sqrt((2 * Math.PI * 6_371_000) / scaleHeightM);
    expect(amount.kind === "column" ? amount.columnPerM2 / layer.columnPerM2 : 0).toBeCloseTo(
      chapman,
      -1,
    );
  });

  describe("γ_L from a table's near-wing shares", () => {
    // Shares (2 ÷ π) arctan(22γ ÷ (γ² + 75)) at γ ÷ p of 1.0 and 0.8 × 10⁻⁶ cm⁻¹ Pa⁻¹ at 200 and
    // 300 K, to two figures as the reduction writes them; the 1 MPa node's share, 0.18, is past the
    // recovery's 0.1.
    const temperaturesK = [200, 300];
    const pressuresPa = [10, 100, 1e3, 1e4, 1e5, 1e6];
    const perPa = [1e-6, 0.8e-6];
    const cells = [{ nm: 690, weights: [1], sigmasM2: Array.from({ length: 12 }, () => 1e-30) }];
    const table = readKDistribution(
      toySidecar(temperaturesK, pressuresPa, 1, {
        nearWingShare: perPa.flatMap((g) => pressuresPa.map((p) => nearWingShareOf(g * p))),
      }),
      [toyPart(cells, { variants: 1, temperatures: 2, pressures: 6 })],
      "toy",
    );

    it.each([
      [200, 101_325, 1e-6],
      [200, 3e6, 1e-6],
      [300, 101_325, 0.8e-6],
      [250, 101_325, 1e-6],
    ])(
      "returns γ_L within its shares' two-figure rounding, never below it: %d K, %d Pa",
      (temperatureK, broadeningPa, widestPerPa) => {
        const gamma = lorentzHalfWidthPerCm(table, temperatureK, broadeningPa) ?? Number.NaN;
        const exact = widestPerPa * broadeningPa;
        expect([gamma >= exact, gamma / exact < 1.11]).toEqual([true, true]);
      },
    );
  });

  it("weighs a channel's cells by the colour pipeline's rows, their sum the sun's colour", () => {
    const weights = boundCellWeights(THE_SUN);
    const sums = weights.cells.map((row) => row.reduce((s, w) => s + w, 0));
    expect(sums.map((s, c) => s / (weights.totals[c] ?? Number.NaN))).toEqual([1, 1, 1]);
  });

  it("bounds an opaque channel's change by Σ|w| ÷ Σw, its signed weights' own", () => {
    const weights = boundCellWeights(THE_SUN);
    const opaque = channelChanges(weights, new Float64Array(PENDING_CELL_COUNT).fill(10));
    const expected = weights.cells.map(
      (row, c) => row.reduce((s, w) => s + Math.abs(w), 0) / (weights.totals[c] ?? Number.NaN),
    );
    expect(opaque).toEqual(expected.map((e) => expect.closeTo(e, 12)));
  });
});

describe("Δ_c against an f64 spectral reference", () => {
  it("bounds the channel change of Earth's held O₂–CO₂ and its estimated N₂O lines from above", () => {
    const body = earthBody();
    const specs = absorberSpecs(body.layers, body.column, undefined, undefined, {
      kDistributions: tables,
      bulkMoleFractions: EARTH_BULK,
    });
    const verdicts = negligibleAbsorbers(body, { specs });
    const pairSpec = specs.specs.find(
      (s): s is PairAbsorberSpec => s.kind === "pair" && s.pairs.some((p) => p.pair === "O2+CO2"),
    );
    const columnSpec = specs.specs.find(
      (s): s is ColumnAbsorberSpec =>
        s.kind === "column" && s.species.some((x) => x.species === "N2O"),
    );
    const pair = pairSpec?.pairs.find((p) => p.pair === "O2+CO2");
    const n2o = columnSpec?.species.find((x) => x.species === "N2O");
    const pairVerdict = absorptionOf(verdicts, "O2+CO2");
    const n2oVerdict = absorptionOf(verdicts, "N2O");
    if (
      pairSpec === undefined ||
      columnSpec === undefined ||
      pair === undefined ||
      n2o === undefined ||
      pairVerdict === undefined ||
      n2oVerdict === undefined
    ) {
      throw new Error("Earth draws O2+CO2 and N2O, both estimated");
    }
    const onlyPair: PairAbsorberSpec = {
      ...pairSpec,
      pairs: [pair],
      pairColumnPerM5: pair.pairColumnPerM5,
    };
    const onlyN2o: ColumnAbsorberSpec = {
      ...columnSpec,
      species: [n2o],
      columnPerM2: n2o.columnPerM2,
    };
    const [pairEntry] = pairVerdict.entries;
    const [n2oEntry] = n2oVerdict.entries;
    const pairU = pairEntry?.amount.kind === "pair" ? pairEntry.amount.pairColumnPerM5 : Number.NaN;
    const n2oU = n2oEntry?.amount.kind === "column" ? n2oEntry.amount.columnPerM2 : Number.NaN;
    // Every read the bound covers: up to k = 100 times u_max, at the lines' stored rows to the datum.
    const excess: number[] = [];
    for (const [s, sun] of SUNS.entries()) {
      const pairOptics = absorberOptics(onlyPair, sun);
      const n2oOptics = absorberOptics(onlyN2o, sun);
      for (const factor of [1, 10, 100]) {
        const t = channelTransmittance(
          pairOptics,
          { kind: "pair", pairColumnPerM5: factor * pairU },
          undefined,
        );
        excess.push(...t.map((tc, c) => Math.abs(1 - tc) - (pairVerdict.changes[s]?.[c] ?? 0)));
        for (const pressurePa of [10, 1e3, 3.162_28e4, 101_325]) {
          const tl = channelTransmittance(
            n2oOptics,
            { kind: "column", columnPerM2: factor * n2oU },
            pressurePa,
          );
          excess.push(...tl.map((tc, c) => Math.abs(1 - tc) - (n2oVerdict.changes[s]?.[c] ?? 0)));
        }
      }
    }
    expect(Math.max(...excess)).toBeLessThanOrEqual(0);
  });
});

describe("the negligible-absorbers bound on an Earth-class body", () => {
  let verdicts: ReadonlyArray<NegligibleVerdict> = [];

  beforeAll(() => {
    verdicts = negligibleAbsorbers(earthBody());
  });

  it("finds its N₂O negligible, drawn with estimated line shapes at k = 100", () => {
    const n2o = absorptionOf(verdicts, "N2O");
    // Recorded: 4.2 × 10⁻⁷, its largest over the three suns and channels.
    expect([n2o?.negligible, n2o?.entries.map((e) => [e.origin, e.k, e.rule])]).toEqual([
      true,
      [["drawn", 100, "estimatedData"]],
    ]);
    expect(n2o?.largestChange).toBeLessThan(1e-6);
  });

  it("finds its O₂–CO₂ slice negligible, held off 296 K at k = 100, and tests no O₂–N₂", () => {
    const o2co2 = absorptionOf(verdicts, "O2+CO2");
    // Recorded: 2.9 × 10⁻⁶. O₂–N₂ is measured over 206–346 K (Karman et al. 2018), so not tested.
    expect([o2co2?.negligible, o2co2?.entries[0]?.rule, absorptionOf(verdicts, "O2+N2")]).toEqual([
      true,
      "pastTemperatures",
      undefined,
    ]);
    expect(o2co2?.largestChange).toBeLessThan(1e-5);
  });

  it("finds its foreign H₂O continuum below 567 nm not negligible at k = 100", () => {
    // Recorded: 0.147 (green, the 30,000 K sun).
    expect(absorptionOf(verdicts, "H2O+air")?.negligible).toBe(false);
  });

  it("finds its self H₂O continuum below 567 nm not negligible at k = 100", () => {
    // Recorded: 5.0 × 10⁻³.
    expect(absorptionOf(verdicts, "H2O+H2O")?.negligible).toBe(false);
  });

  it("takes the foreign continuum's bound from its cells below 567 nm alone, the rest measured", () => {
    const air = absorptionOf(verdicts, "H2O+air");
    const entry = air?.entries[0];
    const sigma = pairCrossSectionAt("H2O+air", entry?.temperatureK ?? Number.NaN);
    if (air === undefined || entry === undefined || sigma === undefined) {
      throw new Error("Earth draws H2O+air, estimated below 567 nm");
    }
    const u = entry.amount.kind === "pair" ? entry.amount.pairColumnPerM5 : Number.NaN;
    const depths = Float64Array.from({ length: PENDING_CELL_COUNT }, (_, c) =>
      380 + c < 567 ? 100 * (sigma.crossSectionsM5[c] ?? 0) * u : 0,
    );
    const expected = SUNS.map((sun) => channelChanges(boundCellWeights(sun), depths));
    expect(air.changes.flat().map((d, i) => d / (expected.flat()[i] ?? Number.NaN))).toEqual(
      expected.flat().map(() => expect.closeTo(1, 12)),
    );
  });

  it("takes no label from CO₂–N₂'s held χ below 295 K, about 5 × 10⁻⁶ in its strongest cell", () => {
    const held = heldChiOf(verdicts, "CO2");
    const mix = fixedGMixOf(verdicts, "CO2");
    const co2 = tables.find((t) => t.species === "CO2");
    if (held === undefined || mix === undefined || co2 === undefined) {
      throw new Error("Earth's CO2 holds CO2-N2's chi below 295 K");
    }
    // The strongest cell, f₃⟨σ⟩u_max on the whole chord, its mean at 1 bar (716 nm).
    const kd = kDistributionAt(co2, held.temperatureK, mix.selfShare);
    const strongestM2 = Math.max(
      ...cellSigmasAt(kd, 101_325).map((sigmas, c) =>
        cellMeanSigma(kd.cells[c]?.weights ?? new Float64Array(0), sigmas),
      ),
    );
    const u = held.amount.kind === "column" ? held.amount.columnPerM2 : Number.NaN;
    const strongest = held.lorentzShare * strongestM2 * u;
    // Recorded: f₃ 0.019 (γ_L 0.090 cm⁻¹), the strongest cell 5.0 × 10⁻⁶, Δ_c,χ 3.1 × 10⁻⁸.
    expect([held.negligible, strongest > 3e-6 && strongest < 1e-5]).toEqual([true, true]);
    expect(held.largestChange).toBeLessThan(1e-6);
  });

  it("takes no label from CO₂'s fixed-g mix", () => {
    const mix = fixedGMixOf(verdicts, "CO2");
    expect([mix?.negligible, (mix?.largestColumnRatio ?? Infinity) < 1e-4]).toEqual([true, true]);
  });

  it("tests nothing it draws measured, nor N₂ below 1,800 K, nor H₂O's hot list below 296 K", () => {
    const keys = verdicts.map((v) => `${v.key} ${v.test}`);
    expect(keys).toEqual([
      "CO2 heldChi",
      "CO2 fixedGMix",
      "O2+CO2 absorption",
      "N2O absorption",
      "H2O broadener",
      "H2O+H2O absorption",
      "H2O+air absorption",
    ]);
  });

  it("takes no label from CO₂'s broadening of its H₂O lines, which hold no width for it: their effect bound (R08.T10.c)", () => {
    const broadener = broadenerOf(verdicts, "H2O");
    // Recorded: ε 3.25 × 10⁻⁴ (330 ppm over the mixture's Σ x r), Δ_c(k = 1) 0.512 and Δ_c,γ
    // 1.66 × 10⁻⁴, both red under the 30,000 K sun (Σ|w| ÷ Σw passes 1 where r̄ is negative); the
    // ruling's "at most 6.3 × 10⁻⁴". Until R08.T10.c the lines were `estimatedData`, 1.96 at k = 100.
    expect([
      broadener?.negligible,
      (broadener?.widthlessShare ?? 0) > 3e-4 && (broadener?.widthlessShare ?? 1) < 3.5e-4,
      absorptionOf(verdicts, "H2O"),
    ]).toEqual([true, true, undefined]);
    expect(broadener?.largestChange).toBeLessThan(6.3e-4);
  });

  it("bounds the change by the width's factor, −ln(1 − ε), times the lines' own bound at k = 1", () => {
    const broadener = broadenerOf(verdicts, "H2O");
    if (broadener === undefined) {
      throw new Error(
        "Earth's H2O lines are broadened by CO2, which their table holds no width for",
      );
    }
    const factor = -Math.log1p(-broadener.widthlessShare);
    expect([
      broadener.widthFactor,
      broadener.changes.flat().map((d, i) => d / (broadener.lineChanges.flat()[i] ?? Number.NaN)),
    ]).toEqual([
      expect.closeTo(factor, 15),
      broadener.changes.flat().map(() => expect.closeTo(factor, 12)),
    ]);
  });

  it("labels its H₂O lines once CO₂ is raised past about 2,000 ppm, where Δ_c,γ passes 10⁻³", () => {
    const co2 = EARTH_BULK.get("CO2") ?? Number.NaN;
    const raised = [5, 10].map(
      (factor) =>
        broadenerOf(negligibleAbsorbers(earthWith("CO2", factor * co2)), "H2O")?.negligible,
    );
    // Recorded: it crosses between 6.0 and 6.1 times the 330 ppm, 9.97 × 10⁻⁴ and 1.014 × 10⁻³, at
    // ε 1.95 × 10⁻³ and 1.98 × 10⁻³; 8.3 × 10⁻⁴ at 5 times, 1.66 × 10⁻³ at 10.
    expect(raised).toEqual([true, false]);
  });

  it("labels its H₂O lines at any share of a gas with a dipole their table holds no width for, at k = 100", () => {
    // NH₃ at a part per billion: H₂O's table holds only its self width.
    const ammonia = negligibleAbsorbers(earthWith("NH3", 1e-9));
    const h2o = absorptionOf(ammonia, "H2O");
    // Recorded: 1.96, as before R08.T10.c, red under the 30,000 K sun.
    expect([
      h2o?.negligible,
      h2o?.entries.map((e) => [e.origin, e.rule]),
      broadenerOf(ammonia, "H2O"),
    ]).toEqual([false, [["drawn", "estimatedData"]], undefined]);
  });
});

describe("the negligible-absorbers bound on a Mars-class body", () => {
  let verdicts: ReadonlyArray<NegligibleVerdict> = [];

  beforeAll(() => {
    verdicts = negligibleAbsorbers(marsBody());
  });

  it("finds its CO's pending overtones negligible, its 7–0 band drawn measured", () => {
    const co = absorptionOf(verdicts, "CO");
    // Recorded: 1.4 × 10⁻⁸. Hot CO is not in force at 214 K, under its table's 296 K.
    expect([
      co?.negligible,
      co?.entries.map((e) => [e.origin, e.origin === "pending" ? e.part : undefined, e.k]),
    ]).toEqual([true, [["pending", "overtones", 100]]]);
    expect(co?.largestChange).toBeLessThan(1e-6);
  });

  it("finds its H₂O continuum negligible at k = 100, CO₂ its partner", () => {
    const air = absorptionOf(verdicts, "H2O+air");
    const self = absorptionOf(verdicts, "H2O+H2O");
    // Recorded: 4.5 × 10⁻⁶ and 1.5 × 10⁻⁸; decision-r08-t4d-kdist.md §6.5's "under 10⁻⁵".
    expect([air?.negligible, self?.negligible]).toEqual([true, true]);
    expect(Math.max(air?.largestChange ?? 1, self?.largestChange ?? 1)).toBeLessThan(1e-5);
  });

  it("takes no label from CO₂'s held χ or its fixed-g mix", () => {
    expect([
      heldChiOf(verdicts, "CO2")?.negligible,
      fixedGMixOf(verdicts, "CO2")?.negligible,
    ]).toEqual([true, true]);
  });

  it("takes no label from CO's broadening of its O₂ lines, about 10⁻⁸ by their effect bound (R08.T10.c)", () => {
    const broadener = broadenerOf(verdicts, "O2");
    // Recorded: ε 5.2 × 10⁻⁴ (0.06% CO), Δ_c(k = 1) 1.35 × 10⁻⁵ and Δ_c,γ 7.1 × 10⁻⁹, both red under
    // the 2,500 K sun. Until R08.T10.c the lines were `estimatedData`, 1.35 × 10⁻³ at k = 100.
    expect([broadener?.negligible, absorptionOf(verdicts, "O2")]).toEqual([true, undefined]);
    expect(broadener?.largestChange).toBeLessThan(1e-7);
  });

  it("bounds its H₂O lines, which CO₂ broadens with no width on file, by their whole light at k = 1", () => {
    const broadener = broadenerOf(verdicts, "H2O");
    // CO₂ is 95% of the mixture, so ε is 0.95 and the width's factor its ceiling, 1: whatever the
    // widths, the lines move no channel by more than their own bound. Recorded: 2.8 × 10⁻⁴, red
    // under the 30,000 K sun.
    expect([broadener?.widthFactor, broadener?.changes, broadener?.negligible]).toEqual([
      1,
      broadener?.lineChanges,
      true,
    ]);
    expect(broadener?.widthlessShare).toBeGreaterThan(0.9);
  });

  it("labels its CO₂ at 214 K, inside CO₂'s estimated 212.5–225 K, past 10⁻³ at k = 100", () => {
    const co2 = absorptionOf(verdicts, "CO2");
    // Recorded: 2.0 × 10⁻³ (red, the 2,500 K sun; 1.8 × 10⁻³ under the Sun).
    expect([co2?.negligible, co2?.entries.map((e) => e.rule)]).toEqual([false, ["estimatedData"]]);
  });

  it("names a column raised past its bound: CO's overtones at a million times Mars's CO", () => {
    const body = marsBody();
    const raised = negligibleAbsorbers({ ...body, layers: scaled(body.layers, "CO", 1e6) });
    expect([
      absorptionOf(verdicts, "CO")?.negligible,
      absorptionOf(raised, "CO")?.negligible,
    ]).toEqual([true, false]);
  });
});

describe("the hundredfold margin", () => {
  it("is what an estimated entry needs: measured, the same cell means would be negligible", () => {
    const body = marsBody();
    const layers = scaled(body.layers, "CO", 3e5);
    const overtones = PENDING_ABSORBERS.find((p) => p.key === "CO" && p.part === "overtones");
    const bound = overtones?.bound;
    if (overtones === undefined || bound?.kind !== "species") {
      throw new Error("CO's overtones are pending, per molecule");
    }
    // Its one 296 K row, given over 100–400 K so that Mars's 214 K lies within its nodes.
    const row = bound.cellMeansM2[0] ?? new Float64Array(PENDING_CELL_COUNT);
    const spanned = (basis: "measured" | "estimated"): PendingAbsorber => ({
      ...overtones,
      bound: { ...bound, basis, temperaturesK: [100, 400], cellMeansM2: [row, row] },
    });
    const asEstimated = absorptionOf(
      negligibleAbsorbers({ ...body, layers }, { pending: [spanned("estimated")] }),
      "CO",
    );
    const asMeasured = absorptionOf(
      negligibleAbsorbers({ ...body, layers }, { pending: [spanned("measured")] }),
      "CO",
    );
    expect([
      asEstimated?.entries[0]?.k,
      asEstimated?.negligible,
      asMeasured?.entries[0]?.k,
      asMeasured?.negligible,
    ]).toEqual([100, false, 1, true]);
  });
});

describe("the negligible-absorbers bound on a Venus-class body", () => {
  it("finds its CO₂ not negligible: held at 500 K past 10 MPa, hot CO₂ pending in R08.T4.f", () => {
    const co2 = absorptionOf(negligibleAbsorbers(venusBody()), "CO2");
    // Recorded: 0.58. The drawn table and the hot list alternate in one share.
    expect([
      co2?.negligible,
      co2?.entries.map((e) => [
        e.origin,
        e.origin === "pending" ? e.task : undefined,
        e.share,
        e.k,
      ]),
    ]).toEqual([
      false,
      [
        ["drawn", undefined, "lines", 100],
        ["pending", "R08.T4.f", "lines", 100],
      ],
    ]);
  });
});

describe("the negligible-absorbers bound on a Uranus-class body", () => {
  it("draws H₂'s quadrupole lines measured at 1 bar, and tests them not", () => {
    const verdicts = negligibleAbsorbers(uranusBody(1e5));
    expect(absorptionOf(verdicts, "H2")).toBeUndefined();
  });

  it("labels them to a 300 km-amagat deck, where their line shape is estimated, at k = 100", () => {
    const h2 = absorptionOf(negligibleAbsorbers(uranusBody(3.7e5)), "H2");
    // Recorded: 0.10, red under the 30,000 K sun; 3.3 × 10⁻³ at k = 1, were the lines measured.
    expect([h2?.negligible, h2?.entries.map((e) => e.rule)]).toEqual([false, ["estimatedData"]]);
  });
});

describe("the negligible-absorbers bound on a Saturn-class body (R08.T10.c)", () => {
  it("takes no label from CH₄'s broadening of its H₂ lines, about 7 × 10⁻⁶ by their effect bound", () => {
    const verdicts = negligibleAbsorbers(saturnBody());
    const broadener = broadenerOf(verdicts, "H2");
    // Recorded: ε 5.5 × 10⁻³ at 134 K (each width at its own exponent), Δ_c(k = 1) 1.22 × 10⁻³ and
    // Δ_c,γ 6.7 × 10⁻⁶, both red under the 30,000 K sun. Until R08.T10.c the lines were
    // `estimatedData` at k = 100.
    expect([broadener?.negligible, absorptionOf(verdicts, "H2")]).toEqual([true, undefined]);
    expect(broadener?.largestChange).toBeLessThan(1e-5);
  });
});

describe("the negligible-absorbers bound on a hot giant's K wing", () => {
  /** K's column on a 1 kPa datum at a hundredth of solar, held as the perturbers grow. */
  const kColumn = (() => {
    const body = hotGiantBody(1e3);
    return (body.layers.find((l) => l.species === "K")?.columnPerM2 ?? Number.NaN) / 100;
  })();

  it("grows with its perturber density, K's column held", () => {
    const thin = absorptionOf(negligibleAbsorbers(hotGiantBody(1e3, kColumn / 100)), "K+H2");
    const doubled = absorptionOf(negligibleAbsorbers(hotGiantBody(2e3, kColumn / 100)), "K+H2");
    expect((doubled?.largestChange ?? 0) / (thin?.largestChange ?? 1)).toBeCloseTo(2, 2);
  });

  it("is negligible only where the pair column allows", () => {
    const low = absorptionOf(negligibleAbsorbers(hotGiantBody(1e3, kColumn)), "K+H2");
    const high = absorptionOf(negligibleAbsorbers(hotGiantBody(1e4, kColumn)), "K+H2");
    // Recorded: 4.7 × 10⁻⁴ on a 1 kPa datum, 4.7 × 10⁻³ on a 10 kPa one.
    expect([low?.negligible, high?.negligible, high?.entries[0]?.rule]).toEqual([
      true,
      false,
      "estimatedData",
    ]);
  });

  it("forms H₂⁻'s pending free–free from H₂'s and e⁻'s layers", () => {
    const verdicts = negligibleAbsorbers(hotGiantBody(1e4, undefined, 1e-9));
    const h2Minus = absorptionOf(verdicts, "H2+e-");
    expect(h2Minus?.entries.map((e) => [e.origin, e.amount.kind, e.k])).toEqual([
      ["pending", "pair", 100],
    ]);
  });
});

describe("CO₂'s held χ and fixed-g mix on a snowball-class body", () => {
  it("takes no label from the mix at an even share at 3 bar", () => {
    // Recorded: the largest u_max ÷ u_mix 0.027 (715 nm).
    expect(fixedGMixOf(negligibleAbsorbers(snowballBody(3e5)), "CO2")?.negligible).toBe(true);
  });

  it("labels a column raised past a cell's u_mix at the same share, at 150 bar", () => {
    // The whole column raised, so that the share stays even: past the tables' 10 MPa, where §16
    // places the first mix failures a body can reach. Recorded: the largest u_max ÷ u_mix 1.37.
    expect(fixedGMixOf(negligibleAbsorbers(snowballBody(1.5e7)), "CO2")?.negligible).toBe(false);
  });

  it("reads the mix at the body's self share, w 0.57 at an even share, whatever the column", () => {
    const shares = [3e5, 1.5e7].map(
      (pa) => fixedGMixOf(negligibleAbsorbers(snowballBody(pa)), "CO2")?.selfShare,
    );
    expect(shares).toEqual([expect.closeTo(0.57, 2), expect.closeTo(0.57, 2)]);
  });

  it("takes no label from CO₂–N₂'s held χ at 3 bar", () => {
    // Recorded: 4.5 × 10⁻⁴ (f₃ 0.065).
    expect(heldChiOf(negligibleAbsorbers(snowballBody(3e5)), "CO2")?.negligible).toBe(true);
  });

  it("labels CO₂–N₂'s held χ at 10 bar, past 10⁻³", () => {
    // Recorded: 4.9 × 10⁻³ (f₃ 0.21).
    expect(heldChiOf(negligibleAbsorbers(snowballBody(1e6)), "CO2")?.largestChange).toBeGreaterThan(
      NEGLIGIBLE_CHANGE,
    );
  });
});
