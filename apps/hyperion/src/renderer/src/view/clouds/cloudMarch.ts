/**
 * The cloud volume's march through its three-dimensional density (plan R11, R11.T7.c; Design note
 * 10): a view ray's stretch of the deck, each sample's density from the cloud map's column
 * (`cloudMap.ts`) and the shape noises (`cloudShape.ts`), its sunlight through a march towards the
 * source, its sky light by its depth in its column; the `f64` twin of `cloudVolume.wgsl`'s
 * `cloud_march`.
 *
 * @remarks
 * - **The stretch** is the ray's first through the deck, from above, from below or from within it
 *   ({@link deckStretch}), cut by the caller where the scene stands nearer. Its samples are
 *   uniform in v ∈ [0, 1) at (s + j) ÷ N for the frame's jitter j, at t₀ + L v^p, p 1 for a stretch
 *   of at most 8 deck depths and growing to 2 at 64 ({@link stepWarp}), so that a long, grazing
 *   stretch, the horizon's from the ground or within the deck, takes its fine steps near the
 *   camera.
 * - **The share.** Where the footprint at the entry resolves the field (its unresolved deviation s
 *   small) every sample takes its own column's mean τ, the clouds' own structure; where it does
 *   not, as the layer would see it, the ray is drawn in the covered share f of the entry's
 *   footprint and each column's τ over it, the layer's independent-pixel form (Design note 10), so
 *   that the volume from afar is the layer: share = 1 − w (1 − f), w = smoothstep(0.1, 0.5, s),
 *   and 1 at full flattening ({@link rayShare}).
 * - **The sun's optical depth** to a sample is marched towards the source over
 *   {@link LIGHT_REACH_DEPTHS} deck depths at most, its steps growing as the square of their
 *   index, the samples reading the base noise alone (normalised by G(w_b, 0)), so that a cloud's
 *   side away from the source is shaded by the cloud itself (Schneider and Vos 2015's light march);
 *   a second source takes the column's plane-parallel path, its share above the sample over the
 *   source's cosine, and the flattening mixes the first's to it too.
 * - **The light**, per unit of each source's irradiance face-on: Hillaire 2016's energy-conserving
 *   step, the two-lobe phase and Wrenninge et al. 2013's octaves with their correction K to
 *   δ-Eddington at the sample's column τ (`cloudVolume.ts`); the sky's, per unit of its irradiance
 *   ÷ π, the slab's diffuse reflectance at the column's top going to its diffuse transmittance at
 *   its base by the share of its τ above the sample (the deck's fog's lean of 2026-11-18, taken
 *   through the cloud), the slab's the conservative two-stream's, R = ¾(1 − g)τ ÷ (1 + ¾(1 − g)τ),
 *   mixed to δ-Eddington's at the entry's column by the flattening.
 * - The march stops once the view's transmittance falls under {@link LEAST_TRANSMITTANCE}.
 */
import { type Vec3 } from "../../geometry/vec3";
import type { CloudMapTwin } from "./cloudMap";
import { deltaEddington } from "./cloudOptics";
import {
  baseNoiseAt,
  BASE_FEATURE_TILES,
  type CloudColumn,
  type CloudShapeFrame,
  columnPlace,
  type DensityInputs,
  densityPerM,
  DETAIL_FEATURE_TILES,
  detailNoiseAt,
  noiseShare,
  shareAbove,
} from "./cloudShape";
import type { CloudShapeNoises } from "./cloudShapeNoise";
import {
  CLOUD_OCTAVES,
  CLOUD_VOLUME_LEAST_MU,
  escape,
  type OctaveCorrection,
  octaveCorrectionAt,
  twoLobeOf,
  twoLobePhase,
} from "./cloudVolume";
import type { CloudOptics } from "./condensates";

/** The deck depths a sample's march towards the source reaches at most. */
export const LIGHT_REACH_DEPTHS = 3;

/** The least cosine the march towards the source takes its path's length at. */
export const LIGHT_LEAST_MU = 1 / LIGHT_REACH_DEPTHS;

/** The view's transmittance under which the march stops. */
export const LEAST_TRANSMITTANCE = 3e-3;

/** The unresolved deviations between which a ray turns from its own columns to the layer's share. */
export const SHARE_SD_RANGE = [0.1, 0.5] as const;

/** The least share a ray is drawn in. */
export const LEAST_SHARE = 1e-4;

/** The cosine of the sky's light on the slab: 0.6, the two-stream diffusivity's 1 ÷ 1.66. */
export const SKY_MU = 0.6;

function clamp01(x: number): number {
  return Math.min(Math.max(x, 0), 1);
}

function smoothstep(a: number, b: number, x: number): number {
  const t = clamp01((x - a) / (b - a));
  return t * t * (3 - 2 * t);
}

/** A ray's distances to the spheroid |S x| = r, S = diag(1, 1, k), near then far; `null` if it misses. */
function spheroidHits(o: Vec3, d: Vec3, k: number, r: number): readonly [number, number] | null {
  const so = { x: o.x, y: o.y, z: o.z * k };
  const sd = { x: d.x, y: d.y, z: d.z * k };
  const a = sd.x * sd.x + sd.y * sd.y + sd.z * sd.z;
  const b = so.x * sd.x + so.y * sd.y + so.z * sd.z;
  const c = so.x * so.x + so.y * so.y + so.z * so.z - r * r;
  const disc = b * b - a * c;
  if (disc < 0) {
    return null;
  }
  const root = Math.sqrt(disc);
  return [(-b - root) / a, (-b + root) / a];
}

/**
 * A ray's first stretch through the deck between `baseM` and `topM` above the datum `datumM`, its
 * surfaces the spheroids of flattening k (the layer's a_s ÷ c_s): from above to the base or out of
 * the top again, from below from the base, from within from the camera; `null` where it has none.
 */
export function deckStretch(
  originM: Vec3,
  direction: Vec3,
  k: number,
  datumM: number,
  baseM: number,
  topM: number,
): readonly [number, number] | null {
  const outer = spheroidHits(originM, direction, k, datumM + topM);
  if (outer === null || outer[1] <= 0) {
    return null;
  }
  const inner = spheroidHits(originM, direction, k, datumM + baseM) ?? [-1, -1];
  let t0 = Math.max(outer[0], 0);
  let t1 = outer[1];
  if (inner[0] > t0) {
    t1 = inner[0];
  } else if (inner[0] < 0 && inner[1] > t0) {
    t0 = inner[1];
  }
  return t1 > t0 ? [t0, t1] : null;
}

/** p of a stretch's warp t₀ + L v^p (the module's remarks): 1 to 8 depths, 2 from 64. */
export function stepWarp(lengthM: number, depthM: number): number {
  return 1 + clamp01(Math.log2(Math.max(lengthM / (8 * depthM), 1)) / 3);
}

/** The share a ray is drawn in from its entry's column (the module's remarks). */
export function rayShare(entry: CloudColumn, flat: number): number {
  const w = smoothstep(SHARE_SD_RANGE[0], SHARE_SD_RANGE[1], entry.unresolvedSd);
  const weight = w + flat * (1 - w);
  return 1 - weight * (1 - entry.fraction);
}

/**
 * The column at a ray's entry into the deck at `t0`, over the pixel's footprint there, its growth
 * with distance over the view's cosine against the deck, which sets the ray's share.
 */
export function marchEntry(
  map: CloudMapTwin,
  originM: Vec3,
  direction: Vec3,
  t0: number,
  k: number,
  footprintPerM: number,
): CloudColumn {
  const x0 = along(originM, direction, t0);
  const n = { x: x0.x, y: x0.y, z: x0.z * k * k };
  const muView = Math.max(
    Math.abs(dot3(direction, n)) / Math.hypot(n.x, n.y, n.z),
    CLOUD_VOLUME_LEAST_MU,
  );
  return map.columnAt(x0, Math.max((footprintPerM * t0) / muView, 1));
}

/** The conservative two-stream's diffuse reflectance of a slab (the module's remarks). */
export function diffuseReflectance(tau: number, asymmetry: number): number {
  const x = 0.75 * (1 - asymmetry) * Math.max(tau, 0);
  return x / (1 + x);
}

/** What a scene's march reads: its map, noises, density and light. */
export interface CloudMarchScene {
  readonly map: CloudMapTwin;
  readonly noises: CloudShapeNoises;
  readonly frame: CloudShapeFrame;
  readonly inputs: DensityInputs;
  /** The deck's top above the datum, m. */
  readonly topM: number;
  /** a, the datum's equatorial radius, m. */
  readonly datumRadiusM: number;
  /** k, the deck's spheroids' a ÷ c. */
  readonly flattening: number;
  readonly optics: CloudOptics;
  readonly correction: OctaveCorrection;
}

/** The march's levers, which the frame-rate floor steps (`cloudVolumePass.ts`' `VOLUME_LEVERS`). */
export interface CloudMarchLevers {
  /** N, the view's samples. */
  readonly viewSteps: number;
  /** The samples of each march towards the source. */
  readonly lightSteps: number;
  /** Whether the detail noise is read. */
  readonly detail: boolean;
}

/** A view ray's march. */
export interface CloudMarchRay {
  readonly originM: Vec3;
  readonly direction: Vec3;
  /** Its stretch, m along it, cut where the scene stands nearer. */
  readonly t0: number;
  readonly t1: number;
  /** The unit directions to its sources, body-fixed: the first marched, the second plane-parallel. */
  readonly suns: readonly [Vec3, Vec3 | null];
  /** The footprint's growth with distance, m per m; at least 1 m. */
  readonly footprintPerM: number;
  /** The frame's jitter of its samples, in [0, 1). */
  readonly jitter: number;
}

/** A view ray's march's light, per unit of each source's irradiance face-on, and the sky's. */
export interface CloudMarch {
  /** The radiance towards the camera from each source, sr⁻¹, before the share. */
  readonly sun: readonly [number, number];
  /** The sky's, per unit of its irradiance ÷ π, before the share. */
  readonly sky: number;
  /** The view's transmittance through the stretch, before the share. */
  readonly transmittance: number;
  /** The distance along the ray its opacity is weighed at, m; t₀ where it has none. */
  readonly meanDistanceM: number;
  /** The share it is drawn in. */
  readonly share: number;
}

/** A place's height above the datum and its unit normal on the deck's spheroids. */
function heightAndNormal(
  x: Vec3,
  k: number,
  datum: number,
): { readonly height: number; readonly normal: Vec3 } {
  const scaled = Math.hypot(x.x, x.y, x.z * k);
  const n = { x: x.x, y: x.y, z: x.z * k * k };
  const length = Math.hypot(n.x, n.y, n.z);
  return { height: scaled - datum, normal: { x: n.x / length, y: n.y / length, z: n.z / length } };
}

function along(o: Vec3, d: Vec3, t: number): Vec3 {
  return { x: o.x + t * d.x, y: o.y + t * d.y, z: o.z + t * d.z };
}

function dot3(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

/**
 * The density at a place and footprint (the module's remarks), the detail read where its share is
 * above 0 and `detail` holds.
 */
function densityAt(
  scene: CloudMarchScene,
  x: Vec3,
  footprintM: number,
  share: number,
  detail: boolean,
): { readonly sigma: number; readonly column: CloudColumn; readonly above: number } {
  const { inputs, frame, noises, map } = scene;
  const { height } = heightAndNormal(x, scene.flattening, scene.datumRadiusM);
  const column = map.columnAt(x, footprintM);
  const place = columnPlace(height, column.topShare, inputs);
  if (!(column.meanTau > 0) || !(place.u >= 0 && place.u < 1)) {
    return { sigma: 0, column, above: shareAbove(place, inputs) };
  }
  const wb = noiseShare(frame.baseTileM * BASE_FEATURE_TILES, footprintM) * frame.baseRateShare;
  const wd = detail
    ? noiseShare(frame.detailTileM * DETAIL_FEATURE_TILES, footprintM) * frame.detailRateShare
    : 0;
  const nb = wb > 0 ? baseNoiseAt(noises.base, frame, x) : 0.5;
  let nd = 0.5;
  if (wd > 0) {
    const plane = map.planeOf(x);
    nd = detailNoiseAt(
      noises.detail,
      noises.curl,
      frame,
      x,
      plane,
      map.frame.east,
      map.frame.north,
    );
  }
  const sigma = densityPerM(
    column.meanTau / share,
    place,
    { base: nb, detail: nd, baseShare: wb, detailShare: wd },
    inputs,
  );
  return { sigma, column, above: shareAbove(place, inputs) };
}

/** The sun's optical depth to a sample by the march towards it (the module's remarks). */
function sunDepth(
  scene: CloudMarchScene,
  x: Vec3,
  height: number,
  muSun: number,
  sun: Vec3,
  footprintM: number,
  share: number,
  lightSteps: number,
): number {
  const depth = scene.inputs.depthM;
  const reach = Math.min(
    Math.max(scene.topM - height, 0) / Math.max(muSun, LIGHT_LEAST_MU),
    LIGHT_REACH_DEPTHS * depth,
  );
  let tau = 0;
  for (let j = 0; j < lightSteps; j += 1) {
    const at = reach * ((j + 0.5) / lightSteps) ** 2;
    const step = (reach * (2 * j + 1)) / (lightSteps * lightSteps);
    tau += densityAt(scene, along(x, sun, at), footprintM, share, false).sigma * step;
  }
  return tau;
}

/** A view ray's march (the module's remarks). */
export function marchCloudRay(
  scene: CloudMarchScene,
  levers: CloudMarchLevers,
  ray: CloudMarchRay,
  entry: CloudColumn,
): CloudMarch {
  const { originM: o, direction: d, t0, t1, footprintPerM } = ray;
  const flat = scene.inputs.flat;
  const share = rayShare(entry, flat);
  const none = { sun: [0, 0] as const, sky: 0, transmittance: 1, meanDistanceM: t0, share };
  if (!(share >= LEAST_SHARE) || !(t1 > t0)) {
    return none;
  }
  const length = t1 - t0;
  const p = stepWarp(length, scene.inputs.depthM);
  const n = levers.viewSteps;
  const g = scene.optics.asymmetry;
  const albedo = 1 - scene.optics.coAlbedo;
  const lobes = twoLobeOf(g);
  const entryTau = entry.meanTau / share;
  const entrySlab =
    flat > 0 && entryTau > 0
      ? deltaEddington({
          tau: entryTau,
          asymmetry: g,
          singleScatterAlbedo: albedo,
          mu0: SKY_MU,
        })
      : { reflectance: 0, diffuseTransmittance: 0 };
  const suns = ray.suns;
  const cosines = suns.map((s) => (s === null ? 0 : dot3(s, d)));
  const light: [number, number] = [0, 0];
  let sky = 0;
  let view = 1;
  let weighed = 0;
  let weight = 0;
  for (let s = 0; s < n; s += 1) {
    const v = (s + ray.jitter) / n;
    const t = t0 + length * v ** p;
    const dt = (length * p * Math.max(v, 0.5 / n) ** (p - 1)) / n;
    const x = along(o, d, t);
    const footprint = Math.max(footprintPerM * t, 1);
    const { sigma, column, above } = densityAt(scene, x, footprint, share, levers.detail);
    if (!(sigma > 0)) {
      continue;
    }
    const columnTau = column.meanTau / share;
    const { height, normal } = heightAndNormal(x, scene.flattening, scene.datumRadiusM);
    const viewStep = sigma * dt;
    const scattered = view * sigma * dt * escape(viewStep);
    for (const [j, sun] of suns.entries()) {
      if (sun === null) {
        continue;
      }
      const rawMu = dot3(normal, sun);
      if (rawMu < -Math.sqrt((2 * Math.max(height, 0)) / scene.datumRadiusM)) {
        continue;
      }
      const muSun = Math.max(rawMu, CLOUD_VOLUME_LEAST_MU);
      const planeParallel = (columnTau * above) / muSun;
      const marched =
        j === 0 && flat < 1
          ? sunDepth(scene, x, height, rawMu, sun, footprint, share, levers.lightSteps)
          : planeParallel;
      const tauSun = marched + flat * (planeParallel - marched);
      const multiple = octaveCorrectionAt(scene.correction, columnTau, muSun);
      let a = 1;
      let b = 1;
      let c = 1;
      let source = 0;
      for (let i = 0; i < CLOUD_OCTAVES.count; i += 1) {
        source +=
          (i === 0 ? 1 : multiple) *
          b *
          albedo *
          twoLobePhase(cosines[j] ?? 0, lobes, c) *
          Math.exp(-a * tauSun);
        a *= CLOUD_OCTAVES.extinction;
        b *= CLOUD_OCTAVES.scattering;
        c *= CLOUD_OCTAVES.phase;
      }
      light[j] = (light[j] ?? 0) + scattered * source;
    }
    const reflect = diffuseReflectance(columnTau, g);
    const slabR = reflect + flat * (entrySlab.reflectance - reflect);
    const slabT = 1 - reflect + flat * (entrySlab.diffuseTransmittance - (1 - reflect));
    const opacity = 1 - Math.exp(-viewStep);
    sky += view * opacity * (slabR + above * (slabT - slabR));
    weighed += view * opacity * t;
    weight += view * opacity;
    view *= Math.exp(-viewStep);
    if (view < LEAST_TRANSMITTANCE) {
      break;
    }
  }
  return {
    sun: light,
    sky,
    transmittance: view,
    meanDistanceM: weight > 0 ? weighed / weight : t0,
    share,
  };
}
