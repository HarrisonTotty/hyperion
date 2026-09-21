/**
 * The client's model of galaxy data, with units in its names.
 *
 * @remarks
 * Wire shapes stop at `wire.ts`, which builds these types from plan 04's responses (plan 05,
 * design note D8), so that the rest of the client never reads a wire field name. The census's
 * layers are the one wire type passed through, since they are already a table.
 */
import type { LayerCensus, MassLayer, Population, SystemIdHex } from "@hyperion/protocol";

import type { Vec3 } from "../../spatial/vec3";

/**
 * Half the edge of the root cube, the galaxy's whole extent: coordinates run from −65,536 ly to
 * 65,536 ly on every axis.
 *
 * @remarks
 * Restated from plan 01's coordinates (`coords`), which owns the value; the server refuses a
 * chart centre outside it (plan 04, design note 24).
 */
export const ROOT_CUBE_HALF_LY = 65_536;

/**
 * The clock window H: a chart time lies within this many years of the epoch.
 *
 * @remarks
 * Restated from plan 01's `time::CLOCK_WINDOW_H`, which owns the value; the server refuses a query
 * time outside it (plan 04, design note 24).
 */
export const CLOCK_WINDOW_YR = 1_000;

/**
 * The census limit every chart query is sent with: the most systems a chart may expect to show.
 *
 * @remarks
 * The top of the brainstorm's "a few thousand" (plan 05, design note D20), well inside the
 * server's ceiling of 20,000.
 */
export const CHART_SYSTEM_LIMIT = 4_000;

/** A point in the `GALACTIC` frame as x, y and z in light-years. */
export type CentreLy = readonly [xLy: number, yLy: number, zLy: number];

/** A mass layer's place from the lightest, A, to the heaviest, E. */
export type LayerIndex = 0 | 1 | 2 | 3 | 4;

/** One star system on a chart, as it is at the chart's time. */
export interface ChartSystem {
  /** The ID as on the wire, lower case; only `formatHex64` upper-cases it for the screen. */
  readonly id: SystemIdHex;
  readonly designation: string;
  /** Offset from the chart centre along the `GALACTIC` axes. */
  readonly relLy: Vec3;
  /** Straight-line distance from the chart centre. */
  readonly distanceLy: number;
  /** Position in the `GALACTIC` frame, for the readout's cylindrical coordinates. */
  readonly positionLy: Vec3;
  readonly layer: MassLayer;
  readonly population: Population;
  /** Initial mass of the primary star. */
  readonly initialMassMsun: number;
  readonly ageMyr: number;
}

/**
 * What a chart is complete for.
 *
 * @remarks
 * `complete` above the lower mass edge of the lightest layer returned; `nothing_fits` when no
 * layer fits the census limit, which is an answer with no systems, not an error.
 */
export type ChartCensus =
  { readonly kind: "complete"; readonly aboveMsun: number } | { readonly kind: "nothing_fits" };

/** A range query's answer, ready to chart. */
export interface ChartResult {
  /** The centre the server answered for, in the `GALACTIC` frame. */
  readonly centreLy: CentreLy;
  /** The radius the server answered for. */
  readonly radiusLy: number;
  /** The chart time, in years from the epoch. */
  readonly timeYr: number;
  readonly census: ChartCensus;
  /** The census of all five layers, A to E, for the legend and the census table. */
  readonly layers: ReadonlyArray<LayerCensus>;
  /** Every system returned, nearest first, ties by ID. */
  readonly systems: ReadonlyArray<ChartSystem>;
}
