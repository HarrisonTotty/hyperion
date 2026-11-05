/**
 * Hand gas giants for the envelope's medium (plan R08, R08.T16.a; Design note 16): `body_detail`
 * answers in the protocol fixture's Jupiter's record form, each with a hand envelope on a profile of
 * the sim's `planetary/envelope_profile` golden.
 *
 * @remarks
 * - **The hand Jupiter** carries Jupiter's 1-bar figure and rotation, so that Design note 17's
 *   slicing runs on it, and a hand cloud stack after the fixture's own: its NH₃, NH₄SH and H₂O
 *   decks at the fixture's bases, tops, optical depths and f_sed, each deck's sizes a narrow gamma
 *   distribution the client's optics take (the fixture's σ_g 2 log-normals reach past them).
 * - **The cold giants' haze and chromophore** (fix-p14-giant-chromophore): the hand Jupiter and
 *   Saturn carry the generator's NH₃-ice haze, their NH₃ deck's second mode, and its chromophore,
 *   each the fixture's Jupiter's mode under the rules the sim holds for generator version 30
 *   (`photochem::CHROMOPHORE_MIXING_VERSION`) on the hand giant's own NH₃ deck: the haze from the
 *   deck's base to 0.1 bar at τ(550) {@link AMMONIA_HAZE_TAU_550}, and the chromophore
 *   {@link CHROMOPHORE_MIXING_RATIO} of the gas the haze spans, at most
 *   {@link CHROMOPHORE_MOST_KG_M2} ({@link coldGiantHazes}).
 * - **The hand ice giant** is Uranus-like: a CH₄ deck of τ 13.5, the largest an ice giant's CH₄
 *   deck holds over set A at generator version 24 (B3 k-1's 01ffeb2d5fff0001.0700), over an H₂S
 *   deck, at hand bases near the observed ones.
 * - **The hand Saturn** is the sim's own Saturn as `planetary/envelope_atmosphere`'s golden draws it
 *   at generator version 24: its composition, μ, H₂O, NH₄SH and NH₃ decks, the decks at the hand
 *   Jupiter's 1 µm (the golden's are 28–100 µm, past the optics' budget).
 *
 * The fields a record carries that the medium does not read are the fixture's, and say nothing of
 * the body.
 */
import type {
  AerosolModeDto,
  BodyEnvelopeDto,
  BodyRecordDto,
  CondensateDeckDto,
  GasShareDto,
} from "@hyperion/protocol";

import type { BodyDetail } from "../lib/system/model";
import { type DetailAnswer, detailOf, withRecord } from "./atmosphereRecords";
import { type GoldenEnvelope, goldenEnvelope } from "./envelopeProfileGolden";
import { earthDetail, jupiterDetail } from "./planetaryFixture";

/**
 * Jupiter's 1-bar equatorial radius, m: 71,492 ± 4 km (Archinal et al. 2018, Celest. Mech. Dyn.
 * Astron. 130, 22, Table 4).
 */
export const JUPITER_EQUATORIAL_RADIUS_M = 71_492e3;

/** Jupiter's 1-bar polar radius, m: 66,854 ± 10 km (Archinal et al. 2018, Table 4). */
export const JUPITER_POLAR_RADIUS_M = 66_854e3;

/**
 * Jupiter's spin, rad s⁻¹: System III's W = 284.95° + 870.536° d⁻¹ (Archinal et al. 2018, Table 2),
 * a period of 9 h 55 m 29.7 s.
 */
export const JUPITER_SPIN_RAD_S = (870.536 * Math.PI) / 180 / 86_400;

/**
 * Jupiter's moment of inertia over M a², the wire's `moment_of_inertia_factor`, which the
 * `rotational` law does not read: Ni 2018's (A&A 613, A32, §4) C ÷ M R² of 0.2756 over the mean
 * radius of 69,911 km, carried to the equatorial 71,492 km, 0.2756 × (69,911 ÷ 71,492)², 0.2635.
 */
const JUPITER_MOMENT_OF_INERTIA_FACTOR = 0.2756 * (69_911 / 71_492) ** 2;

/** An envelope's hand profile from a golden one: its grey parameters, its join and its adiabat. */
function profileOf(
  golden: GoldenEnvelope,
): Pick<
  BodyEnvelopeDto,
  | "internal_temperature_k"
  | "irradiation_temperature_k"
  | "profile"
  | "join_pressure_pa"
  | "adiabat"
> {
  const { profile } = golden;
  return {
    internal_temperature_k: profile.internalK,
    irradiation_temperature_k: profile.irradiationK,
    profile: {
      thermal_opacity_m2_per_kg: profile.grey.thermalOpacityM2PerKg,
      optical_depth_exponent: profile.grey.opticalDepthExponent,
      opacity_ratio: profile.grey.opacityRatio,
      redistribution_factor: profile.grey.redistributionFactor,
    },
    join_pressure_pa: profile.nodes[0]?.pressurePa ?? Number.NaN,
    adiabat: profile.nodes.map(({ pressurePa, temperatureK }) => ({
      pressure_pa: pressurePa,
      temperature_k: temperatureK,
    })),
  };
}

/**
 * A deck's mode of a narrow gamma distribution: `material` of `phase` and `shape`, of effective
 * radius `effectiveRadiusM` (variance 0.1), τ(550) `tau550` from `basePa` up to `topPa` by the
 * exponent `fSed`, over the whole sky.
 */
export function deckMode(
  material: string,
  phase: AerosolModeDto["phase"],
  shape: AerosolModeDto["shape"],
  effectiveRadiusM: number,
  tau550: number,
  [basePa, topPa]: readonly [number, number],
  fSed: number,
  temperatureK: number,
): AerosolModeDto {
  return {
    material,
    phase,
    deck: material,
    shape,
    size_distribution: {
      type: "gamma",
      effective_radius_m: effectiveRadiusM,
      effective_variance: 0.1,
    },
    column_mass_kg_m2: 0.01,
    tau_550: tau550,
    tau_basis: "sphere",
    base_pa: basePa,
    top_pa: topPa,
    f_sed: fSed,
    coverage: 1,
    temperature_k: temperatureK,
  };
}

/** The deck a mode names, its vapour 10⁻⁴ below and 10⁻⁶ above it. */
function deckOf(mode: AerosolModeDto): CondensateDeckDto {
  return {
    condensate: mode.material,
    phase: mode.phase,
    base_pa: mode.base_pa,
    base_temperature_k: mode.temperature_k ?? Number.NaN,
    top_pa: mode.top_pa,
    vapour_mole_fraction_below: 1e-4,
    vapour_mole_fraction_above: 1e-6,
    f_sed: mode.f_sed,
    extrapolated: false,
  };
}

/**
 * A hand envelope: a golden profile, a composition and its μ, and decks' modes beside hazes, one
 * condensate deck for each deck the modes name (its first mode's).
 */
function handEnvelope(
  golden: GoldenEnvelope,
  composition: ReadonlyArray<GasShareDto>,
  meanMolarMassGPerMol: number,
  decks: ReadonlyArray<AerosolModeDto>,
  hazes: ReadonlyArray<AerosolModeDto>,
): BodyEnvelopeDto {
  return {
    supercritical: null,
    ...profileOf(golden),
    composition: [...composition],
    mean_molar_mass_g_per_mol: meanMolarMassGPerMol,
    heat_capacity_over_r: 3.5,
    decks: [...decks],
    hazes: [...hazes],
    haze_tau_550: hazes.reduce((sum, mode) => sum + (mode.tau_550 ?? 0), 0),
    condensate_decks: decks
      .filter((mode, i) => decks.findIndex((m) => m.deck === mode.deck) === i)
      .map(deckOf),
    absorbers: [],
    not_formed: [],
  };
}

/** A record's envelope section replaced, and its bulk gravity set to `gravityMS2`. */
function withEnvelope(
  record: BodyRecordDto,
  envelope: BodyEnvelopeDto,
  gravityMS2: number,
): BodyRecordDto {
  if (record.bulk.state !== "ok") {
    throw new Error("the fixture's giant has its bulk section");
  }
  return {
    ...record,
    bulk: { state: "ok", value: { ...record.bulk.value, surface_gravity_m_s2: gravityMS2 } },
    envelope: { state: "ok", value: envelope },
  };
}

/** The protocol fixture's Jupiter's envelope, which the hand Jupiter takes its gases from. */
function fixtureEnvelope(): BodyEnvelopeDto {
  const envelope = jupiterDetail().record.envelope;
  if (envelope?.state !== "ok") {
    throw new Error("the fixture's Jupiter has its envelope");
  }
  return envelope.value;
}

/**
 * The top of the cold giants' NH₃-ice haze, Pa: the sim's `photochem::AMMONIA_HAZE_TOP`, 0.1 bar.
 */
const AMMONIA_HAZE_TOP_PA = 1e4;

/**
 * The NH₃-ice haze's τ(550) under 30's rules, its layer spanning an e-fold of pressure or more: the
 * sim's `photochem::AMMONIA_HAZE_OPTICAL_DEPTH_MIXED`, 7, fitted with the chromophore's mixing on
 * Jupiter's measured Bond albedo.
 */
export const AMMONIA_HAZE_TAU_550 = 7;

/**
 * The chromophore's mass mixing ratio in the gas its haze spans under 30's rules, kg per kg: the
 * sim's `photochem::CHROMOPHORE_MIXING_RATIO`, 7 × 10⁻⁸.
 */
export const CHROMOPHORE_MIXING_RATIO = 7e-8;

/**
 * The most chromophore a cold giant takes, kg m⁻²: the sim's `photochem::CHROMOPHORE_COLUMN`,
 * 6.3 × 10⁻⁴, Braude et al.'s (2020, Icarus 338, 113589, Table 4) Great Red Spot.
 */
export const CHROMOPHORE_MOST_KG_M2 = 6.3e-4;

/**
 * A cold giant's NH₃-ice haze and chromophore under 30's rules (the module's remarks) on its NH₃
 * deck's mode `ammonia` under gravity `gravityMS2`: the fixture's Jupiter's two modes, the haze
 * from the deck's base and at its temperature to {@link AMMONIA_HAZE_TOP_PA} at τ(550)
 * {@link AMMONIA_HAZE_TAU_550}, and the chromophore of {@link CHROMOPHORE_MIXING_RATIO} times
 * (`p_base` − 0.1 bar) ÷ g, at most {@link CHROMOPHORE_MOST_KG_M2}, its τ(550) its column's at the
 * fixture's mass extinction; its peak and width the fixture's. The layer spans more than an e-fold
 * on both hand giants, so the haze takes its whole τ.
 *
 * @throws Error if the fixture's Jupiter lacks either mode, or the deck spans under an e-fold or has
 *   no temperature.
 */
export function coldGiantHazes(
  ammonia: AerosolModeDto,
  gravityMS2: number,
): { readonly haze: AerosolModeDto; readonly chromophore: AerosolModeDto } {
  const fixture = fixtureEnvelope();
  const haze = fixture.decks.find(
    (mode) => mode.deck === "NH3" && mode.top_pa === AMMONIA_HAZE_TOP_PA,
  );
  const chromophore = fixture.hazes.find((mode) => mode.material === "chromophore");
  if (haze === undefined || chromophore === undefined) {
    throw new Error("the fixture's Jupiter carries its NH₃ haze and its chromophore");
  }
  if (!(Math.log(ammonia.base_pa / AMMONIA_HAZE_TOP_PA) >= 1)) {
    throw new Error("the hand giant's NH₃ deck spans an e-fold of pressure to the haze's top");
  }
  const hazeTau = haze.tau_550 ?? Number.NaN;
  const temperatureK = ammonia.temperature_k;
  if (temperatureK === undefined) {
    throw new Error("the hand giant's NH₃ deck has its base temperature");
  }
  const column = Math.min(
    (CHROMOPHORE_MIXING_RATIO * (ammonia.base_pa - AMMONIA_HAZE_TOP_PA)) / gravityMS2,
    CHROMOPHORE_MOST_KG_M2,
  );
  const extinction =
    (chromophore.tau_550 ?? Number.NaN) / (chromophore.column_mass_kg_m2 ?? Number.NaN);
  return {
    haze: {
      ...haze,
      column_mass_kg_m2: ((haze.column_mass_kg_m2 ?? Number.NaN) * AMMONIA_HAZE_TAU_550) / hazeTau,
      tau_550: AMMONIA_HAZE_TAU_550,
      base_pa: ammonia.base_pa,
      temperature_k: temperatureK,
    },
    chromophore: { ...chromophore, column_mass_kg_m2: column, tau_550: column * extinction },
  };
}

/**
 * The hand Jupiter's decks (the module's remarks): NH₃ ice at 0.6415–0.1081 bar, τ 2.39 and f_sed
 * 368; NH₄SH at 2.128–1.130 bar, τ 1.60 and f_sed 954; and liquid H₂O at 4.873–1.771 bar, τ 1.73
 * and f_sed 7,132, the fixture's; at 1 µm of effective radius, a hand size inside the 0.3–15 µm of
 * Jupiter's measured cloud particles (Sromovsky and Fry 2010, Icarus 210, 211, its abstract), the
 * water's too, though its cloud's are tens of µm, since a droplet of 10 µm costs the optics about
 * 29 s and one of 1 µm under one.
 */
export function handJupiterDecks(nh3FSed = 367.863_608_190_027_3): ReadonlyArray<AerosolModeDto> {
  return [
    deckMode(
      "H2O",
      "liquid",
      "sphere",
      1e-6,
      1.729_128_673_575_883_7,
      [487_260.57, 177_089.48],
      7_132.2,
      278.16,
    ),
    deckMode(
      "NH4SH",
      "solid",
      "crystal",
      1e-6,
      1.595_328_332_236_391,
      [212_776.17, 112_982.73],
      954.39,
      215.77,
    ),
    deckMode(
      "NH3",
      "solid",
      "crystal",
      1e-6,
      2.394_185_524_685_682_2,
      [64_149.773_299_822_13, 10_807.741_278_364_832],
      nh3FSed,
      149.16,
    ),
  ];
}

/** The NH₃ deck's mode among `decks`. */
function ammoniaOf(decks: ReadonlyArray<AerosolModeDto>): AerosolModeDto {
  const ammonia = decks.find((mode) => mode.deck === "NH3");
  if (ammonia === undefined) {
    throw new Error("a hand cold giant has its NH₃ deck");
  }
  return ammonia;
}

/**
 * The hand Jupiter (the module's remarks): the protocol fixture's Jupiter on Jupiter's 1-bar figure
 * and spin (its axes and its lock the fixture's Earth's, which the medium does not read), its
 * envelope the golden's Jupiter profile at the golden's gravity under the fixture's composition
 * and μ, with {@link handJupiterDecks}' decks and its NH₃ haze and chromophore under 30's rules
 * ({@link coldGiantHazes}): the haze from 0.6415 bar, the chromophore 1.53 × 10⁻⁴ kg m⁻².
 *
 * @param nh3FSed - The NH₃ deck's exponent: the fixture's sheet by default.
 */
export function handJupiterDetail(nh3FSed?: number): DetailAnswer {
  const golden = goldenEnvelope("jupiter");
  const fixture = fixtureEnvelope();
  const { rotation: earthRotation, figure: earthFigure } = earthDetail().record;
  if (earthRotation?.state !== "ok" || earthFigure?.state !== "ok") {
    throw new Error("the fixture's Earth has its rotation and figure");
  }
  const decks = handJupiterDecks(nh3FSed);
  const { haze, chromophore } = coldGiantHazes(ammoniaOf(decks), golden.profile.gravityMS2);
  const envelope = handEnvelope(
    golden,
    fixture.composition,
    fixture.mean_molar_mass_g_per_mol,
    [...decks, haze],
    [chromophore],
  );
  return withRecord(jupiterDetail(), (record) => ({
    ...withEnvelope(record, envelope, golden.profile.gravityMS2),
    figure: {
      state: "ok",
      value: {
        equatorial_radius_m: JUPITER_EQUATORIAL_RADIUS_M,
        polar_radius_m: JUPITER_POLAR_RADIUS_M,
        flattening: 1 - JUPITER_POLAR_RADIUS_M / JUPITER_EQUATORIAL_RADIUS_M,
        pole: earthFigure.value.pole,
        moment_of_inertia_factor: JUPITER_MOMENT_OF_INERTIA_FACTOR,
        law: "rotational",
        datum: "one_bar",
      },
    },
    rotation: {
      state: "ok",
      value: { ...earthRotation.value, initial_rate_rad_s: JUPITER_SPIN_RAD_S },
    },
  }));
}

/**
 * The hand Jupiter's record as the client converts a reply ({@link handJupiterDetail}'s): what
 * `ATMOSPHERE TEST`'s `TEST JUPITER` gives the atmosphere cache, which builds it as a server body's
 * (R08.T16.a.2).
 */
export function handJupiterRecord(): BodyDetail {
  return detailOf(handJupiterDetail());
}

/**
 * The sim's Saturn's composition at 1 bar by mole fraction, largest first: the 15 gases
 * `planetary/envelope_atmosphere`'s golden gives it (generator version 24), which sum to 1 within
 * 10⁻¹⁵, inside the wire's 10⁻⁹.
 */
const SATURN_COMPOSITION: ReadonlyArray<GasShareDto> = [
  { species: "H2", mole_fraction: 0.919_272_319_373_326_8 },
  { species: "He", mole_fraction: 0.074_214_490_450_354_04 },
  { species: "CH4", mole_fraction: 0.006_255_273_657_226_346 },
  { species: "NH3", mole_fraction: 0.000_166_277_348_709_187_9 },
  { species: "Ar", mole_fraction: 5.816_180_748_636_967e-5 },
  { species: "N2", mole_fraction: 2.713_194_489_048_405_4e-5 },
  { species: "PH3", mole_fraction: 5.304_419_886_410_08e-6 },
  { species: "HCl", mole_fraction: 9.012_814_605_964_491e-7 },
  { species: "Ne", mole_fraction: 1.001_384_369_870_857_8e-7 },
  { species: "Kr", mole_fraction: 3.504_595_464_432_328_6e-8 },
  { species: "Xe", mole_fraction: 3.424_821_132_236_196_4e-9 },
  { species: "CO", mole_fraction: 1.021_305_650_163_863_2e-9 },
  { species: "H2O", mole_fraction: 5.571_145_604_898_302e-11 },
  { species: "CO2", mole_fraction: 2.524_073_825_600_432_6e-11 },
  { species: "HCN", mole_fraction: 5.189_020_840_622_026e-12 },
];

/** The sim's Saturn's mean molar mass, g mol⁻¹: 2.2568, the golden's. */
const SATURN_MEAN_MOLAR_MASS_G_PER_MOL = 2.256_821_101_118_976_3;

/** The hand Saturn's decks (its remarks), each at 1 µm. */
function handSaturnDecks(): ReadonlyArray<AerosolModeDto> {
  return [
    deckMode(
      "H2O",
      "liquid",
      "sphere",
      1e-6,
      0.628_268_955_982_142_5,
      [1_095_594.921_249_072, 505_375.818_629_654_5],
      96_363.329_669_900_98,
      311.927_238_470_180_9,
    ),
    deckMode(
      "NH4SH",
      "solid",
      "crystal",
      1e-6,
      0.597_289_752_979_824_1,
      [414_508.394_118_045_5, 251_188.643_150_957_53],
      11_615.199_485_369_712,
      233.869_571_660_028_9,
    ),
    deckMode(
      "NH3",
      "solid",
      "crystal",
      1e-6,
      0.962_560_241_342_463,
      [127_164.230_628_933_7, 50_818.763_805_015_63],
      4_584.518_181_497_106,
      161.856_562_601_728_06,
    ),
  ];
}

/**
 * The hand Saturn (the module's remarks): the fixture's Jupiter's record with the golden's Saturn
 * profile at its gravity, the sim's Saturn's gases and μ ({@link SATURN_COMPOSITION}), its decks
 * at the golden's bases, tops, base temperatures, optical depths and f_sed (liquid H₂O at
 * 10.96–5.054 bar, τ 0.628; NH₄SH at 4.145–2.512 bar, τ 0.597; NH₃ ice at 1.272–0.5082 bar,
 * τ 0.963), each at 1 µm, and its NH₃ haze and chromophore under 30's rules ({@link coldGiantHazes}):
 * the haze from 1.272 bar, the chromophore at its bound, 6.3 × 10⁻⁴ kg m⁻² (0.2568–0.1558 bar,
 * τ 0.307); Saturn's 1-bar radius.
 */
export function handSaturnDetail(): DetailAnswer {
  const golden = goldenEnvelope("saturn");
  const decks = handSaturnDecks();
  const { haze, chromophore } = coldGiantHazes(ammoniaOf(decks), golden.profile.gravityMS2);
  const envelope = handEnvelope(
    golden,
    SATURN_COMPOSITION,
    SATURN_MEAN_MOLAR_MASS_G_PER_MOL,
    [...decks, haze],
    [chromophore],
  );
  return withRecord(jupiterDetail(), (record) => {
    const giant = withEnvelope(record, envelope, golden.profile.gravityMS2);
    if (giant.bulk.state !== "ok") {
      throw new Error("the fixture's giant has its bulk section");
    }
    // Saturn's 1-bar equatorial radius, 60,268 ± 4 km (Archinal et al. 2018, Table 4).
    return { ...giant, bulk: { state: "ok", value: { ...giant.bulk.value, radius_m: 60_268e3 } } };
  });
}

/** The hand ice giant's CH₄ deck's τ(550): 13.5, B3 k-1's 01ffeb2d5fff0001.0700's. */
export const ICE_GIANT_METHANE_TAU = 13.5;

/**
 * The hand ice giant (the module's remarks): the fixture's Jupiter's record with the golden's
 * Uranus profile at its gravity; H₂, He and CH₄ at Uranus's 0.825, 0.152 and 0.023 (the helium
 * Conrath et al. 1987's, JGR 92, 15003; CH₄ at the deep 2.3%, Lindal et al. 1987, JGR 92, 14987);
 * a CH₄ ice deck of τ 13.5 from 1.2 bar,
 * near the observed cloud's 1.2–1.3 bar (Sromovsky et al. 2011, Icarus 215, 292), up to 0.6 bar
 * at f_sed 10, over an H₂S ice deck of τ 5 from 4 bar up to 2.5 bar; Uranus's 1-bar radius.
 */
export function handIceGiantDetail(): DetailAnswer {
  const golden = goldenEnvelope("uranus");
  const composition: ReadonlyArray<GasShareDto> = [
    { species: "H2", mole_fraction: 0.825 },
    { species: "He", mole_fraction: 0.152 },
    { species: "CH4", mole_fraction: 0.023 },
  ];
  const meanMolarMassGPerMol = 0.825 * 2.016 + 0.152 * 4.003 + 0.023 * 16.043;
  const envelope = handEnvelope(
    golden,
    composition,
    meanMolarMassGPerMol,
    [
      deckMode("H2S", "solid", "crystal", 1e-6, 5, [4e5, 2.5e5], 10, 120),
      deckMode("CH4", "solid", "crystal", 1.5e-6, ICE_GIANT_METHANE_TAU, [1.2e5, 0.6e5], 10, 80),
    ],
    [],
  );
  return withRecord(jupiterDetail(), (record) => {
    const giant = withEnvelope(record, envelope, golden.profile.gravityMS2);
    if (giant.bulk.state !== "ok") {
      throw new Error("the fixture's giant has its bulk section");
    }
    // Uranus's 1-bar equatorial radius, 25,559 ± 4 km (Archinal et al. 2018, Table 4).
    return { ...giant, bulk: { state: "ok", value: { ...giant.bulk.value, radius_m: 25_559e3 } } };
  });
}
