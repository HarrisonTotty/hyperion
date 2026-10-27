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
 * of its radiance; neither is gated.
 *
 * Every radiance is diffuse alone, per unit of the sun's irradiance (each source's weight 1,
 * `twinView`), then times {@link TwinOptions.sunIrradiance}: the unit of the reference's
 * `radiance`. The tables are built once a medium, as one slice and one band of an oblate body's
 * (`oblatePlanetTwin` of `kernelMedium(medium)` on the sphere, with no spin: its band's tables are
 * `planetTwin`'s), and read at {@link TwinOptions.sizes}, `TABLE_SIZES.high` by default, as the
 * twin's gate tests take them.
 *
 * Built for a sphere ({@link TwinFigure}). R08.T13.c1 adds what its cases need: a spheroid's
 * figure for `saturn-oblate` (the oblate marches and the sliced lookups over its slices and bands),
 * and, for `earth-ozone-slab`, the layer split and the pieces as options.
 */

import { normalise, type Vec3 } from "../../../geometry/vec3";
import { DEFAULT_FOV_DEG } from "../../camera/projection";
import { lookAlong } from "../../camera/quaternion";
import type { Rgb } from "../../photometry/toneCurve";
import { coneAverage } from "../gate";
import {
  type AtmosphereCamera,
  marchWeightOf,
  type SunState,
  TABLE_SIZES,
  type TableSizes,
} from "../hillaire";
import type { AtmosphereMedium } from "../medium";
import { bodyGravity } from "../oblate";
import type { SurfaceLightingReader } from "../surfaceLighting";
import {
  type AbsorberCurveSlots,
  IRRADIANCE_SIZE,
  kernelMedium,
  NO_ABSORBER_CURVES,
} from "../tables";
import {
  aerialPerspectiveTwin,
  IRRADIANCE_KERNEL_OPTIONS,
  OBLATE_KERNEL_SIZES,
  oblateIrradianceTwin,
  oblatePlanetTwin,
  type PlanetTwin,
  rayMarchTargetTexelTwin,
  skyViewReadTwin,
  skyViewTexelsTwin,
  surfaceLightingTwin,
  twinView,
  type TwinView,
} from "../tablesCpu";
import {
  geodeticPoint,
  localDirectionVector,
  REFERENCE_DETECTOR_HALF_ANGLE_DEG,
  type ReferenceGeometry,
} from "./geometries";

/** The figure a twin's case stands on: a sphere of the ground's radius (the module's remarks). */
export interface TwinFigure {
  readonly kind: "sphere";
  /** The ground's radius, m: the tables' `bottomRadiusM`. */
  readonly radiusM: number;
}

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
}

/** The options a twin takes where its caller gives none. */
export const DEFAULT_TWIN_OPTIONS: TwinOptions = {
  sizes: TABLE_SIZES.high,
  detectorHalfAngleDeg: REFERENCE_DETECTOR_HALF_ANGLE_DEG,
  sunIrradiance: [1, 1, 1],
  curves: NO_ABSORBER_CURVES,
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

/** How a geometry is drawn (the module's remarks). */
export type TwinGeometryKind = "ground" | "orbit" | "aerial";

/**
 * How the twin draws a geometry of `medium`: `aerial` for a path to a target, `orbit` for an
 * observer at or above the top, `ground` for one under the march band.
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
  const spheroid = { equatorialRadiusM: figure.radiusM, polarRadiusM: figure.radiusM };
  const originM = geodeticPoint(spheroid, geometry.observer.latitudeDeg, geometry.observer.heightM);
  const limitM = geometry.maxDistanceM ?? Number.POSITIVE_INFINITY;
  const meets = (direction: Vec3): Rgb => {
    const groundM = groundDistanceM(originM, direction, figure.radiusM);
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
  /** The per-planet tables the marches read. */
  readonly planet: PlanetTwin;
  /** The ground's lighting, `surfaceLighting.wgsl`'s lookups on the same tables. */
  readonly surfaceLighting: SurfaceLightingReader;
  /** The twin's diffuse radiance at a geometry, averaged over the cone (the module's remarks). */
  readonly radiance: (geometry: ReferenceGeometry) => Rgb;
  /** The drawn surface's part of {@link ReferenceTwin.radiance}: 0 where no ray meets the ground. */
  readonly surfaceRadiance: (geometry: ReferenceGeometry) => Rgb;
  /**
   * The share of a geometry's cone, by the cone average's own weights, whose directions meet the
   * ground within the view: before the target for aerial perspective.
   */
  readonly groundConeShare: (geometry: ReferenceGeometry) => number;
  /** The radiance along one direction, body-fixed, per unit of the sun's irradiance. */
  readonly radianceAlong: (geometry: ReferenceGeometry, direction: Vec3) => TwinParts;
  /**
   * The lit ground's radiance at a point of the datum, per unit of the sun's irradiance (the
   * module's remarks), the sun along `sun`.
   */
  readonly litGroundRadiance: (groundM: Vec3, sun: Vec3) => Rgb;
}

/**
 * The twin of a medium on a figure (the module's remarks): its tables built once, each sun's sky
 * view as it is first read.
 *
 * @throws RangeError as `oblatePlanetTwin`, for a medium the tables refuse.
 */
export function referenceTwin(
  medium: AtmosphereMedium,
  figure: TwinFigure,
  options: Partial<TwinOptions> = {},
): ReferenceTwin {
  const settings: TwinOptions = { ...DEFAULT_TWIN_OPTIONS, ...options };
  const drawn = kernelMedium(medium);
  const spheroid = { equatorialRadiusM: figure.radiusM, polarRadiusM: figure.radiusM };
  // A sphere with no spin: one slice and one band, s ≡ 1, whatever the bulk gravity, which is read
  // nowhere (`hillaire.ts`'s `bodyOfFigure`).
  const body = {
    figure: spheroid,
    gravity: bodyGravity(
      {
        figure: spheroid,
        massKg: 1,
        angularVelocityRadS: null,
        bulkGravityMS2: 9.806_65,
        figureLaw: null,
      },
      figure.radiusM,
    ),
  };
  const oblate = oblatePlanetTwin(drawn, body, settings.curves, OBLATE_KERNEL_SIZES);
  const [band] = oblate.bands;
  const [first, ...more] = band?.multiScatteringBySlot ?? [];
  if (band === undefined || first === undefined) {
    throw new Error("a sphere's tables have one band, with a multiple-scattering layer");
  }
  const planet: PlanetTwin = {
    medium: drawn,
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

  const observerOf = (geometry: ReferenceGeometry): Vec3 =>
    geodeticPoint(spheroid, geometry.observer.latitudeDeg, geometry.observer.heightM);
  const cameraAlong = (originM: Vec3, direction: Vec3, fovXRad: number): AtmosphereCamera => ({
    positionM: originM,
    orientation: lookAlong(direction, normalise(originM)),
    fovXRad,
    viewport: TWIN_VIEWPORT,
  });
  const defaultFovRad = (DEFAULT_FOV_DEG * Math.PI) / 180;

  const litGroundRadiance = (groundM: Vec3, sun: Vec3): Rgb => {
    // The point's latitude and the sun's cosine and azimuth from north towards east there, on the
    // sphere, as the lit pass gives them to `surfaceLighting.wgsl`.
    const n = normalise(groundM);
    const latitudeRad = Math.asin(Math.min(Math.max(n.z, -1), 1));
    const longitudeRad = Math.atan2(n.y, n.x);
    const north = {
      x: -Math.sin(latitudeRad) * Math.cos(longitudeRad),
      y: -Math.sin(latitudeRad) * Math.sin(longitudeRad),
      z: Math.cos(latitudeRad),
    };
    const east = { x: -Math.sin(longitudeRad), y: Math.cos(longitudeRad), z: 0 };
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
        spheroid,
        NEAR_M,
      );
      built = {
        view,
        texelAt: skyViewTexelsTwin(planet, view, sizes.skyView, sizes.skyViewSamples),
      };
      skyViews.set(key, built);
    }
    return built;
  };

  const groundAlong = (geometry: ReferenceGeometry, direction: Vec3): TwinParts => {
    const { view, texelAt } = skyViewOf(geometry);
    return {
      total: skyViewReadTwin(texelAt, sizes.skyView, drawn, view, direction),
      surface: DARK,
    };
  };

  const orbitAlong = (geometry: ReferenceGeometry, direction: Vec3): TwinParts => {
    const originM = observerOf(geometry);
    const sun = sunState(geometry);
    const view = twinView(cameraAlong(originM, direction, defaultFovRad), sun, spheroid, NEAR_M);
    const groundM = groundDistanceM(originM, direction, figure.radiusM);
    const depth = groundM === null ? 0 : NEAR_M / groundM;
    const x = (TWIN_VIEWPORT.widthPx - 1) / 2;
    const y = (TWIN_VIEWPORT.heightPx - 1) / 2;
    const [r, g, b, t] = rayMarchTargetTexelTwin(planet, view, sizes, x, y, depth);
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
    const view = twinView(cameraAlong(originM, direction, fovXRad), sun, spheroid, NEAR_M);
    const size = { widthTexels: 2, heightTexels: 2, slices: sizes.aerialPerspective.slices };
    const reachM = sizes.aerialPerspectiveReachM;
    const volume = aerialPerspectiveTwin(
      planet,
      view,
      size,
      sizes.aerialPerspectiveSamplesPerSlice,
      reachM,
    );
    const groundM = groundDistanceM(originM, direction, figure.radiusM);
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

  return {
    planet,
    surfaceLighting: lighting,
    radiance: (geometry) => coneOf(geometry).total,
    surfaceRadiance: (geometry) => coneOf(geometry).surface,
    groundConeShare: (geometry) => groundConeShare(geometry, figure, settings.detectorHalfAngleDeg),
    radianceAlong,
    litGroundRadiance,
  };
}
