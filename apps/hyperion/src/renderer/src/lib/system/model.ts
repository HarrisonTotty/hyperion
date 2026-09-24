/**
 * The client's model of one planetary system at one time, with units in its names (plan 14, phase
 * I).
 *
 * @remarks
 * Wire shapes stop at `wire.ts` and `bodiesWire.ts`, which build these types from a `system_summary`
 * or a `system_bodies` answer, as plan 05's design note D8 has `lib/galaxy/wire.ts` do for the
 * chart, so that the `SYSTEM` display never reads a wire field name. The kinds and phases pass
 * through as the wire's string unions, since they are already names. The hosts are the system's
 * stars; the planets, their moons and rings, the belts, the discs and the halo are its bodies, from
 * `system_bodies` (P14.T35.b–c), each section tagged with its state as the server tagged it.
 */
import type {
  ArchitectureClassDto,
  BeltKindDto,
  BodyIdHex,
  DestructionCauseDto,
  DetailLevelDto,
  KickModeDto,
  MoonOriginDto,
  ObjectKindDto,
  PhaseDto,
  PlanetClassDto,
  SystemIdHex,
  UniverseIdHex,
  UniverseTime,
  VariableKindDto,
} from "@hyperion/protocol";

import type { Vec3 } from "../../spatial/vec3";
import type { KeplerOrbit } from "../orbit";

/**
 * A value that this generator version may not compute yet, as the wire's `Modelled<T>` carries it
 * (the orchestrator's ruling 54): not computed, computed with none, or a value.
 *
 * @remarks
 * `not_modelled` is the guide's Missing state, the em dash; `none` is data, such as a star that
 * does not vary.
 */
export type Modelled<T> =
  | { readonly kind: "not_modelled" }
  | { readonly kind: "none" }
  | { readonly kind: "value"; readonly value: T };

/** A value the wire leaves out until its task lands: not computed yet, or its value. */
export type Pending<T> =
  { readonly kind: "not_modelled" } | { readonly kind: "value"; readonly value: T };

/** The kick a remnant was born with. */
export interface NatalKick {
  readonly speedKmS: number;
  readonly mode: KickModeDto;
}

/** A neutron star seen as a pulsar. */
export interface Pulsar {
  readonly spinPeriodS: number;
  readonly periodDerivativeSPerS: number;
  readonly magneticFieldG: number;
  readonly alive: boolean;
  readonly magnetar: boolean;
}

/** What a dead star left, by kind, with what this generator version knows of it. */
export type HostRemnant =
  | {
      readonly kind: "white_dwarf";
      /** The time since it formed. */
      readonly coolingAgeMyr: number;
      readonly natalKick: Pending<NatalKick>;
    }
  | {
      readonly kind: "neutron_star";
      readonly pulsar: Pending<Pulsar>;
      readonly natalKick: Pending<NatalKick>;
    }
  | {
      readonly kind: "black_hole";
      /** c J ÷ (G M²), in `[0, 1)`. */
      readonly dimensionlessSpin: Pending<number>;
      readonly natalKick: Pending<NatalKick>;
    }
  | { readonly kind: "no_remnant" };

/** How a star's light varies. */
export interface Variability {
  readonly kind: VariableKindDto;
  readonly periodD: number;
  /** Peak to peak in V. */
  readonly amplitudeMag: number;
}

/** One star of a system, as it is at the answer's time. */
export interface HostBody {
  /** Its body ID as the wire writes it, the system's ID and its body index. */
  readonly id: BodyIdHex;
  /** 0 for the primary, 1–15 for companions (plan 14, design note D3). */
  readonly bodyIndex: number;
  /** Its designation of record: the system's, then ` /` and the body index (plan 14, D22). */
  readonly designation: string;
  readonly kind: ObjectKindDto;
  readonly phase: PhaseDto;
  /** Its class as an astronomer writes it: `G2V`, `DA9.2`, `NS`, `BH`, `NONE`. */
  readonly spectralClass: string;
  readonly initialMassMsun: number;
  /** A remnant's gravitational mass; zero where nothing is left. */
  readonly massMsun: number;
  /** Exactly zero for a black hole and where nothing is left. */
  readonly luminosityLsun: number;
  /** A remnant's physical radius, a black hole's Schwarzschild radius, zero where nothing is left. */
  readonly radiusRsun: number;
  /** `null` for an object with no light. */
  readonly teffK: number | null;
  /** `null` for a star still living. */
  readonly remnant: HostRemnant | null;
  /** When it dies, within the clock window; `null` when that is outside it. */
  readonly deathTime: UniverseTime | null;
  readonly rotationPeriodD: Modelled<number>;
  /** log₁₀ of the ratio of its X-ray to its bolometric luminosity. */
  readonly activityLogLxLbol: Modelled<number>;
  readonly variability: Modelled<Variability>;
}

/**
 * One node of a system's hierarchy: a star, or a pair of nodes on a relative orbit (plan 11's
 * `HierarchyDto`, listed depth first from the root, node 0).
 */
export type HierarchyNode =
  | {
      readonly kind: "star";
      /** The star's body index, and so its place among the hosts. */
      readonly bodyIndex: number;
      /** The mass the server places it by about each barycentre: its initial mass. */
      readonly massMsun: number;
    }
  | {
      readonly kind: "pair";
      /** The node list's index of the inner member, which holds the lower-indexed stars. */
      readonly inner: number;
      /** The node list's index of the outer member. */
      readonly outer: number;
      /** The outer member's barycentre about the inner member's, in the galactic axes. */
      readonly orbit: KeplerOrbit;
    };

/** One system at one time, as `system_summary` answered for it. */
export interface SystemModel {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  /** The instant the answer describes. */
  readonly time: UniverseTime;
  /** Whether the system exists at that time; its stars are born together. */
  readonly formed: boolean;
  /** Its age at that time, negative before it forms. */
  readonly ageMyr: number;
  /** Its metallicity [Fe/H], fixed at its birth. */
  readonly feHDex: number;
  /** Every star, by body index, the primary first; none before the system forms. */
  readonly hosts: ReadonlyArray<HostBody>;
  /** How the stars are paired, depth first from the root; empty before the system forms. */
  readonly hierarchy: ReadonlyArray<HierarchyNode>;
}

/**
 * An optional section of a body's or a system's record, in the state the server tagged it with
 * (the orchestrator's ruling 34; plan 14, P14.T34): the section's value, withheld at the granted
 * detail level, not computed by this generator version, or meaningless for the body's kind.
 *
 * @remarks
 * The client never infers a state. `not_modelled` reads `NOT YET MODELLED` and is never "none";
 * `not_resolved` reads `NOT RESOLVED`; `not_applicable` leaves the section out. "None" is a value.
 */
export type Section<T> =
  | { readonly state: "ok"; readonly value: T }
  | { readonly state: "not_resolved" }
  | { readonly state: "not_modelled" }
  | { readonly state: "not_applicable" };

/** What a body orbits: a star, a pair of stars, the system's barycentre, or another body. */
export type OrbitHost =
  | { readonly kind: "star"; readonly bodyIndex: number }
  | {
      readonly kind: "pair";
      /** The body index of the star the pair is keyed by: its outer member's first star. */
      readonly keyBodyIndex: number;
    }
  | { readonly kind: "barycentre" }
  | { readonly kind: "body"; readonly id: BodyIdHex };

/** What a body is (plan 14's `BodyKind`); a planet's class by composition is its bulk's. */
export type BodyKind =
  | { readonly kind: "planet" }
  | { readonly kind: "dwarf_planet" }
  | { readonly kind: "moon"; readonly origin: MoonOriginDto }
  | { readonly kind: "ring" }
  | { readonly kind: "belt"; readonly beltKind: BeltKindDto }
  | { readonly kind: "cometary_halo" }
  | { readonly kind: "protoplanetary_disc" }
  | { readonly kind: "debris_disc" }
  | { readonly kind: "unresolved" };

/** What has become of a body at the answer's time. */
export type BodyState =
  | { readonly kind: "not_yet_formed" }
  | { readonly kind: "present" }
  | { readonly kind: "destroyed"; readonly cause: DestructionCauseDto; readonly at: UniverseTime }
  | { readonly kind: "unbound"; readonly at: UniverseTime };

/** A body's orbit about what it orbits, and until when its elements hold. */
export interface BodyOrbit {
  readonly parent: OrbitHost;
  /** In the system frame for a planet; in its planet's frame, about its equator, for a moon. */
  readonly orbit: KeplerOrbit;
  /** The last instant at which the elements hold; `null` when that is past the clock window. */
  readonly validUntil: UniverseTime | null;
}

/** A body's mass fractions, each in `[0, 1]`. */
export interface MassFractions {
  readonly iron: number;
  readonly rock: number;
  readonly water: number;
  /** The hydrogen and helium envelope's. */
  readonly envelope: number;
}

/** A body's bulk section: what its derivation gives, less its mass. */
export interface BulkProperties {
  readonly radiusM: number;
  readonly densityKgM3: number;
  readonly surfaceGravityMS2: number;
  readonly planetClass: PlanetClassDto;
  readonly massFractions: MassFractions;
  readonly equilibriumTemperatureK: number;
}

/** One body of a system as the system's list carries it (the protocol's `BodySummaryDto`). */
export interface SystemBody {
  readonly id: BodyIdHex;
  /** Its index within its system, in plan 14's `slot << 8 | sub` layout (design note D3). */
  readonly bodyIndex: number;
  /** Its designation of record: the system's, then ` /` and the body index (plan 14, D22). */
  readonly designation: string;
  readonly kind: BodyKind;
  /** Its label for people, as `A b` (plan 14, D22). */
  readonly label: Section<string>;
  /** What it orbits; `null` only for a free-floating object, the root of its system. */
  readonly parent: OrbitHost | null;
  readonly state: BodyState;
  /**
   * Where it is at the answer's time, in metres from the system's barycentre along the galactic
   * axes; `null` for a body not present and for a population.
   */
  readonly positionM: Vec3 | null;
  /** Its mass in Earth masses, from the wire's kilograms by the simulation's Earth mass. */
  readonly massMearth: Section<number>;
  readonly orbit: Section<BodyOrbit>;
  readonly moons: Section<ReadonlyArray<BodyIdHex>>;
  readonly rings: Section<ReadonlyArray<BodyIdHex>>;
  readonly bulk: Section<BulkProperties>;
}

/** A body's hooks: what the generators of surfaces, life and civilisations read. */
export interface BodyHooks {
  /** The seed of its surface map, 16 lower-case hexadecimal digits. */
  readonly surfaceSeed: string;
}

/** One body's whole record (the protocol's `BodyRecordDto`): its list entry, its surface and hooks. */
export interface BodyRecord extends SystemBody {
  /** No generator version computes a surface yet, so an `ok` surface carries nothing. */
  readonly surface: Section<never>;
  readonly hooks: Section<BodyHooks>;
}

/** One body's record as `body_detail` answered for it. */
export interface BodyDetail {
  /** The instant the record describes. */
  readonly time: UniverseTime;
  /** The detail level the record holds. */
  readonly granted: DetailLevelDto;
  readonly record: BodyRecord;
}

/**
 * A plane in the system frame, by its normal's inclination to galactic north and its ascending
 * node, with the conventions of an orbit's elements.
 */
export interface SystemPlane {
  readonly inclinationRad: number;
  readonly ascendingNodeRad: number;
}

/**
 * A host's habitable zone at the answer's time: Kopparapu et al.'s five limits, in metres from the
 * host, each `null` where the companions alone give more than its flux.
 */
export interface HabitableZone {
  readonly recentVenusM: number | null;
  readonly runawayGreenhouseM: number | null;
  /** The conservative zone's inner edge. */
  readonly moistGreenhouseM: number | null;
  /** The conservative zone's outer edge. */
  readonly maximumGreenhouseM: number | null;
  readonly earlyMarsM: number | null;
  /** Whether the host's temperature lay outside the fit's range, so that the zone is extrapolated. */
  readonly extrapolated: boolean;
}

/** One stable zone of a system, and what belongs to its host (the protocol's `ZoneDto`). */
export interface Zone {
  /** A star, a pair or the barycentre; never a body. */
  readonly host: OrbitHost;
  /** The inner limit its companions set, m; `null` for a star's zone, which its disc bounds. */
  readonly innerM: number | null;
  /** The outer limit its companions set, m; `null` at the top of the hierarchy. */
  readonly outerM: number | null;
  readonly snowLineM: number;
  readonly plane: SystemPlane;
  readonly architecture: ArchitectureClassDto;
  /** `null` when the host has none at the answer's time. */
  readonly habitableZone: HabitableZone | null;
}

/** Every body of one system at one time, beside its hosts (the protocol's `SystemBodiesDto`). */
export interface SystemBodies {
  /** The detail level every record holds. */
  readonly granted: DetailLevelDto;
  /** One per host of planets, in hierarchy order and inside out. */
  readonly zones: ReadonlyArray<Zone>;
  /** The orbit map's reference plane (D21); `null` when the system has no zone. */
  readonly systemPlane: SystemPlane | null;
  readonly belts: Section<ReadonlyArray<BodyIdHex>>;
  readonly halo: Section<BodyIdHex | null>;
  /** Every body, in index order. */
  readonly bodies: ReadonlyArray<SystemBody>;
}
