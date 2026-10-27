/**
 * The client's writer of R08.T13's sweep cases (plan R08, R08.T13.a; `decision-r08-t13-scope.md`,
 * rulings 2–4 and 8): the media of the sweep's grid, its geometry set, and `SWEEP_FORMAT` 1, the
 * file `hyperion-fit atmosphere-sweep` reads (`crates/hyperion-fit/src/atmosphere/sweep.rs`), with
 * the CPU twin's radiance at every candidate point (`view/atmosphere/reference/twin.ts`).
 *
 * @remarks
 * - **The layer** (ruling 2): one term on a sphere of `earth`'s radius, R05's `tableRadiusM` of
 *   WGS 84 ({@link EARTH_REFERENCE_RADIUS_M}), its density e^(−h ÷ H) at H = 8 km up to a top of
 *   100 km (12.5 H), over a grey Lambertian ground of 0.3, or 0.8 in the bright column: both hand
 *   values, near the Bond albedos of Earth and Mars, 0.294 and 0.250 (NASA NSSDC's planetary fact
 *   sheets), which are whole-planet values, clouds and air included.
 * - **Grey, one wavelength, scalar.** The medium is the same in every channel: scattering ω β₀ and
 *   absorption (1 − ω) β₀ at relative density 1, the tracer tracing 550 nm alone and the twin's
 *   three channels equal, of which the file keeps channel 1. τ is the vertical extinction optical
 *   depth from the ground to the top, β₀ H (1 − e^(−12.5)), so β₀ = τ ÷ (H (1 − e^(−12.5))), and
 *   the file carries β₀ so that both pipelines read the same one (ruling 2; main's reading 7).
 * - **The phase.** At g = 0, Rayleigh's with dry air's depolarisation at 550 nm,
 *   ρ = `depolarisationOfKingFactor(dryAirKingFactor(550))` (Bates 1984's King factors through
 *   Bodhaine et al. 1999, eq. 23, `rayleigh.ts`): the gas worlds' phase. At g > 0, Henyey and
 *   Greenstein 1941's (ApJ 93, 70, eq. 2, p. 76), (1 − g²) ÷ (4π (1 + g² − 2gμ)^1.5), on the
 *   client's u grid ({@link PHASE_TABLE_U}, the kernels' own), normalised so that its integral
 *   read linearly in u is 1, its mean cosine within 10⁻⁴ of g (the tracer's check on reading): no
 *   forward delta and no matrix, both pipelines reading the one table.
 * - **The grid** (ruling 3): ω ∈ {1, 0.99, 0.95, 0.9, 0.8, 0.6, 0.4} a file, g ∈ {0, 0.3, 0.5,
 *   0.65, 0.75, 0.85, 0.9} its columns, and τ on the half-octave ladder from 1/8 to 8 below which
 *   the floor point, 1/32, stands: fifteen candidates a column. The bright column, ω 1 and g 0 over
 *   a ground of 0.8, is a file of its own, under the same prefix (main's reading 8).
 * - **The geometry set** (ruling 4), 54 geometries and the fluxes a point: the ground's 41 views
 *   (view zenith {0, 30, 60, 75, 85}° × relative azimuth {0, 90, 180}° × sun zenith
 *   {0, 30, 60, 80}°, less the duplicates and
 *   the views within 3° of the sun), the disc at its five phases, the limb at its five tangent
 *   heights in the template's H, and aerial perspective over its three paths, with `geometries.ts`'s
 *   suns; and the fluxes at a sun zenith of 0°, recorded and never setting τ*. Each geometry
 *   records once its cone's share that meets the ground (`twin.ts`'s `groundConeShare`; the
 *   ruling's "#107's twin ground"), where the twin draws the lit ground the tracer reflects off:
 *   recorded, never gated.
 * - **The floor scan** (`decision-r08-t13-scope.md`, "#107's floor misses", item 2): each column's
 *   floor point, τ 1/32, again over each of {@link SWEEP_SCAN_GROUND_ALBEDOS}'s grey grounds, the
 *   twin's radiance at each, so that the tracer can tell a geometry the floor fails over a black
 *   ground (a fixed miss) from one it fails over a brighter ground alone (a ground miss), and find
 *   A*(ω, g), the least albedo of a ground miss (`sweep.rs`'s module documentation).
 * - **Seeds** (main's reading 4): every file takes the case seed {@link SWEEP_CASE_SEED}, a point's
 *   final trace the seed plus its index and its pilot {@link SWEEP_PILOT_SEED} plus its index; the
 *   index is unique over the eight files, ω row × 105 + g column × 15 + τ candidate, the bright
 *   column's 735 + τ candidate, and a scan point's 750 + 7 × (ω row × 7 + g column) + its ground,
 *   the bright column's 1,093 + its ground.
 *
 * Typed arrays become plain arrays, which `bless.ts`'s `tableText` requires.
 */

import type { Rgb } from "../view/photometry/toneCurve";
import {
  henyeyGreenstein,
  phaseTableAsymmetry,
  phaseTableIntegral,
} from "../view/atmosphere/aerosol";
import { type AtmosphereMedium, type PhaseFunction, phaseTable } from "../view/atmosphere/medium";
import { depolarisationOfKingFactor, dryAirKingFactor } from "../view/atmosphere/rayleigh";
import {
  EARTH_REFERENCE_RADIUS_M,
  SPHERE_ORBIT_HEIGHT_M,
} from "../view/atmosphere/reference/cases";
import {
  aerialSet,
  discSet,
  GROUND_OBSERVER_HEIGHT_M,
  groundSet,
  type LocalDirection,
  REFERENCE_DETECTOR_HALF_ANGLE_DEG,
  type ReferenceAggregate,
  type ReferenceGeometry,
  sphereLimbSet,
} from "../view/atmosphere/reference/geometries";
import {
  groundConeShare,
  referenceTwin,
  type TwinFigure,
  type TwinGeometryKind,
  twinGeometryKind,
} from "../view/atmosphere/reference/twin";
import { PHASE_TABLE_MU, PHASE_TABLE_U } from "../view/atmosphere/sizeDistribution";

/** The sweep format the writer writes, `sweep.rs`'s `SWEEP_FORMAT`. */
export const SWEEP_FORMAT = 1;

/** The sweep's figure: a sphere of `earth`'s radius (ruling 2). */
export const SWEEP_FIGURE: TwinFigure = { kind: "sphere", radiusM: EARTH_REFERENCE_RADIUS_M };

/** The template's scale height, m: 8 km (ruling 2). */
export const SWEEP_SCALE_HEIGHT_M = 8_000;

/** The template's top, m: 100 km, 12.5 scale heights. */
export const SWEEP_TOP_HEIGHT_M = 100_000;

/** The template's ground albedo: 0.3, a hand value (the module's remarks). */
export const SWEEP_GROUND_ALBEDO = 0.3;

/** The bright column's ground albedo: 0.8, a hand value, a bright snow's (ruling 3). */
export const SWEEP_BRIGHT_GROUND_ALBEDO = 0.8;

/** The one wavelength the sweep traces, nm. */
export const SWEEP_WAVELENGTH_NM = 550;

/** The sun's irradiance at the top, in any unit: 1. */
export const SWEEP_SUN_IRRADIANCE = 1;

/** Every sweep file's case seed (main's reading 4), clear of `REFERENCE_SEEDS` and the pilots'. */
export const SWEEP_CASE_SEED = 10_000;

/** The first pilot seed, a point's pilot taking it plus its index (ruling 6). */
export const SWEEP_PILOT_SEED = 1_000_000;

/** The pilot's samples a geometry, N_p (ruling 6). */
export const SWEEP_PILOT_SAMPLES = 2_048;

/** The single-scattering albedos, one file each (ruling 3). */
export const SWEEP_SINGLE_SCATTERING_ALBEDOS: ReadonlyArray<number> = [
  1, 0.99, 0.95, 0.9, 0.8, 0.6, 0.4,
];

/** The asymmetries, a column each, Rayleigh's at 0 (ruling 3). */
export const SWEEP_ASYMMETRIES: ReadonlyArray<number> = [0, 0.3, 0.5, 0.65, 0.75, 0.85, 0.9];

/** The role of a τ candidate in the walk (`sweep.rs`): the floor, an octave or a half-octave point. */
export type TauRole = "floor" | "octave" | "half-octave";

/** One τ candidate. */
export interface TauCandidate {
  readonly tau: number;
  readonly role: TauRole;
}

/**
 * The fifteen τ candidates of a column (ruling 3): the floor point, 1/32, then the half-octave
 * ladder 2^(k ÷ 2) from 2^(−3.5) to 8, its whole powers of 2 the octave points.
 */
export const SWEEP_TAU_CANDIDATES: ReadonlyArray<TauCandidate> = [
  { tau: 1 / 32, role: "floor" },
  ...Array.from({ length: 14 }, (_, i): TauCandidate => {
    const k = i - 7;
    return { tau: 2 ** (k / 2), role: k % 2 === 0 ? "octave" : "half-octave" };
  }),
];

/** The candidates' count, a column's. */
export const SWEEP_TAU_COUNT = SWEEP_TAU_CANDIDATES.length;

/** The indices one ω row takes: 7 columns of 15 (main's reading 4). */
const ROW_INDICES = SWEEP_ASYMMETRIES.length * SWEEP_TAU_COUNT;

/** The bright column's first index: after the seven rows' 735. */
export const SWEEP_BRIGHT_FIRST_INDEX = SWEEP_SINGLE_SCATTERING_ALBEDOS.length * ROW_INDICES;

/**
 * The floor scan's grey grounds, ascending from the black (`decision-r08-t13-scope.md`, "#107's
 * floor misses", item 2): 0, 0.02, 0.05, 0.1, 0.2, 0.5 and 0.8, the ruling's.
 */
export const SWEEP_SCAN_GROUND_ALBEDOS: ReadonlyArray<number> = [0, 0.02, 0.05, 0.1, 0.2, 0.5, 0.8];

/** The scan's first point index: after the bright column's, 750. */
export const SWEEP_SCAN_FIRST_INDEX = SWEEP_BRIGHT_FIRST_INDEX + SWEEP_TAU_COUNT;

/** A column's first scan index, by its place among the fifty columns, the bright column last. */
function scanFirstIndex(columnOfGrid: number): number {
  return SWEEP_SCAN_FIRST_INDEX + columnOfGrid * SWEEP_SCAN_GROUND_ALBEDOS.length;
}

/** The sweep ground's sun zeniths, degrees: Design note 10's but the 90° and 95° suns (ruling 4). */
export const SWEEP_GROUND_SUN_ZENITHS_DEG: ReadonlyArray<number> = [0, 30, 60, 80];

/** The sweep ground's relative azimuths, degrees: Design note 10's but 45° and 135° (ruling 4). */
export const SWEEP_GROUND_RELATIVE_AZIMUTHS_DEG: ReadonlyArray<number> = [0, 90, 180];

/** Dry air's depolarisation ratio at 550 nm, the Rayleigh column's ρ (the module's remarks). */
export const SWEEP_RAYLEIGH_DEPOLARISATION = depolarisationOfKingFactor(
  dryAirKingFactor(SWEEP_WAVELENGTH_NM),
);

/** A geometry's kind, as the sweep groups its σ and costs. */
export type SweepGeometryKind = "ground" | "disc" | "limb" | "aerial";

/** One geometry of the sweep with its kind. */
export interface SweepGeometry extends ReferenceGeometry {
  readonly kind: SweepGeometryKind;
}

/** A set of geometries, each marked with its kind. */
function kinded(set: ReadonlyArray<ReferenceGeometry>, kind: SweepGeometryKind): SweepGeometry[] {
  return set.map((g) => ({ ...g, kind }));
}

/**
 * The sweep's 54 geometries a point, ruling 4's 55 with {@link SWEEP_AGGREGATE}, in the order the
 * files hold them.
 */
export function sweepGeometries(): SweepGeometry[] {
  const ground = groundSet(
    "ground",
    { heightM: GROUND_OBSERVER_HEIGHT_M, latitudeDeg: 0 },
    SWEEP_GROUND_RELATIVE_AZIMUTHS_DEG,
    0,
    true,
    SWEEP_GROUND_SUN_ZENITHS_DEG,
  );
  return [
    ...kinded(ground, "ground"),
    ...kinded(discSet(SPHERE_ORBIT_HEIGHT_M), "disc"),
    ...kinded(
      sphereLimbSet(EARTH_REFERENCE_RADIUS_M, SWEEP_SCALE_HEIGHT_M, SPHERE_ORBIT_HEIGHT_M),
      "limb",
    ),
    ...kinded(aerialSet(), "aerial"),
  ];
}

/** The sweep's one aggregate: the fluxes under a sun at the zenith (ruling 4). */
export const SWEEP_AGGREGATE: ReferenceAggregate = {
  name: "flux-s0",
  latitudeDeg: 0,
  sun: { zenithDeg: 0, azimuthDeg: 0 },
};

/**
 * β₀, m⁻¹: the extinction at relative density 1 that gives a vertical optical depth `tau` from
 * the ground to the top (the module's remarks).
 */
export function sweepExtinctionPerM(tau: number): number {
  return tau / (SWEEP_SCALE_HEIGHT_M * (1 - Math.exp(-SWEEP_TOP_HEIGHT_M / SWEEP_SCALE_HEIGHT_M)));
}

/** A Henyey–Greenstein table on {@link PHASE_TABLE_U} for one channel, normalised (the remarks). */
export function henyeyGreensteinEntries(asymmetry: number): Float64Array {
  const raw = PHASE_TABLE_MU.map((mu) => henyeyGreenstein(asymmetry, mu));
  const integral = phaseTableIntegral(phaseTable(PHASE_TABLE_U, [raw, raw, raw]), 0);
  return raw.map((v) => v / integral);
}

/**
 * A column's phase (the module's remarks).
 *
 * @throws RangeError for a table whose mean cosine misses g by more than 10⁻⁴, which the tracer
 *   would refuse.
 */
export function sweepPhase(asymmetry: number): PhaseFunction {
  if (asymmetry === 0) {
    const rho = SWEEP_RAYLEIGH_DEPOLARISATION;
    return { kind: "rayleigh", depolarisation: [rho, rho, rho] };
  }
  const entries = henyeyGreensteinEntries(asymmetry);
  const table = phaseTable(PHASE_TABLE_U, [entries, entries, entries]);
  const meanCosine = phaseTableAsymmetry(table, 0);
  if (!(Math.abs(meanCosine - asymmetry) <= 1e-4)) {
    throw new RangeError(`the table of g ${asymmetry} has the mean cosine ${meanCosine}`);
  }
  return { kind: "tabulated", table };
}

/** One point of the grid: ω, g, τ and the ground's albedo. */
export interface SweepPoint {
  readonly singleScatteringAlbedo: number;
  readonly asymmetry: number;
  readonly tau: number;
  readonly groundAlbedo: number;
}

/** A point's medium, grey in all three channels (the module's remarks). */
export function sweepMedium(point: SweepPoint): AtmosphereMedium {
  const beta = sweepExtinctionPerM(point.tau);
  const omega = point.singleScatteringAlbedo;
  const scattering = omega * beta;
  const absorption = (1 - omega) * beta;
  const albedo = point.groundAlbedo;
  return {
    name: "ms-sweep",
    topHeightM: SWEEP_TOP_HEIGHT_M,
    groundAlbedo: [albedo, albedo, albedo],
    terms: [
      {
        name: "medium",
        density: { kind: "exponential", scaleHeightM: SWEEP_SCALE_HEIGHT_M },
        scattering: [scattering, scattering, scattering],
        absorption: [absorption, absorption, absorption],
        phase: sweepPhase(point.asymmetry),
      },
    ],
  };
}

/** The largest relative spread of a triple the twin calls equal: rounding's, a few ulps. */
export const TWIN_CHANNEL_SPREAD = 1e-12;

/**
 * The twin's channel-1 radiance of a point at each of `geometries`, per unit of the sun's
 * irradiance.
 *
 * @throws Error where the twin's three channels differ beyond {@link TWIN_CHANNEL_SPREAD}, or a
 *   radiance is not finite and at least 0: a blocker of the ruling's 12.
 */
export function sweepTwinRadiances(
  point: SweepPoint,
  geometries: ReadonlyArray<ReferenceGeometry>,
): number[] {
  const twin = referenceTwin(sweepMedium(point), SWEEP_FIGURE);
  return geometries.map((geometry) => {
    const rgb: Rgb = twin.radiance(geometry);
    const [r, g, b] = rgb;
    if (![r, g, b].every((v) => Number.isFinite(v) && v >= 0)) {
      throw new Error(`the twin's radiance at ${geometry.name} is ${rgb.join(", ")}`);
    }
    const spread = Math.max(Math.abs(r - g), Math.abs(b - g));
    if (spread > TWIN_CHANNEL_SPREAD * Math.max(Math.abs(g), Number.MIN_VALUE)) {
      throw new Error(`the twin's channels differ at ${geometry.name}: ${rgb.join(", ")}`);
    }
    return g;
  });
}

/** One file of the sweep: its ω, its columns' g and its ground. */
export interface SweepFileSpec {
  /** `ms-sweep-w<ω×1000>`, and `-a<albedo×1000>` for the bright column. */
  readonly name: string;
  readonly singleScatteringAlbedo: number;
  readonly asymmetries: ReadonlyArray<number>;
  readonly groundAlbedo: number;
  /** Each column's first point index. */
  readonly firstIndices: ReadonlyArray<number>;
  /** Each column's first scan point index. */
  readonly scanFirstIndices: ReadonlyArray<number>;
}

/** A thousandth's label: ω 0.95 is `w950`. */
function thousandths(value: number): string {
  return String(Math.round(value * 1000));
}

/** The eight files (ruling 8; main's reading 8): one an ω row, and the bright column's. */
export const SWEEP_FILES: ReadonlyArray<SweepFileSpec> = [
  ...SWEEP_SINGLE_SCATTERING_ALBEDOS.map((omega, row) => ({
    name: `ms-sweep-w${thousandths(omega)}`,
    singleScatteringAlbedo: omega,
    asymmetries: SWEEP_ASYMMETRIES,
    groundAlbedo: SWEEP_GROUND_ALBEDO,
    firstIndices: SWEEP_ASYMMETRIES.map(
      (_, column) => row * ROW_INDICES + column * SWEEP_TAU_COUNT,
    ),
    scanFirstIndices: SWEEP_ASYMMETRIES.map((_, column) =>
      scanFirstIndex(row * SWEEP_ASYMMETRIES.length + column),
    ),
  })),
  {
    name: `ms-sweep-w1000-a${thousandths(SWEEP_BRIGHT_GROUND_ALBEDO)}`,
    singleScatteringAlbedo: 1,
    asymmetries: [0],
    groundAlbedo: SWEEP_BRIGHT_GROUND_ALBEDO,
    firstIndices: [SWEEP_BRIGHT_FIRST_INDEX],
    scanFirstIndices: [
      scanFirstIndex(SWEEP_SINGLE_SCATTERING_ALBEDOS.length * SWEEP_ASYMMETRIES.length),
    ],
  },
];

/** A point of a column: a τ candidate of the walk, or the floor over one of the scan's grounds. */
export type SweepSpot =
  | { readonly kind: "walk"; readonly candidateIndex: number }
  | { readonly kind: "scan"; readonly groundIndex: number };

/** The walk's spot at τ candidate `candidateIndex` of {@link SWEEP_TAU_CANDIDATES}. */
export function walkSpot(candidateIndex: number): SweepSpot {
  return { kind: "walk", candidateIndex };
}

/** The scan's spot over ground `groundIndex` of {@link SWEEP_SCAN_GROUND_ALBEDOS}. */
export function scanSpot(groundIndex: number): SweepSpot {
  return { kind: "scan", groundIndex };
}

/** A column's phase as the format writes it. */
export type SweepPhaseJson =
  { readonly kind: "rayleigh" } | { readonly kind: "tabulated"; readonly a1: readonly number[] };

/** One column of a file as the format writes it. */
export interface SweepColumnJson {
  readonly asymmetry: number;
  readonly phase: SweepPhaseJson;
  /** Each τ candidate's point index. */
  readonly points: readonly number[];
  /** The twin's channel-1 radiance at each τ candidate (rows) and geometry (entries). */
  readonly twin: readonly (readonly number[])[];
  /** Each scan ground's point index. */
  readonly scanPoints: readonly number[];
  /** The twin's channel-1 radiance at the floor over each scan ground (rows), by geometry. */
  readonly scanTwin: readonly (readonly number[])[];
}

/** A geometry as the format writes it. */
export interface SweepGeometryJson {
  readonly name: string;
  readonly image: string;
  readonly kind: SweepGeometryKind;
  readonly observer: { readonly heightM: number; readonly latitudeDeg: number };
  readonly view: LocalDirection;
  readonly sun: LocalDirection;
  readonly maxDistanceM?: number;
  /**
   * The share of its cone that meets the ground within the view (`twin.ts`'s `groundConeShare`):
   * recorded, never gated (the ruling's "#107's twin ground").
   */
  readonly groundConeShare: number;
}

/** A sweep file (the format's fields, `sweep.rs`'s module documentation). */
export interface SweepFileJson {
  readonly format: number;
  readonly name: string;
  readonly description: string;
  readonly seed: number;
  readonly pilotSeed: number;
  readonly pilotSamples: number;
  readonly template: {
    readonly radiusM: number;
    readonly scaleHeightM: number;
    readonly topHeightM: number;
    readonly groundAlbedo: number;
    readonly wavelengthNm: number;
    readonly sunIrradiance: number;
    readonly detectorHalfAngleDeg: number;
    readonly rayleighDepolarisation: number;
  };
  readonly singleScatteringAlbedo: number;
  readonly tau: readonly number[];
  readonly tauRoles: readonly TauRole[];
  readonly extinctionPerM: readonly number[];
  readonly phaseU: readonly number[];
  readonly geometries: readonly SweepGeometryJson[];
  readonly aggregate: {
    readonly name: string;
    readonly latitudeDeg: number;
    readonly sun: LocalDirection;
  };
  /** The floor scan's grounds, {@link SWEEP_SCAN_GROUND_ALBEDOS}. */
  readonly scanGroundAlbedos: readonly number[];
  readonly columns: readonly SweepColumnJson[];
}

/** What each file says of itself. */
export const SWEEP_DESCRIPTION =
  "Written by apps/hyperion/src/renderer/src/view/atmosphere/reference/sweepCases.test.ts (plan R08, R08.T13.a) from the client's CPU twin; read by crates/hyperion-fit/src/atmosphere/sweep.rs. Do not edit by hand.";

/** How the twin draws each kind of geometry (`twin.ts`'s remarks). */
const DRAWN_AS: Readonly<Record<SweepGeometryKind, TwinGeometryKind>> = {
  ground: "ground",
  disc: "orbit",
  limb: "orbit",
  aerial: "aerial",
};

/** A geometry in the format. */
function geometryJson(geometry: SweepGeometry): SweepGeometryJson {
  return {
    name: geometry.name,
    image: geometry.image,
    kind: geometry.kind,
    observer: { heightM: geometry.observer.heightM, latitudeDeg: geometry.observer.latitudeDeg },
    view: { zenithDeg: geometry.view.zenithDeg, azimuthDeg: geometry.view.azimuthDeg },
    sun: { zenithDeg: geometry.sun.zenithDeg, azimuthDeg: geometry.sun.azimuthDeg },
    ...(geometry.maxDistanceM === undefined ? {} : { maxDistanceM: geometry.maxDistanceM }),
    groundConeShare: groundConeShare(geometry, SWEEP_FIGURE),
  };
}

/** A column's phase in the format. */
function phaseJson(asymmetry: number): SweepPhaseJson {
  const phase = sweepPhase(asymmetry);
  let json: SweepPhaseJson;
  switch (phase.kind) {
    case "rayleigh":
      json = { kind: "rayleigh" };
      break;
    case "tabulated":
      json = { kind: "tabulated", a1: Array.from(phase.table.values[0]) };
      break;
    case "none":
    case "cornette-shanks":
      throw new Error(`a sweep column's phase is Rayleigh's or a table, not ${phase.kind}`);
  }
  return json;
}

/**
 * A point of a file, at its column and spot.
 *
 * @throws RangeError for a column, τ candidate or scan ground the file does not hold.
 */
export function sweepPointOf(spec: SweepFileSpec, column: number, spot: SweepSpot): SweepPoint {
  const asymmetry = spec.asymmetries[column];
  let tau: number | undefined;
  let groundAlbedo: number | undefined;
  switch (spot.kind) {
    case "walk":
      tau = SWEEP_TAU_CANDIDATES[spot.candidateIndex]?.tau;
      groundAlbedo = spec.groundAlbedo;
      break;
    case "scan":
      tau = SWEEP_TAU_CANDIDATES[0]?.tau;
      groundAlbedo = SWEEP_SCAN_GROUND_ALBEDOS[spot.groundIndex];
      break;
  }
  if (asymmetry === undefined || tau === undefined || groundAlbedo === undefined) {
    throw new RangeError(`${spec.name} has no column ${column} at ${JSON.stringify(spot)}`);
  }
  return { singleScatteringAlbedo: spec.singleScatteringAlbedo, asymmetry, tau, groundAlbedo };
}

/**
 * A file in the format, the twin's radiance at column c and spot s from `twinAt(c, s)`.
 *
 * @throws Error for a geometry the twin would draw otherwise than its kind says, which the
 *   module's set never has.
 */
export function sweepFileJson(
  spec: SweepFileSpec,
  twinAt: (column: number, spot: SweepSpot) => readonly number[],
): SweepFileJson {
  const geometries = sweepGeometries();
  const probe = sweepMedium(sweepPointOf(spec, 0, walkSpot(0)));
  for (const g of geometries) {
    const drawn = twinGeometryKind(g, probe);
    const expected = DRAWN_AS[g.kind];
    if (drawn !== expected) {
      throw new Error(`${g.name} is a ${g.kind} geometry, which the twin draws as ${drawn}`);
    }
  }
  return {
    format: SWEEP_FORMAT,
    name: spec.name,
    description: SWEEP_DESCRIPTION,
    seed: SWEEP_CASE_SEED,
    pilotSeed: SWEEP_PILOT_SEED,
    pilotSamples: SWEEP_PILOT_SAMPLES,
    template: {
      radiusM: EARTH_REFERENCE_RADIUS_M,
      scaleHeightM: SWEEP_SCALE_HEIGHT_M,
      topHeightM: SWEEP_TOP_HEIGHT_M,
      groundAlbedo: spec.groundAlbedo,
      wavelengthNm: SWEEP_WAVELENGTH_NM,
      sunIrradiance: SWEEP_SUN_IRRADIANCE,
      detectorHalfAngleDeg: REFERENCE_DETECTOR_HALF_ANGLE_DEG,
      rayleighDepolarisation: SWEEP_RAYLEIGH_DEPOLARISATION,
    },
    singleScatteringAlbedo: spec.singleScatteringAlbedo,
    tau: SWEEP_TAU_CANDIDATES.map((c) => c.tau),
    tauRoles: SWEEP_TAU_CANDIDATES.map((c) => c.role),
    extinctionPerM: SWEEP_TAU_CANDIDATES.map((c) => sweepExtinctionPerM(c.tau)),
    phaseU: Array.from(PHASE_TABLE_U),
    geometries: geometries.map(geometryJson),
    aggregate: {
      name: SWEEP_AGGREGATE.name,
      latitudeDeg: SWEEP_AGGREGATE.latitudeDeg,
      sun: { zenithDeg: SWEEP_AGGREGATE.sun.zenithDeg, azimuthDeg: SWEEP_AGGREGATE.sun.azimuthDeg },
    },
    scanGroundAlbedos: Array.from(SWEEP_SCAN_GROUND_ALBEDOS),
    columns: spec.asymmetries.map((asymmetry, column) => ({
      asymmetry,
      phase: phaseJson(asymmetry),
      points: SWEEP_TAU_CANDIDATES.map((_, k) => (spec.firstIndices[column] ?? Number.NaN) + k),
      twin: SWEEP_TAU_CANDIDATES.map((_, k) => Array.from(twinAt(column, walkSpot(k)))),
      scanPoints: SWEEP_SCAN_GROUND_ALBEDOS.map(
        (_, a) => (spec.scanFirstIndices[column] ?? Number.NaN) + a,
      ),
      scanTwin: SWEEP_SCAN_GROUND_ALBEDOS.map((_, a) => Array.from(twinAt(column, scanSpot(a)))),
    })),
  };
}

/** The twin's radiance of a file's point at every geometry (the module's remarks). */
export function sweepTwinColumn(spec: SweepFileSpec, column: number, spot: SweepSpot): number[] {
  return sweepTwinRadiances(sweepPointOf(spec, column, spot), sweepGeometries());
}

/**
 * The smoke's sweep, which `hyperion-fit atmosphere-sweep --smoke` embeds
 * (`crates/hyperion-fit/src/atmosphere/sweep_smoke.case.json`): a cut of {@link SWEEP_SMOKE.source}
 * to two columns, its first five τ candidates (the floor and two octaves with their half-octaves)
 * with the whole floor scan, and four geometries, two ground views and two of the disc's phases,
 * which a small pilot holds without refusing (a limb's and aerial perspective's σ there is noisy,
 * and the walk would stop at its floor).
 */
export const SWEEP_SMOKE = {
  source: "ms-sweep-w900",
  name: "sweep-smoke",
  asymmetries: [0, 0.75],
  candidates: [0, 1, 2, 3, 4],
  geometries: ["ground-s30-v60-a180", "ground-s60-v60-a90", "disc-p30", "disc-p75"],
  pilotSamples: 1024,
} as const;

/** What the smoke's file says of itself. */
export const SWEEP_SMOKE_DESCRIPTION =
  "Written by apps/hyperion/src/renderer/src/view/atmosphere/reference/sweepCases.test.ts (plan R08, R08.T13.a), a cut of ms-sweep-w900.case.json for hyperion-fit atmosphere-sweep --smoke; read by crates/hyperion-fit/src/cli.rs. Do not edit by hand.";

/**
 * A sweep file cut to {@link SWEEP_SMOKE}'s columns, candidates and geometries, at its pilot count.
 *
 * @throws Error for a column or geometry the file does not hold.
 */
export function smokeSweepFile(file: SweepFileJson): SweepFileJson {
  const columns = SWEEP_SMOKE.asymmetries.map((g) => {
    const column = file.columns.find((c) => c.asymmetry === g);
    if (column === undefined) {
      throw new Error(`${file.name} has no column of g ${g}`);
    }
    return column;
  });
  const geometries = SWEEP_SMOKE.geometries.map((name) => {
    const at = file.geometries.findIndex((g) => g.name === name);
    if (at < 0) {
      throw new Error(`${file.name} has no geometry ${name}`);
    }
    return at;
  });
  const pick = <T>(values: readonly T[]): T[] =>
    SWEEP_SMOKE.candidates.map((k) => {
      const value = values[k];
      if (value === undefined) {
        throw new Error(`${file.name} has no τ candidate ${k}`);
      }
      return value;
    });
  return {
    ...file,
    name: SWEEP_SMOKE.name,
    description: SWEEP_SMOKE_DESCRIPTION,
    pilotSamples: SWEEP_SMOKE.pilotSamples,
    tau: pick(file.tau),
    tauRoles: pick(file.tauRoles),
    extinctionPerM: pick(file.extinctionPerM),
    geometries: geometries.map((at) => file.geometries[at] ?? geometryMissing(at)),
    columns: columns.map((column): SweepColumnJson => ({
      asymmetry: column.asymmetry,
      phase: column.phase,
      points: pick(column.points),
      twin: pick(column.twin).map((row) => geometries.map((at) => row[at] ?? Number.NaN)),
      scanPoints: column.scanPoints,
      scanTwin: column.scanTwin.map((row) => geometries.map((at) => row[at] ?? Number.NaN)),
    })),
  };
}

/** A geometry index past a file's, which {@link smokeSweepFile}'s lookup has ruled out. */
function geometryMissing(at: number): never {
  throw new Error(`no geometry ${at}`);
}

/** The spec of {@link SWEEP_SMOKE.source}. */
export function smokeSourceSpec(): SweepFileSpec {
  const spec = SWEEP_FILES.find((s) => s.name === SWEEP_SMOKE.source);
  if (spec === undefined) {
    throw new Error(`no sweep file is named ${SWEEP_SMOKE.source}`);
  }
  return spec;
}

/**
 * The smoke's file computed afresh: its source's file with the twin at the smoke's points alone,
 * cut.
 */
export function smokeSweepFileComputed(): SweepFileJson {
  const spec = smokeSourceSpec();
  const columns = new Set(SWEEP_SMOKE.asymmetries.map((g) => spec.asymmetries.indexOf(g)));
  const candidates = new Set<number>(SWEEP_SMOKE.candidates);
  const geometries = sweepGeometries();
  const kept = new Set<string>(SWEEP_SMOKE.geometries);
  const subset = geometries.filter((g) => kept.has(g.name));
  const full = sweepFileJson(spec, (column, spot) => {
    const wanted = spot.kind === "scan" || candidates.has(spot.candidateIndex);
    if (!columns.has(column) || !wanted) {
      return geometries.map(() => 0);
    }
    const values = sweepTwinRadiances(sweepPointOf(spec, column, spot), subset);
    return geometries.map((g) => {
      const at = subset.findIndex((s) => s.name === g.name);
      return at < 0 ? 0 : (values[at] ?? Number.NaN);
    });
  });
  return smokeSweepFile(full);
}
