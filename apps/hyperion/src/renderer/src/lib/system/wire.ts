/**
 * The adapter between the `system_summary` messages and the client's model of a system.
 *
 * @remarks
 * The only module outside `test/` that reads the summary's unit-suffixed field names, as plan 05's
 * design note D8 has `lib/galaxy/wire.ts` do for the chart. The wire is decoded but not validated,
 * so an answer the display could not draw or read is reported as a fault in words, as the chart
 * reports `CHART DATA INVALID`, rather than thrown from a render.
 */
import {
  type BodyIdHex,
  formatBodyId,
  type HierarchyNodeDto,
  isHex64,
  type NatalKickDto,
  type OrbitDto,
  type RemnantDto,
  type RequestOf,
  type StarSummaryDto,
  type SystemIdHex,
  type SystemSummaryDto,
  type UniverseIdHex,
  type UniverseTime,
  type VariabilityDto,
} from "@hyperion/protocol";

import { type KeplerOrbit, NEAR_PARABOLIC_ECCENTRICITY } from "../orbit";
import type {
  HierarchyNode,
  HostBody,
  HostRemnant,
  Modelled,
  NatalKick,
  Pending,
  SystemModel,
  Variability,
} from "./model";

/** The highest body index a star may have: stars are 0–15 (plan 14, design note D3). */
const LAST_STAR_INDEX = 15;

/** A system's answer as the display can use it, or why it cannot. */
export type SystemModelResult =
  | { readonly kind: "ok"; readonly model: SystemModel }
  | { readonly kind: "fault"; readonly fault: string };

/** The `system_summary` request for a system at a time. */
export function toSummaryRequest(
  universe: UniverseIdHex,
  system: SystemIdHex,
  time: UniverseTime,
): RequestOf<"system_summary"> {
  return { kind: "system_summary", universe, system, time };
}

/**
 * A body's designation of record: its system's, then ` /` and its body index in decimal (plan 01's
 * `Designation`; plan 14, design note D22).
 */
export function bodyDesignation(systemDesignation: string, bodyIndex: number): string {
  return `${systemDesignation} /${bodyIndex}`;
}

/** The kepler elements of a wire orbit, as the client propagates them (plan 14, D18). */
export function toKeplerOrbit(orbit: OrbitDto): KeplerOrbit {
  return {
    semiMajorAxisM: orbit.semi_major_axis_m,
    eccentricity: orbit.eccentricity,
    inclinationRad: orbit.inclination_rad,
    ascendingNodeRad: orbit.ascending_node_rad,
    argumentOfPeriapsisRad: orbit.argument_of_periapsis_rad,
    meanAnomalyAtEpochRad: orbit.mean_anomaly_at_epoch_rad,
    periodS: orbit.period_s,
  };
}

/** Whether the client can propagate an orbit: the ranges `lib/orbit.ts` accepts. */
function propagatable(orbit: KeplerOrbit): boolean {
  return (
    orbit.eccentricity >= 0 &&
    orbit.eccentricity < NEAR_PARABOLIC_ECCENTRICITY &&
    Number.isFinite(orbit.semiMajorAxisM) &&
    orbit.semiMajorAxisM > 0 &&
    Number.isFinite(orbit.periodS) &&
    orbit.periodS > 0 &&
    orbit.inclinationRad >= 0 &&
    orbit.inclinationRad <= Math.PI &&
    Number.isFinite(orbit.ascendingNodeRad) &&
    Number.isFinite(orbit.argumentOfPeriapsisRad) &&
    Number.isFinite(orbit.meanAnomalyAtEpochRad)
  );
}

function modelled<T, U>(value: T | null | undefined, convert: (present: T) => U): Modelled<U> {
  if (value === undefined) {
    return { kind: "not_modelled" };
  }
  return value === null ? { kind: "none" } : { kind: "value", value: convert(value) };
}

function pending<T, U>(value: T | undefined, convert: (present: T) => U): Pending<U> {
  return value === undefined ? { kind: "not_modelled" } : { kind: "value", value: convert(value) };
}

function toKick(kick: NatalKickDto): NatalKick {
  return { speedKmS: kick.speed_km_s, mode: kick.mode };
}

function toRemnant(remnant: RemnantDto): HostRemnant {
  let result: HostRemnant;
  switch (remnant.type) {
    case "white_dwarf":
      result = {
        kind: "white_dwarf",
        coolingAgeMyr: remnant.cooling_age_myr,
        natalKick: pending(remnant.natal_kick, toKick),
      };
      break;
    case "neutron_star":
      result = {
        kind: "neutron_star",
        pulsar: pending(remnant.pulsar, (pulsar) => ({
          spinPeriodS: pulsar.spin_period_s,
          periodDerivativeSPerS: pulsar.period_derivative_s_per_s,
          magneticFieldG: pulsar.magnetic_field_g,
          alive: pulsar.alive,
          magnetar: pulsar.magnetar,
        })),
        natalKick: pending(remnant.natal_kick, toKick),
      };
      break;
    case "black_hole":
      result = {
        kind: "black_hole",
        dimensionlessSpin: pending(remnant.dimensionless_spin, (spin) => spin),
        natalKick: pending(remnant.natal_kick, toKick),
      };
      break;
    case "no_remnant":
      result = { kind: "no_remnant" };
      break;
  }
  return result;
}

function toVariability(variability: VariabilityDto): Variability {
  return {
    kind: variability.kind,
    periodD: variability.period_d,
    amplitudeMag: variability.amplitude_mag,
  };
}

function toHost(system: SystemIdHex, designation: string, star: StarSummaryDto): HostBody {
  const id: BodyIdHex = formatBodyId({ system, bodyIndex: star.body_index });
  return {
    id,
    bodyIndex: star.body_index,
    designation: bodyDesignation(designation, star.body_index),
    kind: star.kind,
    phase: star.phase,
    spectralClass: star.class,
    initialMassMsun: star.initial_mass_msun,
    massMsun: star.mass_msun,
    luminosityLsun: star.luminosity_lsun,
    radiusRsun: star.radius_rsun,
    teffK: star.teff_k,
    remnant: star.remnant === null ? null : toRemnant(star.remnant),
    deathTime: star.death_time,
    rotationPeriodD: modelled(star.rotation_period_d, (period) => period),
    activityLogLxLbol: modelled(star.activity_log_lx_lbol, (activity) => activity),
    variability: modelled(star.variability, toVariability),
  };
}

function toNode(node: HierarchyNodeDto): HierarchyNode {
  return node.type === "star"
    ? { kind: "star", bodyIndex: node.body_index, massMsun: node.mass_msun }
    : { kind: "pair", inner: node.inner, outer: node.outer, orbit: toKeplerOrbit(node.orbit) };
}

function finiteAtLeastZero(value: number): boolean {
  return Number.isFinite(value) && value >= 0;
}

/** Why a star's values cannot be shown, or `null` when they can. */
function starFault(star: StarSummaryDto): string | null {
  const numbers = [star.initial_mass_msun, star.mass_msun, star.luminosity_lsun, star.radius_rsun];
  const teffUsable = star.teff_k === null || (Number.isFinite(star.teff_k) && star.teff_k > 0);
  const coolingUsable =
    star.remnant?.type !== "white_dwarf" || finiteAtLeastZero(star.remnant.cooling_age_myr);
  return numbers.every(finiteAtLeastZero) && teffUsable && coolingUsable
    ? null
    : `star ${star.body_index} values unusable`;
}

/**
 * Why a hierarchy cannot place the stars, or `null` when it can: every star is one node, every node
 * but the root is one pair's member, a pair's members come after it, and every mass and orbit can
 * be used.
 */
function hierarchyFault(
  nodes: ReadonlyArray<HierarchyNode>,
  bodyIndices: ReadonlyArray<number>,
): string | null {
  const children = new Set<number>();
  const stars: number[] = [];
  for (const [index, node] of nodes.entries()) {
    if (node.kind === "star") {
      if (!(Number.isFinite(node.massMsun) && node.massMsun > 0)) {
        return "hierarchy mass unusable";
      }
      stars.push(node.bodyIndex);
      continue;
    }
    for (const child of [node.inner, node.outer]) {
      const valid = Number.isInteger(child) && child > index && child < nodes.length;
      if (!valid || children.has(child)) {
        return "hierarchy malformed";
      }
      children.add(child);
    }
    if (!propagatable(node.orbit)) {
      return "orbit unusable";
    }
  }
  const everyNodeOnce = nodes.length === 0 || children.size === nodes.length - 1;
  const sameStars =
    stars.length === bodyIndices.length &&
    stars.toSorted((a, b) => a - b).every((star, index) => star === bodyIndices[index]);
  return everyNodeOnce && sameStars ? null : "hierarchy malformed";
}

/**
 * Turns a `system_summary` answer into the display's model, or says why it cannot be shown.
 *
 * @remarks
 * The universe, system and time come from the answer's echo of its request, so what is drawn is
 * what was answered. Each star's ID is its system's and its body index, and its designation the
 * system's with ` /` and the index. A value this generator version does not compute arrives absent
 * and is `not_modelled`, one computed as none arrives `null` and is `none` (the orchestrator's
 * ruling 54).
 *
 * @param designation - The system's designation of record, which the chart's answer carried.
 */
export function toSystemModel(response: SystemSummaryDto, designation: string): SystemModelResult {
  if (!isHex64(response.system)) {
    return { kind: "fault", fault: "system ID malformed" };
  }
  if (!Number.isFinite(response.age_myr) || !Number.isFinite(response.fe_h_dex)) {
    return { kind: "fault", fault: "system values unusable" };
  }
  const stars = response.stars;
  const indices = stars.map((star) => star.body_index);
  const ordered = indices.every(
    (index, position) =>
      Number.isInteger(index) &&
      index >= 0 &&
      index <= LAST_STAR_INDEX &&
      (position === 0 ? index === 0 : index > (indices[position - 1] ?? LAST_STAR_INDEX)),
  );
  if (!ordered) {
    return { kind: "fault", fault: "star list malformed" };
  }
  const formed = response.existence === "exists";
  if (formed && stars.length === 0) {
    return { kind: "fault", fault: "star list empty" };
  }
  for (const star of stars) {
    const fault = starFault(star);
    if (fault !== null) {
      return { kind: "fault", fault };
    }
  }
  const hierarchy = response.hierarchy.nodes.map(toNode);
  const fault = hierarchyFault(hierarchy, indices);
  if (fault !== null) {
    return { kind: "fault", fault };
  }
  return {
    kind: "ok",
    model: {
      universe: response.universe,
      system: response.system,
      time: response.time,
      formed,
      ageMyr: response.age_myr,
      feHDex: response.fe_h_dex,
      hosts: stars.map((star) => toHost(response.system, designation, star)),
      hierarchy,
    },
  };
}
