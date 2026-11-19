/**
 * The cloud volume's passes (plan R11, R11.T7.c; Design notes 10 and 18): the high setting's clouds
 * of one body marched through their three-dimensional density at a quarter of the view's pixels
 * with temporal reprojection, then laid over the body's air at the view's own resolution, as the
 * two-dimensional layer is laid.
 *
 * @remarks
 * - **Four passes a frame** (`cloudMap.wgsl`, `cloudVolumeMarch.wgsl`, `cloudVolumeResolve.wgsl`,
 *   `cloudVolumeComposite.wgsl`): the cloud map, the layer's field about the camera at five scales
 *   (`cloudMap.ts`); the march of each pixel's ray through the deck's density (`cloudShape.ts`,
 *   `cloudMarch.ts`) into the volume's target, at half the view's width and height
 *   ({@link volumeTargetSize}), its steps offset by the blue noise moved by the frame's jitter
 *   ({@link volumeJitter}); the resolve, which blends it with the last frame's resolved clouds
 *   where the same cloud stood then ({@link reprojectedUv}), held to the march's 3 × 3 range, into
 *   the other of two histories; and the composite, bilinear, over the target the body's composite
 *   drew, where the scene stands no nearer than the deck.
 * - **Which body.** The nearest of the frame's bodies whose clouds the layer would draw, on a
 *   setting whose `clouds` is `volume`, while the camera stands within {@link VOLUME_REACH_M} of
 *   its deck's top ({@link withinVolumeReach}); farther, and for every other body, the layer, which
 *   the volume has become by then ({@link volumeFlatteningAt}), so that the hand-over shows no
 *   switch (the owner's requirement, 2026-11-19: one volumetric field from orbit to the ground).
 *   The frame-rate floor's step no longer sheds the volume for the layer; it takes its coarser
 *   levers ({@link VOLUME_LEVERS}). Until the condensate's correction and the world's noises are
 *   made, the layer stands in.
 * - **The correction** of the octaves to δ-Eddington (`cloudVolume.ts`' `octaveCorrectionOf`) is
 *   made a μ₀ row a step over frames within the frame's budget ({@link OctaveCorrectionBuilder}),
 *   per condensate's optics, into an `r32float` texture of 2 KiB in the `clouds` category.
 * - **The world's noises** (`cloudShapeNoise.ts`), about 2 MiB, are made by a worker from its
 *   cloud seed, one world at a time, the last the volume drew.
 * - **The history** holds where the last frame resolved the same body at the same size; a cut, a
 *   new body or a new size begins again from the march alone.
 * - Memory, in `clouds`: three quarter-resolution `rgba16float` targets, about 12.4 MiB at 1080p;
 *   the map, 2.5 MiB; the noises, 2.05 MiB; 2 KiB a condensate.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { cross, dot, norm, scale, sub, type Vec3 } from "../../geometry/vec3";
import { tan } from "../../lib/pinnedMath";
import { type SpheroidFigure, geodeticOf } from "../atmosphere/hillaire";
import {
  SURFACE_LIGHTING_WGSL,
  type SurfaceLightingTables,
  surfaceLightingDrawTextures,
  surfaceLightingTextures,
} from "../atmosphere/surfaceLighting";
import { NEAR_PLANE_M } from "../camera/projection";
import type { Quaternion } from "../camera/pose";
import { rotate } from "../camera/quaternion";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  RenderTarget,
  TexelRect,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
import { HDR_COLOUR_FORMAT } from "../photometry/toneCurve";
import { type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import frameWgsl from "../shaders/frame.wgsl?raw";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import {
  CLOUD_LAYER_PASS,
  CLOUD_TEXTURES,
  type CloudBodyGpu,
  type CloudLayerView,
  cloudLayerUniforms,
} from "./cloudLayer";
import { cloudSeedOf, type CoverageTexels, sampleCoverage } from "./cloudDensity";
import {
  CLOUD_MAP_LEVELS,
  CLOUD_MAP_MEMBERS,
  CLOUD_MAP_SIDE,
  CloudMapAnchor,
  type CloudMapFrame,
  cloudMapUniforms,
} from "./cloudMap";
import {
  CLOUD_SHADOW_MAP_MATERIAL,
  CLOUD_SHADOW_PASS,
  CLOUD_SHADOW_ROWS,
  CLOUD_SHADOW_TEXTURES,
  packCloudShadow,
  SHADOW_MAP_LEVELS,
  shadowMapUniforms,
} from "./cloudShadowMap";
import type { CloudMarchLevers } from "./cloudMarch";
import {
  BASE_FEATURE_TILES,
  CELL_FEATURE_TILES,
  type CloudShapeFrame,
  cloudShapeFrame,
  type CloudShapeKind,
  cloudShapeKindOf,
  CURL_TILES,
  DETAIL_FEATURE_TILES,
  normalisationTable,
} from "./cloudShape";
import {
  BASE_NOISE_SIZE,
  CloudShapeNoiseSource,
  CURL_NOISE_SIZE,
  DETAIL_NOISE_SIZE,
  type ShapeNoiseWorkerLike,
} from "./cloudShapeNoise";
import { CORRECTION_MUS, CORRECTION_TAUS, octaveCorrectionRow } from "./cloudVolume";
import type { CloudCondensate, CloudOptics } from "./condensates";
import cloudFieldWgsl from "./shaders/cloudField.wgsl?raw";
import cloudMapWgsl from "./shaders/cloudMap.wgsl?raw";
import cloudShapeWgsl from "./shaders/cloudShape.wgsl?raw";
import cloudOpticsWgsl from "./shaders/cloudOptics.wgsl?raw";
import cloudVolumeWgsl from "./shaders/cloudVolume.wgsl?raw";
import cloudVolumeCompositeWgsl from "./shaders/cloudVolumeComposite.wgsl?raw";
import cloudVolumeMarchWgsl from "./shaders/cloudVolumeMarch.wgsl?raw";
import cloudVolumeResolveWgsl from "./shaders/cloudVolumeResolve.wgsl?raw";

/** The label of the volume's march in `PassTimes`. */
export const CLOUD_VOLUME_PASS = "clouds volume";

/** The label of its temporal resolve. */
export const CLOUD_RESOLVE_PASS = "clouds resolve";

/** The volume's levers at each step of the frame-rate floor. */
export type VolumeStep = "full" | "stepped" | "lowest";

/**
 * The march's levers at each step of the frame-rate floor (lean, 2026-11-19): in full, 64 view
 * samples, 6 towards the source and the detail noise; once the floor steps the air down, 40, 4 and
 * no detail; at its lowest step, 24 and 3. The floor degrades the volume and never sheds it
 * (the owner's requirement, 2026-11-19); the levers are the owner's Mac pair (`--cloud-steps`).
 */
export const VOLUME_LEVERS: Readonly<Record<VolumeStep, CloudMarchLevers>> = {
  full: { viewSteps: 64, lightSteps: 6, detail: true },
  stepped: { viewSteps: 40, lightSteps: 4, detail: false },
  lowest: { viewSteps: 24, lightSteps: 3, detail: false },
};

/**
 * The floor's step a frame's volume takes: `stepped` while the floor draws its air at a lower
 * setting's sizes than the frame's (`PhotorealFrame.atmosphereSetting`), `lowest` at its lowest
 * step (`PhotorealFrame.floorLowest`), else `full`.
 */
export function volumeStepOf(
  setting: QualitySetting,
  atmosphereSetting: QualitySetting | undefined,
  floorLowest: boolean | undefined,
): VolumeStep {
  if (floorLowest === true) {
    return "lowest";
  }
  return (atmosphereSetting ?? setting) === setting ? "full" : "stepped";
}

/**
 * The camera's height above its deck's top within which the volume is drawn, m (lean, 2026-11-19):
 * 300 km, below most low orbits' clouds seen from above, where the volume has flattened to the
 * layer's slab ({@link volumeFlatteningAt}); above it the layer, from orbit and for distant discs.
 */
export const VOLUME_REACH_M = 300_000;

/** The height above the deck's top from which the volume flattens towards the layer, m. */
export const VOLUME_FLAT_FROM_M = 100_000;

/**
 * φ, the volume's flattening towards the layer's slab (`cloudShape.ts`' remarks) at the camera's
 * height above its deck's top: 0 within {@link VOLUME_FLAT_FROM_M}, rising smoothly to 1 at
 * {@link VOLUME_REACH_M}, where the layer takes over.
 */
export function volumeFlatteningAt(heightAboveTopM: number): number {
  const t = Math.min(
    Math.max((heightAboveTopM - VOLUME_FLAT_FROM_M) / (VOLUME_REACH_M - VOLUME_FLAT_FROM_M), 0),
    1,
  );
  return t * t * (3 - 2 * t);
}

/** Whether a camera stands within the volume's reach of a deck (the module's remarks). */
export function withinVolumeReach(heightAboveTopM: number): boolean {
  return heightAboveTopM < VOLUME_REACH_M;
}

/**
 * Whether a frame draws a body's clouds as the volume (the module's remarks): on a setting whose
 * `clouds` is `volume`, within the deck's reach, whatever the frame-rate floor's step.
 */
export function volumeWanted(setting: QualitySetting, heightAboveTopM: number): boolean {
  return SETTINGS[setting].clouds === "volume" && withinVolumeReach(heightAboveTopM);
}

/** The unit direction to a source, body-fixed, from its offset in the frame's axes. */
export function towardsBodyFixed(toBodyFixed: Quaternion, towardsM: Vec3): Vec3 {
  const d = rotate(toBodyFixed, towardsM);
  return scale(d, 1 / norm(d));
}

/** A camera's height above a deck's top, m, from its place, body-fixed, on the body's datum. */
export function heightAboveDeckM(
  cameraM: Vec3,
  figure: SpheroidFigure,
  condensate: Pick<CloudCondensate, "topAltitudeM">,
): number {
  return geodeticOf(cameraM, figure).heightM - condensate.topAltitudeM;
}

/**
 * The weight of a frame's march in its resolve, the rest the reprojected history's (lean,
 * 2026-11-18): an eighth, some 16 frames to settle within 12%.
 */
export const TEMPORAL_WEIGHT = 0.125;

/** The golden ratio's fraction, by which the frames' jitter steps (an additive recurrence). */
const JITTER_STEP = 0.6180339887498949;

/** The march's bindings in `@group(2)` (`cloudVolumeMarch.wgsl`'s). */
export const CLOUD_VOLUME_BINDINGS = {
  sceneDepth: 0,
  map: 1,
  mapSampler: 2,
  layer: 3,
  shape: 4,
  /** The first of R08's surface lighting's nine tables. */
  atmosphere: 5,
  correction: 14,
  fieldNoise: 15,
  baseNoise: 16,
  detailNoise: 17,
  curlNoise: 18,
  noiseSampler: 19,
} as const;

/** The names a volume's draws bind their own textures, samplers and rows under. */
export const CLOUD_VOLUME_TEXTURES = {
  correction: "cloudOctaveCorrection",
  fieldNoise: "cloudFieldNoise",
  march: "cloudMarch",
  history: "cloudHistory",
  historySampler: "cloudHistorySampler",
  resolved: "cloudResolved",
  resolvedSampler: "cloudResolvedSampler",
  map: "cloudMap",
  mapSampler: "cloudMapSampler",
  shape: "cloudShape",
  baseNoise: "cloudBaseNoise",
  detailNoise: "cloudDetailNoise",
  curlNoise: "cloudCurlNoise",
  noiseSampler: "cloudNoiseRepeat",
} as const;

/** The march's `Draw` members after `offsetFromCameraM`, each a `vec4f`. */
const MARCH_MEMBERS = [
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
  "deck",
  ...CLOUD_MAP_MEMBERS,
  "march",
] as const;

/**
 * The march's WGSL: `frame.wgsl`, R08's surface lighting, the density, the optics, the volume's
 * light and the pass.
 */
export const CLOUD_VOLUME_WGSL = `${frameWgsl}\n${SURFACE_LIGHTING_WGSL}\n${cloudShapeWgsl}\n${cloudOpticsWgsl}\n${cloudVolumeWgsl}\n${cloudVolumeMarchWgsl}`;

/** The volume's march: a full-screen triangle into the volume's quarter-resolution target. */
export const CLOUD_VOLUME_MATERIAL: WgslMaterialSpec = {
  name: "cloud volume",
  displayName: "CLOUD VOLUME",
  vertexWgsl: CLOUD_VOLUME_WGSL,
  fragmentWgsl: CLOUD_VOLUME_WGSL,
  uniforms: MARCH_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [
    {
      name: CLOUD_VOLUME_TEXTURES.mapSampler,
      filter: "linear",
      address: "clamp-to-edge",
      binding: CLOUD_VOLUME_BINDINGS.mapSampler,
    },
    {
      name: CLOUD_VOLUME_TEXTURES.noiseSampler,
      filter: "linear",
      address: "repeat",
      binding: CLOUD_VOLUME_BINDINGS.noiseSampler,
    },
  ],
  textures: [
    { name: "sceneDepth", binding: CLOUD_VOLUME_BINDINGS.sceneDepth, sampleType: "depth" },
    { name: CLOUD_VOLUME_TEXTURES.map, binding: CLOUD_VOLUME_BINDINGS.map },
    ...surfaceLightingTextures(CLOUD_VOLUME_BINDINGS.atmosphere),
    {
      name: CLOUD_VOLUME_TEXTURES.correction,
      binding: CLOUD_VOLUME_BINDINGS.correction,
      sampleType: "unfilterable-float",
    },
    {
      name: CLOUD_VOLUME_TEXTURES.fieldNoise,
      binding: CLOUD_VOLUME_BINDINGS.fieldNoise,
      viewDimension: "3d",
    },
    {
      name: CLOUD_VOLUME_TEXTURES.baseNoise,
      binding: CLOUD_VOLUME_BINDINGS.baseNoise,
      viewDimension: "3d",
    },
    {
      name: CLOUD_VOLUME_TEXTURES.detailNoise,
      binding: CLOUD_VOLUME_BINDINGS.detailNoise,
      viewDimension: "3d",
    },
    { name: CLOUD_VOLUME_TEXTURES.curlNoise, binding: CLOUD_VOLUME_BINDINGS.curlNoise },
  ],
  storageBuffers: [
    { name: CLOUD_TEXTURES.layer, binding: CLOUD_VOLUME_BINDINGS.layer },
    { name: CLOUD_VOLUME_TEXTURES.shape, binding: CLOUD_VOLUME_BINDINGS.shape },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The map pass's `Draw` members after `offsetFromCameraM`. */
const MAP_MEMBERS = [...CLOUD_MAP_MEMBERS, "shape"] as const;

/** The cloud map's pass (`cloudMap.wgsl`): a full-screen triangle into the map's levels. */
export const CLOUD_MAP_MATERIAL: WgslMaterialSpec = {
  name: "cloud map",
  displayName: "CLOUD MAP",
  vertexWgsl: `${frameWgsl}\n${preciseTrigWgsl}\n${cloudFieldWgsl}\n${cloudMapWgsl}`,
  fragmentWgsl: `${frameWgsl}\n${preciseTrigWgsl}\n${cloudFieldWgsl}\n${cloudMapWgsl}`,
  uniforms: MAP_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [{ name: CLOUD_TEXTURES.sampler, filter: "linear", address: "repeat", binding: 2 }],
  textures: [
    { name: CLOUD_TEXTURES.coverage, binding: 0 },
    { name: CLOUD_TEXTURES.noise, binding: 1, viewDimension: "3d" },
  ],
  storageBuffers: [{ name: CLOUD_TEXTURES.layer, binding: 3 }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The label of the cloud map's pass in `PassTimes`. */
export const CLOUD_MAP_PASS = "clouds map";

/** The resolve's `Draw` members after `offsetFromCameraM`. */
const RESOLVE_MEMBERS = [
  "camera",
  "right",
  "upRay",
  "forward",
  "shell",
  "moved",
  "lastRight",
  "lastUp",
  "lastForward",
  "view",
] as const;

/** The volume's temporal resolve: the march blended with the reprojected history. */
export const CLOUD_RESOLVE_MATERIAL: WgslMaterialSpec = {
  name: "cloud volume resolve",
  displayName: "CLOUD VOLUME RESOLVE",
  vertexWgsl: `${frameWgsl}\n${cloudVolumeResolveWgsl}`,
  fragmentWgsl: `${frameWgsl}\n${cloudVolumeResolveWgsl}`,
  uniforms: RESOLVE_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [
    {
      name: CLOUD_VOLUME_TEXTURES.historySampler,
      filter: "linear",
      address: "clamp-to-edge",
      binding: 2,
    },
  ],
  textures: [
    { name: CLOUD_VOLUME_TEXTURES.march, binding: 0 },
    { name: CLOUD_VOLUME_TEXTURES.history, binding: 1 },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The composite's `Draw` members after `offsetFromCameraM`. */
const COMPOSITE_MEMBERS = ["camera", "right", "upRay", "forward", "deck", "view"] as const;

/** The volume's composite over the body's air, premultiplied, keeping the meter class. */
export const CLOUD_COMPOSITE_MATERIAL: WgslMaterialSpec = {
  name: "cloud volume composite",
  displayName: "CLOUD VOLUME COMPOSITE",
  vertexWgsl: `${frameWgsl}\n${cloudVolumeCompositeWgsl}`,
  fragmentWgsl: `${frameWgsl}\n${cloudVolumeCompositeWgsl}`,
  uniforms: COMPOSITE_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [
    {
      name: CLOUD_VOLUME_TEXTURES.resolvedSampler,
      filter: "linear",
      address: "clamp-to-edge",
      binding: 2,
    },
  ],
  textures: [
    { name: "sceneDepth", binding: 0, sampleType: "depth" },
    { name: CLOUD_VOLUME_TEXTURES.resolved, binding: 1 },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "premultiplied",
};

/**
 * The volume's five materials, in the order the renderer makes them (`march`, `resolve`,
 * `composite`, `map`, `shadow`), for the catalogue (compiled in the smoke's part 26).
 */
export const CLOUD_VOLUME_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  CLOUD_VOLUME_MATERIAL,
  CLOUD_RESOLVE_MATERIAL,
  CLOUD_COMPOSITE_MATERIAL,
  CLOUD_MAP_MATERIAL,
  CLOUD_SHADOW_MAP_MATERIAL,
];

/** The volume's targets' size for a view of `size`: half its width and height, rounded up. */
export function volumeTargetSize(size: ViewSize): ViewSize {
  return {
    widthPx: Math.max(1, Math.ceil(size.widthPx / 2)),
    heightPx: Math.max(1, Math.ceil(size.heightPx / 2)),
  };
}

/** Frame `index`'s jitter of the march's steps, in [0, 1): the golden ratio's recurrence. */
export function volumeJitter(index: number): number {
  const at = index * JITTER_STEP;
  return at - Math.floor(at);
}

/**
 * A condensate's correction table made a μ₀ row a step (`cloudVolume.ts`' `octaveCorrectionRow`),
 * so that its making, some tens of milliseconds, is spread over frames; the table at its last
 * step is `octaveCorrectionOf`'s, bit for bit.
 */
export class OctaveCorrectionBuilder {
  readonly #optics: CloudOptics;
  readonly #factors = new Float32Array(CORRECTION_TAUS * CORRECTION_MUS);
  #row = 0;

  constructor(optics: CloudOptics) {
    this.#optics = optics;
  }

  /** Does one row; the finished factors once the last is done, `null` before. */
  step(): Float32Array | null {
    if (this.#row < CORRECTION_MUS) {
      octaveCorrectionRow(this.#optics, this.#row, this.#factors);
      this.#row += 1;
    }
    return this.#row === CORRECTION_MUS ? this.#factors : null;
  }
}

/** A camera as the resolve reprojects into it: its place and basis, body-fixed. */
export interface VolumeCamera {
  /** Its place from the body's centre, body-fixed, m. */
  readonly positionM: Vec3;
  /** Its rays' basis: right × tan(fovX ÷ 2), up × tan(fovY ÷ 2), forward, body-fixed. */
  readonly right: Vec3;
  readonly up: Vec3;
  readonly forward: Vec3;
}

/** A camera's place and basis from the layer's view of it. */
export function volumeCameraOf(view: Pick<CloudLayerView, "camera">): VolumeCamera {
  const { camera } = view;
  const tanX = tan(camera.fovXRad / 2);
  const tanY = tanX / (camera.viewport.widthPx / camera.viewport.heightPx);
  const right = rotate(camera.orientation, { x: 1, y: 0, z: 0 });
  const up = rotate(camera.orientation, { x: 0, y: 1, z: 0 });
  return {
    positionM: camera.positionM,
    right: { x: right.x * tanX, y: right.y * tanX, z: right.z * tanX },
    up: { x: up.x * tanY, y: up.y * tanY, z: up.z * tanY },
    forward: rotate(camera.orientation, { x: 0, y: 0, z: -1 }),
  };
}

/**
 * Where a place, body-fixed, m, stood in `last`'s view, as `cloudVolumeResolve.wgsl` reprojects
 * it: its texture coordinates from the top left, or `null` behind that camera or outside its view.
 */
export function reprojectedUv(
  placeM: Vec3,
  last: VolumeCamera,
): { readonly u: number; readonly v: number } | null {
  const v = sub(placeM, last.positionM);
  const z = dot(v, last.forward);
  if (!(z > 0)) {
    return null;
  }
  const rightSq = dot(last.right, last.right);
  const upSq = dot(last.up, last.up);
  const x = dot(v, last.right) / rightSq / z;
  const y = dot(v, last.up) / upSq / z;
  if (Math.abs(x) > 1 || Math.abs(y) > 1) {
    return null;
  }
  return { u: 0.5 + 0.5 * x, v: 0.5 - 0.5 * y };
}

/** A body's volume this frame: what its map, march, resolve and composite draw from. */
export interface CloudVolumeDraw {
  readonly id: BodyIdHex;
  readonly gpu: CloudBodyGpu;
  readonly condensate: CloudCondensate;
  /** The layer's view of the body this frame, at the frame's own viewport. */
  readonly view: CloudLayerView;
  readonly lighting: SurfaceLightingTables;
  readonly sceneDepth: TextureHandle;
  readonly correction: TextureHandle;
  /** The scene's time, s on the universe clock, by which the noises drift with the wind. */
  readonly timeS: number;
  /** Simulation seconds per real second, which fades the noises as the field's octaves. */
  readonly rate: number;
  /** The march's levers at the frame-rate floor's step ({@link VOLUME_LEVERS}). */
  readonly levers: CloudMarchLevers;
}

/** The volume's materials and the full-screen triangle they draw. */
export interface CloudVolumeMaterials {
  readonly march: MaterialHandle;
  readonly resolve: MaterialHandle;
  readonly composite: MaterialHandle;
  readonly map: MaterialHandle;
  readonly shadow: MaterialHandle;
  readonly triangle: MeshHandle;
}

/** What a body's volume is prepared from before its ground draws ({@link CloudVolumes.prepare}). */
export interface CloudVolumePrepare {
  readonly id: BodyIdHex;
  readonly gpu: CloudBodyGpu;
  readonly condensate: CloudCondensate;
  readonly view: Pick<CloudLayerView, "camera" | "figure" | "shell" | "octaves">;
  readonly correction: TextureHandle;
  readonly timeS: number;
  readonly rate: number;
  /** The unit direction to the frame's brightest source, body-fixed; `null` for none. */
  readonly sun: Vec3 | null;
}

/** A pass's rotation and projection, which its full-screen draws read none of. */
interface PassMatrices {
  readonly viewRotation: Float32Array;
  readonly projection: Float32Array;
}

/** A view's volume targets: the march's and the two histories, at a quarter of the view's pixels. */
interface VolumeTargets {
  readonly march: RenderTarget;
  readonly histories: readonly [RenderTarget, RenderTarget];
  readonly size: ViewSize;
}

/** The time a correction table's making may take a frame, ms. */
export const CORRECTION_STEP_BUDGET_MS = 1;

/** The shape's rows (`cloudShape.wgsl`'s CLOUD_SHAPE_ROWS). */
export const CLOUD_SHAPE_ROWS = 25;

/**
 * A deck's shape rows (`cloudShape.wgsl`'s layout), into `out`, {@link CLOUD_SHAPE_ROWS} × 4 `f32`s:
 * its noises' tiles and drifts, their features and time-rate shares, its kind, κ's normalisation
 * table.
 */
export function packCloudShape(
  frame: CloudShapeFrame,
  kind: CloudShapeKind,
  out: Float32Array = new Float32Array(4 * CLOUD_SHAPE_ROWS),
): Float32Array {
  out.fill(0);
  out.set(
    [
      1 / frame.baseTileM,
      1 / frame.detailTileM,
      1 / (frame.detailTileM * CURL_TILES),
      kind.profileExponent,
    ],
    0,
  );
  const { baseDrift: b, detailDrift: d } = frame;
  out.set([b.x, b.y, b.z, frame.baseRateShare], 4);
  out.set([d.x, d.y, d.z, frame.detailRateShare], 8);
  out.set(
    [
      frame.baseTileM * BASE_FEATURE_TILES,
      frame.detailTileM * DETAIL_FEATURE_TILES,
      kind.cumuliform ? 1 : 0,
      frame.baseTileM * CELL_FEATURE_TILES,
    ],
    12,
  );
  out.set(normalisationTable(kind.profileExponent), 16);
  return out;
}

/**
 * The wind at a camera's place from its body's coverage texels, as a body-fixed velocity, m s⁻¹:
 * the texels' eastward and northward wind there along the place's east and north.
 */
export function windAtM(coverage: CoverageTexels, cameraM: Vec3): Vec3 {
  const cell = sampleCoverage(coverage, cameraM);
  const up = scale(cameraM, 1 / norm(cameraM));
  let east = cross({ x: 0, y: 0, z: 1 }, up);
  if (norm(east) < 1e-9) {
    east = { x: 0, y: 1, z: 0 };
  }
  east = scale(east, 1 / norm(east));
  const north = cross(up, east);
  return {
    x: cell.eastMps * east.x + cell.northMps * north.x,
    y: cell.eastMps * east.y + cell.northMps * north.y,
    z: cell.eastMps * east.z + cell.northMps * north.z,
  };
}

/** The engine's calls a view's cloud volume makes. */
type VolumeEngine = Pick<
  RenderEngine,
  | "createRenderTarget"
  | "createTexture"
  | "writeTexture"
  | "releaseTexture"
  | "createBuffer"
  | "writeBuffer"
  | "releaseBuffer"
>;

/** A world's shape noises on the GPU. */
interface NoiseTextures {
  readonly seed: number;
  readonly base: TextureHandle;
  readonly detail: TextureHandle;
  readonly curl: TextureHandle;
}

/** Starts the shape-noise worker on the renderer's own module, where workers run. */
export function browserShapeNoiseWorker(): ShapeNoiseWorkerLike {
  return new Worker(new URL("./shapeNoise.worker.ts", import.meta.url), {
    type: "module",
    name: "cloud shape noise",
  });
}

/** The page's worker where one can run, else none, so that the builder makes the noises. */
function defaultWorker(): ShapeNoiseWorkerLike | null {
  return typeof Worker === "function" ? browserShapeNoiseWorker() : null;
}

/**
 * A view's cloud volume (the module's remarks): its targets and history, made at its first frame
 * and at a new size; its map; each condensate's correction texture, made over frames; and the
 * drawn world's noises; it owns them and none of the materials.
 */
export class CloudVolumes {
  readonly #engine: VolumeEngine;
  readonly #name: string;
  readonly #corrections = new Map<
    string,
    { builder: OctaveCorrectionBuilder | null; texture: TextureHandle | null }
  >();
  readonly #anchor = new CloudMapAnchor();
  readonly #noises: CloudShapeNoiseSource;
  readonly #shapeRows = new Float32Array(4 * CLOUD_SHAPE_ROWS);
  #targets: VolumeTargets | null = null;
  #map: RenderTarget | null = null;
  #shape: BufferHandle | null = null;
  #shadow: RenderTarget | null = null;
  #shadowRows: BufferHandle | null = null;
  readonly #shadowRowsData = new Float32Array(4 * CLOUD_SHADOW_ROWS);
  /** The map this frame's {@link CloudVolumes.prepare} drew, which its draw takes. */
  #prepared: {
    readonly id: BodyIdHex;
    readonly mapFrame: CloudMapFrame;
    readonly mapUniforms: Readonly<Record<string, Float32Array>>;
    readonly shape: BufferHandle;
  } | null = null;
  #noiseTextures: NoiseTextures | null = null;
  /** The history the last frame resolved into, and what it resolved; `null` where none holds. */
  #last: {
    readonly at: 0 | 1;
    readonly id: BodyIdHex;
    readonly camera: VolumeCamera;
  } | null = null;
  #frames = 0;

  /**
   * @param worker - The shape-noise worker, `null` for none, when the noises are made a step a
   *   frame; the page's own where it can run by default.
   */
  constructor(
    engine: VolumeEngine,
    name: string,
    worker: ShapeNoiseWorkerLike | null = defaultWorker(),
  ) {
    this.#engine = engine;
    this.#name = name;
    this.#noises = new CloudShapeNoiseSource(worker);
  }

  /**
   * The correction texture of a condensate's optics, made a row a step for up to `budgetMs` of
   * `nowMs`'s clock at each call until it is made; `null` before.
   */
  correction(
    optics: CloudOptics,
    nowMs: () => number,
    budgetMs = CORRECTION_STEP_BUDGET_MS,
  ): TextureHandle | null {
    const key = `${String(optics.asymmetry)}/${String(optics.coAlbedo)}`;
    let held = this.#corrections.get(key);
    if (held === undefined) {
      held = { builder: new OctaveCorrectionBuilder(optics), texture: null };
      this.#corrections.set(key, held);
    }
    const { builder } = held;
    if (builder !== null) {
      const start = nowMs();
      let factors = builder.step();
      while (factors === null && nowMs() - start < budgetMs) {
        factors = builder.step();
      }
      if (factors !== null) {
        const size = { width: CORRECTION_TAUS, height: CORRECTION_MUS };
        const texture = this.#engine.createTexture({
          name: `${this.#name} cloud correction ${key}`,
          size,
          dimension: "2d",
          format: "r32float",
          mips: 1,
          usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
          category: "clouds",
        });
        this.#engine.writeTexture(texture, { x: 0, y: 0 }, size, factors);
        held.texture = texture;
        held.builder = null;
      }
    }
    return held.texture;
  }

  /**
   * Whether a body's world's shape noises are made and on the GPU, asking for them where they are
   * not: the worker makes them, or the builder within `budgetMs` of `nowMs`'s clock a call.
   */
  noisesReady(id: BodyIdHex, nowMs: () => number): boolean {
    const seed = cloudSeedOf(id);
    if (this.#noiseTextures?.seed === seed) {
      return true;
    }
    const made = this.#noises.noisesOf(seed, nowMs);
    if (made === null) {
      return false;
    }
    this.#upload(seed, made);
    return true;
  }

  /** Makes a body's world's shape noises at once, for a still's first sample. */
  prepareNoises(id: BodyIdHex): void {
    const seed = cloudSeedOf(id);
    if (this.#noiseTextures?.seed !== seed) {
      this.#upload(seed, this.#noises.noisesNow(seed));
    }
  }

  #upload(seed: number, made: { base: Uint8Array; detail: Uint8Array; curl: Uint8Array }): void {
    this.#releaseNoises();
    const cube = (what: string, side: number, texels: Uint8Array): TextureHandle => {
      const size = { width: side, height: side, depthOrArrayLayers: side };
      const texture = this.#engine.createTexture({
        name: `${this.#name} cloud ${what} noise`,
        size,
        dimension: "3d",
        format: "r8unorm",
        mips: 1,
        usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
        category: "clouds",
      });
      this.#engine.writeTexture(texture, { x: 0, y: 0, z: 0 }, size, texels);
      return texture;
    };
    const curlSize = { width: CURL_NOISE_SIZE, height: CURL_NOISE_SIZE };
    const curl = this.#engine.createTexture({
      name: `${this.#name} cloud curl noise`,
      size: curlSize,
      dimension: "2d",
      format: "r8unorm",
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "clouds",
    });
    this.#engine.writeTexture(curl, { x: 0, y: 0 }, curlSize, made.curl);
    this.#noiseTextures = {
      seed,
      base: cube("base", BASE_NOISE_SIZE, made.base),
      detail: cube("detail", DETAIL_NOISE_SIZE, made.detail),
      curl,
    };
  }

  /** Drops the history, so that the next frame resolves from its march alone (a cut). */
  cut(): void {
    this.#last = null;
  }

  /**
   * Prepares a body's volume this frame before its ground draws: its map, and its shadow map for
   * the brightest source, `null` for none, whose texture and rows the ground beneath reads
   * (`cloudGround.wgsl`), returned; the frame's {@link CloudVolumes.draw} then draws from the map.
   * The world's noises must be ready.
   *
   * @throws Error where the world's noises are not on the GPU.
   */
  prepare(
    volume: CloudVolumePrepare,
    materials: Pick<CloudVolumeMaterials, "map" | "shadow" | "triangle">,
    pass: PassMatrices,
  ): { readonly shadowMap: TextureHandle; readonly shadowRows: BufferHandle } {
    const noises = this.#noiseTextures;
    if (noises?.seed !== cloudSeedOf(volume.id)) {
      throw new Error(
        `the cloud volume of ${volume.id} was prepared before its world's noises were made`,
      );
    }
    const mapped = this.#mapped(volume, materials, pass);
    this.#prepared = { id: volume.id, ...mapped };
    const rows = this.#shadowRowsBuffer();
    const shadow = this.#shadowTarget();
    const { view, gpu, condensate, sun } = volume;
    if (sun === null) {
      this.#engine.writeBuffer(rows, 0, packCloudShadow(null, this.#shadowRowsData));
      return { shadowMap: shadow.colour, shadowRows: rows };
    }
    const k = view.shell.equatorialRadiusM / view.shell.polarRadiusM;
    const frame = {
      map: mapped.mapFrame,
      sun,
      datumRadiusM: view.figure.equatorialRadiusM,
      flattening: k,
    };
    shadow.render({
      label: CLOUD_SHADOW_PASS,
      viewRotation: pass.viewRotation,
      projection: pass.projection,
      draws: [
        {
          mesh: materials.triangle,
          material: materials.shadow,
          offsetFromCameraM: new Float32Array(3),
          uniforms: shadowMapUniforms(
            frame,
            { baseM: condensate.baseAltitudeM, topM: condensate.topAltitudeM },
            view.shell.equatorialRadiusM,
            condensate,
          ),
          textures: {
            [CLOUD_SHADOW_TEXTURES.map]: this.#mapTarget().colour,
            [CLOUD_SHADOW_TEXTURES.baseNoise]: noises.base,
            [CLOUD_SHADOW_TEXTURES.detailNoise]: noises.detail,
            [CLOUD_SHADOW_TEXTURES.curlNoise]: noises.curl,
            [CLOUD_SHADOW_TEXTURES.fieldNoise]: gpu.noise,
            [CLOUD_SHADOW_TEXTURES.correction]: volume.correction,
          },
          storageBuffers: {
            [CLOUD_SHADOW_TEXTURES.layer]: gpu.layer,
            [CLOUD_SHADOW_TEXTURES.shape]: mapped.shape,
          },
        },
      ],
      postProcesses: [],
    });
    this.#engine.writeBuffer(rows, 0, packCloudShadow(frame, this.#shadowRowsData));
    return { shadowMap: shadow.colour, shadowRows: rows };
  }

  /**
   * A frame's map of a body's volume: its noises' shape rows written, its map's frame from the
   * anchor and its pass drawn.
   */
  #mapped(
    volume: Pick<CloudVolumeDraw, "gpu" | "condensate" | "timeS" | "rate"> & {
      readonly view: Pick<CloudLayerView, "camera" | "shell" | "octaves">;
    },
    materials: Pick<CloudVolumeMaterials, "map" | "triangle">,
    pass: PassMatrices,
  ): {
    readonly mapFrame: CloudMapFrame;
    readonly mapUniforms: Readonly<Record<string, Float32Array>>;
    readonly shape: BufferHandle;
  } {
    const { view, gpu, condensate } = volume;
    const depthM = Math.max(condensate.topAltitudeM - condensate.baseAltitudeM, 1);
    const kind = cloudShapeKindOf(condensate);
    const shapeFrame = cloudShapeFrame(
      depthM,
      gpu.texels === undefined ? { x: 0, y: 0, z: 0 } : windAtM(gpu.texels, view.camera.positionM),
      volume.timeS,
      volume.rate,
    );
    const shape = this.#shapeBuffer();
    this.#engine.writeBuffer(shape, 0, packCloudShape(shapeFrame, kind, this.#shapeRows));
    const mapFrame = this.#anchor.frameFor(view.camera.positionM, view.shell.equatorialRadiusM);
    const mapUniforms = cloudMapUniforms(mapFrame);
    this.#mapTarget().render({
      label: CLOUD_MAP_PASS,
      viewRotation: pass.viewRotation,
      projection: pass.projection,
      draws: [
        {
          mesh: materials.triangle,
          material: materials.map,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            ...mapUniforms,
            shape: new Float32Array([
              view.shell.equatorialRadiusM / view.shell.polarRadiusM,
              view.octaves,
              0,
              0,
            ]),
          },
          textures: {
            [CLOUD_TEXTURES.coverage]: gpu.coverage,
            [CLOUD_TEXTURES.noise]: gpu.noise,
          },
          storageBuffers: { [CLOUD_TEXTURES.layer]: gpu.layer },
        },
      ],
      postProcesses: [],
    });
    return { mapFrame, mapUniforms, shape };
  }

  /**
   * Draws a body's volume: its map, its march and its resolve into the volume's own targets, then
   * its composite into `target`, within `scissor` where given, over what the body's composite drew.
   * The world's noises must be ready ({@link CloudVolumes.noisesReady}).
   *
   * @param size - The frame's own size, the scene depth's.
   * @throws Error where the world's noises are not on the GPU.
   */
  draw(
    volume: CloudVolumeDraw,
    materials: CloudVolumeMaterials,
    target: RenderTarget,
    size: ViewSize,
    pass: PassMatrices,
    scissor: TexelRect | null,
  ): void {
    const noises = this.#noiseTextures;
    if (noises?.seed !== cloudSeedOf(volume.id)) {
      throw new Error(
        `the cloud volume of ${volume.id} was drawn before its world's noises were made`,
      );
    }
    const targets = this.#targetsOf(size);
    const quarter = targets.size;
    const { view, gpu, condensate } = volume;
    const marchView: CloudLayerView = {
      ...view,
      camera: { ...view.camera, viewport: quarter },
    };
    const layer = cloudLayerUniforms(marchView);
    const jitter = volumeJitter(this.#frames);
    this.#frames += 1;
    // The map this frame's preparation drew, or drawn here where none was.
    const prepared = this.#prepared?.id === volume.id ? this.#prepared : null;
    this.#prepared = null;
    const { mapUniforms, shape } = prepared ?? this.#mapped(volume, materials, pass);
    const mapTarget = this.#mapTarget();
    const flat = volumeFlatteningAt(
      heightAboveDeckM(view.camera.positionM, view.figure, condensate),
    );
    targets.march.render({
      label: CLOUD_VOLUME_PASS,
      viewRotation: pass.viewRotation,
      projection: pass.projection,
      draws: [
        {
          mesh: materials.triangle,
          material: materials.march,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            ...layer,
            levers: new Float32Array([
              volume.levers.viewSteps,
              volume.levers.lightSteps,
              jitter,
              volume.levers.detail ? 1 : 0,
            ]),
            deck: new Float32Array([
              condensate.baseAltitudeM,
              condensate.topAltitudeM,
              size.widthPx,
              size.heightPx,
            ]),
            ...mapUniforms,
            march: new Float32Array([flat, 0, 0, 0]),
          },
          textures: {
            sceneDepth: volume.sceneDepth,
            [CLOUD_VOLUME_TEXTURES.map]: mapTarget.colour,
            ...surfaceLightingDrawTextures(volume.lighting),
            [CLOUD_VOLUME_TEXTURES.correction]: volume.correction,
            [CLOUD_VOLUME_TEXTURES.fieldNoise]: gpu.noise,
            [CLOUD_VOLUME_TEXTURES.baseNoise]: noises.base,
            [CLOUD_VOLUME_TEXTURES.detailNoise]: noises.detail,
            [CLOUD_VOLUME_TEXTURES.curlNoise]: noises.curl,
          },
          storageBuffers: {
            [CLOUD_TEXTURES.layer]: gpu.layer,
            [CLOUD_VOLUME_TEXTURES.shape]: shape,
          },
        },
      ],
      postProcesses: [],
    });
    const camera = volumeCameraOf(marchView);
    const last = this.#last?.id === volume.id ? this.#last : null;
    const into: 0 | 1 = last === null ? 0 : last.at === 0 ? 1 : 0;
    const from = targets.histories[into === 0 ? 1 : 0];
    const moved =
      last === null ? { x: 0, y: 0, z: 0 } : sub(camera.positionM, last.camera.positionM);
    const lastCamera = last?.camera ?? camera;
    const shell = memberOf(layer, "shell");
    targets.histories[into].render({
      label: CLOUD_RESOLVE_PASS,
      viewRotation: pass.viewRotation,
      projection: pass.projection,
      draws: [
        {
          mesh: materials.triangle,
          material: materials.resolve,
          offsetFromCameraM: new Float32Array(3),
          uniforms: {
            camera: new Float32Array([
              camera.positionM.x,
              camera.positionM.y,
              camera.positionM.z,
              last === null ? 0 : 1,
            ]),
            right: memberOf(layer, "right"),
            upRay: memberOf(layer, "upRay"),
            forward: memberOf(layer, "forward"),
            shell: new Float32Array([shell[0] ?? 0, shell[1] ?? 1, 0, TEMPORAL_WEIGHT]),
            moved: new Float32Array([moved.x, moved.y, moved.z, 0]),
            lastRight: overSquare(lastCamera.right),
            lastUp: overSquare(lastCamera.up),
            lastForward: new Float32Array([
              lastCamera.forward.x,
              lastCamera.forward.y,
              lastCamera.forward.z,
              0,
            ]),
            view: new Float32Array([quarter.widthPx, quarter.heightPx, 0, 0]),
          },
          textures: {
            [CLOUD_VOLUME_TEXTURES.march]: targets.march.colour,
            [CLOUD_VOLUME_TEXTURES.history]: from.colour,
          },
        },
      ],
      postProcesses: [],
    });
    this.#last = { at: into, id: volume.id, camera };
    target.render({
      label: CLOUD_LAYER_PASS,
      viewRotation: pass.viewRotation,
      projection: pass.projection,
      draws: [
        {
          mesh: materials.triangle,
          material: materials.composite,
          offsetFromCameraM: new Float32Array(3),
          uniforms: compositeUniforms(view, condensate),
          textures: {
            sceneDepth: volume.sceneDepth,
            [CLOUD_VOLUME_TEXTURES.resolved]: targets.histories[into].colour,
          },
        },
      ],
      postProcesses: [],
      colourLoad: "load",
      ...(scissor === null ? {} : { scissor }),
    });
  }

  /** Lets go of every condensate's correction but those of `kept`'s optics, by their keys. */
  keepOnly(kept: ReadonlyArray<CloudOptics>): void {
    const keys = new Set(
      kept.map((optics) => `${String(optics.asymmetry)}/${String(optics.coAlbedo)}`),
    );
    for (const [key, held] of this.#corrections) {
      if (!keys.has(key)) {
        if (held.texture !== null) {
          this.#engine.releaseTexture(held.texture);
        }
        this.#corrections.delete(key);
      }
    }
  }

  /** Lets go of everything, the worker with it. */
  dispose(): void {
    this.keepOnly([]);
    this.#releaseTargets();
    this.#releaseNoises();
    this.#map?.dispose();
    this.#map = null;
    this.#shadow?.dispose();
    this.#shadow = null;
    if (this.#shape !== null) {
      this.#engine.releaseBuffer(this.#shape);
      this.#shape = null;
    }
    if (this.#shadowRows !== null) {
      this.#engine.releaseBuffer(this.#shadowRows);
      this.#shadowRows = null;
    }
    this.#prepared = null;
    this.#noises.dispose();
    this.#anchor.reset();
    this.#last = null;
  }

  #releaseNoises(): void {
    const held = this.#noiseTextures;
    if (held !== null) {
      this.#engine.releaseTexture(held.base);
      this.#engine.releaseTexture(held.detail);
      this.#engine.releaseTexture(held.curl);
      this.#noiseTextures = null;
    }
  }

  #mapTarget(): RenderTarget {
    if (this.#map === null) {
      this.#map = this.#engine.createRenderTarget({
        name: `${this.#name}:cloud map`,
        size: { widthPx: CLOUD_MAP_SIDE, heightPx: CLOUD_MAP_SIDE * CLOUD_MAP_LEVELS },
        format: HDR_COLOUR_FORMAT,
        mips: 1,
        depth: false,
        category: "clouds",
      });
    }
    return this.#map;
  }

  #shadowTarget(): RenderTarget {
    if (this.#shadow === null) {
      this.#shadow = this.#engine.createRenderTarget({
        name: `${this.#name}:cloud shadow`,
        size: { widthPx: CLOUD_MAP_SIDE, heightPx: CLOUD_MAP_SIDE * SHADOW_MAP_LEVELS },
        format: HDR_COLOUR_FORMAT,
        mips: 1,
        depth: false,
        category: "clouds",
      });
    }
    return this.#shadow;
  }

  #shadowRowsBuffer(): BufferHandle {
    if (this.#shadowRows === null) {
      this.#shadowRows = this.#engine.createBuffer({
        name: `${this.#name} cloud shadow rows`,
        bytes: 16 * CLOUD_SHADOW_ROWS,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "clouds",
      });
    }
    return this.#shadowRows;
  }

  #shapeBuffer(): BufferHandle {
    if (this.#shape === null) {
      this.#shape = this.#engine.createBuffer({
        name: `${this.#name} cloud shape`,
        bytes: 16 * CLOUD_SHAPE_ROWS,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "clouds",
      });
    }
    return this.#shape;
  }

  #targetsOf(size: ViewSize): VolumeTargets {
    const quarter = volumeTargetSize(size);
    const held = this.#targets;
    if (
      held !== null &&
      held.size.widthPx === quarter.widthPx &&
      held.size.heightPx === quarter.heightPx
    ) {
      return held;
    }
    this.#releaseTargets();
    this.#last = null;
    const made = (what: string): RenderTarget =>
      this.#engine.createRenderTarget({
        name: `${this.#name}:cloud ${what}`,
        size: quarter,
        format: HDR_COLOUR_FORMAT,
        mips: 1,
        depth: false,
        category: "clouds",
      });
    const targets: VolumeTargets = {
      march: made("march"),
      histories: [made("history 0"), made("history 1")],
      size: quarter,
    };
    this.#targets = targets;
    return targets;
  }

  #releaseTargets(): void {
    const held = this.#targets;
    if (held !== null) {
      held.march.dispose();
      for (const history of held.histories) {
        history.dispose();
      }
      this.#targets = null;
    }
  }
}

/**
 * The layer's uniform `name` (`cloudLayerUniforms`'), which the volume's passes share.
 *
 * @throws Error where the layer's uniforms hold no such member, a renamed one.
 */
function memberOf(uniforms: Readonly<Record<string, Float32Array>>, name: string): Float32Array {
  const member = uniforms[name];
  if (member === undefined) {
    throw new Error(`the cloud layer's uniforms hold no ${name}`);
  }
  return member;
}

/** `v` over its squared length, as a `vec4f`: a ray basis's axis ÷ its tangent. */
function overSquare(v: Vec3): Float32Array {
  const sq = dot(v, v);
  return new Float32Array([v.x / sq, v.y / sq, v.z / sq, 0]);
}

/**
 * The composite's uniforms: the camera's place and rays at the frame's own viewport, and the deck's
 * flattening and its top's and base's quadric constants, |S o|² − r², formed in `f64`.
 */
export function compositeUniforms(
  view: CloudLayerView,
  condensate: Pick<CloudCondensate, "baseAltitudeM" | "topAltitudeM">,
): Readonly<Record<string, Float32Array>> {
  const layer = cloudLayerUniforms(view);
  const o = view.camera.positionM;
  const k = view.shell.equatorialRadiusM / view.shell.polarRadiusM;
  const scaled = o.x * o.x + o.y * o.y + o.z * k * (o.z * k);
  const a = view.figure.equatorialRadiusM;
  const top = a + condensate.topAltitudeM;
  const base = a + condensate.baseAltitudeM;
  return {
    camera: new Float32Array([o.x, o.y, o.z, norm(o)]),
    right: memberOf(layer, "right"),
    upRay: memberOf(layer, "upRay"),
    forward: memberOf(layer, "forward"),
    deck: new Float32Array([k, scaled - top * top, scaled - base * base, NEAR_PLANE_M]),
    view: new Float32Array([view.camera.viewport.widthPx, view.camera.viewport.heightPx, 0, 0]),
  };
}
