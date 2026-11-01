/**
 * The client's model of galaxy data, with units in its names.
 *
 * @remarks
 * Wire shapes stop at `wire.ts`, which builds these types from plan 04's responses (plan 05,
 * design note D8), and at `featureWire.ts`, which builds the features' from plan 09's, so that the
 * rest of the client never reads a wire field name. The census's layers and the features' census
 * lines are the wire types passed through, since they are already tables, the layers with only
 * their layer and status read into the model's own; so are the closed enums that name a feature's
 * kind, a density map's population or an emission class.
 *
 * A mass layer, a population and a layer's status are of enums plan 04 names as growing, which a
 * newer server may send a new string of without a protocol bump. Each therefore has an `unknown`
 * arm here, which `wire.ts` gives a string this build does not know and every reader shows as
 * Missing, so that the switches over them stay exhaustive.
 */
import type {
  CatalogueClassWire,
  CentreTracerWire,
  ClassKindWire,
  DisruptionOutcomeWire,
  EmissionClassWire,
  FeatureIdHex,
  FeatureKindCensus,
  FeatureKindWire,
  GenerationWire,
  LayerCensus,
  LayerStatus,
  MapFeatureKindWire,
  MassLayer,
  MultiplicityWire,
  ObjectKindDto,
  Population,
  ShellPhaseWire,
  SystemIdHex,
} from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";

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
 * The mode a query is asked in (plan 12, Design note 4): `now`, the present, or `observed`, what
 * an observer at `observerLy` receives at the query's time.
 *
 * @remarks
 * Observed mode changes what is reported, never what is found: the systems, the census and the
 * positions are the present's in both, and each system found also carries its
 * {@link Observation}.
 */
export type QueryMode =
  | { readonly kind: "now" }
  | {
      readonly kind: "observed";
      /** Where the observer is, in the `GALACTIC` frame. */
      readonly observerLy: CentreLy;
    };

/** The present: the mode every query is asked in unless the operator chooses another. */
export const NOW_MODE: QueryMode = { kind: "now" };

/** Whether two modes are the same: both `now`, or both observed from the same point. */
export function sameQueryMode(a: QueryMode, b: QueryMode): boolean {
  if (a.kind === "now" || b.kind === "now") {
    return a.kind === b.kind;
  }
  const [ax, ay, az] = a.observerLy;
  const [bx, by, bz] = b.observerLy;
  return ax === bx && ay === by && az === bz;
}

/**
 * What an observer receives from one system: when its light left it, how old that light is, and
 * where it shows the system (plan 12, P12.T6).
 *
 * @remarks
 * Carried by a row only in observed mode. The row's own position, age and velocity stay the
 * present's (Design note 4), so a reading of the system as observed takes its age from
 * {@link observedAgeMyr} and shows no velocity, which is present state the crew cannot know (the
 * port ruling, §6.2).
 */
export interface Observation {
  /** When the light the observer receives at the chart's time left the system, years from the epoch. */
  readonly emittedYr: number;
  /** How long that light has travelled, in Julian years: the chart time less {@link emittedYr}. */
  readonly lightAgeYr: number;
  /**
   * Where the observer sees the system, its position when the light left: the offset from the
   * chart centre along the `GALACTIC` axes, as {@link ChartSystem.relLy} is.
   */
  readonly apparentRelLy: Vec3;
  /**
   * The stated bound on the apparent position's error from the neglected curvature of the system's
   * galactic orbit, in light-years at the system: a bound, not an uncertainty; zero for an orbit
   * followed in full.
   */
  readonly curvatureBoundLy: number;
  /** The same bound as the angle it subtends at the observer, in arcseconds. */
  readonly curvatureBoundArcsec: number;
}

/**
 * A mass layer's place from the lightest stellar layer, A, to the heaviest, E: also its symbols'
 * size class. The two substellar layers take layer A's, 0, since no size class below A is drawn
 * (plan 13, design note 15).
 */
export type LayerIndex = 0 | 1 | 2 | 3 | 4;

/** A mass layer as the chart reads it: plan 04's, or `unknown` for one this build does not know. */
export type ChartLayer = MassLayer | "unknown";

/** A stellar population as the chart reads it: plan 04's, or `unknown` for one this build does not know. */
export type ChartPopulation = Population | "unknown";

/** A layer's status as the chart reads it: plan 04's, or `unknown` for one this build does not know. */
export type ChartLayerStatus = LayerStatus | "unknown";

/** One layer's line of a chart's census: the wire's, with its layer and status read. */
export interface ChartLayerLine extends Readonly<Omit<LayerCensus, "layer" | "status">> {
  readonly layer: ChartLayer;
  readonly status: ChartLayerStatus;
}

/**
 * What a chart's object is, as its layer says (plan 13, design note 10): a star system, a
 * free-floating brown dwarf, or a free-floating planet, which has no stellar state (P13.T5.d); or
 * `unknown`, for an object of a layer this build does not know, which is listed and not drawn.
 */
export type ChartKind = "stellar" | "brown_dwarf" | "rogue_planet" | "unknown";

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
   * How many stars the system has, the primary included: 1 to 5, since a primary has at most three
   * stellar companions (plan 11, rulings 74 and 81) and one bound brown dwarf, which the count
   * includes (P11.T2.d), so one digit holds it.
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
  readonly layer: ChartLayer;
  /** What it is, from its layer. */
  readonly kind: ChartKind;
  readonly population: ChartPopulation;
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
  /**
   * What the chart's observer receives from it, for a row answered in observed mode (plan 12,
   * P12.T6); `null` for a row without it, which reads as in `now` whatever was asked, since a
   * server built before P12.T6 ignores the mode.
   */
  readonly observed: Observation | null;
  /** What placed it (plan 09, P09.T39). */
  readonly origin: SystemOrigin;
  /** Its class in its feature's tables, for a member of a feature or of the centre; else `null`. */
  readonly memberClass: MemberClass | null;
  /** What it is as a supernova catalogue entry at the chart's time; `null` for any other system. */
  readonly supernova: SupernovaState | null;
}

/**
 * A catalogue class (plan 09's `ClassId` registry) as the wire names it, or `unknown` for one this
 * build does not know: the enum grows (plan 04's reserved list), so a newer server may send a class
 * registered later, which reads Missing.
 */
export type CatalogueClass = CatalogueClassWire | "unknown";

/**
 * What placed a system (plan 09, P09.T39): a layer's grid, a feature, the galactic centre or a
 * catalogue class; `not_sent` for a row from a server that does not say, as one before P09.T40
 * does not and P09.T40's does not before its members' gate opens (P09.T2.c, version 26); and
 * `unknown` for an origin this build does not know, which the enum's growing allows (plan 04's
 * reserved list), and for plan 10's members of streams and dwarf cores until P10.T11 names them;
 * either reads Missing.
 */
export type SystemOrigin =
  | { readonly kind: "not_sent" }
  | { readonly kind: "grid" }
  | { readonly kind: "feature_member"; readonly feature: FeatureIdHex }
  | { readonly kind: "centre_member" }
  | { readonly kind: "catalogue_system"; readonly catalogueClass: CatalogueClass }
  | { readonly kind: "unknown" };

/**
 * A member's class in its feature's tables (plan 09, P09.T39): what its systems are and where in a
 * cluster's or the centre's tables they were drawn from, or the catalogue class of an entry of a
 * feature-level list; `unknown` for a class this build does not know, which reads Missing.
 */
export type MemberClass =
  | {
      readonly kind: "cluster";
      readonly classKind: ClassKindWire;
      /**
       * The layer whose band its primaries' initial masses fall in; `unknown` for one this build
       * does not know, which reads Missing.
       */
      readonly band: ChartLayer;
      readonly generation: GenerationWire;
      readonly multiplicity: MultiplicityWire;
    }
  | {
      readonly kind: "centre";
      readonly classKind: ClassKindWire;
      readonly band: ChartLayer;
      /** The index of the star-formation episode its members were born in, from 0, the oldest. */
      readonly episode: number;
      readonly tracer: CentreTracerWire;
    }
  | { readonly kind: "listed"; readonly catalogueClass: CatalogueClass }
  | { readonly kind: "unknown" };

/** A supernova remnant's shell at one age (plan 09's `ShellState`). */
export interface ShellState {
  /** The radius of its outer shock from the explosion site. */
  readonly radiusLy: number;
  readonly shockSpeedKmS: number;
  readonly phase: ShellPhaseWire;
  /** Bright in X-rays and radio: free expansion and Sedov–Taylor. */
  readonly xRayAndRadio: boolean;
  /** Optical filaments, its shock faster than 70 km/s. */
  readonly optical: boolean;
  /** A shell of neutral hydrogen seen at 21 cm: the two snowplough phases. */
  readonly neutralHydrogen: boolean;
}

/** What a supernova left, at one time after it (plan 09's `RemnantState`). */
export interface RemnantState {
  /** The compact remnant's distance from the explosion site; `null` where it left none. */
  readonly offsetLy: number | null;
  /** Whether the compact remnant drives a bow shock. */
  readonly bowShock: boolean;
  /** The spin-down luminosity powering its pulsar's wind nebula, in watts; `null` for none. */
  readonly nebulaPowerW: number | null;
  /** A Type Ia's surviving companion's distance from the site; `null` for none. */
  readonly survivorOffsetLy: number | null;
}

/**
 * What a supernova catalogue entry is at the chart's time (plan 09's `SupernovaState`): the living
 * progenitor, the supernova with its shell, or once the shell has merged with the gas, the bare
 * remnant.
 */
export type SupernovaState =
  | {
      readonly kind: "progenitor";
      readonly untilExplosionYr: number;
      /** A Type Ia's double white dwarf's orbital period, in seconds; `null` for any other. */
      readonly pairPeriodS: number | null;
    }
  | {
      readonly kind: "supernova";
      readonly ageYr: number;
      readonly shell: ShellState;
      readonly remnant: RemnantState;
    }
  | { readonly kind: "bare_remnant"; readonly ageYr: number; readonly remnant: RemnantState };

/** Myr in one Julian year, the light age's unit on the wire. */
const MYR_PER_YR = 1e-6;

/**
 * A system's age when the light the observer receives left it, `ageMyr − lightAgeYr ÷ 10⁶`,
 * negative for one not yet formed then; `null` for a row without an observation (the port ruling,
 * §6.2).
 */
export function observedAgeMyr(system: ChartSystem): number | null {
  return system.observed === null ? null : system.ageMyr - system.observed.lightAgeYr * MYR_PER_YR;
}

/**
 * A system's age as the chart shows it: when its light left it for a row answered in observed
 * mode ({@link observedAgeMyr}), and at the chart's time for every other.
 */
export function shownAgeMyr(system: ChartSystem): number {
  return observedAgeMyr(system) ?? system.ageMyr;
}

/**
 * Whether a chart's object is a free-floating planet that exists at the time the chart shows it,
 * the chart's time or, as observed, when its light left: one that has formed, which alone is
 * drawn, listed as `PLANET` and counted as one.
 */
export function isFormedPlanet(system: ChartSystem): boolean {
  return system.kind === "rogue_planet" && shownAgeMyr(system) > 0;
}

/**
 * What a chart is complete for.
 *
 * @remarks
 * `complete` above the lower mass edge of the lightest layer returned, which is also named, since a
 * free-floating planet's edge is written in Earth masses (plan 13, design note 14), or `unknown`
 * when a layer this build does not know is the lightest included, so that no unit is known for the
 * edge; `nothing_fits` when no layer fits the census limit, which is an answer with no systems, not
 * an error.
 */
export type ChartCensus =
  | { readonly kind: "complete"; readonly aboveMsun: number; readonly layer: ChartLayer }
  | { readonly kind: "nothing_fits" };

/** A range query's answer, ready to chart. */
export interface ChartResult {
  /** The centre the server answered for, in the `GALACTIC` frame. */
  readonly centreLy: CentreLy;
  /** The radius the server answered for. */
  readonly radiusLy: number;
  /** The chart time, in years from the epoch. */
  readonly timeYr: number;
  /**
   * The mode the query was asked in, and from where, taken from the request it answers, so that an
   * answer on show is never relabelled by a later choice. Each row says for itself whether it was
   * answered so ({@link ChartSystem.observed}).
   */
  readonly mode: QueryMode;
  readonly census: ChartCensus;
  /**
   * The census of the five stellar layers, A to E, then the substellar layers the query asked for,
   * for the legend and the census table.
   */
  readonly layers: ReadonlyArray<ChartLayerLine>;
  /** Every system returned, nearest first, ties by ID. */
  readonly systems: ReadonlyArray<ChartSystem>;
}

/**
 * One large feature on a chart, as it is at the chart's time (plan 09): a cluster, an association,
 * a star-forming region, a molecular cloud or the galactic centre.
 */
export interface ChartFeature {
  /** The ID as on the wire, lower case: the word its members share with their own bits zeroed. */
  readonly id: FeatureIdHex;
  /** Its designation, derived from the ID (`GPF F207`, `CENTRE`): never a proper name. */
  readonly designation: string;
  readonly kind: FeatureKindWire;
  /** Its centre's offset from the chart centre along the `GALACTIC` axes. */
  readonly relLy: Vec3;
  /** Straight-line distance from the chart centre to its centre. */
  readonly distanceLy: number;
  /** Its centre in the `GALACTIC` frame. */
  readonly positionLy: Vec3;
  /** Its radius at the chart's time: the sphere the chart draws it as. */
  readonly extentLy: number;
  /** Its age at the chart's time; `null` for a molecular cloud, which has none, and the centre. */
  readonly ageMyr: number | null;
  /** Its present mass, in M☉: its stars', or a molecular cloud's gas's. */
  readonly massMsun: number;
  /**
   * The expected number of its member systems at the chart's time, the model's count over its
   * classes: an expectation, not the number of rows a range query returns; zero for a cloud.
   */
  readonly expectedMembers: number;
  /** What it shines as besides its stars; `null` for one that shines as nothing but them. */
  readonly emission: EmissionClassWire | null;
  /** The radius of a nursery's superbubble at the chart's time; `null` for one with none. */
  readonly bubbleRadiusLy: number | null;
}

/** A features-in-range answer, ready to chart (plan 09, P09.T41.a). */
export interface ChartFeatures {
  /** The centre the server answered for, in the `GALACTIC` frame. */
  readonly centreLy: CentreLy;
  /** The radius the server answered for. */
  readonly radiusLy: number;
  /** The time the features were placed at, in years from the epoch. */
  readonly timeYr: number;
  /** One line for each kind asked for, in the order asked: whether it is included, and why not. */
  readonly census: ReadonlyArray<FeatureKindCensus>;
  /** Every feature of the included kinds within range, nearest first, ties by ID. */
  readonly features: ReadonlyArray<ChartFeature>;
}

/** A feature the galaxy map draws (plan 09, P09.T41.b): a globular or the centre, at the map's time. */
export interface MapFeature {
  /** The ID as on the wire, lower case. */
  readonly id: FeatureIdHex;
  readonly designation: string;
  readonly kind: FeatureKindWire;
  /** Its centre in the `GALACTIC` frame. */
  readonly positionLy: Vec3;
  /** Its radius at the map's time. */
  readonly extentLy: number;
  /** Its present mass, in M☉, which orders the labels. */
  readonly massMsun: number;
}

/** A supernova remnant's bright shell the galaxy map draws (plan 09, P09.T41.b): about its site. */
export interface MapShell {
  /** The catalogue system that exploded. */
  readonly system: SystemIdHex;
  readonly designation: string;
  /** The explosion site, the shell's centre, in the `GALACTIC` frame. */
  readonly siteLy: Vec3;
  /** The radius of its outer shock at the map's time. */
  readonly radiusLy: number;
}

/**
 * One kind's layer of the galaxy map's feature overlay (plan 09, P09.T41.b): all of the kind, or
 * none, with the server's reason that it does not generate them.
 */
export type MapFeatureLayer =
  | {
      readonly kind: "globular" | "galactic_centre";
      readonly status: "complete";
      readonly features: ReadonlyArray<MapFeature>;
    }
  | {
      readonly kind: "bright_shell";
      readonly status: "complete";
      readonly shells: ReadonlyArray<MapShell>;
    }
  | { readonly kind: MapFeatureKindWire; readonly status: "not_generated" };

/** One member class of a feature with its expected count (plan 09's `ClassCount`). */
export interface ClassCount {
  readonly memberClass: MemberClass;
  readonly initialMassMinMsun: number;
  readonly initialMassMaxMsun: number;
  /** The mean present mass of its systems. */
  readonly meanMassMsun: number;
  /** Its expected count: an expectation, not a count of rows. */
  readonly expected: number;
}

/** A feature's structure at a time (plan 09, P09.T40.b's `feature_detail`), by what it is. */
export type FeatureStructure =
  | {
      readonly kind: "cluster";
      readonly coreRadiusLy: number;
      readonly halfMassRadiusLy: number;
      readonly tidalRadiusLy: number;
      readonly centralEscapeSpeedKmS: number;
      readonly relaxationTimeMyr: number;
      readonly coreCollapsed: boolean;
      /** The expected number of stellar black holes it still holds. */
      readonly blackHoles: number;
      readonly classes: ReadonlyArray<ClassCount>;
    }
  | {
      readonly kind: "centre";
      readonly influenceRadiusLy: number;
      readonly lossConeRadiusLy: number;
      readonly classes: ReadonlyArray<ClassCount>;
    }
  | { readonly kind: "cloud"; readonly coreRadiusLy: number; readonly centralDensityPerCm3: number }
  | { readonly kind: "association"; readonly radiusLy: number; readonly expansionSpeedKmS: number };

/** One feature's detail at a time: what the readout adds to its summary. */
export interface FeatureDetailModel {
  readonly id: FeatureIdHex;
  /** The time it describes, in years from the epoch. */
  readonly timeYr: number;
  readonly structure: FeatureStructure;
}

/** One tidal disruption by the central black hole (plan 09's `TidalDisruptionVictim`). */
export interface TidalDisruption {
  readonly victim: SystemIdHex;
  /** Its pericentre passage, in years from the epoch. */
  readonly timeYr: number;
  readonly starMassMsun: number;
  readonly outcome: DisruptionOutcomeWire;
  /** Its debris tail's peak 2–10 keV luminosity, in watts; `null` for a star swallowed whole. */
  readonly peakLuminosityW: number | null;
}

/** The central black hole at a time (plan 09, P09.T40.b's `black_hole_state`). */
export interface BlackHoleModel {
  /** The time it describes, in years from the epoch. */
  readonly timeYr: number;
  readonly massMsun: number;
  /** Its unabsorbed 2–10 keV luminosity, in watts; `null` from a server that does not say. */
  readonly luminosityW: number | null;
  /** Its disruptions at or before the time, newest first. */
  readonly disruptions: ReadonlyArray<TidalDisruption>;
}
