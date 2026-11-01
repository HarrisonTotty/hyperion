/**
 * Hand gas giants for the envelope's medium (plan R08, R08.T16.a; Design note 16): `body_detail`
 * answers in the protocol fixture's Jupiter's record form, each with a hand envelope on a profile of
 * the sim's `planetary/envelope_profile` golden.
 *
 * @remarks
 * - **The hand Jupiter** carries Jupiter's 1-bar figure and rotation, so that Design note 17's
 *   slicing runs on it, and a hand cloud stack after the fixture's own: its NH₃, NH₄SH and H₂O
 *   decks at the fixture's bases, tops, optical depths and f_sed, and its chromophore haze, each
 *   deck's sizes a narrow gamma distribution the client's optics take (the fixture's σ_g 2
 *   log-normals reach past them).
 * - **The hand ice giant** is Uranus-like: a CH₄ deck of τ 13.5, the largest an ice giant's CH₄
 *   deck holds over set A at generator version 24 (B3 k-1's 01ffeb2d5fff0001.0700), over an H₂S
 *   deck, at hand bases near the observed ones.
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

/** A hand envelope: a golden profile, a composition and its μ, and decks' modes beside hazes. */
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
    condensate_decks: decks.map(deckOf),
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

/**
 * The hand Jupiter (the module's remarks): the protocol fixture's Jupiter on Jupiter's 1-bar figure
 * and spin (its axes and its lock the fixture's Earth's, which the medium does not read), its
 * envelope the golden's Jupiter profile at the golden's gravity under the fixture's
 * composition, μ and chromophore haze, with {@link handJupiterDecks}' decks.
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
  const envelope = handEnvelope(
    golden,
    fixture.composition,
    fixture.mean_molar_mass_g_per_mol,
    handJupiterDecks(nh3FSed),
    fixture.hazes,
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
