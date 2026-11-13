/**
 * The decoration's integer lattice and its fields: the TypeScript reference that `decoration.wgsl`
 * mirrors (plan R11, R11.T2.a; Design notes 3 and 20).
 *
 * @remarks
 * - **The lattice.** Decoration is evaluated on the cube sphere's own grid, never from world-space
 *   `f32` positions (Design note 3). A lattice of level m is a face's grid of 2^m × 2^m cells; the
 *   place (u, v) ∈ [0, 1]² of a patch (face, n, i, j) lies in the cell (⌊(i + u) 2^(m − n)⌋,
 *   ⌊(j + v) 2^(m − n)⌋), formed in integers ({@link latticePlace}), so that it neither swims as
 *   the camera moves nor differs between two views of one patch, and two patches of one face, of
 *   any levels, meet on one lattice with no seam. Each lattice point is keyed by
 *   {@link latticeHash} of the body's decoration seed, a salt, the face, the level and the point,
 *   Wellons's `lowbias32` in `u32` arithmetic, which `decoration.wgsl` computes to the bit.
 * - **The octaves.** The coarsest octave's lattice is the body's {@link decorationLevel}, the
 *   coarsest whose cells are at most half the band limit across anywhere on a face, so that its
 *   noise's wavelengths, about two cells and shorter, lie below the band limit and no octave above
 *   it is ever summed. Each of the {@link DECORATION_OCTAVES} octaves halves the cell. The crater
 *   scars take {@link SCAR_OCTAVES} lattices from {@link SCAR_LEVEL_OFFSET} levels coarser, whose
 *   largest scar is half a cell, the band limit at most; the ripples one lattice
 *   {@link RIPPLE_LEVEL_OFFSET} levels finer.
 * - **The fade.** Every octave fades across one octave of pixel footprint (Design note 3): wholly
 *   in while its cell spans 2 px or more, out at 1 px ({@link octaveFade}), the footprint the view's
 *   pixel at the texel's distance (`screenSpaceErrorPx`'s), never a screen-space derivative, so that
 *   a still's tile fades as the untiled still does (`stillPlan.ts`' `footprintView`).
 * - **Zero mean.** Every field is a sum of terms each odd or of zero integral about its own centre:
 *   gradient noise's corners, a ripple's sinusoid and a scar's bowl, each scar wholly inside its
 *   lattice cell. Over a patch, whose cells the lattices' nest in, only the noise terms its edges
 *   cut and the ripples' last part-period are left, and the mean of each perturbation is within
 *   10⁻³ of none (the tests).
 * - **What it is not.** It is decoration: it perturbs the shading normal and the class albedo
 *   only, never a vertex, and it is not bit-identical across GPUs, since nothing reads it back; only
 *   the lattice's integers and hashes are exact.
 */

import type { PatchKey } from "../../terrain/patchKey";

/**
 * Chris Wellons's `lowbias32`, a 32-bit integer hash: a two-round xorshift-multiply permutation of
 * low bias (its exact bias 0.17353), shifts 16, 15 and 16 and multipliers 0x7feb352d and
 * 0x846ca68b (Wellons 2018, "Prospecting for Hash Functions", and the `skeeto/hash-prospector`
 * repository's README, its `lowbias32`, checked 2026-11-13), in `u32` arithmetic:
 * `decoration.wgsl`'s `decoration_mix`, to the bit.
 */
export function lowbias32(x: number): number {
  let h = x >>> 0;
  h = (h ^ (h >>> 16)) >>> 0;
  h = Math.imul(h, 0x7feb352d) >>> 0;
  h = (h ^ (h >>> 15)) >>> 0;
  h = Math.imul(h, 0x846ca68b) >>> 0;
  h = (h ^ (h >>> 16)) >>> 0;
  return h;
}

/**
 * The salts that key each of the decoration's fields apart on one lattice: the noise of the normal
 * and the albedo (one hash, its bits split between them), a ripple's wobble and a crater scar's.
 */
export const DECORATION_SALT = { noise: 1, wobble: 2, scar: 3 } as const;

/** The salt by which a body's decoration seed is drawn from its detail seed. */
const SEED_SALT = 0x6465_636f;

/**
 * A body's decoration seed, `u32`, from its detail seed (16 lowercase hexadecimal digits,
 * `ViewBody.detailSeed`), so that two worlds of one substance differ and every console sees one
 * world the same (Design note 21).
 *
 * @throws RangeError for a seed that is not 16 lowercase hexadecimal digits.
 */
export function decorationSeed(detailSeed: string): number {
  if (!/^[0-9a-f]{16}$/u.test(detailSeed)) {
    throw new RangeError(`a detail seed is 16 lowercase hexadecimal digits, not ${detailSeed}`);
  }
  const high = Number.parseInt(detailSeed.slice(0, 8), 16);
  const low = Number.parseInt(detailSeed.slice(8), 16);
  return lowbias32((lowbias32((high ^ SEED_SALT) >>> 0) ^ low) >>> 0);
}

/** The largest lattice level: a cell's integers, and the next one's, stay below 2³². */
export const MAX_LATTICE_LEVEL = 31;

/**
 * The hash of a lattice's level and salt, which every point of it shares (the first two rounds of
 * {@link latticeHash}, formed once an octave); `decoration_prefix`.
 *
 * @param salt - One of {@link DECORATION_SALT}, below 2²⁴.
 * @param face - 0 to 5.
 * @param level - 0 to {@link MAX_LATTICE_LEVEL}.
 */
export function latticePrefix(seed: number, salt: number, face: number, level: number): number {
  return lowbias32((seed ^ lowbias32(((salt << 8) | (face << 5) | level) >>> 0)) >>> 0);
}

/**
 * The hash of lattice point (x, y) under {@link latticePrefix}'s `prefix`: two more rounds of
 * `lowbias32`, `decoration.wgsl`'s `decoration_point`, to the bit.
 */
export function latticePoint(prefix: number, x: number, y: number): number {
  return lowbias32((lowbias32((prefix ^ x) >>> 0) ^ y) >>> 0);
}

/**
 * The hash of point (x, y) of the lattice of `level` on `face`, under `salt`, for a body of
 * decoration seed `seed`: {@link latticePoint} of {@link latticePrefix}; `decoration_hash`.
 */
export function latticeHash(
  seed: number,
  salt: number,
  face: number,
  level: number,
  x: number,
  y: number,
): number {
  return latticePoint(latticePrefix(seed, salt, face, level), x >>> 0, y >>> 0);
}

/** A texel's place on one lattice: its cell's integers and its fraction across the cell. */
export interface LatticePlace {
  /** The cell's column, from 0 at the face's u = −1 edge. */
  readonly x: number;
  /** The cell's row, from 0 at the face's v = −1 edge. */
  readonly y: number;
  /** Its fraction across the cell along x, [0, 1). */
  readonly fx: number;
  /** Its fraction across the cell along y, [0, 1). */
  readonly fy: number;
}

/** One axis of {@link latticePlace}: the cell and the fraction of patch index `i` at `t`. */
function axisPlace(i: number, t: number, patchLevel: number, level: number): [number, number] {
  if (level >= patchLevel) {
    const scale = 2 ** (level - patchLevel);
    const along = t * scale;
    const whole = Math.floor(along);
    return [i * scale + whole, along - whole];
  }
  const size = 2 ** (patchLevel - level);
  return [Math.floor(i / size), ((i % size) + t) / size];
}

/**
 * The place of the point (u, v) of patch `key`, each in [0, 1] from the patch's low corner (the lit
 * terrain's `patchUv`), on the lattice of `level`: `decoration.wgsl`'s `decoration_place`, whose
 * integers, and for a `u` and `v` exact in `f32` whose fractions, it gives to the bit.
 *
 * @throws RangeError for a level outside 0 to {@link MAX_LATTICE_LEVEL}.
 */
export function latticePlace(key: PatchKey, u: number, v: number, level: number): LatticePlace {
  if (!Number.isInteger(level) || level < 0 || level > MAX_LATTICE_LEVEL) {
    throw new RangeError(`a lattice level is 0 to ${MAX_LATTICE_LEVEL}, not ${level}`);
  }
  const [x, fx] = axisPlace(key.i, u, key.level, level);
  const [y, fy] = axisPlace(key.j, v, key.level, level);
  return { x, y, fx, fy };
}

/**
 * The largest arc of a face's grid per unit of its cell coordinate s, in units of the body's
 * radius: the maximum over a face of |∂p ÷ ∂s| on the unit sphere, du/ds √(1 + v²) ÷ (1 + u² + v²)
 * for S2's quadratic warp, 1.70490 at s ≈ 0.7398, v = 0 (computed on a grid of 10⁶ points,
 * 2026-11-02; the least, 1.257, is at a corner), rounded up.
 */
export const FACE_ARC_PER_S = 1.705;

/** The noise octaves the decoration sums, the coarsest at {@link decorationLevel}. */
export const DECORATION_OCTAVES = 6;

/**
 * The share of the albedo's amplitude each finer octave keeps, 1 ÷ √2, so that the colour
 * variation's power falls as the wavenumber's inverse (a lean, 2026-11-02); the normal's slope
 * keeps its amplitude from octave to octave, a self-affine surface of Hurst exponent 1.
 */
export const ALBEDO_OCTAVE_SHARE = Math.SQRT1_2;

/** The crater scars' octaves, each a lattice of its own, the coarsest two levels above the noise's. */
export const SCAR_OCTAVES = 4;

/** How many levels coarser than {@link decorationLevel} the coarsest scar lattice lies. */
export const SCAR_LEVEL_OFFSET = 2;

/** How many levels finer than {@link decorationLevel} the ripples' lattice lies. */
export const RIPPLE_LEVEL_OFFSET = 3;

/**
 * The coarsest level of a body's decoration lattice: the least level m whose cells, of at most
 * {@link FACE_ARC_PER_S} a ÷ 2^m across, are at most half the band limit, so that its noise's
 * wavelengths, about two cells and shorter, lie below it; at least {@link SCAR_LEVEL_OFFSET}.
 *
 * @param equatorialRadiusM - The body's equatorial radius a, m, its largest.
 * @param bandLimitM - R05's band limit, m (`bandLimitM()`; never redefined here).
 * @throws RangeError where a lattice the decoration reads would pass {@link MAX_LATTICE_LEVEL}.
 */
export function decorationLevel(equatorialRadiusM: number, bandLimitM: number): number {
  const level = Math.max(
    SCAR_LEVEL_OFFSET,
    Math.ceil(Math.log2((FACE_ARC_PER_S * equatorialRadiusM) / (bandLimitM / 2))),
  );
  const finest = level + Math.max(DECORATION_OCTAVES - 1, RIPPLE_LEVEL_OFFSET);
  if (!(finest <= MAX_LATTICE_LEVEL)) {
    throw new RangeError(`a body of radius ${equatorialRadiusM} m needs a lattice past level 31`);
  }
  return level;
}

/**
 * The largest cell, m, of the lattice at `level` on a body of equatorial radius a, anywhere on a
 * face: {@link FACE_ARC_PER_S} a ÷ 2^level.
 */
export function largestCellM(equatorialRadiusM: number, level: number): number {
  return (FACE_ARC_PER_S * equatorialRadiusM) / 2 ** level;
}

/** S2's quadratic warp, s ∈ [0, 1] to u ∈ [−1, 1] (`patchVertex.wgsl`'s `stToUv`). */
function stToUv(s: number): number {
  if (s >= 0.5) {
    return (4 * s * s - 1) / 3;
  }
  const r = 1 - s;
  return (1 - 4 * r * r) / 3;
}

/** du/ds of the warp at s. */
function warpRate(s: number): number {
  return s >= 0.5 ? (8 * s) / 3 : (8 * (1 - s)) / 3;
}

/**
 * The widths, m, along a face's x and y, of a cell of the lattice at `level` at the face's cell
 * coordinates (s, t) on a body of radius `radiusM`: du/ds √(1 + v²) ÷ (1 + u² + v²) and
 * dv/dt √(1 + u²) ÷ (1 + u² + v²) times the radius ÷ 2^level; `decoration_cell_m`.
 */
export function latticeCellM(
  s: number,
  t: number,
  radiusM: number,
  level: number,
): readonly [number, number] {
  const u = stToUv(s);
  const v = stToUv(t);
  const q = 1 + u * u + v * v;
  const cells = radiusM / 2 ** level;
  return [
    (warpRate(s) * Math.sqrt(1 + v * v) * cells) / q,
    (warpRate(t) * Math.sqrt(1 + u * u) * cells) / q,
  ];
}

/**
 * A view's pixels per radian at unit distance, px/rad: W ÷ (2 tan(fov_h ÷ 2)), by
 * which a length L at a distance d spans L × px/rad ÷ d px, as `select.ts`' `screenSpaceErrorPx`
 * reckons it. A still takes its own from `stillPlan.ts`' `footprintView`.
 */
export function decorationPxPerRad(view: {
  readonly viewport: { readonly widthPx: number };
  readonly fovXRad: number;
}): number {
  return view.viewport.widthPx / (2 * Math.tan(view.fovXRad / 2));
}

/**
 * An octave's weight at a texel (Design note 3): 1 while its cell `spacingM` spans 2 px or more at
 * `distanceM`, 0 at 1 px or less, and log₂ of its span between, so that it fades across one octave
 * of pixel footprint; 1 at no distance; `decoration_fade`.
 */
export function octaveFade(spacingM: number, distanceM: number, pxPerRad: number): number {
  if (!(distanceM > 0)) {
    return 1;
  }
  const spanPx = (spacingM * pxPerRad) / distanceM;
  return Math.min(1, Math.max(0, Math.log2(spanPx)));
}

/** The eight unit gradients of the gradient noise, by a hash's three bits. */
const GRADIENTS: ReadonlyArray<readonly [number, number]> = [
  [1, 0],
  [Math.SQRT1_2, Math.SQRT1_2],
  [0, 1],
  [-Math.SQRT1_2, Math.SQRT1_2],
  [-1, 0],
  [-Math.SQRT1_2, -Math.SQRT1_2],
  [0, -1],
  [Math.SQRT1_2, -Math.SQRT1_2],
];

/** A gradient of {@link GRADIENTS} by three bits of `hash`, from bit `shift`. */
function gradientOf(hash: number, shift: number): readonly [number, number] {
  const gradient = GRADIENTS[(hash >>> shift) & 7];
  if (gradient === undefined) {
    throw new Error("three bits index eight gradients");
  }
  return gradient;
}

/** Perlin's quintic fade, 6t⁵ − 15t⁴ + 10t³ (Perlin 2002, "Improving noise", SIGGRAPH). */
function quintic(t: number): number {
  return t * t * t * (t * (6 * t - 15) + 10);
}

/** The quintic fade's rate, 30t²(t − 1)². */
function quinticRate(t: number): number {
  return 30 * t * t * (t - 1) * (t - 1);
}

/** Gradient noise's value and its rates along the lattice's x and y, per cell. */
export interface NoiseSample {
  readonly value: number;
  readonly dx: number;
  readonly dy: number;
}

/** √2, which takes 2D gradient noise of unit gradients, whose extremes are ±√½, to ±1. */
const NOISE_SCALE = Math.SQRT2;

/**
 * Gradient noise at `place` (Perlin 2002, with the quintic fade; its rates analytic): each corner's
 * gradient g · (f − c) weighted by an even fade about the corner, so that each term is odd and of
 * zero integral over its support. One hash a corner gives both fields' gradients, the normal's
 * from bits 0–2 and the albedo's from bits 3–5 (`which`); `decoration_noise`.
 *
 * @param prefix - The octave's {@link latticePrefix} under {@link DECORATION_SALT}'s `noise`.
 * @param which - 0 for the normal's field, 3 for the albedo's: the gradient's first bit.
 */
export function gradientNoise(prefix: number, place: LatticePlace, which: 0 | 3): NoiseSample {
  const { x, y, fx, fy } = place;
  const g00 = gradientOf(latticePoint(prefix, x >>> 0, y >>> 0), which);
  const g10 = gradientOf(latticePoint(prefix, (x + 1) >>> 0, y >>> 0), which);
  const g01 = gradientOf(latticePoint(prefix, x >>> 0, (y + 1) >>> 0), which);
  const g11 = gradientOf(latticePoint(prefix, (x + 1) >>> 0, (y + 1) >>> 0), which);
  const a = g00[0] * fx + g00[1] * fy;
  const b = g10[0] * (fx - 1) + g10[1] * fy;
  const c = g01[0] * fx + g01[1] * (fy - 1);
  const d = g11[0] * (fx - 1) + g11[1] * (fy - 1);
  const u = quintic(fx);
  const v = quintic(fy);
  const du = quinticRate(fx);
  const dv = quinticRate(fy);
  const k = a - b - c + d;
  const value = a + u * (b - a) + v * (c - a) + u * v * k;
  const dx =
    g00[0] +
    u * (g10[0] - g00[0]) +
    v * (g01[0] - g00[0]) +
    u * v * (g00[0] - g10[0] - g01[0] + g11[0]) +
    du * (b - a + v * k);
  const dy =
    g00[1] +
    u * (g10[1] - g00[1]) +
    v * (g01[1] - g00[1]) +
    u * v * (g00[1] - g10[1] - g01[1] + g11[1]) +
    dv * (c - a + u * k);
  return { value: NOISE_SCALE * value, dx: NOISE_SCALE * dx, dy: NOISE_SCALE * dy };
}

/** How strongly a texel's material takes each of the decoration's fields (Design note 2). */
export interface DecorationMix {
  /** The slope each noise octave adds to the shading normal, its gradients' scale, rad. */
  readonly roughness: number;
  /** The coarsest noise octave's relative amplitude on the class albedo. */
  readonly albedo: number;
  /** The share of the texel whose ground takes ripples (`Sand`'s). */
  readonly ripples: number;
  /** The share of the texel whose ground keeps small craters. */
  readonly scars: number;
}

/** No decoration: the reference surface's, and a sea floor's, which the ocean draws. */
export const NO_DECORATION_MIX: DecorationMix = { roughness: 0, albedo: 0, ripples: 0, scars: 0 };

/**
 * Each material class's mix by its code (`MaterialClass`, 0 to 14); `decoration.wgsl`'s
 * `decoration_class`. A lean (2026-11-02), unsourced: presentation amplitudes R12 may tune, a
 * rock's normal rougher than a fine grain's, ripples on `Sand` alone, the scars where small craters
 * last (rock, regolith, dust and ice), none on `Seabed`, which the ocean draws. Every class's
 * photometric law already carries its unresolved roughness, so these stay a few degrees.
 */
export const DECORATION_CLASSES: ReadonlyArray<DecorationMix> = [
  { roughness: 0.06, albedo: 0.05, ripples: 0, scars: 1 }, // Bedrock
  { roughness: 0.03, albedo: 0.04, ripples: 0, scars: 1 }, // Regolith
  { roughness: 0.03, albedo: 0.05, ripples: 0, scars: 0 }, // Soil
  { roughness: 0.015, albedo: 0.03, ripples: 1, scars: 0 }, // Sand
  { roughness: 0.02, albedo: 0.04, ripples: 0, scars: 0 }, // Sediment
  { roughness: 0.06, albedo: 0.05, ripples: 0, scars: 1 }, // VolcanicRock
  { roughness: 0.015, albedo: 0.02, ripples: 0, scars: 0 }, // Melt
  { roughness: 0.02, albedo: 0.02, ripples: 0, scars: 1 }, // Glacier
  { roughness: 0.015, albedo: 0.02, ripples: 0, scars: 0 }, // Snow
  { roughness: 0.025, albedo: 0.03, ripples: 0, scars: 0 }, // SeaIce
  { roughness: 0.015, albedo: 0.03, ripples: 0, scars: 1 }, // Frost
  { roughness: 0.02, albedo: 0.04, ripples: 0, scars: 0 }, // OrganicSediment
  NO_DECORATION_MIX, // Seabed
  { roughness: 0.02, albedo: 0.04, ripples: 0, scars: 0 }, // Evaporite
  { roughness: 0.02, albedo: 0.03, ripples: 0, scars: 1 }, // Dust
];

/**
 * The mix of a texel of classes `codes` at `shares` (summing to at most 1, the rest the reference
 * surface's, which takes none): each field the shares' sum of their classes'.
 */
export function mixOf(codes: ReadonlyArray<number>, shares: ReadonlyArray<number>): DecorationMix {
  let roughness = 0;
  let albedo = 0;
  let ripples = 0;
  let scars = 0;
  codes.forEach((code, k) => {
    const mix = DECORATION_CLASSES[code] ?? NO_DECORATION_MIX;
    const share = shares[k] ?? 0;
    roughness += share * mix.roughness;
    albedo += share * mix.albedo;
    ripples += share * mix.ripples;
    scars += share * mix.scars;
  });
  return { roughness, albedo, ripples, scars };
}

/**
 * Impact ripples' wavelength, m: Earth's are of decimetre wavelength, and Mars's small ripples,
 * read as impact ripples, 5–12 cm, beside its 1–5 m large ones (Lapôtre et al. 2016, Science 353,
 * 55, the accepted manuscript's pp. 3–4; checked 2026-11-13): one figure for every air, a lean.
 */
export const RIPPLE_WAVELENGTH_M = 0.1;

/**
 * Impact ripples' index, wavelength ÷ crest height: a mean of roughly 18 for sand ripples, against
 * about 15 for granule ripples, with large departures (Sharp 1963, "Wind ripples", J. Geol. 71,
 * 617, its abstract; checked 2026-11-13), so their steepest slope is π ÷ 18, about 10°.
 */
export const RIPPLE_INDEX = 18;

/** The integer length of a ripple's direction on its lattice, by which its angle is quantised. */
const RIPPLE_DIRECTION_LENGTH = 8;

/**
 * A ripple field's direction and wavelength on its lattice, in integers so that its phase is exact
 * at any cell: crests along (p x + q y) ≡ const (mod n), the wind along (p, q), the wavelength
 * n ÷ |(p, q)| cells. `decoration.wgsl` takes it as a `vec3i`, n = 0 for none.
 */
export interface RippleDirection {
  readonly p: number;
  readonly q: number;
  readonly n: number;
}

/**
 * The ripples of a patch whose wind, towards which the air moves, lies along (`alongS`, `alongT`)
 * of its face's grid, at a wavelength of `wavelengthCells` cells of the ripples' lattice: (p, q)
 * the wind's direction at a length of 8, n the nearest whole wavelength along it; `null` in a calm.
 */
export function rippleOf(
  alongS: number,
  alongT: number,
  wavelengthCells: number,
): RippleDirection | null {
  const length = Math.hypot(alongS, alongT);
  if (!(length > 0) || !(wavelengthCells > 0)) {
    return null;
  }
  const p = Math.round((RIPPLE_DIRECTION_LENGTH * alongS) / length);
  const q = Math.round((RIPPLE_DIRECTION_LENGTH * alongT) / length);
  const n = Math.max(1, Math.round(wavelengthCells * Math.hypot(p, q)));
  return { p, q, n };
}

/**
 * Trask's equilibrium crater density at 1 km, km⁻²: N(>D) = 0.079 D⁻² with D in km (Trask 1966,
 * JPL Technical Report 32-800, p. 263, eq. 1, as `hyperion_surface::craters::saturation` cites it).
 */
const SATURATION_PER_KM2_AT_1_KM = 0.079;

/**
 * The scars a scar lattice's cell holds at saturation: N(>c ÷ 4) − N(>c ÷ 2) of Trask's
 * equilibrium over a cell of side c, (16 − 4) × 0.079 = 0.948, whatever the cell's size, since the
 * law is scale-free; a scar lies wholly in its cell, so a cell holds one at most.
 */
export const SCARS_PER_CELL = 12 * SATURATION_PER_KM2_AT_1_KM;

/**
 * Each scar octave's share of saturation, coarsest first: the body's cumulative crater density at
 * the octave's middle diameter, 3c ÷ 8 for its cell c, over Trask's equilibrium there, in [0, 1]
 * (R09's `craters::cumulative_density` against `craters::saturation`, extended below the band
 * limit: Design note 20).
 *
 * @param cumulativePerKm2 - The body's N(>D), km⁻², at a diameter in metres; 0 where its air
 *   screens such craters or none is modelled.
 * @param coarsestCellM - The coarsest scar lattice's cell, m.
 */
export function scarShares(
  cumulativePerKm2: (diameterM: number) => number,
  coarsestCellM: number,
): readonly [number, number, number, number] {
  const share = (octave: number): number => {
    const diameterM = (3 * coarsestCellM) / 8 / 2 ** octave;
    const dKm = diameterM / 1_000;
    const saturation = SATURATION_PER_KM2_AT_1_KM / (dKm * dKm);
    const ratio = cumulativePerKm2(diameterM) / saturation;
    return Number.isFinite(ratio) ? Math.min(1, Math.max(0, ratio)) : 0;
  };
  return [share(0), share(1), share(2), share(3)];
}

/**
 * The least and greatest depth-to-diameter ratio of a scar (Design note 20), drawn towards the
 * mature end: 0.10, the mean of the Moon's random fresh small craters (Daubar et al. 2014, JGR
 * Planets 119, 2620, its abstract; checked 2026-11-13), and about 0.05 or less on mature ground
 * (Design note 20, after Stopar et al. 2017, Icarus 298, 34, whose abstract gives 0.11–0.17 for
 * the freshest under 400 m and states no mature figure; not re-checked in its text).
 */
export const SCAR_DEPTH_RATIO = { mature: 0.05, fresh: 0.1 } as const;

/** What decorates one texel: its patch and place, its body's, its view's and its material's. */
export interface DecorationTexelInput {
  /** The body's decoration seed ({@link decorationSeed}). */
  readonly seed: number;
  /** The body's {@link decorationLevel}. */
  readonly level: number;
  readonly key: PatchKey;
  /** The texel's place in its patch, [0, 1] from its low corner along u. */
  readonly u: number;
  /** The texel's place in its patch, [0, 1] from its low corner along v. */
  readonly v: number;
  /**
   * The widths, m, of a cell of the lattice at {@link DecorationTexelInput.level} at the texel,
   * along the face's x and y ({@link latticeCellM}).
   */
  readonly cellM: readonly [number, number];
  /** The texel's distance from the camera, m. */
  readonly distanceM: number;
  /** The view's {@link decorationPxPerRad}. */
  readonly pxPerRad: number;
  readonly mix: DecorationMix;
  /** The patch's ripples, or `null` for none. */
  readonly ripple: RippleDirection | null;
  /** Each scar octave's share of saturation, coarsest first ({@link scarShares}). */
  readonly scarShares: ReadonlyArray<number>;
}

/** What decoration does to a texel: a factor on its class albedo and a slope on its normal. */
export interface TexelDecoration {
  /** The factor on the class albedo, about 1. */
  readonly albedo: number;
  /** The slope added to the ground, ∂h ÷ ∂x and ∂h ÷ ∂y along the face's x and y. */
  readonly slope: readonly [number, number];
}

/** No decoration. */
export const UNDECORATED: TexelDecoration = { albedo: 1, slope: [0, 0] };

/** Bits 16–31 of a hash as a fraction in [0, 1). */
function upperFraction(hash: number): number {
  return (hash >>> 16) / 65_536;
}

/** Bits 0–15 of a hash as a fraction in [0, 1). */
function lowerFraction(hash: number): number {
  return (hash & 0xffff) / 65_536;
}

/**
 * One scar octave's slope at the texel, or none; `decoration_scar`. A scar is normal detail only
 * (open question 18's ruling, Design note 20): it changes no albedo.
 */
function scarAt(
  input: DecorationTexelInput,
  octave: number,
  share: number,
): readonly [number, number] | null {
  const level = input.level - SCAR_LEVEL_OFFSET + octave;
  const place = latticePlace(input.key, input.u, input.v, level);
  const first = latticeHash(
    input.seed,
    DECORATION_SALT.scar,
    input.key.face,
    level,
    place.x,
    place.y,
  );
  if (!(lowerFraction(first) < SCARS_PER_CELL * share)) {
    return null;
  }
  const second = lowbias32(first);
  const third = lowbias32(second);
  // D in cells over [¼, ½], drawn from N(>D) ∝ D⁻² by its inverse.
  const diameter = 1 / Math.sqrt(16 - 12 * upperFraction(first));
  const radius = diameter / 2;
  const cx = radius + (1 - diameter) * lowerFraction(second);
  const cy = radius + (1 - diameter) * upperFraction(second);
  const freshness = upperFraction(third) ** 3;
  const depthRatio =
    SCAR_DEPTH_RATIO.mature + (SCAR_DEPTH_RATIO.fresh - SCAR_DEPTH_RATIO.mature) * freshness;
  // Metres; the bowl round in metres and inside its cell along the narrower axis.
  const scale = 2 ** (input.level - level);
  const sx = input.cellM[0] * scale;
  const sy = input.cellM[1] * scale;
  const dx = (place.fx - cx) * sx;
  const dy = (place.fy - cy) * sy;
  const radiusM = radius * Math.min(sx, sy);
  const r = Math.hypot(dx, dy) / radiusM;
  if (!(r < 1)) {
    return null;
  }
  // H = −δ D (1 − ρ²)²: its slope 8 δ ρ (1 − ρ²) outwards, 0 at the rim and the centre.
  const slope = 8 * depthRatio * r * (1 - r * r);
  const towardsX = r > 0 ? dx / (r * radiusM) : 0;
  const towardsY = r > 0 ? dy / (r * radiusM) : 0;
  return [slope * towardsX, slope * towardsY];
}

/** The ripples' slope at the texel along the face's x and y; `decoration_ripple`. */
function rippleAt(input: DecorationTexelInput, ripple: RippleDirection): readonly [number, number] {
  const level = input.level + RIPPLE_LEVEL_OFFSET;
  const place = latticePlace(input.key, input.u, input.v, level);
  const { p, q, n } = ripple;
  // (p x + q y) mod n from each term's residue, as the shader's `u32` arithmetic forms it.
  const residue = (k: number): number => ((k % n) + n) % n;
  const whole = (residue(p) * (place.x % n) + residue(q) * (place.y % n)) % n;
  const phase = (2 * Math.PI * (whole + p * place.fx + q * place.fy)) / n;
  // Wavier crests: the phase wobbled by a noise two levels coarser, of a quarter turn.
  const coarse = level - 2;
  const wobble = gradientNoise(
    latticePrefix(input.seed, DECORATION_SALT.wobble, input.key.face, coarse),
    latticePlace(input.key, input.u, input.v, coarse),
    0,
  );
  const turn = Math.PI / 2;
  const scale = 2 ** (input.level - level);
  const sx = input.cellM[0] * scale;
  const sy = input.cellM[1] * scale;
  const rate = Math.cos(phase + turn * wobble.value);
  // h = A sin φ with A = λ ÷ (2 RI) and λ = n ÷ |(p, q)| cells: ∂h ÷ ∂x = A cos φ ∂φ ÷ ∂x.
  const amplitudeM = ((n / Math.hypot(p, q)) * Math.sqrt(sx * sy)) / (2 * RIPPLE_INDEX);
  const phiX = (2 * Math.PI * p) / n + (turn * wobble.dx) / 4;
  const phiY = (2 * Math.PI * q) / n + (turn * wobble.dy) / 4;
  return [(amplitudeM * rate * phiX) / sx, (amplitudeM * rate * phiY) / sy];
}

/**
 * The decoration of one texel (Design notes 2, 3 and 20): the noise octaves' roughness and colour
 * variation, the ripples and the crater scars, each faded by its own octave of footprint, summed
 * in that order; `decoration.wgsl`'s `decorate`, of which this is the reference to the shader's
 * `f32` rounding.
 */
export function decorateTexel(input: DecorationTexelInput): TexelDecoration {
  const { mix, distanceM, pxPerRad } = input;
  let albedo = 1;
  let slopeX = 0;
  let slopeY = 0;
  const meanCell = Math.sqrt(input.cellM[0] * input.cellM[1]);
  let albedoShare = 1;
  for (let octave = 0; octave < DECORATION_OCTAVES; octave += 1) {
    const level = input.level + octave;
    const step = 2 ** octave;
    const fade = octaveFade(meanCell / step, distanceM, pxPerRad);
    if (!(fade > 0)) {
      // Each finer octave fades sooner.
      break;
    }
    const prefix = latticePrefix(input.seed, DECORATION_SALT.noise, input.key.face, level);
    const place = latticePlace(input.key, input.u, input.v, level);
    const normal = gradientNoise(prefix, place, 0);
    const colour = gradientNoise(prefix, place, 3);
    // h = σ c n, c the octave's mean cell: its slope σ (c ÷ s) ∂n ÷ ∂x for each axis's width s.
    slopeX += (fade * mix.roughness * normal.dx * meanCell) / input.cellM[0];
    slopeY += (fade * mix.roughness * normal.dy * meanCell) / input.cellM[1];
    albedo += fade * mix.albedo * albedoShare * colour.value;
    albedoShare *= ALBEDO_OCTAVE_SHARE;
  }
  if (input.ripple !== null && input.ripple.n > 0 && mix.ripples > 0) {
    const fade = octaveFade(RIPPLE_WAVELENGTH_M / 2, distanceM, pxPerRad);
    if (fade > 0) {
      const [x, y] = rippleAt(input, input.ripple);
      slopeX += fade * mix.ripples * x;
      slopeY += fade * mix.ripples * y;
    }
  }
  if (mix.scars > 0) {
    for (let octave = 0; octave < SCAR_OCTAVES; octave += 1) {
      const cellM = (meanCell * 2 ** SCAR_LEVEL_OFFSET) / 2 ** octave;
      const fade = octaveFade(cellM / 4, distanceM, pxPerRad);
      if (!(fade > 0)) {
        break;
      }
      const share = input.scarShares[octave] ?? 0;
      const scar = share > 0 ? scarAt(input, octave, share) : null;
      if (scar !== null) {
        slopeX += fade * mix.scars * scar[0];
        slopeY += fade * mix.scars * scar[1];
      }
    }
  }
  return { albedo: Math.max(0, albedo), slope: [slopeX, slopeY] };
}

/** A 3-vector of the decoration's normal arithmetic. */
type Vec3Tuple = readonly [number, number, number];

/**
 * The shading normal `normal` perturbed by the slope (∂h ÷ ∂x, ∂h ÷ ∂y) along the unit tangents
 * `tangentX` and `tangentY` of the face's grid at the texel: n − sₓ tₓ − s_y t_y, normalised, the
 * normal of the height field h added to the ground; `decoration_normal`.
 */
export function perturbNormal(
  normal: Vec3Tuple,
  tangentX: Vec3Tuple,
  tangentY: Vec3Tuple,
  slope: readonly [number, number],
): Vec3Tuple {
  const x = normal[0] - slope[0] * tangentX[0] - slope[1] * tangentY[0];
  const y = normal[1] - slope[0] * tangentX[1] - slope[1] * tangentY[1];
  const z = normal[2] - slope[0] * tangentX[2] - slope[1] * tangentY[2];
  const length = Math.hypot(x, y, z);
  return [x / length, y / length, z / length];
}
