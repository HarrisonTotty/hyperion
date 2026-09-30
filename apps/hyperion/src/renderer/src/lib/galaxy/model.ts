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

/**
 * A mass layer's place from the lightest stellar layer, A, to the heaviest, E: also its symbols'
 * size class. The two substellar layers take layer A's, 0, since no size class below A is drawn
 * (plan 13, design note 15).
 */
export type LayerIndex = 0 | 1 | 2 | 3 | 4;

/**
 * What a chart's object is, as its layer says (plan 13, design note 10): a star system, a
 * free-floating brown dwarf, or a free-floating planet, which has no stellar state (P13.T5.d).
 */
export type ChartKind = "stellar" | "brown_dwarf" | "rogue_planet";

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
  /**
   * How many stars the system has, the primary included: 1 to 4, since a primary has at most three
   * companions (plan 11, rulings 74 and 81), so one digit holds it.
   */
  readonly starCount: number;
}

/**
 * One system on a chart, as it is at the chart's time: a star system, or a free-floating brown
 * dwarf or planet (plan 13), which is a system with no star.
 */
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
  /** What it is, from its layer. */
  readonly kind: ChartKind;
  readonly population: Population;
  /** Initial mass of the primary star, or a free-floating object's mass, in M☉. */
  readonly initialMassMsun: number;
  /** Its age at the chart's time; zero or less for one not yet formed. */
  readonly ageMyr: number;
  /**
   * Its primary at the chart's time, or `null` for a system not yet formed then, whose row the
   * server sends without a brief, and for a free-floating planet, which has none.
   */
  readonly star: StarBrief | null;
  /**
   * The metallicity [Fe/H], in dex, of a free-floating planet, which only its row carries, since
   * `system_summary` describes stars and it has none (plan 13, P13.T5.d); `null` for every other
   * system, whose metallicity is its summary's, and for a row the server sent without one.
   */
  readonly feHDex: number | null;
  /** Velocity at the epoch along the `GALACTIC` axes, in km/s (plan 08, P08.T7.a). */
  readonly velocityKmS: Vec3;
}

/**
 * Whether a chart's object is a free-floating planet that exists at the chart's time: one that
 * has formed, which alone is drawn, listed as `PLANET` and counted as one.
 */
export function isFormedPlanet(system: ChartSystem): boolean {
  return system.kind === "rogue_planet" && system.ageMyr > 0;
}

/**
 * What a chart is complete for.
 *
 * @remarks
 * `complete` above the lower mass edge of the lightest layer returned, which is also named, since a
 * free-floating planet's edge is written in Earth masses (plan 13, design note 14); `nothing_fits`
 * when no layer fits the census limit, which is an answer with no systems, not an error.
 */
export type ChartCensus =
  | { readonly kind: "complete"; readonly aboveMsun: number; readonly layer: MassLayer }
  | { readonly kind: "nothing_fits" };

/** A range query's answer, ready to chart. */
export interface ChartResult {
  /** The centre the server answered for, in the `GALACTIC` frame. */
  readonly centreLy: CentreLy;
  /** The radius the server answered for. */
  readonly radiusLy: number;
  /** The chart time, in years from the epoch. */
  readonly timeYr: number;
  readonly census: ChartCensus;
  /**
   * The census of the five stellar layers, A to E, then the substellar layers the query asked for,
   * for the legend and the census table.
   */
  readonly layers: ReadonlyArray<LayerCensus>;
  /** Every system returned, nearest first, ties by ID. */
  readonly systems: ReadonlyArray<ChartSystem>;
}
