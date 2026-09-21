/**
 * The adapter between plan 04's range-query messages and the client's chart model.
 *
 * @remarks
 * The only module outside `test/` that reads the wire's unit-suffixed field names (plan 05,
 * design note D8). Positions are exact on the wire as a light-year cell plus a metre offset, and
 * every offset from the chart centre is taken with `galacticDeltaLy`, which subtracts cells before
 * offsets, so a chart in hundredths of a light-year keeps its precision anywhere in the galaxy.
 */
import {
  type GalacticPosition,
  galacticDeltaLy,
  galacticPositionFromLy,
  type MassLayer,
  type Population,
  type RequestOf,
  type SystemRecord,
  type SystemsInRange,
  type UniverseIdHex,
  universeTimeFromYears,
  universeTimeToYears,
} from "@hyperion/protocol";

import type { Vec3 } from "../../spatial/vec3";
import {
  type CentreLy,
  CHART_SYSTEM_LIMIT,
  type ChartCensus,
  type ChartResult,
  type ChartSystem,
  type LayerIndex,
} from "./model";

/** The origin of the `GALACTIC` frame, from which absolute positions are measured. */
const ORIGIN: GalacticPosition = { cell_ly: [0, 0, 0], offset_m: [0, 0, 0] };

function toVec3([x, y, z]: readonly [number, number, number]): Vec3 {
  return { x, y, z };
}

function toChartSystem(centre: GalacticPosition, record: SystemRecord): ChartSystem {
  const relLy = toVec3(galacticDeltaLy(centre, record.position));
  return {
    id: record.id,
    designation: record.designation,
    relLy,
    distanceLy: Math.hypot(relLy.x, relLy.y, relLy.z),
    positionLy: toVec3(galacticDeltaLy(ORIGIN, record.position)),
    layer: record.layer,
    population: record.population,
    initialMassMsun: record.initial_mass_msun,
    ageMyr: record.age_myr,
  };
}

/** Nearest first; equal distances in ID order, so that the order never depends on the wire's. */
function byDistanceThenId(a: ChartSystem, b: ChartSystem): number {
  if (a.distanceLy !== b.distanceLy) {
    return a.distanceLy - b.distanceLy;
  }
  // IDs are all 16 lower-case hex digits, so string order is numeric order.
  if (a.id === b.id) {
    return 0;
  }
  return a.id < b.id ? -1 : 1;
}

function toChartCensus(completeAboveMsun: number | null): ChartCensus {
  return completeAboveMsun === null
    ? { kind: "nothing_fits" }
    : { kind: "complete", aboveMsun: completeAboveMsun };
}

/**
 * Turns a range query's answer into a chart.
 *
 * @remarks
 * The centre, radius and time come from the response's echo of its request, never from what the
 * client asked for, so that what is drawn is what was answered.
 */
export function toChartResult(response: SystemsInRange): ChartResult {
  const [x, y, z] = galacticDeltaLy(ORIGIN, response.centre);
  return {
    centreLy: [x, y, z],
    radiusLy: response.radius_ly,
    timeYr: universeTimeToYears(response.time),
    census: toChartCensus(response.census.complete_above_msun),
    layers: response.census.layers,
    systems: response.systems
      .map((record) => toChartSystem(response.centre, record))
      .toSorted(byDistanceThenId),
  };
}

/**
 * Builds the range query for a chart.
 *
 * @remarks
 * The centre is carried exactly as a cell and an offset (`galacticPositionFromLy`), so a negative
 * coordinate falls in the cell below it. Every query carries {@link CHART_SYSTEM_LIMIT}.
 *
 * @param centreLy - The chart centre in the `GALACTIC` frame.
 * @param radiusLy - The query radius.
 * @param timeYr - The chart time in years from the epoch.
 * @param minLayer - The lightest layer wanted.
 * @throws RangeError when a coordinate or the time is not finite.
 */
export function toRangeRequest(
  universe: UniverseIdHex,
  centreLy: CentreLy,
  radiusLy: number,
  timeYr: number,
  minLayer: MassLayer,
): RequestOf<"systems_in_range"> {
  return {
    kind: "systems_in_range",
    universe,
    centre: galacticPositionFromLy(centreLy),
    radius_ly: radiusLy,
    time: universeTimeFromYears(timeYr),
    min_layer: minLayer,
    limit: CHART_SYSTEM_LIMIT,
  };
}

/** The upper-case name of a stellar population, as the readout shows it. */
export function populationLabel(population: Population): string {
  let label: string;
  switch (population) {
    case "young_thin_disc":
      label = "YOUNG THIN DISC";
      break;
    case "old_thin_disc":
      label = "OLD THIN DISC";
      break;
    case "thick_disc":
      label = "THICK DISC";
      break;
    case "bulge":
      label = "BULGE";
      break;
    case "long_bar":
      label = "LONG BAR";
      break;
    case "nuclear_disc":
      label = "NUCLEAR DISC";
      break;
    case "halo":
      label = "HALO";
      break;
  }
  return label;
}

/** A mass layer's place from the lightest: A is 0 and E is 4. */
export function layerIndex(layer: MassLayer): LayerIndex {
  let index: LayerIndex;
  switch (layer) {
    case "a":
      index = 0;
      break;
    case "b":
      index = 1;
      break;
    case "c":
      index = 2;
      break;
    case "d":
      index = 3;
      break;
    case "e":
      index = 4;
      break;
  }
  return index;
}
