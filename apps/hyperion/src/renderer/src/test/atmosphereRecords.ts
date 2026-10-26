/**
 * Hand records for the atmosphere's assembly and cache tests (plan R08, R08.T10.a): `body_detail`
 * answers built from the shared wire fixture (`planetaryFixture.ts`) with the surface, inventory,
 * figure or bulk a case needs, converted as the client converts a reply
 * (`lib/system/bodiesWire.ts`'s `toBodyDetail`).
 *
 * @remarks
 * Each builder states what it changes and where its figures come from. The fields a record carries
 * that the medium does not read (craters, climate, ices) are the fixture's, and say nothing about
 * the body.
 */
import type {
  AbsorberDto,
  AerosolInventoryDto,
  AerosolModeDto,
  BodyRecordDto,
  BodySurfaceDto,
  BulkPropertiesDto,
  FigureLawDto,
  ResponseFor,
} from "@hyperion/protocol";

import { toBodyDetail } from "../lib/system/bodiesWire";
import type { BodyDetail } from "../lib/system/model";
import type { Rgb } from "../view/photometry/toneCurve";
import type { BodyAtmosphereInput, BodyLight } from "../view/atmosphere/atmosphereInput";
import { MARS_AIR } from "../view/atmosphere/channels";
import { EARTH_OZONE_PROFILE, OZONE_COLUMN_DU, DOBSON_UNIT_PER_M2 } from "../view/atmosphere/earth";
import { THE_SUN } from "./lineRanges";
import { earthDetail, FIXTURE_SYSTEM, titanDetail } from "./planetaryFixture";

/** A `body_detail` answer, as the fixture gives one. */
export type DetailAnswer = ResponseFor<"body_detail">;

/** The designation the converted records carry. */
const DESIGNATION = "ATMOSPHERE TEST BODY";

/** The Sun as a body's one light: the fixture's star, `THE_SUN`'s spectrum. */
export const SUN_LIGHT: BodyLight = { star: `${FIXTURE_SYSTEM}.0100`, spectrum: THE_SUN };

/** The ground every record input takes unless a test gives its own: 0.3 per channel, R07's provisional. */
export const TEST_GROUND_ALBEDO: Rgb = [0.3, 0.3, 0.3];

/**
 * An answer converted as the client converts a reply.
 *
 * @throws Error if the conversion faults: a builder's bug.
 */
export function detailOf(answer: DetailAnswer): BodyDetail {
  const result = toBodyDetail(answer, FIXTURE_SYSTEM, DESIGNATION);
  if (result.kind !== "ok") {
    throw new Error(`a test record does not convert: ${result.fault}`);
  }
  return result.detail;
}

/** A converted record as the assembly's input, lit by the Sun unless a test gives its lights. */
export function recordInput(
  detail: BodyDetail,
  lights: ReadonlyArray<BodyLight> = [SUN_LIGHT],
  groundAlbedo: Rgb = TEST_GROUND_ALBEDO,
): BodyAtmosphereInput {
  return { kind: "record", detail, lights, groundAlbedo };
}

/** An answer's record with `change` applied. */
export function withRecord(
  answer: DetailAnswer,
  change: (record: BodyRecordDto) => BodyRecordDto,
): DetailAnswer {
  return { ...answer, record: change(answer.record) };
}

/**
 * An answer's surface, `ok`, with `change` applied.
 *
 * @throws Error for an answer whose surface is not `ok`.
 */
export function withSurface(
  answer: DetailAnswer,
  change: (surface: BodySurfaceDto) => BodySurfaceDto,
): DetailAnswer {
  const surface = answer.record.surface;
  if (surface.state !== "ok") {
    throw new Error("the answer's surface section is not ok");
  }
  return withRecord(answer, (record) => ({
    ...record,
    surface: { state: "ok", value: change(surface.value) },
  }));
}

/** An answer's surface with its inventory. */
export function withInventory(answer: DetailAnswer, inventory: AerosolInventoryDto): DetailAnswer {
  return withSurface(answer, (surface) => ({ ...surface, aerosols: inventory }));
}

/**
 * An answer's figure law, its figure section `ok`.
 *
 * @throws Error for an answer whose figure is not `ok`.
 */
export function withFigureLaw(answer: DetailAnswer, law: FigureLawDto): DetailAnswer {
  const figure = answer.record.figure;
  if (figure?.state !== "ok") {
    throw new Error("the answer's figure section is not ok");
  }
  return withRecord(answer, (record) => ({
    ...record,
    figure: { state: "ok", value: { ...figure.value, law } },
  }));
}

/** An inventory with nothing in it. */
export const EMPTY_INVENTORY: AerosolInventoryDto = { modes: [], decks: [], absorbers: [] };

/** A mode of spheres or crystals on a gamma distribution, its column mass a placeholder. */
export function gammaMode(mode: {
  readonly material: string;
  readonly phase: AerosolModeDto["phase"];
  readonly shape?: AerosolModeDto["shape"];
  readonly deck?: string;
  readonly effectiveRadiusM: number;
  readonly tau550: number;
  readonly basePa: number;
  readonly topPa: number;
}): AerosolModeDto {
  return {
    material: mode.material,
    phase: mode.phase,
    deck: mode.deck ?? null,
    shape: mode.shape ?? "sphere",
    size_distribution: {
      type: "gamma",
      effective_radius_m: mode.effectiveRadiusM,
      effective_variance: 0.1,
    },
    column_mass_kg_m2: 1e-3,
    tau_550: mode.tau550,
    base_pa: mode.basePa,
    top_pa: mode.topPa,
    f_sed: 0,
  };
}

/** A record absorber on a layer of constant mixing ratio. */
export function layerAbsorber(
  species: string,
  columnPerM2: number,
  basePa: number,
  topPa: number,
): AbsorberDto {
  return { species, column_m2: columnPerM2, base_pa: basePa, top_pa: topPa, profile: null };
}

/** An answer with the fixture's surface fields but these conditions and gases. */
function withAir(
  answer: DetailAnswer,
  air: {
    readonly state: BodySurfaceDto["state"];
    readonly temperatureK: number;
    readonly pressurePa: number;
    readonly gases: ReadonlyArray<readonly [string, number]>;
    readonly meanMolarMassGPerMol: number;
  },
): DetailAnswer {
  return withSurface(answer, (surface) => ({
    ...surface,
    state: air.state,
    mean_surface_temperature_k: air.temperatureK,
    surface_pressure_pa: air.pressurePa,
    gases: air.gases.map(([species, moleFraction]) => ({ species, mole_fraction: moleFraction })),
    mean_molar_mass_g_per_mol: air.meanMolarMassGPerMol,
  }));
}

/** An answer's bulk section and mass, with no rotation or figure: a sphere of its bulk radius. */
function withBody(
  answer: DetailAnswer,
  body: {
    readonly radiusM: number;
    readonly gravityMS2: number;
    readonly massKg: number;
    readonly bulk: BulkPropertiesDto;
  },
): DetailAnswer {
  return withRecord(answer, (record) => {
    const { rotation: _rotation, figure: _figure, ...rest } = record;
    return {
      ...rest,
      mass_kg: { state: "ok", value: body.massKg },
      bulk: {
        state: "ok",
        value: { ...body.bulk, radius_m: body.radiusM, surface_gravity_m_s2: body.gravityMS2 },
      },
    };
  });
}

/** The fixture Earth's bulk section, which the hand bodies take with their own radius and gravity. */
function earthBulk(): BulkPropertiesDto {
  const bulk = earthDetail().record.bulk;
  if (bulk.state !== "ok") {
    throw new Error("the fixture Earth has no bulk section");
  }
  return bulk.value;
}

/**
 * The CO₂ and N₂ of a Venus-class air (NSSDCA's Venus fact sheet, D. R. Williams, 2024: 96.5% CO₂,
 * 3.5% N₂), and its mean molar mass from CO₂'s 44.0095 and N₂'s 28.0134 g mol⁻¹ (CIAAW 2021).
 */
const VENUS_GASES: ReadonlyArray<readonly [string, number]> = [
  ["CO2", 0.965],
  ["N2", 0.035],
];
const VENUS_MEAN_MOLAR_MASS_G_PER_MOL = 0.965 * 44.0095 + 0.035 * 28.0134;

/**
 * A Venus-class record (Design note 16's two Venus cases): NSSDCA's 6,051.8 km, 8.87 m s⁻² and
 * 4.8673 × 10²⁴ kg, its 737 K mean surface temperature, and the given surface pressure (the real
 * 92 bar, or plan 14's 58), its air {@link VENUS_GASES}, with no rotation or figure section and no
 * inventory sent (`not_modelled`).
 */
function venusDetail(pressurePa: number): BodyDetail {
  const body = withBody(earthDetail(), {
    radiusM: 6_051_800,
    gravityMS2: 8.87,
    massKg: 4.8673e24,
    bulk: earthBulk(),
  });
  return detailOf(
    withAir(body, {
      state: "runaway_greenhouse",
      temperatureK: 737,
      pressurePa,
      gases: VENUS_GASES,
      meanMolarMassGPerMol: VENUS_MEAN_MOLAR_MASS_G_PER_MOL,
    }),
  );
}

/** {@link venusDetail} at the real 92 bar (9.2 MPa). */
export function venus92Detail(): BodyDetail {
  return venusDetail(9.2e6);
}

/** {@link venusDetail} at plan 14's 58 bar (5.8 MPa). */
export function venus58Detail(): BodyDetail {
  return venusDetail(5.8e6);
}

/**
 * A tenuous Pluto-like record with a tholin haze: NSSDCA's Pluto fact sheet's 1,188.3 km,
 * 0.62 m s⁻² and 1.303 × 10²² kg; about 1 Pa of N₂ with 0.5% CH₄ at 40 K (New Horizons: about
 * 11 µbar at the surface, G. R. Gladstone et al., Science 351 (2016) aad8866; CH₄ 0.25–0.6%,
 * E. Lellouch et al., Icarus 286 (2017) 289); and one haze mode of tholin aggregates, monomers of
 * 10 nm, 1,000 to an aggregate at D_f 2 (W. Cheng et al., Icarus 290 (2017) 112: monomers of about
 * 10 nm), τ(550) 0.01, well mixed from the ground to 10⁻³ Pa.
 */
export function plutoHazeDetail(): BodyDetail {
  const body = withBody(titanDetail(), {
    radiusM: 1_188_300,
    gravityMS2: 0.62,
    massKg: 1.303e22,
    bulk: earthBulk(),
  });
  const air = withAir(body, {
    state: "tenuous",
    temperatureK: 40,
    pressurePa: 1,
    gases: [
      ["N2", 0.995],
      ["CH4", 0.005],
    ],
    meanMolarMassGPerMol: 0.995 * 28.0134 + 0.005 * 16.0425,
  });
  return detailOf(
    withInventory(air, {
      modes: [
        {
          material: "tholin",
          phase: "solid",
          deck: null,
          shape: "aggregate",
          size_distribution: {
            type: "aggregate",
            monomer_radius_m: 1e-8,
            monomer_count: 1_000,
            fractal_dimension: 2,
            // optool's and Tazaki's k_f = (5 ÷ 3)^(D_f ÷ 2), at D_f 2 (`aggregate.ts`).
            fractal_prefactor: 5 / 3,
          },
          column_mass_kg_m2: 1e-6,
          tau_550: 0.01,
          base_pa: 1,
          top_pa: 1e-3,
          f_sed: 0,
        },
      ],
      decks: [],
      absorbers: [],
    }),
  );
}

/**
 * The generated Earth with a key this client does not know in each keyed member: a gas `Xq`
 * (1% of the air, taken from N₂), an absorber `Zq2`, and a haze mode of a material `newhaze` in a
 * deck `newdeck`, which its inventory lists.
 */
export function unknownKeysDetail(): BodyDetail {
  const air = withSurface(earthDetail(), (surface) => ({
    ...surface,
    gases: [
      { species: "N2", mole_fraction: 0.9705 },
      { species: "H2O", mole_fraction: 0.0102 },
      { species: "Xq", mole_fraction: 0.01 },
      { species: "Ar", mole_fraction: 0.009 },
      { species: "CO2", mole_fraction: 0.0003 },
    ],
  }));
  return detailOf(
    withInventory(air, {
      modes: [
        gammaMode({
          material: "newhaze",
          phase: "solid",
          deck: "newdeck",
          effectiveRadiusM: 5e-7,
          tau550: 0.05,
          basePa: 50_000,
          topPa: 20_000,
        }),
      ],
      decks: [{ condensate: "newdeck", vapour_mole_fraction_above: 0 }],
      absorbers: [layerAbsorber("Zq2", 1e20, 50_000, 10_000)],
    }),
  );
}

/** Earth's air column, molecules m⁻²: p_s ÷ (μ m_u g), 101,325 Pa at 28.97 g mol⁻¹ and 9.80665 m s⁻². */
const EARTH_AIR_COLUMN_PER_M2 = 101_325 / ((28.97 / 6.022_140_76e26) * 9.806_65);

/**
 * The absorbers of {@link sixProfilesDetail}, six vertical profiles on an air of N₂ and Ar:
 * the measured ozone profile's 300 DU (`earth.ts`), CH₄ at 1.8 ppm to 200 hPa, SO₂ at 10²¹ m⁻²
 * over 800–300 hPa, H₂O at AFGL's 1.42 g cm⁻² to 300 hPa (its two continuum pairs on that layer),
 * O₂ at 21% through the column (O₂–O₂ and O₂–N₂ on it), and H₂ and He at 10²⁶ m⁻² each over
 * 500–100 hPa (H₂–H₂ and H₂–He there, the profile of least optical depth).
 */
export function sixProfilesInventory(): AerosolInventoryDto {
  const ozoneBasePa = EARTH_OZONE_PROFILE[0]?.pressurePa ?? Number.NaN;
  const ozoneTopPa = EARTH_OZONE_PROFILE.at(-1)?.pressurePa ?? Number.NaN;
  // The wire's profile is relative to its peak, in [0, 1].
  const ozonePeak = Math.max(...EARTH_OZONE_PROFILE.map((node) => node.mixingRatio));
  return {
    modes: [],
    decks: [],
    absorbers: [
      {
        species: "O3",
        column_m2: OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2,
        base_pa: ozoneBasePa,
        top_pa: ozoneTopPa,
        profile: EARTH_OZONE_PROFILE.map(({ pressurePa, mixingRatio }) => ({
          pressure_pa: pressurePa,
          relative_mixing_ratio: mixingRatio / ozonePeak,
        })),
      },
      layerAbsorber("CH4", 1.8e-6 * EARTH_AIR_COLUMN_PER_M2, 101_325, 20_000),
      layerAbsorber("SO2", 1e21, 80_000, 30_000),
      layerAbsorber("H2O", 4.75e26, 101_325, 30_000),
      // The wire's tops are above 0: 10⁻³ Pa, above the column's 10⁻⁷ p_s.
      layerAbsorber("O2", 0.21 * EARTH_AIR_COLUMN_PER_M2, 101_325, 1e-3),
      layerAbsorber("H2", 1e26, 50_000, 10_000),
      layerAbsorber("He", 1e26, 50_000, 10_000),
    ],
  };
}

/** The generated Earth's surface on an air of N₂ and Ar, with {@link sixProfilesInventory}. */
export function sixProfilesDetail(): BodyDetail {
  return detailOf(sixProfilesAnswer());
}

/** {@link sixProfilesDetail}'s answer. */
function sixProfilesAnswer(): DetailAnswer {
  const air = withSurface(earthDetail(), (surface) => ({
    ...surface,
    gases: [
      { species: "N2", mole_fraction: 0.99 },
      { species: "Ar", mole_fraction: 0.01 },
    ],
    mean_molar_mass_g_per_mol: 0.99 * 28.0134 + 0.01 * 39.948,
  }));
  return withInventory(air, sixProfilesInventory());
}

/**
 * {@link sixProfilesDetail} with three haze modes of liquid H₂SO₄ (τ(550) 0.5, 0.3 and 10⁻¹²), so
 * that its five kept absorber profiles and three modes pass the medium's seven places beside the
 * molecular term, and the mode of least depth, under any kept profile's, is left out.
 */
export function pastTheCapDetail(): BodyDetail {
  return detailOf(pastTheCapAnswer([]));
}

/**
 * {@link pastTheCapDetail} with free electrons too, 10¹⁰ m⁻² through the column: Thomson's σ_T U,
 * 6.7 × 10⁻¹⁹, the least of its terms with the thinnest haze's, so that both are left out.
 */
export function pastTheCapWithElectronsDetail(): BodyDetail {
  return detailOf(pastTheCapAnswer([layerAbsorber("e-", 1e10, 101_325, 1e-3)]));
}

/** {@link pastTheCapDetail}'s answer, with these absorbers beside the six profiles'. */
function pastTheCapAnswer(more: ReadonlyArray<AbsorberDto>): DetailAnswer {
  const answer = sixProfilesAnswer();
  const inventory = sixProfilesInventory();
  const acid = (tau550: number, basePa: number, topPa: number): AerosolModeDto =>
    gammaMode({
      material: "H2SO4",
      phase: "liquid",
      effectiveRadiusM: 1e-6,
      tau550,
      basePa,
      topPa,
    });
  return withInventory(answer, {
    ...inventory,
    absorbers: [...inventory.absorbers, ...more],
    modes: [acid(0.5, 30_000, 10_000), acid(0.3, 60_000, 30_000), acid(1e-12, 90_000, 60_000)],
  });
}

/**
 * The generated Earth with free electrons, 10²⁵ m⁻² through the column (σ_T U = 6.7 × 10⁻⁴): a
 * Thomson term on e⁻'s layer (R08.T4.c).
 */
export function electronsDetail(): BodyDetail {
  return detailOf(
    withInventory(earthDetail(), {
      ...EMPTY_INVENTORY,
      absorbers: [layerAbsorber("e-", 1e25, 101_325, 1e-3)],
    }),
  );
}

/**
 * Mars's air number column at the fixture's figures, molecules m⁻²: p_s ÷ (μ m_u g), 636 Pa at
 * 43.49 g mol⁻¹ and 3.71 m s⁻² (`channels.ts`'s `MARS_FIT_COLUMN`).
 */
const MARS_AIR_COLUMN_PER_M2 = 636 / ((43.49 / 6.022_140_76e26) * 3.71);

/**
 * A Mars-like record: NASA's Mars Fact Sheet's figures as `channels.ts`'s `MARS_FIT_COLUMN` takes
 * them (636 Pa, about 214 K, 43.49 g mol⁻¹, 3.71 m s⁻², 3,389.5 km) and 6.4171 × 10²³ kg, its air
 * `channels.ts`'s {@link MARS_AIR}, no rotation or figure section and an empty inventory; its CO
 * as a record absorber at `coColumnFactor` times the gas's column where the factor is not 1, so
 * that its pending overtones pass their bound (the bound's own test: a million times Mars's CO).
 */
export function marsLikeDetail(coColumnFactor = 1): BodyDetail {
  const body = withBody(earthDetail(), {
    radiusM: 3_389_500,
    gravityMS2: 3.71,
    massKg: 6.4171e23,
    bulk: earthBulk(),
  });
  const air = withAir(body, {
    state: "temperate",
    temperatureK: 214,
    pressurePa: 636,
    gases: MARS_AIR.map(({ species, moleFraction }) => [species, moleFraction] as const),
    meanMolarMassGPerMol: 43.49,
  });
  const coFraction = MARS_AIR.find((g) => g.species === "CO")?.moleFraction ?? Number.NaN;
  return detailOf(
    withInventory(air, {
      ...EMPTY_INVENTORY,
      absorbers:
        coColumnFactor === 1
          ? []
          : [layerAbsorber("CO", coColumnFactor * coFraction * MARS_AIR_COLUMN_PER_M2, 636, 1e-4)],
    }),
  );
}

/**
 * An Earth-class record with line species: the generated Earth's surface with O₂ at 20.95%
 * (N₂ 77.08%, H₂O 1.02%, Ar 0.92%, CO₂ 0.03%), μ from CIAAW 2021's molar masses, no inventory.
 */
export function earthClassLinesDetail(): BodyDetail {
  const gases: ReadonlyArray<readonly [string, number]> = [
    ["N2", 0.7708],
    ["O2", 0.2095],
    ["H2O", 0.0102],
    ["Ar", 0.0092],
    ["CO2", 0.0003],
  ];
  const molar: Readonly<Record<string, number>> = {
    N2: 28.0134,
    O2: 31.9988,
    H2O: 18.0153,
    Ar: 39.948,
    CO2: 44.0095,
  };
  return detailOf(
    withSurface(earthDetail(), (surface) => ({
      ...surface,
      gases: gases.map(([species, moleFraction]) => ({ species, mole_fraction: moleFraction })),
      mean_molar_mass_g_per_mol: gases.reduce(
        (sum, [species, x]) => sum + x * (molar[species] ?? Number.NaN),
        0,
      ),
    })),
  );
}
