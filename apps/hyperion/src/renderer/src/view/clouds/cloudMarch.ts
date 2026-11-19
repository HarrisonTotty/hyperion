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
  CELL_FEATURE_TILES,
  CELL_REGION_M,
  cellHeightNoiseAt,
  cellNoiseAt,
  cellOf,
  type CloudColumn,
  type CloudShapeFrame,
  type ColumnField,
  columnFromMap,
  cumulusShare,
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

/**
 * The cosine of the sky's light on the slab: 0.6, the diffusivity factor's 1 ÷ 1.66 rounded
 * (Elsasser 1942, Harvard Meteorological Studies 6), exact for a layer that absorbs and an
 * approximation for one that scatters: δ-Eddington's T at 0.6 is within 1% of its mean over an
 * isotropic sky at τ ≤ 2 and 4–5% under it at τ 8–30. The layer's own (`cloudShadow.ts`).
 */
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

/**
 * The share a ray is drawn in from its entry's column (the module's remarks): the layer's share f
 * by the larger of the field's unresolved weight and the share of its cumulus cells the entry's
 * footprint leaves unresolved, `unresolvedCells` (`cellsUnresolved`).
 */
export function rayShare(entry: CloudColumn, flat: number, unresolvedCells = 0): number {
  const w = Math.max(
    smoothstep(SHARE_SD_RANGE[0], SHARE_SD_RANGE[1], entry.unresolvedSd),
    unresolvedCells,
  );
  const weight = w + flat * (1 - w);
  return 1 - weight * (1 - entry.fraction);
}

/**
 * The share of a column's cumulus cells a footprint leaves unresolved: its cumuliform share times
 * one less the cells' noise share there (`cloudShape.ts`' `cumulusShare` and `noiseShare`).
 */
export function cellsUnresolved(
  scene: Pick<CloudMarchScene, "inputs" | "frame">,
  column: Pick<CloudColumn, "cover">,
  footprintM: number,
): number {
  const cellM = scene.frame.baseTileM * CELL_FEATURE_TILES;
  return cumulusShare(column.cover, scene.inputs.kind) * (1 - noiseShare(cellM, footprintM));
}

/** A column with no clouds: beyond the map. */
const CLEAR: CloudColumn = {
  fraction: 0,
  meanTau: 0,
  topShare: 1,
  unresolvedSd: 0,
  excess: 0,
  cover: 0,
};

/**
 * The column above or below a place, body-fixed, m, at a footprint: read from the map at the
 * place's direction on the layer's shell, its octaves finer than the map's added where `refine`
 * holds (`cloudShape.ts`' `columnFromMap`); clear beyond the map.
 */
export function columnAt(
  scene: Pick<CloudMarchScene, "map" | "field" | "inputs" | "axisRatio" | "shellRadiusM">,
  x: Vec3,
  footprintM: number,
  refine: boolean,
): CloudColumn {
  const k = scene.axisRatio;
  const onShell = scene.shellRadiusM / Math.hypot(x.x, x.y, x.z * k);
  const p = { x: x.x * onShell, y: x.y * onShell, z: x.z * onShell };
  const read = scene.map.readAt(p, footprintM);
  return read === null
    ? CLEAR
    : columnFromMap(scene.field, scene.inputs.kind, p, footprintM, read, refine);
}

/**
 * The pixel's footprint at a ray's entry into the deck at `t0`, m: its growth with distance over
 * the view's cosine against the deck.
 */
export function entryFootprintM(
  scene: Pick<CloudMarchScene, "axisRatio">,
  originM: Vec3,
  direction: Vec3,
  t0: number,
  footprintPerM: number,
): number {
  const k = scene.axisRatio;
  const x0 = along(originM, direction, t0);
  const n = { x: x0.x, y: x0.y, z: x0.z * k * k };
  const muView = Math.max(
    Math.abs(dot3(direction, n)) / Math.hypot(n.x, n.y, n.z),
    CLOUD_VOLUME_LEAST_MU,
  );
  return Math.max((footprintPerM * t0) / muView, 1);
}

/** The field's footprint a column is read at: a liquid deck's at least half a cell's. */
function fieldFootprintM(
  scene: Pick<CloudMarchScene, "inputs" | "frame">,
  footprintM: number,
): number {
  return scene.inputs.kind.cumuliform
    ? Math.max(footprintM, (scene.frame.baseTileM * CELL_FEATURE_TILES) / 2)
    : footprintM;
}

/** The column at a ray's entry into the deck at `t0`, over the pixel's footprint there. */
export function marchEntry(
  scene: CloudMarchScene,
  originM: Vec3,
  direction: Vec3,
  t0: number,
  footprintPerM: number,
): CloudColumn {
  const footprintM = entryFootprintM(scene, originM, direction, t0, footprintPerM);
  return columnAt(scene, along(originM, direction, t0), fieldFootprintM(scene, footprintM), true);
}

/**
 * The share of a cloud's background seen through it at its view transmittance `view` and
 * asymmetry g: the δ-scaled e^(−(1 − g²) τ), the forward peak's light passing as δ-Eddington's
 * similarity has it (Joseph, Wiscombe and Weinman 1976), taken to 0 where the view's reaches the
 * march's stop, {@link LEAST_TRANSMITTANCE}, so that a thin edge shows its background, where e^(−τ)
 * drew a rim darker than it (the eye check of 2026-11-19), and a thick core none.
 */
export function backgroundTransmittance(view: number, asymmetry: number): number {
  const scaled = 1 - asymmetry * asymmetry;
  const least = LEAST_TRANSMITTANCE ** scaled;
  return Math.min(Math.max((Math.max(view, 1e-30) ** scaled - least) / (1 - least), 0), 1);
}

/** The conservative two-stream's diffuse reflectance of a slab (the module's remarks). */
export function diffuseReflectance(tau: number, asymmetry: number): number {
  const x = 0.75 * (1 - asymmetry) * Math.max(tau, 0);
  return x / (1 + x);
}

/** What a scene's march reads: its map, noises, density and light. */
export interface CloudMarchScene {
  readonly map: CloudMapTwin;
  /** The body's field, whose finer octaves each view sample adds to the map's. */
  readonly field: ColumnField;
  /** a_s, the layer's shell's equatorial radius, m, on which the map's columns stand. */
  readonly shellRadiusM: number;
  readonly noises: CloudShapeNoises;
  readonly frame: CloudShapeFrame;
  readonly inputs: DensityInputs;
  /** The deck's top above the datum, m. */
  readonly topM: number;
  /** a, the datum's equatorial radius, m. */
  readonly datumRadiusM: number;
  /** k, the deck's spheroids' a ÷ c. */
  readonly axisRatio: number;
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
  /** Its stretch's start, m along it. */
  readonly t0M: number;
  /** Its stretch's end, m along it, cut where the scene stands nearer. */
  readonly t1M: number;
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
  /**
   * The share of the background seen through the stretch, before the share
   * ({@link backgroundTransmittance} of the view's transmittance).
   */
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
 * A sample's density, its column's in-cloud τ over the share, the share of it above, and the
 * region's column its cells read, `null` where it read none.
 */
interface SampleDensity {
  readonly sigma: number;
  readonly columnTau: number;
  readonly above: number;
  readonly region: CloudColumn | null;
}

/**
 * The density at a place and footprint (the module's remarks): its column, read at the field's
 * footprint and refined to it where `refine` holds, laid through the deck as a sheet, mixed with
 * its cumulus cells by their share (`cloudShape.ts`' `cellOf`, `cumulusShare`), the detail read
 * where its share is above 0 and `detail` holds; the cells' region `regionIn` where given (a march
 * towards the source's, whose path of at most three deck depths the 40 km region does not see),
 * else read at the place from the map alone.
 */
function densityAt(
  scene: CloudMarchScene,
  x: Vec3,
  footprintM: number,
  share: number,
  detail: boolean,
  refine: boolean,
  regionIn: CloudColumn | null,
): SampleDensity {
  const { inputs, frame, noises, map } = scene;
  const { height } = heightAndNormal(x, scene.axisRatio, scene.datumRadiusM);
  const column = columnAt(scene, x, fieldFootprintM(scene, footprintM), refine);
  if (!(column.meanTau > 0)) {
    return { sigma: 0, columnTau: 0, above: 0, region: regionIn };
  }
  const wb = noiseShare(frame.baseTileM * BASE_FEATURE_TILES, footprintM) * frame.baseRateShare;
  const wd = detail
    ? noiseShare(frame.detailTileM * DETAIL_FEATURE_TILES, footprintM) * frame.detailRateShare
    : 0;
  const cellM = frame.baseTileM * CELL_FEATURE_TILES;
  const wCell =
    cumulusShare(column.cover, inputs.kind) *
    noiseShare(cellM, footprintM) *
    frame.baseRateShare *
    (1 - inputs.flat);
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
  const noise = { base: nb, detail: nd, baseShare: wb, detailShare: wd };
  const sheetPlace = columnPlace(height, column.topShare, inputs);
  const sheetTau = column.meanTau / share;
  const sheet = densityPerM(sheetTau, sheetPlace, noise, inputs);
  const sheetAbove = shareAbove(sheetPlace, inputs);
  if (!(wCell > 0)) {
    return { sigma: sheet, columnTau: sheetTau, above: sheetAbove, region: regionIn };
  }
  // The cell's pattern is the cells' noise on the layer's shell above or below the sample, its
  // cover and τ the region's.
  const k = scene.axisRatio;
  const onShell = scene.shellRadiusM / Math.hypot(x.x, x.y, x.z * k);
  const region = regionIn ?? columnAt(scene, x, Math.max(footprintM, CELL_REGION_M), false);
  const shell = { x: x.x * onShell, y: x.y * onShell, z: x.z * onShell };
  const cell = cellOf(
    region,
    column,
    cellNoiseAt(noises.base, frame, shell),
    cellHeightNoiseAt(noises.base, frame, shell),
    scene.field.params.meanTau,
  );
  const cellPlace = columnPlace(height, cell.topShare, inputs);
  const cellTau = cell.inCloudTau / share;
  const cellSigma = densityPerM(cellTau, cellPlace, noise, inputs);
  return {
    sigma: sheet + wCell * (cellSigma - sheet),
    columnTau: sheetTau + wCell * (cellTau - sheetTau),
    above: sheetAbove + wCell * (shareAbove(cellPlace, inputs) - sheetAbove),
    region,
  };
}

/**
 * σ(550) per metre at a place and footprint as the shadow map reads it (`cloudShadowMap.ts`): the
 * march's density at a share of 1, the detail left out, the map's columns unrefined.
 */
export function densityPerMAt(scene: CloudMarchScene, x: Vec3, footprintM: number): number {
  return densityAt(scene, x, footprintM, 1, false, false, null).sigma;
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
  region: CloudColumn | null,
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
    tau +=
      densityAt(scene, along(x, sun, at), footprintM, share, false, false, region).sigma * step;
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
  const { originM: o, direction: d, t0M: t0, t1M: t1, footprintPerM } = ray;
  const flat = scene.inputs.flat;
  const share = rayShare(
    entry,
    flat,
    cellsUnresolved(scene, entry, entryFootprintM(scene, o, d, t0, footprintPerM)),
  );
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
    const { sigma, columnTau, above, region } = densityAt(
      scene,
      x,
      footprint,
      share,
      levers.detail,
      true,
      null,
    );
    if (!(sigma > 0)) {
      continue;
    }
    const { height, normal } = heightAndNormal(x, scene.axisRatio, scene.datumRadiusM);
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
          ? sunDepth(scene, x, height, rawMu, sun, footprint, share, levers.lightSteps, region)
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
    transmittance: backgroundTransmittance(view, g),
    meanDistanceM: weight > 0 ? weighed / weight : t0,
    share,
  };
}
