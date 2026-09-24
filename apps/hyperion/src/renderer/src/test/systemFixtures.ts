/**
 * Builders for the `SYSTEM` display's server messages, each a copy of a wire-form pin in
 * `crates/hyperion-protocol/src/`, so that the client is tested against what the server is pinned
 * to send.
 *
 * @remarks
 * `stellar.rs` pins the summary of a white dwarf with a Sun-like companion (`summary`), a system not
 * yet born, a black hole, the remnants' forms and the `unknown_system` error; `orbit.rs` pins a
 * single star's hierarchy and a hierarchical triple; `planetary.rs` pins the section tags. Each
 * builder names the pin it copies. Overrides make the variations a test needs from them.
 */
import type {
  HierarchyDto,
  OrbitDto,
  RequestError,
  ResponseFor,
  SectionDto,
  StarSummaryDto,
  SystemSummaryDto,
  UniverseTime,
} from "@hyperion/protocol";

import type { LayerBand } from "../displays/galaxy/chartModel";
import type { SystemTarget } from "../displays/system/systemTarget";
import { vec3 } from "../spatial/vec3";

/** The universe of the pins, `UNIVERSE` in `stellar.rs`. */
export const PIN_UNIVERSE = "0123456789abcdef";

/** The system of the pins, `SYSTEM` in `stellar.rs` and `planetary.rs`. */
export const PIN_SYSTEM = "0200080020000000";

/** The time of the pinned summary, a century after the epoch (`epoch_plus_a_century`). */
export const A_CENTURY: UniverseTime = { seconds: 3_155_760_000, nanos: 0 };

/** The Sun-like star of `stellar.rs`'s `sunlike`: every field computed, none it does not. */
export function aSunlikeStar(overrides: Partial<StarSummaryDto> = {}): StarSummaryDto {
  return {
    body_index: 0,
    kind: "dwarf",
    phase: "main_sequence",
    class: "G2V",
    initial_mass_msun: 1,
    mass_msun: 0.999_75,
    core_mass_msun: 0,
    luminosity_lsun: 1,
    radius_rsun: 1,
    teff_k: 5_772,
    absolute_v_mag: 4.825,
    colour_b_v_mag: 0.65,
    mass_loss_rate_msun_per_yr: 2.5e-14,
    remnant: null,
    death_time: null,
    ...overrides,
  };
}

/** The cool white dwarf of `stellar.rs`'s `white_dwarf`, from a 2.5 M☉ star dead some 4 Gyr. */
export function aWhiteDwarf(overrides: Partial<StarSummaryDto> = {}): StarSummaryDto {
  return {
    body_index: 0,
    kind: "white_dwarf",
    phase: "carbon_oxygen_white_dwarf",
    class: "DA9.2",
    initial_mass_msun: 2.5,
    mass_msun: 0.687_5,
    core_mass_msun: 0.687_5,
    luminosity_lsun: 0.000_107_5,
    radius_rsun: 0.011_5,
    teff_k: 5_480,
    absolute_v_mag: null,
    colour_b_v_mag: null,
    mass_loss_rate_msun_per_yr: 0,
    remnant: { type: "white_dwarf", cooling_age_myr: 3_950.5 },
    death_time: null,
    ...overrides,
  };
}

/** The black hole of `stellar.rs`'s `a_black_hole_has_no_light_and_no_colour`. */
export function aBlackHole(overrides: Partial<StarSummaryDto> = {}): StarSummaryDto {
  return {
    body_index: 0,
    kind: "black_hole",
    phase: "black_hole",
    class: "BH",
    initial_mass_msun: 40,
    mass_msun: 12.5,
    core_mass_msun: 12.5,
    luminosity_lsun: 0,
    radius_rsun: 5.3e-5,
    teff_k: null,
    absolute_v_mag: null,
    colour_b_v_mag: null,
    mass_loss_rate_msun_per_yr: 0,
    remnant: { type: "black_hole" },
    death_time: { seconds: -20_000_000_000, nanos: 0 },
    ...overrides,
  };
}

/**
 * A neutron star with the remnant form `stellar.rs`'s `remnant_wire_forms` pins for one whose pulsar
 * and kick are not modelled yet, at ruling 27's 12.2 km.
 */
export function aNeutronStar(overrides: Partial<StarSummaryDto> = {}): StarSummaryDto {
  return aBlackHole({
    kind: "neutron_star",
    phase: "neutron_star",
    class: "NS",
    initial_mass_msun: 15,
    mass_msun: 1.4,
    core_mass_msun: 1.4,
    luminosity_lsun: 2.5e-6,
    radius_rsun: 12.2 / 695_700,
    teff_k: 350_000,
    remnant: { type: "neutron_star" },
    ...overrides,
  });
}

/** The pair orbit of `stellar.rs`'s `pair_orbit`: a = 23.5 au, e = 0.5, 60.9 yr. */
export function aPairOrbit(overrides: Partial<OrbitDto> = {}): OrbitDto {
  return {
    period_s: 1_921_736_528.404_686,
    semi_major_axis_m: 3_515_625_000_000,
    eccentricity: 0.5,
    inclination_rad: 1.5,
    ascending_node_rad: 3.5,
    argument_of_periapsis_rad: 4.25,
    mean_anomaly_at_epoch_rad: 0.125,
    mu_m3_s2: 4.644_935_401_444_779_6e20,
    ...overrides,
  };
}

/** The wide orbit of `orbit.rs`'s `wide_pair_orbit`, like Alpha Centauri AB's: 23.5 au, 80.8 yr. */
export function aWidePairOrbit(): OrbitDto {
  return {
    period_s: 2_548_768_078.333_065_5,
    semi_major_axis_m: 3_515_625_000_000,
    eccentricity: 0.5,
    inclination_rad: 1.5,
    ascending_node_rad: 3.5,
    argument_of_periapsis_rad: 4.25,
    mean_anomaly_at_epoch_rad: 0.125,
    mu_m3_s2: 2.640_625e20,
  };
}

/** The close orbit of `orbit.rs`'s hierarchical triple: 0.1 au, 8.9 days. */
export function aClosePairOrbit(): OrbitDto {
  return {
    period_s: 769_529.898_097_118_5,
    semi_major_axis_m: 1.5e10,
    eccentricity: 0,
    inclination_rad: 0.25,
    ascending_node_rad: 0,
    argument_of_periapsis_rad: 0,
    mean_anomaly_at_epoch_rad: 2,
    mu_m3_s2: 2.25e20,
  };
}

/** The pinned hierarchy of the white dwarf (2.5 M☉ at birth) and its Sun-like companion. */
export function aBinaryHierarchy(orbit: OrbitDto = aPairOrbit()): HierarchyDto {
  return {
    nodes: [
      { type: "pair", inner: 1, outer: 2, orbit },
      { type: "star", body_index: 0, mass_msun: 2.5 },
      { type: "star", body_index: 1, mass_msun: 1 },
    ],
  };
}

/** `orbit.rs`'s `a_single_star_is_one_star_node`. */
export function aSingleStarHierarchy(massMsun = 1): HierarchyDto {
  return { nodes: [{ type: "star", body_index: 0, mass_msun: massMsun }] };
}

/**
 * `orbit.rs`'s `a_hierarchical_triple_is_listed_depth_first`: a close pair, stars 0 and 1, with
 * star 2 on a wide orbit about it.
 */
export function aTripleHierarchy(): HierarchyDto {
  return {
    nodes: [
      { type: "pair", inner: 1, outer: 4, orbit: aWidePairOrbit() },
      { type: "pair", inner: 2, outer: 3, orbit: aClosePairOrbit() },
      { type: "star", body_index: 0, mass_msun: 1.2 },
      { type: "star", body_index: 1, mass_msun: 0.5 },
      { type: "star", body_index: 2, mass_msun: 0.29 },
    ],
  };
}

/**
 * The summary `stellar.rs`'s `summary` pins: a white dwarf with a Sun-like companion, as Sirius is
 * but older, a century after the epoch.
 */
export function aSystemSummary(overrides: Partial<SystemSummaryDto> = {}): SystemSummaryDto {
  return {
    universe: PIN_UNIVERSE,
    system: PIN_SYSTEM,
    time: A_CENTURY,
    existence: "exists",
    age_myr: 4_600.5,
    fe_h_dex: -0.125,
    stars: [aWhiteDwarf(), aSunlikeStar({ body_index: 1 })],
    hierarchy: aBinaryHierarchy(),
    ...overrides,
  };
}

/** The summary of `stellar.rs`'s `a_system_not_yet_born_has_no_stars`. */
export function aNotYetBornSummary(overrides: Partial<SystemSummaryDto> = {}): SystemSummaryDto {
  return aSystemSummary({
    existence: "not_yet_born",
    age_myr: -0.000_5,
    stars: [],
    hierarchy: { nodes: [] },
    ...overrides,
  });
}

/** A single Sun-like star, from the pins of `sunlike` and `a_single_star_is_one_star_node`. */
export function aSingleStarSummary(star: StarSummaryDto = aSunlikeStar()): SystemSummaryDto {
  return aSystemSummary({
    stars: [star],
    hierarchy: aSingleStarHierarchy(star.initial_mass_msun),
  });
}

/** The triple of `a_hierarchical_triple_is_listed_depth_first`, its stars Sun-like. */
export function aTripleSummary(): SystemSummaryDto {
  return aSystemSummary({
    stars: [
      aSunlikeStar({ initial_mass_msun: 1.2 }),
      aSunlikeStar({ body_index: 1, initial_mass_msun: 0.5 }),
      aSunlikeStar({ body_index: 2, initial_mass_msun: 0.29 }),
    ],
    hierarchy: aTripleHierarchy(),
  });
}

/** A `system_summary` response body carrying a summary. */
export function aSummaryResponse(summary: SystemSummaryDto): ResponseFor<"system_summary"> {
  return { kind: "system_summary", ...summary };
}

/** `stellar.rs`'s `unknown_system_request_error_wire_form`. */
export function anUnknownSystemError(): RequestError {
  return {
    code: "unknown_system",
    message: "no system 0200080020000000 in this universe",
    field: "system",
  };
}

/** The server's answer to a kind it does not serve yet, as `Handlers` answers `system_summary`. */
export function anUnsupportedError(): RequestError {
  return {
    code: "unsupported",
    message: "system_summary is not served yet",
    field: null,
  };
}

/** The server's answer to `system_bodies` before P14.T36, as `Handlers` answers it. */
export function anUnsupportedBodiesError(): RequestError {
  return {
    code: "unsupported",
    message: "system_bodies is not served yet",
    field: null,
  };
}

/** `planetary.rs`'s `section_not_modelled_wire_form`. */
export const SECTION_NOT_MODELLED: SectionDto<never> = { state: "not_modelled" };

/** `planetary.rs`'s `section_not_resolved_wire_form`. */
export const SECTION_NOT_RESOLVED: SectionDto<never> = { state: "not_resolved" };

/** `planetary.rs`'s `section_not_applicable_wire_form`. */
export const SECTION_NOT_APPLICABLE: SectionDto<never> = { state: "not_applicable" };

/** `planetary.rs`'s `section_ok_carries_a_list_as_its_value`: a planet's one moon. */
export const SECTION_OK_LIST: SectionDto<ReadonlyArray<string>> = {
  state: "ok",
  value: ["0200080020000000.0101"],
};

/** The census bands of `galaxyFixtures`' `aCensus`, as the chart hands them to `OPEN SYSTEM`. */
export const BANDS: ReadonlyArray<LayerBand> = [
  { layer: "a", index: 0, minMsun: 0.08, maxMsun: 0.5 },
  { layer: "b", index: 1, minMsun: 0.5, maxMsun: 0.75 },
  { layer: "c", index: 2, minMsun: 0.75, maxMsun: 2.5 },
  { layer: "d", index: 3, minMsun: 2.5, maxMsun: 8 },
  { layer: "e", index: 4, minMsun: 8, maxMsun: 150 },
];

/** A target as `OPEN SYSTEM` opens it: the pinned system at the pinned time, 26,000 ly out. */
export function aSystemTarget(overrides: Partial<SystemTarget> = {}): SystemTarget {
  return {
    universe: PIN_UNIVERSE,
    system: PIN_SYSTEM,
    designation: "H7K 4C0RFZ D-7",
    positionLy: vec3(26_000, 0, 12),
    time: A_CENTURY,
    bands: BANDS,
    ...overrides,
  };
}
