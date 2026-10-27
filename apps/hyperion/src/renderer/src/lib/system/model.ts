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
  AerosolShapeDto,
  AerosolTauBasisDto,
  ArchitectureClassDto,
  BeltCompositionDto,
  BinaryClassDto,
  BeltKindDto,
  BeltSiteDto,
  BodyIdHex,
  ClimateForcingDto,
  ClimateModelDto,
  CraterTargetClassDto,
  CrustLidDto,
  DestructionCauseDto,
  DetailLevelDto,
  FigureDatumDto,
  FigureLawDto,
  KickModeDto,
  MagneticFieldClassDto,
  MoonOriginDto,
  ObjectKindDto,
  OceanFloorDto,
  PhaseDto,
  PhaseTemplateDto,
  PlanetClassDto,
  RadiationClassDto,
  RedoxClassDto,
  RingKindDto,
  RingMaterialDto,
  SpinResonanceDto,
  StarEventKindDto,
  SubstancePhaseDto,
  SurfaceMaterialDto,
  SurfaceStateDto,
  SystemIdHex,
  TaxonomicClassDto,
  TectonicRegimeDto,
  ThermalRegimeDto,
  UniverseIdHex,
  UniverseTime,
  VariableKindDto,
} from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
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

/** The planetary nebula a star lights (plan 06, P06.T16.b). */
export interface PlanetaryNebula {
  readonly radiusLy: number;
  readonly expansionSpeedKmS: number;
  /** The time since the shell was ejected. */
  readonly ageYr: number;
  /** The mass of its ionised gas. */
  readonly ionisedMassMsun: number;
  /** Its excitation class, 0–12 on Reid and Parker's (2010) scale. */
  readonly excitationClass: number;
}

/** An event in progress on a star at the answer's time. */
export interface StarEvent {
  readonly kind: StarEventKindDto;
  readonly onset: UniverseTime;
  readonly durationS: number;
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
  /** Its absolute visual magnitude `M_V`; `null` where the bolometric corrections have none. */
  readonly absoluteVMag: number | null;
  /** `null` for a star still living. */
  readonly remnant: HostRemnant | null;
  /** When it dies, within the clock window; `null` when that is outside it. */
  readonly deathTime: UniverseTime | null;
  readonly rotationPeriodD: Modelled<number>;
  /** log₁₀ of the ratio of its X-ray to its bolometric luminosity. */
  readonly activityLogLxLbol: Modelled<number>;
  readonly variability: Modelled<Variability>;
  readonly planetaryNebula: Modelled<PlanetaryNebula>;
  /** Its events in progress, by onset; none is an empty list. */
  readonly activeEvents: Pending<ReadonlyArray<StarEvent>>;
  /** The class of the interacting binary it belongs to (plan 11, P11.T5); none for no class. */
  readonly binaryClass: Modelled<BinaryClassDto>;
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
  /** From its hosts' light alone, K (ruling 112.7). */
  readonly equilibriumTemperatureK: number;
  /** A giant's, its internal heat included, K; `null` for a body with no luminosity of its own. */
  readonly effectiveTemperatureK: number | null;
}

/**
 * Whether and when a body's tides lock its spin (the protocol's `locking_age_s` and `locks_at`):
 * never; at a system age τ, from the system's birth, whose instant lies outside the clock's range;
 * or at τ, at an instant within it.
 */
export type RotationLock =
  | { readonly kind: "never" }
  | { readonly kind: "outside_clock"; readonly lockingAgeS: number }
  | { readonly kind: "in_clock"; readonly lockingAgeS: number; readonly locksAt: UniverseTime };

/**
 * A body's body-fixed frame and rotation law (the protocol's `BodyRotationDto`, plan 14's
 * P14.T46.f): its pole and equator's axes along the galactic axes, and every parameter of the
 * rotation angle W(t), which `rotation.ts` evaluates at any time in the clock window.
 */
export interface SystemBodyRotation {
  /** The spin axis, a unit vector along the galactic axes, by the right-hand rule. */
  readonly pole: Vec3;
  /** The equator's ascending node on the orbital plane, a unit vector: where W is 0. */
  readonly equatorNode: Vec3;
  /** The equator's axis a quarter-turn east of the node, pole × node: where W is π ÷ 2. */
  readonly equatorQuarter: Vec3;
  /** The angle between the pole and the orbit's normal, in `[0, π]`. */
  readonly obliquityRad: number;
  /** The primordial spin rate ω₀, positive: prograde about the pole. */
  readonly initialRateRadPerS: number;
  /** The spin rate after the lock, `ω_L`: the mean motion, or 1.5 times it in the 3:2 state. */
  readonly lockedRateRadPerS: number;
  /** The system's age at the epoch, positive. */
  readonly ageAtEpochS: number;
  /** Whether and when the body locks. */
  readonly lock: RotationLock;
  /** The state the body locks into. */
  readonly resonance: SpinResonanceDto;
  /** The locked angle's clock's period: the orbit's, or two orbits' for a 3:2 body. */
  readonly clockPeriodS: number;
  /** The clock's mean anomaly at the epoch. */
  readonly clockMeanAnomalyAtEpochRad: number;
  /** `W_p`, the angle at which the prime meridian faces the primary at pericentre. */
  readonly subPrimaryAngleRad: number;
  /** W at the epoch. */
  readonly phaseAtEpochRad: number;
  /** δ, the phase the capture into the resonance takes up before the lock. */
  readonly capturePhaseRad: number;
}

/**
 * A body's figure (the protocol's `BodyFigureDto`, plan 14's P14.T46.d–f): the reference spheroid
 * (a, a, c) about its pole that every height is measured from, of its bulk's volume.
 */
export interface SystemBodyFigure {
  /** a. */
  readonly equatorialRadiusM: number;
  /** c, at most a. */
  readonly polarRadiusM: number;
  /** (a − c) ÷ a, dimensionless. */
  readonly flattening: number;
  /** The rotation's pole, the spheroid's symmetry axis, a unit vector along the galactic axes. */
  readonly pole: Vec3;
  /** C ÷ M a², the factor the flattening was found with. */
  readonly momentOfInertiaFactor: number;
  readonly law: FigureLawDto;
  /** What heights are measured from. */
  readonly datum: FigureDatumDto;
}

/** A quantity in each of the Johnson B, V and R bands. */
export interface JohnsonBands {
  readonly b: number;
  readonly v: number;
  readonly r: number;
}

/**
 * A body's photometry (the protocol's `BodyPhotometryDto`, plan 14's P14.T47): the law its disc and
 * point are shaded with, per band, against π a c (R07 Design note 5).
 */
export interface SystemBodyPhotometry {
  /** The geometric albedo p per band, dimensionless. */
  readonly geometricAlbedo: JohnsonBands;
  readonly phaseTemplate: PhaseTemplateDto;
  /** The template's exponent s per band. */
  readonly phaseExponent: JohnsonBands;
  /** The law's Lommel–Seeliger share L, in `[0, 1]`. */
  readonly lunarLambertShare: number;
  /** The Bond albedo of the body's surface state, in `[0, 1)` (P14.T13.c). */
  readonly bondAlbedo: number;
  /** p_V q_V ÷ A_Bond as plan 14 states it: a check only, never an input. */
  readonly bondRatio: number;
  /** Whether the curve is borrowed (magma, airless ice, a snowball) or the body a hot giant. */
  readonly provisional: boolean;
}

/** A gap a moon's resonance clears in a massive ring. */
export interface RingGap {
  /** The moon whose resonance clears it. */
  readonly moon: BodyIdHex;
  /** The moon's period to the gap's, as the pair (p + 1, p): `[2, 1]`. */
  readonly resonance: readonly [number, number];
  /** Its distance from the planet's centre. */
  readonly radiusM: number;
}

/**
 * A giant's ring (the protocol's `RingDto`): its edges are distances from its planet's centre in
 * its plane, the one {@link Ring.normal} is normal to, or without a normal the planet's orbital
 * plane.
 */
export interface Ring {
  readonly ringKind: RingKindDto;
  readonly material: RingMaterialDto;
  readonly innerEdgeM: number;
  readonly outerEdgeM: number;
  /** The normal optical depth, dimensionless. */
  readonly opticalDepth: number;
  /** Inside out; none in a dusty ring. */
  readonly gaps: ReadonlyArray<RingGap>;
  /**
   * The normal of its plane, a unit vector along the galactic axes (plan 14, P14.T56.a): its
   * planet's local Laplace plane at the geometric mean of its edges, on the spin's side, which is
   * the planet's equator well inside its Laplace radius. `null` from a server that does not send
   * it, whose rings lie in their planet's orbital plane.
   */
  readonly normal: Vec3 | null;
}

/** One annulus of a belt, in metres from the belt's host. */
export interface BeltComponent {
  readonly innerEdgeM: number;
  readonly outerEdgeM: number;
}

/** A gap a planet's resonance clears in an asteroid belt. */
export interface BeltGap {
  /** The giant's period to the gap's: `[3, 1]`. */
  readonly resonance: readonly [number, number];
  /** Its distance from the belt's host. */
  readonly radiusM: number;
}

/**
 * A belt about an orbit host (the protocol's `BeltDto`): its edges lie in its host's zone's plane,
 * in metres from the host.
 */
export interface Belt {
  /** A star, a pair or the barycentre. */
  readonly host: OrbitHost;
  readonly site: BeltSiteDto;
  /** The whole belt's, its scattered component included. */
  readonly innerEdgeM: number;
  readonly outerEdgeM: number;
  /** The belt proper. */
  readonly main: BeltComponent;
  /** A Kuiper-like belt's scattered component; `null` for every other belt. */
  readonly scattered: BeltComponent | null;
  /** Inside out. */
  readonly gaps: ReadonlyArray<BeltGap>;
  /** The size slope q of N(> D) ∝ D^(−q). */
  readonly sizeSlope: number;
  readonly largestDiameterM: number;
  readonly composition: BeltCompositionDto;
  readonly meanEccentricity: number;
  /** To its host's plane. */
  readonly meanInclinationRad: number;
  /** Its dust's luminosity over its host's, dimensionless. */
  readonly fractionalLuminosity: number;
  /** Its dwarf planets, in index order; `not_resolved` below the `bulk` level. */
  readonly members: Section<ReadonlyArray<BodyIdHex>>;
}

/** A system's cometary halo (the protocol's `CometaryHaloDto`), a shell about its host. */
export interface CometaryHalo {
  readonly host: OrbitHost;
  /** The inner radius, from its host. */
  readonly innerEdgeM: number;
  /** The outer radius, from its host. */
  readonly outerEdgeM: number;
  /** Comets over 1 km across, a statistical figure. */
  readonly comets: number;
  /** Long-period comets reaching perihelion inside 5 au × √(L ÷ L☉) of the host, per second. */
  readonly cometRatePerS: number;
}

/** What a population body is and where it lies (the protocol's `PopulationDto`). */
export type Population =
  | ({ readonly kind: "ring" } & Ring)
  | ({ readonly kind: "belt" } & Belt)
  | ({ readonly kind: "cometary_halo" } & CometaryHalo);

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
  /** What a ring, a belt or the halo is and where it lies; `not_applicable` for any other body. */
  readonly population: Section<Population>;
  readonly bulk: Section<BulkProperties>;
  /**
   * Its body-fixed frame and rotation law (`bulk`); `not_modelled` where an older server leaves
   * the field out.
   */
  readonly rotation: Section<SystemBodyRotation>;
  /** Its figure (`bulk`); `not_modelled` where an older server leaves the field out. */
  readonly figure: Section<SystemBodyFigure>;
  /** Its photometry (`bulk`); `not_modelled` where an older server leaves the field out. */
  readonly photometry: Section<SystemBodyPhotometry>;
}

/** A body's hooks: what the generators of surfaces, life and civilisations read. */
export interface BodyHooks {
  /**
   * The seed of the client's local terrain synthesis, 16 lower-case hexadecimal digits, as a
   * section of its own: `not_modelled` until the server derives it (rendering plan R09). The
   * body's surface seed never reaches the client (R04, Design note 17).
   */
  readonly detailSeed: Section<string>;
}

/**
 * One gas of a mixture and its share by number.
 *
 * @remarks
 * The gas is a key of the substance registry, never a closed union: a console names it by
 * `substanceName` (`lib/substances.ts`), and a key it does not know is still read.
 */
export interface GasShare {
  readonly species: string;
  /** In `[0, 1]`. */
  readonly moleFraction: number;
}

/** One substance of a surface's cover, by its registry key, and the share of the surface it covers. */
export interface SubstanceArea {
  readonly substance: string;
  /** In `[0, 1]`. */
  readonly areaFraction: number;
}

/** A body's climate regime (plan 14's P14.T48.d): its three fields and the coarse model it names. */
export interface ClimateRegime {
  readonly thermal: ThermalRegimeDto;
  readonly forcing: ClimateForcingDto;
  /** The climate's condensable by its registry key; `null` for a world with none, as Venus. */
  readonly condensable: string | null;
  readonly model: ClimateModelDto;
}

/** An ocean under a body's surface (plan 14's P14.T24.b and P14.T51.g). */
export interface SubsurfaceOcean {
  readonly shellThicknessM: number;
  /** Its liquid's depth, from the shell's base to its floor. */
  readonly thicknessM: number;
  readonly floor: OceanFloorDto;
  /** Its liquid, by its registry key. */
  readonly liquid: string;
  /** In `[0, 1]`. */
  readonly soluteMassFraction: number;
  /** At the ice shell's base, where the shell meets it. */
  readonly baseTemperatureK: number;
}

/** What screens a body's small impactors before they crater the ground. */
export type CraterScreening =
  | { readonly kind: "none" }
  | { readonly kind: "atmosphere"; readonly columnMassKgM2: number }
  | { readonly kind: "cutoff"; readonly diameterM: number };

/**
 * A body's crater contract (plan 14's P14.T48.c with T48.f's impactors), whose gravity is the bulk
 * section's.
 */
export interface CraterContract {
  /** N☾(>1 km), the lunar-equivalent cumulative crater density above 1 km. */
  readonly lunarEquivalentN1kmPerKm2: number;
  readonly screening: CraterScreening;
  readonly projectileDensityKgM3: number;
  /** The target factor, dimensionless: 1 for rock, 0.12 for an ice-rich crust. */
  readonly kTarget: number;
  readonly targetClass: CraterTargetClassDto;
  readonly targetDensityKgM3: number;
  readonly systemAgeS: number;
  /** `null` where the contract gives none. */
  readonly impactVelocityMS: number | null;
}

/** The span over which a body's surface held liquid (plan 14's P14.T48.b), as times before now. */
export interface WetEpoch {
  /** The liquid that flowed, by its registry key. */
  readonly liquid: string;
  readonly startAgoS: number;
  /** Zero for a world wet now. */
  readonly endAgoS: number;
  /** The time channel-forming flow ran, in Earth-equivalent Julian years. */
  readonly effectiveFlowYr: number;
  readonly runoffMPerYr: number;
  /** The liquid inventory then, over the body's whole surface. */
  readonly paleoInventoryKgM2: number;
}

/** One node of a temperature profile. */
export interface ProfileNode {
  readonly pressurePa: number;
  readonly temperatureK: number;
}

/**
 * The vertical structure of a surface's air (plan 14's P14.T24.e): a scaled adiabat from the
 * section's surface temperature and pressure to an isothermal skin.
 */
export interface VerticalStructure {
  /** α, the factor on the dry adiabat's R ÷ c_p, dimensionless. */
  readonly alpha: number;
  readonly skinTemperatureK: number;
  /** From the surface up, the pressures descending. */
  readonly adiabat: ReadonlyArray<ProfileNode>;
}

/** An aerosol mode's size distribution, by its volume-to-area radius (plan 14's P14.T24.c). */
export type SizeDistribution =
  | {
      readonly kind: "gamma" | "log_normal";
      readonly effectiveRadiusM: number;
      /** Dimensionless. */
      readonly effectiveVariance: number;
    }
  | {
      readonly kind: "aggregate";
      readonly monomerRadiusM: number;
      readonly monomerCount: number;
      /** D_f, in `[1, 2.5]`. */
      readonly fractalDimension: number;
      /** k_f, dimensionless. */
      readonly fractalPrefactor: number;
    };

/** The phases an aerosol mode can hold. */
export type AerosolPhase = Extract<SubstancePhaseDto, "solid" | "liquid">;

/**
 * Why members of an aerosol mode are not modelled (plan 14's P14.T54.b.b): one reason a mode,
 * which says which of its members are.
 *
 * @remarks
 * `no_grain_density` carries the phase and temperature the generator asked for, that temperature
 * the mode's own {@link AerosolMode.temperatureK} (R08's review of P14.T54.b.b, S6). A size's
 * reason (`no_collision_constants`, `no_condensate`, `no_size_distribution`) leaves the size and
 * the optical depth not modelled, `optics_not_modelled` the optical depth (and a rule's column
 * mass), and `no_mass_extinction` an aggregate's column mass and temperature.
 */
export type AerosolNotModelled =
  | {
      readonly kind: "no_grain_density";
      readonly phase: AerosolPhase;
      readonly temperatureK: number;
    }
  | {
      readonly kind:
        | "no_collision_constants"
        | "no_condensate"
        | "no_size_distribution"
        | "optics_not_modelled"
        | "no_mass_extinction";
    };

/** A mode's column optical depth at 550 nm and what it rests on. */
export interface AerosolOpticalDepth {
  /**
   * Dimensionless: the column where the mode lies, over the share of the sky it covers (in-cloud,
   * {@link AerosolMode.coverage} not applied).
   */
  readonly tau550: number;
  readonly basis: AerosolTauBasisDto;
}

/**
 * One aerosol mode: a cloud's, a haze's or a dust layer's particles of one material and phase.
 *
 * @remarks
 * A member the server's generator does not model is not modelled, never none, and
 * {@link AerosolMode.notModelled} says why.
 */
export interface AerosolMode {
  /** Its material, by its registry key. */
  readonly material: string;
  readonly phase: AerosolPhase;
  /**
   * The deck it belongs to, by its condensate's key; `null` for a mode of no deck (the dust, a
   * haze, a sulphate layer).
   */
  readonly deck: string | null;
  readonly shape: AerosolShapeDto;
  readonly sizeDistribution: Pending<SizeDistribution>;
  readonly columnMassKgM2: Pending<number>;
  readonly opticalDepth: Pending<AerosolOpticalDepth>;
  readonly basePa: number;
  readonly topPa: number;
  /**
   * The exponent of its mixing ratio's (p ÷ p_base) law, dimensionless; 0 when well mixed. Its
   * optical depth is spread by the same profile.
   */
  readonly fSed: number;
  /** The share of the sky it covers, in `[0, 1]`. */
  readonly coverage: number;
  /** The temperature its phase, grain density and optical depth were taken at. */
  readonly temperatureK: Pending<number>;
  /** Why its members that are not modelled are not; `null` where every one is. */
  readonly notModelled: AerosolNotModelled | null;
}

/** A condensate deck: where it lies, and its vapour below and above it. */
export interface CondensateDeck {
  /** The condensate, by its registry key, which its modes' `deck` names. */
  readonly condensate: string;
  /** Its phase at its base, which it keeps throughout. */
  readonly phase: AerosolPhase;
  readonly basePa: number;
  readonly baseTemperatureK: number;
  readonly topPa: number;
  /**
   * In `[0, 1]`: the limiting vapour's, by the condensate's law, as the protocol's
   * `CondensateDeckDto` says (its own gas, a product law's scarcer reactant, or an element's atoms
   * per molecule of gas).
   */
  readonly vapourMoleFractionBelow: number;
  /** In `[0, 1]`: the same vapour's, above the deck. */
  readonly vapourMoleFractionAbove: number;
  /** Ackerman and Marley's f_sed, dimensionless; 0 for a deck of well-mixed modes. */
  readonly fSed: number;
  /** Whether the law that placed its base is applied outside what it was fitted over. */
  readonly extrapolated: boolean;
}

/** A condensable whose vapour an air holds that forms no deck, with why. */
export interface NotFormedCondensate {
  /** By its registry key. */
  readonly condensate: string;
  /**
   * `bulk_gas` for a vapour holding at least half the air; `displaced` by the condensate, by its
   * registry key, that takes its limiting vapour first; `supercritical` for a vapour whose partial
   * pressure reaches its critical pressure where the profile cools through its critical
   * temperature, so that it crosses no saturation curve (plan 14's P14.T52.f).
   */
  readonly reason:
    | { readonly kind: "bulk_gas" }
    | { readonly kind: "displaced"; readonly by: string }
    | { readonly kind: "supercritical" };
}

/** One node of an absorber's profile. */
export interface MixingRatioNode {
  readonly pressurePa: number;
  /** Relative to the profile's peak, in `[0, 1]`. */
  readonly relativeMixingRatio: number;
}

/** An absorbing gas that is not one of the section's well-mixed gases, with its column and layer. */
export interface Absorber {
  /** The gas, by its registry key. */
  readonly species: string;
  /** Its column, molecules per square metre. */
  readonly columnPerM2: number;
  readonly basePa: number;
  readonly topPa: number;
  /** Its nodes, the pressures descending; `null` for a layer of constant mixing ratio. */
  readonly profile: ReadonlyArray<MixingRatioNode> | null;
}

/**
 * A surface air's aerosol and absorber inventory (plan 14's P14.T24.c): every list empty for a
 * surface with no air; not modelled for a magma ocean that holds an air.
 */
export interface AerosolInventory {
  readonly modes: ReadonlyArray<AerosolMode>;
  /** At most one a condensate, so that a mode's `deck` names one. */
  readonly decks: ReadonlyArray<CondensateDeck>;
  readonly absorbers: ReadonlyArray<Absorber>;
  readonly notFormed: ReadonlyArray<NotFormedCondensate>;
}

/** The phases a surface condensate can hold. */
export type CondensatePhase = Exclude<SubstancePhaseDto, "vapour">;

/** One condensate at a surface (plan 14's P14.T51.a). */
export interface SurfaceCondensate {
  /** By its registry key. */
  readonly substance: string;
  readonly phase: CondensatePhase;
  /**
   * Its condensed mass in this phase over the whole surface, kg m⁻²: the list's sort key, and its
   * only amount where its layer is not modelled.
   */
  readonly reservoirKgM2: number;
  /**
   * Its reservoir as a global equivalent layer; on a body whose water froze, its whole frozen
   * water, shell, any ocean under a Snowball's ice and deeper ice included, until plan 14's
   * P14.T53.c divides it. Not modelled where the server's registry has not read the phase's
   * density, never none.
   */
  readonly reservoirGelM: Pending<number>;
  /** In `[0, 1]`. */
  readonly areaFraction: number;
  /**
   * The part of its area in cold traps, in `[0, areaFraction]`; not modelled until plan 14's
   * P14.T54.b.
   */
  readonly trappedAreaFraction: Pending<number>;
  readonly seasonal: boolean;
}

/** A body's crust (plan 14's P14.T51.c), each crust by its registry key or `null` for none. */
export interface CrustComposition {
  readonly primary: string | null;
  readonly secondary: string | null;
  readonly tertiary: string | null;
  /** An undifferentiated body's own material; `null` for a body with a crust. */
  readonly bulk: string | null;
  /** Which of them lies at the surface. */
  readonly lid: CrustLidDto;
  readonly provinces: string;
  readonly redox: RedoxClassDto;
  /** ΔIW, log₁₀ units of oxygen fugacity relative to the iron–wüstite buffer. */
  readonly mantleRedoxIw: number;
  /** In `[0, 1]`. */
  readonly meltAreaFraction: number;
}

/** A surface's modifiers (plan 14's P14.T51.d). */
export interface SurfaceModifiers {
  /** Dimensionless, 0 fresh to 1 mature. */
  readonly maturity: number;
  /**
   * Dimensionless, not negative: the space-weathering dose rate relative to the Moon's, which is
   * 1.
   */
  readonly weatheringRate: number;
  /** In `[0, 1]`: the covered fraction of the ground that can take an organic cover. */
  readonly organicCover: number;
  readonly resurfacingMPerYr: number;
  /**
   * An Io's SO₂ frost and sulphur, by registry key, each with its share of the surface; empty for
   * every other body. With the weathering deposits, together at most 1.
   */
  readonly volcanicDeposits: ReadonlyArray<SubstanceArea>;
  /**
   * An oxidised surface's `ferric` deposit (plan 14's P14.T51.j), by registry key, with its share
   * of the surface; empty for every other body. With the volcanic deposits, together at most 1.
   */
  readonly weatheringDeposits: ReadonlyArray<SubstanceArea>;
}

/** One net surface flux of a biosphere. */
export interface BiosphereFlux {
  /** By its registry key. */
  readonly species: string;
  /** In units of Earth's present biotic flux of it, of either sign: negative for a net sink. */
  readonly fluxEarth: number;
}

/** A biosphere's fluxes (plan 14's P14.T53.d). */
export interface Biosphere {
  readonly fluxes: ReadonlyArray<BiosphereFlux>;
}

/**
 * The vapour a surface's melt evaporates over it, its silicate vapour (the protocol's
 * `SilicateVapourDto`, on the wire by plan 14's P14.T35.f): uninhabited, as the wire's type is.
 *
 * @remarks
 * Its members are P14.T55.b's, which gives this type them; until then it has no value, and the
 * client reads none of a newer server's figures.
 */
export type SilicateVapour = never;

/**
 * A body's surface section (the protocol's `BodySurfaceDto`, plan 14's P14.T35.e): its state and
 * material, its surface conditions and climate regime, its global figures, and the members of later
 * tasks.
 *
 * @remarks
 * Every substance is a registry key. A member a later task fills is {@link Modelled}: not modelled
 * while the wire leaves it out, which is never "none", and `none` where the generator computes it
 * as none.
 */
export interface BodySurface {
  readonly state: SurfaceStateDto;
  readonly material: SurfaceMaterialDto;
  readonly meanSurfaceTemperatureK: number;
  /** T(0°) − T(90°): negative where the poles are the warmer. */
  readonly equatorPoleContrastK: number;
  readonly dayNightContrastK: number;
  /**
   * The volatile air's: zero for an airless body, and for a surface holding melt whose volatiles
   * are gone.
   *
   * @remarks
   * It does not include the melt's own vapour, {@link BodySurface.silicateVapour}.
   */
  readonly surfacePressurePa: number;
  /**
   * Largest first, summing to 1; empty for an airless body, or for a surface holding melt with no
   * volatile air, which does not deny the melt's own vapour.
   */
  readonly gases: ReadonlyArray<GasShare>;
  /** `null` with an empty {@link BodySurface.gases}. */
  readonly meanMolarMassGPerMol: number | null;
  /** `c_p` ÷ R, dimensionless; `null` with an empty {@link BodySurface.gases}. */
  readonly heatCapacityOverR: number | null;
  readonly stellarFluxWM2: number;
  readonly radiationClass: RadiationClassDto;
  /** Whether water can be liquid somewhere in the climate's range at the surface pressure. */
  readonly liquidWater: boolean;
  readonly climateRegime: ClimateRegime;
  /** In `[0, 1]`: the sum of {@link BodySurface.surfaceLiquids}. */
  readonly oceanFraction: number;
  /** In `[0, 1]`: the sum of {@link BodySurface.surfaceIces}. */
  readonly iceFraction: number;
  /** In `[0, 1]`. */
  readonly cloudFraction: number;
  readonly surfaceLiquids: ReadonlyArray<SubstanceArea>;
  readonly surfaceIces: ReadonlyArray<SubstanceArea>;
  /** σ_h, the hypsometric standard deviation. */
  readonly sigmaHM: number;
  /** In `[0, 1]`. */
  readonly continentalFraction: number;
  readonly heatFlowWM2: number;
  readonly tectonicRegime: TectonicRegimeDto;
  /** V, dimensionless: the Solar System table's Mars's is 1. */
  readonly volcanismLevel: number;
  /** The present outgassing over Earth's present rate. */
  readonly outgassingEarth: number;
  readonly magneticFieldClass: MagneticFieldClassDto;
  readonly surfaceAgeS: number;
  /** Not modelled for a body no rule computes; `none` for one computed to hold none. */
  readonly subsurfaceOcean: Modelled<SubsurfaceOcean>;
  readonly craters: CraterContract;
  /** `null` for a world never wet. */
  readonly wetEpoch: WetEpoch | null;
  readonly verticalStructure: Modelled<VerticalStructure>;
  readonly aerosols: Modelled<AerosolInventory>;
  readonly condensates: Modelled<ReadonlyArray<SurfaceCondensate>>;
  readonly crust: Modelled<CrustComposition>;
  readonly modifiers: Modelled<SurfaceModifiers>;
  /**
   * The body's taxonomic class (P14.T51.e), the class's name in lower case as the wire sends it.
   *
   * @remarks
   * Not modelled from every server before generator version 24, and where the record lacks what
   * the body's rule reads (its crust, or its modifiers); `none` for a planet, and for a member or
   * moon the rules leave unclassed.
   */
  readonly taxonomicClass: Modelled<TaxonomicClassDto>;
  readonly biosphere: Modelled<Biosphere>;
  /**
   * The melt's own vapour (P14.T35.f), which is not part of the volatile air of
   * {@link BodySurface.surfacePressurePa} and {@link BodySurface.gases}.
   *
   * @remarks
   * Not modelled from every server until P14.T55.b; from then, `none` on a surface computed to hold
   * no melt, and a value over melt, whose members this client does not read yet.
   */
  readonly silicateVapour: Modelled<SilicateVapour>;
}

/** An envelope's irradiated grey profile's parameters (Guillot 2010, Eq. 29). */
export interface GuillotProfile {
  /** κ_th. */
  readonly thermalOpacityM2PerKg: number;
  /** γ = κ_v ÷ κ_th, dimensionless. */
  readonly opacityRatio: number;
  /** f, dimensionless: ¼ averaged over the sphere. */
  readonly redistributionFactor: number;
}

/**
 * A gas-envelope body's visible atmosphere (the protocol's `BodyEnvelopeDto`, plan 14's P14.T24.d,
 * on the wire by P14.T35.e): the section of a giant or a sub-Neptune, whose surface section does
 * not apply, and from generator version 24 of a solid body under a supercritical envelope
 * (P14.T53.b part 2), sent `not_modelled` until P14.T53.b part 3.
 */
export interface BodyEnvelope {
  /**
   * The condensable whose supercritical fluid the envelope is, by its registry key; `null` for an
   * H₂/He envelope. No server sends it other than `null` before P14.T53.b part 3: a solid body
   * under a supercritical envelope has its envelope `not_modelled` until then.
   */
  readonly supercritical: string | null;
  /** Zero for a body with no internal heat. */
  readonly internalTemperatureK: number;
  readonly irradiationTemperatureK: number;
  readonly profile: GuillotProfile;
  /** Where the profile joins the dry adiabat. */
  readonly joinPressurePa: number;
  /** Below the join, the pressures ascending, to 100 bar. */
  readonly adiabat: ReadonlyArray<ProfileNode>;
  /** Largest first, summing to 1. */
  readonly composition: ReadonlyArray<GasShare>;
  readonly meanMolarMassGPerMol: number;
  /** `c_p` ÷ R at the 1-bar level's temperature, dimensionless, for display. */
  readonly heatCapacityOverR: number;
  readonly decks: ReadonlyArray<AerosolMode>;
  /** The haze's optical depth at 550 nm, dimensionless. */
  readonly hazeTau550: number;
}

/**
 * One body's whole record (the protocol's `BodyRecordDto`): its list entry, its surface, envelope
 * and hooks.
 */
export interface BodyRecord extends SystemBody {
  /**
   * `not_applicable` for a body in the gas-envelope state, and from generator version 24 for a
   * solid body under a supercritical envelope (P14.T53.b part 2), whose air is its envelope.
   */
  readonly surface: Section<BodySurface>;
  /**
   * A gas-envelope body's air (`not_modelled` until P14.T24.d), a solid body's under a
   * supercritical envelope from generator version 24 (`not_modelled` until P14.T53.b part 3), and
   * `not_applicable` for every other body; `not_modelled` where an older server leaves the field
   * out.
   */
  readonly envelope: Section<BodyEnvelope>;
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
