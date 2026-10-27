/**
 * The CPU twin's diffuse radiance at a reference geometry, as the client draws it (plan R08,
 * R08.T13.a; `decision-r08-t13-scope.md`, ruling 8, and its "#107's twin ground"): what R08.T13's
 * sweep and its case gate hold to the reference tracer's radiance, averaged over the tracer's
 * detector cone.
 *
 * @remarks
 * A geometry is read by where its observer stands and what it looks at:
 *
 * - **On the ground** (an observer under the march band, `hillaire.ts`'s `marchWeightOf` 0): the
 *   sky view, built at the observer for the geometry's sun and read in each direction as the
 *   composite reads it (`tablesCpu.ts`'s `skyViewTexelsTwin` and `skyViewReadTwin`).
 * - **From orbit** (an observer at or above the top): the ray march's texel at the centre of a
 *   camera at the observer looking along each direction, the default field of view
 *   (`DEFAULT_FOV_DEG`) over {@link TWIN_VIEWPORT}, its far-limb footprint blended in where the
 *   texel's span asks for it (`rayMarchTargetTexelTwin`, R08.T8.c).
 * - **Aerial perspective** (a geometry with `maxDistanceM`): the aerial-perspective volume read as
 *   `aerialRead.wgsl` reads it at the screen's centre of the same camera, at the target's distance.
 *   The composite's read at the centre is the mean of the volume's four central froxel columns,
 *   which a 2 × 2 volume of a camera of 1 ÷ 31 the field's tangent holds exactly
 *   ({@link AERIAL_FROXEL_SCALE}).
 *
 * **The ground** (the ruling's item 1): where a direction of the cone meets the ground within the
 * view, the client draws a surface there, and the twin draws it as the composite lays it: a grey
 * Lambertian ground of the medium's albedo, lit by `surfaceLighting.wgsl`'s lookups as
 * `terrainLit.wgsl` takes them for a star, A ÷ π (μ₀ T(0, μ₀) + E_sky(0, μ₀)) per unit of the sun's
 * irradiance (`atmosphere_sun_transmittance` and `atmosphere_sky_irradiance`, `tablesCpu.ts`'s
 * `surfaceLightingTwin`), behind the march's mean transmittance (from orbit, the march ending on
 * that surface) or the volume's at the ground's distance (aerial perspective). The tracer traces
 * the same ray to the ground and reflects it there, so the comparison is of the whole pixel: the
 * ground's skylight and the ground–sky bounces Hillaire's one bounce leaves out are the analytic
 * path's errors, which τ* sees. {@link ReferenceTwin.groundConeShare} gives a geometry's share of
 * the cone that meets the ground, {@link ReferenceTwin.surfaceRadiance} the drawn surface's part
 * of its radiance; neither is gated. A ground observer's cone never meets the ground in any case
 * (R08.T13.c1 checked the four committed cases' and the sweep's), so the twin draws no surface
 * there, and refuses a ground view that would need one.
 *
 * Every radiance is diffuse alone, per unit of the sun's irradiance (each source's weight 1,
 * `twinView`), then times {@link TwinOptions.sunIrradiance}: the unit of the reference's
 * `radiance`. The tables are built once a medium, as one slice and one band of an oblate body's
 * (`oblatePlanetTwin` of `kernelMedium(medium)` on the sphere, with no spin: its band's tables are
 * `planetTwin`'s), and read at {@link TwinOptions.sizes}, `TABLE_SIZES.high` by default, as the
 * twin's gate tests take them.
 *
 * **The figure** ({@link TwinFigure}, a case's own; R08.T13.c1): a sphere, or a level spheroid
 * (`saturn-oblate`), whose tables are `oblatePlanetTwin`'s slices and bands of its own slicing (or
 * one slice and one band, {@link TwinOptions.slicing}), read by the oblate twins as the client's
 * oblate kernels read them: `oblateSkyViewTexelsTwin` and `oblateSkyViewReadTwin` on the ground,
 * `oblateRayMarchTargetTexelTwin` from orbit, `oblateAerialPerspectiveTwin`, and the lit ground
 * through `surfaceLightingTwin` over `oblateIrradianceTwin`'s bands and the sliced sun's lookup, as
 * `tablesCpuOblate.test.ts` drives them.
 *
 * **The steps** ({@link TwinOptions.transmittanceSteps} and {@link TwinOptions.marchSteps},
 * R08.T13.c1, for `earth-ozone-slab`): the transmittance tables split at the terms' layer edges
 * (R08.T6.d) or in R05's 256 equal steps, and the marches cut into pieces at the edges (R08.T6.i)
 * or in R05.T12.e's placement, each by building that part of the tables from the medium with its
 * layer edges dropped. A medium with no layer edge reads the same either way, to the bit.
 */

import { normalise, type Vec3 } from "../../../geometry/vec3";
import { DEFAULT_FOV_DEG } from "../../camera/projection";
import { lookAlong } from "../../camera/quaternion";
import type { Rgb } from "../../photometry/toneCurve";
import { coneAverage } from "../gate";
import {
  type AtmosphereCamera,
  geodeticOf,
  marchWeightOf,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
  type TableSizes,
} from "../hillaire";
import { type AtmosphereMedium, layerEdgesM, type MediumTerm } from "../medium";
import { bodyGravity, type OblateSlicing } from "../oblate";
import type { SurfaceLightingReader } from "../surfaceLighting";
import {
  type AbsorberCurveSlots,
  IRRADIANCE_SIZE,
  kernelMedium,
  NO_ABSORBER_CURVES,
} from "../tables";
import {
  aerialPerspectiveTwin,
  compositeTransmittanceTwin,
  IRRADIANCE_KERNEL_OPTIONS,
  OBLATE_KERNEL_SIZES,
  type OblatePlanetTwin,
  type OblateTwinSizes,
  oblateAerialPerspectiveTwin,
  oblateCompositeTransmittanceTwin,
  oblateIrradianceTwin,
  oblatePlanetTwin,
  oblateRayMarchTargetTexelTwin,
  oblateSkyViewReadTwin,
  oblateSkyViewTexelsTwin,
  type PlanetTwin,
  rayMarchTargetTexelTwin,
  skyViewReadTwin,
  skyViewTexelsTwin,
  surfaceLightingTwin,
  tableSources,
  transmittanceTablesTwin,
  type TwinBody,
  twinBandOf,
  type TwinVolume,
  twinView,
  type TwinView,
} from "../tablesCpu";
import type { CaseFigure } from "./cases";
import {
  geodeticPoint,
  localDirectionVector,
  REFERENCE_DETECTOR_HALF_ANGLE_DEG,
  type ReferenceGeometry,
} from "./geometries";

/**
 * The figure a twin's case stands on: a case's own (`cases.ts`'s `CaseFigure`), a sphere of the
 * ground's radius or a level spheroid with its gravity and slicing (the module's remarks).
 */
export type TwinFigure = CaseFigure;

/**
 * A spheroid's slicing: its own (`body`, R08.T6.e's), or one slice at κ = 1 and one band at s = 1
 * (`single`), the slice read at h\* and μ as R05's table at h: the check that the gate sees
 * oblateness (R08.T13). A sphere has one slice and one band either way.
 */
export type TwinSlicing = "body" | "single";

/**
 * The transmittance tables' steps: `split`, 256 a ray spread over its pieces between the terms'
 * layer edges (R08.T6.d, the kernel's), or `equal`, R05's 256 equal steps.
 */
export type TwinTransmittanceSteps = "split" | "equal";

/**
 * The marches' steps, the multiple-scattering and sky-irradiance tables' and the per-frame sky
 * view's and ray march's: `pieces`, each side cut at the layer edges it crosses (R08.T6.i, the
 * kernels'), or `placed`, R05.T12.e's placement on the whole side.
 */
export type TwinMarchSteps = "pieces" | "placed";

/** How a twin reads its tables and averages its cone. */
export interface TwinOptions {
  /** The tables' sizes and steps: `TABLE_SIZES.high`, the twin's gate tests' setting. */
  readonly sizes: TableSizes;
  /** The detector cone's half-angle, degrees: the reference's 0.5° by default. */
  readonly detectorHalfAngleDeg: number;
  /** The sun's irradiance per channel at the top, in the case's unit: 1 in each by default. */
  readonly sunIrradiance: Rgb;
  /** The medium's absorbers' curves of growth, one light slot: none by default. */
  readonly curves: AbsorberCurveSlots;
  /** A spheroid's slicing: its own by default. */
  readonly slicing: TwinSlicing;
  /** The transmittance tables' steps: split at the layer edges by default. */
  readonly transmittanceSteps: TwinTransmittanceSteps;
  /** The marches' steps: cut into pieces at the layer edges by default. */
  readonly marchSteps: TwinMarchSteps;
}

/** The options a twin takes where its caller gives none. */
export const DEFAULT_TWIN_OPTIONS: TwinOptions = {
  sizes: TABLE_SIZES.high,
  detectorHalfAngleDeg: REFERENCE_DETECTOR_HALF_ANGLE_DEG,
  sunIrradiance: [1, 1, 1],
  curves: NO_ABSORBER_CURVES,
  slicing: "body",
  transmittanceSteps: "split",
  marchSteps: "pieces",
};

/**
 * The camera a twin looks through from orbit and for aerial perspective, device pixels: 1,921 ×
 * 1,081, about 16:9 and odd on both axes, so that the centre texel's ray is the camera's axis.
 */
export const TWIN_VIEWPORT = { widthPx: 1921, heightPx: 1081 } as const;

/**
 * The tangent of a 2 × 2 aerial-perspective volume's field against the client's camera's: 1 ÷ 31,
 * so that its corner froxels, at (0, 0) and (1, 1) of its screen, stand where the client's 32 × 32
 * volume's four central froxels stand, at 15 ÷ 31 and 16 ÷ 31 (Bruneton's mapping, Design note 19).
 */
export const AERIAL_FROXEL_SCALE = 1 / 31;

/** The near plane the twin's cameras take, m: what a surface's distance is written against. */
const NEAR_M = 1;

/** No light. */
const DARK: Rgb = [0, 0, 0];

/** One slice at κ = 1 and one band at s = 1 ({@link TwinSlicing}'s `single`). */
const ONE_SLICE_ONE_BAND: OblateSlicing = {
  kappa: Float64Array.of(1),
  bandGravityRatio: Float64Array.of(1),
};

/** How a geometry is drawn (the module's remarks). */
export type TwinGeometryKind = "ground" | "orbit" | "aerial";

/**
 * How the twin draws a geometry of `medium`: `aerial` for a path to a target, `orbit` for an
 * observer at or above the top, `ground` for one under the march band, its geodetic height read
 * against the medium's top.
 *
 * @throws RangeError for an observer inside the air but in or above the march band, which no case
 *   has: the twin does not blend the sky view with the march.
 */
export function twinGeometryKind(
  geometry: ReferenceGeometry,
  medium: AtmosphereMedium,
  sizes: TableSizes = TABLE_SIZES.high,
): TwinGeometryKind {
  if (geometry.maxDistanceM !== undefined) {
    return "aerial";
  }
  const heightM = geometry.observer.heightM;
  if (heightM >= medium.topHeightM) {
    return "orbit";
  }
  if (marchWeightOf(heightM, medium.topHeightM, sizes.marchBand) > 0) {
    throw new RangeError(
      `the twin draws no observer in the march band: ${geometry.name} at ${heightM} m`,
    );
  }
  return "ground";
}

/** The distance along a unit ray from `originM` to a sphere of radius `radiusM`, m, or `null`. */
export function groundDistanceM(originM: Vec3, direction: Vec3, radiusM: number): number | null {
  const along = originM.x * direction.x + originM.y * direction.y + originM.z * direction.z;
  const r = Math.hypot(originM.x, originM.y, originM.z);
  const above = (r - radiusM) * (r + radiusM);
  const discriminant = along * along - above;
  if (!(discriminant >= 0) || along >= 0) {
    return null;
  }
  const near = -along - Math.sqrt(discriminant);
  return near > 0 ? near : null;
}

/**
 * The distance along a unit ray from `originM`, outside it, to the spheroid (a, a, c) of `datum`,
 * m, or `null` where the ray misses it or it lies behind: the sphere's form on the axes scaled by
 * a ÷ c along z, its nearer root taken as C ÷ (−B + √(B² − AC)), which does not cancel.
 */
export function spheroidGroundDistanceM(
  originM: Vec3,
  direction: Vec3,
  datum: SpheroidFigure,
): number | null {
  const a = datum.equatorialRadiusM;
  const k = a / datum.polarRadiusM;
  const o = { x: originM.x, y: originM.y, z: originM.z * k };
  const d = { x: direction.x, y: direction.y, z: direction.z * k };
  const quadratic = d.x * d.x + d.y * d.y + d.z * d.z;
  const along = o.x * d.x + o.y * d.y + o.z * d.z;
  const r = Math.hypot(o.x, o.y, o.z);
  const above = (r - a) * (r + a);
  const discriminant = along * along - quadratic * above;
  if (!(discriminant >= 0) || along >= 0) {
    return null;
  }
  const near = above / (-along + Math.sqrt(discriminant));
  return near > 0 ? near : null;
}

/** A figure's datum: the sphere's (a = c = R) or the level spheroid's. */
export function twinDatum(figure: TwinFigure): SpheroidFigure {
  let datum: SpheroidFigure;
  switch (figure.kind) {
    case "sphere":
      datum = { equatorialRadiusM: figure.radiusM, polarRadiusM: figure.radiusM };
      break;
    case "spheroid":
      datum = figure.gravity.spheroid;
      break;
  }
  return datum;
}

/** The distance along a unit ray from `originM` to `figure`'s ground, m, or `null`. */
export function figureGroundDistanceM(
  originM: Vec3,
  direction: Vec3,
  figure: TwinFigure,
): number | null {
  let distanceM: number | null;
  switch (figure.kind) {
    case "sphere":
      distanceM = groundDistanceM(originM, direction, figure.radiusM);
      break;
    case "spheroid":
      distanceM = spheroidGroundDistanceM(originM, direction, figure.gravity.spheroid);
      break;
  }
  return distanceM;
}

/**
 * The share of a geometry's cone, by the cone average's own weights (`gate.ts`'s `coneAverage`),
 * whose directions meet the ground of `figure` within the view: before the target for aerial
 * perspective, anywhere otherwise. It depends on the geometry alone, not the medium.
 *
 * @param detectorHalfAngleDeg - The cone, where the geometry gives none of its own: the
 *   reference's 0.5° by default.
 */
export function groundConeShare(
  geometry: ReferenceGeometry,
  figure: TwinFigure,
  detectorHalfAngleDeg: number = REFERENCE_DETECTOR_HALF_ANGLE_DEG,
): number {
  const datum = twinDatum(figure);
  const originM = geodeticPoint(datum, geometry.observer.latitudeDeg, geometry.observer.heightM);
  const limitM = geometry.maxDistanceM ?? Number.POSITIVE_INFINITY;
  const meets = (direction: Vec3): Rgb => {
    const groundM = figureGroundDistanceM(originM, direction, figure);
    return groundM !== null && groundM < limitM ? [1, 1, 1] : DARK;
  };
  const halfAngleRad = ((geometry.detectorHalfAngleDeg ?? detectorHalfAngleDeg) * Math.PI) / 180;
  return coneAverage(meets, axisOf(geometry), halfAngleRad)[0];
}

/** A geometry's sun as a source of the twin's views: its light per unit of its irradiance. */
function sunState(geometry: ReferenceGeometry): SunState {
  return {
    directionBodyFixed: localDirectionVector(geometry.observer.latitudeDeg, geometry.sun),
    illuminanceLx: [1, 1, 1],
    angularRadiusRad: 0,
  };
}

/** A geometry's view, body-fixed: its cone's axis. */
function axisOf(geometry: ReferenceGeometry): Vec3 {
  return localDirectionVector(geometry.observer.latitudeDeg, geometry.view);
}

/** The point `tM` along a ray. */
function pointAlong(originM: Vec3, direction: Vec3, tM: number): Vec3 {
  return {
    x: originM.x + tM * direction.x,
    y: originM.y + tM * direction.y,
    z: originM.z + tM * direction.z,
  };
}

/** A ground observer's sky view: the frame's view and its table's texels, each as first read. */
interface SkyViewRead {
  readonly view: TwinView;
  readonly texelAt: (x: number, y: number) => Rgb;
}

/** A direction's radiance: the whole pixel's, and the drawn surface's part of it. */
export interface TwinParts {
  readonly total: Rgb;
  readonly surface: Rgb;
}

/** A reference twin for one case: its radiance at a geometry, and the reads it is made of. */
export interface ReferenceTwin {
  /**
   * The per-planet tables a sphere's marches read; on a spheroid its first slice's and band's,
   * which no march reads alone ({@link ReferenceTwin.oblate}).
   */
  readonly planet: PlanetTwin;
  /** The slices and bands the tables are built as: one of each on a sphere. */
  readonly oblate: OblatePlanetTwin;
  /** The ground's lighting, `surfaceLighting.wgsl`'s lookups on the same tables. */
  readonly surfaceLighting: SurfaceLightingReader;
  /**
   * The twin's diffuse radiance at a geometry, averaged over the cone (the module's remarks).
   *
   * @throws RangeError as {@link twinGeometryKind}, or for a ground view whose cone meets the
   *   ground, for which the twin draws no surface.
   */
  readonly radiance: (geometry: ReferenceGeometry) => Rgb;
  /**
   * The drawn surface's part of {@link ReferenceTwin.radiance}: 0 where no ray meets the ground.
   *
   * @throws RangeError as {@link ReferenceTwin.radiance}.
   */
  readonly surfaceRadiance: (geometry: ReferenceGeometry) => Rgb;
  /**
   * The share of a geometry's cone, by the cone average's own weights, whose directions meet the
   * ground within the view: before the target for aerial perspective.
   */
  readonly groundConeShare: (geometry: ReferenceGeometry) => number;
  /**
   * The radiance along one direction, body-fixed, per unit of the sun's irradiance.
   *
   * @throws RangeError as {@link ReferenceTwin.radiance}, for a ground view that meets the ground.
   */
  readonly radianceAlong: (geometry: ReferenceGeometry, direction: Vec3) => TwinParts;
  /**
   * The lit ground's radiance at a point of the datum, per unit of the sun's irradiance (the
   * module's remarks), the sun along `sun`.
   */
  readonly litGroundRadiance: (groundM: Vec3, sun: Vec3) => Rgb;
  /**
   * What the composite multiplies the light from beyond the air along a unit body-fixed
   * `direction` from a geometry's observer by, per channel (`composite.wgsl`;
   * `compositeTransmittanceTwin`, the sliced lookup's `oblateCompositeTransmittanceTwin` on a
   * spheroid): the transmittance tables' read from the observer inside the air, or from the ray's
   * entry into the top above it; (1, 1, 1) where the ray misses the air and (0, 0, 0) where the
   * ground hides space. Towards the sun it is the client's drawn beam (Design note 10).
   */
  readonly transmittanceAlong: (geometry: ReferenceGeometry, direction: Vec3) => Rgb;
}

/** A medium with every tabulated term's layer edges dropped: R05's one piece a ray. */
function withoutLayerEdges(medium: AtmosphereMedium): AtmosphereMedium {
  return {
    ...medium,
    terms: medium.terms.map((term): MediumTerm =>
      term.density.kind === "tabulated"
        ? { ...term, density: { ...term.density, edgesM: [] } }
        : term,
    ),
  };
}

/** Whether any of a medium's terms has a layer edge (`medium.ts`'s `layerEdgesM`). */
function hasLayerEdges(medium: AtmosphereMedium): boolean {
  return medium.terms.some((term) => layerEdgesM(term.density).length > 0);
}

/** The body a figure's tables are built on, its slicing as `slicing` asks. */
function bodyOf(figure: TwinFigure, datum: SpheroidFigure, slicing: TwinSlicing): TwinBody {
  let body: TwinBody;
  switch (figure.kind) {
    case "sphere":
      // A sphere with no spin: one slice and one band, s ≡ 1, whatever the bulk gravity, which is
      // read nowhere (`hillaire.ts`'s `bodyOfFigure`).
      body = {
        figure: datum,
        gravity: bodyGravity(
          {
            figure: datum,
            massKg: 1,
            angularVelocityRadS: null,
            bulkGravityMS2: 9.806_65,
            figureLaw: null,
          },
          figure.radiusM,
        ),
      };
      break;
    case "spheroid": {
      const { gravity } = figure;
      body = {
        figure: datum,
        gravity: slicing === "single" ? { ...gravity, slicing: ONE_SLICE_ONE_BAND } : gravity,
      };
      break;
    }
  }
  return body;
}

/**
 * The slices and bands of `drawn` on `body` under the options' steps (the module's remarks):
 * `oblatePlanetTwin`'s with the kernels' steps, or for a medium with layer edges under another
 * setting, one slice and one band built as `oblatePlanetTwin` builds them, the transmittance from
 * the medium as its option asks and every march from the medium as its own asks.
 *
 * @throws RangeError for the steps' options on a medium with layer edges over several slices or
 *   bands, which no case has.
 */
function tablesOf(
  drawn: AtmosphereMedium,
  body: TwinBody,
  settings: TwinOptions,
): OblatePlanetTwin {
  const kernels = settings.transmittanceSteps === "split" && settings.marchSteps === "pieces";
  if (kernels || !hasLayerEdges(drawn)) {
    return oblatePlanetTwin(drawn, body, settings.curves, OBLATE_KERNEL_SIZES);
  }
  const { kappa, bandGravityRatio } = body.gravity.slicing;
  const [k] = kappa;
  const [s] = bandGravityRatio;
  if (kappa.length !== 1 || bandGravityRatio.length !== 1 || k === undefined || s === undefined) {
    throw new RangeError("the twin takes the steps' options on one slice and one band alone");
  }
  const tablesMedium = settings.transmittanceSteps === "equal" ? withoutLayerEdges(drawn) : drawn;
  const marchMedium = settings.marchSteps === "placed" ? withoutLayerEdges(drawn) : drawn;
  const sizes: OblateTwinSizes =
    settings.marchSteps === "placed"
      ? {
          ...OBLATE_KERNEL_SIZES,
          multiScattering: { ...OBLATE_KERNEL_SIZES.multiScattering, placement: "placed" },
        }
      : OBLATE_KERNEL_SIZES;
  const referenceRadiusM = tableRadiusM(body.figure);
  const slice = transmittanceTablesTwin(
    tablesMedium,
    k * referenceRadiusM,
    settings.curves,
    sizes.transmittance,
    sizes.transmittanceSamples,
  );
  const band = twinBandOf(marchMedium, body, s, settings.curves, sizes, true, slice);
  return {
    medium: marchMedium,
    body,
    referenceRadiusM,
    curves: settings.curves,
    kappa,
    slices: [slice],
    bands: [band],
  };
}

/**
 * The twin of a medium on a figure (the module's remarks): its tables built once, each sun's sky
 * view as it is first read.
 *
 * @throws RangeError as `oblatePlanetTwin`, for a medium the tables refuse, or for the steps'
 *   options on a medium with layer edges over several slices or bands.
 */
export function referenceTwin(
  medium: AtmosphereMedium,
  figure: TwinFigure,
  options: Partial<TwinOptions> = {},
): ReferenceTwin {
  const settings: TwinOptions = { ...DEFAULT_TWIN_OPTIONS, ...options };
  const drawn = kernelMedium(medium);
  const datum = twinDatum(figure);
  const body = bodyOf(figure, datum, settings.slicing);
  const oblate = tablesOf(drawn, body, settings);
  const marched = oblate.medium;
  const [band] = oblate.bands;
  const [first, ...more] = band?.multiScatteringBySlot ?? [];
  if (band === undefined || first === undefined) {
    throw new Error("a body's tables have a band, with a multiple-scattering layer");
  }
  const planet: PlanetTwin = {
    medium: marched,
    bottomRadiusM: band.bottomRadiusM,
    ...band.transmittance,
    multiScattering: first,
    ...(more.length > 0 ? { multiScatteringBySlot: band.multiScatteringBySlot } : {}),
  };
  // The sky's irradiance at the ground: the table's ground row, which the ground reads alone.
  const groundRow = Array.from({ length: IRRADIANCE_SIZE.widthTexels }, (_, x) => [x, 0] as const);
  const lighting = surfaceLightingTwin(
    oblate,
    oblateIrradianceTwin(oblate, IRRADIANCE_KERNEL_OPTIONS, groundRow),
  );
  const { sizes } = settings;
  const sphere = figure.kind === "sphere";
  const sources = tableSources(planet);

  const observerOf = (geometry: ReferenceGeometry): Vec3 =>
    geodeticPoint(datum, geometry.observer.latitudeDeg, geometry.observer.heightM);
  const cameraAlong = (originM: Vec3, direction: Vec3, fovXRad: number): AtmosphereCamera => ({
    positionM: originM,
    orientation: lookAlong(direction, normalise(originM)),
    fovXRad,
    viewport: TWIN_VIEWPORT,
  });
  const defaultFovRad = (DEFAULT_FOV_DEG * Math.PI) / 180;

  // A ground point's latitude, normal, north and east: on a sphere from its radius vector, as the
  // lit pass gives them to `surfaceLighting.wgsl`; on a spheroid its geodetic ones.
  const groundFrameOf = (
    groundM: Vec3,
  ): {
    readonly latitudeRad: number;
    readonly n: Vec3;
    readonly north: Vec3;
    readonly east: Vec3;
  } => {
    let latitudeRad: number;
    let longitudeRad: number;
    let n: Vec3;
    if (sphere) {
      n = normalise(groundM);
      latitudeRad = Math.asin(Math.min(Math.max(n.z, -1), 1));
      longitudeRad = Math.atan2(n.y, n.x);
    } else {
      const geodetic = geodeticOf(groundM, datum);
      n = geodetic.normal;
      latitudeRad = geodetic.latitudeRad;
      longitudeRad = geodetic.longitudeRad;
    }
    return {
      latitudeRad,
      n,
      north: {
        x: -Math.sin(latitudeRad) * Math.cos(longitudeRad),
        y: -Math.sin(latitudeRad) * Math.sin(longitudeRad),
        z: Math.cos(latitudeRad),
      },
      east: { x: -Math.sin(longitudeRad), y: Math.cos(longitudeRad), z: 0 },
    };
  };

  const litGroundRadiance = (groundM: Vec3, sun: Vec3): Rgb => {
    // The point's latitude and the sun's cosine and azimuth from north towards east there, as the
    // lit pass gives them to `surfaceLighting.wgsl`.
    const { latitudeRad, n, north, east } = groundFrameOf(groundM);
    const muSun = n.x * sun.x + n.y * sun.y + n.z * sun.z;
    const azimuthRad = Math.atan2(
      east.x * sun.x + east.y * sun.y + east.z * sun.z,
      north.x * sun.x + north.y * sun.y + north.z * sun.z,
    );
    const transmitted = lighting.sunTransmittance(0, muSun, latitudeRad, azimuthRad, 0);
    const sky = lighting.skyIrradiance(0, muSun, latitudeRad, 0);
    const direct = Math.max(muSun, 0);
    const albedo = drawn.groundAlbedo;
    return [
      (albedo[0] * (direct * transmitted[0] + sky[0])) / Math.PI,
      (albedo[1] * (direct * transmitted[1] + sky[1])) / Math.PI,
      (albedo[2] * (direct * transmitted[2] + sky[2])) / Math.PI,
    ];
  };

  // Each ground observer's sky view, by its observer and sun, built texel by texel as read.
  const skyViews = new Map<string, SkyViewRead>();
  const skyViewOf = (geometry: ReferenceGeometry): SkyViewRead => {
    const { observer, sun } = geometry;
    const key = `${observer.heightM},${observer.latitudeDeg},${sun.zenithDeg},${sun.azimuthDeg}`;
    let built = skyViews.get(key);
    if (built === undefined) {
      const originM = observerOf(geometry);
      const up = normalise(originM);
      const view = twinView(
        cameraAlong(originM, up, defaultFovRad),
        sunState(geometry),
        datum,
        NEAR_M,
      );
      built = {
        view,
        texelAt: sphere
          ? skyViewTexelsTwin(planet, view, sizes.skyView, sizes.skyViewSamples)
          : oblateSkyViewTexelsTwin(oblate, view, sizes.skyView, sizes.skyViewSamples),
      };
      skyViews.set(key, built);
    }
    return built;
  };

  const groundAlong = (geometry: ReferenceGeometry, direction: Vec3): TwinParts => {
    if (figureGroundDistanceM(observerOf(geometry), direction, figure) !== null) {
      throw new RangeError(
        `the twin draws no surface for a ground observer, and ${geometry.name}'s cone meets the ground`,
      );
    }
    const { view, texelAt } = skyViewOf(geometry);
    return {
      total: sphere
        ? skyViewReadTwin(texelAt, sizes.skyView, marched, view, direction)
        : oblateSkyViewReadTwin(texelAt, sizes.skyView, oblate, view, direction),
      surface: DARK,
    };
  };

  const orbitAlong = (geometry: ReferenceGeometry, direction: Vec3): TwinParts => {
    const originM = observerOf(geometry);
    const sun = sunState(geometry);
    const view = twinView(cameraAlong(originM, direction, defaultFovRad), sun, datum, NEAR_M);
    const groundM = figureGroundDistanceM(originM, direction, figure);
    const depth = groundM === null ? 0 : NEAR_M / groundM;
    const x = (TWIN_VIEWPORT.widthPx - 1) / 2;
    const y = (TWIN_VIEWPORT.heightPx - 1) / 2;
    const [r, g, b, t] = sphere
      ? rayMarchTargetTexelTwin(planet, view, sizes, x, y, depth)
      : oblateRayMarchTargetTexelTwin(oblate, view, sizes, x, y, depth);
    if (groundM === null) {
      return { total: [r, g, b], surface: DARK };
    }
    const lit = litGroundRadiance(pointAlong(originM, direction, groundM), sun.directionBodyFixed);
    const surface: Rgb = [lit[0] * t, lit[1] * t, lit[2] * t];
    return { total: [r + surface[0], g + surface[1], b + surface[2]], surface };
  };

  const aerialAlong = (
    geometry: ReferenceGeometry,
    direction: Vec3,
    targetM: number,
  ): TwinParts => {
    const originM = observerOf(geometry);
    const sun = sunState(geometry);
    const fovXRad = 2 * Math.atan(Math.tan(defaultFovRad / 2) * AERIAL_FROXEL_SCALE);
    const view = twinView(cameraAlong(originM, direction, fovXRad), sun, datum, NEAR_M);
    const size = { widthTexels: 2, heightTexels: 2, slices: sizes.aerialPerspective.slices };
    const reachM = sizes.aerialPerspectiveReachM;
    const samplesPerSlice = sizes.aerialPerspectiveSamplesPerSlice;
    const volume: TwinVolume = sphere
      ? aerialPerspectiveTwin(planet, view, size, samplesPerSlice, reachM)
      : oblateAerialPerspectiveTwin(oblate, view, size, samplesPerSlice, reachM);
    const groundM = figureGroundDistanceM(originM, direction, figure);
    const onGround = groundM !== null && groundM < targetM;
    const surfaceM = onGround ? groundM : targetM;
    if (!(surfaceM <= reachM)) {
      throw new RangeError(
        `the twin reads aerial perspective within the volume's reach alone: ${geometry.name}`,
      );
    }
    // `aerialRead.wgsl`: the slice axis at w = d ÷ reach − ½ ÷ slices, linear between slices and
    // held at the edges; the screen's centre, the mean of the four froxels; the first slice's fade.
    const slices = size.slices;
    const w = Math.min(Math.max(surfaceM / reachM - 0.5 / slices, 0), 1);
    const pz = Math.min(Math.max(w * slices - 0.5, 0), slices - 1);
    const z0 = Math.floor(pz);
    const z1 = Math.min(z0 + 1, slices - 1);
    const fz = pz - z0;
    const froxel = (slice: number, channel: number): number => {
      let sum = 0;
      for (let k = 0; k < 4; k += 1) {
        sum += volume.texels[(slice * 4 + k) * 4 + channel] ?? Number.NaN;
      }
      return sum / 4;
    };
    const at = (channel: number): number =>
      froxel(z0, channel) * (1 - fz) + froxel(z1, channel) * fz;
    const fade = Math.min(Math.max(surfaceM / (reachM / slices), 0), 1);
    const inscattered: Rgb = [at(0) * fade, at(1) * fade, at(2) * fade];
    if (!onGround) {
      return { total: inscattered, surface: DARK };
    }
    const transmittance = 1 + (at(3) - 1) * fade;
    const lit = litGroundRadiance(pointAlong(originM, direction, surfaceM), sun.directionBodyFixed);
    const surface: Rgb = [lit[0] * transmittance, lit[1] * transmittance, lit[2] * transmittance];
    return {
      total: [
        inscattered[0] + surface[0],
        inscattered[1] + surface[1],
        inscattered[2] + surface[2],
      ],
      surface,
    };
  };

  const radianceAlong = (geometry: ReferenceGeometry, direction: Vec3): TwinParts => {
    let parts: TwinParts;
    const kind = twinGeometryKind(geometry, drawn, sizes);
    switch (kind) {
      case "ground":
        parts = groundAlong(geometry, direction);
        break;
      case "orbit":
        parts = orbitAlong(geometry, direction);
        break;
      case "aerial": {
        const targetM = geometry.maxDistanceM;
        if (targetM === undefined) {
          throw new Error(`aerial perspective's geometry ${geometry.name} has a target`);
        }
        parts = aerialAlong(geometry, direction, targetM);
        break;
      }
    }
    return parts;
  };
  const halfAngleRadOf = (geometry: ReferenceGeometry): number =>
    ((geometry.detectorHalfAngleDeg ?? settings.detectorHalfAngleDeg) * Math.PI) / 180;
  const scaled = (unit: Rgb): Rgb => {
    const e = settings.sunIrradiance;
    return [unit[0] * e[0], unit[1] * e[1], unit[2] * e[2]];
  };
  // A geometry's cone, both parts averaged in one pass, kept for the surface's read.
  const cones = new Map<ReferenceGeometry, TwinParts>();
  const coneOf = (geometry: ReferenceGeometry): TwinParts => {
    let parts = cones.get(geometry);
    if (parts === undefined) {
      const surfaces: Rgb[] = [];
      const total = coneAverage(
        (direction) => {
          const along = radianceAlong(geometry, direction);
          surfaces.push(along.surface);
          return along.total;
        },
        axisOf(geometry),
        halfAngleRadOf(geometry),
      );
      // The second pass takes the same directions in the same order as the first.
      let next = 0;
      const surface = coneAverage(
        () => {
          const replayed = surfaces[next];
          if (replayed === undefined) {
            throw new Error("the cone's second pass visits more directions than its first");
          }
          next += 1;
          return replayed;
        },
        axisOf(geometry),
        halfAngleRadOf(geometry),
      );
      if (next !== surfaces.length) {
        throw new Error("the cone's second pass visits fewer directions than its first");
      }
      parts = { total: scaled(total), surface: scaled(surface) };
      cones.set(geometry, parts);
    }
    return parts;
  };

  const transmittanceAlong = (geometry: ReferenceGeometry, direction: Vec3): Rgb => {
    const originM = observerOf(geometry);
    const view = twinView(
      cameraAlong(originM, direction, defaultFovRad),
      sunState(geometry),
      datum,
      NEAR_M,
    );
    return sphere
      ? compositeTransmittanceTwin(sources, marched.topHeightM, view, direction)
      : oblateCompositeTransmittanceTwin(oblate, view, direction);
  };

  return {
    planet,
    oblate,
    surfaceLighting: lighting,
    radiance: (geometry) => coneOf(geometry).total,
    surfaceRadiance: (geometry) => coneOf(geometry).surface,
    groundConeShare: (geometry) => groundConeShare(geometry, figure, settings.detectorHalfAngleDeg),
    radianceAlong,
    litGroundRadiance,
    transmittanceAlong,
  };
}
