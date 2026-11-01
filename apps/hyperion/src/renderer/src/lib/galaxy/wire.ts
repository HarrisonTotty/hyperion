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
  type LayerCensus,
  type LayerStatus,
  type MassLayer,
  type ObservedDto,
  type PlacementKind,
  type Population,
  type QueryModeDto,
  type RequestOf,
  type StellarBriefDto,
  type SystemRecord,
  type SystemsInRange,
  type UniverseIdHex,
  universeTimeFromYears,
  universeTimeToYears,
} from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import { toMemberClass, toSupernovaState, toSystemOrigin } from "./featureWire";
import {
  type CentreLy,
  type ChartKind,
  type ChartLayer,
  type ChartLayerLine,
  type ChartLayerStatus,
  type ChartPopulation,
  CHART_SYSTEM_LIMIT,
  type ChartCensus,
  type ChartResult,
  type ChartSystem,
  type LayerIndex,
  NOW_MODE,
  type Observation,
  type QueryMode,
  type StarBrief,
} from "./model";

/** The origin of the `GALACTIC` frame, from which absolute positions are measured. */
const ORIGIN: GalacticPosition = { cell_ly: [0, 0, 0], offset_m: [0, 0, 0] };

/** Every mass layer this build knows; one added to the protocol without an entry is a type error. */
const MASS_LAYERS: Readonly<Record<MassLayer, true>> = {
  a: true,
  b: true,
  c: true,
  d: true,
  e: true,
  brown_dwarf: true,
  rogue_planet: true,
};

/** Every stellar population this build knows, likewise. */
const POPULATIONS: Readonly<Record<Population, true>> = {
  young_thin_disc: true,
  old_thin_disc: true,
  thick_disc: true,
  bulge: true,
  long_bar: true,
  nuclear_disc: true,
  halo: true,
};

/** Every layer status this build knows, likewise. */
const LAYER_STATUSES: Readonly<Record<LayerStatus, true>> = {
  included: true,
  over_limit: true,
  below_mass_floor: true,
  over_cell_budget: true,
};

// The three enums grow without a protocol bump (galaxy plan 04's reserved list), and the decoder
// trusts the generated types, so a newer server's string arrives typed as one this build knows.

/**
 * A mass layer as the chart reads it, `unknown` for one this build does not know: a row's layer,
 * and a feature member's class's band (plan 09, P09.T41.c).
 */
export function toChartLayer(layer: MassLayer): ChartLayer {
  return Object.hasOwn(MASS_LAYERS, layer) ? layer : "unknown";
}

function toChartPopulation(population: Population): ChartPopulation {
  return Object.hasOwn(POPULATIONS, population) ? population : "unknown";
}

function toChartLayerStatus(status: LayerStatus): ChartLayerStatus {
  return Object.hasOwn(LAYER_STATUSES, status) ? status : "unknown";
}

function toChartLayerLine(line: LayerCensus): ChartLayerLine {
  return { ...line, layer: toChartLayer(line.layer), status: toChartLayerStatus(line.status) };
}

function toVec3([x, y, z]: readonly [number, number, number]): Vec3 {
  return { x, y, z };
}

function toStarBrief(brief: StellarBriefDto): StarBrief {
  return {
    kind: brief.kind,
    spectralClass: brief.class,
    logLuminosityLsun: brief.log_luminosity_lsun,
    teffK: brief.teff_k,
    starCount: brief.star_count,
  };
}

/**
 * What an observed row carries, in the client's units: the apparent position as an offset from the
 * chart centre, taken with `galacticDeltaLy` as the row's own is, so that it keeps its precision
 * anywhere in the galaxy.
 */
function toObservation(centre: GalacticPosition, observed: ObservedDto): Observation {
  return {
    emittedYr: universeTimeToYears(observed.emitted),
    lightAgeYr: observed.light_age_yr,
    apparentRelLy: toVec3(galacticDeltaLy(centre, observed.apparent_position)),
    curvatureBoundLy: observed.curvature_error_ly,
    curvatureBoundArcsec: observed.curvature_error_arcsec,
  };
}

function toChartSystem(centre: GalacticPosition, record: SystemRecord): ChartSystem {
  const relLy = toVec3(galacticDeltaLy(centre, record.position));
  const layer = toChartLayer(record.layer);
  return {
    id: record.id,
    designation: record.designation,
    relLy,
    distanceLy: Math.hypot(relLy.x, relLy.y, relLy.z),
    positionLy: toVec3(galacticDeltaLy(ORIGIN, record.position)),
    layer,
    kind: chartKindOf(layer),
    population: toChartPopulation(record.population),
    // Only a grid system's row from a server built since P08.T15.a carries it.
    placement: record.placement ?? null,
    initialMassMsun: record.initial_mass_msun,
    ageMyr: record.age_myr,
    // Every chart query asks for the briefs, so a row without one is a system not yet formed, or
    // a free-floating planet, which never has one.
    star: record.stellar === undefined ? null : toStarBrief(record.stellar),
    // Only a free-floating planet's row carries its metallicity (plan 13, P13.T5.d); any other
    // system's is its summary's.
    feHDex: layer === "rogue_planet" ? (record.fe_h_dex ?? null) : null,
    velocityKmS: toVec3(record.velocity_km_s),
    // Only a row answered in observed mode carries it: one from a server that ignored the mode is
    // the present's, whatever was asked (the port ruling, §6.2).
    observed: record.observed === undefined ? null : toObservation(centre, record.observed),
    // A row from a server that does not say leaves the three out (plan 09, P09.T39), and an origin
    // or a class this build does not know reads as unknown, so that the readout's switches stay
    // exhaustive.
    origin: toSystemOrigin(record.origin),
    memberClass: record.member_class === undefined ? null : toMemberClass(record.member_class),
    supernova: record.supernova === undefined ? null : toSupernovaState(record.supernova),
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

/**
 * The census the chart reads: complete above the edge the server sent, named with the lightest
 * layer it included, or nothing fitting.
 *
 * @remarks
 * A layer this build does not know has no rank, so it is the lightest only where its band starts
 * below that of every known layer included.
 */
function toChartCensus(
  aboveMsun: number | null,
  layers: ReadonlyArray<ChartLayerLine>,
): ChartCensus {
  if (aboveMsun === null) {
    return { kind: "nothing_fits" };
  }
  const included = layers.filter((line) => line.status === "included");
  const lightest = included
    .filter((line) => line.layer !== "unknown")
    .toSorted((a, b) => layerRank(a.layer) - layerRank(b.layer))[0];
  const lighterUnknown = included.some(
    (line) =>
      line.layer === "unknown" && line.mass_min_msun < (lightest?.mass_min_msun ?? Infinity),
  );
  return {
    kind: "complete",
    aboveMsun,
    layer: lighterUnknown ? "unknown" : (lightest?.layer ?? "a"),
  };
}

/**
 * Turns a range query's answer into a chart.
 *
 * @remarks
 * The centre, radius and time come from the response's echo of its request, never from what the
 * client asked for, so that what is drawn is what was answered. The answer echoes no mode, so the
 * mode is the one of the request it answers, which the caller passes; each row says for itself
 * whether it was answered in it.
 *
 * @param mode - The mode of the request this answers; `now` when absent.
 */
export function toChartResult(response: SystemsInRange, mode: QueryMode = NOW_MODE): ChartResult {
  const [x, y, z] = galacticDeltaLy(ORIGIN, response.centre);
  const layers = response.census.layers.map(toChartLayerLine);
  return {
    centreLy: [x, y, z],
    radiusLy: response.radius_ly,
    timeYr: universeTimeToYears(response.time),
    mode,
    census: toChartCensus(response.census.complete_above_msun, layers),
    layers,
    systems: response.systems
      .map((record) => toChartSystem(response.centre, record))
      .toSorted(byDistanceThenId),
  };
}

/**
 * The wire's form of a mode, or `undefined` for `now`, which a request leaves out (the port ruling,
 * §6.1: a request without `mode` is `now`).
 *
 * @remarks
 * The observer is carried exactly as a cell and an offset, as a chart's centre is.
 *
 * @throws RangeError when a coordinate of the observer is not finite.
 */
export function toQueryModeDto(mode: QueryMode): QueryModeDto | undefined {
  let dto: QueryModeDto | undefined;
  switch (mode.kind) {
    case "now":
      dto = undefined;
      break;
    case "observed":
      dto = { type: "observed", observer: galacticPositionFromLy(mode.observerLy) };
      break;
  }
  return dto;
}

/**
 * Builds the range query for a chart.
 *
 * @remarks
 * The centre is carried exactly as a cell and an offset (`galacticPositionFromLy`), so a negative
 * coordinate falls in the cell below it. Every query carries {@link CHART_SYSTEM_LIMIT} and asks
 * for each row's stellar brief, from which the chart draws its symbols and the HR diagram its
 * points. Only an observed query carries `mode`, so that a query in `now` is byte for byte the one
 * a server built before plan 12 reads (the port ruling, §6.1).
 *
 * @param centreLy - The chart centre in the `GALACTIC` frame.
 * @param radiusLy - The query radius.
 * @param timeYr - The chart time in years from the epoch.
 * @param minLayer - The lightest layer wanted.
 * @param mode - The mode to ask in; `now` when absent.
 * @throws RangeError when a coordinate, the observer's included, or the time is not finite.
 */
export function toRangeRequest(
  universe: UniverseIdHex,
  centreLy: CentreLy,
  radiusLy: number,
  timeYr: number,
  minLayer: MassLayer,
  mode: QueryMode = NOW_MODE,
): RequestOf<"systems_in_range"> {
  const request: RequestOf<"systems_in_range"> = {
    kind: "systems_in_range",
    universe,
    centre: galacticPositionFromLy(centreLy),
    radius_ly: radiusLy,
    time: universeTimeFromYears(timeYr),
    min_layer: minLayer,
    limit: CHART_SYSTEM_LIMIT,
    include_stellar: true,
  };
  const dto = toQueryModeDto(mode);
  return dto === undefined ? request : { ...request, mode: dto };
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

/** The upper-case name of a grid system's placement, as its `ORIGIN` reads it (plan 08, P08.T15.a). */
export function placementLabel(placement: PlacementKind): string {
  let label: string;
  switch (placement) {
    case "field":
      label = "FIELD";
      break;
    case "retained":
      label = "RETAINED";
      break;
    case "displaced_remnant":
      label = "DISPLACED REMNANT";
      break;
    case "runaway":
      label = "RUNAWAY";
      break;
    case "walkaway":
      label = "WALKAWAY";
      break;
    case "hypervelocity_survivor":
      label = "HYPERVELOCITY SURVIVOR";
      break;
  }
  return label;
}

/**
 * A grid system's `ORIGIN` in words, its birth population and how its grid placed it, as the readout
 * and the list's row both write it (plan 08, P08.T15.a): `OLD THIN DISC · DISPLACED REMNANT`, the
 * population the em dash where this build does not know it, as the guide composes a reading with a
 * part Missing; `null` for a row without a placement.
 */
export function placedOriginWords(system: ChartSystem): string | null {
  if (system.placement === null) {
    return null;
  }
  const population = system.population === "unknown" ? "—" : populationLabel(system.population);
  return `${population} · ${placementLabel(system.placement)}`;
}

/**
 * A mass layer's size class, its place from the lightest stellar layer: A is 0 and E is 4, and the
 * brown dwarfs and free-floating planets take layer A's 0 (plan 13, design note 15).
 */
export function layerIndex(layer: MassLayer): LayerIndex {
  let index: LayerIndex;
  switch (layer) {
    case "rogue_planet":
    case "brown_dwarf":
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

/**
 * A mass layer's place from the lightest of all seven: the free-floating planets 0, the brown
 * dwarfs 1, then A to E 2 to 6, the order the census reads from the lightest; and 7, after them,
 * for a layer this build does not know, whose place only its band can say.
 */
export function layerRank(layer: ChartLayer): number {
  let rank: number;
  switch (layer) {
    case "rogue_planet":
      rank = 0;
      break;
    case "brown_dwarf":
      rank = 1;
      break;
    case "a":
    case "b":
    case "c":
    case "d":
    case "e":
      rank = 2 + layerIndex(layer);
      break;
    case "unknown":
      rank = 7;
      break;
  }
  return rank;
}

/**
 * What a layer's objects are: star systems, free-floating brown dwarfs or free-floating planets, or
 * not known for a layer this build does not know.
 */
export function chartKindOf(layer: ChartLayer): ChartKind {
  let kind: ChartKind;
  switch (layer) {
    case "a":
    case "b":
    case "c":
    case "d":
    case "e":
      kind = "stellar";
      break;
    case "brown_dwarf":
      kind = "brown_dwarf";
      break;
    case "rogue_planet":
      kind = "rogue_planet";
      break;
    case "unknown":
      kind = "unknown";
      break;
  }
  return kind;
}

/**
 * The letter a layer is named by in the census: `A` to `E` for the stellar layers, and `F` and `G`
 * for the brown dwarfs and the free-floating planets, the letters their designations carry (plan
 * 01's layer letters); `null`, which reads Missing, for a layer this build does not know.
 */
export function layerLetter(layer: ChartLayer): string | null {
  let letter: string | null;
  switch (layer) {
    case "a":
    case "b":
    case "c":
    case "d":
    case "e":
      letter = layer.toUpperCase();
      break;
    case "brown_dwarf":
      letter = "F";
      break;
    case "rogue_planet":
      letter = "G";
      break;
    case "unknown":
      letter = null;
      break;
  }
  return letter;
}
