/**
 * The adapter between the `system_bodies` and `body_detail` messages and the client's model of a
 * system's bodies (plan 14, P14.T35.b–c, T41 and T43).
 *
 * @remarks
 * The only module outside `test/` that reads the bodies' unit-suffixed field names, as `wire.ts` is
 * for the hosts. The wire is decoded but not validated, so an answer the display could not place
 * or read is reported as a fault in words rather than thrown from a render. Every section keeps the
 * state the server tagged it with (the orchestrator's ruling 34): the client never infers one.
 * Masses cross in kilograms (ruling 64.3) and are held in Earth masses, by the simulation's own
 * Earth mass, so that the readout's `M⊕` is the server's.
 */
import {
  type AbsorberDto,
  type AerosolInventoryDto,
  type AerosolModeDto,
  type BandsDto,
  type BiosphereDto,
  type BodyDetailDto,
  type BodyEnvelopeDto,
  type BodyFigureDto,
  type BodyIdHex,
  type BodyKindDto,
  type BodyOrbitDto,
  type BodyPhotometryDto,
  type BodyRecordDto,
  type BodyRotationDto,
  type BodyStateDto,
  type BodySummaryDto,
  type BodySurfaceDto,
  type BulkPropertiesDto,
  type ClimateRegimeDto,
  type CraterContractDto,
  type CraterScreeningDto,
  type CrustCompositionDto,
  type DetailSeedHex,
  type GasShareDto,
  type HabitableZoneDto,
  isBodyId,
  type OrbitDriftDto,
  type OrbitHostDto,
  parseBodyId,
  type PopulationDto,
  type ProfileNodeDto,
  type RequestOf,
  type SectionDto,
  type SilicateVapourDto,
  type SizeDistributionDto,
  type SubstanceAreaDto,
  type SubstancePhaseDto,
  type SubsurfaceOceanDto,
  type SurfaceCondensateDto,
  type SurfaceModifiersDto,
  type SystemBodiesDto,
  type SystemIdHex,
  type SystemPlaneDto,
  type UniverseIdHex,
  type UniverseTime,
  type VerticalStructureDto,
  type WetEpochDto,
  type ZoneDto,
} from "@hyperion/protocol";

import { cross, dot, vec3, type Vec3 } from "../../geometry/vec3";
import { ROTATION_ORTHONORMAL_TOLERANCE } from "../../view/coords/rotation";
import { type KeplerOrbit, NEAR_PARABOLIC_ECCENTRICITY, type OrbitDrift } from "../orbit";
import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "./constants";
import type {
  Absorber,
  AerosolInventory,
  AerosolMode,
  AerosolPhase,
  Biosphere,
  BodyDetail,
  BodyEnvelope,
  BodyKind,
  BodyOrbit,
  BodyRecord,
  BodyState,
  BodySurface,
  BulkProperties,
  ClimateRegime,
  CondensatePhase,
  CraterContract,
  CraterScreening,
  CrustComposition,
  GasShare,
  HabitableZone,
  HierarchyNode,
  JohnsonBands,
  OrbitHost,
  Population,
  ProfileNode,
  RotationLock,
  Section,
  SilicateVapour,
  SizeDistribution,
  SubstanceArea,
  SubsurfaceOcean,
  SurfaceCondensate,
  SurfaceModifiers,
  SystemBodies,
  SystemBody,
  SystemBodyFigure,
  SystemBodyPhotometry,
  SystemBodyRotation,
  SystemModel,
  SystemPlane,
  VerticalStructure,
  WetEpoch,
  Zone,
} from "./model";
import { bodyDesignation, modelled, toKeplerOrbit, toSystemModel } from "./wire";

/**
 * The simulation's Earth mass, in kilograms: GM⊕ ÷ G, 3.986 004 × 10¹⁴ m³ s⁻² over
 * {@link GRAVITATIONAL_CONSTANT_M3_PER_KG_S2}, `units::consts::EARTH_MASS_KG` in `hyperion-sim`, the
 * same quotient to the bit (the orchestrator's ruling 64.3).
 */
export const EARTH_MASS_KG = 3.986_004e14 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2;

/** The detail level the display asks for: everything, which the server grants until P14's overlay. */
const DETAIL_ASKED = "full";

/** A system's bodies as the display can use them, with the hosts they came with, or why not. */
export type SystemBodiesResult =
  | { readonly kind: "ok"; readonly model: SystemModel; readonly bodies: SystemBodies }
  | { readonly kind: "fault"; readonly fault: string };

/** A body's record as the display can use it, or why not. */
export type BodyDetailResult =
  | { readonly kind: "ok"; readonly detail: BodyDetail }
  | { readonly kind: "fault"; readonly fault: string };

/** The `system_bodies` request for a system at a time, at every level of detail. */
export function toBodiesRequest(
  universe: UniverseIdHex,
  system: SystemIdHex,
  time: UniverseTime,
): RequestOf<"system_bodies"> {
  return { kind: "system_bodies", universe, system, time, detail: DETAIL_ASKED };
}

/** The `body_detail` request for one body at a time, at every level of detail. */
export function toBodyDetailRequest(
  universe: UniverseIdHex,
  body: BodyIdHex,
  time: UniverseTime,
): RequestOf<"body_detail"> {
  return { kind: "body_detail", universe, body, time, detail: DETAIL_ASKED };
}

/** A detail seed's form on the wire: exactly 16 lowercase hexadecimal digits (`DetailSeedHex`). */
const DETAIL_SEED_FORM = /^[0-9a-f]{16}$/u;

/**
 * A body record's detail seed (`BodyHooksDto.detail_seed`, R09.T1.b), the seed of the client's
 * terrain synthesis, without reading the rest of the record: `null` where its hooks or the seed
 * are not `ok` (not modelled, not applicable, or above the detail granted) or the seed is not of
 * its wire form.
 *
 * @remarks
 * The terrain session's want of a body (plan R10, R10.T16.a) reads it from one `body_detail` a
 * want, as {@link toBodyDetail}'s record reads it whole for the `SYSTEM` display.
 */
export function bodyDetailSeed(record: Pick<BodyRecordDto, "hooks">): DetailSeedHex | null {
  const { hooks } = record;
  if (hooks.state !== "ok" || hooks.value.detail_seed.state !== "ok") {
    return null;
  }
  const seed = hooks.value.detail_seed.value;
  return DETAIL_SEED_FORM.test(seed) ? seed : null;
}

/**
 * A body record's own orbit about its parent, as the client propagates it (with its drift), without
 * reading the rest of the record: `null` where the section is not `ok` (a free-floating body's is
 * `not_applicable`) or the orbit is not one the client can propagate.
 */
export function bodyRecordOrbit(record: Pick<BodyRecordDto, "orbit">): KeplerOrbit | null {
  const { orbit } = record;
  if (orbit.state !== "ok") {
    return null;
  }
  const converted = faultOf(() => toOrbit(orbit.value));
  return "ok" in converted ? converted.ok.orbit : null;
}

/**
 * Whose orbit sets a body's seasons, by its record (R09's Design note 8; decision-r10-t12-leans §6
 * (b)): a moon's, one whose kind is `moon` about a body, its planet's, whose record holds it; every
 * other body's its own, a circumbinary planet's and a belt member's included.
 */
export type RecordSeasons =
  /** The body's own orbit sets them. */
  | { readonly kind: "own"; readonly orbit: KeplerOrbit }
  /** A moon: its planet's orbit sets them, from the planet's own record. */
  | { readonly kind: "planet"; readonly planet: BodyIdHex }
  /** No orbit the client can propagate is on the record. */
  | { readonly kind: "none" };

/**
 * Whose orbit sets the seasons of the body of `record` (plan R10, T12): the sim's
 * `CoarseInputs::planet_of`'s rule, a moon by its kind about a body, read off the record's kind and
 * parent, and otherwise the record's own orbit ({@link bodyRecordOrbit}).
 */
export function bodyRecordSeasons(
  record: Pick<BodyRecordDto, "kind" | "parent" | "orbit">,
): RecordSeasons {
  const { kind, parent } = record;
  if (kind.type === "moon" && parent?.type === "body") {
    return { kind: "planet", planet: parent.id };
  }
  const orbit = bodyRecordOrbit(record);
  return orbit === null ? { kind: "none" } : { kind: "own", orbit };
}

/**
 * Thrown inside this module for a value the display cannot use, and caught at its two entry points
 * into a fault in words, so that a check deep in a record need not thread a result back up: an
 * answer that breaks the protocol's own ranges is the server's bug, and never leaves this module as
 * an exception.
 */
class Unusable extends Error {}

/** Throws {@link Unusable} naming `what` unless `condition` holds. */
function check(condition: boolean, what: string): void {
  if (!condition) {
    throw new Unusable(what);
  }
}

function positive(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}

function atLeastZero(value: number): boolean {
  return Number.isFinite(value) && value >= 0;
}

function toSection<T, U>(section: SectionDto<T>, convert: (value: T) => U): Section<U> {
  let result: Section<U>;
  switch (section.state) {
    case "ok":
      result = { state: "ok", value: convert(section.value) };
      break;
    case "not_resolved":
    case "not_modelled":
    case "not_applicable":
      result = { state: section.state };
      break;
  }
  return result;
}

function toHost(host: OrbitHostDto): OrbitHost {
  let result: OrbitHost;
  switch (host.type) {
    case "star":
      result = { kind: "star", bodyIndex: host.body_index };
      break;
    case "pair":
      result = { kind: "pair", keyBodyIndex: host.key_body_index };
      break;
    case "barycentre":
      result = { kind: "barycentre" };
      break;
    case "body":
      result = { kind: "body", id: host.id };
      break;
  }
  return result;
}

function toKind(kind: BodyKindDto): BodyKind {
  let result: BodyKind;
  switch (kind.type) {
    case "moon":
      result = { kind: "moon", origin: kind.origin };
      break;
    case "belt":
      result = { kind: "belt", beltKind: kind.belt_kind };
      break;
    case "planet":
    case "dwarf_planet":
    case "ring":
    case "cometary_halo":
    case "protoplanetary_disc":
    case "debris_disc":
    case "unresolved":
      result = { kind: kind.type };
      break;
  }
  return result;
}

function validNanos(nanos: number): boolean {
  return Number.isInteger(nanos) && nanos >= 0 && nanos < 1_000_000_000;
}

/** A time the client computes with: whole seconds that a JavaScript number holds exactly. */
function usableTime(time: UniverseTime): boolean {
  return Number.isSafeInteger(time.seconds) && validNanos(time.nanos);
}

/**
 * A body state's time, which is only displayed: any whole number of seconds (plan 14, P14.T35.d).
 *
 * @remarks
 * A body unbound or destroyed long before the clock window carries a time past ±(2⁵³ − 1) s,
 * about ±285 Myr, which `JSON.parse` rounds: at 6 Gyr to 32 s, a relative 2⁻⁵³ that the years it
 * is shown in cannot reveal. Every time the client computes with lies inside the clock window and
 * is held to {@link usableTime}.
 */
function displayableTime(time: UniverseTime): boolean {
  return Number.isInteger(time.seconds) && validNanos(time.nanos);
}

function toState(state: BodyStateDto): BodyState {
  let result: BodyState;
  switch (state.type) {
    case "not_yet_formed":
    case "present":
      result = { kind: state.type };
      break;
    case "destroyed":
      check(displayableTime(state.at), "state time unusable");
      result = { kind: "destroyed", cause: state.cause, at: state.at };
      break;
    case "unbound":
      check(displayableTime(state.at), "state time unusable");
      result = { kind: "unbound", at: state.at };
      break;
  }
  return result;
}

/** An evolving orbit's drift, with its reference time and its rates checked (P14.T45.c). */
function toDrift(drift: OrbitDriftDto): OrbitDrift {
  check(
    usableTime(drift.reference) &&
      Number.isFinite(drift.semi_major_axis_rate_m_per_s) &&
      Number.isFinite(drift.eccentricity_rate_per_s) &&
      Number.isFinite(drift.mean_motion_rate_rad_per_s2),
    "orbit drift unusable",
  );
  return {
    reference: drift.reference,
    semiMajorAxisRateMPerS: drift.semi_major_axis_rate_m_per_s,
    eccentricityRatePerS: drift.eccentricity_rate_per_s,
    meanMotionRateRadPerS2: drift.mean_motion_rate_rad_per_s2,
  };
}

function toOrbit(orbit: BodyOrbitDto): BodyOrbit {
  const fixed = toKeplerOrbit(orbit.orbit);
  const kepler = orbit.drift === undefined ? fixed : { ...fixed, drift: toDrift(orbit.drift) };
  check(
    kepler.eccentricity >= 0 &&
      kepler.eccentricity < NEAR_PARABOLIC_ECCENTRICITY &&
      positive(kepler.semiMajorAxisM) &&
      positive(kepler.periodS) &&
      kepler.inclinationRad >= 0 &&
      kepler.inclinationRad <= Math.PI &&
      Number.isFinite(kepler.ascendingNodeRad) &&
      Number.isFinite(kepler.argumentOfPeriapsisRad) &&
      Number.isFinite(kepler.meanAnomalyAtEpochRad),
    "orbit unusable",
  );
  check(orbit.valid_until === null || usableTime(orbit.valid_until), "orbit time unusable");
  return { parent: toHost(orbit.parent), orbit: kepler, validUntil: orbit.valid_until };
}

function toBulk(bulk: BulkPropertiesDto): BulkProperties {
  const fractions = bulk.mass_fractions;
  const shares = [fractions.iron, fractions.rock, fractions.water, fractions.envelope];
  check(
    positive(bulk.radius_m) &&
      positive(bulk.density_kg_m3) &&
      positive(bulk.surface_gravity_m_s2) &&
      atLeastZero(bulk.equilibrium_temperature_k) &&
      (bulk.effective_temperature_k === null ||
        (positive(bulk.effective_temperature_k) &&
          bulk.effective_temperature_k >= bulk.equilibrium_temperature_k)) &&
      shares.every((share) => atLeastZero(share) && share <= 1),
    "bulk values unusable",
  );
  return {
    radiusM: bulk.radius_m,
    densityKgM3: bulk.density_kg_m3,
    surfaceGravityMS2: bulk.surface_gravity_m_s2,
    planetClass: bulk.class,
    massFractions: { ...fractions },
    equilibriumTemperatureK: bulk.equilibrium_temperature_k,
    effectiveTemperatureK: bulk.effective_temperature_k,
  };
}

/**
 * A section that an older server leaves out (plan 14, P14.T46.f): absent, it reads as that server's
 * `not_modelled`.
 */
function toOptionalSection<T, U>(
  section: SectionDto<T> | undefined,
  convert: (value: T) => U,
): Section<U> {
  return section === undefined ? { state: "not_modelled" } : toSection(section, convert);
}

function finite(value: number): boolean {
  return Number.isFinite(value);
}

function toVec3(components: [number, number, number]): Vec3 {
  return vec3(components[0], components[1], components[2]);
}

/**
 * How far from orthonormal the wire's equator and pole may be: a quarter of the view's
 * `ROTATION_ORTHONORMAL_TOLERANCE`, so that the axes turned by any W still pass
 * `rotation3FromRows`, whose departures are at most twice the axes' and rounding.
 */
const AXES_ORTHONORMAL_TOLERANCE = ROTATION_ORTHONORMAL_TOLERANCE / 4;

/** Whether a vector is finite and of unit length to {@link AXES_ORTHONORMAL_TOLERANCE}. */
function unit(v: Vec3): boolean {
  return (
    finite(v.x) &&
    finite(v.y) &&
    finite(v.z) &&
    Math.abs(dot(v, v) - 1) <= AXES_ORTHONORMAL_TOLERANCE
  );
}

/** A ring's plane's normal, checked: finite and of unit length, as a pole is. */
function toRingNormal(components: [number, number, number]): Vec3 {
  const normal = toVec3(components);
  check(unit(normal), "ring normal unusable");
  return normal;
}

/** Whether two vectors are perpendicular to {@link AXES_ORTHONORMAL_TOLERANCE}. */
function perpendicular(a: Vec3, b: Vec3): boolean {
  return Math.abs(dot(a, b)) <= AXES_ORTHONORMAL_TOLERANCE;
}

/**
 * A lock as the wire states it, checked: a locking age, positive, wherever a lock's instant is
 * given.
 *
 * @remarks
 * A lock long before or after the clock window is only compared with the times the client computes
 * with, which `JSON.parse`'s rounding of its seconds beyond ±(2⁵³ − 1) s cannot reorder, so its
 * instant need only be whole seconds and nanoseconds.
 */
function toLock(lockingAgeS: number | null, locksAt: UniverseTime | null): RotationLock {
  if (lockingAgeS === null) {
    check(locksAt === null, "rotation lock unusable");
    return { kind: "never" };
  }
  check(positive(lockingAgeS), "rotation law unusable");
  if (locksAt === null) {
    return { kind: "outside_clock", lockingAgeS };
  }
  check(displayableTime(locksAt), "rotation lock unusable");
  return { kind: "in_clock", lockingAgeS, locksAt };
}

/**
 * A rotation law, checked: its node, quarter and pole an orthonormal right-handed triad, its rates,
 * ages and clock period positive, its angles finite, and a lock's instant with its locking age.
 */
function toRotation(rotation: BodyRotationDto): SystemBodyRotation {
  const pole = toVec3(rotation.pole);
  const node = toVec3(rotation.equator_node);
  const quarter = toVec3(rotation.equator_quarter);
  check(
    unit(pole) &&
      unit(node) &&
      unit(quarter) &&
      perpendicular(node, quarter) &&
      perpendicular(node, pole) &&
      perpendicular(quarter, pole) &&
      dot(node, cross(quarter, pole)) > 0,
    "rotation axes unusable",
  );
  check(
    rotation.obliquity_rad >= 0 &&
      rotation.obliquity_rad <= Math.PI &&
      positive(rotation.initial_rate_rad_s) &&
      positive(rotation.locked_rate_rad_s) &&
      positive(rotation.age_at_epoch_s) &&
      positive(rotation.clock_period_s) &&
      [
        rotation.clock_mean_anomaly_at_epoch_rad,
        rotation.sub_primary_angle_rad,
        rotation.phase_at_epoch_rad,
        rotation.capture_phase_rad,
      ].every(finite),
    "rotation law unusable",
  );
  return {
    pole,
    equatorNode: node,
    equatorQuarter: quarter,
    obliquityRad: rotation.obliquity_rad,
    initialRateRadPerS: rotation.initial_rate_rad_s,
    lockedRateRadPerS: rotation.locked_rate_rad_s,
    ageAtEpochS: rotation.age_at_epoch_s,
    lock: toLock(rotation.locking_age_s, rotation.locks_at),
    resonance: rotation.resonance,
    clockPeriodS: rotation.clock_period_s,
    clockMeanAnomalyAtEpochRad: rotation.clock_mean_anomaly_at_epoch_rad,
    subPrimaryAngleRad: rotation.sub_primary_angle_rad,
    phaseAtEpochRad: rotation.phase_at_epoch_rad,
    capturePhaseRad: rotation.capture_phase_rad,
  };
}

/** A figure, checked: 0 < c ≤ a, a flattening in `[0, 1)`, a unit pole, a factor in `(0, 0.4]`. */
function toFigure(figure: BodyFigureDto): SystemBodyFigure {
  const pole = toVec3(figure.pole);
  check(
    positive(figure.equatorial_radius_m) &&
      positive(figure.polar_radius_m) &&
      figure.polar_radius_m <= figure.equatorial_radius_m &&
      figure.flattening >= 0 &&
      figure.flattening < 1 &&
      unit(pole) &&
      figure.moment_of_inertia_factor > 0 &&
      figure.moment_of_inertia_factor <= 0.4,
    "figure unusable",
  );
  return {
    equatorialRadiusM: figure.equatorial_radius_m,
    polarRadiusM: figure.polar_radius_m,
    flattening: figure.flattening,
    pole,
    momentOfInertiaFactor: figure.moment_of_inertia_factor,
    law: figure.law,
    datum: figure.datum,
  };
}

function toBands(bands: BandsDto): JohnsonBands {
  return { b: bands.b, v: bands.v, r: bands.r };
}

/**
 * A photometric section, checked: p and s positive in every band, L in `[0, 1]`, a Bond albedo in
 * `[0, 1)` and a positive stated ratio.
 */
function toPhotometry(photometry: BodyPhotometryDto): SystemBodyPhotometry {
  const p = photometry.geometric_albedo;
  const s = photometry.phase_exponent;
  check(
    [p.b, p.v, p.r, s.b, s.v, s.r].every(positive) &&
      photometry.lunar_lambert_share >= 0 &&
      photometry.lunar_lambert_share <= 1 &&
      photometry.bond_albedo >= 0 &&
      photometry.bond_albedo < 1 &&
      positive(photometry.bond_ratio),
    "photometry unusable",
  );
  return {
    geometricAlbedo: toBands(p),
    phaseTemplate: photometry.phase_template,
    phaseExponent: toBands(s),
    lunarLambertShare: photometry.lunar_lambert_share,
    bondAlbedo: photometry.bond_albedo,
    bondRatio: photometry.bond_ratio,
    provisional: photometry.provisional,
  };
}

function toMassMearth(massKg: number): number {
  check(positive(massKg), "mass unusable");
  return massKg / EARTH_MASS_KG;
}

function toIds(ids: ReadonlyArray<BodyIdHex>): ReadonlyArray<BodyIdHex> {
  check(
    ids.every((id) => isBodyId(id)),
    "body ID malformed",
  );
  return [...ids];
}

/** Checks an annulus's two edges: positive, finite and in order. */
function checkEdges(innerM: number, outerM: number, what: string): void {
  check(positive(innerM) && positive(outerM) && innerM <= outerM, `${what} edges unusable`);
}

/** A resonance as the wire gives it: two positive integers. */
function toResonance(resonance: [number, number]): readonly [number, number] {
  check(
    resonance.every((term) => Number.isInteger(term) && term > 0),
    "resonance unusable",
  );
  return [resonance[0], resonance[1]];
}

/** A ring, a belt or the halo, checked: edges positive and in order, statistics in range. */
function toPopulation(population: PopulationDto): Population {
  let result: Population;
  switch (population.type) {
    case "ring":
      checkEdges(population.inner_edge_m, population.outer_edge_m, "ring");
      check(positive(population.optical_depth), "ring optical depth unusable");
      result = {
        kind: "ring",
        ringKind: population.ring_kind,
        material: population.material,
        innerEdgeM: population.inner_edge_m,
        outerEdgeM: population.outer_edge_m,
        opticalDepth: population.optical_depth,
        gaps: population.gaps.map((gap) => {
          check(isBodyId(gap.moon) && positive(gap.radius_m), "ring gap unusable");
          return { moon: gap.moon, resonance: toResonance(gap.resonance), radiusM: gap.radius_m };
        }),
        normal: population.normal === undefined ? null : toRingNormal(population.normal),
      };
      break;
    case "belt": {
      check(population.host.type !== "body", "belt host malformed");
      checkEdges(population.inner_edge_m, population.outer_edge_m, "belt");
      checkEdges(population.main.inner_edge_m, population.main.outer_edge_m, "belt");
      const scattered = population.scattered;
      if (scattered !== null) {
        checkEdges(scattered.inner_edge_m, scattered.outer_edge_m, "belt");
      }
      check(
        positive(population.size_slope) &&
          positive(population.largest_diameter_m) &&
          atLeastZero(population.mean_eccentricity) &&
          population.mean_eccentricity < 1 &&
          atLeastZero(population.mean_inclination_rad) &&
          population.mean_inclination_rad <= Math.PI &&
          atLeastZero(population.fractional_luminosity),
        "belt statistics unusable",
      );
      result = {
        kind: "belt",
        host: toHost(population.host),
        site: population.site,
        innerEdgeM: population.inner_edge_m,
        outerEdgeM: population.outer_edge_m,
        main: {
          innerEdgeM: population.main.inner_edge_m,
          outerEdgeM: population.main.outer_edge_m,
        },
        scattered:
          scattered === null
            ? null
            : { innerEdgeM: scattered.inner_edge_m, outerEdgeM: scattered.outer_edge_m },
        gaps: population.gaps.map((gap) => {
          check(positive(gap.radius_m), "belt gap unusable");
          return { resonance: toResonance(gap.resonance), radiusM: gap.radius_m };
        }),
        sizeSlope: population.size_slope,
        largestDiameterM: population.largest_diameter_m,
        composition: population.composition,
        meanEccentricity: population.mean_eccentricity,
        meanInclinationRad: population.mean_inclination_rad,
        fractionalLuminosity: population.fractional_luminosity,
        members: toSection(population.members, toIds),
      };
      break;
    }
    case "cometary_halo":
      check(population.host.type !== "body", "halo host malformed");
      checkEdges(population.inner_edge_m, population.outer_edge_m, "halo");
      check(
        atLeastZero(population.comets) && atLeastZero(population.comet_rate_per_s),
        "halo statistics unusable",
      );
      result = {
        kind: "cometary_halo",
        host: toHost(population.host),
        innerEdgeM: population.inner_edge_m,
        outerEdgeM: population.outer_edge_m,
        comets: population.comets,
        cometRatePerS: population.comet_rate_per_s,
      };
      break;
  }
  return result;
}

/** A body of the list, checked and in the display's units. */
function toBody(body: BodySummaryDto, system: SystemIdHex, designation: string): SystemBody {
  check(isBodyId(body.id), "body ID malformed");
  const { system: ofSystem, bodyIndex } = parseBodyId(body.id);
  check(ofSystem === system, `body ${body.id} of another system`);
  const position = body.position_m;
  check(
    position === null || position.every((metres) => Number.isFinite(metres)),
    "position unusable",
  );
  return {
    id: body.id,
    bodyIndex,
    designation: bodyDesignation(designation, bodyIndex),
    kind: toKind(body.kind),
    label: toSection(body.label, (label) => label),
    parent: body.parent === null ? null : toHost(body.parent),
    state: toState(body.state),
    positionM: position === null ? null : vec3(position[0], position[1], position[2]),
    massMearth: toSection(body.mass_kg, toMassMearth),
    orbit: toSection(body.orbit, toOrbit),
    moons: toSection(body.moons, toIds),
    rings: toSection(body.rings, toIds),
    population: toSection(body.population, toPopulation),
    bulk: toSection(body.bulk, toBulk),
    rotation: toOptionalSection(body.rotation, toRotation),
    figure: toOptionalSection(body.figure, toFigure),
    photometry: toOptionalSection(body.photometry, toPhotometry),
  };
}

function toPlane(plane: SystemPlaneDto): SystemPlane {
  check(
    plane.inclination_rad >= 0 &&
      plane.inclination_rad <= Math.PI &&
      Number.isFinite(plane.ascending_node_rad),
    "plane unusable",
  );
  return { inclinationRad: plane.inclination_rad, ascendingNodeRad: plane.ascending_node_rad };
}

function limit(metres: number | null): number | null {
  check(metres === null || atLeastZero(metres), "zone limit unusable");
  return metres;
}

function toHabitableZone(zone: HabitableZoneDto): HabitableZone {
  return {
    recentVenusM: limit(zone.recent_venus_m),
    runawayGreenhouseM: limit(zone.runaway_greenhouse_m),
    moistGreenhouseM: limit(zone.moist_greenhouse_m),
    maximumGreenhouseM: limit(zone.maximum_greenhouse_m),
    earlyMarsM: limit(zone.early_mars_m),
    extrapolated: zone.extrapolated,
  };
}

function toZone(zone: ZoneDto): Zone {
  check(zone.host.type !== "body", "zone host malformed");
  check(positive(zone.snow_line_m), "zone limit unusable");
  check(zone.inner_m === null || positive(zone.inner_m), "zone limit unusable");
  check(zone.outer_m === null || positive(zone.outer_m), "zone limit unusable");
  check(
    zone.inner_m === null || zone.outer_m === null || zone.inner_m < zone.outer_m,
    "zone limits reversed",
  );
  return {
    host: toHost(zone.host),
    innerM: zone.inner_m,
    outerM: zone.outer_m,
    snowLineM: zone.snow_line_m,
    plane: toPlane(zone.plane),
    architecture: zone.architecture,
    habitableZone: zone.habitable_zone === null ? null : toHabitableZone(zone.habitable_zone),
  };
}

/**
 * The body indices that key a pair: each pair node's outer member's first star, as a `pair` host
 * names it (plan 11, design note 5).
 */
function pairKeyIndices(nodes: ReadonlyArray<HierarchyNode>): ReadonlySet<number> {
  const firstStar = (index: number): number | null => {
    let node = nodes[index];
    for (let steps = 0; node?.kind === "pair" && steps < nodes.length; steps += 1) {
      node = nodes[node.inner];
    }
    return node?.kind === "star" ? node.bodyIndex : null;
  };
  const keys = new Set<number>();
  for (const node of nodes) {
    if (node.kind === "pair") {
      const key = firstStar(node.outer);
      if (key !== null) {
        keys.add(key);
      }
    }
  }
  return keys;
}

/**
 * Checks that every host a body or a zone names is there: a star of the system, a pair its
 * hierarchy keys, or a body of the list, and that a body's chain of parents ends at a host.
 */
function checkHosts(model: SystemModel, bodies: SystemBodies): void {
  const stars = new Set(model.hosts.map((host) => host.bodyIndex));
  const pairs = pairKeyIndices(model.hierarchy);
  const byId = new Map(bodies.bodies.map((body) => [body.id, body]));
  const known = (host: OrbitHost): boolean => {
    let result: boolean;
    switch (host.kind) {
      case "star":
        result = stars.has(host.bodyIndex);
        break;
      case "pair":
        result = pairs.has(host.keyBodyIndex);
        break;
      case "barycentre":
        result = model.hosts.length > 0;
        break;
      case "body":
        result = byId.has(host.id);
        break;
    }
    return result;
  };
  for (const zone of bodies.zones) {
    check(known(zone.host), "zone host unknown");
  }
  for (const body of bodies.bodies) {
    const population = body.population.state === "ok" ? body.population.value : null;
    const hosts = [
      body.parent,
      body.orbit.state === "ok" ? body.orbit.value.parent : null,
      population === null || population.kind === "ring" ? null : population.host,
    ];
    for (const host of hosts) {
      check(host === null || known(host), `body ${body.bodyIndex} host unknown`);
    }
    // A chain of bodies ends at a star, a pair or the barycentre within as many steps as there
    // are bodies, or it runs in a circle.
    let at: OrbitHost | null = body.parent;
    for (let steps = 0; at?.kind === "body"; steps += 1) {
      check(
        steps < bodies.bodies.length && at.id !== body.id,
        `body ${body.bodyIndex} parents circular`,
      );
      at = byId.get(at.id)?.parent ?? null;
    }
  }
  const ids = [
    ...(bodies.belts.state === "ok" ? bodies.belts.value : []),
    ...(bodies.halo.state === "ok" && bodies.halo.value !== null ? [bodies.halo.value] : []),
  ];
  check(
    ids.every((id) => byId.has(id)),
    "population unknown",
  );
}

/** Runs a conversion, turning a value it cannot use into a fault in words. */
function faultOf<T>(convert: () => T): { readonly ok: T } | { readonly fault: string } {
  try {
    return { ok: convert() };
  } catch (error: unknown) {
    if (error instanceof Unusable) {
      return { fault: error.message };
    }
    throw error;
  }
}

/**
 * Turns a `system_bodies` answer into the display's model of the system, its hosts and its bodies,
 * or says why it cannot be shown.
 *
 * @remarks
 * The hosts come from the answer's own summary, through `toSystemModel`, so that the bodies and the
 * stars they orbit are one answer at one time. Every body's ID must be of the system and the list in
 * index order; every host a body or a zone names must be in the answer, and a chain of bodies must
 * end at a star, a pair or the barycentre. Orbits must be ones the client can propagate, masses and
 * bulk values positive, positions finite.
 *
 * @param designation - The system's designation of record, which the chart's answer carried.
 */
export function toSystemBodiesModel(
  response: SystemBodiesDto,
  designation: string,
): SystemBodiesResult {
  const hosts = toSystemModel(response.hosts, designation);
  if (hosts.kind === "fault") {
    return hosts;
  }
  const model = hosts.model;
  const converted = faultOf((): SystemBodies => {
    const bodies = response.bodies.map((body) => toBody(body, model.system, designation));
    check(
      bodies.every(
        (body, index) => index === 0 || body.bodyIndex > (bodies[index - 1]?.bodyIndex ?? 0),
      ),
      "body list malformed",
    );
    const result: SystemBodies = {
      granted: response.granted,
      zones: response.zones.map(toZone),
      systemPlane: response.system_plane === null ? null : toPlane(response.system_plane),
      belts: toSection(response.belts, toIds),
      halo: toSection(response.halo, (halo) => (halo === null ? null : (toIds([halo])[0] ?? null))),
      bodies,
    };
    checkHosts(model, result);
    return result;
  });
  return "fault" in converted
    ? { kind: "fault", fault: converted.fault }
    : { kind: "ok", model, bodies: converted.ok };
}

/** Whether a value is a fraction, finite and in `[0, 1]`. */
function fraction(value: number): boolean {
  return Number.isFinite(value) && value >= 0 && value <= 1;
}

/**
 * How far a mixture's mole fractions may sum from 1: 10⁻⁹, the protocol's tolerance, below which
 * rendering plan R08's column refuses a mixture (R08.T3.c).
 */
const MIXTURE_SUM_TOLERANCE = 1e-9;

/**
 * A mixture of gases, checked: each fraction in `[0, 1]`, largest first, and summing to 1 to
 * {@link MIXTURE_SUM_TOLERANCE} unless the mixture is empty. A species is any registry key, a key
 * this client does not know included.
 */
function toGases(gases: ReadonlyArray<GasShareDto>, what: string): ReadonlyArray<GasShare> {
  const sum = gases.reduce((total, gas) => total + gas.mole_fraction, 0);
  check(
    gases.every((gas) => fraction(gas.mole_fraction)) &&
      gases.every(
        (gas, index) => index === 0 || gas.mole_fraction <= (gases[index - 1]?.mole_fraction ?? 1),
      ) &&
      (gases.length === 0 || Math.abs(sum - 1) <= MIXTURE_SUM_TOLERANCE),
    `${what} unusable`,
  );
  return gases.map((gas) => ({ species: gas.species, moleFraction: gas.mole_fraction }));
}

/**
 * Areas by substance, checked: each a fraction, summing to `total` to
 * {@link MIXTURE_SUM_TOLERANCE}.
 */
function toAreas(
  areas: ReadonlyArray<SubstanceAreaDto>,
  total: number,
): ReadonlyArray<SubstanceArea> {
  const sum = areas.reduce((all, area) => all + area.area_fraction, 0);
  check(
    areas.every((area) => fraction(area.area_fraction)) &&
      Math.abs(sum - total) <= MIXTURE_SUM_TOLERANCE,
    "surface cover unusable",
  );
  return areas.map((area) => ({ substance: area.substance, areaFraction: area.area_fraction }));
}

/** A positive figure that may be `null`, as an airless body's mixture figures are. */
function positiveOrNull(value: number | null): boolean {
  return value === null || positive(value);
}

/** Profile nodes, checked: positive, their pressures in the given order. */
function toNodes(
  nodes: ReadonlyArray<ProfileNodeDto>,
  order: "ascending" | "descending",
  what: string,
): ReadonlyArray<ProfileNode> {
  check(
    nodes.every((node) => positive(node.pressure_pa) && positive(node.temperature_k)) &&
      nodes.every((node, index) => {
        const previous = nodes[index - 1];
        if (previous === undefined) {
          return true;
        }
        return order === "ascending"
          ? node.pressure_pa > previous.pressure_pa
          : node.pressure_pa < previous.pressure_pa;
      }),
    `${what} unusable`,
  );
  return nodes.map((node) => ({ pressurePa: node.pressure_pa, temperatureK: node.temperature_k }));
}

function toClimateRegime(regime: ClimateRegimeDto): ClimateRegime {
  return {
    thermal: regime.thermal,
    forcing: regime.forcing,
    condensable: regime.condensable,
    model: regime.model,
  };
}

function toSubsurfaceOcean(ocean: SubsurfaceOceanDto): SubsurfaceOcean {
  check(
    atLeastZero(ocean.shell_thickness_m) &&
      positive(ocean.thickness_m) &&
      fraction(ocean.solute_mass_fraction) &&
      positive(ocean.base_temperature_k),
    "subsurface ocean unusable",
  );
  return {
    shellThicknessM: ocean.shell_thickness_m,
    thicknessM: ocean.thickness_m,
    floor: ocean.floor,
    liquid: ocean.liquid,
    soluteMassFraction: ocean.solute_mass_fraction,
    baseTemperatureK: ocean.base_temperature_k,
  };
}

function toScreening(screening: CraterScreeningDto): CraterScreening {
  let result: CraterScreening;
  switch (screening.type) {
    case "none":
      result = { kind: "none" };
      break;
    case "atmosphere":
      check(atLeastZero(screening.column_mass_kg_m2), "crater contract unusable");
      result = { kind: "atmosphere", columnMassKgM2: screening.column_mass_kg_m2 };
      break;
    case "cutoff":
      check(positive(screening.diameter_m), "crater contract unusable");
      result = { kind: "cutoff", diameterM: screening.diameter_m };
      break;
  }
  return result;
}

function toCraters(craters: CraterContractDto): CraterContract {
  check(
    atLeastZero(craters.lunar_equivalent_n_1km_per_km2) &&
      positive(craters.projectile_density_kg_m3) &&
      positive(craters.k_target) &&
      positive(craters.target_density_kg_m3) &&
      atLeastZero(craters.system_age_s) &&
      positiveOrNull(craters.impact_velocity_m_s),
    "crater contract unusable",
  );
  return {
    lunarEquivalentN1kmPerKm2: craters.lunar_equivalent_n_1km_per_km2,
    screening: toScreening(craters.screening),
    projectileDensityKgM3: craters.projectile_density_kg_m3,
    kTarget: craters.k_target,
    targetClass: craters.target_class,
    targetDensityKgM3: craters.target_density_kg_m3,
    systemAgeS: craters.system_age_s,
    impactVelocityMS: craters.impact_velocity_m_s,
  };
}

function toWetEpoch(epoch: WetEpochDto): WetEpoch {
  check(
    atLeastZero(epoch.end_ago_s) &&
      Number.isFinite(epoch.start_ago_s) &&
      epoch.start_ago_s >= epoch.end_ago_s &&
      atLeastZero(epoch.effective_flow_yr) &&
      atLeastZero(epoch.runoff_m_per_yr) &&
      atLeastZero(epoch.paleo_inventory_kg_m2),
    "wet epoch unusable",
  );
  return {
    liquid: epoch.liquid,
    startAgoS: epoch.start_ago_s,
    endAgoS: epoch.end_ago_s,
    effectiveFlowYr: epoch.effective_flow_yr,
    runoffMPerYr: epoch.runoff_m_per_yr,
    paleoInventoryKgM2: epoch.paleo_inventory_kg_m2,
  };
}

function toVerticalStructure(structure: VerticalStructureDto): VerticalStructure {
  check(
    positive(structure.alpha) && positive(structure.skin_temperature_k),
    "vertical structure unusable",
  );
  return {
    alpha: structure.alpha,
    skinTemperatureK: structure.skin_temperature_k,
    adiabat: toNodes(structure.adiabat, "descending", "vertical structure"),
  };
}

/** The largest fractal dimension an aerosol's aggregate may have: Tazaki and Tanaka 2018's model's. */
const AGGREGATE_MAX_FRACTAL_DIMENSION = 2.5;

function toSizeDistribution(distribution: SizeDistributionDto): SizeDistribution {
  let result: SizeDistribution;
  switch (distribution.type) {
    case "gamma":
    case "log_normal":
      check(
        positive(distribution.effective_radius_m) && positive(distribution.effective_variance),
        "aerosol mode unusable",
      );
      result = {
        kind: distribution.type,
        effectiveRadiusM: distribution.effective_radius_m,
        effectiveVariance: distribution.effective_variance,
      };
      break;
    case "aggregate":
      check(
        positive(distribution.monomer_radius_m) &&
          Number.isFinite(distribution.monomer_count) &&
          distribution.monomer_count >= 1 &&
          distribution.fractal_dimension >= 1 &&
          distribution.fractal_dimension <= AGGREGATE_MAX_FRACTAL_DIMENSION &&
          positive(distribution.fractal_prefactor),
        "aerosol mode unusable",
      );
      result = {
        kind: "aggregate",
        monomerRadiusM: distribution.monomer_radius_m,
        monomerCount: distribution.monomer_count,
        fractalDimension: distribution.fractal_dimension,
        fractalPrefactor: distribution.fractal_prefactor,
      };
      break;
  }
  return result;
}

/** A layer's base and top pressures, checked: positive, the base at or below the top. */
function layerUsable(basePa: number, topPa: number): boolean {
  return positive(basePa) && positive(topPa) && basePa >= topPa;
}

/**
 * The phase of an aerosol mode: `solid` or `liquid`, since a particle is never a vapour or a
 * supercritical fluid (plan 14's P14.T24.c).
 */
function toAerosolPhase(phase: SubstancePhaseDto): AerosolPhase {
  let result: AerosolPhase;
  switch (phase) {
    case "solid":
    case "liquid":
      result = phase;
      break;
    case "vapour":
    case "supercritical":
      throw new Unusable("aerosol phase unusable");
  }
  return result;
}

function toAerosolMode(mode: AerosolModeDto): AerosolMode {
  check(
    atLeastZero(mode.column_mass_kg_m2) &&
      atLeastZero(mode.tau_550) &&
      layerUsable(mode.base_pa, mode.top_pa) &&
      atLeastZero(mode.f_sed),
    "aerosol mode unusable",
  );
  return {
    material: mode.material,
    phase: toAerosolPhase(mode.phase),
    deck: mode.deck,
    shape: mode.shape,
    sizeDistribution: toSizeDistribution(mode.size_distribution),
    columnMassKgM2: mode.column_mass_kg_m2,
    tau550: mode.tau_550,
    basePa: mode.base_pa,
    topPa: mode.top_pa,
    fSed: mode.f_sed,
  };
}

function toAbsorber(absorber: AbsorberDto): Absorber {
  const profile = absorber.profile;
  check(
    atLeastZero(absorber.column_m2) &&
      layerUsable(absorber.base_pa, absorber.top_pa) &&
      (profile === null ||
        profile.every(
          (node, index) =>
            positive(node.pressure_pa) &&
            fraction(node.relative_mixing_ratio) &&
            node.pressure_pa < (profile[index - 1]?.pressure_pa ?? Number.POSITIVE_INFINITY),
        )),
    "absorber unusable",
  );
  return {
    species: absorber.species,
    columnPerM2: absorber.column_m2,
    basePa: absorber.base_pa,
    topPa: absorber.top_pa,
    profile:
      profile === null
        ? null
        : profile.map((node) => ({
            pressurePa: node.pressure_pa,
            relativeMixingRatio: node.relative_mixing_ratio,
          })),
  };
}

function toAerosols(inventory: AerosolInventoryDto): AerosolInventory {
  check(
    inventory.decks.every((deck) => fraction(deck.vapour_mole_fraction_above)),
    "condensate deck unusable",
  );
  return {
    modes: inventory.modes.map(toAerosolMode),
    decks: inventory.decks.map((deck) => ({
      condensate: deck.condensate,
      vapourMoleFractionAbove: deck.vapour_mole_fraction_above,
    })),
    absorbers: inventory.absorbers.map(toAbsorber),
  };
}

/** The phase of a surface condensate: never `vapour`, since it is condensed (P14.T51.a). */
function toCondensatePhase(phase: SubstancePhaseDto): CondensatePhase {
  let result: CondensatePhase;
  switch (phase) {
    case "solid":
    case "liquid":
    case "supercritical":
      result = phase;
      break;
    case "vapour":
      throw new Unusable("condensate unusable");
  }
  return result;
}

function toCondensates(
  condensates: ReadonlyArray<SurfaceCondensateDto>,
): ReadonlyArray<SurfaceCondensate> {
  check(
    condensates.every(
      (condensate) => atLeastZero(condensate.reservoir_gel_m) && fraction(condensate.area_fraction),
    ),
    "condensate unusable",
  );
  return condensates.map((condensate) => ({
    substance: condensate.substance,
    phase: toCondensatePhase(condensate.phase),
    reservoirGelM: condensate.reservoir_gel_m,
    areaFraction: condensate.area_fraction,
    seasonal: condensate.seasonal,
  }));
}

/** Whether a crust's lid names one of its present members. */
function lidPresent(crust: CrustCompositionDto): boolean {
  let present: boolean;
  switch (crust.lid) {
    case "primary":
      present = crust.primary !== null;
      break;
    case "secondary":
      present = crust.secondary !== null;
      break;
    case "bulk":
      present = crust.bulk !== null;
      break;
  }
  return present;
}

/**
 * A crust, checked: `bulk` alone or at least one of `primary` and `secondary`, its lid a present
 * member, its melt area a fraction (plan 14's P14.T51.c).
 */
function toCrust(crust: CrustCompositionDto): CrustComposition {
  const crusts = [crust.primary, crust.secondary, crust.tertiary];
  check(
    (crust.bulk === null
      ? crust.primary !== null || crust.secondary !== null
      : crusts.every((member) => member === null)) &&
      lidPresent(crust) &&
      Number.isFinite(crust.mantle_redox_iw) &&
      fraction(crust.melt_area_fraction),
    "crust unusable",
  );
  return {
    primary: crust.primary,
    secondary: crust.secondary,
    tertiary: crust.tertiary,
    bulk: crust.bulk,
    lid: crust.lid,
    provinces: crust.provinces,
    redox: crust.redox,
    mantleRedoxIw: crust.mantle_redox_iw,
    meltAreaFraction: crust.melt_area_fraction,
  };
}

function toModifiers(modifiers: SurfaceModifiersDto): SurfaceModifiers {
  const deposits = modifiers.volcanic_deposits;
  const weathered = modifiers.weathering_deposits;
  const deposited = [...deposits, ...weathered].reduce(
    (all, deposit) => all + deposit.area_fraction,
    0,
  );
  check(
    fraction(modifiers.maturity) &&
      atLeastZero(modifiers.weathering_rate) &&
      fraction(modifiers.organic_cover) &&
      atLeastZero(modifiers.resurfacing_m_per_yr) &&
      deposits.every((deposit) => fraction(deposit.area_fraction)) &&
      weathered.every((deposit) => fraction(deposit.area_fraction)) &&
      deposited <= 1 + MIXTURE_SUM_TOLERANCE,
    "surface modifiers unusable",
  );
  return {
    maturity: modifiers.maturity,
    weatheringRate: modifiers.weathering_rate,
    organicCover: modifiers.organic_cover,
    resurfacingMPerYr: modifiers.resurfacing_m_per_yr,
    volcanicDeposits: deposits.map((deposit) => ({
      substance: deposit.substance,
      areaFraction: deposit.area_fraction,
    })),
    weatheringDeposits: weathered.map((deposit) => ({
      substance: deposit.substance,
      areaFraction: deposit.area_fraction,
    })),
  };
}

function toBiosphere(biosphere: BiosphereDto): Biosphere {
  check(
    biosphere.fluxes.every((flux) => Number.isFinite(flux.flux_earth)),
    "biosphere unusable",
  );
  return {
    fluxes: biosphere.fluxes.map((flux) => ({ species: flux.species, fluxEarth: flux.flux_earth })),
  };
}

/**
 * A melt's own vapour (P14.T35.f), checked for nothing, since its type has no members until
 * P14.T55.b: a newer server's value reads as a value, never as a fault in the record.
 */
function toSilicateVapour(vapour: SilicateVapourDto): SilicateVapour {
  return vapour;
}

/**
 * A surface section, checked as the photometry is (plan 14, P14.T35.e): temperatures positive,
 * contrasts finite and the day–night one not negative, pressures, fluxes and figures not
 * negative, every fraction in `[0, 1]`, the gases summing to 1 and largest first, the liquids and
 * ices summing to the ocean and ice fractions.
 *
 * @remarks
 * A substance is never checked against a list, so that a key this client does not know, a newer
 * server's, still reads (decision-p14-t35e-wire; a check refusing it would bump the protocol). A
 * member a later task fills is read absent as not modelled, `null` as none.
 */
function toSurface(surface: BodySurfaceDto): BodySurface {
  check(
    positive(surface.mean_surface_temperature_k) &&
      Number.isFinite(surface.equator_pole_contrast_k) &&
      atLeastZero(surface.day_night_contrast_k) &&
      atLeastZero(surface.surface_pressure_pa) &&
      atLeastZero(surface.stellar_flux_w_m2) &&
      positiveOrNull(surface.mean_molar_mass_g_per_mol) &&
      positiveOrNull(surface.heat_capacity_over_r),
    "surface conditions unusable",
  );
  check(
    [
      surface.ocean_fraction,
      surface.ice_fraction,
      surface.cloud_fraction,
      surface.continental_fraction,
    ].every(fraction),
    "surface cover unusable",
  );
  check(
    [
      surface.sigma_h_m,
      surface.heat_flow_w_m2,
      surface.volcanism_level,
      surface.outgassing_earth,
      surface.surface_age_s,
    ].every(atLeastZero),
    "global figures unusable",
  );
  return {
    state: surface.state,
    material: surface.material,
    meanSurfaceTemperatureK: surface.mean_surface_temperature_k,
    equatorPoleContrastK: surface.equator_pole_contrast_k,
    dayNightContrastK: surface.day_night_contrast_k,
    surfacePressurePa: surface.surface_pressure_pa,
    gases: toGases(surface.gases, "surface gases"),
    meanMolarMassGPerMol: surface.mean_molar_mass_g_per_mol,
    heatCapacityOverR: surface.heat_capacity_over_r,
    stellarFluxWM2: surface.stellar_flux_w_m2,
    radiationClass: surface.radiation_class,
    liquidWater: surface.liquid_water,
    climateRegime: toClimateRegime(surface.climate_regime),
    oceanFraction: surface.ocean_fraction,
    iceFraction: surface.ice_fraction,
    cloudFraction: surface.cloud_fraction,
    surfaceLiquids: toAreas(surface.surface_liquids, surface.ocean_fraction),
    surfaceIces: toAreas(surface.surface_ices, surface.ice_fraction),
    sigmaHM: surface.sigma_h_m,
    continentalFraction: surface.continental_fraction,
    heatFlowWM2: surface.heat_flow_w_m2,
    tectonicRegime: surface.tectonic_regime,
    volcanismLevel: surface.volcanism_level,
    outgassingEarth: surface.outgassing_earth,
    magneticFieldClass: surface.magnetic_field_class,
    surfaceAgeS: surface.surface_age_s,
    subsurfaceOcean: modelled(surface.subsurface_ocean, toSubsurfaceOcean),
    craters: toCraters(surface.craters),
    wetEpoch: surface.wet_epoch === null ? null : toWetEpoch(surface.wet_epoch),
    verticalStructure: modelled(surface.vertical_structure, toVerticalStructure),
    aerosols: modelled(surface.aerosols, toAerosols),
    condensates: modelled(surface.condensates, toCondensates),
    crust: modelled(surface.crust, toCrust),
    modifiers: modelled(surface.modifiers, toModifiers),
    biosphere: modelled(surface.biosphere, toBiosphere),
    silicateVapour: modelled(surface.silicate_vapour, toSilicateVapour),
  };
}

/**
 * A gas-envelope body's envelope section, checked (plan 14's P14.T24.d, on the wire by
 * P14.T35.e): temperatures not negative, the profile's parameters and the join positive, the
 * adiabat's pressures rising from the join, none above it, the composition summing to 1 and
 * largest first.
 */
function toEnvelope(envelope: BodyEnvelopeDto): BodyEnvelope {
  const profile = envelope.profile;
  const first = envelope.adiabat[0];
  check(
    atLeastZero(envelope.internal_temperature_k) &&
      atLeastZero(envelope.irradiation_temperature_k) &&
      positive(profile.thermal_opacity_m2_per_kg) &&
      positive(profile.opacity_ratio) &&
      positive(profile.redistribution_factor) &&
      positive(envelope.join_pressure_pa) &&
      (first === undefined || first.pressure_pa >= envelope.join_pressure_pa) &&
      positive(envelope.mean_molar_mass_g_per_mol) &&
      positive(envelope.heat_capacity_over_r) &&
      atLeastZero(envelope.haze_tau_550),
    "envelope unusable",
  );
  return {
    internalTemperatureK: envelope.internal_temperature_k,
    irradiationTemperatureK: envelope.irradiation_temperature_k,
    profile: {
      thermalOpacityM2PerKg: profile.thermal_opacity_m2_per_kg,
      opacityRatio: profile.opacity_ratio,
      redistributionFactor: profile.redistribution_factor,
    },
    joinPressurePa: envelope.join_pressure_pa,
    adiabat: toNodes(envelope.adiabat, "ascending", "envelope"),
    composition: toGases(envelope.composition, "envelope composition"),
    meanMolarMassGPerMol: envelope.mean_molar_mass_g_per_mol,
    heatCapacityOverR: envelope.heat_capacity_over_r,
    decks: envelope.decks.map(toAerosolMode),
    hazeTau550: envelope.haze_tau_550,
  };
}

/** A whole record, checked and in the display's units. */
function toRecord(record: BodyRecordDto, system: SystemIdHex, designation: string): BodyRecord {
  return {
    ...toBody(record, system, designation),
    surface: toSection(record.surface, toSurface),
    envelope: toOptionalSection(record.envelope, toEnvelope),
    hooks: toSection(record.hooks, (hooks) => ({
      detailSeed: toSection(hooks.detail_seed, (seed) => {
        check(DETAIL_SEED_FORM.test(seed), "detail seed malformed");
        return seed;
      }),
    })),
  };
}

/**
 * Turns a `body_detail` answer into the display's record of the body, or says why it cannot be
 * shown.
 *
 * @param system - The system the display shows, which the record's ID must name.
 * @param designation - The system's designation of record.
 */
export function toBodyDetail(
  response: BodyDetailDto,
  system: SystemIdHex,
  designation: string,
): BodyDetailResult {
  const converted = faultOf((): BodyDetail => ({
    time: response.time,
    granted: response.granted,
    record: toRecord(response.record, system, designation),
  }));
  return "fault" in converted
    ? { kind: "fault", fault: converted.fault }
    : { kind: "ok", detail: converted.ok };
}
