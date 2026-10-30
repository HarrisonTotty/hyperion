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
  type BrightShell,
  type FeatureKindWire,
  type FeaturesInRange,
  type FeatureSummary,
  type GalacticPosition,
  type GalaxyFeatures,
  galacticDeltaLy,
  galacticPositionFromLy,
  type MapFeatureKindWire,
  type MapFeatureList,
  type RequestOf,
  type UniverseIdHex,
  universeTimeFromYears,
  universeTimeToYears,
} from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import type {
  CentreLy,
  ChartFeature,
  ChartFeatures,
  MapFeature,
  MapFeatureLayer,
  MapShell,
} from "./model";

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
