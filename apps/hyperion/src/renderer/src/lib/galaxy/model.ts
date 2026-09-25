/**
 * The client's model of galaxy data, with units in its names.
 *
 * @remarks
 * Wire shapes stop at `wire.ts`, which builds these types from plan 04's responses (plan 05,
 * design note D8), so that the rest of the client never reads a wire field name. The census's
 * layers are the one wire type passed through, since they are already a table.
 */
import type {
  LayerCensus,
  MassLayer,
  ObjectKindDto,
  Population,
  SystemIdHex,
} from "@hyperion/protocol";

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

/**
 * What a chart knows of a system's primary at the chart's time, from the range query's brief (plan
 * 06, P06.T33): enough to draw its symbol and place it on the Hertzsprung-Russell diagram.
 */
export interface StarBrief {
  /** What the primary is now, which decides its symbol. */
  readonly kind: ObjectKindDto;
  /** Its class as an astronomer writes it: `G2V`, `M5III`, `DA4.2`, `NS`, `BH`, `NONE`. */
  readonly spectralClass: string;
  /**
   * log₁₀ of its bolometric luminosity over the Sun's; `null` for a black hole or a star that left
   * no remnant, which have none.
   */
  readonly logLuminosityLsun: number | null;
  /** Its effective temperature; `null` where the luminosity is. */
  readonly teffK: number | null;
  /** How many stars the system has, the primary included: 1 to 4. */
  readonly starCount: number;
}

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
  /**
   * Its primary at the chart's time, or `null` for a system not yet formed then, whose row the
   * server sends without a brief.
   */
  readonly star: StarBrief | null;
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
