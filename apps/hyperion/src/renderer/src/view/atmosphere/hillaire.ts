/**
 * Hillaire's atmosphere drawn every frame (plan R05, R05.T12.c, Design note 16): the sky-view
 * table, the aerial-perspective volume and the per-pixel ray march, built from the per-planet tables
 * of `tables.ts`, and the composite that lays them over the terrain target.
 *
 * @remarks
 * The per-planet tables are built once on a sphere whose ground is the figure's mean radius R₁ =
 * (2a + c) ÷ 3, and rebuilt only when the medium changes, never when the sun moves. Every lookup
 * reads them at a point's height above the datum spheroid, with its sun and view cosines taken
 * against the spheroid's normal (Design note 16): the per-frame tables are built on the camera's
 * own sphere, of its Gaussian radius √(MN), and the ray march clips against the spheroid's shells.
 * The per-planet tables hold radiance per unit of illuminance in each channel, the smooth terms
 * evaluated at the fitted channels' wavelengths (plan R08, Design note 5, R08.T6.d). Since R08.T7
 * the per-frame tables are summed over the frame's sky sources in one march (Design note 7): up to
 * {@link MAX_SKY_SOURCES}, each weighted by its per-channel illuminance (R07's `starIlluminance`
 * through R06's colour, {@link SunState.illuminanceLx}) times the frame's pre-exposure, formed in
 * `f64` ({@link frameSourcesOf}). So they hold pre-exposed Rec. 709 luminance, as the composite's
 * output does, and the half float's range follows the exposure into twilight and moonlight
 * (`decision-r08-t6b-floor.md` §6). Each source takes its own light slot's absorber curves: its
 * transmittance, its multiple-scattering layer, its absorber depth along every view path and its
 * disc (`decision-r08-t6b-floor.md` §8). The sky view is indexed by the local azimuth about the
 * first source's horizontal ({@link skyAxisOf}). The composite multiplies what the scene holds
 * behind the sky by its transmittance to space, a disc's own light's inside it, draws the discs of
 * the sources that draw one, and keeps the scene's alpha, R07's `METER_CLASS`.
 *
 * The kernels are `f32`. What they cannot form there without losing it, they are given formed in
 * `f64` and narrowed once (plan R08, Design note 18, R08.T6.g): the camera's height above the
 * datum and the datum's normal, the ray march's quadric constants, the sky view's horizon angle
 * and each source's direction with the sine of its angular radius ({@link viewExactOf},
 * {@link frameSourcesOf}). No kernel forms a height as a difference of radii, and none takes WGSL's
 * builtin trigonometry (`view/shaders/preciseTrig.wgsl`).
 *
 * The sky view and the ray march cut each side of their rays' split where the ray crosses one of
 * the medium's layer edges, the tables' `layerEdges` (plan R08, R08.T6.i; `tables.ts`'s
 * `packLayerEdges`), at the same counts; on a medium with none their steps are R05.T12.e's. The
 * aerial perspective keeps its even steps.
 *
 * Aerial perspective (plan R08, R08.T9.a). The composite lays it over every opaque surface of the
 * scene it is given, through the volume within its 32 km reach and the ray march beyond it. Along
 * the froxels' own rays the march's in-scattering agrees with the volume's to 2% at the volume's
 * far edge (`aerialPerspective.test.ts`); between froxels the volume's bilinear read near the
 * horizon misses it by up to 3.0% at 60° across, and 10.4% at 127° (plan R08's Risks, deferred).
 * Where it applies is the setting's `aerialPerspectiveScope` (`settings.ts`'s
 * `AERIAL_PERSPECTIVE_SCOPE`): on `terrain` (low) the scene is the terrain alone and nothing else
 * takes it; on `scene` (high) every opaque pass drawn into the scene before the composite takes it
 * there, and {@link HillaireAtmosphere.aerialPerspective} publishes the volume and the
 * transmittance tables for the passes the composite does not cover. {@link HillaireAtmosphere.skyView}
 * publishes the frame's sky-view table on both.
 */

import { dot, normalise, scale as scaled, sub, type Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { rotate } from "../camera/quaternion";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type {
  ComputeHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import type { QualitySetting } from "../quality/qualitySetting";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import type { AtmosphereMedium } from "./medium";
import commonWgsl from "./shaders/common.wgsl?raw";
import aerialPerspectiveWgsl from "./shaders/aerialPerspective.wgsl?raw";
import aerialReadWgsl from "./shaders/aerialRead.wgsl?raw";
import compositeWgsl from "./shaders/composite.wgsl?raw";
import mediumWgsl from "./shaders/medium.wgsl?raw";
import rayMarchWgsl from "./shaders/rayMarch.wgsl?raw";
import skyViewWgsl from "./shaders/skyView.wgsl?raw";
import sourceWgsl from "./shaders/source.wgsl?raw";
import viewWgsl from "./shaders/view.wgsl?raw";
import {
  absorberCount,
  lineSlotMask,
  type AbsorberCurveSlots,
  AtmosphereTables,
  KERNEL_ARM_FORMAT,
  MULTI_SCATTERING_SIZE,
  NO_ABSORBER_CURVES,
  packMediumUniform,
  type TableSize,
  TRANSMITTANCE_SIZE,
  withKernelArmFormat,
} from "./tables";

/** A 3D table's size, texels. */
export interface VolumeSize {
  readonly widthTexels: number;
  readonly heightTexels: number;
  readonly slices: number;
}

/**
 * The atmosphere's table sizes and sample counts for one quality setting (Design note 16): the
 * field `SETTINGS[s].atmosphere`, which R08 generalises.
 */
export interface TableSizes {
  /** The per-planet transmittance table, the same on both settings. */
  readonly transmittance: TableSize;
  /** The per-planet multiple-scattering table, the same on both settings. */
  readonly multiScattering: TableSize;
  readonly skyView: TableSize;
  /** Steps along each sky-view ray. */
  readonly skyViewSamples: number;
  readonly aerialPerspective: VolumeSize;
  /** Steps through each slice of the aerial-perspective volume. */
  readonly aerialPerspectiveSamplesPerSlice: number;
  /** The aerial-perspective volume's reach, m. */
  readonly aerialPerspectiveReachM: number;
  /** The ray march's resolution as a fraction of the output's: 1, or 0.5 with a depth-aware upsample. */
  readonly rayMarchScale: 1 | 0.5;
  /** Steps along each ray of the per-pixel ray march. */
  readonly rayMarchSamples: number;
  /**
   * Where aerial perspective applies (plan R08, Design note 11, R08.T9.a; projected as `settings.ts`'s
   * `AERIAL_PERSPECTIVE_SCOPE`): `scene`, the high setting's, over everything opaque, the composite
   * over the opaque passes drawn before it and the volume published to the rest
   * ({@link HillaireAtmosphere.aerialPerspective}); or `terrain`, the low setting's, applied in the
   * one deferred composite to the terrain alone (the budget's "aerial perspective on terrain only").
   */
  readonly aerialPerspectiveScope: "scene" | "terrain";
}

/**
 * The atmosphere's sizes per quality setting (Design note 16).
 *
 * @remarks
 * High, Hillaire's code: sky-view 192 × 108 (sebh's table) with 75 steps, the aerial-perspective volume
 * 32³ reaching 32 km (Hillaire 2020, Table 2 and §5.4, and Bevy), the ray march at full resolution
 * with 36 steps. Low: sky-view 128 × 64 with 16 steps, the volume 32 × 32 × 16, the ray march at
 * half resolution with 16 steps, aerial perspective on the terrain alone. The per-planet tables are
 * full size on both (128 KB, rarely built).
 *
 * The sky view and the ray march place their steps toward each ray's lowest point, not evenly
 * (`marchSteps.ts`), and their counts are R05.T12.e's: its quadrature gate (`marchSteps.test.ts`)
 * holds high's within 2% of a converged march (5% for twilight rays), where even steps were up to
 * 33% off, and low's 16 and 16 no worse over the gate than even steps at the same counts. The high
 * sky view takes 75 steps, not Hillaire 2020's 30 (Table 2; addendum B of
 * `decision-r05-high-atmosphere.md`): from cameras at 60–100 km a limb ray's narrow source at its
 * tangent point needs them, where 30 erred by up to 9.0% at twilight (the band seen from them; 8.2%
 * on their limb) and 75 by 3.2%, in the twin. A count changes only through that gate, T18's
 * included. Two
 * steps a slice of the aerial-perspective volume is this module's choice; T18 revisits it with the
 * spike's measurements.
 *
 * The high ray march takes 36 steps since plan R08's R08.T7, where R05.T12.e set 32: on a medium with
 * layer edges the twilight limb ray "limb, 0.1 H, sun 90°/0°" of the slab Earth missed its 5% at 32
 * (5.63%), from the optically thick air about its lowest point, and 36 is the measured fallback that
 * holds every ray of the slab Earth's gate (`decision-r08-t6d-findings.md` §11.4). The cure that
 * adds no step, a floor on the piece next to t*, cured that ray (2.76%) but put five others over
 * (the disc at 2.80%, against 2%), and was not adopted (plan R08's Risks, "Deviations in T7, as
 * built"). It costs 12.5% more on every high ray-march pixel.
 */
export const TABLE_SIZES: Readonly<Record<QualitySetting, TableSizes>> = {
  high: {
    transmittance: TRANSMITTANCE_SIZE,
    multiScattering: MULTI_SCATTERING_SIZE,
    skyView: { widthTexels: 192, heightTexels: 108 },
    skyViewSamples: 75,
    aerialPerspective: { widthTexels: 32, heightTexels: 32, slices: 32 },
    aerialPerspectiveSamplesPerSlice: 2,
    aerialPerspectiveReachM: 32_000,
    rayMarchScale: 1,
    rayMarchSamples: 36,
    aerialPerspectiveScope: "scene",
  },
  low: {
    transmittance: TRANSMITTANCE_SIZE,
    multiScattering: MULTI_SCATTERING_SIZE,
    skyView: { widthTexels: 128, heightTexels: 64 },
    skyViewSamples: 16,
    aerialPerspective: { widthTexels: 32, heightTexels: 32, slices: 16 },
    aerialPerspectiveSamplesPerSlice: 2,
    aerialPerspectiveReachM: 32_000,
    rayMarchScale: 0.5,
    rayMarchSamples: 16,
    aerialPerspectiveScope: "terrain",
  },
};

/**
 * The datum spheroid the atmosphere stands on: R05's `BodyFigure` (planet.ts, T7.a) has these
 * fields among others, and is accepted wherever this is.
 */
export interface SpheroidFigure {
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
}

/**
 * The camera in the body-fixed axes (z along the pole), as selection's `ViewSelectionInput.camera`,
 * with its field of view and presented size.
 */
export interface AtmosphereCamera {
  /** From the body's centre, m. */
  readonly positionM: Vec3;
  /** Rotates camera axes (−z forward, +y up) into the body-fixed axes. */
  readonly orientation: Quaternion;
  /** The horizontal field of view, rad. */
  readonly fovXRad: number;
  /** The presented size, device pixels. */
  readonly viewport: ViewSize;
}

/**
 * The sources the sky's kernels hold (plan R08, Design note 7; R08.T7): four stars, this plan's
 * first high figure, kept for R07's deferred raise of `MAX_BODY_LIGHTS`, and two planetshine
 * sources. The per-frame bound is 2 + 2 sources on high and 2 + 1 on low, so a raise of R07's count
 * changes no kernel. `settings.ts` exports it, which takes `TABLE_SIZES` from here; `view.wgsl`
 * holds the same number.
 */
export const MAX_SKY_SOURCES = 6;

/**
 * The stars among {@link MAX_SKY_SOURCES}, and so the light slots a march tracks a path for: each
 * star's own absorber curves, a planetshine source taking slot 0's (Design note 7).
 * `settings.ts` exports it; `view.wgsl` holds the same number.
 */
export const MAX_SKY_LIGHT_SLOTS = 4;

/**
 * One of the sky's sources seen from the body: a star of the body's list or a planetshine source
 * (plan R08, Design note 7; R08.T7's `sources.ts` makes them from R07's arrays).
 */
export interface SunState {
  /** The unit direction to the source in the body-fixed axes. */
  readonly directionBodyFixed: Vec3;
  /**
   * The source's illuminance face-on, per linear Rec. 709 channel, lx: for a star R07's
   * `starIlluminance(disc, distance)` (`view/lighting/illuminance.ts`), the star's photopic
   * illuminance spread over the channels by R06's colour (plan R08, Design note 5, R08.T6.d); for a
   * planetshine source R07's `SecondarySource.illuminance`, carried to the camera by the inverse
   * square. The per-view tables hold each source's light per unit of the frame's scale
   * ({@link HillaireAtmosphere.drawFrame}), weighted by it channel by channel.
   */
  readonly illuminanceLx: Rgb;
  /**
   * The disc's angular radius, rad: asin(R ÷ d), which the caller keeps consistent with
   * `illuminanceLx`, the two from one disc at one distance, so that the disc's luminance is the
   * source's mean luminance whatever the distance. Inside it the composite takes the source's own
   * transmittance to space for what lies behind (R08.T7). 0 for none (the comparison with Hillaire's
   * images, which have none).
   */
  readonly angularRadiusRad: number;
  /**
   * The light slot whose absorber curves of growth its light takes (Design note 5;
   * `decision-r08-t6b-floor.md` §8): a star's rank in the body's list, 0 the brightest, and 0 for a
   * planetshine source, which takes the body's brightest star's. Absent: 0.
   */
  readonly curveSlot?: number;
  /**
   * Whether the composite draws the disc itself, a uniform disc of the source's mean luminance.
   * `false` where R06's host-disc layer draws it into the scene beneath (Design note 7), and for a
   * planetshine source, which R07 draws. Absent: drawn when `angularRadiusRad` is above 0.
   */
  readonly drawsDisc?: boolean;
}

/** A source's light slot ({@link SunState.curveSlot}), 0 when absent. */
export function curveSlotOf(sun: SunState): number {
  return sun.curveSlot ?? 0;
}

/** Whether the composite draws a source's disc ({@link SunState.drawsDisc}). */
export function drawsDiscOf(sun: SunState): boolean {
  return sun.drawsDisc ?? sun.angularRadiusRad > 0;
}

/** DN 16's lookup inputs at the camera: R08.T6.f adds the latitude and the gravity scale. */
export interface AtmosphereInputs {
  /** √(MN) + h, m: the radius the per-frame tables put the camera at. */
  readonly radiusM: number;
  /** The geodetic height above the datum, m. */
  readonly heightM: number;
  /** The datum's outward normal under the camera, which μ is measured against. */
  readonly normal: Vec3;
}

/** A point's geodetic latitude and height on a spheroid, with its Gaussian radius. */
export interface Geodetic {
  readonly latitudeRad: number;
  readonly longitudeRad: number;
  readonly heightM: number;
  readonly normal: Vec3;
  /** √(MN), m: the Gaussian radius of curvature at the latitude. */
  readonly gaussianRadiusM: number;
}

/**
 * A body-fixed point's geodetic coordinates on the spheroid (a, a, c), by the classical
 * fixed-point iteration on φ and h.
 *
 * @remarks
 * N = a ÷ √(1 − e² sin²φ) (NIMA TR8350.2, eq. 4-15) and M = a(1 − e²) ÷ (1 − e² sin²φ)^(3/2)
 * (TR8350.2, §7). On WGS 84 eight steps converge to under 10⁻⁸ m; above a flattening of about 0.04
 * they do not (4.7 mm at f = 0.05, metres at 0.1), and R08, which draws flatter bodies, replaces it
 * (with Bowring, Survey Review 23 (1976) 323, for instance).
 */
export function geodeticOf(p: Vec3, figure: SpheroidFigure): Geodetic {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const e2 = 1 - (c * c) / (a * a);
  const rho = Math.hypot(p.x, p.y);
  const longitudeRad = Math.atan2(p.y, p.x);
  let latitude = Math.atan2(p.z, rho * (1 - e2));
  let heightM = 0;
  for (let i = 0; i < 8; i += 1) {
    const sin = Math.sin(latitude);
    const n = a / Math.sqrt(1 - e2 * sin * sin);
    heightM =
      Math.abs(Math.cos(latitude)) > 1e-9
        ? rho / Math.cos(latitude) - n
        : Math.abs(p.z) / Math.abs(sin) - n * (1 - e2);
    latitude = Math.atan2(p.z, rho * (1 - (e2 * n) / (n + heightM)));
  }
  const sin = Math.sin(latitude);
  const w2 = 1 - e2 * sin * sin;
  const n = a / Math.sqrt(w2);
  const m = (a * (1 - e2)) / (w2 * Math.sqrt(w2));
  const cos = Math.cos(latitude);
  return {
    latitudeRad: latitude,
    longitudeRad,
    heightM,
    normal: { x: cos * Math.cos(longitudeRad), y: cos * Math.sin(longitudeRad), z: sin },
    gaussianRadiusM: Math.sqrt(m * n),
  };
}

/**
 * DN 16's lookup inputs at the camera: r = √(MN) + h, h the geodetic height, and the spheroid
 * normal μ is measured against. R08.T6.f widens the result with the latitude and gravity scale.
 */
export function atmosphereInputs(
  camera: Pick<AtmosphereCamera, "positionM">,
  figure: SpheroidFigure,
): AtmosphereInputs {
  const g = geodeticOf(camera.positionM, figure);
  return { radiusM: g.gaussianRadiusM + g.heightM, heightM: g.heightM, normal: g.normal };
}

/**
 * The figure's mean radius R₁ = (2a + c) ÷ 3, m: the per-planet tables' ground; the "mean
 * radius of semi-axes" of NIMA TR8350.2, Table 3.3 (6,371,008.7714 m on WGS 84).
 */
export function tableRadiusM(figure: SpheroidFigure): number {
  return (2 * figure.equatorialRadiusM + figure.polarRadiusM) / 3;
}

/**
 * The camera's height on the sky view's sphere, m: `view.wgsl`'s `skyViewHeight`, which keeps it
 * 1 m clear of the ground and of the top, where the table's horizon split degenerates.
 *
 * @param atmosphereM - The atmosphere's height above the ground, m.
 */
export function skyViewHeight(heightM: number, atmosphereM: number): number {
  return Math.min(Math.max(heightM, 1), atmosphereM - 1);
}

/**
 * What the kernels cannot form in `f32` without losing it, formed in `f64` (plan R08, Design note
 * 18): `view.wgsl`'s `AtmosphereView.exact`, which the uniform narrows once.
 */
export interface ViewExact {
  /**
   * The ray march's ground constant q_g = |S P|² − a², m², P the camera's body-fixed position and
   * S = diag(1, 1, a ÷ c). Near the ground it is a small difference of large squares, which `f32`
   * would leave without digits (0.25 m of an Earth radius); formed here it keeps about 10⁻⁹ m.
   */
  readonly groundQuadricM2: number;
  /** Its top shell's q_t = |S_t P|² − (a + H)², m², S_t = diag(1, 1, (a + H) ÷ (c + H)). */
  readonly topQuadricM2: number;
  /**
   * The sky view's β = atan2(R₀, √(h_s(2R₀ + h_s))), rad: the angle from the nadir to the horizon
   * on the sky view's sphere (sebh's `GroundToHorizonCos` is its cosine), so that the horizon's
   * zenith angle is π − β and its dip below the horizontal π/2 − β; R₀ is the camera's Gaussian
   * radius and h_s its
   * {@link skyViewHeight}. `skyViewUvToParams` and `skyViewParamsToUv` both take it, so that the
   * table's write and read share it.
   */
  readonly horizonAngleRad: number;
}

/**
 * The view's {@link ViewExact} for a camera at `positionM` with its {@link atmosphereInputs}. Each
 * source's sin ρ, which the composite's disc test |d × s| ≤ sin ρ takes since `f32` cannot resolve
 * 1 − cos ρ for ρ under 3.5 × 10⁻⁴ rad, is formed beside it ({@link frameSourcesOf}, R08.T7).
 *
 * @param topHeightM - The atmosphere's height above the datum, m.
 */
export function viewExactOf(
  positionM: Vec3,
  figure: SpheroidFigure,
  topHeightM: number,
  inputs: AtmosphereInputs,
): ViewExact {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const quadric = (equatorialM: number, polarM: number): number => {
    const z = (positionM.z * equatorialM) / polarM;
    return positionM.x ** 2 + positionM.y ** 2 + z * z - equatorialM * equatorialM;
  };
  const gaussianM = inputs.radiusM - inputs.heightM;
  const skyHeightM = skyViewHeight(inputs.heightM, topHeightM);
  return {
    groundQuadricM2: quadric(a, c),
    topQuadricM2: quadric(a + topHeightM, c + topHeightM),
    horizonAngleRad: Math.atan2(gaussianM, Math.sqrt(skyHeightM * (2 * gaussianM + skyHeightM))),
  };
}

/**
 * The sky view's x axis at a camera (plan R08, R08.T7): its azimuth origin
 * ({@link skyAzimuthOrigin}) at the first source, about which the table's local azimuth is
 * measured, so that the brightest star's vertical falls on the table's first and last columns,
 * formed in `f64`; with no source, at the zenith's own fallback.
 *
 * @param normal - The datum's unit outward normal under the camera.
 */
export function skyAxisOf(normal: Vec3, suns: ReadonlyArray<SunState>): Vec3 {
  return skyAzimuthOrigin(normal, suns[0]?.directionBodyFixed ?? normal);
}

/**
 * One of the frame's sources as the per-frame kernels and the composite take it (`view.wgsl`'s
 * `SkySource`), formed in `f64` and narrowed once (plan R08, R08.T7).
 */
export interface FrameSource {
  /** xyz its unit direction, body-fixed; w sin ρ of its angular radius, 0 for none. */
  readonly direction: readonly [number, number, number, number];
  /**
   * rgb its illuminance per channel times the frame's scale (`drawFrame`'s), the weight of its light
   * in the per-view tables; w its light slot, the path, absorber curves and multiple-scattering
   * layer its light takes (0 for every source of a medium with no absorber).
   */
  readonly weight: readonly [number, number, number, number];
  /** rgb its disc's luminance at the top of the air times the frame's scale, 0 where not drawn. */
  readonly disc: readonly [number, number, number, number];
}

/**
 * The frame's sources as the kernels take them, each light weighted by `frameScale`, the
 * per-view tables' unit (plan R08, R08.T7; `decision-r08-t6b-floor.md` §6): the frame's
 * pre-exposure, so that the tables hold pre-exposed luminance, as the composite's output is, and a
 * twilight or moonlit sky stays in the half float's normal range wherever the exposure follows it.
 *
 * @param absorbers - Whether the medium has an absorber: without one every source takes path 0,
 *   since without one the tables are the same for every light.
 * @throws RangeError for more sources than {@link MAX_SKY_SOURCES}, a light slot outside
 *   [0, {@link MAX_SKY_LIGHT_SLOTS}) or, under an absorber, one past the curves' `lightSlots`, or a
 *   frame scale that is not finite and above 0.
 */
export function frameSourcesOf(
  suns: ReadonlyArray<SunState>,
  frameScale: number,
  absorbers: boolean,
  lightSlots: number,
): FrameSource[] {
  if (suns.length > MAX_SKY_SOURCES) {
    throw new RangeError(`the sky's kernels hold ${MAX_SKY_SOURCES} sources, not ${suns.length}`);
  }
  if (!(Number.isFinite(frameScale) && frameScale > 0)) {
    throw new RangeError(`a frame's scale must be finite and above 0, not ${frameScale}`);
  }
  return suns.map((sun) => {
    const slot = curveSlotOf(sun);
    if (!(Number.isInteger(slot) && slot >= 0 && slot < MAX_SKY_LIGHT_SLOTS)) {
      throw new RangeError(
        `a source's light slot must be an integer in [0, ${MAX_SKY_LIGHT_SLOTS}), not ${slot}`,
      );
    }
    if (absorbers && slot >= lightSlots) {
      throw new RangeError(
        `a source takes light slot ${slot}, but the medium's absorbers have curves for ${lightSlots} lights`,
      );
    }
    const s = sun.directionBodyFixed;
    const e = sun.illuminanceLx;
    // The disc's solid angle 2π(1 − cos ρ), as 4π sin²(ρ ÷ 2), which keeps its digits for a small
    // disc where 1 − cos ρ would not even in f64 (a sun of 10⁻⁸ rad).
    const solidAngle = 4 * Math.PI * Math.sin(sun.angularRadiusRad / 2) ** 2;
    const disc = drawsDiscOf(sun) && solidAngle > 0 ? frameScale / solidAngle : 0;
    return {
      direction: [s.x, s.y, s.z, Math.sin(sun.angularRadiusRad)],
      weight: [e[0] * frameScale, e[1] * frameScale, e[2] * frameScale, absorbers ? slot : 0],
      disc: [e[0] * disc, e[1] * disc, e[2] * disc, 0],
    };
  });
}

/**
 * The light slots a frame's marches track a path for: one past the largest source's, at least 1
 * (slot 0, whose path gives the views' mean transmittance); 1 for a medium with no absorber.
 */
export function pathSlotsOf(sources: ReadonlyArray<FrameSource>): number {
  return Math.max(1, ...sources.map((source) => source.weight[3] + 1));
}

/**
 * The per-frame kernels: `view/shaders/preciseTrig.wgsl` (R08.T6.g), `common.wgsl`, `medium.wgsl`,
 * `view.wgsl`, `source.wgsl`, then each kernel, which declares the `medium` uniform, the `terms`
 * buffer and the `densityTables` and `phaseTables` that `medium.wgsl` and `source.wgsl` read in
 * place (R08.T6.b).
 */
function kernel(name: string, wgsl: string): KernelPair {
  return {
    name,
    reference: `${preciseTrigWgsl}\n${commonWgsl}\n${mediumWgsl}\n${viewWgsl}\n${sourceWgsl}\n${wgsl}`,
    subgroup: null,
    readback: "presentation-only",
  };
}

/** The sky-view kernel. */
export const SKY_VIEW_KERNEL = kernel("atmosphere sky view", skyViewWgsl);
/** The aerial-perspective kernel. */
export const AERIAL_PERSPECTIVE_KERNEL = kernel(
  "atmosphere aerial perspective",
  aerialPerspectiveWgsl,
);
/** The per-pixel ray-march kernel. */
export const RAY_MARCH_KERNEL = kernel("atmosphere ray march", rayMarchWgsl);

/**
 * The sources' members, `view.wgsl`'s `AtmosphereView.sources`: each source's direction, weight
 * and disc ({@link FrameSource}), source by source, as the composite's `Draw` names them.
 */
const SOURCE_MEMBERS = Array.from({ length: MAX_SKY_SOURCES }, (_, i) => [
  `sourceDirection${i}`,
  `sourceWeight${i}`,
  `sourceDisc${i}`,
]).flat();

/** The view's members, in `view.wgsl`'s `AtmosphereView` and the composite's `Draw` order. */
const VIEW_MEMBERS = [
  "camera",
  "up",
  "sky",
  "skyAxis",
  "right",
  "upRay",
  "forward",
  "tables",
  "figure",
  "output",
  "exact",
  ...SOURCE_MEMBERS,
] as const;

/**
 * The composite's WGSL: `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `view.wgsl`, the volume's
 * read (`aerialRead.wgsl`, R08.T9.a), then its own.
 */
const COMPOSITE_WGSL = `${preciseTrigWgsl}\n${commonWgsl}\n${viewWgsl}\n${aerialReadWgsl}\n${compositeWgsl}`;

/** The composite: a full-screen draw of pre-exposed luminance over the terrain target. */
export const COMPOSITE_MATERIAL: WgslMaterialSpec = {
  name: "atmosphere composite",
  displayName: "ATMOSPHERE",
  vertexWgsl: COMPOSITE_WGSL,
  fragmentWgsl: COMPOSITE_WGSL,
  uniforms: [...VIEW_MEMBERS, "shell"].map((name) => ({ name, type: "vec4f" as const })),
  samplers: [{ name: "linearClamp", filter: "linear", address: "clamp-to-edge", binding: 6 }],
  textures: [
    { name: "sceneColour", binding: 0 },
    { name: "sceneDepth", binding: 1, sampleType: "depth" },
    { name: "skyView", binding: 2 },
    { name: "aerial", binding: 3, viewDimension: "3d" },
    { name: "rayMarch", binding: 4 },
    { name: "transmittance", binding: 5 },
    // The absorbers' columns beside the transmittance, and the lights' curves of growth, f32 and
    // read by `textureLoad` (R08.T6.c).
    { name: "absorberColumns", binding: 7 },
    { name: "absorberCurves", binding: 8, sampleType: "unfilterable-float" },
    // The pressure-weighted columns beside them (R08.T4.d).
    { name: "absorberPressureColumns", binding: 9 },
    { name: "lastAbsorberPressureColumn", binding: 10 },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/**
 * What the atmosphere is laid over: the colour and reversed-Z depth of the opaque surfaces aerial
 * perspective applies to (R08.T9.a). On the `terrain` scope that is the terrain alone, as R05's
 * spike draws it and R10's lit terrain is drawn (R10.T10.b); on `scene`, every opaque pass drawn
 * into the target before the composite, with its depth. Where the depth is 0 the composite draws
 * the sky.
 */
export interface AtmosphereScene {
  readonly colour: TextureHandle;
  readonly depth: TextureHandle;
  /** The projection's near plane, m, for distances from depth. */
  readonly nearM: number;
  /** The pre-exposure scale (R02's `exposureScale` of the frame's EV100). */
  readonly exposureScale: number;
}

const STORAGE_SAMPLED =
  TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC;

/** The per-frame kernels, each with its output, as one frame dispatches them. */
interface FrameKernels {
  readonly skyView: ComputeHandle;
  readonly aerial: ComputeHandle;
  readonly rayMarch: ComputeHandle;
}

/** The per-frame tables a frame writes: the sky view, the aerial-perspective volume, the march. */
export interface FrameOutputs {
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  readonly rayMarch: TextureHandle;
}

/**
 * A view's aerial perspective as a pass other than the composite reads it (plan R08, R08.T9.a): what
 * {@link HillaireAtmosphere.aerialPerspective} publishes on the `scene` scope.
 *
 * @remarks
 * The volume is read through `shaders/aerialRead.wgsl`'s `aerialPerspectiveAt(volume, sampler,
 * uv, distanceM, reachM)` at a surface within `reachM`. That gives the light in front of the surface
 * per unit of {@link AerialPerspective.scaleCdPerM2} and its mean transmittance, so that a pass lays
 * `colour × a + rgb × scaleCdPerM2 × exposure` over its own colour as the composite does; beyond the
 * reach the composite's ray march serves the scene's own depth. The transmittance tables are read
 * together, through `common.wgsl`'s `transmittanceUnitAt` and `transmittanceThroughTables` at the
 * light's curve slot (R08.T6.c) with the line slots' pressure columns (R08.T4.d), every lookup at a
 * height above the datum and a cosine against its normal (Design note 18), as the composite's
 * `transmittanceToSpace` reads them. Until R08.T6.f the transmittance table is one κ slice; T6.f
 * widens this type with what `oblate.wgsl`'s slice lookup takes.
 *
 * The volume serves a reader within its reach only. A pass drawn after the composite has no march
 * beyond it, so an opaque surface past the reach is drawn into the scene before the composite, or
 * a later task publishes more (plan R08's Risks, "For R11").
 */
export interface AerialPerspective {
  /**
   * The volume, `rgba16float`, 3D: at each froxel's slice the light scattered into the ray from the
   * camera to the slice's far face (rgb, per unit of {@link AerialPerspective.scaleCdPerM2}) and the
   * ray's mean transmittance over the channels (a), froxel (i, j) on the ray through the screen at
   * (i, j) ÷ (size − 1), slice k reaching (k + 1) reach ÷ slices.
   */
  readonly volume: TextureHandle;
  readonly size: VolumeSize;
  /** The volume's reach, m. */
  readonly reachM: number;
  /**
   * The factor per linear Rec. 709 channel that turns the volume's rgb into luminance, cd m⁻²,
   * before pre-exposure: the frame's scale, 1 ÷ its pre-exposure since R08.T7 (R08.T9.a's
   * `scaleLx`, renamed for its unit), the tables holding pre-exposed luminance summed over the drawn
   * sources.
   */
  readonly scaleCdPerM2: Rgb;
  /**
   * The per-planet transmittance table (`tables.ts`): the optical depth of every term but the
   * absorbers in rgb, and absorber slot 0's column u ÷ U in alpha (R08.T6.c).
   */
  readonly transmittance: TextureHandle;
  /** Absorber slots 1–4's columns u ÷ U, beside the transmittance table. */
  readonly absorberColumns: TextureHandle;
  /**
   * The line slots' pressure-weighted columns per (r, μ) to the top, pressure channels 0 to 3 in
   * rgba (R08.T4.d).
   */
  readonly absorberPressureColumns: TextureHandle;
  /** Pressure channel 4's pressure-weighted column in r, one texel for a medium with fewer slots. */
  readonly lastAbsorberPressureColumn: TextureHandle;
  /**
   * The lights' curves of growth, `rgba32float`, read by `textureLoad`; light slot 0 is the drawn
   * sun's.
   */
  readonly absorberCurves: TextureHandle;
  /** How many of the medium's terms are absorbers: `transmittanceThroughTables`'s `count`. */
  readonly absorberCount: number;
  /**
   * The medium's line slots, bit k for absorber slot k (`tables.ts`'s `lineSlotMask`):
   * `transmittanceThroughTables`'s `lines`, from which it takes each slot's pressure channel.
   */
  readonly lineSlots: number;
  /** The per-planet tables' ground, R₁ = (2a + c) ÷ 3 ({@link tableRadiusM}), m. */
  readonly bottomRadiusM: number;
  /** The atmosphere's height above the datum, m. */
  readonly topHeightM: number;
}

/**
 * A view's sky-view table with what reads it (plan R08, R08.T9.a): what
 * {@link HillaireAtmosphere.skyView} publishes, for R11's sky reflected in the ocean.
 *
 * @remarks
 * The table holds the sky's radiance per unit of {@link SkyViewTable.scaleCdPerM2}, summed over the
 * frame's drawn sources (Design note 7, R08.T7), by sebh's zenith parameterisation on the camera's
 * own sphere and the local azimuth about {@link SkyViewTable.azimuthOrigin} over the whole circle.
 * A direction d is read at `view.wgsl`'s `skyViewParamsToUv(groundHitFromHeight(bottomRadiusM,
 * heightM, d · zenith), preciseAtan2(|d × zenith|, d · zenith), skyViewAzimuthUnit(d · x, d · y),
 * size, horizonAngleRad)`, x the azimuth origin and y = zenith × x, through Bruneton's texel mapping
 * (Design note 19). R08.T7's check holds the one table within 2% of per-source tables near each
 * source, so the per-sun fallback is not built.
 */
export interface SkyViewTable {
  /** The table, `rgba16float`, 2D, rgb the radiance per unit of {@link SkyViewTable.scaleCdPerM2}. */
  readonly texture: TextureHandle;
  readonly size: TableSize;
  /**
   * The factor per linear Rec. 709 channel that turns the table's rgb into luminance, cd m⁻²,
   * before pre-exposure: 1 ÷ the frame's pre-exposure since R08.T7 (R08.T9.a's `scaleLx`, renamed
   * for its unit), the table holding pre-exposed luminance.
   */
  readonly scaleCdPerM2: Rgb;
  /** The table's zenith: the datum's outward normal under the camera, body-fixed. */
  readonly zenith: Vec3;
  /**
   * The unit horizontal direction, body-fixed, of azimuth 0: towards the first source's azimuth,
   * the view's sky axis, as `view.wgsl`'s `skyFrame` takes it ({@link skyAxisOf}).
   */
  readonly azimuthOrigin: Vec3;
  /** The sphere the table is built on: the camera's Gaussian radius √(MN), m. */
  readonly bottomRadiusM: number;
  /** The camera's height on that sphere, m, kept 1 m inside the shell ({@link skyViewHeight}). */
  readonly heightM: number;
  /** β, the angle from the nadir to the horizon there, rad ({@link ViewExact.horizonAngleRad}). */
  readonly horizonAngleRad: number;
}

/**
 * What the latest frame published: its view, and the tables {@link HillaireAtmosphere.drawFrame}
 * built for it.
 */
interface DrawnFrame {
  readonly view: AtmosphereCamera;
  readonly aerial: AerialPerspective;
  readonly skyView: SkyViewTable;
}

/** Whether two cameras are one view: the same position, orientation, field of view and size. */
function sameView(a: AtmosphereCamera, b: AtmosphereCamera): boolean {
  const p = a.positionM;
  const q = b.positionM;
  const r = a.orientation;
  const s = b.orientation;
  return (
    p.x === q.x &&
    p.y === q.y &&
    p.z === q.z &&
    r.w === s.w &&
    r.x === s.x &&
    r.y === s.y &&
    r.z === s.z &&
    a.fovXRad === b.fovXRad &&
    a.viewport.widthPx === b.viewport.widthPx &&
    a.viewport.heightPx === b.viewport.heightPx
  );
}

/**
 * The sky view's azimuth 0, `view.wgsl`'s `skyFrame` x: the sun's direction made horizontal
 * against `zenith` and normalised, or, with the sun at the zenith or the nadir, the body-fixed x
 * axis (y where the zenith lies near x) made so.
 */
export function skyAzimuthOrigin(zenith: Vec3, sun: Vec3): Vec3 {
  let x = sub(sun, scaled(zenith, dot(sun, zenith)));
  if (dot(x, x) < 1e-12) {
    const axis: Vec3 = Math.abs(zenith.x) > 0.9 ? { x: 0, y: 1, z: 0 } : { x: 1, y: 0, z: 0 };
    x = sub(axis, scaled(zenith, dot(axis, zenith)));
  }
  return normalise(x);
}

/** The per-frame GPU objects of one device. */
interface FrameResources {
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  readonly kernels: FrameKernels;
  readonly composite: MaterialHandle;
  readonly triangle: MeshHandle;
  rayMarch: TextureHandle | null;
  rayMarchSize: ViewSize | null;
}

/** The kernel arm's per-frame GPU objects (R08.T6.g), made at its first frame. */
interface KernelArmResources {
  readonly kernels: FrameKernels;
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  rayMarch: TextureHandle | null;
  rayMarchSize: ViewSize | null;
}

/** A per-frame table's texture: 2D or 3D, at `format`, under `atmosphere-view`. */
function frameTexture(
  engine: RenderEngine,
  name: string,
  size: [number, number] | [number, number, number],
  format: GPUTextureFormat,
): TextureHandle {
  return engine.createTexture({
    name,
    size,
    dimension: size.length === 3 ? "3d" : "2d",
    format,
    mips: 1,
    usage: STORAGE_SAMPLED,
    category: "atmosphere-view",
  });
}

function createFrameResources(engine: RenderEngine, sizes: TableSizes): FrameResources {
  return {
    skyView: frameTexture(
      engine,
      "atmosphere sky view",
      [sizes.skyView.widthTexels, sizes.skyView.heightTexels],
      "rgba16float",
    ),
    aerial: frameTexture(
      engine,
      "atmosphere aerial perspective",
      [
        sizes.aerialPerspective.widthTexels,
        sizes.aerialPerspective.heightTexels,
        sizes.aerialPerspective.slices,
      ],
      "rgba16float",
    ),
    kernels: {
      skyView: engine.createCompute(SKY_VIEW_KERNEL),
      aerial: engine.createCompute(AERIAL_PERSPECTIVE_KERNEL),
      rayMarch: engine.createCompute(RAY_MARCH_KERNEL),
    },
    composite: engine.createMaterial(COMPOSITE_MATERIAL),
    triangle: engine.createMesh({
      name: "atmosphere composite triangle",
      positions: new Float32Array([-1, -1, 0, 3, -1, 0, -1, 3, 0]),
      indices: null,
      topology: "triangle-list",
      attributes: {},
    }),
    rayMarch: null,
    rayMarchSize: null,
  };
}

function createKernelArmResources(engine: RenderEngine, sizes: TableSizes): KernelArmResources {
  return {
    kernels: {
      skyView: engine.createCompute(withKernelArmFormat(SKY_VIEW_KERNEL)),
      aerial: engine.createCompute(withKernelArmFormat(AERIAL_PERSPECTIVE_KERNEL)),
      rayMarch: engine.createCompute(withKernelArmFormat(RAY_MARCH_KERNEL)),
    },
    skyView: frameTexture(
      engine,
      "atmosphere sky view (kernel arm)",
      [sizes.skyView.widthTexels, sizes.skyView.heightTexels],
      KERNEL_ARM_FORMAT,
    ),
    aerial: frameTexture(
      engine,
      "atmosphere aerial perspective (kernel arm)",
      [
        sizes.aerialPerspective.widthTexels,
        sizes.aerialPerspective.heightTexels,
        sizes.aerialPerspective.slices,
      ],
      KERNEL_ARM_FORMAT,
    ),
    rayMarch: null,
    rayMarchSize: null,
  };
}

/** Workgroups of 8 × 8 covering a 2D size. */
function groups8(width: number, height: number): readonly [number, number, number] {
  return [Math.ceil(width / 8), Math.ceil(height / 8), 1];
}

/**
 * Earth-like atmosphere drawn by Hillaire's four tables: the per-planet pair, rebuilt when the
 * medium changes, and the per-frame sky view, aerial perspective and ray march.
 */
export class HillaireAtmosphere {
  readonly #engine: RenderEngine;
  readonly #sizes: TableSizes;
  readonly #figure: SpheroidFigure;
  readonly #tables: AtmosphereTables;
  readonly #offRestored: () => void;
  #frame: FrameResources;
  /** The smoke's kernel arm (R08.T6.g), made at its first frame. */
  #arm: KernelArmResources | null = null;
  /** Each view member's four floats, reused every frame. */
  readonly #uniforms: Record<string, Float32Array>;
  /**
   * What the latest frame published: `null` before it, and once a restore remakes the frame or a
   * medium is set, until the next.
   */
  #drawn: DrawnFrame | null = null;

  /**
   * Makes the tables and the per-frame resources for `medium` on `figure`.
   *
   * @param tables - The setting's sizes, `SETTINGS[s].atmosphere`.
   * @param curves - Each light's curves of growth for the medium's absorbers, light slot 0 the
   *   drawn sun's (R08.T6.c); none for a medium with no absorber.
   */
  constructor(
    engine: RenderEngine,
    medium: AtmosphereMedium,
    tables: TableSizes,
    figure: SpheroidFigure,
    curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
  ) {
    this.#engine = engine;
    this.#sizes = tables;
    this.#figure = figure;
    this.#tables = new AtmosphereTables(engine, medium, tableRadiusM(figure), curves);
    this.#frame = createFrameResources(engine, tables);
    this.#uniforms = Object.fromEntries(
      [...VIEW_MEMBERS, "shell"].map((name) => [name, new Float32Array(4)]),
    );
    this.#offRestored = engine.onRestored(() => {
      this.#frame = createFrameResources(engine, tables);
      this.#arm = null;
      this.#drawn = null;
    });
  }

  /** The per-planet tables, rebuilt when the medium changes. */
  get tables(): AtmosphereTables {
    return this.#tables;
  }

  /**
   * The sky-view table, the aerial-perspective volume and the ray-march target the latest
   * {@link HillaireAtmosphere.drawFrame} built, the target `null` before the first frame, for the
   * smoke page's agreement checks (R05.T12.e, R08.T6.b). Their kernels are presentation-only, so
   * only a tolerance read-back reads them.
   */
  get frameTables(): {
    readonly skyView: TextureHandle;
    readonly aerial: TextureHandle;
    readonly rayMarch: TextureHandle | null;
  } {
    return {
      skyView: this.#frame.skyView,
      aerial: this.#frame.aerial,
      rayMarch: this.#frame.rayMarch,
    };
  }

  /**
   * The aerial perspective {@link HillaireAtmosphere.drawFrame} built for `view` this frame, for the
   * passes the composite does not cover (R08.T9.a): on the `scene` scope (high) the volume and the
   * transmittance tables; on `terrain` (low) `null`, the one deferred composite applying it to the
   * terrain alone. Widens R05's `aerialPerspectiveVolume()`.
   *
   * @throws Error, on the `scene` scope, if the latest frame was not drawn for `view` (the same
   *   position, orientation, field of view and size), or none has been since the frame's resources
   *   were made or the medium was set: a caller's bug, which would read another view's tables,
   *   released ones or none.
   */
  aerialPerspective(view: AtmosphereCamera): AerialPerspective | null {
    if (this.#sizes.aerialPerspectiveScope === "terrain") {
      return null;
    }
    return this.#drawnFor(view).aerial;
  }

  /**
   * The sky-view table {@link HillaireAtmosphere.drawFrame} built for `view` this frame, with what
   * reads it (R08.T9.a), on both settings: for R11's sky reflected in the ocean.
   *
   * @throws Error if the latest frame was not drawn for `view`, as
   *   {@link HillaireAtmosphere.aerialPerspective} does.
   */
  skyView(view: AtmosphereCamera): SkyViewTable {
    return this.#drawnFor(view).skyView;
  }

  /** The latest frame's publication, if it was drawn for `view`. */
  #drawnFor(view: AtmosphereCamera): DrawnFrame {
    const drawn = this.#drawn;
    if (drawn === null) {
      throw new Error(
        "no atmosphere frame has been drawn since its tables were made or its medium was set",
      );
    }
    if (!sameView(drawn.view, view)) {
      throw new Error("the latest atmosphere frame was drawn for another view");
    }
    return drawn;
  }

  /**
   * Rebuilds the per-planet tables for a new medium, or new curves of growth for its absorbers; the
   * sun's moving never does. The frames draw the medium the tables follow
   * ({@link AtmosphereTables.medium}), so a medium they refuse is not drawn. The latest frame's
   * publication ({@link HillaireAtmosphere.aerialPerspective}, {@link HillaireAtmosphere.skyView})
   * is withdrawn until the next frame, since its tables may have been remade.
   *
   * @param curves - Each light's curves for the medium's absorbers, light slot 0 the drawn sun's.
   * @throws Error, RangeError or `EngineUnavailable` as {@link AtmosphereTables.setMedium}.
   */
  setMedium(medium: AtmosphereMedium, curves: AbsorberCurveSlots = NO_ABSORBER_CURVES): void {
    this.#drawn = null;
    this.#tables.setMedium(medium, tableRadiusM(this.#figure), curves);
  }

  /**
   * Builds the frame's sky view, aerial perspective and ray march, summed over the drawn sources
   * in one march along each ray (Design note 7, R08.T7), and returns the composite draw for the
   * caller to submit, with no depth test, into a target other than the scene's.
   *
   * @remarks
   * The per-view tables hold each source's light weighted by its illuminance times the frame's
   * pre-exposure, `scene.exposureScale` ({@link frameSourcesOf}): pre-exposed luminance, as the
   * composite's output is (`decision-r08-t6b-floor.md` §6), so that a twilight or moonlit sky
   * stays in the half float's normal range wherever the exposure follows it. Their bytes do not
   * grow with the sources. Each source's light takes its own light slot's absorber curves along
   * its path to each scattering point and along the view's own path, and its own
   * multiple-scattering layer under a medium with an absorber (`decision-r08-t6b-floor.md` §8).
   *
   * @param suns - The drawn sources, at most {@link MAX_SKY_SOURCES}: `sources.ts`'s `skySources`
   *   for a body, or a caller's own; none draws a sky of nothing but what lies behind it.
   * @throws RangeError as {@link frameSourcesOf}: too many sources, a light slot the medium's
   *   curves do not hold, or a pre-exposure that is not finite and above 0.
   */
  drawFrame(
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
  ): DrawItem {
    const frame = this.#frame;
    const rayMarch = this.#rayMarchTexture(view.viewport);
    const { inputs, exact } = this.#dispatchFrame(
      frame.kernels,
      { skyView: frame.skyView, aerial: frame.aerial, rayMarch },
      view,
      suns,
      scene,
    );
    this.#drawn = this.#published(view, suns, scene, inputs, exact);
    return {
      mesh: frame.triangle,
      material: frame.composite,
      offsetFromCameraM: new Float32Array(3),
      uniforms: this.#uniforms,
      textures: {
        sceneColour: scene.colour,
        sceneDepth: scene.depth,
        skyView: frame.skyView,
        aerial: frame.aerial,
        rayMarch,
        transmittance: this.#tables.transmittance,
        absorberColumns: this.#tables.absorberColumns,
        absorberCurves: this.#tables.absorberCurves,
        absorberPressureColumns: this.#tables.absorberPressureColumns,
        lastAbsorberPressureColumn: this.#tables.lastAbsorberPressureColumn,
      },
    };
  }

  /**
   * The frame's sky view, aerial perspective and ray march built again by their kernels assembled
   * to write `rgba32float` ({@link withKernelArmFormat}), over the same tables, view and scene: the
   * smoke's kernel arm (plan R08, R08.T6.g; `decision-r08-t6b-floor.md` §4), which holds each
   * kernel's own `f32` output to its twin with no half-float floor. Nothing is drawn; the composite
   * reads only {@link HillaireAtmosphere.drawFrame}'s tables. The kernels and targets are made at
   * the first call, and the ray march's anew when the viewport changes.
   *
   * @returns The arm's three `rgba32float` targets, for a tolerance read-back.
   * @throws RangeError as {@link HillaireAtmosphere.drawFrame}.
   */
  drawKernelArm(
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
  ): FrameOutputs {
    const engine = this.#engine;
    this.#arm ??= createKernelArmResources(engine, this.#sizes);
    const arm = this.#arm;
    const size = arm.rayMarchSize;
    if (
      arm.rayMarch === null ||
      size === null ||
      size.widthPx !== view.viewport.widthPx ||
      size.heightPx !== view.viewport.heightPx
    ) {
      if (arm.rayMarch !== null) {
        engine.releaseTexture(arm.rayMarch);
      }
      const scale = this.#sizes.rayMarchScale;
      arm.rayMarch = frameTexture(
        engine,
        "atmosphere ray march (kernel arm)",
        [
          Math.max(1, Math.ceil(view.viewport.widthPx * scale)),
          Math.max(1, Math.ceil(view.viewport.heightPx * scale)),
        ],
        KERNEL_ARM_FORMAT,
      );
      arm.rayMarchSize = view.viewport;
    }
    const outputs = { skyView: arm.skyView, aerial: arm.aerial, rayMarch: arm.rayMarch };
    this.#dispatchFrame(arm.kernels, outputs, view, suns, scene);
    return outputs;
  }

  /**
   * Stops following the engine's restores, and releases the kernel arm's targets if it drew one
   * (the smoke's, R08.T6.g). The frame's own textures are left to the engine, as before.
   */
  dispose(): void {
    this.#offRestored();
    this.#tables.dispose();
    const arm = this.#arm;
    if (arm !== null) {
      for (const texture of [arm.skyView, arm.aerial, arm.rayMarch]) {
        if (texture !== null) {
          this.#engine.releaseTexture(texture);
        }
      }
      this.#arm = null;
    }
  }

  /**
   * What a frame publishes for `view` (R08.T9.a): its volume, its sky view and their tables, each
   * per unit of the frame's scale (R08.T7): the tables hold pre-exposed luminance, so the factor
   * to luminance is 1 ÷ the pre-exposure in each channel.
   */
  #published(
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
    inputs: AtmosphereInputs,
    exact: ViewExact,
  ): DrawnFrame {
    const frame = this.#frame;
    const sizes = this.#sizes;
    const tables = this.#tables;
    const medium = tables.medium;
    const unit = 1 / scene.exposureScale;
    const scaleCdPerM2: Rgb = [unit, unit, unit];
    return {
      view,
      aerial: {
        volume: frame.aerial,
        size: sizes.aerialPerspective,
        reachM: sizes.aerialPerspectiveReachM,
        scaleCdPerM2,
        transmittance: tables.transmittance,
        absorberColumns: tables.absorberColumns,
        absorberPressureColumns: tables.absorberPressureColumns,
        lastAbsorberPressureColumn: tables.lastAbsorberPressureColumn,
        absorberCurves: tables.absorberCurves,
        absorberCount: absorberCount(medium),
        lineSlots: lineSlotMask(medium),
        bottomRadiusM: tableRadiusM(this.#figure),
        topHeightM: medium.topHeightM,
      },
      skyView: {
        texture: frame.skyView,
        size: sizes.skyView,
        scaleCdPerM2,
        zenith: inputs.normal,
        azimuthOrigin: skyAxisOf(inputs.normal, suns),
        bottomRadiusM: inputs.radiusM - inputs.heightM,
        heightM: skyViewHeight(inputs.heightM, medium.topHeightM),
        horizonAngleRad: exact.horizonAngleRad,
      },
    };
  }

  /**
   * Fills the view's uniform for this frame and dispatches the three per-frame kernels.
   *
   * @returns The camera's lookup inputs, and what was formed for it in `f64`.
   */
  #dispatchFrame(
    kernels: FrameKernels,
    outputs: FrameOutputs,
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
  ): { readonly inputs: AtmosphereInputs; readonly exact: ViewExact } {
    const engine = this.#engine;
    const sizes = this.#sizes;
    const inputs = atmosphereInputs(view, this.#figure);
    const tables = this.#tables;
    const medium = tables.medium;
    const exact = this.#fillView(view, suns, scene, inputs, medium);
    const perFrameMedium = packMediumUniform(medium, inputs.radiusM - inputs.heightM, 1);
    const common = {
      buffers: { terms: tables.terms },
      sampled: {
        densityTables: tables.densityTables,
        phaseTables: tables.phaseTables,
        lobeTables: tables.lobeTables,
        transmittance: tables.transmittance,
        multiScattering: tables.multiScattering,
        absorberColumns: tables.absorberColumns,
        absorberCurves: tables.absorberCurves,
        absorberPressureColumns: tables.absorberPressureColumns,
        lastAbsorberPressureColumn: tables.lastAbsorberPressureColumn,
      },
    };
    const viewUniforms = this.#viewBlock();
    engine.dispatch(
      kernels.skyView,
      {
        ...common,
        uniforms: { medium: perFrameMedium, view: viewUniforms, layerEdges: tables.layerEdges },
        storage: { skyViewOut: { texture: outputs.skyView, level: 0 } },
      },
      groups8(sizes.skyView.widthTexels, sizes.skyView.heightTexels),
      "atmosphere view",
    );
    engine.dispatch(
      kernels.aerial,
      {
        ...common,
        uniforms: { medium: perFrameMedium, view: viewUniforms },
        storage: { aerialOut: { texture: outputs.aerial, level: 0 } },
      },
      groups8(sizes.aerialPerspective.widthTexels, sizes.aerialPerspective.heightTexels),
      "atmosphere view",
    );
    engine.dispatch(
      kernels.rayMarch,
      {
        buffers: common.buffers,
        sampled: { ...common.sampled, sceneDepth: scene.depth },
        uniforms: { medium: perFrameMedium, view: viewUniforms, layerEdges: tables.layerEdges },
        storage: { rayMarchOut: { texture: outputs.rayMarch, level: 0 } },
      },
      groups8(
        Math.ceil(view.viewport.widthPx * sizes.rayMarchScale),
        Math.ceil(view.viewport.heightPx * sizes.rayMarchScale),
      ),
      "atmosphere view",
    );
    return { inputs, exact };
  }

  /**
   * The ray-march target at the setting's scale of the largest output so far: it grows on a
   * resize and is never remade smaller, so that a window resized back and forth makes nothing new;
   * the kernel and the composite address it by the current output's size.
   */
  #rayMarchTexture(viewport: ViewSize): TextureHandle {
    const frame = this.#frame;
    const fits =
      frame.rayMarchSize !== null &&
      frame.rayMarchSize.widthPx >= viewport.widthPx &&
      frame.rayMarchSize.heightPx >= viewport.heightPx;
    if (frame.rayMarch !== null && fits) {
      return frame.rayMarch;
    }
    const grown: ViewSize = {
      widthPx: Math.max(viewport.widthPx, frame.rayMarchSize?.widthPx ?? 0),
      heightPx: Math.max(viewport.heightPx, frame.rayMarchSize?.heightPx ?? 0),
    };
    const scale = this.#sizes.rayMarchScale;
    frame.rayMarch = frameTexture(
      this.#engine,
      "atmosphere ray march",
      [
        Math.max(1, Math.ceil(grown.widthPx * scale)),
        Math.max(1, Math.ceil(grown.heightPx * scale)),
      ],
      "rgba16float",
    );
    frame.rayMarchSize = grown;
    return frame.rayMarch;
  }

  /** The view's members as one `AtmosphereView` block, for the kernels. */
  #viewBlock(): Float32Array {
    const block = new Float32Array(VIEW_MEMBERS.length * 4);
    for (const [i, name] of VIEW_MEMBERS.entries()) {
      const member = this.#uniforms[name];
      if (member !== undefined) {
        block.set(member, i * 4);
      }
    }
    return block;
  }

  #fillView(
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
    inputs: AtmosphereInputs,
    medium: AtmosphereMedium,
  ): ViewExact {
    const set = (name: string, values: readonly [number, number, number, number]): void => {
      this.#uniforms[name]?.set(values);
    };
    const topHeightM = medium.topHeightM;
    const p = view.positionM;
    const n = inputs.normal;
    const absorbers = absorberCount(medium) > 0;
    // The per-view tables' unit is the frame's pre-exposure (`decision-r08-t6b-floor.md` §6): each
    // source's weight is its illuminance times it, so the tables hold pre-exposed luminance and the
    // composite's factor from them, the scale F = 1 ÷ exposureScale times the exposure, is 1.
    const sources = frameSourcesOf(
      suns,
      scene.exposureScale,
      absorbers,
      this.#tables.curves.length,
    );
    const axis = skyAxisOf(n, suns);
    const aspect = view.viewport.widthPx / view.viewport.heightPx;
    const tanX = Math.tan(view.fovXRad / 2);
    const tanY = tanX / aspect;
    const right = rotate(view.orientation, { x: 1, y: 0, z: 0 });
    const up = rotate(view.orientation, { x: 0, y: 1, z: 0 });
    const forward = rotate(view.orientation, { x: 0, y: 0, z: -1 });
    const sizes = this.#sizes;
    const exact = viewExactOf(p, this.#figure, topHeightM, inputs);
    set("camera", [p.x, p.y, p.z, inputs.heightM]);
    set("up", [n.x, n.y, n.z, inputs.radiusM - inputs.heightM]);
    set("sky", [sources.length, pathSlotsOf(sources), this.#tables.multiScatteringLayers, 1]);
    set("skyAxis", [axis.x, axis.y, axis.z, 0]);
    for (let i = 0; i < MAX_SKY_SOURCES; i += 1) {
      const source = sources[i];
      set(`sourceDirection${i}`, source?.direction ?? [0, 0, 1, 0]);
      set(`sourceWeight${i}`, source?.weight ?? [0, 0, 0, 0]);
      set(`sourceDisc${i}`, source?.disc ?? [0, 0, 0, 0]);
    }
    set("right", [right.x * tanX, right.y * tanX, right.z * tanX, 0]);
    set("upRay", [up.x * tanY, up.y * tanY, up.z * tanY, 0]);
    set("forward", [forward.x, forward.y, forward.z, 0]);
    set("tables", [
      tableRadiusM(this.#figure),
      sizes.aerialPerspectiveReachM,
      sizes.aerialPerspective.slices,
      sizes.aerialPerspectiveSamplesPerSlice,
    ]);
    set("figure", [
      this.#figure.equatorialRadiusM,
      this.#figure.polarRadiusM,
      scene.nearM,
      sizes.rayMarchSamples,
    ]);
    set("output", [
      view.viewport.widthPx,
      view.viewport.heightPx,
      sizes.rayMarchScale,
      sizes.skyViewSamples,
    ]);
    set("exact", [exact.groundQuadricM2, exact.topQuadricM2, exact.horizonAngleRad, 0]);
    // The composite's own: the atmosphere's height, formed in f64 (R08.T6.g), how many of the
    // medium's terms are absorbers, whose columns its transmittance to space reads (R08.T6.c), and
    // its line slots, whose pressure channels the read takes (R08.T4.d).
    set("shell", [topHeightM, absorberCount(medium), lineSlotMask(medium), 0]);
    return exact;
  }
}
