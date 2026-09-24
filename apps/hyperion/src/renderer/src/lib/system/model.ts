/**
 * The client's model of one planetary system at one time, with units in its names (plan 14, phase
 * I).
 *
 * @remarks
 * Wire shapes stop at `wire.ts`, which builds these types from a `system_summary` answer, as plan
 * 05's design note D8 has `lib/galaxy/wire.ts` do for the chart, so that the `SYSTEM` display never
 * reads a wire field name. The kinds and phases pass through as the wire's string unions, since
 * they are already names. The hosts are the system's stars; the planets, their moons and rings, the
 * belts and the halo join as further bodies with `system_bodies` (P14.T35.b).
 */
import type {
  BodyIdHex,
  KickModeDto,
  ObjectKindDto,
  PhaseDto,
  SystemIdHex,
  UniverseIdHex,
  UniverseTime,
  VariableKindDto,
} from "@hyperion/protocol";

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
