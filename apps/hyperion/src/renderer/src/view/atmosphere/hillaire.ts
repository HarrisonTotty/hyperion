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
 * The tables hold radiance per unit of the sun's illuminance in each channel, the smooth terms
 * evaluated at the fitted channels' wavelengths (plan R08, Design note 5, R08.T6.d); the composite
 * turns it into pre-exposed Rec. 709 luminance with the sun's per-channel illuminance, R07's
 * `starIlluminance` through R06's colour ({@link SunState.illuminanceLx}), in place of R05's
 * `solar.ts` factors, and clamps the sun's disc after pre-exposure.
 *
 * The kernels are `f32`. What they cannot form there without losing it, they are given formed in
 * `f64` and narrowed once (plan R08, Design note 18, R08.T6.g): the camera's height above the
 * datum and the datum's normal, the ray march's quadric constants, the sky view's horizon angle
 * and the sine of the sun's angular radius ({@link viewExactOf}). No kernel forms a height as a
 * difference of radii, and none takes WGSL's builtin trigonometry (`view/shaders/preciseTrig.wgsl`).
 *
 * The sky view and the ray march cut each side of their rays' split where the ray crosses one of
 * the medium's layer edges, the tables' `layerEdges` (plan R08, R08.T6.i; `tables.ts`'s
 * `packLayerEdges`), at the same counts; on a medium with none their steps are R05.T12.e's. The
 * aerial perspective keeps its even steps.
 */

import type { Vec3 } from "../../geometry/vec3";
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
   * Where aerial perspective applies: `scene`, whose volume other passes may sample
   * ({@link HillaireAtmosphere.aerialPerspectiveVolume}); or `terrain`, the low setting's, applied in
   * the one deferred pass to the terrain alone (the budget's "aerial perspective on terrain only").
   */
  readonly aerialPerspectiveScope: "scene" | "terrain";
}

/**
 * The atmosphere's sizes per quality setting (Design note 16).
 *
 * @remarks
 * High, Hillaire's code: sky-view 192 × 108 (sebh's table) with 75 steps, the aerial-perspective volume
 * 32³ reaching 32 km (Hillaire 2020, Table 2 and §5.4, and Bevy), the ray march at full resolution
 * with 32 steps. Low: sky-view 128 × 64 with 16 steps, the volume 32 × 32 × 16, the ray march at
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
    rayMarchSamples: 32,
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

/** The sun, seen from the body. */
export interface SunState {
  /** The unit direction to the sun in the body-fixed axes. */
  readonly directionBodyFixed: Vec3;
  /**
   * The sun's illuminance at the body, face-on, per linear Rec. 709 channel, lx: R07's
   * `starIlluminance(disc, distance)` (`view/lighting/illuminance.ts`), the star's photopic
   * illuminance spread over the channels by R06's colour (plan R08, Design note 5, R08.T6.d). The
   * tables hold light per unit of it, channel by channel, so it is what turns them into luminance.
   */
  readonly illuminanceLx: Rgb;
  /**
   * The disc's angular radius, rad: asin(R★ ÷ d), which the caller keeps consistent with
   * `illuminanceLx`, the two from one disc at one distance, so that the disc's luminance is the
   * star's mean luminance whatever the distance. 0 draws no disc (the comparison with Hillaire's
   * images, which have none).
   */
  readonly angularRadiusRad: number;
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
  /**
   * sin ρ of the sun's angular radius, which the composite's disc test |d × s| ≤ sin ρ takes, since
   * `f32` cannot resolve 1 − cos ρ for ρ under 3.5 × 10⁻⁴ rad.
   */
  readonly sinSunRadius: number;
}

/**
 * The view's {@link ViewExact} for a camera at `positionM` with its {@link atmosphereInputs}.
 *
 * @param topHeightM - The atmosphere's height above the datum, m.
 */
export function viewExactOf(
  positionM: Vec3,
  figure: SpheroidFigure,
  topHeightM: number,
  inputs: AtmosphereInputs,
  sunAngularRadiusRad: number,
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
    sinSunRadius: Math.sin(sunAngularRadiusRad),
  };
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

/** The view's members, in `view.wgsl`'s `AtmosphereView` and the composite's `Draw` order. */
const VIEW_MEMBERS = [
  "camera",
  "up",
  "sun",
  "skyScale",
  "sunDisc",
  "right",
  "upRay",
  "forward",
  "tables",
  "figure",
  "output",
  "exact",
] as const;

/** The composite's WGSL: `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `view.wgsl`, then its own. */
const COMPOSITE_WGSL = `${preciseTrigWgsl}\n${commonWgsl}\n${viewWgsl}\n${compositeWgsl}`;

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

/** What the atmosphere is laid over: the terrain target's colour and reversed-Z depth. */
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
   * The aerial-perspective volume for other passes to sample, or `null` on a setting that applies
   * aerial perspective to the terrain alone.
   */
  aerialPerspectiveVolume(): TextureHandle | null {
    return this.#sizes.aerialPerspectiveScope === "scene" ? this.#frame.aerial : null;
  }

  /**
   * Rebuilds the per-planet tables for a new medium, or new curves of growth for its absorbers; the
   * sun's moving never does. The frames draw the medium the tables follow
   * ({@link AtmosphereTables.medium}), so a medium they refuse is not drawn.
   *
   * @param curves - Each light's curves for the medium's absorbers, light slot 0 the drawn sun's.
   * @throws Error, RangeError or `EngineUnavailable` as {@link AtmosphereTables.setMedium}.
   */
  setMedium(medium: AtmosphereMedium, curves: AbsorberCurveSlots = NO_ABSORBER_CURVES): void {
    this.#tables.setMedium(medium, tableRadiusM(this.#figure), curves);
  }

  /**
   * Builds the frame's sky view, aerial perspective and ray march, and returns the composite draw
   * for the caller to submit, with no depth test, into a target other than the scene's.
   */
  drawFrame(view: AtmosphereCamera, sun: SunState, scene: AtmosphereScene): DrawItem {
    const frame = this.#frame;
    const rayMarch = this.#rayMarchTexture(view.viewport);
    this.#dispatchFrame(
      frame.kernels,
      { skyView: frame.skyView, aerial: frame.aerial, rayMarch },
      view,
      sun,
      scene,
    );
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
   */
  drawKernelArm(view: AtmosphereCamera, sun: SunState, scene: AtmosphereScene): FrameOutputs {
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
    this.#dispatchFrame(arm.kernels, outputs, view, sun, scene);
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

  /** Fills the view's uniform for this frame and dispatches the three per-frame kernels. */
  #dispatchFrame(
    kernels: FrameKernels,
    outputs: FrameOutputs,
    view: AtmosphereCamera,
    sun: SunState,
    scene: AtmosphereScene,
  ): void {
    const engine = this.#engine;
    const sizes = this.#sizes;
    const inputs = atmosphereInputs(view, this.#figure);
    const tables = this.#tables;
    const medium = tables.medium;
    this.#fillView(view, sun, scene, inputs, medium);
    const perFrameMedium = packMediumUniform(medium, inputs.radiusM - inputs.heightM, 1);
    const common = {
      buffers: { terms: tables.terms },
      sampled: {
        densityTables: tables.densityTables,
        phaseTables: tables.phaseTables,
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
    sun: SunState,
    scene: AtmosphereScene,
    inputs: AtmosphereInputs,
    medium: AtmosphereMedium,
  ): void {
    const set = (name: string, values: readonly [number, number, number, number]): void => {
      this.#uniforms[name]?.set(values);
    };
    const topHeightM = medium.topHeightM;
    const p = view.positionM;
    const n = inputs.normal;
    const s = sun.directionBodyFixed;
    const sunRgb = sun.illuminanceLx;
    // The disc's solid angle 2π(1 − cos ρ), as 4π sin²(ρ ÷ 2), which keeps its digits for a small
    // disc where 1 − cos ρ would not even in f64 (a sun of 10⁻⁸ rad).
    const solidAngle = 4 * Math.PI * Math.sin(sun.angularRadiusRad / 2) ** 2;
    const disc = solidAngle > 0 ? 1 / solidAngle : 0;
    const aspect = view.viewport.widthPx / view.viewport.heightPx;
    const tanX = Math.tan(view.fovXRad / 2);
    const tanY = tanX / aspect;
    const right = rotate(view.orientation, { x: 1, y: 0, z: 0 });
    const up = rotate(view.orientation, { x: 0, y: 1, z: 0 });
    const forward = rotate(view.orientation, { x: 0, y: 0, z: -1 });
    const sizes = this.#sizes;
    const exact = viewExactOf(p, this.#figure, topHeightM, inputs, sun.angularRadiusRad);
    set("camera", [p.x, p.y, p.z, inputs.heightM]);
    set("up", [n.x, n.y, n.z, inputs.radiusM - inputs.heightM]);
    set("sun", [s.x, s.y, s.z, sun.angularRadiusRad]);
    // The tables hold light per unit of the sun's illuminance in each channel (R08.T6.d), so the
    // sky, the ground the march sees and the disc all take the sun's own.
    set("skyScale", [sunRgb[0], sunRgb[1], sunRgb[2], scene.exposureScale]);
    set("sunDisc", [sunRgb[0] * disc, sunRgb[1] * disc, sunRgb[2] * disc, 0]);
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
    set("exact", [
      exact.groundQuadricM2,
      exact.topQuadricM2,
      exact.horizonAngleRad,
      exact.sinSunRadius,
    ]);
    // The composite's own: the atmosphere's height, formed in f64 (R08.T6.g), how many of the
    // medium's terms are absorbers, whose columns its transmittance to space reads (R08.T6.c), and
    // its line slots, whose pressure channels the read takes (R08.T4.d).
    set("shell", [topHeightM, absorberCount(medium), lineSlotMask(medium), 0]);
  }
}
