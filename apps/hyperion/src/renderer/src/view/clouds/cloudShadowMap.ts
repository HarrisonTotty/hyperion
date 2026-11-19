/**
 * The cloud volume's shadow map (plan R11, R11.T7.c; Design note 10): the volume's light on the
 * ground about the camera, made each frame before the terrain draws, which the terrain's cloudy
 * forms take into R10's `terrain_shadow_factor` (the direct part) and `terrain_sky_factor` (the
 * diffuse part), and which any other pass that lights a surface under the clouds, the sea's among
 * them, samples the same way (`cloudGround.wgsl`, its binding contract in its header); the CPU
 * reference of `cloudShadowMap.wgsl` and `cloudGround.wgsl`.
 *
 * @remarks
 * - **Its plane** is the cloud map's (`cloudMap.ts`), its first two levels': 256² texels each, 160 m
 *   and 640 m across, 20 km and 82 km either side of the camera, so that a texel's place on the
 *   ground is the map's own, held still as the camera moves.
 * - **A texel** at its place on the datum holds the three factors of `terrain_cloud_light`
 *   (`cloudShadow.wgsl`) for the frame's brightest source: x the direct transmittance e^(−τ_sun),
 *   τ_sun the volume's density marched along the ray towards the source through the deck
 *   ({@link SHADOW_SUN_STEPS} steps), so that each cumulus casts its own shadow; and, from the
 *   column above the place ({@link SHADOW_COLUMN_STEPS} steps, τ_v), y the sky's light through it,
 *   δ-Eddington's total transmittance at the diffusivity's cosine 0.6, and z the source's light it
 *   scatters down, δ-Eddington's T_diff at the source's cosine. The density is the march's
 *   (`cloudMarch.ts`), at the texel's footprint, without the detail noise.
 * - **A read** (`cloudGround.wgsl`'s `cloud_ground_light`) at a place takes level 0 within 0.8 of
 *   its half-width, blends to level 1 across the rest, and beyond 0.8 of level 1's blends to the
 *   layer's own tap, which also serves a second source, the low setting, and a frame that draws no
 *   volume: so the map changes nothing where it has nothing to say.
 * - Memory: 256 × 512 `rgba16float`, 1 MiB, and 9 rows, in the `clouds` category.
 */
import type { Vec3 } from "../../geometry/vec3";
import type { WgslMaterialSpec } from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import {
  CLOUD_MAP_MEMBERS,
  CLOUD_MAP_SIDE,
  type CloudMapFrame,
  cloudMapUniforms,
  mapTexelM,
  texelPlace,
} from "./cloudMap";
import { type CloudMarchScene, deckStretch, densityPerMAt } from "./cloudMarch";
import { deltaEddington } from "./cloudOptics";
import type { CloudGroundLight } from "./cloudShadow";
import cloudGroundWgsl from "./shaders/cloudGround.wgsl?raw";
import cloudOpticsWgsl from "./shaders/cloudOptics.wgsl?raw";
import cloudShadowMapWgsl from "./shaders/cloudShadowMap.wgsl?raw";
import cloudShapeWgsl from "./shaders/cloudShape.wgsl?raw";
import cloudVolumeWgsl from "./shaders/cloudVolume.wgsl?raw";
import { CLOUD_SHADOW_ROWS } from "./cloudShadowRows";

/** The shadow map's levels: the cloud map's first two. */
export const SHADOW_MAP_LEVELS = 2;

/** The steps of a texel's march towards the source through the deck. */
export const SHADOW_SUN_STEPS = 12;

/** The steps of its march up the column above it. */
export const SHADOW_COLUMN_STEPS = 8;

/** The share of a level's half-width read before the next level, or the layer, blends in. */
export const SHADOW_INNER = 0.8;

export { CLOUD_SHADOW_ROWS } from "./cloudShadowRows";

/** The diffusivity's cosine the sky's light takes through the clouds. */
const SKY_MU = 0.6;

/** The least cosine of the source the slab's optics are taken at. */
const LEAST_MU = 0.02;

/** What a frame's shadow map stands for: its plane, its source and its deck. */
export interface CloudShadowFrame {
  readonly map: CloudMapFrame;
  /** The unit direction to the source it was made for, body-fixed. */
  readonly sun: Vec3;
  /** a, the datum's equatorial radius, m, by which the terrain's places ÷ a are turned to metres. */
  readonly datumRadiusM: number;
  /** k, the deck's spheroids' a ÷ c. */
  readonly flattening: number;
}

/**
 * The shadow map's rows (`cloudGround.wgsl`'s layout), into `out`: 0 to 5 the map's plane
 * (`cloudMapUniforms`, its members in order); 6 the source's direction and 1 where the map holds; 7
 * x the datum's a, m, y k, z level 1's half-width over level 0's, w {@link SHADOW_INNER}; 8 unused.
 * `null` packs a map that holds nothing, so that every read takes the layer's tap.
 */
export function packCloudShadow(
  frame: CloudShadowFrame | null,
  out: Float32Array = new Float32Array(4 * CLOUD_SHADOW_ROWS),
): Float32Array {
  out.fill(0);
  if (frame === null) {
    return out;
  }
  const plane = cloudMapUniforms(frame.map);
  CLOUD_MAP_MEMBERS.forEach((name, i) => {
    out.set(plane[name] ?? new Float32Array(4), 4 * i);
  });
  out.set([frame.sun.x, frame.sun.y, frame.sun.z, 1], 24);
  out.set([frame.datumRadiusM, frame.flattening, mapTexelM(1) / mapTexelM(0), SHADOW_INNER], 28);
  return out;
}

/** The shadow map pass's `Draw` members after `offsetFromCameraM`. */
const SHADOW_MEMBERS = [...CLOUD_MAP_MEMBERS, "sun", "deck", "optics"] as const;

/** The shadow map pass's names of its textures, samplers and rows. */
export const CLOUD_SHADOW_TEXTURES = {
  map: "cloudMap",
  mapSampler: "cloudMapSampler",
  layer: "cloudLayer",
  shape: "cloudShape",
  baseNoise: "cloudBaseNoise",
  detailNoise: "cloudDetailNoise",
  curlNoise: "cloudCurlNoise",
  noiseSampler: "cloudNoiseRepeat",
  fieldNoise: "cloudFieldNoise",
  correction: "cloudOctaveCorrection",
} as const;

/** The shadow map pass's WGSL: the frame, the trigonometry, the density, the optics and the pass. */
const SHADOW_WGSL = `${frameWgsl}\n${preciseTrigWgsl}\n${cloudShapeWgsl}\n${cloudOpticsWgsl}\n${cloudVolumeWgsl}\n${cloudShadowMapWgsl}`;

/** The shadow map's pass (`cloudShadowMap.wgsl`): a full-screen triangle into its two levels. */
export const CLOUD_SHADOW_MAP_MATERIAL: WgslMaterialSpec = {
  name: "cloud shadow map",
  displayName: "CLOUD SHADOW MAP",
  vertexWgsl: SHADOW_WGSL,
  fragmentWgsl: SHADOW_WGSL,
  uniforms: SHADOW_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
  samplers: [
    {
      name: CLOUD_SHADOW_TEXTURES.mapSampler,
      filter: "linear",
      address: "clamp-to-edge",
      binding: 1,
    },
    {
      name: CLOUD_SHADOW_TEXTURES.noiseSampler,
      filter: "linear",
      address: "repeat",
      binding: 8,
    },
  ],
  textures: [
    { name: CLOUD_SHADOW_TEXTURES.map, binding: 0 },
    { name: CLOUD_SHADOW_TEXTURES.baseNoise, binding: 4, viewDimension: "3d" },
    { name: CLOUD_SHADOW_TEXTURES.detailNoise, binding: 5, viewDimension: "3d" },
    { name: CLOUD_SHADOW_TEXTURES.curlNoise, binding: 6 },
    { name: CLOUD_SHADOW_TEXTURES.fieldNoise, binding: 7, viewDimension: "3d" },
    {
      name: CLOUD_SHADOW_TEXTURES.correction,
      binding: 9,
      sampleType: "unfilterable-float",
    },
  ],
  storageBuffers: [
    { name: CLOUD_SHADOW_TEXTURES.layer, binding: 2 },
    { name: CLOUD_SHADOW_TEXTURES.shape, binding: 3 },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The label of the shadow map's pass in `PassTimes`. */
export const CLOUD_SHADOW_PASS = "clouds shadow";

/** The WGSL a pass that reads the shadow map composes (`cloudGround.wgsl`). */
export const CLOUD_GROUND_WGSL = cloudGroundWgsl;

/**
 * Level k's texel (i, j)'s place on the datum, body-fixed, m: the map's texel place, its height
 * taken off by the shell's (`cloudMap.ts`' `texelPlace` on the datum's spheroid).
 */
export function shadowTexelPlace(
  frame: CloudShadowFrame,
  level: number,
  i: number,
  j: number,
): Vec3 {
  const a = frame.datumRadiusM;
  return texelPlace(
    frame.map,
    { equatorialRadiusM: a, polarRadiusM: a / frame.flattening },
    level,
    i,
    j,
  );
}

/**
 * The shadow map's texel at a place on the datum (the module's remarks), by the march's twin
 * (`cloudMarch.ts`): the direct transmittance towards `frame.sun` through the deck, and the sky's
 * and the scattered light through the column above it.
 *
 * @param footprintM - The texel's size, m, at which the density is read.
 */
export function shadowTexelAt(
  scene: CloudMarchScene,
  frame: CloudShadowFrame,
  placeM: Vec3,
  footprintM: number,
): CloudGroundLight {
  const k = scene.flattening;
  const n = { x: placeM.x, y: placeM.y, z: placeM.z * k * k };
  const length = Math.hypot(n.x, n.y, n.z);
  const up = { x: n.x / length, y: n.y / length, z: n.z / length };
  const depthOf = (direction: Vec3, steps: number): number => {
    const stretch = deckStretch(
      placeM,
      direction,
      k,
      scene.datumRadiusM,
      scene.inputs.baseM,
      scene.topM,
    );
    if (stretch === null) {
      return 0;
    }
    const [t0, t1] = stretch;
    const dt = (t1 - t0) / steps;
    let tau = 0;
    for (let s = 0; s < steps; s += 1) {
      const t = t0 + (s + 0.5) * dt;
      const x = {
        x: placeM.x + t * direction.x,
        y: placeM.y + t * direction.y,
        z: placeM.z + t * direction.z,
      };
      tau += densityPerMAt(scene, x, footprintM) * dt;
    }
    return tau;
  };
  const tauSun = depthOf(frame.sun, SHADOW_SUN_STEPS);
  const tauColumn = depthOf(up, SHADOW_COLUMN_STEPS);
  const mu0 = Math.max(up.x * frame.sun.x + up.y * frame.sun.y + up.z * frame.sun.z, LEAST_MU);
  const albedo = 1 - scene.optics.coAlbedo;
  const sun = deltaEddington({
    tau: tauColumn,
    asymmetry: scene.optics.asymmetry,
    singleScatterAlbedo: albedo,
    mu0,
  });
  const sky = deltaEddington({
    tau: tauColumn,
    asymmetry: scene.optics.asymmetry,
    singleScatterAlbedo: albedo,
    mu0: SKY_MU,
  });
  return {
    direct: Math.exp(-tauSun),
    sky: sky.directTransmittance + Math.max(sky.diffuseTransmittance, 0),
    diffuse: Math.max(sun.diffuseTransmittance, 0),
  };
}

/** The shadow map's draw members for a frame's map, source and deck. */
export function shadowMapUniforms(
  frame: CloudShadowFrame,
  deck: { readonly baseM: number; readonly topM: number },
  shellRadiusM: number,
  optics: { readonly asymmetry: number; readonly coAlbedo: number },
): Readonly<Record<string, Float32Array>> {
  return {
    ...cloudMapUniforms(frame.map),
    sun: new Float32Array([frame.sun.x, frame.sun.y, frame.sun.z, frame.datumRadiusM]),
    deck: new Float32Array([deck.baseM, deck.topM, frame.flattening, shellRadiusM]),
    optics: new Float32Array([optics.asymmetry, optics.coAlbedo, CLOUD_MAP_SIDE, 0]),
  };
}
