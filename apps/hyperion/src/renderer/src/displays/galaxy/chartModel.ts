/**
 * The pure model behind the `LOCAL CHART` panel: a range query's answer as a scene for the general
 * spatial view, and the words the census is read in.
 *
 * @remarks
 * Nothing here touches the DOM or the server. The scene's lengths are light-years, its positions
 * offsets from the chart centre along the galactic axes, and its labels the text the view draws
 * beside each curve.
 */
import type { LayerCensus, LayerStatus, MassLayer } from "@hyperion/protocol";

import { formatNumber } from "../../lib/format";
import type { ChartCensus, ChartResult, LayerIndex } from "../../lib/galaxy/model";
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

/** What a chart's scene is built from besides the query's answer. */
export interface ChartSceneOptions {
  /** The named directions at the chart centre, which the grid and the triad follow. */
  readonly frame: LocalFrame;
  /** How far the ship could go, an operator setting in M1 (plan 05, design note D9). */
  readonly driveRangeLy: number;
  readonly selectedId: string | null;
  /** The commanded destination; always `null` in M1, which commands nothing. */
  readonly destinationId: string | null;
}

/** Whether the drive range's circle and plane ring belong on a chart of this query radius. */
function rangeIsShown(radiusLy: number, driveRangeLy: number): boolean {
  return driveRangeLy <= radiusLy;
}

function rangeLabel(driveRangeLy: number): string {
  return `RANGE ${formatChartLengthLy(driveRangeLy)} ly SET`;
}

function planeLabel(driveRangeLy: number): string {
  return `PLANE ${formatChartLengthLy(driveRangeLy)} ly`;
}

function toMark(
  system: ChartResult["systems"][number],
  driveRangeLy: number,
  index: LayerIndex,
): PointMark {
  return {
    id: system.id,
    position: system.relLy,
    shape: "circle",
    sizeClass: index,
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
 * chart centre is within the drive range. The drive range is drawn twice, as the guide's 3D
 * conventions require: as a sphere, whose outline is a circle at every angle, and as a ring on the
 * reference plane, which measures distance within the plane alone.
 */
export function toScene(result: ChartResult, options: ChartSceneOptions): SpatialScene {
  const { frame, driveRangeLy, selectedId, destinationId } = options;
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
    points: result.systems.map((system) => toMark(system, driveRangeLy, layerIndex(system.layer))),
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

/** How many systems of a chart lie within the drive range. */
export function inRangeCount(result: ChartResult, driveRangeLy: number): number {
  return result.systems.filter((system) => system.distanceLy <= driveRangeLy).length;
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
