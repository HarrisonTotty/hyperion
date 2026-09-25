/**
 * The pure model behind the `LOCAL CHART` panel: a range query's answer as a scene for the general
 * spatial view, and the words the census is read in.
 *
 * @remarks
 * Nothing here touches the DOM or the server. The scene's lengths are light-years, its positions
 * offsets from the chart centre along the galactic axes, and its labels the text the view draws
 * beside each curve.
 */
import type { LayerCensus, LayerStatus, MassLayer, ObjectKindDto } from "@hyperion/protocol";

import { formatNumber } from "../../lib/format";
import type { ChartCensus, ChartResult, ChartSystem, LayerIndex } from "../../lib/galaxy/model";
import { starSizeClass, starSymbol } from "../../lib/galaxy/starSymbols";
import { layerIndex } from "../../lib/galaxy/wire";
import type { LocalFrame } from "../../spatial/frame";
import type { PlaneRing, PointMark, SpatialScene, SphereMark } from "../../spatial/marks";
import { ceil125, gridSpacing, RADIUS_STEPS_LY } from "../../spatial/scale";

/**
 * The drive range a chart starts with, in light-years (plan 05, design note D9).
 *
 * @remarks
 * An operator setting until a ship and a drive exist, which is why every place that writes it says
 * `SET`.
 */
export const DEFAULT_DRIVE_RANGE_LY = 50;

/** The most decimals a chart length is written with: 0.01 ly is the smallest query radius. */
const MAX_LENGTH_DECIMALS = 4;

/** Writes a value with as few decimals as it needs to be exact, up to `mostDecimals`. */
function fewestDecimals(value: number, mostDecimals: number): string {
  let decimals = 0;
  while (decimals < mostDecimals && Number(value.toFixed(decimals)) !== value) {
    decimals += 1;
  }
  return formatNumber(value, decimals);
}

/**
 * Writes a chart length in light-years with as few decimals as it needs, without its unit.
 *
 * @remarks
 * For the curve labels and the controls, whose lengths are 1-2-5 query radii and entered drive
 * ranges, not measurements: `50`, `0.05`, `37.5`. A system's distance is a measurement and keeps
 * one precision for the whole chart instead (plan 05, design note D21).
 */
export function formatChartLengthLy(ly: number): string {
  return fewestDecimals(ly, MAX_LENGTH_DECIMALS);
}

/**
 * Writes the edge of a mass layer's band in M☉, without its unit: `0.08`, `0.5`, `150`.
 *
 * @remarks
 * The edges are exact band boundaries, not measurements, so they are written as they are defined
 * rather than to a fixed precision.
 */
export function formatBandMsun(massMsun: number): string {
  return fewestDecimals(massMsun, 2);
}

/**
 * The query radius a chart uses while the operator has not chosen one: the drive range, raised to
 * the next radius step so that everything within range is fetched.
 */
export function queryRadiusForDriveRange(driveRangeLy: number): number {
  const wanted = ceil125(driveRangeLy);
  const largest = RADIUS_STEPS_LY.at(-1);
  const smallest = RADIUS_STEPS_LY[0];
  if (largest === undefined || smallest === undefined) {
    throw new Error("the chart has no radius steps");
  }
  return Math.min(largest, Math.max(smallest, wanted));
}

/**
 * The decimals every distance on a chart of this query radius is written with.
 *
 * @remarks
 * One precision per chart, so that a column of distances lines up and a value does not change its
 * precision as the selection moves (plan 05, design note D21).
 */
export function distanceDecimalsFor(queryRadiusLy: number): number {
  if (queryRadiusLy >= 10) {
    return 2;
  }
  return queryRadiusLy >= 1 ? 3 : 4;
}

/**
 * Which systems a chart shows, by what their primary is now: every one, the living stars and brown
 * dwarfs, or the white dwarfs, neutron stars and black holes (plan 06, P06.T35.b).
 */
export type StarFilter = "all" | "living" | "remnants";

/** The filters in the order the `STARS` selector offers them and its key steps through them. */
export const STAR_FILTERS: ReadonlyArray<StarFilter> = ["all", "living", "remnants"];

/** A filter's name, as the selector and the count line write it: `ALL`, `LIVING`, `REMNANTS`. */
export function starFilterLabel(filter: StarFilter): string {
  let label: string;
  switch (filter) {
    case "all":
      label = "ALL";
      break;
    case "living":
      label = "LIVING";
      break;
    case "remnants":
      label = "REMNANTS";
      break;
  }
  return label;
}

/** The filter after `filter`, from the last back to the first, as the selector's key steps. */
export function nextStarFilter(filter: StarFilter): StarFilter {
  const next = STAR_FILTERS[(STAR_FILTERS.indexOf(filter) + 1) % STAR_FILTERS.length];
  if (next === undefined) {
    throw new Error("the STARS selector has no filters");
  }
  return next;
}

/**
 * Which `STARS` filter besides `ALL` a primary of this kind passes: `living` for a star or brown
 * dwarf that has not died, `remnants` for a white dwarf, neutron star or black hole, and `null`
 * for a star that died and left nothing, which only `ALL` shows (the owner's draft of the guide's
 * nomenclature, r9 D2.3).
 *
 * @remarks
 * A brown dwarf never burns hydrogen and never dies, so it counts with the living. Exhaustive over
 * the wire's kinds, with no default, so that a new kind is a type error here until it is placed.
 */
function filterOf(kind: ObjectKindDto): "living" | "remnants" | null {
  let filter: "living" | "remnants" | null;
  switch (kind) {
    case "protostar":
    case "pre_main_sequence":
    case "dwarf":
    case "subgiant":
    case "giant":
    case "supergiant":
    case "wolf_rayet":
    case "hot_subdwarf":
    case "substellar":
      filter = "living";
      break;
    case "white_dwarf":
    case "neutron_star":
    case "black_hole":
      filter = "remnants";
      break;
    case "no_remnant":
      filter = null;
      break;
  }
  return filter;
}

/**
 * Whether a system passes a chart's `STARS` filter.
 *
 * @remarks
 * A system not yet formed at the chart's time has no primary, and a star that left no remnant has
 * nothing left, so only `ALL` shows either.
 */
export function passesStarFilter(system: ChartSystem, filter: StarFilter): boolean {
  if (filter === "all") {
    return true;
  }
  return system.star !== null && filterOf(system.star.kind) === filter;
}

/** The systems of a chart that pass its filter, in the chart's order; all of them under `ALL`. */
export function filterSystems(
  systems: ReadonlyArray<ChartSystem>,
  filter: StarFilter,
): ReadonlyArray<ChartSystem> {
  return filter === "all" ? systems : systems.filter((system) => passesStarFilter(system, filter));
}

/**
 * How many of a chart's systems are shown, saying what the filter hides: the total alone under
 * `ALL`, and `412 OF 1630 SHOWN: LIVING` otherwise (the guide's honest-data rule; a colon, not the
 * plan's em dash, which is the Missing state: the owner's draft, r9 D2.3).
 */
export function shownCountText(shown: number, total: number, filter: StarFilter): string {
  if (filter === "all") {
    return formatNumber(total, 0);
  }
  return `${formatNumber(shown, 0)} OF ${formatNumber(total, 0)} SHOWN: ${starFilterLabel(filter)}`;
}

/** What a chart's scene is built from besides the query's answer. */
export interface ChartSceneOptions {
  /** The named directions at the chart centre, which the grid and the triad follow. */
  readonly frame: LocalFrame;
  /** How far the ship could go, an operator setting in M1 (plan 05, design note D9). */
  readonly driveRangeLy: number;
  readonly selectedId: string | null;
  /** The commanded destination; always `null` in M1, which commands nothing. */
  readonly destinationId: string | null;
  /** Which systems are drawn; `all` when absent. */
  readonly starFilter?: StarFilter | undefined;
}

/** Whether the drive range's circle and plane ring belong on a chart of this query radius. */
function rangeIsShown(radiusLy: number, driveRangeLy: number): boolean {
  return driveRangeLy <= radiusLy;
}

function rangeLabel(driveRangeLy: number): string {
  return `RANGE ${formatChartLengthLy(driveRangeLy)} ly SET`;
}

/**
 * The plane ring's label, which says `SET` as the range sphere's does: the two curves draw the same
 * operator-set value a short distance apart, and one of them left unmarked would read as a
 * measurement (plan 05, design note D9, and the orchestrator's ruling 10).
 */
function planeLabel(driveRangeLy: number): string {
  return `PLANE ${formatChartLengthLy(driveRangeLy)} ly SET`;
}

/**
 * A system's mark, or `null` for one with nothing to draw: a star that left no remnant, or a system
 * not yet formed, which are listed and not drawn (plan 06, design note 17).
 */
function toMark(system: ChartSystem, driveRangeLy: number, index: LayerIndex): PointMark | null {
  const shape = system.star === null ? null : starSymbol(system.star.kind);
  if (shape === null) {
    return null;
  }
  return {
    id: system.id,
    position: system.relLy,
    shape,
    sizeClass: starSizeClass(shape, index),
    // The 3D distance at the chart time, so a system in front of or behind the sphere, which
    // projects inside its circle, is not taken for one within range.
    status: system.distanceLy <= driveRangeLy ? "available" : "plain",
    label: system.designation,
    labelPriority: system.initialMassMsun,
  };
}

/** The grid's rings out to the query radius, with the drive range's ring labelled among them. */
function planeRings(radiusLy: number, spacing: number, driveRangeLy: number): PlaneRing[] {
  const rings: PlaneRing[] = [];
  const labelled = rangeIsShown(radiusLy, driveRangeLy);
  for (let ring = 1; ring * spacing <= radiusLy * (1 + 1e-12); ring += 1) {
    const radius = ring * spacing;
    rings.push({
      radius,
      label: labelled && radius === driveRangeLy ? planeLabel(driveRangeLy) : "",
    });
  }
  if (labelled && !rings.some((ring) => ring.radius === driveRangeLy)) {
    rings.push({ radius: driveRangeLy, label: planeLabel(driveRangeLy) });
    rings.sort((a, b) => a.radius - b.radius);
  }
  return rings;
}

/**
 * Turns a range query's answer into the scene the spatial view draws.
 *
 * @remarks
 * The query radius is the result's own, never what was asked for, so the circles always measure
 * what is drawn. A system is available, and so drawn in `--accent`, when its distance from the
 * chart centre is within the drive range. Each mark takes its primary's symbol and size class
 * (`starSymbol`, `starSizeClass`); only the systems that pass the `STARS` filter are drawn, and a
 * star that left no remnant or a system not yet formed is not drawn at all. The drive range is
 * drawn twice, as the guide's 3D
 * conventions require: as a sphere, whose outline is a circle at every angle, and as a ring on the
 * reference plane, which measures distance within the plane alone.
 */
export function toScene(result: ChartResult, options: ChartSceneOptions): SpatialScene {
  const { frame, driveRangeLy, selectedId, destinationId, starFilter = "all" } = options;
  const spacing = gridSpacing(result.radiusLy);
  const spheres: SphereMark[] = [];
  if (rangeIsShown(result.radiusLy, driveRangeLy)) {
    spheres.push({ radius: driveRangeLy, role: "range", label: rangeLabel(driveRangeLy) });
  }
  spheres.push({
    radius: result.radiusLy,
    role: "data_edge",
    label: `QUERY EDGE ${formatChartLengthLy(result.radiusLy)} ly`,
  });
  return {
    frame,
    points: filterSystems(result.systems, starFilter).flatMap((system) => {
      const mark = toMark(system, driveRangeLy, layerIndex(system.layer));
      return mark === null ? [] : [mark];
    }),
    spheres,
    plane: {
      spacing,
      extent: result.radiusLy,
      rings: planeRings(result.radiusLy, spacing, driveRangeLy),
    },
    selectedId,
    destinationId,
  };
}

/** How many of these systems lie within the drive range. */
export function inRangeCount(systems: ReadonlyArray<ChartSystem>, driveRangeLy: number): number {
  return systems.filter((system) => system.distanceLy <= driveRangeLy).length;
}

/** The census line under a chart: what the result is complete above, or that nothing fits. */
export type CensusLine =
  | { readonly kind: "complete"; readonly text: string; readonly aboveMsun: number }
  | { readonly kind: "nothing_fits"; readonly text: string };

/**
 * The words for a chart's census.
 *
 * @remarks
 * `COMPLETE ABOVE` is followed by the mass and its drawn unit, so the caller sets them. Nothing
 * fitting the census limit is an answer, not an error, and says what the operator can do.
 */
export function censusLine(census: ChartCensus): CensusLine {
  let line: CensusLine;
  switch (census.kind) {
    case "complete":
      line = { kind: "complete", text: "COMPLETE ABOVE", aboveMsun: census.aboveMsun };
      break;
    case "nothing_fits":
      line = { kind: "nothing_fits", text: "NOTHING FITS: reduce radius" };
      break;
  }
  return line;
}

/**
 * What the operator can do about a layer the server left out, or `null` when it left out none.
 *
 * @remarks
 * A layer over the census limit is a dense region, where the brainstorm's range query prefers a
 * smaller radius to a higher mass floor; a layer over the server's cell budget is a large volume.
 */
export function censusHint(layers: ReadonlyArray<LayerCensus>): string | null {
  let dense = false;
  let large = false;
  for (const layer of layers) {
    const status: LayerStatus = layer.status;
    switch (status) {
      case "over_limit":
        dense = true;
        break;
      case "over_cell_budget":
        large = true;
        break;
      case "included":
      case "below_mass_floor":
        break;
    }
  }
  if (dense) {
    return "DENSE REGION: reduce radius before raising MIN MASS";
  }
  return large ? "LARGE VOLUME: layers dropped by the server cell budget; reduce radius" : null;
}

/** One mass layer's band, as the legend and the mass floors read it. */
export interface LayerBand {
  readonly layer: MassLayer;
  /** The layer's place from the lightest, which is also its symbol's size class. */
  readonly index: LayerIndex;
  /** The lower edge of the band, in M☉ of primary initial mass. */
  readonly minMsun: number;
  /** The upper edge of the band, in M☉ of primary initial mass. */
  readonly maxMsun: number;
}

/** How many mass layers a census lists: plan 04's five, `a` to `e`. */
const LAYER_COUNT = 5;

/**
 * The five mass bands of a chart's census, lightest first, or `null` when the census does not list
 * all five layers once each.
 *
 * @remarks
 * The edges come from the census the server sent with the answer, so the client holds no copy of
 * the band table. A census that does not list every layer is an unusable answer, not a bug in the
 * client, so it is reported rather than thrown: see {@link chartDataFault}.
 */
export function layerBands(layers: ReadonlyArray<LayerCensus>): ReadonlyArray<LayerBand> | null {
  const bands = layers.map((layer): LayerBand => ({
    layer: layer.layer,
    index: layerIndex(layer.layer),
    minMsun: layer.mass_min_msun,
    maxMsun: layer.mass_max_msun,
  }));
  const seen = new Set(bands.map((band) => band.index));
  if (bands.length !== LAYER_COUNT || seen.size !== LAYER_COUNT) {
    return null;
  }
  return bands.toSorted((a, b) => a.index - b.index);
}

/**
 * Why a range query's answer cannot be charted, in words, or `null` when it can.
 *
 * @remarks
 * The wire is decoded but not validated, so an answer the client cannot use must read as a fault
 * and not take the display down: the counterpart of the galaxy map's `MAP DATA INVALID` (plan 05,
 * T8.d). The brainstorm's range query answers with a complete census per layer, so a census that
 * does not name every layer leaves it unknown what the answer is complete above, and the whole
 * answer goes unshown.
 */
export function chartDataFault(result: ChartResult): string | null {
  if (!(result.radiusLy > 0) || !Number.isFinite(result.radiusLy)) {
    return "query radius unusable";
  }
  return layerBands(result.layers) === null ? "census incomplete" : null;
}
