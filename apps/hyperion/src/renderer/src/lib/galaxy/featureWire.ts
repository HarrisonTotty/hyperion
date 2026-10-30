/**
 * The adapter between plan 09's feature messages and the client's model of galaxy data.
 *
 * @remarks
 * With `wire.ts`, the only modules outside `test/` that read the wire's unit-suffixed field names
 * (plan 05, design note D8). Positions are exact on the wire as a light-year cell plus a metre
 * offset, and every offset from the chart centre is taken with `galacticDeltaLy`, which subtracts
 * cells before offsets, as the range query's rows are.
 */
import {
  type BlackHoleState,
  type BrightShell,
  type CatalogueClassWire,
  type ClassCountWire,
  type FeatureDetail,
  type FeatureIdHex,
  type FeatureKindWire,
  type FeaturesInRange,
  type FeatureSummary,
  type GalacticPosition,
  type GalaxyFeatures,
  galacticDeltaLy,
  galacticPositionFromLy,
  type MapFeatureKindWire,
  type MapFeatureList,
  type MemberClassWire,
  type RemnantSummary,
  type RequestOf,
  type ShellSummary,
  type SupernovaStateWire,
  type SystemOriginWire,
  type UniverseIdHex,
  universeTimeFromYears,
  universeTimeToYears,
} from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import type {
  BlackHoleModel,
  CatalogueClass,
  CentreLy,
  ChartFeature,
  ChartFeatures,
  ClassCount,
  FeatureDetailModel,
  FeatureStructure,
  MapFeature,
  MapFeatureLayer,
  MapShell,
  MemberClass,
  RemnantState,
  ShellState,
  SupernovaState,
  SystemOrigin,
} from "./model";
import { toChartLayer } from "./wire";

/** The origin of the `GALACTIC` frame, from which absolute positions are measured. */
const ORIGIN: GalacticPosition = { cell_ly: [0, 0, 0], offset_m: [0, 0, 0] };

function toVec3([x, y, z]: readonly [number, number, number]): Vec3 {
  return { x, y, z };
}

/**
 * Every kind a chart asks for, in plan 09's order: the catalogue's five, the centre, and plan 10's
 * streams and dwarf cores, which the census says are not generated until plan 10 places them, so
 * that the chart says so rather than show them as none.
 */
export const CHART_FEATURE_KINDS: ReadonlyArray<FeatureKindWire> = [
  "globular",
  "open_cluster",
  "association",
  "star_forming_region",
  "molecular_cloud",
  "galactic_centre",
  "stream",
  "dwarf_core",
];

/** One feature as the chart holds it, its offset taken from `centre`. */
export function toChartFeature(centre: GalacticPosition, summary: FeatureSummary): ChartFeature {
  const relLy = toVec3(galacticDeltaLy(centre, summary.position));
  return {
    id: summary.id,
    designation: summary.designation,
    kind: summary.kind,
    relLy,
    distanceLy: Math.hypot(relLy.x, relLy.y, relLy.z),
    positionLy: toVec3(galacticDeltaLy(ORIGIN, summary.position)),
    extentLy: summary.extent_ly,
    ageMyr: summary.age_myr,
    massMsun: summary.mass_msun,
    expectedMembers: summary.expected_members,
    emission: summary.emission,
    bubbleRadiusLy: summary.bubble_radius_ly,
  };
}

/** Nearest first; equal distances in ID order, so that the order never depends on the wire's. */
function byDistanceThenId(a: ChartFeature, b: ChartFeature): number {
  if (a.distanceLy !== b.distanceLy) {
    return a.distanceLy - b.distanceLy;
  }
  // IDs are all 16 lower-case hex digits, so string order is numeric order.
  if (a.id === b.id) {
    return 0;
  }
  return a.id < b.id ? -1 : 1;
}

/**
 * Turns a features-in-range answer into the chart's features.
 *
 * @remarks
 * The centre, radius and time come from the response's echo of its request, so that what is drawn
 * is what was answered, and a chart draws the features only of an answer for its own centre,
 * radius and time. The features come in the server's order, which the client sorts for itself.
 */
export function toChartFeatures(response: FeaturesInRange): ChartFeatures {
  const [x, y, z] = galacticDeltaLy(ORIGIN, response.centre);
  return {
    centreLy: [x, y, z],
    radiusLy: response.radius_ly,
    timeYr: universeTimeToYears(response.time),
    census: response.census,
    features: response.features
      .map((summary) => toChartFeature(response.centre, summary))
      .toSorted(byDistanceThenId),
  };
}

/**
 * Builds the features-in-range query for a chart: the chart's own centre, radius and time, every
 * kind of {@link CHART_FEATURE_KINDS}.
 *
 * @throws RangeError when a coordinate or the time is not finite.
 */
export function toFeaturesRequest(
  universe: UniverseIdHex,
  centreLy: CentreLy,
  radiusLy: number,
  timeYr: number,
): RequestOf<"features_in_range"> {
  return {
    kind: "features_in_range",
    universe,
    centre: galacticPositionFromLy(centreLy),
    radius_ly: radiusLy,
    time: universeTimeFromYears(timeYr),
    kinds: [...CHART_FEATURE_KINDS],
  };
}

/** Every kind the galaxy map's overlay asks for, in the order its toggles offer them. */
export const MAP_FEATURE_KINDS: ReadonlyArray<MapFeatureKindWire> = [
  "globular",
  "galactic_centre",
  "bright_shell",
];

function toMapFeature(summary: FeatureSummary): MapFeature {
  return {
    id: summary.id,
    designation: summary.designation,
    kind: summary.kind,
    positionLy: toVec3(galacticDeltaLy(ORIGIN, summary.position)),
    extentLy: summary.extent_ly,
    massMsun: summary.mass_msun,
  };
}

function toMapShell(shell: BrightShell): MapShell {
  return {
    system: shell.system,
    designation: shell.designation,
    siteLy: toVec3(galacticDeltaLy(ORIGIN, shell.site)),
    radiusLy: shell.shell.radius_ly,
  };
}

/** One kind's list of the map's answer as the overlay holds it. */
function toMapFeatureLayer(list: MapFeatureList): MapFeatureLayer {
  let layer: MapFeatureLayer;
  switch (list.kind) {
    case "globular":
    case "galactic_centre":
      layer =
        list.status === "complete"
          ? { kind: list.kind, status: "complete", features: list.features.map(toMapFeature) }
          : { kind: list.kind, status: "not_generated" };
      break;
    case "bright_shell":
      layer =
        list.status === "complete"
          ? { kind: list.kind, status: "complete", shells: list.shells.map(toMapShell) }
          : { kind: list.kind, status: "not_generated" };
      break;
  }
  return layer;
}

/**
 * Turns the galaxy map's features answer into the overlay's layers, one for each kind in the
 * answer's order (plan 09, P09.T41.b).
 */
export function toMapFeatureLayers(response: GalaxyFeatures): ReadonlyArray<MapFeatureLayer> {
  return response.lists.map(toMapFeatureLayer);
}

/**
 * Builds the galaxy map's features request: every kind of {@link MAP_FEATURE_KINDS}, at the epoch,
 * the time the map's own pictures are of.
 */
export function toGalaxyFeaturesRequest(universe: UniverseIdHex): RequestOf<"galaxy_features"> {
  return {
    kind: "galaxy_features",
    universe,
    kinds: [...MAP_FEATURE_KINDS],
    time: universeTimeFromYears(0),
  };
}

/**
 * Every origin this build knows, by its tag: the enum grows (plan 04's reserved list), so a newer
 * server's origin may be one this build lacks, which the decoder, trusting the generated types,
 * hands on as one of these.
 */
const KNOWN_ORIGINS: Readonly<Record<SystemOriginWire["type"], true>> = {
  grid: true,
  feature_member: true,
  centre_member: true,
  catalogue_system: true,
};

/** Every catalogue class this build knows: the registry's nine, which grow as its origins do. */
const KNOWN_CATALOGUE_CLASSES: Readonly<Record<CatalogueClassWire, true>> = {
  core_collapse: true,
  type_ia: true,
  stellar_merger: true,
  neutron_star_merger: true,
  luminous_blue_variable: true,
  xray_binary: true,
  accreting_white_dwarf: true,
  tidal_disruption_victim: true,
  accreting_white_dwarf_slow: true,
};

/** Every member class this build knows, by its tag, which grow as the origins do. */
const KNOWN_MEMBER_CLASSES: Readonly<Record<MemberClassWire["type"], true>> = {
  cluster: true,
  centre: true,
  listed: true,
};

/** A catalogue class as the wire names it, or `unknown` for one this build does not know. */
export function toCatalogueClass(value: CatalogueClassWire): CatalogueClass {
  return Object.hasOwn(KNOWN_CATALOGUE_CLASSES, value) ? value : "unknown";
}

/**
 * What placed a range row's system (plan 09, P09.T39): `not_sent` for a row without the key, from a
 * server that does not say, and `unknown` for an origin this build does not know, so that the
 * readout's switches stay exhaustive and the reading is Missing.
 */
export function toSystemOrigin(origin: SystemOriginWire | undefined): SystemOrigin {
  if (origin === undefined) {
    return { kind: "not_sent" };
  }
  if (!Object.hasOwn(KNOWN_ORIGINS, origin.type)) {
    return { kind: "unknown" };
  }
  let model: SystemOrigin;
  switch (origin.type) {
    case "grid":
    case "centre_member":
      model = { kind: origin.type };
      break;
    case "feature_member":
      model = { kind: "feature_member", feature: origin.feature };
      break;
    case "catalogue_system":
      model = { kind: "catalogue_system", catalogueClass: toCatalogueClass(origin.class) };
      break;
  }
  return model;
}

/** A member's class, or `unknown` for one this build does not know. */
export function toMemberClass(memberClass: MemberClassWire): MemberClass {
  if (!Object.hasOwn(KNOWN_MEMBER_CLASSES, memberClass.type)) {
    return { kind: "unknown" };
  }
  let model: MemberClass;
  switch (memberClass.type) {
    case "cluster":
      model = {
        kind: "cluster",
        classKind: memberClass.kind,
        band: toChartLayer(memberClass.band),
        generation: memberClass.generation,
        multiplicity: memberClass.multiplicity,
      };
      break;
    case "centre":
      model = {
        kind: "centre",
        classKind: memberClass.kind,
        band: toChartLayer(memberClass.band),
        episode: memberClass.episode,
        tracer: memberClass.tracer,
      };
      break;
    case "listed":
      model = { kind: "listed", catalogueClass: toCatalogueClass(memberClass.class) };
      break;
  }
  return model;
}

function toShellState(shell: ShellSummary): ShellState {
  return {
    radiusLy: shell.radius_ly,
    shockSpeedKmS: shell.shock_speed_km_s,
    phase: shell.phase,
    xRayAndRadio: shell.emission.x_ray_and_radio,
    optical: shell.emission.optical,
    neutralHydrogen: shell.emission.neutral_hydrogen,
  };
}

function toRemnantState(remnant: RemnantSummary): RemnantState {
  return {
    offsetLy: remnant.offset_ly,
    bowShock: remnant.bow_shock,
    nebulaPowerW: remnant.nebula?.power_w ?? null,
    survivorOffsetLy: remnant.survivor_offset_ly,
  };
}

/** A supernova catalogue entry's state at the query's time (plan 09, P09.T39). */
export function toSupernovaState(state: SupernovaStateWire): SupernovaState {
  let model: SupernovaState;
  switch (state.type) {
    case "progenitor":
      model = {
        kind: "progenitor",
        untilExplosionYr: state.until_explosion_yr,
        pairPeriodS: state.pair_period_s,
      };
      break;
    case "supernova":
      model = {
        kind: "supernova",
        ageYr: state.age_yr,
        shell: toShellState(state.shell),
        remnant: toRemnantState(state.remnant),
      };
      break;
    case "bare_remnant":
      model = { kind: "bare_remnant", ageYr: state.age_yr, remnant: toRemnantState(state.remnant) };
      break;
  }
  return model;
}

function toClassCount(count: ClassCountWire): ClassCount {
  return {
    memberClass: toMemberClass(count.class),
    initialMassMinMsun: count.initial_mass_min_msun,
    initialMassMaxMsun: count.initial_mass_max_msun,
    meanMassMsun: count.mean_mass_msun,
    expected: count.expected,
  };
}

/** A feature's detail answer, its structure by what the feature is (plan 09, P09.T40.b). */
export function toFeatureDetail(response: FeatureDetail): FeatureDetailModel {
  const { structure } = response;
  let model: FeatureStructure;
  switch (structure.type) {
    case "cluster":
      model = {
        kind: "cluster",
        coreRadiusLy: structure.core_radius_ly,
        halfMassRadiusLy: structure.half_mass_radius_ly,
        tidalRadiusLy: structure.tidal_radius_ly,
        centralEscapeSpeedKmS: structure.central_escape_speed_km_s,
        relaxationTimeMyr: structure.relaxation_time_myr,
        coreCollapsed: structure.core_collapsed,
        blackHoles: structure.black_holes,
        classes: structure.classes.map(toClassCount),
      };
      break;
    case "centre":
      model = {
        kind: "centre",
        influenceRadiusLy: structure.influence_radius_ly,
        lossConeRadiusLy: structure.loss_cone_radius_ly,
        classes: structure.classes.map(toClassCount),
      };
      break;
    case "cloud":
      model = {
        kind: "cloud",
        coreRadiusLy: structure.core_radius_ly,
        centralDensityPerCm3: structure.central_density_per_cm3,
      };
      break;
    case "association":
      model = {
        kind: "association",
        radiusLy: structure.radius_ly,
        expansionSpeedKmS: structure.expansion_speed_km_s,
      };
      break;
  }
  return {
    id: response.feature.id,
    timeYr: universeTimeToYears(response.time),
    structure: model,
  };
}

/** The black hole's answer (plan 09, P09.T40.b). */
export function toBlackHole(response: BlackHoleState): BlackHoleModel {
  return {
    timeYr: universeTimeToYears(response.time),
    massMsun: response.mass_msun,
    luminosityW: response.luminosity?.total_w ?? null,
    disruptions: response.disruptions.map((disruption) => ({
      victim: disruption.victim,
      timeYr: universeTimeToYears(disruption.time),
      starMassMsun: disruption.star_mass_msun,
      outcome: disruption.outcome,
      peakLuminosityW: disruption.peak_luminosity_w,
    })),
  };
}

/**
 * Builds the request for one feature's detail at the chart's time.
 *
 * @throws RangeError when the time is not finite.
 */
export function toFeatureDetailRequest(
  universe: UniverseIdHex,
  feature: FeatureIdHex,
  timeYr: number,
): RequestOf<"feature_detail"> {
  return { kind: "feature_detail", universe, feature, time: universeTimeFromYears(timeYr) };
}

/**
 * Builds the request for the central black hole's state at the chart's time.
 *
 * @throws RangeError when the time is not finite.
 */
export function toBlackHoleRequest(
  universe: UniverseIdHex,
  timeYr: number,
): RequestOf<"black_hole_state"> {
  return { kind: "black_hole_state", universe, time: universeTimeFromYears(timeYr) };
}
