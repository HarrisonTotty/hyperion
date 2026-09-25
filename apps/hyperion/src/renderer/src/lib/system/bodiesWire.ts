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
  type BodyDetailDto,
  type BodyIdHex,
  type BodyKindDto,
  type BodyOrbitDto,
  type BodyRecordDto,
  type BodyStateDto,
  type BodySummaryDto,
  type BulkPropertiesDto,
  type HabitableZoneDto,
  isBodyId,
  type OrbitHostDto,
  parseBodyId,
  type PopulationDto,
  type RequestOf,
  type SectionDto,
  type SystemBodiesDto,
  type SystemIdHex,
  type SystemPlaneDto,
  type UniverseIdHex,
  type UniverseTime,
  type ZoneDto,
} from "@hyperion/protocol";

import { vec3 } from "../../spatial/vec3";
import { NEAR_PARABOLIC_ECCENTRICITY } from "../orbit";
import type {
  BodyDetail,
  BodyKind,
  BodyOrbit,
  BodyRecord,
  BodyState,
  BulkProperties,
  HabitableZone,
  HierarchyNode,
  OrbitHost,
  Population,
  Section,
  SystemBodies,
  SystemBody,
  SystemModel,
  SystemPlane,
  Zone,
} from "./model";
import { bodyDesignation, toKeplerOrbit, toSystemModel } from "./wire";

/**
 * The simulation's Earth mass, in kilograms: GM⊕ ÷ G, 3.986 004 × 10¹⁴ m³ s⁻² over
 * 6.674 30 × 10⁻¹¹ m³ kg⁻¹ s⁻², `units::consts::EARTH_MASS_KG` in `hyperion-sim`, the same quotient
 * to the bit (the orchestrator's ruling 64.3).
 */
export const EARTH_MASS_KG = 3.986_004e14 / 6.674_3e-11;

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

function usableTime(time: UniverseTime): boolean {
  return (
    Number.isSafeInteger(time.seconds) &&
    Number.isInteger(time.nanos) &&
    time.nanos >= 0 &&
    time.nanos < 1_000_000_000
  );
}

function toState(state: BodyStateDto): BodyState {
  let result: BodyState;
  switch (state.type) {
    case "not_yet_formed":
    case "present":
      result = { kind: state.type };
      break;
    case "destroyed":
      check(usableTime(state.at), "state time unusable");
      result = { kind: "destroyed", cause: state.cause, at: state.at };
      break;
    case "unbound":
      check(usableTime(state.at), "state time unusable");
      result = { kind: "unbound", at: state.at };
      break;
  }
  return result;
}

function toOrbit(orbit: BodyOrbitDto): BodyOrbit {
  const kepler = toKeplerOrbit(orbit.orbit);
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

/** A whole record, checked and in the display's units. */
function toRecord(record: BodyRecordDto, system: SystemIdHex, designation: string): BodyRecord {
  return {
    ...toBody(record, system, designation),
    // `BodySurfaceDto` has no value yet, so no `ok` surface can parse; its value passes as it is.
    surface: toSection(record.surface, (surface) => surface),
    hooks: toSection(record.hooks, (hooks) => {
      check(/^[0-9a-f]{16}$/.test(hooks.surface_seed), "surface seed malformed");
      return { surfaceSeed: hooks.surface_seed };
    }),
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
