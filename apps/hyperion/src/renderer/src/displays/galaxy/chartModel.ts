/**
 * The pure model behind the `LOCAL CHART` panel: a range query's answer as a scene for the general
 * spatial view, and the words the census is read in.
 *
 * @remarks
 * Nothing here touches the DOM or the server. The scene's lengths are light-years, its positions
 * offsets from the chart centre along the galactic axes, and its labels the text the view draws
 * beside each curve.
 */
import type { FeatureIdHex, MassLayer, ObjectKindDto, SystemIdHex } from "@hyperion/protocol";

import {
  AU_PER_LY,
  formatNumber,
  formatScaleLength,
  formatSigned,
  formatSignificant,
  formatSubstellarMass,
  MEARTH_PER_MSUN,
  MJUP_PER_MSUN,
  SCALE_AU_BELOW_LY,
} from "../../lib/format";
import {
  type ChartCensus,
  type ChartFeature,
  type ChartFeatures,
  type ChartLayer,
  type ChartLayerLine,
  type ChartLayerStatus,
  type ChartResult,
  type ChartSystem,
  type LayerIndex,
  NOW_MODE,
  type Observation,
  type QueryMode,
  shownAgeMyr,
} from "../../lib/galaxy/model";
import { featureMarkLabel } from "../../lib/galaxy/featureWords";
import { chartSymbol, starSizeClass } from "../../lib/galaxy/starSymbols";
import { layerIndex, layerRank } from "../../lib/galaxy/wire";
import type { LocalFrame } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import { APPARENT_OFFSET_MIN_PX } from "../../spatial/apparent";
import type {
  FeatureMark,
  PlaneRing,
  PointMark,
  SegmentMark,
  ShellMark,
  SizeClass,
  SpatialScene,
  SphereMark,
} from "../../spatial/marks";
import { ceil125, gridSpacing, RADIUS_STEPS_LY } from "../../spatial/scale";

/**
 * The drive range a chart starts with, in light-years (plan 05, design note D9).
 *
 * @remarks
 * An operator setting until a ship and a drive exist, which is why every place that writes it says
 * `SET`.
 */
export const DEFAULT_DRIVE_RANGE_LY = 50;

/**
 * The most decimals a chart length is written with in light-years: 0.01 ly is the smallest query
 * radius read in them.
 */
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
 * Whether a query radius is one of the centre's fine steps, below 0.01 ly, whose lengths the chart
 * reads in astronomical units, as the scale bar does (`SCALE_AU_BELOW_LY`).
 *
 * @remarks
 * The tolerance is `formatScaleLength`'s, so that a computed 0.01 ly does not fall to AU through
 * rounding in the last bit and the selector, the query edge and the distances agree.
 */
function readsInAu(queryRadiusLy: number): boolean {
  return queryRadiusLy < SCALE_AU_BELOW_LY * (1 - 1e-9);
}

/**
 * Writes a query radius with its unit: `50 ly`, `0.01 ly`, and below 0.01 ly in astronomical units
 * to three figures, `316 AU`, `63.2 AU` (plan 09, P09.T42).
 *
 * @remarks
 * For the `QUERY RADIUS` selector's steps and the `QUERY EDGE` label, so that the two write a
 * radius alike, and for the drive range's `RANGE` and `PLANE` labels, which the query radius follows
 * (plan 05, design note D9): `RANGE 94.9 AU SET`. The fine steps are 1-2-5 values in light-years,
 * 0.005 ly to 0.001 ly, read in the unit the scale bar takes below 0.01 ly
 * ({@link formatScaleLength}).
 */
export function formatQueryRadius(ly: number): string {
  return readsInAu(ly) ? formatScaleLength(ly) : `${formatChartLengthLy(ly)} ly`;
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

/** The decimals a distance keeps in astronomical units, on a chart of the centre's fine steps. */
const AU_DISTANCE_DECIMALS = 1;

/** The unit and precision every distance on one chart is written in. */
export interface ChartDistanceUnit {
  /** The unit's symbol, beside each value and in a list's `DIST` header: `ly` or `AU`. */
  readonly symbol: "ly" | "AU";
  /** How many of the unit make one light-year. */
  readonly perLy: number;
  /** The decimals every distance is written with. */
  readonly decimals: number;
}

/**
 * The unit and precision every distance on a chart of this query radius is written in: the lists'
 * `DIST` columns, and the readouts' `DIST` and offsets.
 *
 * @remarks
 * In light-years at {@link distanceDecimalsFor}'s decimals, down to a radius of 0.01 ly; on a
 * narrower chart, the centre's fine steps, in astronomical units to 0.1 AU, as the scale bar and
 * the radius selector read there (plan 09, P09.T42). One unit and one precision per chart, so that
 * a column of distances lines up and no value changes its unit as the selection moves; they are
 * static, so no hysteresis is needed (plan 05, design note D21).
 */
export function distanceUnitFor(queryRadiusLy: number): ChartDistanceUnit {
  return readsInAu(queryRadiusLy)
    ? { symbol: "AU", perLy: AU_PER_LY, decimals: AU_DISTANCE_DECIMALS }
    : { symbol: "ly", perLy: 1, decimals: distanceDecimalsFor(queryRadiusLy) };
}

/** Writes a distance on a chart in the chart's unit, without it: `0.0042`, `41.3`. */
export function formatChartDistance(ly: number, unit: ChartDistanceUnit): string {
  return formatNumber(ly * unit.perLy, unit.decimals);
}

/**
 * The most characters a distance takes in a list's `DIST` column, its own room: `~500.00`, a system
 * read as observed on a chart from 10 ly (`styles.css`, `.system-list__row`).
 */
export const DISTANCE_COLUMN_CH = 7;

/**
 * Writes a feature's distance on a chart in the chart's unit, without it: at the chart's precision
 * while that fits the `DIST` column, and past it to the guide's three significant figures, grouped
 * and in E notation from 1,000,000 (`126,000`, `300`, `6.64E8`; plan 09, P09.T42).
 *
 * @remarks
 * A feature is listed where its sphere reaches the chart, so its centre may lie far beyond the
 * query edge: 2 ly from a chart below 0.01 ly is `126,482.2` AU, and 300 ly from a chart below 1 ly
 * is `300.0000`, each wider than the column. The unit stays the chart's, which the column's header
 * names for every row, and the value keeps the fixed field the guide asks for (§ Numbers, units and
 * time). The readout writes the selected feature's distance the same way, so that one quantity
 * reads alike in both. A system lies within the query radius, so its distance always fits.
 */
export function formatFeatureDistance(ly: number, unit: ChartDistanceUnit): string {
  const text = formatChartDistance(ly, unit);
  return text.length <= DISTANCE_COLUMN_CH ? text : formatSignificant(ly * unit.perLy);
}

/** Writes a signed offset on a chart in the chart's unit, without it: `+0.0042`, `-41.3`. */
export function formatChartOffset(ly: number, unit: ChartDistanceUnit): string {
  return formatSigned(ly * unit.perLy, unit.decimals);
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
    case "stripped_helium_star":
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
 * nothing left, so only `ALL` shows either. A free-floating brown dwarf is living, as its brief
 * says; a free-floating planet is no star, living or dead, and only `ALL` shows it (plan 13,
 * P13.T8.d).
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

/** How many of a chart's systems are shown, in its two parts: {@link shownCount}'s. */
export interface ShownCount {
  /** The count, a value of the answer: `412 OF 1630`, or the total alone under `ALL`, `1630`. */
  readonly figure: string;
  /** What the filter hides, the console's own choice in words, `SHOWN: LIVING`; none under `ALL`. */
  readonly filterWords: string | null;
}

/**
 * How many of a chart's systems are shown, saying what the filter hides: the total alone under
 * `ALL`, and `412 OF 1630 SHOWN: LIVING` otherwise (the guide's honest-data rule; a colon, not the
 * plan's em dash, which is the Missing state: the owner's draft, r9 D2.3).
 *
 * @remarks
 * The figure and the filter's words are given apart, since a stale answer's `S` follows the figure
 * and comes before the words, `9 OF 88 S SHOWN: REMNANTS`: the filter is the console's own choice,
 * not a value of the answer (plan 05, P05.T14). The census and the HR page read the same parts.
 */
export function shownCount(shown: number, total: number, filter: StarFilter): ShownCount {
  if (filter === "all") {
    return { figure: formatNumber(total, 0), filterWords: null };
  }
  return {
    figure: `${formatNumber(shown, 0)} OF ${formatNumber(total, 0)}`,
    filterWords: `SHOWN: ${starFilterLabel(filter)}`,
  };
}

/** What a chart has selected: a system, or a large feature (plan 09, P09.T41.a). */
export type ChartSelection =
  | { readonly kind: "system"; readonly id: SystemIdHex }
  | { readonly kind: "feature"; readonly id: FeatureIdHex };

/**
 * What a feature's mark's ID starts with: a feature's ID is also a well-formed member's system ID
 * (plan 09, P09.T39), so the two must never share a mark's ID on one chart.
 */
const FEATURE_MARK_PREFIX = "feature:";

/** The ID of a feature's mark on a chart. */
export function featureMarkId(id: FeatureIdHex): string {
  return `${FEATURE_MARK_PREFIX}${id}`;
}

/** The ID of a selection's mark on a chart: a system's own ID, or its feature's mark's. */
export function selectionMarkId(selection: ChartSelection): string {
  return selection.kind === "system" ? selection.id : featureMarkId(selection.id);
}

/** What a mark picked on a chart selects: the feature whose mark it is, or the system it is. */
export function selectionOfMark(markId: string): ChartSelection {
  return markId.startsWith(FEATURE_MARK_PREFIX)
    ? { kind: "feature", id: markId.slice(FEATURE_MARK_PREFIX.length) }
    : { kind: "system", id: markId };
}

/**
 * A feature mark's size class: the largest, 1 rem across, since a feature is a group of systems
 * or a cloud and its Greek cross must read open south of the plane, its arms a third of its span.
 */
export const FEATURE_SIZE_CLASS: SizeClass = 4;

/**
 * Where a feature's label ranks among the chart's: above every system's, whose rank is its initial
 * mass of at most 150 M☉, since a feature's kind is named by its label alone on the chart (the
 * guide's 3D conventions) where a system's is named by its shape; and among the features by mass.
 */
const FEATURE_LABEL_PRIORITY = 1e12;

/** A superbubble's label at its circle: what it is, and the nursery it belongs to. */
export function superbubbleLabel(designation: string): string {
  return `SUPERBUBBLE ${designation}`;
}

/** A feature's mark: the Greek cross at its centre, labelled with its kind and designation. */
function toFeatureMark(feature: ChartFeature): FeatureMark {
  return {
    id: featureMarkId(feature.id),
    position: feature.relLy,
    shape: "greek-cross",
    sizeClass: FEATURE_SIZE_CLASS,
    // Colour is kept free for status, and a feature has none on the chart yet.
    status: "plain",
    label: featureMarkLabel(feature.kind, feature.designation),
    labelPriority: FEATURE_LABEL_PRIORITY + feature.massMsun,
    radius: feature.extentLy,
  };
}

/** A nursery's superbubble, a circle at its true radius about the feature, or none. */
function toBubble(feature: ChartFeature): ShellMark | null {
  return feature.bubbleRadiusLy === null
    ? null
    : {
        id: `bubble:${feature.id}`,
        centre: feature.relLy,
        radius: feature.bubbleRadiusLy,
        label: superbubbleLabel(feature.designation),
      };
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
  /**
   * The features in range, from an answer for the chart's own centre, radius and time (plan 09,
   * P09.T41.a); none when absent.
   */
  readonly features?: ReadonlyArray<ChartFeature> | undefined;
}

/** Whether the drive range's circle and plane ring belong on a chart of this query radius. */
function rangeIsShown(radiusLy: number, driveRangeLy: number): boolean {
  return driveRangeLy <= radiusLy;
}

function rangeLabel(driveRangeLy: number): string {
  return `RANGE ${formatQueryRadius(driveRangeLy)} SET`;
}

/**
 * The plane ring's label, which says `SET` as the range sphere's does: the two curves draw the same
 * operator-set value a short distance apart, and one of them left unmarked would read as a
 * measurement (plan 05, design note D9, and the orchestrator's ruling 10).
 */
function planeLabel(driveRangeLy: number): string {
  return `PLANE ${formatQueryRadius(driveRangeLy)} SET`;
}

/**
 * A system's mark, or `null` for one with nothing to draw: a star that left no remnant, or a system
 * not yet formed, which are listed and not drawn (plan 06, design note 17). A free-floating planet
 * is an inverted triangle at layer A's size (plan 13, design note 15).
 */
function toMark(system: ChartSystem, driveRangeLy: number, index: LayerIndex): PointMark | null {
  // As observed, a free-floating planet is drawn once it had formed when its light left.
  const shape = chartSymbol(system.kind, system.star?.kind ?? null, shownAgeMyr(system) > 0);
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

/** The label of the observer's mark on the chart (plan 12, P12.T11). */
export const OBSERVER_LABEL = "OBSERVER";

/**
 * The mode a chart's answer reads in (plan 12, P12.T11): the one it was asked in, but `now` for
 * one asked observed whose rows all came without their observation, as a server that ignores the
 * mode answers, so that the chart never claims a mode its answer was not given in. An answer with
 * no rows keeps the mode asked, which nothing contradicts.
 */
export function answeredMode(result: ChartResult): QueryMode {
  const ignored =
    result.mode.kind === "observed" &&
    result.systems.length > 0 &&
    result.systems.every((system) => system.observed === null);
  return ignored ? NOW_MODE : result.mode;
}

/**
 * The dashed joiner from a drawn system's mark to its apparent position, with the tick across its
 * end there (plan 12, P12.T11), or none for a row answered without an observation.
 *
 * @remarks
 * 1 px in `--text-muted`, which carries meaning at 6:1 where `--line` does not; the selected
 * system's in `--text` 2 px wide, as a selected orbit is, so that the selection is not carried by
 * colour alone (the line ruling). The selected system's is drawn whatever its offset; any other's
 * only where its offset on the screen exceeds {@link APPARENT_OFFSET_MIN_PX}. Both are drawn only
 * outside the mark's outline, so that neither crosses the symbol: with the observer at the chart
 * centre the offsets are under a pixel at the fitted zoom.
 */
function apparentSegment(system: ChartSystem, selected: boolean): SegmentMark | null {
  if (system.observed === null) {
    return null;
  }
  return {
    id: `apparent:${system.id}`,
    from: system.relLy,
    to: system.observed.apparentRelLy,
    line: "dashed",
    stroke: selected ? "text" : "textMuted",
    widthPx: selected ? 2 : 1,
    fromMark: system.id,
    endTick: true,
    minLengthPx: selected ? 0 : APPARENT_OFFSET_MIN_PX,
  };
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
 *
 * In observed mode the chart still draws every system at its present position (plan 12, Design
 * note 4), with its symbol from the brief the row carries, which is its primary when the light
 * left. Each drawn system whose row carries an observation has its apparent position's tick and
 * dashed joiner ({@link apparentSegment}), the selected one's last, so that it lies on top; a row
 * without one has none. The observer of a chart answered in observed mode ({@link answeredMode})
 * is drawn where it lies inside the view, labelled `OBSERVER`; one asked observed and answered in
 * `now`, by a server that ignores the mode, draws none.
 *
 * The features in range (plan 09, P09.T41.a) are each drawn as a Greek cross at its centre, at the
 * largest size class, labelled with its kind's abbreviation and its designation (`OC GPF F207`),
 * and with its extent as its true radius, which the view draws as a circle where it is wider than
 * the mark; the `STARS` filter, which picks systems, leaves them all drawn. A nursery's
 * superbubble is a shell of its own, a circle at its true radius about the feature labelled
 * `SUPERBUBBLE` and the feature's designation, with no mark. A feature's mark takes an ID of its
 * own ({@link featureMarkId}), so that it is picked and selected apart from any system.
 */
export function toScene(result: ChartResult, options: ChartSceneOptions): SpatialScene {
  const {
    frame,
    driveRangeLy,
    selectedId,
    destinationId,
    starFilter = "all",
    features = [],
  } = options;
  const spacing = gridSpacing(result.radiusLy);
  const spheres: SphereMark[] = [];
  if (rangeIsShown(result.radiusLy, driveRangeLy)) {
    spheres.push({ radius: driveRangeLy, role: "range", label: rangeLabel(driveRangeLy) });
  }
  spheres.push({
    radius: result.radiusLy,
    role: "data_edge",
    label: `QUERY EDGE ${formatQueryRadius(result.radiusLy)}`,
  });
  const drawn = filterSystems(result.systems, starFilter).flatMap((system) => {
    // An object of a layer this build does not know has no size class: listed, and not drawn.
    const mark =
      system.layer === "unknown" ? null : toMark(system, driveRangeLy, layerIndex(system.layer));
    return mark === null ? [] : [{ system, mark }];
  });
  const mode = answeredMode(result);
  const segments =
    mode.kind === "now"
      ? []
      : drawn
          .toSorted(
            (a, b) => Number(a.system.id === selectedId) - Number(b.system.id === selectedId),
          )
          .flatMap(({ system }) => {
            const segment = apparentSegment(system, system.id === selectedId);
            return segment === null ? [] : [segment];
          });
  const [centreX, centreY, centreZ] = result.centreLy;
  const bubbles = features.flatMap((feature) => {
    const bubble = toBubble(feature);
    return bubble === null ? [] : [bubble];
  });
  return {
    frame,
    points: drawn.map(({ mark }) => mark),
    ...(features.length === 0 ? {} : { features: features.map(toFeatureMark) }),
    ...(bubbles.length === 0 ? {} : { shells: bubbles }),
    ...(segments.length === 0 ? {} : { segments }),
    ...(mode.kind === "observed"
      ? {
          observer: {
            position: vec3(
              mode.observerLy[0] - centreX,
              mode.observerLy[1] - centreY,
              mode.observerLy[2] - centreZ,
            ),
            label: OBSERVER_LABEL,
          },
        }
      : {}),
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
  | {
      readonly kind: "complete";
      readonly text: string;
      readonly aboveMsun: number;
      /**
       * The edge as it is written, in the unit of the lightest layer included; `null`, which reads
       * Missing, where that layer is one this build does not know.
       */
      readonly above: ReturnType<typeof formatSubstellarMass> | null;
    }
  | { readonly kind: "nothing_fits"; readonly text: string };

/**
 * The words for a chart's census.
 *
 * @remarks
 * `COMPLETE ABOVE` is followed by the mass and its drawn unit, so the caller sets them: in M☉ down
 * to the brown dwarfs (`0.012`), in M⊕ for the free-floating planets (`0.33`; plan 13, design note
 * 14), as {@link formatSubstellarMass} gives them. Nothing fitting the census limit is an answer,
 * not an error, and says what the operator can do.
 */
export function censusLine(census: ChartCensus): CensusLine {
  let line: CensusLine;
  switch (census.kind) {
    case "complete":
      line = {
        kind: "complete",
        text: "COMPLETE ABOVE",
        aboveMsun: census.aboveMsun,
        above:
          census.layer === "unknown" ? null : formatSubstellarMass(census.aboveMsun, census.layer),
      };
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
export function censusHint(layers: ReadonlyArray<ChartLayerLine>): string | null {
  let dense = false;
  let large = false;
  for (const layer of layers) {
    const status: ChartLayerStatus = layer.status;
    switch (status) {
      case "over_limit":
        dense = true;
        break;
      case "over_cell_budget":
        large = true;
        break;
      // A status this build does not know says nothing it could act on.
      case "included":
      case "below_mass_floor":
      case "unknown":
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

/** How many stellar layers a census lists: plan 04's five, `a` to `e`. */
const STELLAR_LAYER_COUNT = 5;

/** Whether a layer is one of the two substellar ones plan 13 adds. */
function isSubstellar(layer: ChartLayer): boolean {
  return layer === "brown_dwarf" || layer === "rogue_planet";
}

/**
 * Whether a census lists the substellar layers as a query can ask for them: none, the brown dwarfs,
 * or the brown dwarfs and the free-floating planets, each once (plan 13, P13.T7).
 */
function substellarLinesUsable(layers: ReadonlyArray<ChartLayerLine>): boolean {
  const brown = layers.filter((line) => line.layer === "brown_dwarf").length;
  const rogue = layers.filter((line) => line.layer === "rogue_planet").length;
  return brown <= 1 && rogue <= 1 && (rogue === 0 || brown === 1);
}

/**
 * The five stellar mass bands of a chart's census, lightest first, or `null` when the census does
 * not list all five stellar layers once each, or lists the substellar ones in a way no query asks
 * for.
 *
 * @remarks
 * The edges come from the census the server sent with the answer, so the client holds no copy of
 * the band table. A census that does not list every layer is an unusable answer, not a bug in the
 * client, so it is reported rather than thrown: see {@link chartDataFault}. The substellar lines,
 * which a query lowered to them adds after E, are not size bands: their objects are drawn at layer
 * A's size (plan 13, design note 15), and their floors are {@link SUBSTELLAR_FLOORS}. Nor is a
 * line of a layer this build does not know, which a newer server may send beside the five.
 */
export function layerBands(layers: ReadonlyArray<ChartLayerLine>): ReadonlyArray<LayerBand> | null {
  if (!substellarLinesUsable(layers)) {
    return null;
  }
  const bands = layers.flatMap((line): ReadonlyArray<LayerBand> =>
    line.layer === "unknown" || isSubstellar(line.layer)
      ? []
      : [
          {
            layer: line.layer,
            index: layerIndex(line.layer),
            minMsun: line.mass_min_msun,
            maxMsun: line.mass_max_msun,
          },
        ],
  );
  const seen = new Set(bands.map((band) => band.index));
  if (bands.length !== STELLAR_LAYER_COUNT || seen.size !== STELLAR_LAYER_COUNT) {
    return null;
  }
  return bands.toSorted((a, b) => a.index - b.index);
}

/** A mass floor below layer A: a substellar layer and the lower edge of its band. */
export interface SubstellarFloor {
  readonly layer: "brown_dwarf" | "rogue_planet";
  /** The lower edge of the layer's band of object mass, in M☉. */
  readonly minMsun: number;
}

/**
 * The two mass floors below layer A, lightest first: the free-floating planets from a third of an
 * Earth mass, and the brown dwarfs from 13 Jupiter masses (plan 13, P13.T8.c).
 *
 * @remarks
 * Restated from plan 13's band limits in `galaxy::substellar` (`ROGUE_PLANET_MIN`,
 * `BROWN_DWARF_MIN`), which own them, in M☉ by the nominal GM of the Earth, Jupiter and the Sun, as
 * the sim converts them. A census carries a substellar layer's band only when the query asked for
 * it, so the selector could not otherwise offer the floor that asks. They read `0.33 M⊕` and
 * `0.012 M☉` ({@link formatSubstellarMass}).
 */
export const SUBSTELLAR_FLOORS: ReadonlyArray<SubstellarFloor> = [
  { layer: "rogue_planet", minMsun: 1 / 3 / MEARTH_PER_MSUN },
  { layer: "brown_dwarf", minMsun: 13 / MJUP_PER_MSUN },
];

/**
 * The census lines of a chart, lightest layer first, as the census table lists them.
 *
 * @remarks
 * A layer this build does not know has no rank, so its line stands by its band: before the first
 * line whose band starts above its own.
 */
export function censusLinesLightestFirst(
  layers: ReadonlyArray<ChartLayerLine>,
): ReadonlyArray<ChartLayerLine> {
  const lines = layers
    .filter((line) => line.layer !== "unknown")
    .toSorted((a, b) => layerRank(a.layer) - layerRank(b.layer));
  for (const line of layers) {
    if (line.layer === "unknown") {
      const above = lines.findIndex((each) => each.mass_min_msun > line.mass_min_msun);
      lines.splice(above === -1 ? lines.length : above, 0, line);
    }
  }
  return lines;
}

/**
 * Why a range query's answer cannot be charted, in words, or `null` when it can.
 *
 * @remarks
 * The wire is decoded but not validated, so an answer the client cannot use must read as a fault
 * and not take the display down: the counterpart of the galaxy map's `MAP DATA INVALID` (plan 05,
 * T8.d). The brainstorm's range query answers with a complete census per layer, so a census that
 * does not name every layer leaves it unknown what the answer is complete above, and the whole
 * answer goes unshown. So does an answer with an observed row the client cannot read (plan 12,
 * P12.T11): a light age or a stated bound that is not a finite number of zero or more, or an
 * emitted time or an apparent position that is not finite, since the readout would print it and
 * the chart draw it.
 */
export function chartDataFault(result: ChartResult): string | null {
  if (!(result.radiusLy > 0) || !Number.isFinite(result.radiusLy)) {
    return "query radius unusable";
  }
  if (layerBands(result.layers) === null) {
    return "census incomplete";
  }
  // A census complete above an edge names a layer it included, whose unit the edge is read in.
  const included = result.layers.some((line) => line.status === "included");
  if (result.census.kind === "complete" && !included) {
    return "census incomplete";
  }
  const unusable = result.systems.some(
    (system) => system.observed !== null && !observationUsable(system.observed),
  );
  return unusable ? "observation unusable" : null;
}

/** Whether a quantity that cannot be negative, such as a light age or a bound, can be read. */
function isUsableMagnitude(value: number): boolean {
  return Number.isFinite(value) && value >= 0;
}

/** Whether an observed row's observation can be read and drawn. */
function observationUsable(observed: Observation): boolean {
  const { x, y, z } = observed.apparentRelLy;
  return (
    isUsableMagnitude(observed.lightAgeYr) &&
    isUsableMagnitude(observed.curvatureBoundLy) &&
    isUsableMagnitude(observed.curvatureBoundArcsec) &&
    Number.isFinite(observed.emittedYr) &&
    Number.isFinite(x) &&
    Number.isFinite(y) &&
    Number.isFinite(z)
  );
}

/** Whether a length or a mass that cannot be negative can be read and drawn. */
function isUsableSize(value: number): boolean {
  return Number.isFinite(value) && value >= 0;
}

/** Whether a feature's values can be read and drawn: finite, and no size below zero. */
function featureUsable(feature: ChartFeature): boolean {
  const { x, y, z } = feature.relLy;
  return (
    Number.isFinite(x) &&
    Number.isFinite(y) &&
    Number.isFinite(z) &&
    isUsableSize(feature.extentLy) &&
    isUsableSize(feature.massMsun) &&
    isUsableSize(feature.expectedMembers) &&
    (feature.ageMyr === null || Number.isFinite(feature.ageMyr)) &&
    (feature.bubbleRadiusLy === null || isUsableSize(feature.bubbleRadiusLy))
  );
}

/**
 * Why a features-in-range answer cannot be charted, in words, or `null` when it can (plan 09,
 * P09.T41.a).
 *
 * @remarks
 * The counterpart of {@link chartDataFault}: the wire is decoded but not validated, so a feature
 * whose position, extent, mass, expected members, age or bubble the chart could not draw or the
 * readout could not print leaves the whole answer unshown, rather than a chart that misdraws one.
 */
export function featureDataFault(features: ChartFeatures): string | null {
  if (!(features.radiusLy > 0) || !Number.isFinite(features.radiusLy)) {
    return "query radius unusable";
  }
  return features.features.every(featureUsable) ? null : "feature unusable";
}
