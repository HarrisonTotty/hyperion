/**
 * The smoke page's clouds checks (plan R11, R11.T7.b and T7.c), in part 26: the field, the `f32`
 * δ-Eddington, the ground's tap and the volume's marches on the run's adapter against their `f64`
 * twins, through the clouds' probe (`view/clouds/cloudProbe.ts`), and the two-dimensional layer
 * drawn over a frame with every texel finite.
 *
 * @remarks
 * One Earth-like body's clouds are made by the layer's own `CloudLayers` (its noise tile, coverage
 * texture and rows), and its twin by the same functions on the CPU. The GPU's filtering and `f32`
 * are not bit-identical to the twins, and need not be (plan R11, Design note 8), so each value is
 * held within a stated tolerance:
 * - the coverage texture's z_c and N at a place to {@link COVERAGE_TOLERANCE}, its wind to
 *   {@link WIND_TOLERANCE_MPS}: the bilinear weights' precision;
 * - z_r, the resolved structure, to {@link RESOLVED_TOLERANCE}, the noise's trilinear weights' over
 *   up to twelve octaves; s, which reads no texture, to {@link UNRESOLVED_TOLERANCE};
 * - the share f and the mean τ formed from the GPU's own z_c, N, z_r and s, which isolates the
 *   normal's tail and density in `f32`, to {@link SHARE_TOLERANCE} and {@link TAU_RELATIVE_TOLERANCE};
 * - the share end to end at footprints that leave s at least {@link COARSE_SD}, where the threshold
 *   is not steep, to {@link COARSE_SHARE_TOLERANCE};
 * - δ-Eddington's R and T_diff to {@link SLAB_TOLERANCE}, T_dir to {@link DIRECT_RELATIVE_TOLERANCE}
 *   of itself (`cloudOptics.wgsl`'s header: 8.0 × 10⁻⁵ in R emulated, before WGSL's `exp`);
 * - the ground's three factors to {@link GROUND_TOLERANCE}, the share's end-to-end error at the
 *   sun's footprint, and exactly (1, 1, 0) above the shell;
 * - a homogeneous column's octave march, its single and its multiple scattering, to
 *   {@link COLUMN_TOLERANCE} and its correction's lookup to
 *   {@link CORRECTION_TOLERANCE}, relative; a view ray's march through the deck, whose every
 *   sample reads the field, to {@link DECK_TOLERANCE} of its radiance at footprints of a twentieth
 *   of the distance, and its transmittance to {@link DECK_TRANSMITTANCE_TOLERANCE}.
 */

import type { Vec3 } from "../geometry/vec3";
import { createAirlessTables } from "../view/atmosphere/surfaceLighting";
import { lookAlong } from "../view/camera/quaternion";
import { CLOUD_ENTRIES } from "../view/engine/catalogue";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, RenderEngine, RenderTarget, TextureHandle } from "../view/engine/types";
import {
  cloudFieldAt,
  cloudFieldParams,
  cloudSeedOf,
  normalDensity,
  normalTail,
} from "../view/clouds/cloudDensity";
import {
  type CloudBodyGpu,
  CLOUD_LAYER_MATERIAL,
  CLOUD_SEASON_STEP_RAD,
  type CloudFrame,
  type CloudLayerView,
  cloudLayerDraw,
  cloudLayerHeightM,
  CloudLayers,
  cloudLayerUniforms,
  cloudShellOf,
  largestWindMps,
  seasonCoverage,
} from "../view/clouds/cloudLayer";
import { cloudNoiseTexels } from "../view/clouds/cloudNoise";
import {
  CLOUD_PROBE_MATERIAL,
  CLOUD_PROBE_NAMES,
  CLOUD_PROBE_OUTPUTS,
  type CloudProbeCase,
  decodeCloudProbe,
  expectedCloudProbe,
  packCloudProbeCases,
  packCloudProbeMarch,
} from "../view/clouds/cloudProbe";
import type { CloudLightInput } from "../view/clouds/cloudShadow";
import { CORRECTION_MUS, CORRECTION_TAUS, octaveCorrectionOf } from "../view/clouds/cloudVolume";
import {
  CLOUD_MAP_LEVELS,
  CLOUD_MAP_SIDE,
  CloudMapAnchor,
  type CloudMapFrame,
  cloudMapTwinFromTexels,
  cloudMapUniforms,
  mapTexelM,
  texelPlace,
} from "../view/clouds/cloudMap";
import { type CloudMarchScene, deckStretch } from "../view/clouds/cloudMarch";
import {
  cloudMapTexelAt,
  cloudShapeFrame,
  cloudShapeKindOf,
  normalisationTable,
} from "../view/clouds/cloudShape";
import {
  BASE_NOISE_SIZE,
  type CloudShapeNoises,
  cloudShapeNoises,
  CURL_NOISE_SIZE,
  DETAIL_NOISE_SIZE,
} from "../view/clouds/cloudShapeNoise";
import {
  CLOUD_SHADOW_MAP_MATERIAL,
  CLOUD_SHADOW_TEXTURES,
  type CloudShadowFrame,
  SHADOW_MAP_LEVELS,
  shadowMapUniforms,
  shadowTexelAt,
  shadowTexelPlace,
} from "../view/clouds/cloudShadowMap";
import {
  CLOUD_COMPOSITE_MATERIAL,
  CLOUD_MAP_MATERIAL,
  CLOUD_RESOLVE_MATERIAL,
  CLOUD_VOLUME_MATERIAL,
  CloudVolumes,
  packCloudShape,
  VOLUME_LEVERS,
  windAtM,
} from "../view/clouds/cloudVolumePass";
import type { CapturedImage } from "./atmosphere";
import { hdrCapture } from "./hdrCapture";
import { type CloudCondensate, STAND_IN_CONDENSATES } from "../view/clouds/condensates";
import type { GraphicsStatusStore } from "../view/engine/status";
import { checkCatalogue } from "./catalogue";
import { type Checks, drawOf, frameOf, fullScreenMesh, halfTexels } from "./harness";

/** The coverage texture's z_c and N, absolute. */
const COVERAGE_TOLERANCE = 5e-3;
/** Its wind, m s⁻¹. */
const WIND_TOLERANCE_MPS = 0.05;
/** z_r, absolute, in standard deviations of z. */
const RESOLVED_TOLERANCE = 0.02;
/** s, absolute. */
const UNRESOLVED_TOLERANCE = 1e-4;
/** f from the GPU's own z_c, N, z_r and s, absolute. */
const SHARE_TOLERANCE = 1e-4;
/** The mean τ from the GPU's own inputs, relative, beside 10⁻⁶ absolute. */
const TAU_RELATIVE_TOLERANCE = 1e-3;
/** The least s at which the share is held end to end. */
const COARSE_SD = 0.3;
/** The share end to end at such a footprint, absolute. */
const COARSE_SHARE_TOLERANCE = 0.02;
/** δ-Eddington's R and T_diff, absolute. */
const SLAB_TOLERANCE = 2e-4;
/** T_dir, relative, beside 10⁻⁷ absolute. */
const DIRECT_RELATIVE_TOLERANCE = 2e-5;
/** The ground's factors, absolute. */
const GROUND_TOLERANCE = 0.02;

const RAD_PER_DEG = Math.PI / 180;

/** An Earth-like datum, m (WGS 84's semi-axes). */
const FIGURE = { equatorialRadiusM: 6_378_137, polarRadiusM: 6_356_752.3 };

/** Liquid water's stand-in deck, from 1 km to 3 km. */
const CONDENSATE: CloudCondensate = {
  condensate: "H2O",
  phase: "liquid",
  asymmetry: STAND_IN_CONDENSATES.liquidWater.asymmetry,
  coAlbedo: STAND_IN_CONDENSATES.liquidWater.coAlbedo,
  inCloudOpticalDepth550: STAND_IN_CONDENSATES.liquidWater.inCloudOpticalDepth550,
  baseAltitudeM: 1_000,
  topAltitudeM: 3_000,
  opticsSource: "stand-in",
  opticalDepthSource: "stand-in",
};

/** The body's frame: a cloud fraction of 0.67, a season of 30°, mid-afternoon of a day. */
const FRAME: CloudFrame = {
  id: "0200080020000000.0103",
  clouds: { cloudFraction: 0.67, surfacePressurePa: 101_325, decks: [] },
  condensate: CONDENSATE,
  figure: FIGURE,
  season: { subsolarLatitudeRad: 120 * CLOUD_SEASON_STEP_RAD },
  time: { seconds: 123_456, nanos: 500_000_000 },
  rate: 1,
  octaves: 12,
};

/** The twins' inputs for `frame`, {@link FRAME} if not given, as `CloudLayers.frame` makes them. */
function twinOf(noise: Uint8Array, frame: CloudFrame = FRAME): CloudLightInput {
  const coverage = seasonCoverage(frame.season, frame.clouds.cloudFraction);
  const params = cloudFieldParams({
    seed: cloudSeedOf(frame.id),
    condensate: frame.condensate,
    radiusM: FIGURE.equatorialRadiusM,
    time: frame.time,
    rate: frame.rate,
    windMps: largestWindMps(coverage),
    octaves: frame.octaves,
  });
  return {
    coverage,
    noise,
    params,
    shell: cloudShellOf(FIGURE, cloudLayerHeightM(CONDENSATE)),
    optics: CONDENSATE,
    datumRadiusM: FIGURE.equatorialRadiusM,
  };
}

/** The unit direction at a geocentric latitude and longitude. */
function directionAt(latitudeDeg: number, longitudeDeg: number): Vec3 {
  const phi = latitudeDeg * RAD_PER_DEG;
  const lambda = longitudeDeg * RAD_PER_DEG;
  return {
    x: Math.cos(phi) * Math.cos(lambda),
    y: Math.cos(phi) * Math.sin(lambda),
    z: Math.sin(phi),
  };
}

/** The point along `u` on the spheroid of semi-axes `a` and `c`. */
function onSpheroid(u: Vec3, a: number, c: number): Vec3 {
  const k = a / c;
  const r = a / Math.hypot(u.x, u.y, u.z * k);
  return { x: u.x * r, y: u.y * r, z: u.z * r };
}

/** The unit vector of `v`. */
function unit(v: Vec3): Vec3 {
  const n = Math.hypot(v.x, v.y, v.z);
  return { x: v.x / n, y: v.y / n, z: v.z / n };
}

/** The places the field is probed at: both hemispheres, both signs of longitude, near a pole. */
const PLACES: ReadonlyArray<readonly [number, number]> = [
  [10, 20],
  [-35, -120],
  [52, 170],
  [-8, -3],
  [84, 60],
];

/** Footprints, m, with the octaves drawn at most: coarse to fine, and a cheaper reader's cap. */
const FOOTPRINTS: ReadonlyArray<readonly [number, number]> = [
  [400_000, 12],
  [60_000, 12],
  [8_000, 12],
  [600, 12],
  [8_000, 4],
];

/** The probe's cases: the field, the optics and the ground's tap. */
function probeCases(twin: CloudLightInput): CloudProbeCase[] {
  const { shell } = twin;
  const field = PLACES.flatMap(([lat, lon]) =>
    FOOTPRINTS.map(([footprintM, octaveCap]): CloudProbeCase => ({
      kind: "field",
      pointM: onSpheroid(directionAt(lat, lon), shell.equatorialRadiusM, shell.polarRadiusM),
      footprintM,
      octaveCap,
    })),
  );
  const coAlbedos = [0, CONDENSATE.coAlbedo, 1e-5, 1e-3, 0.1];
  const optics: CloudProbeCase[] = [];
  for (const coAlbedo of coAlbedos) {
    for (const tau of [0.01, 1, 10, 50]) {
      for (const asymmetry of [0.75, 0.85]) {
        for (const mu0 of [1, 0.5, 0.02]) {
          optics.push({ kind: "optics", tau, asymmetry, coAlbedo, mu0 });
        }
      }
    }
  }
  // About kμ₀ = 1 at g 0.95 and ϖ₀ 0.9 (k ≈ 1.11): the window's middle and both edges.
  for (const mu0 of [0.9, 0.8955, 0.905, 0.9001]) {
    optics.push({ kind: "optics", tau: 2, asymmetry: 0.95, coAlbedo: 0.1, mu0 });
  }
  const sunRadius = 4.65e-3;
  const ground: CloudProbeCase[] = PLACES.flatMap(([lat, lon]) => {
    const u = directionAt(lat, lon);
    const place = onSpheroid(u, FIGURE.equatorialRadiusM, FIGURE.polarRadiusM);
    const overA = {
      x: place.x / FIGURE.equatorialRadiusM,
      y: place.y / FIGURE.equatorialRadiusM,
      z: place.z / FIGURE.equatorialRadiusM,
    };
    const east = unit({ x: -u.y, y: u.x, z: 0 });
    return [0, 0.6, 2.5].flatMap((tilt) =>
      [sunRadius, 0.1].map((sourceRadiusRad): CloudProbeCase => ({
        kind: "ground",
        placeOverA: overA,
        sun: unit({ x: u.x + tilt * east.x, y: u.y + tilt * east.y, z: u.z + tilt * east.z }),
        sourceRadiusRad,
      })),
    );
  });
  // A place above the shell keeps every part.
  const high = onSpheroid(
    directionAt(20, 40),
    FIGURE.equatorialRadiusM + 9_000,
    FIGURE.polarRadiusM + 9_000,
  );
  ground.push({
    kind: "ground",
    placeOverA: {
      x: high.x / FIGURE.equatorialRadiusM,
      y: high.y / FIGURE.equatorialRadiusM,
      z: high.z / FIGURE.equatorialRadiusM,
    },
    sun: directionAt(20, 40),
    sourceRadiusRad: sunRadius,
  });
  return [...field, ...optics, ...ground, ...volumeCases()];
}

/** The volume's deck, from 1 km to 3 km, as {@link CONDENSATE}'s. */
const DECK = { baseM: CONDENSATE.baseAltitudeM, topM: CONDENSATE.topAltitudeM };

/** The place the volume's probe stands over, the map's anchor's nadir: 10° N, 20° E. */
const MARCH_PLACE = directionAt(10, 20);

/** The scene's time the probe's noises drift to, s. */
const MARCH_TIME_S = FRAME.time.seconds + FRAME.time.nanos * 1e-9;

/** A world's three shape noises on the GPU, `r8unorm`. */
interface NoiseTextureSet {
  readonly base: TextureHandle;
  readonly detail: TextureHandle;
  readonly curl: TextureHandle;
}

/** A world's shape noises as textures, as `CloudVolumes` makes them. */
function noiseTexturesOf(engine: RenderEngine, noises: CloudShapeNoises): NoiseTextureSet {
  const cube = (what: string, side: number, texels: Uint8Array): TextureHandle => {
    const size = { width: side, height: side, depthOrArrayLayers: side };
    const texture = engine.createTexture({
      name: `smoke cloud ${what} noise`,
      size,
      dimension: "3d",
      format: "r8unorm",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "clouds",
    });
    engine.writeTexture(texture, { x: 0, y: 0, z: 0 }, size, texels);
    return texture;
  };
  const curlSize = { width: CURL_NOISE_SIZE, height: CURL_NOISE_SIZE };
  const curl = engine.createTexture({
    name: "smoke cloud curl noise",
    size: curlSize,
    dimension: "2d",
    format: "r8unorm",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "clouds",
  });
  engine.writeTexture(curl, { x: 0, y: 0 }, curlSize, noises.curl);
  return {
    base: cube("base", BASE_NOISE_SIZE, noises.base),
    detail: cube("detail", DETAIL_NOISE_SIZE, noises.detail),
    curl,
  };
}

/** The volume's probe's GPU state and its twin's scene (R11.T7.c). */
interface VolumeProbe {
  readonly scene: CloudMarchScene;
  /** The map's frame and texels as the GPU's map pass drew them, read back. */
  readonly frame: CloudMapFrame;
  readonly mapTexels: Float32Array;
  readonly map: RenderTarget;
  readonly noises: NoiseTextureSet;
  readonly shape: BufferHandle;
  readonly march: BufferHandle;
  readonly release: () => void;
}

/**
 * The volume's probe over the body's clouds (R11.T7.c): the map drawn by its own pass about a camera
 * over {@link MARCH_PLACE} and read back, the world's noises, the shape's rows at the frame's time,
 * and the twin's scene over the map as the GPU drew it, so that a march case's GPU and twin read the
 * same columns.
 */
async function volumeProbeOf(
  engine: RenderEngine,
  gpu: NonNullable<ReturnType<CloudLayers["frame"]>>,
  twin: CloudLightInput,
): Promise<VolumeProbe> {
  const shell = twin.shell;
  const cameraM = onSpheroid(
    MARCH_PLACE,
    FIGURE.equatorialRadiusM + 2_000,
    FIGURE.polarRadiusM + 2_000,
  );
  const frame = new CloudMapAnchor().frameFor(cameraM, shell.equatorialRadiusM);
  const kind = cloudShapeKindOf(CONDENSATE);
  const map = engine.createRenderTarget({
    name: "smoke cloud map",
    size: { widthPx: CLOUD_MAP_SIDE, heightPx: CLOUD_MAP_SIDE * CLOUD_MAP_LEVELS },
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "clouds",
  });
  const material = engine.createMaterial(CLOUD_MAP_MATERIAL);
  map.render(
    frameOf("smoke cloud map", [
      drawOf(fullScreenMesh(engine, "smoke cloud map's triangle"), material, [0, 0, 0, 0], {
        uniforms: {
          ...cloudMapUniforms(frame),
          shape: new Float32Array([
            shell.equatorialRadiusM / shell.polarRadiusM,
            FRAME.octaves,
            0,
            0,
          ]),
        },
        textures: {
          [CLOUD_PROBE_NAMES.coverage]: gpu.coverage,
          [CLOUD_PROBE_NAMES.noise]: gpu.noise,
        },
        storageBuffers: { [CLOUD_PROBE_NAMES.layer]: gpu.layer },
      }),
    ]),
  );
  const mapTexels = halfTexels(await engine.readTexture(map.colour));
  const noises = cloudShapeNoises(cloudSeedOf(FRAME.id));
  const noiseTextures = noiseTexturesOf(engine, noises);
  const depthM = CONDENSATE.topAltitudeM - CONDENSATE.baseAltitudeM;
  const shapeFrame = cloudShapeFrame(
    depthM,
    windAtM(twin.coverage, cameraM),
    MARCH_TIME_S,
    FRAME.rate,
  );
  const scene: CloudMarchScene = {
    map: cloudMapTwinFromTexels(frame, mapTexels),
    field: { noise: twin.noise, params: twin.params },
    shellRadiusM: shell.equatorialRadiusM,
    noises,
    frame: shapeFrame,
    inputs: {
      baseM: CONDENSATE.baseAltitudeM,
      depthM,
      kind,
      table: normalisationTable(kind.profileExponent),
      flat: 0,
    },
    topM: CONDENSATE.topAltitudeM,
    datumRadiusM: FIGURE.equatorialRadiusM,
    axisRatio: shell.equatorialRadiusM / shell.polarRadiusM,
    optics: CONDENSATE,
    correction: octaveCorrectionOf(CONDENSATE),
  };
  const buffer = (name: string, data: Float32Array): BufferHandle => {
    const made = engine.createBuffer({
      name,
      bytes: data.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    engine.writeBuffer(made, 0, data);
    return made;
  };
  const shape = buffer("smoke cloud shape", packCloudShape(shapeFrame, kind));
  const march = buffer("smoke cloud march", packCloudProbeMarch(scene));
  return {
    scene,
    frame,
    mapTexels,
    map,
    noises: noiseTextures,
    shape,
    march,
    release: () => {
      map.dispose();
      engine.releaseTexture(noiseTextures.base);
      engine.releaseTexture(noiseTextures.detail);
      engine.releaseTexture(noiseTextures.curl);
      engine.releaseBuffer(shape);
      engine.releaseBuffer(march);
    },
  };
}

/**
 * The volume's march cases (R11.T7.c) about {@link MARCH_PLACE}: from 2 m on the ground straight up
 * and at 45°, from 500 m below the base at 30° up, from within the deck along the horizon, from just
 * above the tops straight down and slanting, and from 10 km up slanting down, each through its
 * stretch of the deck, at the full levers and the stepped floor's.
 */
function marchCases(scene: CloudMarchScene): CloudProbeCase[] {
  const u = MARCH_PLACE;
  const east = unit({ x: -u.y, y: u.x, z: 0 });
  const at = (heightM: number): Vec3 =>
    onSpheroid(u, FIGURE.equatorialRadiusM + heightM, FIGURE.polarRadiusM + heightM);
  const towards = (up: number, along: number): Vec3 =>
    unit({
      x: up * u.x + along * east.x,
      y: up * u.y + along * east.y,
      z: up * u.z + along * east.z,
    });
  const sun = towards(0.8, 0.6);
  const rays: ReadonlyArray<readonly [Vec3, Vec3]> = [
    [at(2), towards(1, 0)],
    [at(2), towards(1, 1)],
    [at(CONDENSATE.baseAltitudeM - 500), towards(0.58, 1)],
    [at(2_000), towards(0, 1)],
    [at(CONDENSATE.topAltitudeM + 100), towards(-1, 0)],
    [at(CONDENSATE.topAltitudeM + 100), towards(-0.5, 1)],
    [at(10_000), towards(-0.6, 1)],
  ];
  return rays.flatMap(([originM, direction]) => {
    const stretch = deckStretch(
      originM,
      direction,
      scene.axisRatio,
      scene.datumRadiusM,
      scene.inputs.baseM,
      scene.topM,
    );
    if (stretch === null) {
      return [];
    }
    return [VOLUME_LEVERS.full, VOLUME_LEVERS.stepped].map((levers, i): CloudProbeCase => ({
      kind: "march",
      originM,
      direction,
      sun,
      t0: stretch[0],
      t1: stretch[1],
      viewSteps: levers.viewSteps,
      lightSteps: levers.lightSteps,
      detail: levers.detail,
      footprintPerM: 2e-3,
      jitter: 0.37 + 0.25 * i,
    }));
  });
}

/** The volume's column cases (R11.T7.c): homogeneous columns of the octaves' march. */
function volumeCases(): CloudProbeCase[] {
  const columns: CloudProbeCase[] = [];
  for (const tau of [0.5, 1, 10, 50]) {
    for (const [mu0, muView, cosTheta] of [
      [1, 1, -1],
      [0.5, 0.8, -0.2],
      [0.75, 0.3, 0.4],
    ] as const) {
      columns.push({
        kind: "column",
        tau,
        mu0,
        muView,
        cosTheta,
        asymmetry: CONDENSATE.asymmetry,
        coAlbedo: CONDENSATE.coAlbedo,
        steps: 32,
      });
    }
  }
  return columns;
}

/** The largest of `values`, 0 for none. */
function largest(values: ReadonlyArray<number>): number {
  return values.reduce((most, value) => Math.max(most, value), 0);
}

/** The probe's results and their twins, for one kind of case. */
interface Compared {
  readonly gpu: ReadonlyArray<number>;
  readonly cpu: ReadonlyArray<number>;
  readonly probe: CloudProbeCase;
}

/**
 * R11.T7.b's field, optics and ground's tap, and R11.T7.c's volume's marches, against their twins
 * (the module's remarks).
 */
export async function checkCloudTwins(engine: RenderEngine, checks: Checks): Promise<void> {
  const layers = new CloudLayers(engine, "smoke");
  layers.step(() => performance.now(), Number.POSITIVE_INFINITY);
  const gpu = layers.frame(FRAME);
  if (gpu === null) {
    layers.dispose();
    throw new Error("the clouds' noise tile was not made in one unbounded step");
  }
  const twin = twinOf(cloudNoiseTexels());
  const correction = octaveCorrectionOf(CONDENSATE);
  const volume = await volumeProbeOf(engine, gpu, twin);
  const cases = [...probeCases(twin), ...marchCases(volume.scene)];
  const packed = packCloudProbeCases(cases);
  // Made one by one, each released however a later one fails.
  const made: { texture?: TextureHandle; buffer?: BufferHandle; target?: RenderTarget } = {};
  let results: number[][];
  let shadow: ShadowMapRead;
  try {
    const correctionTexture = engine.createTexture({
      name: "cloud octave correction",
      size: { width: CORRECTION_TAUS, height: CORRECTION_MUS },
      dimension: "2d",
      format: "r32float",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "clouds",
    });
    made.texture = correctionTexture;
    engine.writeTexture(
      correctionTexture,
      { x: 0, y: 0 },
      { width: CORRECTION_TAUS, height: CORRECTION_MUS },
      correction.factors,
    );
    const caseBuffer = engine.createBuffer({
      name: "cloud probe cases",
      bytes: packed.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    made.buffer = caseBuffer;
    engine.writeBuffer(caseBuffer, 0, packed);
    const target = engine.createRenderTarget({
      name: "cloud probe",
      size: { widthPx: CLOUD_PROBE_OUTPUTS, heightPx: cases.length },
      format: "r32uint",
      mips: 1,
      depth: false,
      category: "render-targets",
    });
    made.target = target;
    const material = engine.createMaterial(CLOUD_PROBE_MATERIAL);
    target.render(
      frameOf(
        "cloud probe",
        [
          drawOf(fullScreenMesh(engine, "cloud probe's triangle"), material, [0, 0, 0, 0], {
            uniforms: {},
            textures: {
              [CLOUD_PROBE_NAMES.coverage]: gpu.coverage,
              [CLOUD_PROBE_NAMES.noise]: gpu.noise,
              [CLOUD_PROBE_NAMES.correction]: correctionTexture,
              [CLOUD_PROBE_NAMES.map]: volume.map.colour,
              [CLOUD_PROBE_NAMES.baseNoise]: volume.noises.base,
              [CLOUD_PROBE_NAMES.detailNoise]: volume.noises.detail,
              [CLOUD_PROBE_NAMES.curlNoise]: volume.noises.curl,
              [CLOUD_PROBE_NAMES.fieldNoise]: gpu.noise,
            },
            storageBuffers: {
              [CLOUD_PROBE_NAMES.layer]: gpu.layer,
              [CLOUD_PROBE_NAMES.cases]: caseBuffer,
              [CLOUD_PROBE_NAMES.shape]: volume.shape,
              [CLOUD_PROBE_NAMES.marchRows]: volume.march,
            },
          }),
        ],
        CLOUD_PROBE_OUTPUTS / cases.length,
      ),
    );
    results = decodeCloudProbe(
      await engine.readTexture(target.colour, 0, undefined, "tolerance"),
      cases.length,
    );
    shadow = await shadowMapOf(engine, gpu, volume, correctionTexture);
  } finally {
    made.target?.dispose();
    if (made.buffer !== undefined) {
      engine.releaseBuffer(made.buffer);
    }
    if (made.texture !== undefined) {
      engine.releaseTexture(made.texture);
    }
    volume.release();
    layers.dispose();
  }
  const expected = expectedCloudProbe(cases, twin, correction, volume.scene);
  const compared: Compared[] = cases.map((probe, i) => ({
    gpu: results[i] ?? [],
    cpu: expected[i] ?? [],
    probe,
  }));
  checkField(
    checks,
    compared.filter(({ probe }) => probe.kind === "field"),
    twin.params.meanTau,
  );
  checkOptics(
    checks,
    compared.filter(({ probe }) => probe.kind === "optics"),
  );
  checkGround(
    checks,
    compared.filter(({ probe }) => probe.kind === "ground"),
  );
  checkVolume(
    checks,
    compared.filter(({ probe }) => probe.kind === "column"),
    compared.filter(({ probe }) => probe.kind === "march"),
  );
  checkMap(checks, volume, twin);
  checkShadowMap(checks, volume, shadow);
}

/** The sun the shadow map is drawn for: 40° up at the probe's place, towards its east. */
function shadowSun(): Vec3 {
  const u = MARCH_PLACE;
  const east = unit({ x: -u.y, y: u.x, z: 0 });
  return unit({
    x: Math.sin(0.7) * u.x + Math.cos(0.7) * east.x,
    y: Math.sin(0.7) * u.y + Math.cos(0.7) * east.y,
    z: Math.sin(0.7) * u.z + Math.cos(0.7) * east.z,
  });
}

/** A shadow map drawn by its pass and read back, with its frame. */
interface ShadowMapRead {
  readonly frame: CloudShadowFrame;
  readonly texels: Float32Array;
}

/**
 * The cloud volume's shadow map (R11.T7.c) drawn by its pass over the volume's probe's map and
 * noises, for {@link shadowSun}, and read back.
 */
async function shadowMapOf(
  engine: RenderEngine,
  gpu: NonNullable<ReturnType<CloudLayers["frame"]>>,
  volume: VolumeProbe,
  correction: TextureHandle,
): Promise<ShadowMapRead> {
  const { scene } = volume;
  const frame: CloudShadowFrame = {
    map: volume.frame,
    sun: shadowSun(),
    datumRadiusM: FIGURE.equatorialRadiusM,
    axisRatio: scene.axisRatio,
    deck: DECK,
  };
  const target = engine.createRenderTarget({
    name: "smoke cloud shadow map",
    size: { widthPx: CLOUD_MAP_SIDE, heightPx: CLOUD_MAP_SIDE * SHADOW_MAP_LEVELS },
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "clouds",
  });
  try {
    target.render(
      frameOf("smoke cloud shadow map", [
        drawOf(
          fullScreenMesh(engine, "smoke cloud shadow map's triangle"),
          engine.createMaterial(CLOUD_SHADOW_MAP_MATERIAL),
          [0, 0, 0, 0],
          {
            uniforms: shadowMapUniforms(frame, scene.shellRadiusM, CONDENSATE),
            textures: {
              [CLOUD_SHADOW_TEXTURES.map]: volume.map.colour,
              [CLOUD_SHADOW_TEXTURES.baseNoise]: volume.noises.base,
              [CLOUD_SHADOW_TEXTURES.detailNoise]: volume.noises.detail,
              [CLOUD_SHADOW_TEXTURES.curlNoise]: volume.noises.curl,
              [CLOUD_SHADOW_TEXTURES.fieldNoise]: gpu.noise,
              [CLOUD_SHADOW_TEXTURES.correction]: correction,
            },
            storageBuffers: {
              [CLOUD_SHADOW_TEXTURES.layer]: gpu.layer,
              [CLOUD_SHADOW_TEXTURES.shape]: volume.shape,
            },
          },
        ),
      ]),
    );
    return { frame, texels: halfTexels(await engine.readTexture(target.colour)) };
  } finally {
    target.dispose();
  }
}

/** A shadow map texel's factors against its twin's, absolute. */
const SHADOW_TOLERANCE = 0.03;
/** The texels sampled a level: a 6 × 6 grid of them. */
const SHADOW_SAMPLES = 6;

/**
 * R11.T7.c's shadow map as its pass drew it against its twin's texels (`cloudShadowMap.ts`'
 * `shadowTexelAt` over the map the GPU drew): over 36 texels a level, each of its three factors to
 * {@link SHADOW_TOLERANCE}, every texel within [0, 1], and some in shadow.
 */
function checkShadowMap(checks: Checks, volume: VolumeProbe, shadow: ShadowMapRead): void {
  let worst = 0;
  let sound = true;
  let shaded = 0;
  for (let level = 0; level < SHADOW_MAP_LEVELS; level += 1) {
    for (let a = 0; a < SHADOW_SAMPLES; a += 1) {
      for (let b = 0; b < SHADOW_SAMPLES; b += 1) {
        const i = Math.floor(((a + 0.5) * CLOUD_MAP_SIDE) / SHADOW_SAMPLES);
        const j = Math.floor(((b + 0.5) * CLOUD_MAP_SIDE) / SHADOW_SAMPLES);
        const at = 4 * ((level * CLOUD_MAP_SIDE + j) * CLOUD_MAP_SIDE + i);
        const gpu = Array.from(shadow.texels.subarray(at, at + 3));
        const cpu = shadowTexelAt(
          volume.scene,
          shadow.frame,
          shadowTexelPlace(shadow.frame, level, i, j),
          mapTexelM(level),
        );
        sound &&= gpu.every((v) => Number.isFinite(v) && v >= 0 && v <= 1.001);
        worst = Math.max(
          worst,
          Math.abs((gpu[0] ?? Number.NaN) - cpu.direct),
          Math.abs((gpu[1] ?? Number.NaN) - cpu.sky),
          Math.abs((gpu[2] ?? Number.NaN) - cpu.diffuse),
        );
        shaded += cpu.direct < 0.5 ? 1 : 0;
      }
    }
  }
  checks.check(
    "R11.T7.c the cloud shadow map's pass draws its twin's light on the ground",
    sound && worst <= SHADOW_TOLERANCE && shaded > 0,
    `largest |Δ| ${worst.toExponential(2)} (≤ ${SHADOW_TOLERANCE}) over ${2 * SHADOW_SAMPLES * SHADOW_SAMPLES} texels, ${shaded} of them in shadow, every one ${sound ? "within [0, 1]" : "NOT within [0, 1]"}`,
  );
}

/** The map's texels sampled a level: an 8 × 8 grid of them. */
const MAP_SAMPLES = 8;
/** A texel's resolved sum, absolute: the noise's trilinear filter over twelve octaves and `f16`. */
const MAP_SUM_TOLERANCE = 0.05;
/** A texel's threshold and wind, absolute: the coverage's bilinear weights and `f16`. */
const MAP_THRESHOLD_TOLERANCE = 0.01;
const MAP_WIND_TOLERANCE_MPS = 0.1;

/**
 * R11.T7.c's cloud map as its pass drew it against its twin's texels (`cloudShape.ts`'
 * `cloudMapTexelAt`): over 64 texels a level, the resolved sum to {@link MAP_SUM_TOLERANCE}, the
 * threshold to {@link MAP_THRESHOLD_TOLERANCE} and the wind to {@link MAP_WIND_TOLERANCE_MPS}.
 */
function checkMap(checks: Checks, volume: VolumeProbe, twin: CloudLightInput): void {
  let sum = 0;
  let threshold = 0;
  let wind = 0;
  let sound = true;
  for (let level = 0; level < CLOUD_MAP_LEVELS; level += 1) {
    for (let a = 0; a < MAP_SAMPLES; a += 1) {
      for (let b = 0; b < MAP_SAMPLES; b += 1) {
        const i = Math.floor(((a + 0.5) * CLOUD_MAP_SIDE) / MAP_SAMPLES);
        const j = Math.floor(((b + 0.5) * CLOUD_MAP_SIDE) / MAP_SAMPLES);
        const at = 4 * ((level * CLOUD_MAP_SIDE + j) * CLOUD_MAP_SIDE + i);
        const texel = Array.from(volume.mapTexels.subarray(at, at + 4));
        const place = texelPlace(volume.frame, twin.shell, level, i, j);
        const cpu = cloudMapTexelAt(
          twin.coverage,
          twin.noise,
          twin.params,
          place,
          mapTexelM(level),
        );
        const [
          gpuSum = Number.NaN,
          gpuThreshold = Number.NaN,
          east = Number.NaN,
          north = Number.NaN,
        ] = texel;
        sound &&= texel.every(Number.isFinite);
        sum = Math.max(sum, Math.abs(gpuSum - cpu.resolvedSum));
        threshold = Math.max(threshold, Math.abs(gpuThreshold - cpu.threshold));
        wind = Math.max(wind, Math.abs(east - cpu.eastMps), Math.abs(north - cpu.northMps));
      }
    }
  }
  checks.check(
    "R11.T7.c the cloud map's pass draws its twin's texels",
    sound &&
      sum <= MAP_SUM_TOLERANCE &&
      threshold <= MAP_THRESHOLD_TOLERANCE &&
      wind <= MAP_WIND_TOLERANCE_MPS,
    `largest |Δ| of the resolved sum ${sum.toExponential(2)} (≤ ${MAP_SUM_TOLERANCE}), the threshold ${threshold.toExponential(2)} (≤ ${MAP_THRESHOLD_TOLERANCE}), the wind ${wind.toExponential(2)} m/s (≤ ${MAP_WIND_TOLERANCE_MPS}), over ${5 * MAP_SAMPLES * MAP_SAMPLES} texels, every one ${sound ? "finite" : "NOT finite"}`,
  );
}

/** A column's march, relative. */
const COLUMN_TOLERANCE = 2e-4;
/** The correction's interpolation, relative. */
const CORRECTION_TOLERANCE = 1e-5;
/**
 * A march's radiance, relative beside 10⁻⁶ sr⁻¹ absolute: the noises' GPU filtering against the
 * twins' f64, through the erosion's remaps; 2.8 × 10⁻³ measured on SwiftShader (2026-11-19).
 */
const DECK_TOLERANCE = 0.01;
/** A march's view transmittance, absolute; 1.5 × 10⁻⁴ measured. */
const DECK_TRANSMITTANCE_TOLERANCE = 0.005;

/** A GPU value's difference from its twin's, relative to the twin's beside `floor`. */
function relative(gpu: number | undefined, cpu: number | undefined, floor: number): number {
  return Math.abs((gpu ?? Number.NaN) - (cpu ?? Number.NaN)) / (Math.abs(cpu ?? 0) + floor);
}

function checkVolume(
  checks: Checks,
  columns: ReadonlyArray<Compared>,
  decks: ReadonlyArray<Compared>,
): void {
  const marched = columns.map(({ gpu, cpu }) =>
    Math.max(relative(gpu[0], cpu[0], 1e-9), relative(gpu[1], cpu[1], 1e-9)),
  );
  const corrected = columns.map(({ gpu, cpu }) => relative(gpu[2], cpu[2], 1e-6));
  checks.check(
    "R11.T7.c the volume's column march and its correction equal their twins",
    allFinite(columns) &&
      largest(marched) <= COLUMN_TOLERANCE &&
      largest(corrected) <= CORRECTION_TOLERANCE,
    `largest relative difference ${largest(marched).toExponential(2)} (≤ ${COLUMN_TOLERANCE}) and ${largest(corrected).toExponential(2)} (≤ ${CORRECTION_TOLERANCE}) over ${columns.length} columns`,
  );
  const radiance = decks.map(({ gpu, cpu }) => relative(gpu[0], cpu[0], 1e-6 / DECK_TOLERANCE));
  const sky = decks.map(({ gpu, cpu }) => relative(gpu[1], cpu[1], 1e-6 / DECK_TOLERANCE));
  const transmittance = decks.map(({ gpu, cpu }) => off(gpu, cpu, 2));
  const share = decks.map(({ gpu, cpu }) => off(gpu, cpu, 4));
  const lit = decks.filter(({ cpu }) => (cpu[0] ?? 0) > 0).length;
  checks.check(
    "R11.T7.c the volume's march through its 3D density equals its twin",
    allFinite(decks) &&
      lit >= decks.length / 2 &&
      largest(radiance) <= DECK_TOLERANCE &&
      largest(sky) <= DECK_TOLERANCE &&
      largest(transmittance) <= DECK_TRANSMITTANCE_TOLERANCE &&
      largest(share) <= 1e-3,
    `largest relative difference ${largest(radiance).toExponential(2)} in the sun's and ${largest(sky).toExponential(2)} in the sky's (≤ ${DECK_TOLERANCE}), |ΔT| ${largest(transmittance).toExponential(2)} (≤ ${DECK_TRANSMITTANCE_TOLERANCE}), |Δshare| ${largest(share).toExponential(2)}, ${lit} of ${decks.length} rays lit`,
  );
}

/** The difference of a GPU value from its twin's, `NaN` where either is missing. */
function off(a: ReadonlyArray<number>, b: ReadonlyArray<number>, i: number): number {
  return Math.abs((a[i] ?? Number.NaN) - (b[i] ?? Number.NaN));
}

/** Whether every value is finite. */
function allFinite(rows: ReadonlyArray<Compared>): boolean {
  return rows.every(({ gpu }) => gpu.length === CLOUD_PROBE_OUTPUTS && gpu.every(Number.isFinite));
}

function checkField(checks: Checks, rows: ReadonlyArray<Compared>, meanTau: number): void {
  const thresholds = rows.map(({ gpu, cpu }) => off(gpu, cpu, 5));
  const scales = rows.map(({ gpu, cpu }) => off(gpu, cpu, 6));
  const winds = rows.map(({ gpu, cpu }) => off(gpu, cpu, 7));
  checks.check(
    "R11.T7.b the clouds' coverage texture read at a place equals its twin's",
    allFinite(rows) &&
      largest(thresholds) <= COVERAGE_TOLERANCE &&
      largest(scales) <= COVERAGE_TOLERANCE &&
      largest(winds) <= WIND_TOLERANCE_MPS,
    `largest |Δz_c| ${largest(thresholds).toExponential(2)}, |ΔN| ${largest(scales).toExponential(2)} (≤ ${COVERAGE_TOLERANCE}), |Δu| ${largest(winds).toExponential(2)} m/s (≤ ${WIND_TOLERANCE_MPS}) over ${rows.length} cases`,
  );
  const resolved = rows.map(({ gpu, cpu }) => off(gpu, cpu, 3));
  const sd = rows.map(({ gpu, cpu }) => off(gpu, cpu, 4));
  checks.check(
    "R11.T7.b the clouds' field's resolved structure and unresolved spread equal their twin's",
    largest(resolved) <= RESOLVED_TOLERANCE && largest(sd) <= UNRESOLVED_TOLERANCE,
    `largest |Δz_r| ${largest(resolved).toExponential(2)} (≤ ${RESOLVED_TOLERANCE}), |Δs| ${largest(sd).toExponential(2)} (≤ ${UNRESOLVED_TOLERANCE})`,
  );
  // The share and mean τ of the GPU's own z_c, N, z_r and s, in f64.
  const own = rows.map(({ gpu }) => {
    const [, , , zr = 0, s = 1, zc = 0, scale = 0] = gpu;
    const d = (zc - zr) / s;
    const q = normalTail(d);
    return { q, tau: Math.max(meanTau * scale * s * (normalDensity(d) - d * q), 0) };
  });
  const shares = rows.map(({ gpu }, i) => Math.abs((gpu[0] ?? Number.NaN) - (own[i]?.q ?? 0)));
  const taus = rows.map(({ gpu }, i) => {
    const want = own[i]?.tau ?? 0;
    return Math.abs((gpu[1] ?? Number.NaN) - want) / (TAU_RELATIVE_TOLERANCE * want + 1e-6);
  });
  checks.check(
    "R11.T7.b the clouds' field's share and mean τ follow its own structure in f32",
    largest(shares) <= SHARE_TOLERANCE && largest(taus) <= 1,
    `largest |Δf| ${largest(shares).toExponential(2)} (≤ ${SHARE_TOLERANCE}), mean τ ${largest(taus).toFixed(3)} of its tolerance`,
  );
  const coarse = rows.filter(({ cpu }) => (cpu[4] ?? 0) >= COARSE_SD);
  const coarseShares = coarse.map(({ gpu, cpu }) => off(gpu, cpu, 0));
  checks.check(
    "R11.T7.b the clouds' share at a coarse footprint equals its twin's",
    coarse.length >= 5 && largest(coarseShares) <= COARSE_SHARE_TOLERANCE,
    `largest |Δf| ${largest(coarseShares).toExponential(2)} (≤ ${COARSE_SHARE_TOLERANCE}) over ${coarse.length} cases of s ≥ ${COARSE_SD}`,
  );
}

function checkOptics(checks: Checks, rows: ReadonlyArray<Compared>): void {
  const reflectance = rows.map(({ gpu, cpu }) => off(gpu, cpu, 0));
  const diffuse = rows.map(({ gpu, cpu }) => off(gpu, cpu, 2));
  const direct = rows.map(
    ({ gpu, cpu }) => off(gpu, cpu, 1) / (DIRECT_RELATIVE_TOLERANCE * (cpu[1] ?? 0) + 1e-7),
  );
  checks.check(
    "R11.T7.b the f32 δ-Eddington equals its f64 twin",
    allFinite(rows) &&
      largest(reflectance) <= SLAB_TOLERANCE &&
      largest(diffuse) <= SLAB_TOLERANCE &&
      largest(direct) <= 1,
    `largest |ΔR| ${largest(reflectance).toExponential(2)}, |ΔT_diff| ${largest(diffuse).toExponential(2)} (≤ ${SLAB_TOLERANCE}), T_dir ${largest(direct).toFixed(3)} of its tolerance, over ${rows.length} cases`,
  );
  const thick = rows.find(
    ({ probe }) => probe.kind === "optics" && probe.tau === 10 && probe.mu0 === 1,
  );
  checks.check(
    "R11.T7.b a τ = 10 layer under a zenith sun lets through under 10⁻⁴ of the direct beam",
    thick !== undefined && (thick.gpu[1] ?? 1) < 1e-4,
    `T_dir ${(thick?.gpu[1] ?? Number.NaN).toExponential(3)}`,
  );
}

function checkGround(checks: Checks, rows: ReadonlyArray<Compared>): void {
  const below = rows.slice(0, -1);
  const differences = below.map(({ gpu, cpu }) =>
    Math.max(off(gpu, cpu, 0), off(gpu, cpu, 1), off(gpu, cpu, 2)),
  );
  checks.check(
    "R11.T7.b the clouds' light on the ground equals its twin's",
    allFinite(rows) && largest(differences) <= GROUND_TOLERANCE,
    `largest difference ${largest(differences).toExponential(2)} (≤ ${GROUND_TOLERANCE}) over ${below.length} cases`,
  );
  const above = rows.at(-1)?.gpu ?? [];
  checks.check(
    "R11.T7.b a place above the clouds' shell keeps all its light",
    above[0] === 1 && above[1] === 1 && above[2] === 0,
    `(${above.slice(0, 3).join(", ")})`,
  );
}

/** The layer's draws: a camera above the shell looking down past the limb, and one beneath it. */
const LAYER_VIEWS = [
  { heightM: 400_000, pitchRad: -0.35, name: "above" },
  { heightM: 300, pitchRad: 0.6, name: "below" },
] as const;

/** The layer's target's side, px. */
const LAYER_SIDE_PX = 32;

/**
 * The layer's view of the Earth-like body from one of {@link LAYER_VIEWS}: over 15° N, 25° E,
 * looking east, pitched above or below the horizon, under a sun a little east of overhead.
 */
function layerViewOf(
  view: { readonly heightM: number; readonly pitchRad: number },
  shell: ReturnType<typeof cloudShellOf>,
  size: { readonly widthPx: number; readonly heightPx: number },
  octaves: number,
  place: { readonly latitudeDeg: number; readonly longitudeDeg: number } = {
    latitudeDeg: 15,
    longitudeDeg: 25,
  },
): CloudLayerView {
  const at = directionAt(place.latitudeDeg, place.longitudeDeg);
  const radius = FIGURE.equatorialRadiusM + view.heightM;
  const east = unit({ x: -at.y, y: at.x, z: 0 });
  const c = Math.cos(view.pitchRad);
  const s = Math.sin(view.pitchRad);
  const orientation = lookAlong(
    { x: c * east.x + s * at.x, y: c * east.y + s * at.y, z: c * east.z + s * at.z },
    at,
  );
  return {
    camera: {
      positionM: { x: at.x * radius, y: at.y * radius, z: at.z * radius },
      orientation,
      fovXRad: 1.2,
      viewport: size,
    },
    figure: FIGURE,
    shell,
    airTopM: 100_000,
    suns: [
      {
        directionBodyFixed: unit({ x: at.x + 0.3 * east.x, y: at.y + 0.3 * east.y, z: at.z }),
        illuminanceLx: [120_000, 118_000, 110_000],
        angularRadiusRad: 4.65e-3,
        curveSlot: 0,
      },
    ],
    exposureScale: 1e-5,
    octaves,
  };
}

/** Of half-float texels, how many draw (α above 0) and how many are not finite or of α outside [0, 1]. */
function tallyTexels(texels: Float32Array): { readonly drawn: number; readonly bad: number } {
  let drawn = 0;
  let bad = 0;
  for (let i = 0; i < texels.length; i += 4) {
    const alpha = texels[i + 3] ?? Number.NaN;
    const finite = [0, 1, 2, 3].every((k) => Number.isFinite(texels[i + k] ?? Number.NaN));
    if (!finite || alpha < 0 || alpha > 1) {
      bad += 1;
    } else if (alpha > 0) {
      drawn += 1;
    }
  }
  return { drawn, bad };
}

/**
 * A body's clouds for another check to bind: {@link FRAME}'s state, its noise made at once (plan
 * R07's airless twins of the lit terrain's cloudy forms, `litTerrain.ts`). `dispose` releases it.
 */
export function smokeCloudBody(
  engine: RenderEngine,
  name: string,
): { readonly gpu: CloudBodyGpu; readonly dispose: () => void } {
  const layers = new CloudLayers(engine, name);
  layers.step(() => performance.now(), Number.POSITIVE_INFINITY);
  const gpu = layers.frame(FRAME);
  if (gpu === null) {
    layers.dispose();
    throw new Error("the clouds' state was not made");
  }
  return { gpu, dispose: () => layers.dispose() };
}

/** R11.T7.b's layer drawn from above and below its shell: every texel finite, its α in [0, 1]. */
export async function checkCloudLayerDraw(engine: RenderEngine, checks: Checks): Promise<void> {
  const layers = new CloudLayers(engine, "smoke layer");
  layers.step(() => performance.now(), Number.POSITIVE_INFINITY);
  const gpu = layers.frame(FRAME);
  const tables = createAirlessTables(engine, "cloud layer: no air");
  const size = { widthPx: LAYER_SIDE_PX, heightPx: LAYER_SIDE_PX };
  const scene = engine.createRenderTarget({
    name: "cloud layer scene",
    size,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const target = engine.createRenderTarget({
    name: "cloud layer",
    size,
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
  try {
    if (gpu === null || scene.depth === null) {
      throw new Error("the clouds' state or the scene's depth was not made");
    }
    scene.render(frameOf("cloud layer scene", []));
    const material = engine.createMaterial(CLOUD_LAYER_MATERIAL);
    const triangle = fullScreenMesh(engine, "cloud layer's triangle");
    const shell = cloudShellOf(FIGURE, cloudLayerHeightM(CONDENSATE));
    // Released with the check: plan R07's airless twins run it again (`airlessPasses.ts`).
    try {
      for (const view of LAYER_VIEWS) {
        const uniforms = cloudLayerUniforms(layerViewOf(view, shell, size, 9));
        target.render(
          frameOf(`cloud layer ${view.name}`, [
            cloudLayerDraw(triangle, material, gpu, scene.depth, tables, uniforms),
          ]),
        );
        // The harness's checks run in order: each reads the GPU back before the next draws.
        // oxlint-disable-next-line no-await-in-loop
        const { drawn, bad } = tallyTexels(halfTexels(await engine.readTexture(target.colour)));
        checks.check(
          `R11.T7.b the cloud layer seen from ${view.name} its shell draws every texel finite`,
          bad === 0 && drawn > 0,
          `${drawn} of ${LAYER_SIDE_PX * LAYER_SIDE_PX} texels drawn, ${bad} not finite or of α outside [0, 1]`,
        );
      }
    } finally {
      engine.releaseMaterial(material);
      engine.releaseMesh(triangle);
    }
  } finally {
    target.dispose();
    scene.dispose();
    engine.releaseTexture(tables.body);
    layers.dispose();
  }
}

/** The volume's frames drawn at each of its views: enough for its history to hold. */
const VOLUME_FRAMES = 3;

/**
 * R11.T7.c's volume drawn from above and below the deck through its march, its resolve and its
 * composite (`cloudVolumePass.ts`), several frames so that its history holds: every texel of the
 * composite finite, its α in [0, 1], and some drawn.
 */
export async function checkCloudVolumeDraw(engine: RenderEngine, checks: Checks): Promise<void> {
  const layers = new CloudLayers(engine, "smoke volume");
  layers.step(() => performance.now(), Number.POSITIVE_INFINITY);
  const gpu = layers.frame(FRAME);
  const volumes = new CloudVolumes(engine, "smoke volume");
  const correction = volumes.correction(
    CONDENSATE,
    () => performance.now(),
    Number.POSITIVE_INFINITY,
  );
  const tables = createAirlessTables(engine, "cloud volume: no air");
  const size = { widthPx: LAYER_SIDE_PX, heightPx: LAYER_SIDE_PX };
  const scene = engine.createRenderTarget({
    name: "cloud volume scene",
    size,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const target = engine.createRenderTarget({
    name: "cloud volume",
    size,
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
  try {
    if (gpu === null || scene.depth === null || correction === null) {
      throw new Error("the clouds' state, their correction or the scene's depth was not made");
    }
    scene.render(frameOf("cloud volume scene", []));
    const materials = {
      march: engine.createMaterial(CLOUD_VOLUME_MATERIAL),
      resolve: engine.createMaterial(CLOUD_RESOLVE_MATERIAL),
      composite: engine.createMaterial(CLOUD_COMPOSITE_MATERIAL),
      map: engine.createMaterial(CLOUD_MAP_MATERIAL),
      shadow: engine.createMaterial(CLOUD_SHADOW_MAP_MATERIAL),
      triangle: fullScreenMesh(engine, "cloud volume's triangle"),
    };
    volumes.prepareNoises(FRAME.id);
    const shell = cloudShellOf(FIGURE, cloudLayerHeightM(CONDENSATE));
    const { viewRotation, projection } = frameOf("cloud volume", []);
    const pass = { viewRotation, projection };
    // Released with the check: plan R07's airless twins run it again (`airlessPasses.ts`).
    try {
      for (const view of LAYER_VIEWS) {
        volumes.cut();
        for (let frame = 0; frame < VOLUME_FRAMES; frame += 1) {
          target.render(frameOf(`cloud volume ${view.name} clear`, []));
          volumes.draw(
            {
              id: FRAME.id,
              gpu,
              condensate: CONDENSATE,
              view: layerViewOf(view, shell, size, 12),
              lighting: tables,
              sceneDepth: scene.depth,
              correction,
              timeS: MARCH_TIME_S,
              rate: FRAME.rate,
              levers: VOLUME_LEVERS.full,
            },
            materials,
            target,
            size,
            pass,
            null,
          );
        }
        // The harness's checks run in order: each reads the GPU back before the next draws.
        // oxlint-disable-next-line no-await-in-loop
        const { drawn, bad } = tallyTexels(halfTexels(await engine.readTexture(target.colour)));
        checks.check(
          `R11.T7.c the cloud volume seen from ${view.name} its deck draws every texel finite`,
          bad === 0 && drawn > 0,
          `${drawn} of ${LAYER_SIDE_PX * LAYER_SIDE_PX} texels drawn, ${bad} not finite or of α outside [0, 1]`,
        );
      }
    } finally {
      engine.releaseMaterial(materials.march);
      engine.releaseMaterial(materials.resolve);
      engine.releaseMaterial(materials.composite);
      engine.releaseMaterial(materials.map);
      engine.releaseMesh(materials.triangle);
    }
  } finally {
    target.dispose();
    scene.dispose();
    engine.releaseTexture(tables.body);
    volumes.dispose();
    layers.dispose();
  }
}
/** The place of 0–30° N, 0–60° E whose clouds' share is the largest at a 20 km footprint. */
function cloudiestPlace(twin: CloudLightInput): {
  readonly latitudeDeg: number;
  readonly longitudeDeg: number;
} {
  let best = { latitudeDeg: 15, longitudeDeg: 25 };
  let most = -1;
  for (let latitudeDeg = 0; latitudeDeg <= 30; latitudeDeg += 1) {
    for (let longitudeDeg = 0; longitudeDeg <= 60; longitudeDeg += 1) {
      const u = directionAt(latitudeDeg, longitudeDeg);
      const r = twin.shell.equatorialRadiusM;
      const share = cloudFieldAt(
        twin.coverage,
        twin.noise,
        twin.params,
        { x: u.x * r, y: u.y * r, z: u.z * r },
        20_000,
      ).fraction;
      if (share > most) {
        most = share;
        best = { latitudeDeg, longitudeDeg };
      }
    }
  }
  return best;
}

/** The clouds' captures' side, px. */
const CAPTURE_SIDE_PX = 384;

/** The frames a captured volume is drawn, for its history to settle. */
const CAPTURE_VOLUME_FRAMES = 24;

/**
 * The clouds' captures' views (plan R11, R11.T7.d's check by hand): the Earth-like body from orbit,
 * its disc whole, at an equinox; from 400 km; from within the layer; and from the ground beneath it.
 */
const CAPTURE_VIEWS = [
  { name: "orbit", heightM: 3 * FIGURE.equatorialRadiusM, pitchRad: -Math.PI / 2, fovXRad: 0.62 },
  { name: "above", heightM: 400_000, pitchRad: -0.35, fovXRad: 1.2 },
  { name: "within", heightM: cloudLayerHeightM(CONDENSATE), pitchRad: 0.02, fovXRad: 1.2 },
  { name: "below", heightM: 300, pitchRad: 0.6, fovXRad: 1.2 },
] as const;

/**
 * R11.T7.d's captures by hand: the Earth-like body's clouds over black at an equinox, from each of
 * {@link CAPTURE_VIEWS}, on each setting (the layer at the low setting's 9 octaves, the volume at
 * the high setting's 12, its history settled), with no air, for the zonal check against ISCCP's
 * pattern: from orbit over 15° N, 25° E, the sun over the equator at 25° E; from nearer, over the
 * cloudiest place of 0–30° N, 0–60° E by the field's twin at a 20 km footprint, the sun a little
 * east of its zenith.
 */
export async function captureClouds(engine: RenderEngine): Promise<CapturedImage[]> {
  const frame = { ...FRAME, season: { subsolarLatitudeRad: 0 } };
  const layers = new CloudLayers(engine, "capture clouds");
  layers.step(() => performance.now(), Number.POSITIVE_INFINITY);
  const volumes = new CloudVolumes(engine, "capture clouds");
  const correction = volumes.correction(
    CONDENSATE,
    () => performance.now(),
    Number.POSITIVE_INFINITY,
  );
  const tables = createAirlessTables(engine, "cloud captures: no air");
  const size = { widthPx: CAPTURE_SIDE_PX, heightPx: CAPTURE_SIDE_PX };
  const scene = engine.createRenderTarget({
    name: "cloud captures scene",
    size,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const target = engine.createRenderTarget({
    name: "cloud captures",
    size,
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
  const images: CapturedImage[] = [];
  try {
    // One body's rows a frame: each setting's are written just before its draws.
    const rowsOn = (octaves: number): ReturnType<CloudLayers["frame"]> =>
      layers.frame({ ...frame, octaves });
    if (rowsOn(9) === null || scene.depth === null || correction === null) {
      throw new Error("the clouds' state, their correction or the scene's depth was not made");
    }
    scene.render(frameOf("cloud captures scene", []));
    const layer = engine.createMaterial(CLOUD_LAYER_MATERIAL);
    const materials = {
      march: engine.createMaterial(CLOUD_VOLUME_MATERIAL),
      resolve: engine.createMaterial(CLOUD_RESOLVE_MATERIAL),
      composite: engine.createMaterial(CLOUD_COMPOSITE_MATERIAL),
      map: engine.createMaterial(CLOUD_MAP_MATERIAL),
      shadow: engine.createMaterial(CLOUD_SHADOW_MAP_MATERIAL),
      triangle: fullScreenMesh(engine, "cloud captures' triangle"),
    };
    volumes.prepareNoises(frame.id);
    const shell = cloudShellOf(FIGURE, cloudLayerHeightM(CONDENSATE));
    const { viewRotation, projection } = frameOf("cloud captures", []);
    const sun = directionAt(0, 25);
    const cloudiest = cloudiestPlace(twinOf(cloudNoiseTexels(), { ...frame, octaves: 12 }));
    for (const view of CAPTURE_VIEWS) {
      const orbit = view.name === "orbit";
      const base = {
        ...layerViewOf(view, shell, size, 9, orbit ? undefined : cloudiest),
        deck: { baseM: CONDENSATE.baseAltitudeM, topM: CONDENSATE.topAltitudeM },
      };
      const camera = { ...base.camera, fovXRad: view.fovXRad };
      const lit = orbit
        ? {
            ...base,
            camera,
            suns: [
              {
                directionBodyFixed: sun,
                illuminanceLx: [120_000, 118_000, 110_000] as const,
                angularRadiusRad: 4.65e-3,
                curveSlot: 0,
              },
            ],
          }
        : { ...base, camera };
      // The layer at the low setting's octaves.
      const low = rowsOn(9);
      if (low === null) {
        throw new Error("the clouds' rows were not made");
      }
      target.render(frameOf(`cloud captures ${view.name} low`, []));
      target.render({
        ...frameOf(`cloud captures ${view.name} layer`, [
          cloudLayerDraw(
            materials.triangle,
            layer,
            low,
            scene.depth,
            tables,
            cloudLayerUniforms(lit),
          ),
        ]),
        colourLoad: "load",
      });
      // The harness's captures run in order: each reads the GPU back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const layerTexels = halfTexels(await engine.readTexture(target.colour));
      images.push(hdrCapture(`clouds-${view.name}-low-layer`, layerTexels, size, 1, 1));
      // The volume at the high setting's, its history settled.
      const high = rowsOn(12);
      if (high === null) {
        throw new Error("the clouds' rows were not made");
      }
      volumes.cut();
      for (let at = 0; at < CAPTURE_VOLUME_FRAMES; at += 1) {
        target.render(frameOf(`cloud captures ${view.name} high`, []));
        volumes.draw(
          {
            id: frame.id,
            gpu: high,
            condensate: CONDENSATE,
            view: { ...lit, octaves: 12 },
            lighting: tables,
            sceneDepth: scene.depth,
            correction,
            timeS: MARCH_TIME_S,
            rate: FRAME.rate,
            levers: VOLUME_LEVERS.full,
          },
          materials,
          target,
          size,
          { viewRotation, projection },
          null,
        );
      }
      // The harness's captures run in order: each reads the GPU back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const volumeTexels = halfTexels(await engine.readTexture(target.colour));
      images.push(hdrCapture(`clouds-${view.name}-high-volume`, volumeTexels, size, 1, 1));
    }
  } finally {
    target.dispose();
    scene.dispose();
    engine.releaseTexture(tables.body);
    volumes.dispose();
    layers.dispose();
  }
  return images;
}

/** R11.T7.b's kernels compiled: the layer, the probe and the terrain's cloudy forms (`CLOUD_ENTRIES`). */
export async function checkCloudCatalogue(
  engine: RenderEngine,
  status: GraphicsStatusStore,
  checks: Checks,
): Promise<void> {
  await checkCatalogue(engine, status, CLOUD_ENTRIES, checks);
}
