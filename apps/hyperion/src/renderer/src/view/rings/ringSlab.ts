/**
 * A ring as an optical-depth slab (plan R11, R11.T9.c; Design notes 13 to 15 and 18): its record,
 * its radial profile's texels, the order its draws take among the frame's airs, its materials, and
 * the CPU twin of `ringSlab.wgsl`.
 *
 * @remarks
 * The slab is the annulus in the ring plane at every distance its planet is drawn larger than a
 * point, the far annulus beyond 10⁹ m included (R07's; Design note 14's "one model from its annulus
 * to its particles"): each fragment is the view ray's crossing of the plane, so that its edges fall
 * where R02's ellipses do. The lit face is the classical single-scattering layer times the
 * shadowing table's f_e plus its multiple scattering, the unlit face the transmitted beam
 * single-scattered plus its own (Design notes 14 and 15), the same on both settings; the planet's
 * shadow on it is R07's eclipse term with the planet the occluder, on both settings. The particles'
 * opposition surge (Design note 14) multiplies f_e, whose broad peak is the interparticle shadow
 * hiding: the surge's half-width of 0.20°–0.28° is the particles' own coherent backscatter, so the
 * peak is not counted twice (R11.T9.b's records; a lean, 2026-11-18). Light behind the ring is seen
 * through it by exp(−τ ÷ μ), grey, its particles being large.
 *
 * The ring's place among the frame's transparent layers is R02's (`transparentLayerOrder`) between
 * bodies, back to front; within its own body the order is the fragment's, since a ring plane lies
 * on either side of its planet's air along different rays (R02's "heuristic"): the fragments beyond
 * the near side of the air's shell are drawn before the air's composite, which lays the air over
 * them, and the others after it ({@link transparentSteps}).
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { LitBodyInput } from "../bodies/draw";
import { type DiscLight, type DiscOccluder, MAX_DISC_LIGHTS } from "../bodies/discShading";
import { figurePole } from "../bodies/oblate";
import { type DepthLayer, transparentLayerOrder } from "../depth/depth";
import type { MeshSpec, TextureBindingSpec, WgslMaterialSpec } from "../engine/types";
import { annulusVisibleFraction } from "../lighting/annuli";
import { hostAnnuli, type LightAtPoint } from "../lighting/hostLights";
import type { Rgb } from "../photometry/toneCurve";
import type { ViewRing } from "../scene/model";
import frameWgsl from "../shaders/frame.wgsl?raw";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import {
  DUSTY_BACKWARD_ASYMMETRY,
  DUSTY_FORWARD_ASYMMETRY,
  DUSTY_FORWARD_WEIGHT,
  dustyPhase,
  POWER_LAW_NORMALISATION,
  POWER_LAW_PHASE_EXPONENT,
  powerLawPhase,
  ringReflectance,
  ringTransmitted,
  SPARSE_SURGE_AMPLITUDE,
  SURGE_HALF_WIDTH_RAD,
} from "./ringPhotometry";
import { type RingBand, type RingProfile, type RingRegion, ringProfile } from "./ringProfile";
import ringSlabWgsl from "./shaders/ringSlab.wgsl?raw";
import { type RingFace, shadowingLookup, shadowingRatios } from "./shadowingLookup";
import { SHADOWING_HEADER } from "./shadowingTable";

/**
 * The slabs' pass label (R12's `PASS_ROWS` `primary:rings`): one pass a run of ring draws between
 * two of the frame's air composites (`transparentSteps`).
 */
export const RINGS_PASS_LABEL = "rings";

/** The first light's row, and the rows a light takes, as a disc record's lights. */
const FIRST_LIGHT_ROW = 5;
const LIGHT_ROWS = 8;

/** The stars that light a ring at most: its planet's, {@link MAX_DISC_LIGHTS}. */
export const MAX_RING_LIGHTS = MAX_DISC_LIGHTS;

/** The first occluder's row. */
const FIRST_OCCLUDER_ROW = FIRST_LIGHT_ROW + MAX_RING_LIGHTS * LIGHT_ROWS;

/**
 * The disc bodies a ring is hidden behind at most: 4, those that could stand before it on the
 * view, the largest first ({@link ringOccludersOf}). A body drawn with depth needs no slot: the
 * depth test, or the scene's depth the slab reads, hides the ring behind it.
 */
export const MAX_RING_OCCLUDERS = 4;

/** `vec4f` rows of a ring's record (`ringSlab.wgsl`'s `RING_ROWS`): its occluders' come last. */
export const RING_SLAB_ROWS = FIRST_OCCLUDER_ROW + MAX_RING_OCCLUDERS;

/**
 * Texels across a ring's profile, inner edge to outer: 2,048, some 30 km a texel across Saturn's
 * main rings, so that the Cassini Division's 4,590 km spans about 150.
 */
export const RING_PROFILE_TEXELS = 2048;

/** The profile's levels, each a row (`ringSlab.wgsl`): 2,048 texels down to a footprint of all. */
export const RING_PROFILE_LEVELS = 12;

/** The annulus mesh's segments: 256, whose outer polygon stands 7.5 × 10⁻⁵ out at its corners. */
export const RING_MESH_SEGMENTS = 256;

/**
 * How far the mesh reaches past each edge, a share of the edge's radius: 1.5%, so that the pixels
 * just outside an edge are drawn and its coverage falls off over the pixel (a ring of 70 px reaches
 * one pixel out).
 */
export const RING_MESH_MARGIN = 0.015;

/** Below this |n · d| a view ray runs along the plane, its crossing held off the annulus. */
const PARALLEL_COSINE = 1e-20;

/** The crossing's distance is held within ± this many a, beyond every annulus drawn. */
const FARTHEST_OVER_A = 1e6;

/** Below this |μ − μ₀| ÷ max(μ, μ₀) the unlit face takes its limit's expansion in `f32`. */
const UNLIT_NEAR_F32 = 1e-3;

/** `rgba16float`'s largest value, which the slab's light is held under. */
const HDR_MAX = 65504;

/** A number as a WGSL `f32` literal. */
function f32Literal(value: number): string {
  const text = String(value);
  return /[.e]/u.test(text) ? text : `${text}.0`;
}

/**
 * The slab's constants as WGSL, composed before `ringSlab.wgsl`: its record's layout, its
 * profile's and the shadowing table's axes, and the photometry's figures, each from the module that
 * owns it, so that the shader and its twin cannot part.
 */
export const RING_SLAB_CONSTANTS_WGSL = `
const RING_ROWS : u32 = ${RING_SLAB_ROWS}u;
const RING_FIRST_LIGHT_ROW : u32 = ${FIRST_LIGHT_ROW}u;
const RING_LIGHT_ROWS : u32 = ${LIGHT_ROWS}u;
const RING_MAX_LIGHTS : u32 = ${MAX_RING_LIGHTS}u;
const RING_FIRST_OCCLUDER_ROW : u32 = ${FIRST_OCCLUDER_ROW}u;
const RING_MAX_OCCLUDERS : u32 = ${MAX_RING_OCCLUDERS}u;
const RING_PROFILE_TEXELS : u32 = ${RING_PROFILE_TEXELS}u;
const RING_PROFILE_LEVELS : u32 = ${RING_PROFILE_LEVELS}u;
const RING_MESH_MARGIN : f32 = ${f32Literal(RING_MESH_MARGIN)};
const RING_PARALLEL_COSINE : f32 = ${f32Literal(PARALLEL_COSINE)};
const RING_FARTHEST : f32 = ${f32Literal(FARTHEST_OVER_A)};
const RING_UNLIT_NEAR : f32 = ${f32Literal(UNLIT_NEAR_F32)};
const RING_HDR_MAX : f32 = ${f32Literal(HDR_MAX)};
const RING_POWER_LAW_NORMALISATION : f32 = ${f32Literal(POWER_LAW_NORMALISATION)};
const RING_POWER_LAW_EXPONENT : f32 = ${f32Literal(POWER_LAW_PHASE_EXPONENT)};
const RING_DUSTY_FORWARD_WEIGHT : f32 = ${f32Literal(DUSTY_FORWARD_WEIGHT)};
const RING_DUSTY_FORWARD_G : f32 = ${f32Literal(DUSTY_FORWARD_ASYMMETRY)};
const RING_DUSTY_BACKWARD_G : f32 = ${f32Literal(DUSTY_BACKWARD_ASYMMETRY)};
const RING_SPARSE_SURGE_AMPLITUDE : f32 = ${f32Literal(SPARSE_SURGE_AMPLITUDE)};
const RING_TAU_NODES : u32 = ${SHADOWING_HEADER.tau.nodes}u;
const RING_TAU_SCALE : f32 = ${f32Literal(SHADOWING_HEADER.tau.scale)};
const RING_TAU_MAX : f32 = ${f32Literal(SHADOWING_HEADER.tau.max)};
const RING_MU_NODES : u32 = ${SHADOWING_HEADER.mu.nodes}u;
const RING_MU_MIN : f32 = ${f32Literal(SHADOWING_HEADER.mu.min)};
const RING_PHASE_NODES : u32 = ${SHADOWING_HEADER.phase.nodes}u;
const RING_PHASE_SCALE : f32 = ${f32Literal(SHADOWING_HEADER.phase.scale)};
`;

/** A ring as its slab draws it, every length over its planet's equatorial radius a. */
export interface RingSlabRecord {
  /** The planet's centre from the camera ÷ a, along the galactic axes. */
  readonly centre: Vec3;
  /** The plane's offset n · centre ÷ a, in `f64`: the camera's height below the plane. */
  readonly planeOffset: number;
  /** The ring plane's unit normal, along the galactic axes. */
  readonly normal: Vec3;
  readonly innerOverA: number;
  readonly outerOverA: number;
  /** The planet's unit pole, along the galactic axes. */
  readonly pole: Vec3;
  /** c ÷ a, 1 for a sphere. */
  readonly polarOverEquatorial: number;
  /** Whether the ring is a dusty sheet, whose particles scatter forward and show no surge. */
  readonly dusty: boolean;
  /** The scale its profile's optical depths are stored over ({@link profileTauScale}). */
  readonly tauScale: number;
  /** The exposure scale over π, so that the slab holds pre-exposed luminance. */
  readonly exposureOverPi: number;
  /** Its planet's stars, at most {@link MAX_RING_LIGHTS}, relative to the planet's centre ÷ a. */
  readonly lights: ReadonlyArray<DiscLight>;
  /** The disc bodies it may lie behind, at most {@link MAX_RING_OCCLUDERS}, from the planet ÷ a. */
  readonly occluders: ReadonlyArray<DiscOccluder>;
}

/** A body that may stand before a ring, from the camera, m. */
export interface RingOccluderCandidate {
  readonly id: BodyIdHex;
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/**
 * The disc bodies a ring may lie behind on the view, the largest seen first, at most
 * {@link MAX_RING_OCCLUDERS}: each other than its planet whose sphere overlaps the ring's bounding
 * sphere's on the view and whose near side lies nearer than the ring's far side.
 *
 * @param centreM - The ringed planet's centre from the camera, m.
 * @param outerM - The ring's outer edge, m.
 */
export function ringOccludersOf(
  planet: BodyIdHex,
  centreM: Vec3,
  outerM: number,
  candidates: ReadonlyArray<RingOccluderCandidate>,
): RingOccluderCandidate[] {
  const distanceM = norm(centreM);
  const ringRad = distanceM > outerM ? Math.asin(outerM / distanceM) : Math.PI;
  const towards = scale(centreM, 1 / distanceM);
  const seen = candidates.flatMap((body) => {
    const d = norm(body.centreM);
    if (body.id === planet || !(d > body.radiusM) || d - body.radiusM >= distanceM + outerM) {
      return [];
    }
    const bodyRad = Math.asin(body.radiusM / d);
    const along = scale(body.centreM, 1 / d);
    const apart = Math.atan2(norm(cross(along, towards)), dot(along, towards));
    return apart < ringRad + bodyRad ? [{ body, bodyRad }] : [];
  });
  return seen
    .toSorted((a, b) => b.bodyRad - a.bodyRad || (a.body.id < b.body.id ? -1 : 1))
    .slice(0, MAX_RING_OCCLUDERS)
    .map(({ body }) => body);
}

/** What a ring's record is made with beyond its ring and planet. */
export interface RingSlabOptions {
  /** The eclipse term's annuli, the setting's `discAnnuli`. */
  readonly annuli: number;
  /** The frame's exposure scale, so that the slab holds pre-exposed luminance. */
  readonly exposureScale: number;
}

/**
 * A ring's record from its ring, its planet as the frame lights it, its planet's stars (its
 * record's sources, `BodyLightSources.stars`) and the disc bodies it may lie behind.
 */
export function ringSlabRecordOf(
  ring: ViewRing,
  planet: LitBodyInput,
  stars: ReadonlyArray<LightAtPoint>,
  occluders: ReadonlyArray<RingOccluderCandidate>,
  options: RingSlabOptions,
): RingSlabRecord {
  const { equatorialRadiusM: a, polarRadiusM: c } = planet.figure;
  const centre = scale(planet.centreM, 1 / a);
  const normal = normalise(ring.normal);
  return {
    centre,
    planeOffset: dot(normal, planet.centreM) / a,
    normal,
    innerOverA: ring.innerRadiusM / a,
    outerOverA: ring.outerRadiusM / a,
    pole: figurePole(planet.figure),
    polarOverEquatorial: c / a,
    dusty: ring.ringKind === "dusty",
    tauScale: profileTauScale(ringProfile(ring)),
    exposureOverPi: options.exposureScale / Math.PI,
    lights: stars.slice(0, MAX_RING_LIGHTS).map((light) => {
      const d = norm(light.toStarM);
      return {
        direction: scale(light.toStarM, 1 / d),
        distance: d / a,
        radius: light.host.disc.radius_m / a,
        illuminance: light.illuminance,
        annuli: hostAnnuli(light.host.disc, options.annuli),
      };
    }),
    occluders: occluders.slice(0, MAX_RING_OCCLUDERS).map((body) => ({
      centre: scale(sub(body.centreM, planet.centreM), 1 / a),
      radius: body.radiusM / a,
    })),
  };
}

/** The annuli each of a record's lights carries per channel, the frame's K. */
function annulusCount(record: RingSlabRecord): number {
  return record.lights[0]?.annuli[0].flux.length ?? 0;
}

/** The records, {@link RING_SLAB_ROWS} `vec4f`s each, as `ringSlab.wgsl` reads them. */
export function packRingSlabRecords(records: ReadonlyArray<RingSlabRecord>): Float32Array {
  const out = new Float32Array(Math.max(records.length, 1) * RING_SLAB_ROWS * 4);
  records.forEach((record, i) => {
    const base = i * RING_SLAB_ROWS * 4;
    const put = (row: number, values: ArrayLike<number>): void => {
      out.set(Array.from(values), base + row * 4);
    };
    const { centre, normal, pole } = record;
    put(0, [centre.x, centre.y, centre.z, record.planeOffset]);
    put(1, [normal.x, normal.y, normal.z, record.innerOverA]);
    put(2, [pole.x, pole.y, pole.z, record.polarOverEquatorial]);
    const lights = record.lights.slice(0, MAX_RING_LIGHTS);
    const occluders = record.occluders.slice(0, MAX_RING_OCCLUDERS);
    put(3, [record.outerOverA, lights.length, occluders.length, record.exposureOverPi]);
    put(4, [record.dusty ? 1 : 0, annulusCount(record), record.tauScale, 0]);
    lights.forEach((light, j) => {
      const first = FIRST_LIGHT_ROW + j * LIGHT_ROWS;
      put(first, [light.direction.x, light.direction.y, light.direction.z, light.distance]);
      put(first + 1, [...light.illuminance, light.radius]);
      light.annuli.forEach((set, c) => {
        put(
          first + 2 + 2 * c,
          [1, 2, 3, 4].map((edge) => set.edges[edge] ?? 1),
        );
        put(
          first + 3 + 2 * c,
          [0, 1, 2, 3].map((annulus) => set.flux[annulus] ?? 0),
        );
      });
    });
    occluders.forEach((occluder, k) => {
      const at = occluder.centre;
      put(FIRST_OCCLUDER_ROW + k, [at.x, at.y, at.z, occluder.radius]);
    });
  });
  return out;
}

/** A band's surge half-width, rad: its region's (`SURGE_HALF_WIDTH_RAD`), 0 for dust, which has none. */
function halfWidthOf(region: RingRegion): number {
  let width: number;
  switch (region) {
    case "dense":
    case "sparse":
    case "gap":
      width = SURGE_HALF_WIDTH_RAD[region];
      break;
    case "dusty":
      width = 0;
      break;
  }
  return width;
}

/** The display channels r, g and b, as an `Rgb`'s indices. */
const CHANNELS = [0, 1, 2] as const;

/** The profile texture's rows: each level's first component, its second, then their mixture. */
export const RING_PROFILE_ROWS = 3 * RING_PROFILE_LEVELS;

/**
 * The scale a profile's optical depths are stored over: its densest band's τ, so that a dusty
 * ring's 10⁻⁷–10⁻⁵ keeps a half-float's precision (`ringProfileTexels`).
 */
export function profileTauScale(profile: RingProfile): number {
  return Math.max(...profile.bands.map((band) => band.tau), Number.MIN_VALUE);
}

/** A run of bands' optical depth, ϖ₀ and surge half-width averaged by length, and its length. */
interface BandMix {
  length: number;
  tau: number;
  albedo: [number, number, number];
  width: number;
}

/** The bands' length-weighted mean, `null` for none. */
function meanOf(
  parts: ReadonlyArray<{ readonly overlap: number; readonly band: RingBand }>,
): BandMix | null {
  const length = parts.reduce((sum, part) => sum + part.overlap, 0);
  if (!(length > 0)) {
    return null;
  }
  const mean = (of: (band: RingBand) => number): number =>
    parts.reduce((sum, part) => sum + part.overlap * of(part.band), 0) / length;
  return {
    length,
    tau: mean((band) => band.tau),
    albedo: [
      mean((band) => band.albedo[0]),
      mean((band) => band.albedo[1]),
      mean((band) => band.albedo[2]),
    ],
    width: mean((band) => halfWidthOf(band.region)),
  };
}

/**
 * A ring's profile as its slab samples it: {@link RING_PROFILE_TEXELS} texels across the annulus
 * and {@link RING_PROFILE_LEVELS} levels, `rgba16float`, x fastest (`ringSlab.wgsl`'s
 * `ring_profile`).
 *
 * @remarks
 * A pixel's light is the area average of the light over its footprint, and the light let through
 * the area average of exp(−τ ÷ μ), which a mean τ would understate wherever bands of unlike τ share
 * the footprint (exp is convex: a gap of τ 0.15 beside a band of 1.5 seen at 4.5° lets through
 * 0.074 of the light behind, the mean τ 3 × 10⁻⁵). So each level's texel holds the bands of the
 * 2^l texels about its own, held within the annulus so that its edges keep their own bands, as two
 * components and the share of the second: the bands thinner than the geometric mean of the
 * window's least and greatest τ, and the rest, each averaged by length (τ, ϖ₀ and the surge's
 * half-width w), which the slab shades apart and mixes by length (a lean, 2026-11-18). Exact for a
 * profile of two optical depths, as the provisional one is (a ring and its gaps); a window of one
 * holds it twice, at a share of 0. Rows l, {@link RING_PROFILE_LEVELS} + l and
 * 2 {@link RING_PROFILE_LEVELS} + l hold the first component's (ϖ₀ r, g, b, τ ÷ the scale), the
 * second's, and (its share, the first's w, the second's w, 0), τ over {@link profileTauScale}.
 */
export function ringProfileTexels(profile: RingProfile): Float16Array {
  const out = new Float16Array(RING_PROFILE_TEXELS * RING_PROFILE_ROWS * 4);
  const span = profile.outerM - profile.innerM;
  const texelM = span / RING_PROFILE_TEXELS;
  const tauScale = profileTauScale(profile);
  const put = (row: number, i: number, values: readonly [number, number, number, number]): void => {
    out.set(values, (row * RING_PROFILE_TEXELS + i) * 4);
  };
  for (let level = 0; level < RING_PROFILE_LEVELS; level += 1) {
    const halfM = (texelM * 2 ** level) / 2;
    for (let i = 0; i < RING_PROFILE_TEXELS; i += 1) {
      const middle = profile.innerM + (i + 0.5) * texelM;
      const from = Math.max(middle - halfM, profile.innerM);
      const to = Math.min(middle + halfM, profile.outerM);
      const parts = profile.bands.flatMap((band) => {
        const overlap = Math.min(to, band.outerM) - Math.max(from, band.innerM);
        return overlap > 0 ? [{ overlap, band }] : [];
      });
      const taus = parts.map((part) => part.band.tau);
      const split = Math.sqrt(Math.min(...taus) * Math.max(...taus));
      const thin = meanOf(parts.filter((part) => part.band.tau < split));
      const first = meanOf(parts.filter((part) => !(part.band.tau < split)));
      if (first === null) {
        continue;
      }
      const second = thin ?? first;
      const share = thin === null ? 0 : thin.length / (thin.length + first.length);
      put(level, i, [...first.albedo, first.tau / tauScale]);
      put(RING_PROFILE_LEVELS + level, i, [...second.albedo, second.tau / tauScale]);
      put(2 * RING_PROFILE_LEVELS + level, i, [share, first.width, second.width, 0]);
    }
  }
  return out;
}

/** One component of a profile's fetch: its particles' ϖ₀, τ and the surge's half-width, rad. */
export interface ProfileComponent {
  readonly albedo: Rgb;
  readonly tau: number;
  readonly halfWidthRad: number;
}

/** What one footprint's fetch of a profile gives: its two components and the second's share. */
export interface ProfileSample {
  readonly first: ProfileComponent;
  readonly second: ProfileComponent;
  readonly share: number;
}

/**
 * A profile's texels sampled as `ringSlab.wgsl`'s three fetches take them: bilinear, clamped to
 * the edge, at u across the annulus and `level` between its rows, τ times `tauScale`.
 */
export function profileSample(
  texels: Float16Array,
  u: number,
  level: number,
  tauScale: number,
): ProfileSample {
  const x = Math.min(Math.max(u, 0), 1) * RING_PROFILE_TEXELS - 0.5;
  const i0 = Math.floor(x);
  const wx = x - i0;
  const column = (i: number): number => Math.min(Math.max(i, 0), RING_PROFILE_TEXELS - 1);
  const l0 = Math.floor(level);
  const wl = level - l0;
  const l1 = Math.min(l0 + 1, RING_PROFILE_LEVELS - 1);
  const texel = (row: number, i: number, c: number): number =>
    texels[(row * RING_PROFILE_TEXELS + column(i)) * 4 + c] ?? 0;
  const at = (block: number, c: number): number => {
    const first = block * RING_PROFILE_LEVELS;
    const along = (row: number): number =>
      texel(row, i0, c) * (1 - wx) + texel(row, i0 + 1, c) * wx;
    return along(first + l0) * (1 - wl) + along(first + l1) * wl;
  };
  const component = (block: number, width: number): ProfileComponent => ({
    albedo: [at(block, 0), at(block, 1), at(block, 2)],
    tau: at(block, 3) * tauScale,
    halfWidthRad: at(2, width),
  });
  return { first: component(0, 1), second: component(1, 2), share: at(2, 0) };
}

/** The profile's level for a pixel whose footprint across the radius is `footprintOverA`. */
export function profileLevel(record: RingSlabRecord, footprintOverA: number): number {
  const span = record.outerOverA - record.innerOverA;
  const texels = (footprintOverA / span) * RING_PROFILE_TEXELS;
  return Math.min(Math.max(Math.log2(Math.max(texels, 1)), 0), RING_PROFILE_LEVELS - 1);
}

/**
 * The particles' own opposition surge with half-width `halfWidthRad`, as the slab reads it from
 * its profile: 1 + (A(τ) − 1) e^(−α ln 2 ÷ w), A(τ) = 1 + 0.45 ÷ (1 + τ) (`ringPhotometry.ts`'
 * `oppositionSurge` at its region's w); 1 for a dusty ring or a width of 0.
 */
export function surgeAt(
  alphaRad: number,
  tau: number,
  halfWidthRad: number,
  dusty: boolean,
): number {
  if (dusty || !(halfWidthRad > 0)) {
    return 1;
  }
  const amplitude = (SPARSE_SURGE_AMPLITUDE - 1) / (1 + Math.max(tau, 0));
  return 1 + amplitude * Math.exp((-Math.max(alphaRad, 0) * Math.LN2) / halfWidthRad);
}

/** Which of a ring's fragments a draw keeps, by its planet's air shell along the view ray. */
export type RingPart = "all" | "beyond-air" | "before-air";

/** Each part's code in `ringSlab.wgsl`'s `Draw.part`. */
export const RING_PART_CODES: Readonly<Record<RingPart, number>> = {
  all: 0,
  "beyond-air": 1,
  "before-air": 2,
};

/** A ray's two roots against a sphere, by the cross product's closest approach, or `null`. */
function sphereHits(ray: Vec3, centre: Vec3, radius: number): readonly [number, number] | null {
  const along = dot(centre, ray);
  const across = cross(centre, ray);
  const h2 = radius * radius - dot(across, across);
  if (h2 < 0) {
    return null;
  }
  const half = Math.sqrt(h2);
  return [along - half, along + half];
}

/** `p` stretched along the pole by a ÷ c, which makes the planet's spheroid the unit sphere. */
function stretched(p: Vec3, pole: Vec3, stretch: number): Vec3 {
  return add(p, scale(pole, (stretch - 1) * dot(p, pole)));
}

/**
 * The share of light j's flux per channel the planet leaves visible at the ring point `x`, as
 * `ringSlab.wgsl`'s `ring_planet_visible` forms it: the eclipse term with the planet the occluder,
 * in the frame stretched by a ÷ c along the pole (`ringShadows.ts`' `planetShadowOnRing`).
 */
export function ringPlanetVisible(record: RingSlabRecord, light: DiscLight, x: Vec3): Rgb {
  const stretch = 1 / record.polarOverEquatorial;
  const p = stretched(x, record.pole, stretch);
  const toStar = sub(stretched(scale(light.direction, light.distance), record.pole, stretch), p);
  const starDistance = norm(toStar);
  const planetDistance = norm(p);
  if (planetDistance <= 1) {
    return [0, 0, 0];
  }
  if (planetDistance >= starDistance) {
    return [1, 1, 1];
  }
  const starRadius = Math.asin(Math.min(light.radius / starDistance, 1));
  const planetRadius = Math.asin(1 / planetDistance);
  const along = scale(p, -1 / planetDistance);
  const towards = scale(toStar, 1 / starDistance);
  const separation = Math.atan2(norm(cross(along, towards)), dot(along, towards));
  if (separation >= planetRadius + starRadius) {
    return [1, 1, 1];
  }
  const visible = (c: (typeof CHANNELS)[number]): number =>
    annulusVisibleFraction(light.annuli[c], planetRadius / starRadius, separation / starRadius);
  return [visible(0), visible(1), visible(2)];
}

/** One fragment of the slab as its twin forms it. */
export interface RingSlabFragment {
  /** Its light towards the camera, pre-exposed, per channel, times its coverage. */
  readonly radiance: Rgb;
  /** The share of the light behind it that it takes, times its coverage: its premultiplied alpha. */
  readonly alpha: number;
  /** The share of the pixel its edges leave it. */
  readonly coverage: number;
  /** Its point from the planet's centre ÷ a. */
  readonly point: Vec3;
}

/** `v` held within [0, 1]. */
function clamp01(v: number): number {
  return Math.min(Math.max(v, 0), 1);
}

/** A twin fragment's draw: its part, and its planet's air shell's radius ÷ a for a part's test. */
export interface RingFragmentDraw {
  readonly part: RingPart;
  readonly shellOverA: number;
}

const ALL_FRAGMENTS: RingFragmentDraw = { part: "all", shellOverA: 0 };

/**
 * The CPU twin of `ringSlab.wgsl`'s fragment in `f64`: the slab along the view ray `ray` (unit,
 * along the galactic axes, from the camera) through a pixel whose footprint across the radius is
 * `footprintOverA` (the shader's derivatives), or `null` where the draw keeps no fragment there.
 *
 * @remarks
 * Its photometry is `ringPhotometry.ts`' `ringReflectance` and `ringTransmitted` and its tables
 * `shadowingLookup.ts`' and {@link profileSample}'s, the GPU's own half-floats with its weights.
 * It reads no depth: a pixel the scene's depth hides is the caller's to leave out.
 */
export function ringSlabFragment(
  record: RingSlabRecord,
  texels: Float16Array,
  ray: Vec3,
  footprintOverA: number,
  draw: RingFragmentDraw = ALL_FRAGMENTS,
): RingSlabFragment | null {
  const n = record.normal;
  const den = dot(n, ray);
  const alongPlane = Math.abs(den) < PARALLEL_COSINE ? PARALLEL_COSINE : den;
  const t = Math.min(Math.max(record.planeOffset / alongPlane, -FARTHEST_OVER_A), FARTHEST_OVER_A);
  const x = sub(scale(ray, t), record.centre);
  const r = norm(x);
  const inner = record.innerOverA;
  const outer = record.outerOverA;
  const span = outer - inner;
  const level = profileLevel(record, footprintOverA);
  const sample = profileSample(texels, (r - inner) / span, level, record.tauScale);
  const edge = Math.max(footprintOverA, 1e-30);
  const coverage = clamp01((r - inner) / edge + 0.5) * clamp01((outer - r) / edge + 0.5);
  if (!(t > 0 && coverage > 0)) {
    return null;
  }
  const stretch = 1 / record.polarOverEquatorial;
  const scaledRay = stretched(ray, record.pole, stretch);
  const length = norm(scaledRay);
  const planet = sphereHits(
    scale(scaledRay, 1 / length),
    stretched(record.centre, record.pole, stretch),
    1,
  );
  if (planet !== null && planet[1] > 0 && planet[0] / length < t) {
    return null;
  }
  for (const occluder of record.occluders.slice(0, MAX_RING_OCCLUDERS)) {
    const hits = sphereHits(ray, add(record.centre, occluder.centre), occluder.radius);
    if (hits !== null && hits[1] > 0 && hits[0] < t) {
      return null;
    }
  }
  if (draw.part !== "all") {
    const shell = sphereHits(ray, record.centre, draw.shellOverA);
    const beyond = shell !== null && shell[1] > 0 && t > Math.max(shell[0], 0);
    if (beyond !== (draw.part === "beyond-air")) {
      return null;
    }
  }
  const shaded = ringSlabShade(record, x, ray, sample);
  return {
    radiance: [
      coverage * Math.min(shaded.radiance[0], HDR_MAX),
      coverage * Math.min(shaded.radiance[1], HDR_MAX),
      coverage * Math.min(shaded.radiance[2], HDR_MAX),
    ],
    alpha: coverage * (1 - shaded.transmittance),
    coverage,
    point: x,
  };
}

/** The slab's light at a ring point and the share of the light behind it it lets through. */
interface RingShade {
  readonly radiance: Rgb;
  readonly transmittance: number;
}

/**
 * The slab's light at the ring point `x` seen along `ray` (`ringSlab.wgsl`'s `ring_shade`): each
 * light's I/F per channel times its face-on illuminance, the planet's shadow and exposure ÷ π, for
 * each of the footprint's two components, mixed with the light each lets through by the second's
 * share.
 */
function ringSlabShade(
  record: RingSlabRecord,
  x: Vec3,
  ray: Vec3,
  sample: ProfileSample,
): RingShade {
  const n = record.normal;
  const den = dot(n, ray);
  const mu = Math.abs(den);
  const components = [
    { part: sample.first, weight: 1 - sample.share },
    { part: sample.second, weight: sample.share },
  ];
  const radiance = [0, 0, 0];
  for (const light of record.lights.slice(0, MAX_RING_LIGHTS)) {
    const towards = normalise(sub(scale(light.direction, light.distance), x));
    const sN = dot(towards, n);
    const mu0 = Math.abs(sN);
    if (!(mu0 > 0)) {
      continue;
    }
    const visible = ringPlanetVisible(record, light, x);
    if (!visible.some((v) => v > 0)) {
      continue;
    }
    const back = scale(ray, -1);
    const alpha = Math.atan2(norm(cross(towards, back)), dot(towards, back));
    const particles = record.dusty ? dustyPhase(alpha) : powerLawPhase(alpha);
    const muEff = (2 * mu * mu0) / (mu + mu0);
    const range = Math.max(Math.PI - 2 * Math.asin(Math.min(muEff, 1)), 1e-6);
    const face: RingFace = sN * den < 0 ? "lit" : "unlit";
    const v = face === "lit" ? alpha / range : (Math.PI - alpha) / range;
    const geometry = face === "lit" ? mu0 / (mu + mu0) : mu0;
    for (const { part, weight } of components) {
      const tau = Math.max(part.tau, 0);
      const phase = particles * surgeAt(alpha, tau, part.halfWidthRad, record.dusty);
      const table = shadowingLookup(face, tau, muEff, v);
      const q = shadowingRatios(face, tau, muEff);
      for (const c of CHANNELS) {
        const albedo = part.albedo[c];
        const scatter = { tau, mu, mu0, albedo, phase };
        const single =
          face === "lit" ? ringReflectance(scatter, table.shadowing) : ringTransmitted(scatter);
        const w2 = albedo * albedo;
        const multiple =
          (w2 * table.order2 * geometry) / (1 - w2 * q.even) +
          (w2 * albedo * table.order3 * geometry) / (1 - w2 * q.odd);
        radiance[c] =
          (radiance[c] ?? 0) +
          weight * (single + multiple) * light.illuminance[c] * visible[c] * record.exposureOverPi;
      }
    }
  }
  const through = (tau: number): number => (mu > 0 ? Math.exp(-Math.max(tau, 0) / mu) : 0);
  return {
    radiance: [radiance[0] ?? 0, radiance[1] ?? 0, radiance[2] ?? 0],
    transmittance:
      (1 - sample.share) * through(sample.first.tau) + sample.share * through(sample.second.tau),
  };
}

/** One step of a frame's transparent layers: a body's air composite, or a draw of its ring. */
export type TransparentStep<A> =
  | { readonly kind: "air"; readonly air: A }
  | { readonly kind: "ring"; readonly ring: number; readonly part: RingPart };

/** A body's air as {@link transparentSteps} orders it: its body and its shell's radius, m. */
export interface OrderedAir<A> {
  readonly body: BodyIdHex;
  readonly radiusM: number;
  readonly air: A;
}

/**
 * The frame's airs and rings in the order they are drawn (R02's `transparentLayerOrder` between
 * bodies): each body's in turn, back to front by its distance, and within a body with an air drawn
 * its rings' fragments beyond the air's near side first, then its air, then its rings' others; a
 * body drawn without air draws its rings whole.
 *
 * @param rings - The frame's drawn rings, each its index and its planet.
 * @param distanceM - The camera's distance from a body's centre, m.
 */
export function transparentSteps<A>(
  airs: ReadonlyArray<OrderedAir<A>>,
  rings: ReadonlyArray<{ readonly index: number; readonly body: BodyIdHex }>,
  distanceM: (body: BodyIdHex) => number,
): TransparentStep<A>[] {
  const layers: DepthLayer[] = [
    ...airs.map((each): DepthLayer => ({
      kind: "shell",
      id: `air:${each.body}`,
      body: each.body,
      radiusM: each.radiusM,
    })),
    ...rings.map((ring): DepthLayer => ({
      kind: "plane",
      id: `ring:${ring.index}`,
      body: ring.body,
    })),
  ];
  const bodies: BodyIdHex[] = [];
  for (const layer of transparentLayerOrder(layers, { bodyDistanceM: distanceM })) {
    if (layer.kind !== "opaque" && !bodies.includes(layer.body)) {
      bodies.push(layer.body);
    }
  }
  const steps: TransparentStep<A>[] = [];
  for (const body of bodies) {
    const own = rings.filter((ring) => ring.body === body).map((ring) => ring.index);
    const air = airs.find((each) => each.body === body);
    const ringsAs = (part: RingPart): void => {
      for (const ring of own) {
        steps.push({ kind: "ring", ring, part });
      }
    };
    if (air === undefined) {
      ringsAs("all");
    } else {
      ringsAs("beyond-air");
      steps.push({ kind: "air", air: air.air });
      ringsAs("before-air");
    }
  }
  return steps;
}

/** The two unit vectors spanning the plane of unit `n`, as `ringSlab.wgsl`'s `ring_plane_axes`. */
export function planeAxes(n: Vec3): readonly [Vec3, Vec3] {
  const a = Math.abs(n.x) > 0.9 ? vec3(0, 1, 0) : vec3(1, 0, 0);
  const e1 = normalise(cross(n, a));
  return [e1, cross(n, e1)];
}

/**
 * The annulus's mesh: {@link RING_MESH_SEGMENTS} quads between its inner polygon, inscribed in a
 * unit circle, and its outer one, circumscribed about it (each corner out by 1 ÷ cos(π ÷ N)), each
 * corner (x, y, edge) in the plane's axes, edge 0 inner and 1 outer; `ringSlab.wgsl` scales each
 * polygon to its edge with its margin.
 */
export function ringSlabMesh(name: string): MeshSpec {
  const n = RING_MESH_SEGMENTS;
  const out = 1 / Math.cos(Math.PI / n);
  const positions = new Float32Array(n * 2 * 3);
  for (let i = 0; i < n; i += 1) {
    const theta = (2 * Math.PI * i) / n;
    positions.set([Math.cos(theta), Math.sin(theta), 0], i * 6);
    positions.set([Math.cos(theta) * out, Math.sin(theta) * out, 1], i * 6 + 3);
  }
  const indices = new Uint32Array(n * 6);
  for (let i = 0; i < n; i += 1) {
    const inner = 2 * i;
    const outer = inner + 1;
    const nextInner = (2 * (i + 1)) % (2 * n);
    const nextOuter = nextInner + 1;
    indices.set([inner, outer, nextOuter, inner, nextOuter, nextInner], i * 6);
  }
  return { name, positions, indices, topology: "triangle-list", attributes: {} };
}

/** The ray through a point of a view, px, along the galactic axes: `ringSlab.wgsl`'s, rotated. */
export function pixelRay(
  xPx: number,
  yPx: number,
  rows: readonly [Vec3, Vec3, Vec3],
  fovXRad: number,
  viewport: { readonly widthPx: number; readonly heightPx: number },
): Vec3 {
  const s = 1 / Math.tan(fovXRad / 2);
  const aspect = viewport.widthPx / viewport.heightPx;
  const ndcX = (xPx / viewport.widthPx) * 2 - 1;
  const ndcY = 1 - (yPx / viewport.heightPx) * 2;
  const view = normalise(vec3(ndcX / s, ndcY / (s * aspect), -1));
  const [r0, r1, r2] = rows;
  return vec3(dot(r0, view), dot(r1, view), dot(r2, view));
}

/** The shader's variant that tests depth through the target's own attachment: no read of its own. */
const ATTACHED_DEPTH_WGSL = `
fn scene_depth_nearer(position : vec4f) -> bool {
  return false;
}
`;

/**
 * The variant drawn into an air target, which holds no depth: the scene's depth read at the
 * fragment's pixel, nearer under reversed-Z where it is greater, as the attachment's
 * `greater-equal` test would refuse it.
 */
const SAMPLED_DEPTH_WGSL = `
@group(2) @binding(5) var scene_depth : texture_depth_2d;

fn scene_depth_nearer(position : vec4f) -> bool {
  return textureLoad(scene_depth, vec2u(position.xy), 0) > position.z;
}
`;

/** The slab's source with its depth's variant. */
function slabSource(depth: string): string {
  return frameWgsl + preciseTrigWgsl + RING_SLAB_CONSTANTS_WGSL + depth + ringSlabWgsl;
}

/** The textures both materials bind: the profile and the shadowing table's two. */
const SLAB_TEXTURES: ReadonlyArray<TextureBindingSpec> = [
  { name: "ringProfile", binding: 1 },
  { name: "ringShadowing", binding: 2, viewDimension: "3d" },
  { name: "ringRatios", binding: 3 },
];

/** The scene's depth, which the `sampled` material reads (`ringSlab.wgsl`'s `scene_depth_nearer`). */
export const RING_SCENE_DEPTH_TEXTURE = "sceneDepth";

/** The materials' shared state: premultiplied over the light behind, testing depth and writing none. */
function slabMaterial(
  name: string,
  displayName: string,
  depth: string,
  sampled: boolean,
): WgslMaterialSpec {
  const source = slabSource(depth);
  return {
    name,
    displayName,
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [
      { name: "ring", type: "u32" },
      { name: "part", type: "u32" },
      { name: "shellOverA", type: "f32" },
      { name: "radiusM", type: "f32" },
    ],
    samplers: [{ name: "ringSampler", filter: "linear", address: "clamp-to-edge", binding: 4 }],
    textures: sampled
      ? [...SLAB_TEXTURES, { name: RING_SCENE_DEPTH_TEXTURE, binding: 5, sampleType: "depth" }]
      : SLAB_TEXTURES,
    storageBuffers: [{ name: "rings", binding: 0 }],
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "premultiplied",
  };
}

/**
 * The slab's two materials (`ringSlab.wgsl`): `attached`, drawn into the scene target, whose depth
 * attachment it tests; `sampled`, drawn into an air target after a composite, reading the scene's
 * depth itself.
 */
export const RING_SLAB_MATERIALS: Readonly<Record<"attached" | "sampled", WgslMaterialSpec>> = {
  attached: slabMaterial("rings:slab", "RING SLABS", ATTACHED_DEPTH_WGSL, false),
  sampled: slabMaterial("rings:slabOverAir", "RINGS OVER AIR", SAMPLED_DEPTH_WGSL, true),
};
