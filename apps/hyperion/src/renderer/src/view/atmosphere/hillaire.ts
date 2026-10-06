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
 * The tables hold radiance per unit spectral solar irradiance at the channels' wavelengths; the
 * composite turns it into pre-exposed Rec. 709 luminance with `solar.ts`'s sky factors and clamps
 * the sun's disc after pre-exposure.
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
import type { QualitySetting } from "../quality/qualitySetting";
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
  SKY_SPECTRAL_TO_LUMINANCE,
  skyLuminanceScale,
  SUN_SPECTRAL_TO_LUMINANCE,
  sunIlluminanceRgb,
} from "./solar";
import {
  AtmosphereTables,
  MULTI_SCATTERING_SIZE,
  packMedium,
  type TableSize,
  TRANSMITTANCE_SIZE,
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
 * High, Hillaire's code: sky-view 192 × 108 with 30 steps (sebh's), the aerial-perspective volume
 * 32³ reaching 32 km (Hillaire 2020, Table 2 and §5.4, and Bevy), the ray march at full resolution
 * with 32 steps. Low: sky-view 128 × 64 with 16 steps, the volume 32 × 32 × 16, the ray march at
 * half resolution with 16 steps, aerial perspective on the terrain alone. The per-planet tables are
 * full size on both (128 KB, rarely built). The per-slice and ray-march step counts are this
 * module's choices; T18 revisits them with the spike's measurements.
 */
export const TABLE_SIZES: Readonly<Record<QualitySetting, TableSizes>> = {
  high: {
    transmittance: TRANSMITTANCE_SIZE,
    multiScattering: MULTI_SCATTERING_SIZE,
    skyView: { widthTexels: 192, heightTexels: 108 },
    skyViewSamples: 30,
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
  /** The sun's distance, au; its illuminance scales as its inverse square. */
  readonly distanceAu: number;
  /**
   * The disc's angular radius, rad: asin(R★ ÷ d), which the caller keeps consistent with
   * `distanceAu`, so that the disc's luminance does not change with distance. 0 draws no disc (the
   * comparison with Hillaire's images, which have none).
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
 * The per-frame kernels: `common.wgsl`, `medium.wgsl`, `view.wgsl`, `source.wgsl`, then each
 * kernel, which declares the `medium` uniform that `medium.wgsl` and `source.wgsl` read in place.
 */
function kernel(name: string, wgsl: string): KernelPair {
  return {
    name,
    reference: `${commonWgsl}\n${mediumWgsl}\n${viewWgsl}\n${sourceWgsl}\n${wgsl}`,
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
  "sunOverSky",
] as const;

/** The composite: a full-screen draw of pre-exposed luminance over the terrain target. */
export const COMPOSITE_MATERIAL: WgslMaterialSpec = {
  name: "atmosphere composite",
  displayName: "ATMOSPHERE",
  vertexWgsl: `${commonWgsl}\n${viewWgsl}\n${compositeWgsl}`,
  fragmentWgsl: `${commonWgsl}\n${viewWgsl}\n${compositeWgsl}`,
  uniforms: [...VIEW_MEMBERS, "shell"].map((name) => ({ name, type: "vec4f" as const })),
  samplers: [{ name: "linearClamp", filter: "linear", address: "clamp-to-edge", binding: 6 }],
  textures: [
    { name: "sceneColour", binding: 0 },
    { name: "sceneDepth", binding: 1, sampleType: "depth" },
    { name: "skyView", binding: 2 },
    { name: "aerial", binding: 3, viewDimension: "3d" },
    { name: "rayMarch", binding: 4 },
    { name: "transmittance", binding: 5 },
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

/** The per-frame GPU objects of one device. */
interface FrameResources {
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  readonly skyViewKernel: ComputeHandle;
  readonly aerialKernel: ComputeHandle;
  readonly rayMarchKernel: ComputeHandle;
  readonly composite: MaterialHandle;
  readonly triangle: MeshHandle;
  rayMarch: TextureHandle | null;
  rayMarchSize: ViewSize | null;
}

function createFrameResources(engine: RenderEngine, sizes: TableSizes): FrameResources {
  return {
    skyView: engine.createTexture({
      name: "atmosphere sky view",
      size: [sizes.skyView.widthTexels, sizes.skyView.heightTexels],
      dimension: "2d",
      format: "rgba16float",
      mips: 1,
      usage: STORAGE_SAMPLED,
      category: "atmosphere-view",
    }),
    aerial: engine.createTexture({
      name: "atmosphere aerial perspective",
      size: [
        sizes.aerialPerspective.widthTexels,
        sizes.aerialPerspective.heightTexels,
        sizes.aerialPerspective.slices,
      ],
      dimension: "3d",
      format: "rgba16float",
      mips: 1,
      usage: STORAGE_SAMPLED,
      category: "atmosphere-view",
    }),
    skyViewKernel: engine.createCompute(SKY_VIEW_KERNEL),
    aerialKernel: engine.createCompute(AERIAL_PERSPECTIVE_KERNEL),
    rayMarchKernel: engine.createCompute(RAY_MARCH_KERNEL),
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
  #medium: AtmosphereMedium;
  #frame: FrameResources;
  /** Each view member's four floats, reused every frame. */
  readonly #uniforms: Record<string, Float32Array>;

  /**
   * Makes the tables and the per-frame resources for `medium` on `figure`.
   *
   * @param tables - The setting's sizes, `SETTINGS[s].atmosphere`.
   */
  constructor(
    engine: RenderEngine,
    medium: AtmosphereMedium,
    tables: TableSizes,
    figure: SpheroidFigure,
  ) {
    this.#engine = engine;
    this.#sizes = tables;
    this.#figure = figure;
    this.#medium = medium;
    this.#tables = new AtmosphereTables(engine, medium, tableRadiusM(figure));
    this.#frame = createFrameResources(engine, tables);
    this.#uniforms = Object.fromEntries(
      [...VIEW_MEMBERS, "shell"].map((name) => [name, new Float32Array(4)]),
    );
    this.#offRestored = engine.onRestored(() => {
      this.#frame = createFrameResources(engine, tables);
    });
  }

  /** The per-planet tables, rebuilt when the medium changes. */
  get tables(): AtmosphereTables {
    return this.#tables;
  }

  /**
   * The aerial-perspective volume for other passes to sample, or `null` on a setting that applies
   * aerial perspective to the terrain alone.
   */
  aerialPerspectiveVolume(): TextureHandle | null {
    return this.#sizes.aerialPerspectiveScope === "scene" ? this.#frame.aerial : null;
  }

  /** Rebuilds the per-planet tables for a new medium; the sun never does. */
  setMedium(medium: AtmosphereMedium): void {
    this.#medium = medium;
    this.#tables.setMedium(medium, tableRadiusM(this.#figure));
  }

  /**
   * Builds the frame's sky view, aerial perspective and ray march, and returns the composite draw
   * for the caller to submit, with no depth test, into a target other than the scene's.
   */
  drawFrame(view: AtmosphereCamera, sun: SunState, scene: AtmosphereScene): DrawItem {
    const engine = this.#engine;
    const frame = this.#frame;
    const sizes = this.#sizes;
    const inputs = atmosphereInputs(view, this.#figure);
    const u = this.#uniforms;
    this.#fillView(view, sun, scene, inputs);

    const tableBottom = tableRadiusM(this.#figure);
    const perFrameMedium = packMedium(this.#medium, inputs.radiusM - inputs.heightM, 1);
    const common = {
      buffers: {},
      sampled: {
        transmittance: this.#tables.transmittance,
        multiScattering: this.#tables.multiScattering,
      },
    };
    const viewUniforms = this.#viewBlock();
    engine.dispatch(
      frame.skyViewKernel,
      {
        ...common,
        uniforms: { medium: perFrameMedium, view: viewUniforms },
        storage: { skyViewOut: { texture: frame.skyView, level: 0 } },
      },
      groups8(sizes.skyView.widthTexels, sizes.skyView.heightTexels),
      "atmosphere view",
    );
    engine.dispatch(
      frame.aerialKernel,
      {
        ...common,
        uniforms: { medium: perFrameMedium, view: viewUniforms },
        storage: { aerialOut: { texture: frame.aerial, level: 0 } },
      },
      groups8(sizes.aerialPerspective.widthTexels, sizes.aerialPerspective.heightTexels),
      "atmosphere view",
    );
    const rayMarch = this.#rayMarchTexture(view.viewport);
    engine.dispatch(
      frame.rayMarchKernel,
      {
        buffers: {},
        sampled: { ...common.sampled, sceneDepth: scene.depth },
        uniforms: { medium: perFrameMedium, view: viewUniforms },
        storage: { rayMarchOut: { texture: rayMarch, level: 0 } },
      },
      groups8(
        Math.ceil(view.viewport.widthPx * sizes.rayMarchScale),
        Math.ceil(view.viewport.heightPx * sizes.rayMarchScale),
      ),
      "atmosphere view",
    );
    const shell = u["shell"];
    if (shell !== undefined) {
      shell[0] = tableBottom + this.#medium.topHeightM;
      shell[1] = inputs.radiusM - inputs.heightM;
      shell[2] = this.#medium.topHeightM;
      shell[3] = 0;
    }
    return {
      mesh: frame.triangle,
      material: frame.composite,
      offsetFromCameraM: new Float32Array(3),
      uniforms: u,
      textures: {
        sceneColour: scene.colour,
        sceneDepth: scene.depth,
        skyView: frame.skyView,
        aerial: frame.aerial,
        rayMarch,
        transmittance: this.#tables.transmittance,
      },
    };
  }

  /** Stops following the engine's restores. */
  dispose(): void {
    this.#offRestored();
    this.#tables.dispose();
  }

  /**
   * The ray-march target at the setting's scale of the largest output so far: it grows on a
   * resize and is never remade smaller, since the engine offers no release of a texture; the
   * kernel and the composite address it by the current output's size.
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
    frame.rayMarch = this.#engine.createTexture({
      name: "atmosphere ray march",
      size: [
        Math.max(1, Math.ceil(grown.widthPx * scale)),
        Math.max(1, Math.ceil(grown.heightPx * scale)),
      ],
      dimension: "2d",
      format: "rgba16float",
      mips: 1,
      usage: STORAGE_SAMPLED,
      category: "atmosphere-view",
    });
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
  ): void {
    const set = (name: string, values: readonly [number, number, number, number]): void => {
      this.#uniforms[name]?.set(values);
    };
    const p = view.positionM;
    const n = inputs.normal;
    const s = sun.directionBodyFixed;
    const distance2 = sun.distanceAu * sun.distanceAu;
    const sky = skyLuminanceScale();
    const sunRgb = sunIlluminanceRgb();
    const solidAngle = 2 * Math.PI * (1 - Math.cos(sun.angularRadiusRad));
    const disc = solidAngle > 0 ? 1 / (distance2 * solidAngle) : 0;
    const sunFactors = SUN_SPECTRAL_TO_LUMINANCE;
    const skyFactors = SKY_SPECTRAL_TO_LUMINANCE;
    const aspect = view.viewport.widthPx / view.viewport.heightPx;
    const tanX = Math.tan(view.fovXRad / 2);
    const tanY = tanX / aspect;
    const right = rotate(view.orientation, { x: 1, y: 0, z: 0 });
    const up = rotate(view.orientation, { x: 0, y: 1, z: 0 });
    const forward = rotate(view.orientation, { x: 0, y: 0, z: -1 });
    const sizes = this.#sizes;
    set("camera", [p.x, p.y, p.z, inputs.heightM]);
    set("up", [n.x, n.y, n.z, inputs.radiusM - inputs.heightM]);
    set("sun", [s.x, s.y, s.z, sun.angularRadiusRad]);
    set("skyScale", [
      sky[0] / distance2,
      sky[1] / distance2,
      sky[2] / distance2,
      scene.exposureScale,
    ]);
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
    // Sunlight reflected by the grey ground keeps the sun's spectrum, so it takes the sun's
    // factors, not the sky's (Bruneton 2017, model.cc, GetSunAndSkyIlluminance).
    set("sunOverSky", [
      sunFactors[0] / skyFactors[0],
      sunFactors[1] / skyFactors[1],
      sunFactors[2] / skyFactors[2],
      0,
    ]);
    set("output", [
      view.viewport.widthPx,
      view.viewport.heightPx,
      sizes.rayMarchScale,
      sizes.skyViewSamples,
    ]);
  }
}
