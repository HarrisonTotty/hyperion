/**
 * The sea near its surface (plan R11, R11.T8.c; Design notes 11, 12 and 19): below 20 km, Bruneton,
 * Neyret and Holzschuch 2010's switch, a grid projected from the screen onto the sea under the
 * camera, displaced by the sea's Gerstner set, its normals the set's and its unresolved slopes the
 * BRDF's; the CPU twin, in `f64`, of `oceanNear.wgsl`, and the packing of what that shader reads.
 *
 * @remarks
 * - **The grid** is a lattice of screen points ({@link oceanGridMesh}, {@link OCEAN_GRID_CELL_PX}
 *   apart), each one's ray through the frame's projection met with the sea-level spheroid itself
 *   in the camera's frame ({@link oceanNearHit}): in the space stretched along the pole by a ÷ c,
 *   where the spheroid is a sphere of radius a, the near root t = C₀ ÷ (−B + √(B² − A C₀)), A = |d′|²,
 *   B = |o′| (Û′ · d′) and C₀ = |o′|² − a², the camera's stretched distance |o′| and C₀ formed in
 *   `f64`, so that no large quantity cancels in `f32`. A ray above the horizon is taken down onto it,
 *   the tangent from the camera at its own azimuth, so that the grid's rows above the horizon
 *   collapse there and draw nothing. Its border, beyond the view, is a skirt left undisplaced
 *   ({@link OCEAN_GRID_MARGIN}).
 * - **The waves are the sea's.** The set ({@link oceanNearSet}) is `gerstner.ts`' at its wind texel's
 *   wind, a ladder fixed by the sea state alone, from twice the peak's wavelength down to
 *   {@link OCEAN_NEAR_SHORTEST_OVER_PEAK} of it, 32 components on the high setting and 8 on the low
 *   (`oceanComponents`), so that no camera height moves a wavenumber or a frequency. Its texel is
 *   the one where the view's forward ray meets open water, else the camera's nadir's, else none (a
 *   glassy sea) ({@link nearSeaAnchor}); its components lie along the wind's frame tangent at that
 *   texel's centre, the anchor, and a point's Lagrangian coordinates are its offsets along that
 *   frame, camera-relative in the shader, the camera's own offset from the anchor added to each
 *   phase in `f64` with φ − ω t ({@link oceanWavesAt}), so that a crest moves with the water and
 *   not with the camera. The set changes whole where the anchor's texel does (a lean, 2026-11-18).
 * - **Filtered by the grid and the pixel** (Bruneton et al. 2010, eqs. 2 and 3): each component's
 *   displacement is faded by the footprint of a grid cell there and its normal by a pixel's, by
 *   their smoothstep from 1.0 to 2.5 footprints a wavelength (their §7.1's N_min and N_max;
 *   {@link componentFade}), so that the grid is displaced only by waves it can sample.
 * - **One slope budget.** The BRDF keeps, per axis of the local wind, the sea's total Cox–Munk
 *   variance at the point's own texel less what the drawn components resolve there, 1 − √(1 −
 *   (k f A)²) each split between the axes as cos² and sin² of its angle (Bruneton's eq. 4 and §7.1),
 *   never below the floor of 10⁻⁴, plus the curvature across the footprint as the far sea filters
 *   it; so a component faded out moves its slopes into the BRDF, and the glint is continuous across
 *   the switch ({@link oceanNearPatchRadiance}). A normal facing away from the camera is reflected
 *   towards it, n − 2 (n · v) v (Bruneton's §3).
 * - **The water's thickness** (the brainstorm's depth rules): the bed's distance along the pixel's
 *   ray, from the scene's depth (the opaque passes', copied before the pass), less the sea's, within
 *   {@link OCEAN_DEPTH_THICKNESS_MAX_M}, its vertical share by the ray's cosine to the sea's up; the
 *   bathymetry beyond it, or where no bed was drawn ({@link oceanThickness}). The lower layer is
 *   culled once its separation from the sea falls below {@link OCEAN_LAYER_CULL_SHARE} of the
 *   camera's distance ({@link oceanLayerCull}): there the sea is drawn over the bed with no water,
 *   and at its own depth, the bed's, which no precision orders; a bed nearer than that hides the sea.
 * - **Refraction**, on the high setting only (`oceanRefraction`): the bed seen along the view ray
 *   refracted at the wave's normal, its light from the scene's colour copied before the pass at the
 *   pixel where that ray meets the bed, its light in air carried down to it and back through the
 *   column, (1 − F_s)(1 − F_v) ÷ n² × e^(−X), X = K_d H (1 ÷ μ_s,t + 1 ÷ μ_v,t), μ the refracted
 *   cosines; and the column's own light over the same path, R∞ (1 − e^(−X)) (Maritorena, Morel and
 *   Gentili 1994's construction, one X in both terms). On the low setting, and beyond the depth's
 *   range, the bed is the far sea's: its albedo seen through the column, X = 2 K_d H ÷ μ_s,t.
 */
import type { UniverseTime } from "@hyperion/protocol";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { fractionOfPeriod } from "../../lib/orbit";
import { geodeticOf } from "../atmosphere/hillaire";
import type { Quaternion } from "../camera/pose";
import { rotate } from "../camera/quaternion";
import { classMapTexelOf } from "../bodies/discSurface";
import { faceUvToXyz, stToUv } from "../terrain/cube";
import type { Rgb } from "../photometry/toneCurve";
import type { QualitySetting } from "../quality/qualitySetting";
import { type GerstnerSet, gerstnerSet } from "./gerstner";
import { fresnelReflectance, refractedCosine, UPWELLING_Q_SR } from "./liquids";
import type { OceanSea } from "./oceanBody";
import {
  eastNorthOf,
  OCEAN_FAR_SWITCH_M,
  type OceanFarOptics,
  oceanGlintFactor,
  oceanSlopeVariance,
  oceanWhitecapFraction,
  type OceanPointLight,
  type SlopeVariance,
  WHITECAP_ALBEDO,
} from "./oceanFar";
import { type OceanTexel, oceanTexelAt, type OceanTexels } from "./oceanMap";
import { CLIMATE_WIND_HEIGHT_M, seaState, SLOPE_VARIANCE_FLOOR } from "./seaState";
import { setRmsHeightM } from "./shoreline";

/** The camera's height above the sea below which the near sea is drawn, m: the far sea's switch. */
export const OCEAN_NEAR_BELOW_M = OCEAN_FAR_SWITCH_M;

/**
 * The view distance within which the water's thickness is the scene's depth less the sea's, m: 10⁵
 * (the brainstorm's depth rules: beyond it the depth's error, 0.12 m at 1,000 km, passes the
 * shallows' thickness, and the bathymetry is taken).
 */
export const OCEAN_DEPTH_THICKNESS_MAX_M = 1e5;

/**
 * The share of the camera's distance below which a layer's separation from the sea is not ordered
 * by depth, and the lower layer is culled: 10⁻⁶ (the brainstorm's depth rules: reversed-Z orders two
 * surfaces reliably while their separation along the view passes about 10⁻⁶ of the distance).
 */
export const OCEAN_LAYER_CULL_SHARE = 1e-6;

/**
 * The projected grid's cell on each setting, px: 4 on high and 8 on low (a lean, 2026-11-18: about
 * 1.6 × 10⁵ vertices at 1080p on high, 4 × 10⁴ on low, the margin included).
 */
export const OCEAN_GRID_CELL_PX: Readonly<Record<QualitySetting, number>> = { high: 4, low: 8 };

/**
 * The grid's reach beyond the view on each side, in NDC: 0.1. Its border is a skirt the waves leave
 * where it lies ({@link oceanGridMesh}), since near a low camera they pull the grid's edge in by
 * more than any margin: a third of the view's height at the bottom of a frame from 2 m under a
 * 7 m/s sea, which drew no sea there (R11.T8.d's coast from 2 m, 2026-11-18).
 */
export const OCEAN_GRID_MARGIN = 0.1;

/** The grid's columns and rows of cells for a view of `widthPx` × `heightPx` at `cellPx`. */
export function oceanGridSize(
  widthPx: number,
  heightPx: number,
  cellPx: number,
): { readonly columns: number; readonly rows: number } {
  const reach = 1 + OCEAN_GRID_MARGIN;
  return {
    columns: Math.max(2, Math.ceil((reach * widthPx) / cellPx)),
    rows: Math.max(2, Math.ceil((reach * heightPx) / cellPx)),
  };
}

/**
 * The grid's mesh: (columns + 1) × (rows + 1) points in NDC over [−1 − m, 1 + m]², z 1 on the
 * border, the skirt `oceanNear.wgsl` leaves undisplaced, and 0 within, and two triangles a cell,
 * counter-clockwise, as 32-bit indices.
 */
export function oceanGridMesh(
  columns: number,
  rows: number,
): { readonly positions: Float32Array; readonly indices: Uint32Array } {
  const reach = 1 + OCEAN_GRID_MARGIN;
  const positions = new Float32Array(3 * (columns + 1) * (rows + 1));
  for (let j = 0; j <= rows; j += 1) {
    for (let i = 0; i <= columns; i += 1) {
      const at = 3 * (j * (columns + 1) + i);
      positions[at] = reach * ((2 * i) / columns - 1);
      positions[at + 1] = reach * ((2 * j) / rows - 1);
      positions[at + 2] = i === 0 || i === columns || j === 0 || j === rows ? 1 : 0;
    }
  }
  const indices = new Uint32Array(6 * columns * rows);
  let k = 0;
  for (let j = 0; j < rows; j += 1) {
    for (let i = 0; i < columns; i += 1) {
      const a = j * (columns + 1) + i;
      const b = a + 1;
      const c = a + columns + 1;
      const d = c + 1;
      indices.set([a, b, d, a, d, c], k);
      k += 6;
    }
  }
  return { positions, indices };
}

/**
 * The fade's band, in footprints a wavelength: Bruneton et al. 2010's §7.1 N_min = 1.0 and N_max =
 * 2.5.
 */
export const OCEAN_FADE_BAND: readonly [number, number] = [1, 2.5];

/**
 * A component's share drawn at a point whose footprint (a grid cell's for the displacement, a
 * pixel's for the normal) is `footprintM`: Bruneton et al. 2010's smoothstep w(N_min, N_max, λ ÷ ℓ),
 * 3x² − 2x³ of x = (λ ÷ ℓ − N_min) ÷ (N_max − N_min) held to [0, 1] (the module's remarks).
 */
export function componentFade(wavelengthM: number, footprintM: number): number {
  if (!(footprintM > 0)) {
    return 1;
  }
  const [low, high] = OCEAN_FADE_BAND;
  const x = Math.min(Math.max((wavelengthM / footprintM - low) / (high - low), 0), 1);
  return x * x * (3 - 2 * x);
}

/** What the lower-layer cull makes of a sea's pixel over its bed ({@link oceanLayerCull}). */
export type OceanLayer =
  /** The bed lies below the sea by more than the cull: the sea over its water. */
  | "sea"
  /** The two within the cull: the bed culled, the sea drawn with no water, at the bed's depth. */
  | "culled"
  /** The bed nearer by more than the cull: it hides the sea, a shore or a rock above it. */
  | "bed";

/**
 * The lower-layer cull of a sea's pixel (the module's remarks): by the bed's distance along the
 * pixel's ray less the sea's, against {@link OCEAN_LAYER_CULL_SHARE} of the sea's distance.
 */
export function oceanLayerCull(seaDistanceM: number, bedDistanceM: number): OceanLayer {
  const separation = bedDistanceM - seaDistanceM;
  const cull = OCEAN_LAYER_CULL_SHARE * seaDistanceM;
  if (separation < -cull) {
    return "bed";
  }
  return separation <= cull ? "culled" : "sea";
}

/** The water's thickness at a sea's pixel, and where it came from. */
export interface OceanThickness {
  /** The water's vertical depth, m, at least 0. */
  readonly depthM: number;
  /** The scene's depth or the bathymetry ({@link oceanThickness}). */
  readonly from: "depth" | "bathymetry";
  readonly layer: OceanLayer;
}

/**
 * The water's thickness at a sea's pixel (the module's remarks): within
 * {@link OCEAN_DEPTH_THICKNESS_MAX_M}, where the scene drew a bed, the bed's distance along the
 * ray less the sea's times the ray's cosine to the sea's up, 0 where the cull takes the bed; else
 * the texel's bathymetric depth.
 *
 * @param bedDistanceM - The bed's distance along the pixel's ray, m; `null` where the scene drew
 *   none (its depth 0, at infinity).
 * @param viewCosine - |d · U|, the ray's cosine to the sea's up there.
 */
export function oceanThickness(
  seaDistanceM: number,
  bedDistanceM: number | null,
  viewCosine: number,
  bathymetryM: number,
): OceanThickness {
  if (bedDistanceM === null || seaDistanceM > OCEAN_DEPTH_THICKNESS_MAX_M) {
    return { depthM: Math.max(bathymetryM, 0), from: "bathymetry", layer: "sea" };
  }
  const layer = oceanLayerCull(seaDistanceM, bedDistanceM);
  const depthM = layer === "sea" ? (bedDistanceM - seaDistanceM) * viewCosine : 0;
  return { depthM, from: "depth", layer };
}

/** The near sea's frame: the camera over the sea-level spheroid, stretched ({@link oceanNearFrame}). */
export interface OceanNearFrame {
  /** The camera's geodetic height above the sea-level spheroid, m. */
  readonly heightM: number;
  /** The sea-level spheroid's semi-axes, m, and a ÷ c, the stretch along the pole. */
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
  readonly stretch: number;
  /** |o′|, the camera's distance from the centre in the stretched space, m. */
  readonly stretchedDistanceM: number;
  /** Û′ = o′ ÷ |o′|, the camera's direction in the stretched space. */
  readonly stretchedUp: Vec3;
  /** C₀ = |o′|² − a² = (|o′| − a)(|o′| + a), m², formed in `f64`. */
  readonly excessM2: number;
}

/** The near sea's frame under a camera at `cameraM` (from the body's centre, body-fixed, m). */
export function oceanNearFrame(
  optics: Pick<OceanFarOptics, "equatorialRadiusM" | "polarRadiusM">,
  cameraM: Vec3,
): OceanNearFrame {
  const a = optics.equatorialRadiusM;
  const c = optics.polarRadiusM;
  const k = a / c;
  const stretched = vec3(cameraM.x, cameraM.y, cameraM.z * k);
  const distance = norm(stretched);
  return {
    heightM: geodeticOf(cameraM, { equatorialRadiusM: a, polarRadiusM: c }).heightM,
    equatorialRadiusM: a,
    polarRadiusM: c,
    stretch: k,
    stretchedDistanceM: distance,
    stretchedUp: scale(stretched, 1 / distance),
    excessM2: (distance - a) * (distance + a),
  };
}

/** Where a near sea's ray meets the sea-level spheroid ({@link oceanNearHit}). */
export interface OceanNearHit {
  /** The distance along the ray, m. */
  readonly tM: number;
  /** The point from the camera, body-fixed, m. */
  readonly pointM: Vec3;
  /** The unit ray it lies along: the ray's own, or its horizon's where it was taken down. */
  readonly direction: Vec3;
  /** Whether the ray missed and was taken down onto the horizon. */
  readonly clamped: boolean;
}

/**
 * Where the unit ray `direction` from the camera meets the sea-level spheroid (the module's
 * remarks), a ray above the horizon taken down onto it at its own azimuth: the twin of
 * `oceanNear.wgsl`'s `ocean_near_hit`.
 */
export function oceanNearHit(frame: OceanNearFrame, direction: Vec3): OceanNearHit {
  const k = frame.stretch;
  const up = frame.stretchedUp;
  const d = vec3(direction.x, direction.y, direction.z * k);
  const a2 = dot(d, d);
  const b = frame.stretchedDistanceM * dot(up, d);
  const c0 = frame.excessM2;
  const disc = b * b - a2 * c0;
  if (b < 0 && disc >= 0) {
    const tM = c0 / (-b + Math.sqrt(disc));
    return { tM, pointM: scale(direction, tM), direction, clamped: false };
  }
  // The tangent from the camera at this azimuth, in the stretched space: its cosine to −Û′ is
  // √C₀ ÷ |o′| and its length √C₀.
  const w = normalise(d);
  const horizontal = sub(w, scale(up, dot(up, w)));
  const along =
    norm(horizontal) > 1e-12 ? normalise(horizontal) : normalise(cross(up, vec3(0, 0, 1)));
  const reach = Math.sqrt(Math.max(c0, 0));
  const cos = reach / frame.stretchedDistanceM;
  const sin = frame.equatorialRadiusM / frame.stretchedDistanceM;
  const tangent = add(scale(up, -cos), scale(along, sin));
  const offset = vec3(reach * tangent.x, reach * tangent.y, (reach * tangent.z) / k);
  const tM = norm(offset);
  return { tM, pointM: offset, direction: scale(offset, 1 / tM), clamped: true };
}

/** The spheroid's outward unit normal at the point `pointM` from its centre. */
export function spheroidNormalAt(
  frame: Pick<OceanNearFrame, "equatorialRadiusM" | "polarRadiusM">,
  pointM: Vec3,
): Vec3 {
  const a2 = frame.equatorialRadiusM * frame.equatorialRadiusM;
  const c2 = frame.polarRadiusM * frame.polarRadiusM;
  return normalise(vec3(pointM.x / a2, pointM.y / a2, pointM.z / c2));
}

/**
 * Where the near sea's waves are laid (the module's remarks): the centre of their wind's texel on
 * the sea-level spheroid, the wind's frame tangent there, and the wind.
 */
export interface OceanWaveAnchor {
  /** The texel's centre on the sea-level spheroid, from the body's centre, body-fixed, m. */
  readonly pointM: Vec3;
  /** Along the wind and across it, tangent at the anchor, body-fixed. */
  readonly upwind: Vec3;
  readonly across: Vec3;
  /** The texel's 10 m wind, east and north at the anchor, m/s. */
  readonly windEastNorthMps: readonly [number, number];
}

/**
 * The near sea's anchor (the module's remarks): the open-water texel where the view's forward ray
 * meets the sea, else the camera's nadir's, its centre and its wind; else calm, anchored under the
 * camera.
 *
 * @param orientation - Rotates the camera's axes (−z forward) into the body-fixed ones.
 */
export function nearSeaAnchor(
  optics: Pick<OceanFarOptics, "equatorialRadiusM" | "polarRadiusM">,
  texels: OceanTexels,
  cameraM: Vec3,
  orientation: Quaternion,
): OceanWaveAnchor {
  const frame = oceanNearFrame(optics, cameraM);
  const ahead = oceanNearHit(frame, rotate(orientation, vec3(0, 0, -1)));
  const nadir = spheroidNormalAt(frame, cameraM);
  const candidates = [...(ahead.clamped ? [] : [add(cameraM, ahead.pointM)]), nadir];
  for (const direction of candidates) {
    const texel = oceanTexelAt(texels, [direction.x, direction.y, direction.z]);
    if (texel !== null) {
      return anchorAt(frame, texels.faceTexels, direction, [texel.windEastMps, texel.windNorthMps]);
    }
  }
  return anchorAt(frame, texels.faceTexels, nadir, [0, 0]);
}

/** The anchor of the texel holding `direction`: its centre on the spheroid and the wind's frame. */
function anchorAt(
  frame: OceanNearFrame,
  faceTexels: number,
  direction: Vec3,
  windEastNorthMps: readonly [number, number],
): OceanWaveAnchor {
  const { face, i, j } = classMapTexelOf([direction.x, direction.y, direction.z], faceTexels);
  const [x, y, z] = faceUvToXyz(
    face,
    stToUv((i + 0.5) / faceTexels),
    stToUv((j + 0.5) / faceTexels),
  );
  const u = normalise(vec3(x, y, z));
  const a2 = frame.equatorialRadiusM * frame.equatorialRadiusM;
  const c2 = frame.polarRadiusM * frame.polarRadiusM;
  const radius = 1 / Math.sqrt((u.x * u.x + u.y * u.y) / a2 + (u.z * u.z) / c2);
  const pointM = scale(u, radius);
  const up = spheroidNormalAt(frame, pointM);
  const { east, north } = eastNorthOf(up);
  const wind = add(scale(east, windEastNorthMps[0]), scale(north, windEastNorthMps[1]));
  const upwind = norm(wind) > 0 ? normalise(wind) : east;
  return { pointM, upwind, across: cross(up, upwind), windEastNorthMps };
}

/**
 * The shortest wave of the near sea's ladder, in units of the peak's wavelength: 1 ÷ 64 (a lean,
 * 2026-11-18: the ladder spans 128 times from twice the peak, so that 32 components step by 2^(7 ÷
 * 32), about 16% in wavelength, and 8 by 2^(7 ÷ 8); every wave shorter is the BRDF's).
 */
export const OCEAN_NEAR_SHORTEST_OVER_PEAK = 1 / 64;

/**
 * The near sea's Gerstner set: `gerstner.ts`' for `sea` at the 10 m wind `wind10Mps`, fully
 * developed, its components `count` (the setting's `oceanComponents`), a ladder fixed by the sea
 * state alone down to {@link OCEAN_NEAR_SHORTEST_OVER_PEAK} of the peak's wavelength (the module's
 * remarks); none for a glassy sea.
 */
export function oceanNearSet(
  sea: Pick<OceanSea, "liquid" | "air">,
  gravityMps2: number,
  wind10Mps: number,
  count: number,
): GerstnerSet {
  const peakWavelengthM = oceanNearPeakWavelengthM(sea, gravityMps2, wind10Mps);
  return gerstnerSet(
    nearSeaState(sea, gravityMps2, wind10Mps),
    count,
    OCEAN_NEAR_SHORTEST_OVER_PEAK * peakWavelengthM,
  );
}

/** The near sea's state at the 10 m wind `wind10Mps`, fully developed. */
function nearSeaState(
  sea: Pick<OceanSea, "liquid" | "air">,
  gravityMps2: number,
  wind10Mps: number,
): ReturnType<typeof seaState> {
  return seaState({
    windMps: wind10Mps,
    windHeightM: CLIMATE_WIND_HEIGHT_M,
    gravityMps2,
    liquid: sea.liquid,
    air: sea.air,
    fetchM: null,
  });
}

/**
 * The near sea's peak wavelength in deep water, m, 2π ÷ k_p of its sea state, which the shore's
 * surf similarity takes for L₀ (`shoreline.ts`); 0 for a glassy sea.
 */
export function oceanNearPeakWavelengthM(
  sea: Pick<OceanSea, "liquid" | "air">,
  gravityMps2: number,
  wind10Mps: number,
): number {
  const state = nearSeaState(sea, gravityMps2, wind10Mps);
  return state.peakWavenumberRadPerM > 0 ? (2 * Math.PI) / state.peakWavenumberRadPerM : 0;
}

/** The waves' rows: a header, then two `vec4f` a component. */
export const OCEAN_WAVE_HEADER_ROWS = 1;

/** The components the waves' rows hold at most: the high setting's 32. */
export const OCEAN_MAX_COMPONENTS = 32;

/** The waves' rows a sea's buffer holds: the header and two a component. */
export const OCEAN_WAVE_ROWS = OCEAN_WAVE_HEADER_ROWS + 2 * OCEAN_MAX_COMPONENTS;

/** One component as the shader takes it, its phase at the frame's time and the camera's place. */
export interface OceanWave {
  /** k along the anchor's wind frame's axes, rad/m. */
  readonly kxRadPerM: number;
  readonly kyRadPerM: number;
  readonly amplitudeM: number;
  /** φ − ω t + k · ξ, ξ the camera's offset from the anchor, reduced into [−π, π), rad. */
  readonly phaseRad: number;
  readonly steepness: number;
  readonly wavelengthM: number;
  /** cos² of its angle from the wind: its slopes' share along the wind. */
  readonly upwindShare: number;
}

/**
 * A set's components at the scene's time `time` for a camera at `cameraM` (from the body's centre,
 * body-fixed, m): each phase φ − ω t + k · ξ formed in `f64`, ω t exactly through the period's
 * fraction and ξ the camera's offset from `anchor` along its wind frame, so that the shader's
 * camera-relative Lagrangian coordinates name the same waves wherever the camera stands.
 */
export function oceanWavesAt(
  set: GerstnerSet,
  time: UniverseTime,
  anchor: Pick<OceanWaveAnchor, "pointM" | "upwind" | "across">,
  cameraM: Vec3,
): ReadonlyArray<OceanWave> {
  const offset = sub(cameraM, anchor.pointM);
  const xi = dot(offset, anchor.upwind);
  const eta = dot(offset, anchor.across);
  return set.components.slice(0, OCEAN_MAX_COMPONENTS).map((c) => {
    const k = c.wavenumberRadPerM;
    const cos = Math.cos(c.directionRad);
    const sin = Math.sin(c.directionRad);
    // ω t mod 2π through the period's exact fraction (`fractionOfPeriod`), whatever the time.
    const turned =
      c.angularFrequencyRadS > 0
        ? 2 * Math.PI * fractionOfPeriod(time, (2 * Math.PI) / c.angularFrequencyRadS)
        : 0;
    const phase = c.phaseRad - turned + k * cos * xi + k * sin * eta;
    return {
      kxRadPerM: k * cos,
      kyRadPerM: k * sin,
      amplitudeM: c.amplitudeM,
      phaseRad: phase - 2 * Math.PI * Math.floor((phase + Math.PI) / (2 * Math.PI)),
      steepness: c.steepness,
      wavelengthM: (2 * Math.PI) / k,
      upwindShare: cos * cos,
    };
  });
}

/**
 * The waves' rows as `oceanNear.wgsl` reads them, {@link OCEAN_WAVE_ROWS} `vec4f`s: a header of
 * (components, the set's H_rms, the peak's deep-water wavelength L₀, 0), the shore's
 * (`shoreline.ts`), then each component's (k_x, k_y, A, φ − ω t + k · ξ) and (Q, λ, cos², 0).
 */
export function packOceanWaves(
  waves: ReadonlyArray<OceanWave>,
  set: GerstnerSet,
  peakWavelengthM: number,
): Float32Array<ArrayBuffer> {
  const out = new Float32Array(4 * OCEAN_WAVE_ROWS);
  const drawn = waves.slice(0, OCEAN_MAX_COMPONENTS);
  out.set([drawn.length, setRmsHeightM(set), peakWavelengthM, 0]);
  drawn.forEach((w, i) => {
    const at = 4 * (OCEAN_WAVE_HEADER_ROWS + 2 * i);
    out.set([w.kxRadPerM, w.kyRadPerM, w.amplitudeM, w.phaseRad], at);
    out.set([w.steepness, w.wavelengthM, w.upwindShare, 0], at + 4);
  });
  return out;
}

/** The sea's surface at a Lagrangian point, its components faded by a footprint. */
export interface OceanNearSurface {
  /** The displacement from the mean surface: along the wind, across it and up, m. */
  readonly displacementM: readonly [number, number, number];
  /** The surface's unit normal in the wind's frame (along, across, up). */
  readonly normal: readonly [number, number, number];
  /** The slope variance the drawn components resolve, per axis of the wind. */
  readonly resolved: SlopeVariance;
  /** The Jacobian of the horizontal map: the area a unit of the Lagrangian grid covers. */
  readonly jacobian: number;
}

/**
 * The surface at the Lagrangian point (x₀, y₀) with each component faded by `footprintM`
 * ({@link componentFade}) and its amplitude scaled by `amplitudeScale`, the shore's depth limit s
 * (`shoreline.ts`' `shoreAmplitudeScale`): its displacement, x₀ − Σ Q f s A k̂ sin θ along and
 * Σ f s A cos θ up, θ = k · (x₀, y₀) + φ − ω t + k · ξ, (x₀, y₀) camera-relative; its normal,
 * ∂P ÷ ∂x₀ × ∂P ÷ ∂y₀; and the variance its drawn components resolve. The twin of
 * `oceanNear.wgsl`'s `ocean_near_surface`.
 */
export function oceanNearSurface(
  waves: ReadonlyArray<OceanWave>,
  x0M: number,
  y0M: number,
  footprintM: number,
  amplitudeScale = 1,
): OceanNearSurface {
  let dx = 0;
  let dy = 0;
  let dz = 0;
  // ∂P/∂x₀ = (1 − xx, −xy, −zx) and ∂P/∂y₀ = (−xy, 1 − yy, −zy).
  let xx = 0;
  let yy = 0;
  let xy = 0;
  let zx = 0;
  let zy = 0;
  let upwind = 0;
  let crosswind = 0;
  for (const w of waves) {
    const fade = componentFade(w.wavelengthM, footprintM);
    if (fade === 0) {
      continue;
    }
    const a = fade * amplitudeScale * w.amplitudeM;
    const k = Math.hypot(w.kxRadPerM, w.kyRadPerM);
    const ux = w.kxRadPerM / k;
    const uy = w.kyRadPerM / k;
    const theta = w.kxRadPerM * x0M + w.kyRadPerM * y0M + w.phaseRad;
    const s = Math.sin(theta);
    const c = Math.cos(theta);
    dx -= w.steepness * a * ux * s;
    dy -= w.steepness * a * uy * s;
    dz += a * c;
    const qakc = w.steepness * a * k * c;
    xx += qakc * ux * ux;
    yy += qakc * uy * uy;
    xy += qakc * ux * uy;
    zx += a * k * ux * s;
    zy += a * k * uy * s;
    const ka = k * a;
    const v = 1 - Math.sqrt(Math.max(1 - ka * ka, 0));
    upwind += v * w.upwindShare;
    crosswind += v * (1 - w.upwindShare);
  }
  const tx = vec3(1 - xx, -xy, -zx);
  const ty = vec3(-xy, 1 - yy, -zy);
  const n = normalise(cross(tx, ty));
  return {
    displacementM: [dx, dy, dz],
    normal: [n.x, n.y, n.z],
    resolved: { upwind, crosswind },
    jacobian: (1 - xx) * (1 - yy) - xy * xy,
  };
}

/**
 * The BRDF's variance at a near point (the module's remarks): the texel's total at its own wind less
 * the drawn components' resolved, per axis, at least the floor, plus the curvature across the
 * footprint, (ℓ ÷ a)² ÷ 12.
 */
export function nearBrdfVariance(
  wind10Mps: number,
  optics: Pick<OceanFarOptics, "sea" | "equatorialRadiusM">,
  resolved: SlopeVariance,
  footprintM: number,
): SlopeVariance {
  const total = oceanSlopeVariance(wind10Mps, optics.sea);
  const spread = footprintM / optics.equatorialRadiusM;
  const pixel = (spread * spread) / 12;
  return {
    upwind: Math.max(total.upwind - resolved.upwind, SLOPE_VARIANCE_FLOOR) + pixel,
    crosswind: Math.max(total.crosswind - resolved.crosswind, SLOPE_VARIANCE_FLOOR) + pixel,
  };
}

/** What the near sea's light at a point is formed from, beyond the far sea's optics. */
export interface OceanNearPoint {
  /** The unit direction from the point to the camera. */
  readonly view: Vec3;
  /** The local sphere's up and the wave's unit normal there, body-fixed. */
  readonly up: Vec3;
  readonly normal: Vec3;
  /** The texel's wind (east, north at the point) and depth. */
  readonly texel: OceanTexel;
  /** The drawn components' resolved variance there. */
  readonly resolved: SlopeVariance;
  readonly footprintM: number;
  /** The water's thickness there ({@link oceanThickness}). */
  readonly thickness: OceanThickness;
  /**
   * The shore's foam share there (`shoreline.ts`' `shoreFoamShare`), 0 off the shore; drawn, as the
   * whitecaps are, on the setting that draws them (`whitecaps`).
   */
  readonly shoreFoam: number;
  /**
   * The bed's light in air where the refracted ray meets it, cd/m² per channel, from the scene's
   * colour (refraction on); `null` on the low setting, or beyond the depth's range, where the bed is
   * the far sea's albedo.
   */
  readonly bedRadiance: Rgb | null;
}

/** The least refracted view cosine the column's path divides by (`oceanNear.wgsl`'s). */
export const OCEAN_NEAR_LEAST_REFRACTED = 1e-3;

/**
 * A wave's normal as the light takes it: reflected towards the camera where it faces away, n − 2 (n ·
 * v) v (Bruneton et al. 2010, §3).
 */
export function facingNormal(normal: Vec3, view: Vec3): Vec3 {
  const facing = dot(normal, view);
  return facing < 0 ? normalise(sub(normal, scale(view, 2 * facing))) : normal;
}

/** The near sea's light at a point, cd/m² per channel. */
export interface OceanNearLight {
  /** The whole, the glint capped. */
  readonly radiance: Rgb;
  /** The glint's own, uncapped. */
  readonly glint: Rgb;
}

/**
 * The near sea's radiance at a point under `light` (the module's remarks): the glint of the BRDF's
 * unresolved slopes about the wave's normal, Fresnel at its facet, the sky reflected, the column's
 * own light and the bed through it, refracted where the point gives the bed's light, and the
 * whitecaps. The twin of `oceanNear.wgsl`'s `ocean_near_light`.
 */
export function oceanNearLight(
  optics: OceanFarOptics,
  light: OceanPointLight,
  point: OceanNearPoint,
): OceanNearLight {
  const sun = optics.sunDirection;
  const { view, up } = point;
  const normal = facingNormal(point.normal, view);
  const { east, north } = eastNorthOf(up);
  const windVector = add(
    scale(east, point.texel.windEastMps),
    scale(north, point.texel.windNorthMps),
  );
  const wind10 = norm(windVector);
  // The wind's direction on the wave's facet: its tangent there.
  const along = wind10 > 0 ? scale(windVector, 1 / wind10) : east;
  const tangent = sub(along, scale(normal, dot(along, normal)));
  const upwind =
    norm(tangent) > 1e-9 ? normalise(tangent) : normalise(cross(normal, vec3(0, 0, 1)));
  const variance = nearBrdfVariance(wind10, optics, point.resolved, point.footprintM);
  const factor = oceanGlintFactor(view, sun, normal, upwind, variance, optics.refractiveIndex);
  const muV = Math.max(dot(normal, view), 0);
  const muS = Math.max(dot(normal, sun), 0);
  // The wind's whitecaps and the shore's foam, as independent shares of the surface, on the setting
  // that draws whitecaps.
  const caps = optics.whitecaps ? oceanWhitecapFraction(wind10, optics.sea) : 0;
  const shore = optics.whitecaps ? point.shoreFoam : 0;
  const whitecaps = 1 - (1 - caps) * (1 - shore);
  const depth = point.thickness.depthM;
  const channel = (k: 0 | 1 | 2): { radiance: number; glint: number } => {
    const n = optics.refractiveIndex[k];
    const glint = light.sunLux[k] * factor[k];
    const fresnelView = fresnelReflectance(n, muV);
    const fresnelSun = fresnelReflectance(n, muS);
    const sky = (fresnelView * light.skyLux[k]) / Math.PI;
    const down = light.sunLux[k] * Math.max(dot(up, sun), 0) + light.skyLux[k];
    let water = 0;
    const attenuation = optics.attenuationPerM;
    if (attenuation !== null) {
      const kd = attenuation[k];
      const muSt = refractedCosine(n, muS);
      const muVt = refractedCosine(n, muV);
      const entering =
        light.sunLux[k] * muS * (1 - fresnelSun) + light.skyLux[k] * (1 - optics.skyReflectance);
      const deep = optics.deepReflectance[k];
      const bed = point.bedRadiance;
      // One path for the column and the bed: down and back up the refracted rays with the bed's
      // light, twice the sun's way with the far sea's albedo.
      const through =
        bed === null
          ? Math.exp((-2 * kd * depth) / muSt)
          : Math.exp(-kd * depth * (1 / muSt + 1 / Math.max(muVt, OCEAN_NEAR_LEAST_REFRACTED)));
      water = ((1 - fresnelView) * deep * (1 - through) * entering) / (n * n * UPWELLING_Q_SR);
      water +=
        bed === null
          ? ((1 - fresnelView) * optics.bedAlbedo[k] * through * entering) /
            (n * n * UPWELLING_Q_SR)
          : ((bed[k] * (1 - fresnelSun) * (1 - fresnelView)) / (n * n)) * through;
    }
    const foam = (WHITECAP_ALBEDO * down) / Math.PI;
    return {
      radiance:
        (1 - whitecaps) * (Math.min(glint, optics.glintCapCdM2) + sky + water) + whitecaps * foam,
      glint,
    };
  };
  const [r, g, b] = [channel(0), channel(1), channel(2)];
  return { radiance: [r.radiance, g.radiance, b.radiance], glint: [r.glint, g.glint, b.glint] };
}

/** The wave's normal from the wind's frame (along, across, up) into the body-fixed axes. */
export function nearNormalOf(
  frame: Pick<OceanWaveAnchor, "upwind" | "across">,
  up: Vec3,
  normal: readonly [number, number, number],
): Vec3 {
  const along = normalise(sub(frame.upwind, scale(up, dot(frame.upwind, up))));
  const across = cross(up, along);
  return normalise(
    add(add(scale(along, normal[0]), scale(across, normal[1])), scale(up, normal[2])),
  );
}

/** A patch of sea seen from a camera, as the switch's continuity is measured. */
export interface OceanPatch {
  readonly optics: OceanFarOptics;
  readonly light: OceanPointLight;
  readonly texel: OceanTexel;
  /** The camera from the patch's centre, m, along the patch's up: its height. */
  readonly cameraM: Vec3;
  /** The patch's up, body-fixed, and its half-width, m. */
  readonly up: Vec3;
  readonly halfWidthM: number;
  /** The near sea's waves and the pixel's angle, rad. */
  readonly waves: ReadonlyArray<OceanWave>;
  readonly frame: Pick<OceanWaveAnchor, "upwind" | "across">;
}

/**
 * The mean radiance of a patch of sea as the near sea draws it, cd/m², and the glint's own: over a
 * `samples` × `samples` Lagrangian grid, each sample's light weighted by the area it covers as the
 * camera sees it, its Jacobian times its facet's projected share, J (n · v) ÷ (n · U); the
 * integrals over the patch ÷ its projected area, which the far sea's means over the same patch are
 * held to.
 */
export function oceanNearPatchRadiance(
  patch: OceanPatch,
  samples: number,
): { readonly radiance: Rgb; readonly glint: Rgb } {
  const sum: [number, number, number] = [0, 0, 0];
  const glint: [number, number, number] = [0, 0, 0];
  let weight = 0;
  const { optics, frame } = patch;
  for (let i = 0; i < samples; i += 1) {
    for (let j = 0; j < samples; j += 1) {
      const x0 = patch.halfWidthM * ((2 * (i + 0.5)) / samples - 1);
      const y0 = patch.halfWidthM * ((2 * (j + 0.5)) / samples - 1);
      const flat = add(scale(frame.upwind, x0), scale(frame.across, y0));
      const toCamera = sub(patch.cameraM, flat);
      const distance = norm(toCamera);
      const view = scale(toCamera, 1 / distance);
      const footprint =
        (distance * optics.pixelAngleRad) / Math.sqrt(Math.max(dot(view, patch.up), 0.01));
      const surface = oceanNearSurface(patch.waves, x0, y0, footprint);
      const normal = nearNormalOf(frame, patch.up, surface.normal);
      const lightAt = oceanNearLight(optics, patch.light, {
        view,
        up: patch.up,
        normal,
        texel: patch.texel,
        resolved: surface.resolved,
        footprintM: footprint,
        thickness: { depthM: patch.texel.depthM, from: "bathymetry", layer: "sea" },
        shoreFoam: 0,
        bedRadiance: null,
      });
      const w =
        (surface.jacobian * Math.max(dot(normal, view), 0)) / Math.max(dot(normal, patch.up), 1e-6);
      for (let k = 0; k < 3; k += 1) {
        sum[k] = (sum[k] ?? 0) + w * (lightAt.radiance[k] ?? 0);
        glint[k] = (glint[k] ?? 0) + w * (lightAt.glint[k] ?? 0);
      }
      weight += w;
    }
  }
  return {
    radiance: [sum[0] / weight, sum[1] / weight, sum[2] / weight],
    glint: [glint[0] / weight, glint[1] / weight, glint[2] / weight],
  };
}

/**
 * Whether the camera stands low enough over the sea for the near sea: its geodetic height above the
 * sea-level spheroid under {@link OCEAN_NEAR_BELOW_M}, and above it.
 */
export function drawsNearSea(
  optics: Pick<OceanFarOptics, "equatorialRadiusM" | "polarRadiusM">,
  cameraM: Vec3,
): boolean {
  const { heightM } = geodeticOf(cameraM, {
    equatorialRadiusM: optics.equatorialRadiusM,
    polarRadiusM: optics.polarRadiusM,
  });
  return heightM > 0 && heightM < OCEAN_NEAR_BELOW_M;
}
