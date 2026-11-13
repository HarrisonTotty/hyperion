/**
 * The two-dimensional cloud layer (plan R11, R11.T7.b; Design notes 8, 10 and 18): its material, the
 * rows and uniforms its draw and the terrain's cloud light read, and each body's GPU state, which a
 * photorealistic view keeps for the bodies whose clouds it draws.
 *
 * @remarks
 * - **Which bodies.** A body whose cloud fraction lies strictly between 0 and 1 and which has a deck
 *   (`condensates.ts`' `cloudCondensateOf`): an overcast's deck is R08's split, or its uniform
 *   medium's, and a clear sky has none (Design note 9).
 * - **What a body holds on the GPU**, in the `clouds` category: its rows ({@link CLOUD_LAYER_ROWS},
 *   `cloudField.wgsl`'s), written each frame, and its coverage texture, `rgba16float`
 *   {@link CLOUD_COVERAGE_WIDTH} × {@link CLOUD_COVERAGE_HEIGHT} (256 KiB), written again when its
 *   season moves by {@link CLOUD_SEASON_STEP_RAD}; beside them the noise tile all bodies share
 *   (256 KiB), made over frames ({@link CloudLayers.step}). The layer draws nothing before the tile
 *   is made, and so a body's arrival drops no frame.
 * - **The season and the coverage.** The season is the subsolar latitude of the body's brightest
 *   star at the frame; at each step of it the offset is solved again over the zonal field alone,
 *   which is one row of latitudes (`coverage.ts`' `calibrateOffset`), and the field is formed. The
 *   zonal stand-in is the same at every longitude, so the field is a row repeated.
 * - **The height.** The layer stands at the deck's base plus {@link LAYER_HEIGHT_SHARE} of its
 *   depth (lean, 2026-11-02): a deck's condensate lies mostly low in it, its mixing ratio falling
 *   above its base as (p ÷ p_base)^f_sed.
 * - **Not drawn yet.** The photorealistic style's pass after each body's composite, the decks
 *   carried from R08's assembly and the terrain's cloudy lit forms are the wiring's, a later
 *   change's (plan R11's T7.b records); the smoke harness draws the layer and probes its field.
 */
import type { BodyIdHex, UniverseTime } from "@hyperion/protocol";

import { norm, scale, sub, type Vec3 } from "../../geometry/vec3";
import { asin, tan } from "../../lib/pinnedMath";
import {
  type AtmosphereCamera,
  geodeticOf,
  type SpheroidFigure,
  type SunState,
} from "../atmosphere/hillaire";
import {
  SURFACE_LIGHTING_WGSL,
  type SurfaceLightingTables,
  surfaceLightingDrawTextures,
  surfaceLightingTextures,
} from "../atmosphere/surfaceLighting";
import { NEAR_PLANE_M } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import type { Quaternion } from "../camera/pose";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  DrawItem,
  MeshHandle,
  RenderEngine,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import frameWgsl from "../shaders/frame.wgsl?raw";
import { fromHalfBits, toHalfBits } from "../sky/half";
import { CLOUD_NOISE_SIZE, CloudNoiseBuilder } from "./cloudNoise";
import {
  CLOUD_OCTAVE_RATIO,
  type CloudFieldParams,
  cloudFieldParams,
  cloudSeedOf,
  coverageTexels,
  type CoverageTexels,
} from "./cloudDensity";
import { type BodyClouds, type CloudCondensate, cloudCondensateOf } from "./condensates";
import {
  calibrateOffset,
  type CloudSeason,
  coverageField,
  PROVISIONAL_ZONAL_CLIMATE,
  provisionalZonalWindMps,
} from "./coverage";
import cloudFieldWgsl from "./shaders/cloudField.wgsl?raw";
import cloudLayerWgsl from "./shaders/cloudLayer.wgsl?raw";
import cloudOpticsWgsl from "./shaders/cloudOptics.wgsl?raw";

/** The label of a body's layer pass in `PassTimes` (Design note 18's `clouds`). */
export const CLOUD_LAYER_PASS = "clouds";

/** The rows of a body's clouds (`cloudField.wgsl`'s CLOUD_LAYER_ROWS). */
export const CLOUD_LAYER_ROWS = 34;

/** The coverage texture's cells around a parallel. */
export const CLOUD_COVERAGE_WIDTH = 256;

/** Its cells along a meridian: 1.4° each. */
export const CLOUD_COVERAGE_HEIGHT = 128;

/** The step of the subsolar latitude at which a body's coverage is made again: 0.25°. */
export const CLOUD_SEASON_STEP_RAD = (0.25 * Math.PI) / 180;

/** The share of a deck's depth above its base at which the layer stands (the module's remarks). */
export const LAYER_HEIGHT_SHARE = 0.25;

/**
 * The octaves the layer draws at most on each setting: 12 on high, where the layer stands in for the
 * volume when the floor steps it down, to ℓ₀ ÷ 2¹¹ (about 490 m on an Earth); 9 on low, to about
 * 3.9 km, which bounds its taps to 18 a pixel for its budget of about 1 ms.
 */
export const LAYER_OCTAVES: Readonly<Record<QualitySetting, number>> = { high: 12, low: 9 };

/** The layer's height above the datum for a condensate, m (the module's remarks). */
export function cloudLayerHeightM(
  condensate: Pick<CloudCondensate, "baseAltitudeM" | "topAltitudeM">,
): number {
  return (
    condensate.baseAltitudeM +
    LAYER_HEIGHT_SHARE * (condensate.topAltitudeM - condensate.baseAltitudeM)
  );
}

/**
 * The condensate a body's layer draws, or `null` for a body whose clouds are not this plan's: a cloud
 * fraction of 0 or 1, or no deck (Design note 9).
 */
export function layerCondensateOf(clouds: BodyClouds): CloudCondensate | null {
  const { cloudFraction } = clouds;
  if (!(cloudFraction > 0 && cloudFraction < 1)) {
    return null;
  }
  return cloudCondensateOf(clouds.decks, clouds.surfacePressurePa);
}

/** The material's bindings in `@group(2)` (`cloudLayer.wgsl`'s). */
export const CLOUD_LAYER_BINDINGS = {
  sceneDepth: 0,
  coverage: 1,
  noise: 2,
  sampler: 3,
  layer: 4,
  /** The first of R08's surface lighting's nine tables (`SURFACE_LIGHTING_BINDINGS`). */
  atmosphere: 5,
} as const;

/** The layer's `Draw` members after `offsetFromCameraM`, each a `vec4f` (`cloudLayer.wgsl`'s). */
const LAYER_MEMBERS = [
  "camera",
  "cameraNormal",
  "right",
  "upRay",
  "forward",
  "shell",
  "view",
  "source0",
  "sourceLight0",
  "source1",
  "sourceLight1",
  "levers",
] as const;

/** The layer's WGSL: `frame.wgsl`, R08's surface lighting, the field, the optics and the pass. */
export const CLOUD_LAYER_WGSL = `${frameWgsl}\n${SURFACE_LIGHTING_WGSL}\n${cloudFieldWgsl}\n${cloudOpticsWgsl}\n${cloudLayerWgsl}`;

/** The names a body's draw binds its clouds' textures and rows under. */
export const CLOUD_TEXTURES = {
  coverage: "cloudCoverage",
  noise: "cloudNoise",
  layer: "cloudLayer",
  sampler: "cloudSampler",
} as const;

/**
 * The layer's material: a full-screen triangle with no depth test, premultiplied into the target
 * its body's composite wrote, keeping its meter class.
 */
export const CLOUD_LAYER_MATERIAL: WgslMaterialSpec = {
  name: "cloud layer",
  displayName: "CLOUD LAYER",
  vertexWgsl: CLOUD_LAYER_WGSL,
  fragmentWgsl: CLOUD_LAYER_WGSL,
  uniforms: LAYER_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [
    {
      name: CLOUD_TEXTURES.sampler,
      filter: "linear",
      address: "repeat",
      binding: CLOUD_LAYER_BINDINGS.sampler,
    },
  ],
  textures: [
    { name: "sceneDepth", binding: CLOUD_LAYER_BINDINGS.sceneDepth, sampleType: "depth" },
    { name: CLOUD_TEXTURES.coverage, binding: CLOUD_LAYER_BINDINGS.coverage },
    { name: CLOUD_TEXTURES.noise, binding: CLOUD_LAYER_BINDINGS.noise, viewDimension: "3d" },
    ...surfaceLightingTextures(CLOUD_LAYER_BINDINGS.atmosphere),
  ],
  storageBuffers: [{ name: CLOUD_TEXTURES.layer, binding: CLOUD_LAYER_BINDINGS.layer }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "premultiplied",
};

/** The layer's place: its spheroid's semi-axes and its height above the datum, m. */
export interface CloudShell {
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
  readonly heightM: number;
}

/** A body's layer shell on its datum `figure` at `heightM`: the spheroid of a + h and c + h. */
export function cloudShellOf(figure: SpheroidFigure, heightM: number): CloudShell {
  return {
    equatorialRadiusM: figure.equatorialRadiusM + heightM,
    polarRadiusM: figure.polarRadiusM + heightM,
    heightM,
  };
}

/**
 * A body's rows (`cloudField.wgsl`'s layout), into `out`, {@link CLOUD_LAYER_ROWS} × 4 `f32`s.
 *
 * @param datumRadiusM - The datum's equatorial radius, m, by which the terrain's places, ÷ a, are
 *   turned into metres (`cloudShadow.wgsl`).
 */
export function packCloudLayer(
  params: CloudFieldParams,
  shell: CloudShell,
  condensate: Pick<CloudCondensate, "asymmetry" | "coAlbedo">,
  datumRadiusM: number,
  out: Float32Array = new Float32Array(4 * CLOUD_LAYER_ROWS),
): Float32Array {
  out.fill(0);
  out.set([shell.equatorialRadiusM, shell.polarRadiusM, shell.heightM, params.meanTau], 0);
  out.set([condensate.asymmetry, condensate.coAlbedo, params.octaves, params.baseCellM], 4);
  out.set([...params.advectionS, ...params.phaseWeights], 8);
  let total = 0;
  let power = 1;
  for (let k = 0; k < params.octaves; k += 1) {
    total += power;
    power *= CLOUD_OCTAVE_RATIO * CLOUD_OCTAVE_RATIO;
  }
  out.set([1 / Math.sqrt(total), datumRadiusM], 12);
  out.set(params.octaveFades, 16);
  const r = params.rotation;
  out.set([r[0] ?? 0, r[1] ?? 0, r[2] ?? 0], 28);
  out.set([r[3] ?? 0, r[4] ?? 0, r[5] ?? 0], 32);
  out.set([r[6] ?? 0, r[7] ?? 0, r[8] ?? 0], 36);
  for (let k = 0; k < params.octaves; k += 1) {
    for (let i = 0; i < 2; i += 1) {
      const at = 3 * (2 * k + i);
      out.set(
        [params.offsets[at] ?? 0, params.offsets[at + 1] ?? 0, params.offsets[at + 2] ?? 0],
        4 * (10 + 2 * k + i),
      );
    }
  }
  return out;
}

/** What a body's layer draw is lit and seen by this frame. */
export interface CloudLayerView {
  readonly camera: AtmosphereCamera;
  readonly figure: SpheroidFigure;
  readonly shell: CloudShell;
  /** The air's top above the datum, m (`AtmosphereMedium.topHeightM`). */
  readonly airTopM: number;
  /** The sources that light it, at most two drawn, brightest first (`skySources`'). */
  readonly suns: ReadonlyArray<SunState>;
  readonly exposureScale: number;
  readonly octaves: number;
}

/**
 * A body's layer draw's uniforms, formed in `f64` and narrowed once: its camera's place and rays
 * in the body-fixed axes, the shell's quadric constant |S o|² − a_s² (S = diag(1, 1, a_s ÷ c_s)),
 * which `f32` cannot form near the shell, and its two brightest sources.
 */
export function cloudLayerUniforms(view: CloudLayerView): Record<string, Float32Array> {
  const { camera, figure, shell } = view;
  const o = camera.positionM;
  const geodetic = geodeticOf(o, figure);
  const aspect = camera.viewport.widthPx / camera.viewport.heightPx;
  const tanX = tan(camera.fovXRad / 2);
  const tanY = tanX / aspect;
  const right = rotate(camera.orientation, { x: 1, y: 0, z: 0 });
  const up = rotate(camera.orientation, { x: 0, y: 1, z: 0 });
  const forward = rotate(camera.orientation, { x: 0, y: 0, z: -1 });
  const k = shell.equatorialRadiusM / shell.polarRadiusM;
  const quadric =
    o.x * o.x + o.y * o.y + o.z * k * (o.z * k) - shell.equatorialRadiusM * shell.equatorialRadiusM;
  const radPerPx = (2 * tanX) / camera.viewport.widthPx;
  const sources = view.suns.slice(0, 2);
  const source = (i: number): readonly [Float32Array, Float32Array] => {
    const sun = sources[i];
    if (sun === undefined) {
      return [new Float32Array([0, 0, 1, 0]), new Float32Array(4)];
    }
    const d = sun.directionBodyFixed;
    const [r, g, b] = sun.illuminanceLx;
    return [
      new Float32Array([d.x, d.y, d.z, sun.curveSlot ?? 0]),
      new Float32Array([r * view.exposureScale, g * view.exposureScale, b * view.exposureScale, 0]),
    ];
  };
  const [source0, sourceLight0] = source(0);
  const [source1, sourceLight1] = source(1);
  const n = geodetic.normal;
  return {
    camera: new Float32Array([o.x, o.y, o.z, geodetic.heightM]),
    cameraNormal: new Float32Array([n.x, n.y, n.z, geodetic.latitudeRad]),
    right: new Float32Array([right.x * tanX, right.y * tanX, right.z * tanX, 0]),
    upRay: new Float32Array([up.x * tanY, up.y * tanY, up.z * tanY, 0]),
    forward: new Float32Array([forward.x, forward.y, forward.z, 0]),
    shell: new Float32Array([quadric, k, NEAR_PLANE_M, radPerPx]),
    view: new Float32Array([
      camera.viewport.widthPx,
      camera.viewport.heightPx,
      view.airTopM,
      sources.length,
    ]),
    source0,
    sourceLight0,
    source1,
    sourceLight1,
    levers: new Float32Array([view.octaves, 0, 0, 0]),
  };
}

/** The subsolar latitude of a body's brightest light, its season (`coverage.ts`' `CloudSeason`). */
export function seasonOf(
  toBodyFixed: Quaternion,
  bodyCentreM: Vec3,
  brightestCentreM: Vec3,
): CloudSeason {
  const towards = sub(brightestCentreM, bodyCentreM);
  const d = rotate(toBodyFixed, scale(towards, 1 / norm(towards)));
  return { subsolarLatitudeRad: asin(Math.min(Math.max(d.z, -1), 1)) };
}

/** A body's clouds' GPU state this frame, which its layer draw and its terrain bind. */
export interface CloudBodyGpu {
  /** Its rows. */
  readonly layer: BufferHandle;
  /** Its coverage texture. */
  readonly coverage: TextureHandle;
  /** The noise tile. */
  readonly noise: TextureHandle;
}

/** A body's state held across frames. */
interface HeldBody {
  readonly layer: BufferHandle;
  readonly coverage: TextureHandle;
  readonly rows: Float32Array;
  /** The season and fraction the coverage was made for, and its texels. */
  made: {
    readonly seasonStep: number;
    readonly fraction: number;
    readonly texels: CoverageTexels;
  } | null;
}

/** What a frame asks of a body's clouds. */
export interface CloudFrame {
  readonly id: BodyIdHex;
  readonly clouds: BodyClouds;
  readonly condensate: CloudCondensate;
  readonly figure: SpheroidFigure;
  readonly season: CloudSeason;
  readonly time: Pick<UniverseTime, "seconds" | "nanos">;
  readonly rate: number;
  readonly octaves: number;
}

/** The time the noise tile's making may take a frame, ms. */
export const NOISE_STEP_BUDGET_MS = 1;

/**
 * The clouds of a view's bodies (the module's remarks): the noise tile, made over frames, and each
 * drawn body's rows and coverage, made at its first frame and let go when a frame draws it no
 * more ({@link CloudLayers.keepOnly}).
 */
export class CloudLayers {
  readonly #engine: Pick<
    RenderEngine,
    | "createBuffer"
    | "createTexture"
    | "writeBuffer"
    | "writeTexture"
    | "releaseBuffer"
    | "releaseTexture"
  >;
  readonly #name: string;
  readonly #bodies = new Map<BodyIdHex, HeldBody>();
  #builder: CloudNoiseBuilder | null = new CloudNoiseBuilder();
  #noise: TextureHandle | null = null;

  constructor(
    engine: Pick<
      RenderEngine,
      | "createBuffer"
      | "createTexture"
      | "writeBuffer"
      | "writeTexture"
      | "releaseBuffer"
      | "releaseTexture"
    >,
    name: string,
  ) {
    this.#engine = engine;
    this.#name = name;
  }

  /** Whether the noise tile is made, so that clouds can be drawn. */
  get ready(): boolean {
    return this.#noise !== null;
  }

  /**
   * Makes the noise tile a slice at a time for up to `budgetMs` of `nowMs`'s clock; the tile is
   * written once its last slice is done.
   */
  step(nowMs: () => number, budgetMs: number = NOISE_STEP_BUDGET_MS): void {
    const builder = this.#builder;
    if (builder === null) {
      return;
    }
    const start = nowMs();
    let texels = builder.step();
    while (texels === null && nowMs() - start < budgetMs) {
      texels = builder.step();
    }
    if (texels === null) {
      return;
    }
    const size = {
      width: CLOUD_NOISE_SIZE,
      height: CLOUD_NOISE_SIZE,
      depthOrArrayLayers: CLOUD_NOISE_SIZE,
    };
    const noise = this.#engine.createTexture({
      name: `${this.#name} cloud noise`,
      size,
      dimension: "3d",
      format: "r8unorm",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "clouds",
    });
    this.#engine.writeTexture(noise, { x: 0, y: 0, z: 0 }, size, texels);
    this.#noise = noise;
    this.#builder = null;
  }

  /**
   * A body's clouds this frame: its coverage made again where its season or fraction moved, its rows
   * written; `null` before the noise tile is made.
   */
  frame(frame: CloudFrame): CloudBodyGpu | null {
    const noise = this.#noise;
    if (noise === null) {
      return null;
    }
    const held = this.#held(frame.id);
    const seasonStep = Math.round(frame.season.subsolarLatitudeRad / CLOUD_SEASON_STEP_RAD);
    const fraction = frame.clouds.cloudFraction;
    let made = held.made;
    if (made === null || made.seasonStep !== seasonStep || made.fraction !== fraction) {
      const texels = seasonCoverage(
        { subsolarLatitudeRad: seasonStep * CLOUD_SEASON_STEP_RAD },
        fraction,
      );
      this.#engine.writeTexture(
        held.coverage,
        { x: 0, y: 0, z: 0 },
        { width: CLOUD_COVERAGE_WIDTH, height: CLOUD_COVERAGE_HEIGHT },
        halfTexels(texels.texels),
      );
      made = { seasonStep, fraction, texels };
      held.made = made;
    }
    const params = cloudFieldParams({
      seed: cloudSeedOf(frame.id),
      condensate: frame.condensate,
      radiusM: frame.figure.equatorialRadiusM,
      time: frame.time,
      rate: frame.rate,
      windMps: largestWindMps(made.texels),
      octaves: frame.octaves,
    });
    const shell = cloudShellOf(frame.figure, cloudLayerHeightM(frame.condensate));
    packCloudLayer(params, shell, frame.condensate, frame.figure.equatorialRadiusM, held.rows);
    this.#engine.writeBuffer(held.layer, 0, held.rows);
    return { layer: held.layer, coverage: held.coverage, noise };
  }

  /** Lets go of every body's state but those in `kept`. */
  keepOnly(kept: ReadonlySet<BodyIdHex>): void {
    for (const [id, held] of this.#bodies) {
      if (!kept.has(id)) {
        this.#release(held);
        this.#bodies.delete(id);
      }
    }
  }

  /** Lets go of everything. */
  dispose(): void {
    this.keepOnly(new Set());
    if (this.#noise !== null) {
      this.#engine.releaseTexture(this.#noise);
      this.#noise = null;
    }
    this.#builder = null;
  }

  #held(id: BodyIdHex): HeldBody {
    let held = this.#bodies.get(id);
    if (held === undefined) {
      held = {
        layer: this.#engine.createBuffer({
          name: `${this.#name} clouds ${id}`,
          bytes: 16 * CLOUD_LAYER_ROWS,
          usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
          category: "clouds",
        }),
        coverage: this.#engine.createTexture({
          name: `${this.#name} cloud coverage ${id}`,
          size: { width: CLOUD_COVERAGE_WIDTH, height: CLOUD_COVERAGE_HEIGHT },
          dimension: "2d",
          format: "rgba16float",
          mips: 1,
          usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
          category: "clouds",
        }),
        rows: new Float32Array(4 * CLOUD_LAYER_ROWS),
        made: null,
      };
      this.#bodies.set(id, held);
    }
    return held;
  }

  #release(held: HeldBody): void {
    this.#engine.releaseBuffer(held.layer);
    this.#engine.releaseTexture(held.coverage);
  }
}

/**
 * A season's coverage texels at plan 14's `fraction` on the provisional zonal climatology: the
 * offset solved over one column of latitudes, then the field's rows repeated around the parallels,
 * each texel narrowed to `f16` and widened again, as the texture holds it.
 */
export function seasonCoverage(season: CloudSeason, fraction: number): CoverageTexels {
  const column = {
    zonal: PROVISIONAL_ZONAL_CLIMATE,
    surveyed: null,
    season,
    widthCells: 1,
    heightCells: CLOUD_COVERAGE_HEIGHT,
  };
  const offset = calibrateOffset(column, fraction);
  const field = coverageField({ ...column, widthCells: CLOUD_COVERAGE_WIDTH }, offset);
  return coverageTexels(
    field,
    (latitudeRad) => ({ eastMps: provisionalZonalWindMps(latitudeRad, season), northMps: 0 }),
    (value) => fromHalfBits(toHalfBits(value)),
  );
}

/** Texels as `rgba16float`'s half floats. */
function halfTexels(texels: Float32Array): Uint16Array {
  return Uint16Array.from(texels, (value) => toHalfBits(value));
}

/** The largest wind speed a coverage holds, m s⁻¹, which sets the time-rate fade. */
export function largestWindMps(coverage: CoverageTexels): number {
  let most = 0;
  for (let t = 0; t < coverage.texels.length; t += 4) {
    most = Math.max(most, Math.hypot(coverage.texels[t + 2] ?? 0, coverage.texels[t + 3] ?? 0));
  }
  return most;
}

/**
 * A body's layer draw: a full-screen triangle with its uniforms, over the scene's depth, its
 * clouds' textures and rows and its air's tables.
 */
export function cloudLayerDraw(
  triangle: MeshHandle,
  material: DrawItem["material"],
  gpu: CloudBodyGpu,
  sceneDepth: TextureHandle,
  tables: SurfaceLightingTables,
  uniforms: Record<string, Float32Array>,
): DrawItem {
  return {
    mesh: triangle,
    material,
    offsetFromCameraM: new Float32Array(3),
    uniforms,
    textures: {
      sceneDepth,
      [CLOUD_TEXTURES.coverage]: gpu.coverage,
      [CLOUD_TEXTURES.noise]: gpu.noise,
      ...surfaceLightingDrawTextures(tables),
    },
    storageBuffers: { [CLOUD_TEXTURES.layer]: gpu.layer },
  };
}
