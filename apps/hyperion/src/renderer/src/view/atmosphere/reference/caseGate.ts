/**
 * R08.T13.c1's case gate (plan R08, R08.T13; `decision-r08-t13-scope.md`, ruling 10, and its
 * addenda "#107's twin ground" and "#107's floor misses"): Design note 10's metric (`gate.ts`)
 * applied to the CPU twin (`twin.ts`) on the four committed references' scalar radiance, scalar
 * against scalar, with R08.T13's checks on those cases, as the record `caseGate.json` holds them.
 *
 * @remarks
 * The harness makes no runtime import outside `view/atmosphere/` (its types `Vec3` and `Rgb`
 * aside) and none from `test/`, so that R08.T14.f can run it against the vector radiance with the
 * polarisation correction: `caseTwin`, `twinValueOf`, `computeTwinValues`, `caseGateRecord` and
 * `twinValuesOfRecord`, and the checks' own quadratures, `chordTransmittance`, `legsApartOf`,
 * `chromaticSurfaceOf` and `downwellingLxOf`. It is in two parts:
 *
 * - **The twin's values** ({@link TwinValues}): every number the record reads from a twin, each
 *   rounded to {@link RECORD_SIGNIFICANT_FIGURES} significant figures as computed
 *   ({@link computeTwinValues}), or read back from a committed record
 *   ({@link twinValuesOfRecord}). Each case's twin is built once, from its own medium, figure,
 *   curves and sun (`sunIlluminanceLx` as the twin's irradiance), at `TABLE_SIZES.high`
 *   ({@link caseTwin}); `saturn-oblate` also with one slice and one band, and `earth-ozone-slab`
 *   also with R05's equal transmittance steps and with R05.T12.e's placement.
 * - **The record** ({@link caseGateRecord}): a pure function of the cases, their references and
 *   those values, so that the record computed from its own values is itself. Every verdict is the
 *   recorded values', which differ from the unrounded ones by at most 5 × 10⁻⁸ of themselves.
 *
 * The checks (R08.T13's tests, ruling 10):
 *
 * 1. the gate on every geometry, per channel, against `readingsOf(…, "scalar")`: the passes, misses
 *    and refusals by image, each image's worst |ΔL| ÷ (T + 4σ_ref) and the misses by name; and
 *    `mars`'s ground views more than {@link OFF_SUN_DEG}° from the sun on their own, for
 *    R08.T13.c2;
 * 2. `mars`'s aureole: its eight points, the blue-over-red ratio (channel 2 over channel 0) of the
 *    twin and of the reference, and where each crosses 1, beside the rovers' 7–12°;
 * 3. `saturn-oblate` with its slicing, and with one slice and one band at its limbs, where some
 *    comparison above 1 H should reach |ΔL| ≥ T + 8σ_ref;
 * 4. every case's two beam checks (Design note 10): the reference's `sunOpticalDepth` against the
 *    client's own quadrature (`references.ts`'s `clientBeam`), and the client's drawn beam (the
 *    composite's transmittance towards the sun from an observer inside the air) as a radiance
 *    under the metric (`gate.ts`'s `beamPasses`);
 * 5. `earth-ozone-slab`'s beam with the split and with equal steps, at the ground with the sun at
 *    80° and 90° and through each limb (the composite's transmittance along the line of sight
 *    against the client's own converged quadrature of the chord, the tracer giving none), and its
 *    sky there with the pieces and with R05.T12.e's placement;
 * 6. R08.T9.b's surface irradiance at Earth (direct and diffuse at a sun zenith of 0°) against
 *    `earth`'s `flux-s0` downwelling flux, to 2% by `gate.ts`'s `aggregatePasses`;
 * 7. the legs apart (#65's deferred item) on `earth` and `earth-ozone-slab`: the twin's
 *    T(u_view) T(u_sun) against the reference's T(u_view + u_sun), by the client's own curve read;
 * 8. the attribution: each geometry whose cone meets the ground, its cone share, the twin's
 *    surface share, and its verdict with the drawn surface's transmittance taken per channel in
 *    place of the mean, which sorts a miss under R08.T8.b's deferred finding.
 *
 * Every verdict is recorded, never asserted: a miss is a finding (ruling 12).
 */

import type { Vec3 } from "../../../geometry/vec3";
import type { Rgb } from "../../photometry/toneCurve";
import { type AbsorberAmount, absorberTransmittance, type AbsorberCurve } from "../absorbers";
import {
  AGGREGATE_RELATIVE_TOLERANCE,
  AGGREGATE_SIGMA_CAP,
  aggregatePasses,
  aggregateRefusal,
  BEAM_DEPTH_TOLERANCE,
  beamPasses,
  beamRadiance,
  type Channel,
  type Comparison,
  coneAverage,
  gate,
  imageMaxima,
  METRIC_FLOOR,
  METRIC_RELATIVE_TOLERANCE,
  METRIC_SIGMA_ALLOWANCE,
  METRIC_SIGMA_DIVISOR,
  metricTolerance,
  type ReferenceReading,
} from "../gate";
import { DOBSON_UNIT_PER_M2 } from "../earth";
import { TABLE_SIZES } from "../hillaire";
import { densityAt, layerEdgesM, type MediumTerm, phaseAt } from "../medium";
import {
  absorberAmountsToTop,
  absorberPressuresPa,
  curvesOpticalDepth,
  intersectsGround,
  opticalDepthToTop,
  ORACLE_STEPS,
} from "../opticalDepth";
import { NO_ABSORBER_CURVES } from "../tables";
import { spheroidOpticalDepth } from "../tablesCpu";
import {
  MARS_AUREOLE_SUN_ZENITHS_DEG,
  MARS_SUN_OFFSETS_DEG,
  type ReferenceCase,
  SATURN_LIMB_LATITUDES_DEG,
  SATURN_LIMB_SCALE_HEIGHTS,
} from "./cases";
import {
  angleBetweenDeg,
  geodeticPoint,
  localDirectionVector,
  type ReferenceGeometry,
} from "./geometries";
import {
  clientBeam,
  fluxReadingsOf,
  type RadianceKind,
  readingsOf,
  type ReferenceFile,
} from "./references";
import {
  figureGroundDistanceM,
  groundConeShare,
  referenceTwin,
  type ReferenceTwin,
  twinDatum,
  twinGeometryKind,
  type TwinGeometryKind,
  type TwinOptions,
} from "./twin";

// --- The record's constants ----------------------------------------------------------------------

/** The record's format: 1. */
export const CASE_GATE_FORMAT = 1;

/** The significant figures every number of the record, the twin's values among them, keeps: 7. */
export const RECORD_SIGNIFICANT_FIGURES = 7;

/**
 * The line ruling 5 draws near the sun, degrees: `mars`'s ground views more than 15° from the sun
 * are kept on their own for R08.T13.c2, whose ground line takes Mars classified thick where they
 * miss (`decision-r08-t13-scope.md`, "#107's floor misses", item 4; R08.T13's split note). "Within
 * 15°" is read inclusively, to 10⁻¹² in cosine, as R08.T13.a's near-sun listing reads it.
 */
export const OFF_SUN_DEG = 15;

/** The σ_ref a gate that must fail is held at: some comparison reaching |ΔL| ≥ T + 8σ_ref. */
export const MUST_FAIL_SIGMAS = 8;

/**
 * The rovers' blue aureole, its crossover from the sun, degrees: 7–12°, R08's science check's
 * reading of where blue equals red in two colour-processed press images (R08.T4.a's record):
 * PIA07997, Spirit's Pancam at Gusev on sol 489, false colour from its 750, 530 and 430 nm
 * filters, and PIA19400, Curiosity's Mastcam at Gale on sol 956, calibrated and white-balanced
 * (their captions, read by R08.T13.c1's science check; neither gives an angle). Not a measured
 * extent, and dependent on each image's white balance: recorded beside the twin's and the
 * reference's crossovers, never asserted.
 */
export const ROVER_CROSSOVER_DEG: readonly [number, number] = [7, 12];

/** The cases the record names. */
export const CASE_NAMES = {
  earth: "earth",
  slab: "earth-ozone-slab",
  mars: "mars",
  saturn: "saturn-oblate",
} as const;

/** The channels' letters: red, green and blue. */
const CHANNEL_LETTERS = ["r", "g", "b"] as const;

const CHANNELS: readonly Channel[] = [0, 1, 2];

/** One case and its committed reference. */
export interface GateCase {
  readonly referenceCase: ReferenceCase;
  readonly reference: ReferenceFile;
}

/** A number as the record keeps it: {@link RECORD_SIGNIFICANT_FIGURES} significant figures. */
export function recorded(value: number): number {
  return value === 0 || !Number.isFinite(value)
    ? value
    : Number(value.toPrecision(RECORD_SIGNIFICANT_FIGURES));
}

/** A number as the record keeps it, or `null`. */
function recordedOrNull(value: number | null): number | null {
  return value === null ? null : recorded(value);
}

/** A triple as the record keeps it. */
export function recordedRgb(value: Rgb): Rgb {
  return [recorded(value[0]), recorded(value[1]), recorded(value[2])];
}

/** A triple of `f` at each channel. */
function perChannel<T>(f: (c: Channel) => T): readonly [T, T, T] {
  return [f(0), f(1), f(2)];
}

/** A case by name, or a thrown error naming it. */
function caseNamed(cases: ReadonlyArray<GateCase>, name: string): GateCase {
  const found = cases.find((c) => c.referenceCase.name === name);
  if (found === undefined) {
    throw new Error(`the case gate has no case ${name}`);
  }
  return found;
}

/** A case's geometry by name, or a thrown error naming it. */
function geometryNamed(referenceCase: ReferenceCase, name: string): ReferenceGeometry {
  const found = referenceCase.geometries.find((g) => g.name === name);
  if (found === undefined) {
    throw new Error(`${referenceCase.name} has no geometry ${name}`);
  }
  return found;
}

// --- The geometries ------------------------------------------------------------------------------

/** A geometry's sun, body-fixed. */
function sunOf(geometry: ReferenceGeometry): Vec3 {
  return localDirectionVector(geometry.observer.latitudeDeg, geometry.sun);
}

/** A geometry's view, body-fixed: its cone's axis. */
function axisOf(geometry: ReferenceGeometry): Vec3 {
  return localDirectionVector(geometry.observer.latitudeDeg, geometry.view);
}

/** A geometry's observer, body-fixed, m. */
function observerOf(referenceCase: ReferenceCase, geometry: ReferenceGeometry): Vec3 {
  return geodeticPoint(
    twinDatum(referenceCase.figure),
    geometry.observer.latitudeDeg,
    geometry.observer.heightM,
  );
}

/** How the twin draws a geometry of a case. */
function kindOf(referenceCase: ReferenceCase, geometry: ReferenceGeometry): TwinGeometryKind {
  return twinGeometryKind(geometry, referenceCase.medium, TABLE_SIZES.high);
}

/** A geometry's observer and sun, which its beam depends on alone. */
function beamKeyOf(geometry: ReferenceGeometry): string {
  const { observer, sun } = geometry;
  return `${observer.heightM},${observer.latitudeDeg},${sun.zenithDeg},${sun.azimuthDeg}`;
}

/** The geometries of one observer and sun: its first geometry, by name, and how many share it. */
interface BeamGroup {
  readonly geometry: ReferenceGeometry;
  readonly sharedBy: number;
}

/** A case's geometries grouped by observer and sun, in the case's order, each by its first. */
function beamGroups(referenceCase: ReferenceCase): BeamGroup[] {
  const groups = new Map<string, { geometry: ReferenceGeometry; sharedBy: number }>();
  for (const geometry of referenceCase.geometries) {
    const key = beamKeyOf(geometry);
    const group = groups.get(key);
    if (group === undefined) {
      groups.set(key, { geometry, sharedBy: 1 });
    } else {
      group.sharedBy += 1;
    }
  }
  return [...groups.values()];
}

/** Whether a geometry's observer is inside the air: the drawn beam's checks hold there alone. */
function insideTheAir(referenceCase: ReferenceCase, geometry: ReferenceGeometry): boolean {
  return kindOf(referenceCase, geometry) !== "orbit";
}

/** A geometry's share of its cone that meets the ground, as recorded. */
function coneShareOf(referenceCase: ReferenceCase, geometry: ReferenceGeometry): number {
  return recorded(
    groundConeShare(geometry, referenceCase.figure, referenceCase.detectorHalfAngleDeg),
  );
}

/** `earth-ozone-slab`'s images the slab's runs take: the sun at 80° and 90°, and the limb. */
const SLAB_IMAGES = ["ground-s80", "ground-s90", "limb"] as const;

/** `saturn-oblate`'s limbs, each with its height in scale heights and its latitude. */
interface SaturnLimb {
  readonly name: string;
  readonly scaleHeights: number;
  readonly latitudeDeg: number;
}

/** `saturn-oblate`'s limbs, as `cases.ts` names them. */
function saturnLimbs(): SaturnLimb[] {
  return SATURN_LIMB_LATITUDES_DEG.flatMap((latitudeDeg) =>
    SATURN_LIMB_SCALE_HEIGHTS.map((scaleHeights) => ({
      name: `limb-lat${latitudeDeg}-${scaleHeights}H`,
      scaleHeights,
      latitudeDeg,
    })),
  );
}

// --- The twin's values ---------------------------------------------------------------------------

/** The twins a case's values are read from: its own, and the runs' other settings. */
export type TwinSetting = "case" | "oneSlice" | "equalSteps" | "placedSteps";

/** The options each setting builds its twin with, over {@link caseTwin}'s. */
export const TWIN_SETTINGS: Readonly<Record<TwinSetting, Partial<TwinOptions>>> = {
  case: {},
  oneSlice: { slicing: "single" },
  equalSteps: { transmittanceSteps: "equal" },
  placedSteps: { marchSteps: "placed" },
};

/**
 * What the record reads from a twin:
 *
 * - `radiance`: a geometry's radiance, averaged over its cone (`twin.ts`'s `radiance`);
 * - `surfaceShare`: the drawn surface's share of it, per channel, 0 where it is dark;
 * - `beam`: the composite's transmittance towards the sun from the observer (`transmittanceAlong`);
 * - `sight`: the composite's transmittance along the cone's axis from the observer;
 * - `chromaticSurface`: the drawn surface's radiance with its transmittance per channel
 *   ({@link chromaticSurfaceOf}), for the attribution;
 * - `downwelling`: the direct and diffuse irradiance on the ground under an aggregate's sun, in the
 *   case's unit, by `surfaceLighting.wgsl`'s lookups.
 */
export type TwinQuantity =
  "radiance" | "surfaceShare" | "beam" | "sight" | "chromaticSurface" | "downwelling";

/** One value the record reads from a twin. */
export interface TwinValueSpec {
  readonly quantity: TwinQuantity;
  readonly setting: TwinSetting;
  readonly caseName: string;
  /** A geometry's name, or for `downwelling` an aggregate's. */
  readonly name: string;
}

/** A twin value's key in {@link TwinValues}: `<setting>/<quantity>/<case>/<name>`. */
export function twinValueKey(spec: TwinValueSpec): string {
  return `${spec.setting}/${spec.quantity}/${spec.caseName}/${spec.name}`;
}

/** Every value the record reads from a twin, by {@link twinValueKey}, as recorded. */
export type TwinValues = ReadonlyMap<string, Rgb>;

/** The aggregate R08.T9.b's surface irradiance is held to: `earth`'s `flux-s0`. */
const IRRADIANCE_AGGREGATE = "flux-s0";

/**
 * Every value the record reads from a twin (the module's remarks), in the order computed.
 *
 * @throws Error for a case the record names but `cases` lacks, or a geometry it names but its case
 *   lacks (`saturn-oblate`'s limbs).
 */
export function twinValueSpecs(cases: ReadonlyArray<GateCase>): TwinValueSpec[] {
  const specs: TwinValueSpec[] = [];
  const add = (
    quantity: TwinQuantity,
    setting: TwinSetting,
    caseName: string,
    name: string,
  ): void => {
    specs.push({ quantity, setting, caseName, name });
  };
  for (const { referenceCase } of cases) {
    const caseName = referenceCase.name;
    for (const geometry of referenceCase.geometries) {
      add("radiance", "case", caseName, geometry.name);
      add("surfaceShare", "case", caseName, geometry.name);
    }
    for (const { geometry } of beamGroups(referenceCase)) {
      if (insideTheAir(referenceCase, geometry)) {
        add("beam", "case", caseName, geometry.name);
      }
    }
    for (const geometry of referenceCase.geometries) {
      if (
        kindOf(referenceCase, geometry) !== "ground" &&
        coneShareOf(referenceCase, geometry) > 0
      ) {
        add("chromaticSurface", "case", caseName, geometry.name);
      }
    }
  }
  const slab = caseNamed(cases, CASE_NAMES.slab).referenceCase;
  for (const geometry of slab.geometries) {
    if (geometry.image === "limb") {
      add("sight", "case", slab.name, geometry.name);
      add("sight", "equalSteps", slab.name, geometry.name);
    }
  }
  for (const { geometry } of beamGroups(slab)) {
    if (geometry.image === "ground-s80" || geometry.image === "ground-s90") {
      add("beam", "equalSteps", slab.name, geometry.name);
    }
  }
  for (const geometry of slab.geometries) {
    if (SLAB_IMAGES.some((image) => image === geometry.image)) {
      add("radiance", "placedSteps", slab.name, geometry.name);
    }
  }
  const saturn = caseNamed(cases, CASE_NAMES.saturn).referenceCase;
  for (const limb of saturnLimbs()) {
    // `geometryNamed` throws where the case lacks the limb the record names.
    add("radiance", "oneSlice", saturn.name, geometryNamed(saturn, limb.name).name);
  }
  const earth = caseNamed(cases, CASE_NAMES.earth).referenceCase;
  add("downwelling", "case", earth.name, IRRADIANCE_AGGREGATE);
  return specs;
}

/**
 * A case's twin (the module's remarks): its medium on its figure, its curves in light slot 0, its
 * sun's illuminance per channel as the twin's irradiance, its detector cone, at `TABLE_SIZES.high`,
 * under `options`.
 *
 * @throws RangeError as `twin.ts`'s `referenceTwin`.
 */
export function caseTwin(
  referenceCase: ReferenceCase,
  options: Partial<TwinOptions> = {},
): ReferenceTwin {
  return referenceTwin(referenceCase.medium, referenceCase.figure, {
    sizes: TABLE_SIZES.high,
    detectorHalfAngleDeg: referenceCase.detectorHalfAngleDeg,
    sunIrradiance: referenceCase.sunIlluminanceLx,
    curves: referenceCase.curves.length > 0 ? [referenceCase.curves] : NO_ABSORBER_CURVES,
    ...options,
  });
}

/** The Simpson intervals of the attribution's and the legs' quadratures: 2,000 a leg. */
const DIAGNOSTIC_STEPS = 2_000;

/**
 * The transmittance per channel from a ground point back along a ray to its observer, by the
 * client's own quadrature: `opticalDepth.ts`'s Simpson rule and each absorber's curve at the
 * path's own column on a sphere, `tablesCpu.ts`'s `spheroidOpticalDepth` on a spheroid; from the
 * ground to the top for an observer above it, less the observer's own leg to the top for one
 * inside the air (`inside`).
 */
function groundToObserverTransmittance(
  referenceCase: ReferenceCase,
  observerM: Vec3,
  groundM: Vec3,
  direction: Vec3,
  inside: boolean,
): Rgb {
  const { medium, figure } = referenceCase;
  const back = { x: -direction.x, y: -direction.y, z: -direction.z };
  let opticalDepth: Rgb;
  switch (figure.kind) {
    case "sphere": {
      const radiusM = figure.radiusM;
      const legOf = (pointM: Vec3): { depth: Rgb; amounts: ReadonlyArray<AbsorberAmount> } => {
        const r = Math.hypot(pointM.x, pointM.y, pointM.z);
        const mu = (pointM.x * back.x + pointM.y * back.y + pointM.z * back.z) / r;
        return {
          depth: opticalDepthToTop(medium, radiusM, r, mu, DIAGNOSTIC_STEPS),
          amounts: absorberAmountsToTop(medium, radiusM, r, mu, DIAGNOSTIC_STEPS),
        };
      };
      const ground = legOf(groundM);
      const observer = inside ? legOf(observerM) : null;
      const depth = perChannel((c) => ground.depth[c] - (observer?.depth[c] ?? 0));
      const amounts = ground.amounts.map((a, k) => {
        const less = observer?.amounts[k];
        return less === undefined ? a : amountLess(a, less);
      });
      const absorbed: Rgb =
        referenceCase.curves.length > 0
          ? curvesOpticalDepth(
              referenceCase.curves,
              amounts,
              amounts.map(() => undefined),
            )
          : [0, 0, 0];
      opticalDepth = perChannel((c) => depth[c] + absorbed[c]);
      break;
    }
    case "spheroid": {
      const body = { figure: figure.gravity.spheroid, gravity: figure.gravity };
      const ground = spheroidOpticalDepth(medium, body, groundM, back, DIAGNOSTIC_PIECE_M);
      const observer = inside
        ? spheroidOpticalDepth(medium, body, observerM, back, DIAGNOSTIC_PIECE_M)
        : null;
      opticalDepth = perChannel((c) => ground.opticalDepth[c] - (observer?.opticalDepth[c] ?? 0));
      break;
    }
  }
  return perChannel((c) => Math.exp(-opticalDepth[c]));
}

/** The pieces of `spheroidOpticalDepth` in the attribution's quadrature, m: 1,000. */
const DIAGNOSTIC_PIECE_M = 1_000;

/** An absorber amount less another of its kind. */
function amountLess(a: AbsorberAmount, b: AbsorberAmount): AbsorberAmount {
  return amountsCombined(a, b, (x, y) => Math.max(x - y, 0));
}

/** Two absorber amounts of one kind summed. */
function amountPlus(a: AbsorberAmount, b: AbsorberAmount): AbsorberAmount {
  return amountsCombined(a, b, (x, y) => x + y);
}

/**
 * Two absorber amounts of one kind, their values combined by `combine`, in that kind's unit.
 *
 * @throws Error for amounts of two kinds.
 */
function amountsCombined(
  a: AbsorberAmount,
  b: AbsorberAmount,
  combine: (x: number, y: number) => number,
): AbsorberAmount {
  let amount: AbsorberAmount;
  switch (a.kind) {
    case "column":
      if (b.kind !== "column") {
        throw new Error("absorber amounts of two kinds");
      }
      amount = { kind: "column", columnPerM2: combine(a.columnPerM2, b.columnPerM2) };
      break;
    case "pair":
      if (b.kind !== "pair") {
        throw new Error("absorber amounts of two kinds");
      }
      amount = { kind: "pair", pairColumnPerM5: combine(a.pairColumnPerM5, b.pairColumnPerM5) };
      break;
  }
  return amount;
}

/**
 * The drawn surface's radiance at a geometry with its transmittance per channel (the attribution):
 * the cone's average, by the twin's own weights, of the lit ground's radiance (`twin.ts`'s
 * `litGroundRadiance`) times {@link groundToObserverTransmittance} along each direction that meets
 * the ground within the view, times the sun's irradiance; the twin's own takes the march's or the
 * volume's mean over the channels there (`composite.wgsl`'s `colour * marched.a`).
 */
export function chromaticSurfaceOf(
  twin: ReferenceTwin,
  referenceCase: ReferenceCase,
  geometry: ReferenceGeometry,
): Rgb {
  const originM = observerOf(referenceCase, geometry);
  const sun = sunOf(geometry);
  const limitM = geometry.maxDistanceM ?? Number.POSITIVE_INFINITY;
  const inside = insideTheAir(referenceCase, geometry);
  const unit = coneAverage(
    (direction) => {
      const groundM = figureGroundDistanceM(originM, direction, referenceCase.figure);
      if (groundM === null || !(groundM < limitM)) {
        return [0, 0, 0];
      }
      const pointM = {
        x: originM.x + groundM * direction.x,
        y: originM.y + groundM * direction.y,
        z: originM.z + groundM * direction.z,
      };
      const lit = twin.litGroundRadiance(pointM, sun);
      const t = groundToObserverTransmittance(referenceCase, originM, pointM, direction, inside);
      return [lit[0] * t[0], lit[1] * t[1], lit[2] * t[2]];
    },
    axisOf(geometry),
    ((geometry.detectorHalfAngleDeg ?? referenceCase.detectorHalfAngleDeg) * Math.PI) / 180,
  );
  const e = referenceCase.sunIlluminanceLx;
  return [unit[0] * e[0], unit[1] * e[1], unit[2] * e[2]];
}

/**
 * The direct and diffuse irradiance on the ground under an aggregate's sun, in the case's unit:
 * E_c (μ₀ T_c(0, μ₀) + E_sky,c(0, μ₀)) by the twin's `surfaceLighting.wgsl` lookups (R08.T9.b).
 *
 * @throws Error for an aggregate the case does not have.
 */
export function downwellingLxOf(
  twin: ReferenceTwin,
  referenceCase: ReferenceCase,
  name: string,
): Rgb {
  const aggregate = referenceCase.aggregates.find((a) => a.name === name);
  if (aggregate === undefined) {
    throw new Error(`${referenceCase.name} has no aggregate ${name}`);
  }
  const mu = Math.cos((aggregate.sun.zenithDeg * Math.PI) / 180);
  const latitudeRad = (aggregate.latitudeDeg * Math.PI) / 180;
  const azimuthRad = (aggregate.sun.azimuthDeg * Math.PI) / 180;
  const direct = twin.surfaceLighting.sunTransmittance(0, mu, latitudeRad, azimuthRad, 0);
  const sky = twin.surfaceLighting.skyIrradiance(0, mu, latitudeRad, 0);
  const e = referenceCase.sunIlluminanceLx;
  return perChannel((c) => e[c] * (Math.max(mu, 0) * direct[c] + sky[c]));
}

/**
 * One value of a twin (the module's remarks), as computed and before it is recorded.
 *
 * @throws Error for a geometry or aggregate the case does not have.
 */
export function twinValueOf(
  twin: ReferenceTwin,
  referenceCase: ReferenceCase,
  spec: TwinValueSpec,
): Rgb {
  if (spec.quantity === "downwelling") {
    return downwellingLxOf(twin, referenceCase, spec.name);
  }
  const geometry = geometryNamed(referenceCase, spec.name);
  let value: Rgb;
  switch (spec.quantity) {
    case "radiance":
      value = twin.radiance(geometry);
      break;
    case "surfaceShare": {
      const total = twin.radiance(geometry);
      const surface = twin.surfaceRadiance(geometry);
      value = perChannel((c) => (surface[c] === 0 ? 0 : surface[c] / total[c]));
      break;
    }
    case "beam":
      value = twin.transmittanceAlong(geometry, sunOf(geometry));
      break;
    case "sight":
      value = twin.transmittanceAlong(geometry, axisOf(geometry));
      break;
    case "chromaticSurface":
      value = chromaticSurfaceOf(twin, referenceCase, geometry);
      break;
  }
  return value;
}

/**
 * The values of `specs`, each recorded, every twin built once a case and setting
 * ({@link caseTwin} under {@link TWIN_SETTINGS}).
 *
 * @throws Error for a spec whose case `cases` lacks; RangeError as `referenceTwin`.
 */
export function computeTwinValues(
  cases: ReadonlyArray<GateCase>,
  specs: ReadonlyArray<TwinValueSpec> = twinValueSpecs(cases),
): Map<string, Rgb> {
  const twins = new Map<string, ReferenceTwin>();
  const values = new Map<string, Rgb>();
  for (const spec of specs) {
    const { referenceCase } = caseNamed(cases, spec.caseName);
    const twinKey = `${spec.setting}/${spec.caseName}`;
    let twin = twins.get(twinKey);
    if (twin === undefined) {
      twin = caseTwin(referenceCase, TWIN_SETTINGS[spec.setting]);
      twins.set(twinKey, twin);
    }
    values.set(twinValueKey(spec), recordedRgb(twinValueOf(twin, referenceCase, spec)));
  }
  return values;
}

/**
 * The keys of `values` that break the gate (ruling 12's blockers): a twin value that is not finite
 * or is negative, each with the channels that are.
 */
export function blockingValues(values: TwinValues): string[] {
  const blocking: string[] = [];
  for (const [key, value] of values) {
    const bad = CHANNELS.filter((c) => !(Number.isFinite(value[c]) && value[c] >= 0));
    if (bad.length > 0) {
      blocking.push(`${key}/${bad.map((c) => CHANNEL_LETTERS[c]).join("")}`);
    }
  }
  return blocking;
}

/**
 * Each case's geometries that its reference does not trace, or traces twice, as `case/name`: the
 * record's rows are the reference's geometries, so none is left out where this is empty.
 */
export function uncoveredGeometries(cases: ReadonlyArray<GateCase>): string[] {
  return cases.flatMap(({ referenceCase, reference }) =>
    referenceCase.geometries
      .filter((g) => reference.geometries.filter((r) => r.name === g.name).length !== 1)
      .map((g) => `${referenceCase.name}/${g.name}`),
  );
}

/** The specs whose values `values` lacks. */
export function missingTwinValues(
  values: TwinValues,
  specs: ReadonlyArray<TwinValueSpec>,
): string[] {
  return specs.map(twinValueKey).filter((key) => !values.has(key));
}

/** A recorded twin value, or a thrown error naming it. */
function valueOf(values: TwinValues, spec: TwinValueSpec): Rgb {
  const key = twinValueKey(spec);
  const value = values.get(key);
  if (value === undefined) {
    throw new Error(`the case gate has no twin value ${key}`);
  }
  return value;
}

// --- The record ----------------------------------------------------------------------------------

/** A value of the record: JSON, as `test/bless.ts`'s `tableText` writes it. */
export type RecordValue =
  | null
  | boolean
  | number
  | string
  | ReadonlyArray<RecordValue>
  | { readonly [key: string]: RecordValue };

/** A verdict of one comparison. */
type Verdict = "pass" | "miss" | "refused";

/** A comparison's verdict. */
function verdictOf(comparison: Comparison): Verdict {
  if (comparison.refused) {
    return "refused";
  }
  return comparison.passes ? "pass" : "miss";
}

/** |ΔL| ÷ (T + 4σ_ref), as recorded, or `null` for a bound of 0. */
function boundRatio(comparison: Comparison): number | null {
  const delta = Math.abs(comparison.client - comparison.reference);
  return comparison.bound > 0 ? recorded(delta / comparison.bound) : null;
}

/** The row columns of each table of the record, in order. */
export const RECORD_COLUMNS = {
  geometry: [
    "name",
    "image",
    "kind",
    "coneShare",
    "twinR",
    "twinG",
    "twinB",
    "surfaceShareR",
    "surfaceShareG",
    "surfaceShareB",
    "referenceR",
    "referenceG",
    "referenceB",
    "sigmaR",
    "sigmaG",
    "sigmaB",
    "toleranceR",
    "toleranceG",
    "toleranceB",
    "verdictR",
    "verdictG",
    "verdictB",
    "ratioR",
    "ratioG",
    "ratioB",
  ],
  image: [
    "image",
    "comparisons",
    "passes",
    "misses",
    "refusals",
    "worstRatio",
    "worstName",
    "worstChannel",
  ],
  offSun: [
    "name",
    "sunDistanceDeg",
    "verdictR",
    "verdictG",
    "verdictB",
    "ratioR",
    "ratioG",
    "ratioB",
  ],
  aureole: [
    "name",
    "sunZenithDeg",
    "offsetDeg",
    "twinBlueOverRed",
    "referenceBlueOverRed",
    "verdictR",
    "verdictG",
    "verdictB",
    "ratioR",
    "ratioG",
    "ratioB",
  ],
  crossover: [
    "sunZenithDeg",
    "twinDeg",
    "referenceDeg",
    "twinDegAtSunColour",
    "referenceDegAtSunColour",
  ],
  oneSlice: [
    "name",
    "scaleHeights",
    "latitudeDeg",
    "twinR",
    "twinG",
    "twinB",
    "deltaOverSigmaR",
    "deltaOverSigmaG",
    "deltaOverSigmaB",
    "excessOverSigmaR",
    "excessOverSigmaG",
    "excessOverSigmaB",
    "reachesR",
    "reachesG",
    "reachesB",
  ],
  beam: [
    "name",
    "image",
    "kind",
    "sharedBy",
    "tauReferenceR",
    "tauReferenceG",
    "tauReferenceB",
    "depthRatio",
    "depthAgrees",
    "clientR",
    "clientG",
    "clientB",
    "ratioR",
    "ratioG",
    "ratioB",
    "verdictR",
    "verdictG",
    "verdictB",
  ],
  slabBeam: [
    "name",
    "path",
    "steps",
    "clientR",
    "clientG",
    "clientB",
    "referenceR",
    "referenceG",
    "referenceB",
    "ratioR",
    "ratioG",
    "ratioB",
    "verdictR",
    "verdictG",
    "verdictB",
  ],
  slabSky: [
    "name",
    "steps",
    "twinR",
    "twinG",
    "twinB",
    "ratioR",
    "ratioG",
    "ratioB",
    "verdictR",
    "verdictG",
    "verdictB",
  ],
  irradiance: ["channel", "twin", "reference", "sigma", "twinOverReferenceLessOne", "verdict"],
  legs: [
    "name",
    "productOverJointLessOneR",
    "productOverJointLessOneG",
    "productOverJointLessOneB",
  ],
  attribution: [
    "name",
    "kind",
    "coneShare",
    "surfaceShareR",
    "surfaceShareG",
    "surfaceShareB",
    "verdictR",
    "verdictG",
    "verdictB",
    "perChannelVerdictR",
    "perChannelVerdictG",
    "perChannelVerdictB",
    "sort",
  ],
  twin: ["key", "r", "g", "b"],
} as const;

/** One case's gate: its comparisons by geometry, in the reference's order. */
interface CaseGate {
  readonly gateCase: GateCase;
  readonly readings: ReadonlyArray<ReferenceReading>;
  readonly maxima: ReadonlyMap<string, Rgb>;
  readonly byName: ReadonlyMap<string, readonly [Comparison, Comparison, Comparison]>;
}

/** The gate of a case's readings of `kind` against its own twin's recorded radiance. */
function caseGateOf(gateCase: GateCase, values: TwinValues, kind: RadianceKind): CaseGate {
  const { referenceCase, reference } = gateCase;
  const readings = readingsOf(referenceCase, reference, kind);
  const client = new Map(
    readings.map((r) => [
      r.name,
      valueOf(values, {
        quantity: "radiance",
        setting: "case",
        caseName: referenceCase.name,
        name: r.name,
      }),
    ]),
  );
  const report = gate(readings, client);
  return {
    gateCase,
    readings,
    maxima: imageMaxima(readings),
    byName: comparisonsByName(report.comparisons),
  };
}

/** A gate's comparisons by geometry, the three channels in order. */
function comparisonsByName(
  comparisons: ReadonlyArray<Comparison>,
): Map<string, readonly [Comparison, Comparison, Comparison]> {
  const grouped = new Map<string, Comparison[]>();
  for (const c of comparisons) {
    grouped.set(c.name, [...(grouped.get(c.name) ?? []), c]);
  }
  const out = new Map<string, readonly [Comparison, Comparison, Comparison]>();
  for (const [name, list] of grouped) {
    const [r, g, b] = list;
    if (list.length !== 3 || r === undefined || g === undefined || b === undefined) {
      throw new Error(`${name} has ${list.length} comparisons, not three channels'`);
    }
    out.set(name, [r, g, b]);
  }
  return out;
}

/** A geometry's three comparisons in a gate, or a thrown error naming it. */
function comparisonsOf(
  caseGate: CaseGate,
  name: string,
): readonly [Comparison, Comparison, Comparison] {
  const found = caseGate.byName.get(name);
  if (found === undefined) {
    throw new Error(`${caseGate.gateCase.referenceCase.name}'s gate has no geometry ${name}`);
  }
  return found;
}

/** A comparison's channels as the record names them: `name/rgb`. */
function channelsLabel(name: string, channels: ReadonlyArray<Channel>): string {
  return `${name}/${channels.map((c) => CHANNEL_LETTERS[c]).join("")}`;
}

/** A gate's verdicts and ratios over a list of geometries: counts, the worst and the misses. */
interface Tally {
  readonly comparisons: number;
  readonly passes: number;
  readonly misses: number;
  readonly refusals: number;
  /** The largest |ΔL| ÷ (T + 4σ_ref) of a comparison not refused, and where. */
  readonly worstRatio: number | null;
  readonly worstName: string | null;
  readonly worstChannel: string | null;
  /** The misses, `name/channels`. */
  readonly missed: ReadonlyArray<string>;
}

/** {@link Tally} over `names`' comparisons in `caseGate`. */
function tallyOf(caseGate: CaseGate, names: ReadonlyArray<string>): Tally {
  let passes = 0;
  let misses = 0;
  let refusals = 0;
  let worstRatio: number | null = null;
  let worstName: string | null = null;
  let worstChannel: string | null = null;
  const missed: string[] = [];
  for (const name of names) {
    const comparisons = comparisonsOf(caseGate, name);
    const missedHere: Channel[] = [];
    for (const c of CHANNELS) {
      const comparison = comparisons[c];
      const verdict = verdictOf(comparison);
      if (verdict === "refused") {
        refusals += 1;
        continue;
      }
      if (verdict === "pass") {
        passes += 1;
      } else {
        misses += 1;
        missedHere.push(c);
      }
      const ratio = boundRatio(comparison);
      if (ratio !== null && (worstRatio === null || ratio > worstRatio)) {
        worstRatio = ratio;
        worstName = name;
        worstChannel = CHANNEL_LETTERS[c];
      }
    }
    if (missedHere.length > 0) {
      missed.push(channelsLabel(name, missedHere));
    }
  }
  return {
    comparisons: names.length * 3,
    passes,
    misses,
    refusals,
    worstRatio,
    worstName,
    worstChannel,
    missed,
  };
}

/** A tally as the record writes it. */
function tallyRecord(tally: Tally): RecordValue {
  return {
    comparisons: tally.comparisons,
    passes: tally.passes,
    misses: tally.misses,
    refusals: tally.refusals,
    worstRatio: tally.worstRatio,
    worstAt:
      tally.worstName === null || tally.worstChannel === null
        ? null
        : `${tally.worstName}/${tally.worstChannel}`,
    missed: [...tally.missed],
  };
}

/** One case's section: its headline, its images' tallies, its misses and every geometry's row. */
function caseSection(caseGate: CaseGate, values: TwinValues): RecordValue {
  const { referenceCase } = caseGate.gateCase;
  const caseName = referenceCase.name;
  const images = [...new Set(caseGate.readings.map((r) => r.image))];
  const imageRows = images.map((image) => {
    const names = caseGate.readings.filter((r) => r.image === image).map((r) => r.name);
    const tally = tallyOf(caseGate, names);
    return [
      image,
      tally.comparisons,
      tally.passes,
      tally.misses,
      tally.refusals,
      tally.worstRatio,
      tally.worstName,
      tally.worstChannel,
    ];
  });
  const all = tallyOf(
    caseGate,
    caseGate.readings.map((r) => r.name),
  );
  const rows = caseGate.readings.map((reading) => {
    const geometry = geometryNamed(referenceCase, reading.name);
    const comparisons = comparisonsOf(caseGate, reading.name);
    const twin = valueOf(values, {
      quantity: "radiance",
      setting: "case",
      caseName,
      name: reading.name,
    });
    const share = valueOf(values, {
      quantity: "surfaceShare",
      setting: "case",
      caseName,
      name: reading.name,
    });
    return [
      reading.name,
      reading.image,
      kindOf(referenceCase, geometry),
      coneShareOf(referenceCase, geometry),
      ...twin,
      ...share,
      ...recordedRgb(reading.radiance),
      ...recordedRgb(reading.standardError),
      ...perChannel((c) => recorded(comparisons[c].tolerance)),
      ...perChannel((c) => verdictOf(comparisons[c])),
      ...perChannel((c) => boundRatio(comparisons[c])),
    ];
  });
  return {
    headline: all.misses === 0 ? "pass" : "miss",
    all: tallyRecord(all),
    images: imageRows,
    geometries: rows,
  };
}

/** Whether a ground view is more than {@link OFF_SUN_DEG}° from its sun, to 10⁻¹² in cosine. */
function offTheSun(geometry: ReferenceGeometry): boolean {
  const cos = Math.cos((angleBetweenDeg(geometry.view, geometry.sun) * Math.PI) / 180);
  return cos < Math.cos((OFF_SUN_DEG * Math.PI) / 180) - 1e-12;
}

/** `mars`'s ground views away from the sun (check 1, {@link OFF_SUN_DEG}): kept for c2. */
function marsOffSunSection(caseGate: CaseGate): RecordValue {
  const { referenceCase } = caseGate.gateCase;
  const ground = referenceCase.geometries.filter((g) => kindOf(referenceCase, g) === "ground");
  const off = ground.filter(offTheSun);
  const near = ground.filter((g) => !offTheSun(g)).map((g) => g.name);
  const tally = tallyOf(
    caseGate,
    off.map((g) => g.name),
  );
  return {
    views: off.length,
    verdict: tally.misses === 0 ? "pass" : "miss",
    tally: tallyRecord(tally),
    nearTheSun: near,
    rows: off.map((g) => offSunRow(caseGate, g)),
  };
}

/** One of `mars`'s ground views away from the sun as the record writes it. */
function offSunRow(caseGate: CaseGate, geometry: ReferenceGeometry): RecordValue {
  const comparisons = comparisonsOf(caseGate, geometry.name);
  const verdicts = perChannel((c) => verdictOf(comparisons[c]));
  const ratios = perChannel((c) => boundRatio(comparisons[c]));
  return [
    geometry.name,
    recorded(angleBetweenDeg(geometry.view, geometry.sun)),
    verdicts[0],
    verdicts[1],
    verdicts[2],
    ratios[0],
    ratios[1],
    ratios[2],
  ];
}

/**
 * Where a ratio crosses `level` over ascending angles, linear in angle between the bracketing
 * points, or `null` where it does not in their range.
 *
 * @param level - The ratio crossed: 1 by default.
 */
export function crossoverDeg(
  anglesDeg: ReadonlyArray<number>,
  ratios: ReadonlyArray<number>,
  level = 1,
): number | null {
  for (let i = 0; i + 1 < anglesDeg.length; i += 1) {
    const a = anglesDeg[i] ?? Number.NaN;
    const b = anglesDeg[i + 1] ?? Number.NaN;
    const ra = ratios[i] ?? Number.NaN;
    const rb = ratios[i + 1] ?? Number.NaN;
    if (ra === level) {
      return a;
    }
    if ((ra - level) * (rb - level) < 0) {
      return a + ((level - ra) * (b - a)) / (rb - ra);
    }
  }
  const last = ratios.at(-1);
  return last === level ? (anglesDeg.at(-1) ?? null) : null;
}

/** A radiance's blue over its red: channel 2 over channel 0. */
function blueOverRed(rgb: Rgb): number {
  return rgb[2] / rgb[0];
}

/**
 * `mars`'s aureole (check 2). Each crossover is taken where blue over red crosses 1, as R08.T13
 * asks, and where it crosses the case's Sun's own blue over red, 0.861: where the sky turns
 * bluer than the sunlight, the like for like of an image white-balanced to the Sun, as the rovers'
 * are read (`ROVER_CROSSOVER_DEG`).
 */
function marsAureoleSection(caseGate: CaseGate, values: TwinValues): RecordValue {
  const { referenceCase } = caseGate.gateCase;
  const caseName = referenceCase.name;
  const sunBlueOverRed = blueOverRed(referenceCase.sunIlluminanceLx);
  const rows: RecordValue[] = [];
  const crossovers: RecordValue[] = [];
  for (const sunZenith of MARS_AUREOLE_SUN_ZENITHS_DEG) {
    const twinRatios: number[] = [];
    const referenceRatios: number[] = [];
    for (const offset of MARS_SUN_OFFSETS_DEG) {
      const name = `aureole-s${sunZenith}-d${offset}`;
      const comparisons = comparisonsOf(caseGate, name);
      const twin = valueOf(values, { quantity: "radiance", setting: "case", caseName, name });
      const reference = perChannel((c) => comparisons[c].reference);
      twinRatios.push(blueOverRed(twin));
      referenceRatios.push(blueOverRed(reference));
      rows.push([
        name,
        sunZenith,
        offset,
        recorded(blueOverRed(twin)),
        recorded(blueOverRed(reference)),
        ...perChannel((c) => verdictOf(comparisons[c])),
        ...perChannel((c) => boundRatio(comparisons[c])),
      ]);
    }
    crossovers.push([
      sunZenith,
      recordedOrNull(crossoverDeg(MARS_SUN_OFFSETS_DEG, twinRatios)),
      recordedOrNull(crossoverDeg(MARS_SUN_OFFSETS_DEG, referenceRatios)),
      recordedOrNull(crossoverDeg(MARS_SUN_OFFSETS_DEG, twinRatios, sunBlueOverRed)),
      recordedOrNull(crossoverDeg(MARS_SUN_OFFSETS_DEG, referenceRatios, sunBlueOverRed)),
    ]);
  }
  const tally = tallyOf(
    caseGate,
    MARS_AUREOLE_SUN_ZENITHS_DEG.flatMap((z) =>
      MARS_SUN_OFFSETS_DEG.map((offset) => `aureole-s${z}-d${offset}`),
    ),
  );
  return {
    verdict: tally.misses === 0 ? "pass" : "miss",
    tally: tallyRecord(tally),
    rows,
    sunBlueOverRed: recorded(sunBlueOverRed),
    crossovers,
    rovers: {
      deg: [...ROVER_CROSSOVER_DEG],
      source:
        "R08's science check's reading of where blue equals red in two colour-processed press images (PIA07997, Pancam false colour at 750, 530 and 430 nm; PIA19400, Mastcam calibrated and white-balanced): not a measured extent, recorded only",
    },
  };
}

/** `saturn-oblate`'s oblateness (check 3). */
function saturnSection(caseGate: CaseGate, values: TwinValues): RecordValue {
  const { referenceCase } = caseGate.gateCase;
  const sliced = tallyOf(
    caseGate,
    caseGate.readings.map((r) => r.name),
  );
  let reachesAbove1H = false;
  let reachesFrom1H = false;
  let largestAbove1H: { value: number; at: string } | null = null;
  let largestFrom1H: { value: number; at: string } | null = null;
  const rows = saturnLimbs().map((limb) => {
    const reading = caseGate.readings.find((r) => r.name === limb.name);
    if (reading === undefined) {
      throw new Error(`${referenceCase.name}'s reference has no ${limb.name}`);
    }
    const comparisons = comparisonsOf(caseGate, limb.name);
    const twin = valueOf(values, {
      quantity: "radiance",
      setting: "oneSlice",
      caseName: referenceCase.name,
      name: limb.name,
    });
    const delta = perChannel((c) => Math.abs(twin[c] - reading.radiance[c]));
    const sigma = reading.standardError;
    const tolerance = perChannel((c) => comparisons[c].tolerance);
    const reaches = perChannel(
      (c) => !comparisons[c].refused && delta[c] >= tolerance[c] + MUST_FAIL_SIGMAS * sigma[c],
    );
    for (const c of CHANNELS) {
      if (comparisons[c].refused) {
        continue;
      }
      const value = delta[c] / sigma[c];
      const at = channelsLabel(limb.name, [c]);
      if (limb.scaleHeights > 1) {
        reachesAbove1H ||= reaches[c];
        if (largestAbove1H === null || value > largestAbove1H.value) {
          largestAbove1H = { value, at };
        }
      }
      if (limb.scaleHeights >= 1) {
        reachesFrom1H ||= reaches[c];
        if (largestFrom1H === null || value > largestFrom1H.value) {
          largestFrom1H = { value, at };
        }
      }
    }
    const overSigma = perChannel((c) => recorded(delta[c] / sigma[c]));
    const excess = perChannel((c) => recorded((delta[c] - tolerance[c]) / sigma[c]));
    const row: RecordValue[] = [limb.name, limb.scaleHeights, limb.latitudeDeg];
    row.push(...twin, ...overSigma, ...excess, ...reaches);
    return row;
  });
  const largest = (found: { value: number; at: string } | null): RecordValue =>
    found === null ? null : { deltaOverSigma: recorded(found.value), at: found.at };
  return {
    sliced: { verdict: sliced.misses === 0 ? "pass" : "miss", tally: tallyRecord(sliced) },
    oneSliceOneBand: {
      reachesAbove1H,
      reachesFrom1H,
      largestAbove1H: largest(largestAbove1H),
      largestFrom1H: largest(largestFrom1H),
      rows,
    },
  };
}

/**
 * The reference's `sunOpticalDepth` of a geometry, per channel, `null` where the ground hides the
 * sun.
 */
function referenceDepthOf(
  reference: ReferenceFile,
  name: string,
): readonly [number | null, number | null, number | null] {
  const entry = reference.geometries.find((g) => g.name === name);
  const depths = entry?.sunOpticalDepth[0];
  if (depths === undefined) {
    throw new Error(`${reference.case}'s reference has no beam at ${name}`);
  }
  return [depths[0] ?? null, depths[1] ?? null, depths[2] ?? null];
}

/** The drawn beam's comparison at one channel: the client's and the reference's radiance. */
interface BeamComparison {
  readonly ratio: number | null;
  readonly verdict: "pass" | "miss" | "hidden" | "shown";
}

/**
 * The client's beam against the reference's in one channel (Design note 10): as radiances E_c T ÷ π
 * under the metric's T at the image's L_max (`beamPasses`); `hidden` where both hide the sun,
 * `shown` where the reference's ground hides it and the client's does not.
 */
function beamComparison(
  irradiance: number,
  clientTransmittance: number,
  referenceDepth: number | null,
  imageMaximum: number,
): BeamComparison {
  if (referenceDepth === null) {
    return { ratio: null, verdict: clientTransmittance === 0 ? "hidden" : "shown" };
  }
  const client = (irradiance * clientTransmittance) / Math.PI;
  const reference = beamRadiance(irradiance, referenceDepth);
  const tolerance = metricTolerance(reference, imageMaximum);
  return {
    ratio: tolerance > 0 ? recorded(Math.abs(client - reference) / tolerance) : null,
    verdict: beamPasses(client, reference, imageMaximum) ? "pass" : "miss",
  };
}

/** The largest of some ratios and where, as the record writes it. */
interface Worst {
  readonly ratio: number;
  readonly at: string;
}

/** The larger of a worst so far and a candidate. */
function worseOf(worst: Worst | null, ratio: number | null, at: string): Worst | null {
  return ratio !== null && (worst === null || ratio > worst.ratio) ? { ratio, at } : worst;
}

/** A worst as the record writes it. */
function worstRecord(worst: Worst | null): RecordValue {
  return worst === null ? null : { ratio: recorded(worst.ratio), at: worst.at };
}

/** One observer and sun's two beam checks (check 4). */
interface BeamRow {
  readonly geometry: ReferenceGeometry;
  readonly sharedBy: number;
  readonly referenceDepth: readonly [number | null, number | null, number | null];
  /** max over the channels of |Δτ| ÷ (10⁻⁶ max(1, τ)), or `null` where one hides the sun alone. */
  readonly depthRatio: number | null;
  readonly depthAgrees: boolean;
  /** The drawn beam's transmittance and comparisons, inside the air alone. */
  readonly drawn: { readonly client: Rgb; readonly compared: readonly BeamComparison[] } | null;
}

/** A case's beam checks at each observer and sun, in the case's order. */
function beamRowsOf(caseGate: CaseGate, values: TwinValues): BeamRow[] {
  const { referenceCase, reference } = caseGate.gateCase;
  return beamGroups(referenceCase).map(({ geometry, sharedBy }) => {
    const referenceDepth = referenceDepthOf(reference, geometry.name);
    const quadrature = clientBeam(referenceCase, geometry);
    const ratios = perChannel((c): number | null => {
      const ref = referenceDepth[c];
      const mine = quadrature?.opticalDepth[c];
      if (ref === null || mine === undefined) {
        return ref === null && mine === undefined ? 0 : null;
      }
      return Math.abs(ref - mine) / (BEAM_DEPTH_TOLERANCE * Math.max(1, ref));
    });
    const known = ratios.filter((r): r is number => r !== null);
    const depthRatio = known.length === 3 ? Math.max(...known) : null;
    let drawn: BeamRow["drawn"] = null;
    if (insideTheAir(referenceCase, geometry)) {
      const client = valueOf(values, {
        quantity: "beam",
        setting: "case",
        caseName: referenceCase.name,
        name: geometry.name,
      });
      const maximum = caseGate.maxima.get(geometry.image) ?? [0, 0, 0];
      const compared = perChannel((c) =>
        beamComparison(referenceCase.sunIlluminanceLx[c], client[c], referenceDepth[c], maximum[c]),
      );
      drawn = { client, compared };
    }
    return {
      geometry,
      sharedBy,
      referenceDepth,
      depthRatio,
      depthAgrees: depthRatio !== null && depthRatio <= 1,
      drawn,
    };
  });
}

/** A case's beams (check 4): its two checks at each observer and sun, and its worst ratios. */
function beamSection(caseGate: CaseGate, values: TwinValues): RecordValue {
  const { referenceCase } = caseGate.gateCase;
  const beamRows = beamRowsOf(caseGate, values);
  let worstDepth: Worst | null = null;
  let worstBeam: Worst | null = null;
  for (const row of beamRows) {
    worstDepth = worseOf(worstDepth, row.depthRatio, row.geometry.name);
    for (const [c, compared] of (row.drawn?.compared ?? []).entries()) {
      worstBeam = worseOf(
        worstBeam,
        compared.ratio,
        `${row.geometry.name}/${CHANNEL_LETTERS[c] ?? "?"}`,
      );
    }
  }
  const rowOf = (row: BeamRow): RecordValue => [
    row.geometry.name,
    row.geometry.image,
    kindOf(referenceCase, row.geometry),
    row.sharedBy,
    ...row.referenceDepth.map((d) => (d === null ? null : recorded(d))),
    row.depthRatio === null ? null : recorded(row.depthRatio),
    row.depthAgrees,
    ...(row.drawn?.client ?? [null, null, null]),
    ...(row.drawn?.compared.map((b) => b.ratio) ?? [null, null, null]),
    ...(row.drawn?.compared.map((b) => b.verdict) ?? [null, null, null]),
  ];
  const horizon = beamRows.filter((row) => row.geometry.image === "ground-s90");
  return {
    depthsAgree: beamRows.every((row) => row.depthAgrees),
    worstDepthRatio: worstRecord(worstDepth),
    worstBeamRatio: worstRecord(worstBeam),
    sphereHorizon: referenceCase.figure.kind === "sphere" ? horizon.map(rowOf) : null,
    rows: beamRows.map(rowOf),
  };
}

/** The tangent height of a limb's axis over a sphere, m. */
function tangentHeightM(referenceCase: ReferenceCase, geometry: ReferenceGeometry): number {
  const { figure } = referenceCase;
  if (figure.kind !== "sphere") {
    throw new RangeError("the slab's chord is taken on a sphere");
  }
  const o = observerOf(referenceCase, geometry);
  const d = axisOf(geometry);
  const along = o.x * d.x + o.y * d.y + o.z * d.z;
  const r2 = o.x * o.x + o.y * o.y + o.z * o.z;
  return Math.sqrt(Math.max(r2 - along * along, 0)) - figure.radiusM;
}

/**
 * The transmittance per channel through a limb's whole chord along its axis, by the client's own
 * quadrature converged below 10⁻⁷ (`references.ts`'s `clientBeam`'s rule): twice the optical
 * depth from the tangent point to the top along the horizontal (`opticalDepth.ts`'s Simpson rule,
 * its intervals doubling from 20,000), and each absorber's curve at the whole chord's column, as
 * the transmittance table reads the chord.
 *
 * @throws RangeError for a case on a spheroid.
 * @throws Error where it does not converge within twelve doublings.
 */
export function chordTransmittance(referenceCase: ReferenceCase, geometry: ReferenceGeometry): Rgb {
  const { medium, figure } = referenceCase;
  if (figure.kind !== "sphere") {
    throw new RangeError("the slab's chord is taken on a sphere");
  }
  const r = figure.radiusM + tangentHeightM(referenceCase, geometry);
  const at = (steps: number): Rgb => {
    const half = opticalDepthToTop(medium, figure.radiusM, r, 0, steps);
    const amounts = absorberAmountsToTop(medium, figure.radiusM, r, 0, steps).map((a) =>
      amountPlus(a, a),
    );
    const pressures = absorberPressuresPa(medium, figure.radiusM, r, 0, true, steps);
    const absorbed: Rgb =
      referenceCase.curves.length > 0
        ? curvesOpticalDepth(referenceCase.curves, amounts, pressures)
        : [0, 0, 0];
    return perChannel((c) => 2 * half[c] + absorbed[c]);
  };
  let previous = at(ORACLE_STEPS);
  for (let k = 1; k <= 12; k += 1) {
    const next = at(ORACLE_STEPS * 2 ** k);
    const before = previous;
    const change = Math.max(
      ...CHANNELS.map((c) => Math.abs(next[c] - before[c]) / Math.max(1, next[c])),
    );
    if (change < 1e-7) {
      return perChannel((c) => Math.exp(-next[c]));
    }
    previous = next;
  }
  throw new Error(`${geometry.name}'s chord did not converge`);
}

/** `earth-ozone-slab`'s runs (check 5): its beams with each steps, and its sky with each. */
function slabSection(caseGate: CaseGate, values: TwinValues): RecordValue {
  const { referenceCase, reference } = caseGate.gateCase;
  const caseName = referenceCase.name;
  const beamRows: RecordValue[] = [];
  const beamMisses: Record<"split" | "equal", string[]> = { split: [], equal: [] };
  const suns = beamGroups(referenceCase).filter(
    ({ geometry }) => geometry.image === "ground-s80" || geometry.image === "ground-s90",
  );
  const limbs = referenceCase.geometries.filter((g) => g.image === "limb");
  for (const [steps, setting] of [
    ["split", "case"],
    ["equal", "equalSteps"],
  ] as const) {
    for (const { geometry } of suns) {
      const t = valueOf(values, { quantity: "beam", setting, caseName, name: geometry.name });
      const depth = referenceDepthOf(reference, geometry.name);
      const maximum = caseGate.maxima.get(geometry.image) ?? [0, 0, 0];
      const compared = perChannel((c) =>
        beamComparison(referenceCase.sunIlluminanceLx[c], t[c], depth[c], maximum[c]),
      );
      const missed = CHANNELS.filter((c) => compared[c].verdict !== "pass");
      if (missed.length > 0) {
        beamMisses[steps].push(channelsLabel(geometry.name, missed));
      }
      beamRows.push([
        geometry.name,
        "sun",
        steps,
        ...t,
        ...depth.map((d) => (d === null ? null : recorded(Math.exp(-d)))),
        ...compared.map((b) => b.ratio),
        ...compared.map((b) => b.verdict),
      ]);
    }
    for (const geometry of limbs) {
      const t = valueOf(values, { quantity: "sight", setting, caseName, name: geometry.name });
      const truth = chordTransmittance(referenceCase, geometry);
      const maximum = caseGate.maxima.get(geometry.image) ?? [0, 0, 0];
      const compared = perChannel((c) =>
        beamComparison(referenceCase.sunIlluminanceLx[c], t[c], -Math.log(truth[c]), maximum[c]),
      );
      const missed = CHANNELS.filter((c) => compared[c].verdict !== "pass");
      if (missed.length > 0) {
        beamMisses[steps].push(channelsLabel(geometry.name, missed));
      }
      beamRows.push([
        geometry.name,
        "sight",
        steps,
        ...t,
        ...recordedRgb(truth),
        ...compared.map((b) => b.ratio),
        ...compared.map((b) => b.verdict),
      ]);
    }
  }
  const skyRows: RecordValue[] = [];
  const skyMisses: Record<"pieces" | "placed", string[]> = { pieces: [], placed: [] };
  const skyGeometries = referenceCase.geometries.filter((g) =>
    SLAB_IMAGES.some((image) => image === g.image),
  );
  for (const [steps, setting] of [
    ["pieces", "case"],
    ["placed", "placedSteps"],
  ] as const) {
    for (const geometry of skyGeometries) {
      const twin = valueOf(values, {
        quantity: "radiance",
        setting,
        caseName,
        name: geometry.name,
      });
      const comparisons = comparisonsOf(caseGate, geometry.name);
      const verdicts = perChannel((c): Verdict => {
        const comparison = comparisons[c];
        if (comparison.refused) {
          return "refused";
        }
        return Math.abs(twin[c] - comparison.reference) <= comparison.bound ? "pass" : "miss";
      });
      const missed = CHANNELS.filter((c) => verdicts[c] === "miss");
      if (missed.length > 0) {
        skyMisses[steps].push(channelsLabel(geometry.name, missed));
      }
      skyRows.push([
        geometry.name,
        steps,
        ...twin,
        ...perChannel((c) => {
          const comparison = comparisons[c];
          return comparison.bound > 0
            ? recorded(Math.abs(twin[c] - comparison.reference) / comparison.bound)
            : null;
        }),
        ...verdicts,
      ]);
    }
  }
  const slab = referenceCase.medium.terms.find((t) => t.absorber !== undefined);
  const edgesM = slab === undefined ? [] : layerEdgesM(slab.density);
  const tangents = limbs.map((g) => [g.name, recorded(tangentHeightM(referenceCase, g) / 1_000)]);
  const lowest = Math.min(...edgesM);
  const highest = Math.max(...edgesM);
  const inside = limbs
    .filter((g) => {
      const h = tangentHeightM(referenceCase, g);
      return h > lowest && h < highest;
    })
    .map((g) => g.name);
  return {
    beams: { missesSplit: beamMisses.split, missesEqual: beamMisses.equal, rows: beamRows },
    sky: { missesPieces: skyMisses.pieces, missesPlaced: skyMisses.placed, rows: skyRows },
    layerEdges: {
      edgesKm: edgesM.map((e) => recorded(e / 1_000)),
      limbTangentsKm: tangents,
      tangentInsideTheSlab: inside,
      waitForT12b2: "the layer-edge checks at limbs tangent to the slab's base and its top",
    },
  };
}

/** R08.T9.b's surface irradiance at Earth (check 6), and `aerial-1km`'s surface share. */
function irradianceSection(
  earthGate: CaseGate,
  values: TwinValues,
  kind: RadianceKind,
): RecordValue {
  const { referenceCase, reference } = earthGate.gateCase;
  const twin = valueOf(values, {
    quantity: "downwelling",
    setting: "case",
    caseName: referenceCase.name,
    name: IRRADIANCE_AGGREGATE,
  });
  const reading = fluxReadingsOf(reference, kind).find(
    (f) => f.name === `${IRRADIANCE_AGGREGATE}/downwelling`,
  );
  if (reading === undefined) {
    throw new Error(`${referenceCase.name}'s reference has no ${IRRADIANCE_AGGREGATE}`);
  }
  const rows = CHANNELS.map((c) => {
    const refused = aggregateRefusal(reading, c);
    let verdict: Verdict = "refused";
    if (refused === undefined) {
      verdict = aggregatePasses(twin[c], reading, c) ? "pass" : "miss";
    }
    return [
      CHANNEL_LETTERS[c],
      twin[c],
      recorded(reading.flux[c]),
      recorded(reading.standardError[c]),
      recorded(twin[c] / reading.flux[c] - 1),
      verdict,
    ];
  });
  const aerial = "aerial-1km";
  const comparisons = comparisonsOf(earthGate, aerial);
  const share = valueOf(values, {
    quantity: "surfaceShare",
    setting: "case",
    caseName: referenceCase.name,
    name: aerial,
  });
  return {
    aggregate: `${IRRADIANCE_AGGREGATE}/downwelling`,
    tolerance: { relative: AGGREGATE_RELATIVE_TOLERANCE, sigmaCap: AGGREGATE_SIGMA_CAP },
    rows,
    aerial1km: {
      coneShare: coneShareOf(referenceCase, geometryNamed(referenceCase, aerial)),
      surfaceShare: [...share],
      verdict: perChannel((c) => verdictOf(comparisons[c])),
      ratio: perChannel((c) => boundRatio(comparisons[c])),
    },
  };
}

/** The view legs' points the legs apart takes along each geometry's axis: 64 midpoints. */
const LEG_POINTS = 64;

/** An absorber term's amount over a path of `densityPathM` of its relative density, m. */
function amountOf(term: MediumTerm, densityPathM: number): AbsorberAmount {
  const absorber = term.absorber;
  if (absorber === undefined) {
    throw new Error(`term ${term.name} is not an absorber`);
  }
  let amount: AbsorberAmount;
  switch (absorber.kind) {
    case "column":
      amount = { kind: "column", columnPerM2: absorber.numberDensityPerM3 * densityPathM };
      break;
    case "pair":
      amount = { kind: "pair", pairColumnPerM5: absorber.pairDensityPerM6 * densityPathM };
      break;
  }
  return amount;
}

/** The product of each absorber's curve read at its amount, per channel. */
function absorbersTransmittance(
  curves: ReadonlyArray<AbsorberCurve>,
  amounts: ReadonlyArray<AbsorberAmount>,
): Rgb {
  const t: [number, number, number] = [1, 1, 1];
  for (const [k, curve] of curves.entries()) {
    const amount = amounts[k];
    if (amount === undefined) {
      throw new Error(`no amount for absorber ${k}`);
    }
    const tk = absorberTransmittance(curve, amount, undefined);
    t[0] *= tk[0];
    t[1] *= tk[1];
    t[2] *= tk[2];
  }
  return t;
}

/**
 * The legs apart at a geometry (#65's deferred item, check 7), per channel: the single-scattered
 * light along the cone's axis, and the ground's one bounce where it ends on the ground, with each
 * absorber's transmittance taken as the twin takes it, T(u_view) T(u_sun), over the same light
 * taken as the reference takes it, T(u_view + u_sun), less 1. Each point's weight is its every
 * term's σ_s ρ p(θ) times e^(−τ) of both legs of every other term (`medium.ts`'s `phaseAt`, θ the
 * same along the ray), on {@link LEG_POINTS} midpoints of the path in the air, the view leg's
 * columns summed at them and the sun's by `opticalDepth.ts`'s Simpson rule; 0 for a medium with no
 * absorber.
 *
 * @throws RangeError for a case on a spheroid.
 */
export function legsApartOf(referenceCase: ReferenceCase, geometry: ReferenceGeometry): Rgb {
  const { medium, figure, curves } = referenceCase;
  if (figure.kind !== "sphere") {
    throw new RangeError("the legs apart are taken on a sphere");
  }
  const absorbers = medium.terms.filter((t) => t.absorber !== undefined);
  if (absorbers.length === 0 || curves.length === 0) {
    return [0, 0, 0];
  }
  const radiusM = figure.radiusM;
  const shell = { bottomRadiusM: radiusM, topRadiusM: radiusM + medium.topHeightM };
  const o = observerOf(referenceCase, geometry);
  const d = axisOf(geometry);
  const s = sunOf(geometry);
  const along = o.x * d.x + o.y * d.y + o.z * d.z;
  const r0 = Math.hypot(o.x, o.y, o.z);
  const startM = r0 > shell.topRadiusM ? enterTopM(r0, along, shell.topRadiusM) : 0;
  if (startM === null) {
    return [0, 0, 0];
  }
  const groundM = figureGroundDistanceM(o, d, figure);
  const exitM = exitTopM(r0, along, shell.topRadiusM);
  const endM = Math.min(geometry.maxDistanceM ?? Number.POSITIVE_INFINITY, groundM ?? exitM, exitM);
  const onGround = groundM !== null && groundM <= endM;
  const dt = (endM - startM) / LEG_POINTS;
  const cosTheta = d.x * s.x + d.y * s.y + d.z * s.z;
  const phases = medium.terms.map((t) => phaseAt(t.phase, cosTheta));
  const viewDepth: [number, number, number] = [0, 0, 0];
  const viewPaths = new Float64Array(absorbers.length);
  const joint: [number, number, number] = [0, 0, 0];
  const product: [number, number, number] = [0, 0, 0];
  const add = (pointM: Vec3, weight: Rgb, depthToHere: Rgb, pathsToHere: Float64Array): void => {
    const r = Math.hypot(pointM.x, pointM.y, pointM.z);
    const mu = (pointM.x * s.x + pointM.y * s.y + pointM.z * s.z) / r;
    if (intersectsGround(shell, r, mu)) {
      return;
    }
    const sunDepth = opticalDepthToTop(medium, radiusM, r, mu, DIAGNOSTIC_STEPS);
    const sunAmounts = absorberAmountsToTop(medium, radiusM, r, mu, DIAGNOSTIC_STEPS);
    const viewAmounts = absorbers.map((t, k) => amountOf(t, pathsToHere[k] ?? 0));
    const tView = absorbersTransmittance(curves, viewAmounts);
    const tSun = absorbersTransmittance(curves, sunAmounts);
    const tJoint = absorbersTransmittance(
      curves,
      viewAmounts.map((a, k) => {
        const b = sunAmounts[k];
        return b === undefined ? a : amountPlus(a, b);
      }),
    );
    for (const c of CHANNELS) {
      const w = weight[c] * Math.exp(-(depthToHere[c] + sunDepth[c]));
      joint[c] += w * tJoint[c];
      product[c] += w * tView[c] * tSun[c];
    }
  };
  for (let k = 0; k < LEG_POINTS; k += 1) {
    const t = startM + (k + 0.5) * dt;
    const pointM = { x: o.x + t * d.x, y: o.y + t * d.y, z: o.z + t * d.z };
    const h = Math.hypot(pointM.x, pointM.y, pointM.z) - radiusM;
    const densities = medium.terms.map((term) => densityAt(term.density, h));
    const absorbing = absorbers.map((term) => densityAt(term.density, h));
    // Each point's view leg runs to it from the path's start: the sums of the steps before it and
    // half its own.
    const here = perChannel((c) =>
      medium.terms.reduce(
        (sum, term, i) =>
          sum + (term.scattering[c] + term.absorption[c]) * (densities[i] ?? Number.NaN),
        0,
      ),
    );
    const depthToHere = perChannel((c) => viewDepth[c] + 0.5 * dt * here[c]);
    const pathsToHere = viewPaths.map((p, i) => p + 0.5 * dt * (absorbing[i] ?? Number.NaN));
    const weight = perChannel(
      (c) =>
        dt *
        medium.terms.reduce(
          (sum, term, i) =>
            sum +
            term.scattering[c] * (densities[i] ?? Number.NaN) * (phases[i]?.[c] ?? Number.NaN),
          0,
        ),
    );
    add(pointM, weight, depthToHere, pathsToHere);
    for (const c of CHANNELS) {
      viewDepth[c] += dt * here[c];
    }
    for (const [i, density] of absorbing.entries()) {
      viewPaths[i] = (viewPaths[i] ?? 0) + dt * density;
    }
  }
  if (onGround) {
    const pointM = { x: o.x + endM * d.x, y: o.y + endM * d.y, z: o.z + endM * d.z };
    const r = Math.hypot(pointM.x, pointM.y, pointM.z);
    const mu0 = (pointM.x * s.x + pointM.y * s.y + pointM.z * s.z) / r;
    if (mu0 > 0) {
      const weight = perChannel((c) => (medium.groundAlbedo[c] * mu0) / Math.PI);
      add(pointM, weight, viewDepth, viewPaths);
    }
  }
  return perChannel((c) => (joint[c] > 0 ? product[c] / joint[c] - 1 : 0));
}

/**
 * The distance from radius `r0` along a ray of `along` = o · d to its entry into a sphere of radius
 * `topRadiusM`, m, or `null` where it misses the sphere or the sphere lies behind.
 */
function enterTopM(r0: number, along: number, topRadiusM: number): number | null {
  const discriminant = along * along - (r0 - topRadiusM) * (r0 + topRadiusM);
  if (!(discriminant >= 0) || along >= 0) {
    return null;
  }
  return -along - Math.sqrt(discriminant);
}

/** The distance from radius `r0` along a ray of `along` = o · d to its exit from a sphere. */
function exitTopM(r0: number, along: number, topRadiusM: number): number {
  const discriminant = Math.max(along * along - (r0 - topRadiusM) * (r0 + topRadiusM), 0);
  return -along + Math.sqrt(discriminant);
}

/**
 * The legs apart at two legs of 10 vertical columns each, per channel: T(10 U) T(10 U) ÷ T(20 U)
 * less 1, U the case's vertical column from the ground (T6.c's science check's 2 × 10 × 300 DU).
 */
function legsAtTenColumns(referenceCase: ReferenceCase): {
  readonly columnDu: number;
  readonly lessOne: Rgb;
} {
  const { medium, figure, curves } = referenceCase;
  if (figure.kind !== "sphere" || curves.length === 0) {
    return { columnDu: 0, lessOne: [0, 0, 0] };
  }
  const vertical = absorberAmountsToTop(medium, figure.radiusM, figure.radiusM, 1);
  const scaled = (factor: number): AbsorberAmount[] =>
    vertical.map((a) =>
      a.kind === "column"
        ? { kind: "column", columnPerM2: factor * a.columnPerM2 }
        : { kind: "pair", pairColumnPerM5: factor * a.pairColumnPerM5 },
    );
  const ten = absorbersTransmittance(curves, scaled(10));
  const twenty = absorbersTransmittance(curves, scaled(20));
  const [first] = vertical;
  return {
    columnDu: first?.kind === "column" ? first.columnPerM2 / DOBSON_UNIT_PER_M2 : 0,
    lessOne: perChannel((c) => (ten[c] * ten[c]) / twenty[c] - 1),
  };
}

/** The legs apart on `earth` and `earth-ozone-slab` (check 7). */
function legsSection(gates: ReadonlyArray<CaseGate>): RecordValue {
  const out: Record<string, RecordValue> = {};
  for (const name of [CASE_NAMES.earth, CASE_NAMES.slab]) {
    const caseGate = gates.find((g) => g.gateCase.referenceCase.name === name);
    if (caseGate === undefined) {
      throw new Error(`the case gate has no case ${name}`);
    }
    const { referenceCase } = caseGate.gateCase;
    const rows = referenceCase.geometries.map((g) => {
      const lessOne = recordedRgb(legsApartOf(referenceCase, g));
      return { name: g.name, lessOne, held: comparisonsOf(caseGate, g.name) };
    });
    // The three largest by size in each channel, over every geometry and over those the gate holds.
    const largestOf = (heldOnly: boolean): RecordValue =>
      CHANNELS.map((c) => {
        const sorted = rows
          .filter((row) => !heldOnly || !row.held[c].refused)
          .toSorted((a, b) => Math.abs(b.lessOne[c]) - Math.abs(a.lessOne[c]));
        return [
          CHANNEL_LETTERS[c],
          ...sorted.slice(0, 3).flatMap((row) => [row.name, row.lessOne[c]]),
        ];
      });
    const ten = legsAtTenColumns(referenceCase);
    out[name] = {
      verticalColumnDu: recorded(ten.columnDu),
      atTenColumnsEachLessOne: [...recordedRgb(ten.lessOne)],
      largest: largestOf(false),
      largestHeld: largestOf(true),
      rows: rows.map((row) => [row.name, row.lessOne[0], row.lessOne[1], row.lessOne[2]]),
    };
  }
  return out;
}

/** The attribution (check 8): each geometry whose cone meets the ground, and its sort. */
function attributionSection(gates: ReadonlyArray<CaseGate>, values: TwinValues): RecordValue {
  const out: Record<string, RecordValue> = {};
  for (const caseGate of gates) {
    const { referenceCase } = caseGate.gateCase;
    const caseName = referenceCase.name;
    const rows: RecordValue[] = [];
    const sorted: Record<string, string[]> = {};
    for (const geometry of referenceCase.geometries) {
      const coneShare = coneShareOf(referenceCase, geometry);
      const kind = kindOf(referenceCase, geometry);
      if (!(coneShare > 0) || kind === "ground") {
        continue;
      }
      const comparisons = comparisonsOf(caseGate, geometry.name);
      const twin = valueOf(values, {
        quantity: "radiance",
        setting: "case",
        caseName,
        name: geometry.name,
      });
      const share = valueOf(values, {
        quantity: "surfaceShare",
        setting: "case",
        caseName,
        name: geometry.name,
      });
      const chromatic = valueOf(values, {
        quantity: "chromaticSurface",
        setting: "case",
        caseName,
        name: geometry.name,
      });
      const perChannelTwin = perChannel((c) => twin[c] - share[c] * twin[c] + chromatic[c]);
      const verdicts = perChannel((c) => verdictOf(comparisons[c]));
      const chromaticVerdicts = perChannel((c): Verdict => {
        const comparison = comparisons[c];
        if (comparison.refused) {
          return "refused";
        }
        return Math.abs(perChannelTwin[c] - comparison.reference) <= comparison.bound
          ? "pass"
          : "miss";
      });
      const missed = CHANNELS.filter((c) => verdicts[c] === "miss");
      const fixed = missed.filter((c) => chromaticVerdicts[c] === "pass");
      let sort = "no miss";
      if (missed.length > 0) {
        if (fixed.length === missed.length) {
          sort = "grey";
        } else if (fixed.length > 0) {
          sort = "partly grey";
        } else {
          sort = "not grey";
        }
      }
      if (missed.length > 0) {
        sorted[sort] = [...(sorted[sort] ?? []), channelsLabel(geometry.name, missed)];
      }
      rows.push([
        geometry.name,
        kind,
        coneShare,
        ...share,
        ...verdicts,
        ...chromaticVerdicts,
        sort,
      ]);
    }
    const groundViews = referenceCase.geometries.filter(
      (g) => kindOf(referenceCase, g) === "ground" && coneShareOf(referenceCase, g) > 0,
    );
    out[caseName] = {
      groundViewsMeetingTheGround: groundViews.map((g) => g.name),
      missesBySort: sorted,
      rows,
    };
  }
  return out;
}

/** The twin's values the gate's rows do not hold, one a row: every one but each geometry's own. */
function twinRows(values: TwinValues, specs: ReadonlyArray<TwinValueSpec>): RecordValue[] {
  return specs
    .filter(
      (spec) =>
        !(
          spec.setting === "case" &&
          (spec.quantity === "radiance" || spec.quantity === "surfaceShare")
        ),
    )
    .map((spec) => {
      const [r, g, b] = valueOf(values, spec);
      return [twinValueKey(spec), r, g, b];
    });
}

/**
 * What the record's head says of its writer and the radiance it reads; the twin's settings are the
 * module's own.
 */
export interface RecordHead {
  /** The test that writes the record. */
  readonly writer: string;
  /**
   * The references' radiance the gate reads: `scalar` for R08.T13's drift gate, scalar against
   * scalar (Design note 10, _Polarisation_), and `vector` for R08.T14.f's.
   */
  readonly radiance: RadianceKind;
}

/**
 * The case gate's record (the module's remarks), from the cases, their references and the twin's
 * recorded values: a pure function of them.
 *
 * @throws Error for a value, geometry or case the record reads that is not there.
 */
export function caseGateRecord(
  cases: ReadonlyArray<GateCase>,
  values: TwinValues,
  head: RecordHead,
): RecordValue {
  const specs = twinValueSpecs(cases);
  const kind = head.radiance;
  const gates = cases.map((c) => caseGateOf(c, values, kind));
  const gateNamed = (name: string): CaseGate => {
    const found = gates.find((g) => g.gateCase.referenceCase.name === name);
    if (found === undefined) {
      throw new Error(`the case gate has no case ${name}`);
    }
    return found;
  };
  const caseSections: Record<string, RecordValue> = {};
  const beamSections: Record<string, RecordValue> = {};
  for (const caseGate of gates) {
    caseSections[caseGate.gateCase.referenceCase.name] = caseSection(caseGate, values);
    beamSections[caseGate.gateCase.referenceCase.name] = beamSection(caseGate, values);
  }
  return {
    description: `R08.T13.c1's case gate: gate.ts's metric on the CPU twin against the four committed references' ${kind} radiance, with R08.T13's checks on them (plan R08). Written by caseGate.test.ts through caseGate.ts; do not edit by hand.`,
    format: CASE_GATE_FORMAT,
    writer: head.writer,
    twin: {
      module: "reference/twin.ts, referenceTwin, through caseGate.ts's caseTwin",
      sizes: "TABLE_SIZES.high",
      sunIrradiance: "each case's sunIlluminanceLx",
      settings: {
        case: "its own slicing, the transmittance split, the marches in pieces",
        oneSlice: "one slice and one band (saturn-oblate)",
        equalSteps: "R05's 256 equal transmittance steps (earth-ozone-slab)",
        placedSteps: "R05.T12.e's placement in every march (earth-ozone-slab)",
      },
      reference:
        kind === "scalar"
          ? "the scalar radiance (readingsOf, scalar), scalar against scalar"
          : `the ${kind} radiance (readingsOf, ${kind})`,
      significantFigures: RECORD_SIGNIFICANT_FIGURES,
    },
    metric: {
      relativeTolerance: METRIC_RELATIVE_TOLERANCE,
      floor: METRIC_FLOOR,
      sigmaAllowance: METRIC_SIGMA_ALLOWANCE,
      sigmaDivisor: METRIC_SIGMA_DIVISOR,
      mustFailSigmas: MUST_FAIL_SIGMAS,
      beamDepthTolerance: BEAM_DEPTH_TOLERANCE,
      aggregateRelativeTolerance: AGGREGATE_RELATIVE_TOLERANCE,
      aggregateSigmaCap: AGGREGATE_SIGMA_CAP,
      offSunDeg: OFF_SUN_DEG,
    },
    columns: Object.fromEntries(
      Object.entries(RECORD_COLUMNS).map(([key, names]) => [key, [...names]]),
    ),
    cases: caseSections,
    marsOffSun: marsOffSunSection(gateNamed(CASE_NAMES.mars)),
    marsAureole: marsAureoleSection(gateNamed(CASE_NAMES.mars), values),
    saturnOblateness: saturnSection(gateNamed(CASE_NAMES.saturn), values),
    beams: beamSections,
    slab: slabSection(gateNamed(CASE_NAMES.slab), values),
    irradiance: irradianceSection(gateNamed(CASE_NAMES.earth), values, kind),
    legsApart: legsSection(gates),
    attribution: attributionSection(gates, values),
    twinValues: twinRows(values, specs),
  };
}

// --- Reading a committed record back -------------------------------------------------------------

/** Whether a value is a plain record of fields. */
function isFields(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** A record's field, or `undefined`. */
function fieldOf(value: unknown, key: string): unknown {
  return isFields(value) ? value[key] : undefined;
}

/** Three numbers at `at` of a row, or `undefined`. */
function rgbAt(row: ReadonlyArray<unknown>, at: number): Rgb | undefined {
  const [r, g, b] = [row[at], row[at + 1], row[at + 2]];
  return typeof r === "number" && typeof g === "number" && typeof b === "number"
    ? [r, g, b]
    : undefined;
}

/** A column's index in one of {@link RECORD_COLUMNS}' tables. */
function columnOf(columns: ReadonlyArray<string>, name: string): number {
  const at = columns.indexOf(name);
  if (at < 0) {
    throw new Error(`the record has no column ${name}`);
  }
  return at;
}

/**
 * The twin's values a committed record holds (its gate rows' radiance and surface share, and its
 * `twinValues` rows), or an empty map for a placeholder or a record of another format.
 */
export function twinValuesOfRecord(record: unknown): Map<string, Rgb> {
  const values = new Map<string, Rgb>();
  if (fieldOf(record, "format") !== CASE_GATE_FORMAT) {
    return values;
  }
  const cases = fieldOf(record, "cases");
  const nameAt = columnOf(RECORD_COLUMNS.geometry, "name");
  const twinAt = columnOf(RECORD_COLUMNS.geometry, "twinR");
  const shareAt = columnOf(RECORD_COLUMNS.geometry, "surfaceShareR");
  if (isFields(cases)) {
    for (const [caseName, section] of Object.entries(cases)) {
      const rows = fieldOf(section, "geometries");
      if (!Array.isArray(rows)) {
        continue;
      }
      for (const row of rows) {
        if (!Array.isArray(row)) {
          continue;
        }
        const name: unknown = row[nameAt];
        const twin = rgbAt(row, twinAt);
        const share = rgbAt(row, shareAt);
        if (typeof name !== "string" || twin === undefined || share === undefined) {
          continue;
        }
        const spec = { setting: "case", caseName, name } as const;
        values.set(twinValueKey({ ...spec, quantity: "radiance" }), twin);
        values.set(twinValueKey({ ...spec, quantity: "surfaceShare" }), share);
      }
    }
  }
  const twinRowsRead = fieldOf(record, "twinValues");
  if (Array.isArray(twinRowsRead)) {
    for (const row of twinRowsRead) {
      if (!Array.isArray(row)) {
        continue;
      }
      const key: unknown = row[0];
      const value = rgbAt(row, 1);
      if (typeof key === "string" && value !== undefined) {
        values.set(key, value);
      }
    }
  }
  return values;
}
