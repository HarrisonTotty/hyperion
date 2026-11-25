/**
 * Decorative scatter's pebbles: the size–frequency law below the authoritative rocks, its lattice
 * and the draws of one lattice cell, the TypeScript reference that `pebbles.wgsl` mirrors (plan R11,
 * R11.T6.a; Design notes 2, 3, 4 and 7).
 *
 * @remarks
 * - **The split.** Every rock of {@link PEBBLE_LARGEST_DIAMETER_M} across or more is authoritative,
 *   placed by `hyperion-surface` (R11.T3) and drawn by R11.T4; the decoration owns only the narrower
 *   ones, so that no rock is in both (Design note 4). A pebble's diameter is drawn below the split
 *   and never reaches it, to the last `f32` ({@link pebbleDiameter}). "Across" is the law's
 *   diameter, the visible outline's, as the rocks' is: a pebble's long axis may pass it, as a rock's
 *   of 0.41 m does.
 * - **The law.** Golombek and Rapp 1997's, which the rocks take: at a rock abundance k the number
 *   of rocks per square metre per metre of diameter is n(D) = (4 k q ÷ π) e^(−qD) ÷ D², q(k) =
 *   1.79 + 0.152 ÷ k per metre (their eqs. 1–2, as `hyperion_surface::scatter::law` takes them),
 *   so that the count per size bin runs on across the split (R11.T6.b). Each palette slot's k is
 *   the rocks' own, `rock_abundance` of the slot's material's site at its patch's centre, from the
 *   worker (`WorkerFields::rock_abundance`, the bake's `rockAbundances`).
 * - **Proposals, then thinning.** Each octave's cells take a Poisson count of proposals at the
 *   intensity λ(D) = C ÷ D², C = (4 ÷ π)(1.79 k_max + 0.152) at the law's largest k
 *   ({@link MAX_ROCK_ABUNDANCE}), above n(D) at every k (1.79 k + 0.152 ≤ C π ÷ 4 and e^(−qD) ≤ 1),
 *   each with a diameter drawn from C ÷ D² on the octave ({@link pebbleDiameter}), and each kept
 *   with the probability n(D) ÷ λ(D) mixed over the ground's palette by its class shares
 *   ({@link pebbleAcceptance}), times the slope's filter ({@link slopeFilter}) and the setting's
 *   `scatterDensity`. The kept points are a Poisson process of the mixed law's intensity times the
 *   density (Lewis and Shedler 1979, "Simulation of nonhomogeneous Poisson processes by thinning",
 *   Naval Research Logistics Quarterly 26, 403–413), so that a density of ½ keeps half as many and
 *   0 none ({@link scatterDensity} scales the count linearly, R11.T6.b).
 * - **The lattice** (Design note 3): each octave a lattice of the cube sphere's own grid, level
 *   {@link pebbleLatticeLevels}, keyed by `decorationLattice.ts`' {@link latticeHash} of the body's
 *   decoration seed, a salt an octave, the face, the level and the cell, so that a pebble neither
 *   swims as the camera moves nor differs between two views or two patches of one place. Every
 *   number of a cell is a word of its hash ({@link pebbleWord}), from its index alone.
 * - **The falloff.** A pebble fades across one octave of pixel footprint as the decoration's
 *   octaves do (`octaveFade`), wholly drawn while its diameter spans {@link PEBBLE_FADE_PX} × 2 px
 *   or more and gone at {@link PEBBLE_FADE_PX}, by its radius: so the falloff follows the view's
 *   footprint and no pebble flickers below two pixels, the flash limit's guard (Design note 3).
 * - **Its shape** is one half-buried ellipsoid of the rocks' middle proportions
 *   ({@link PEBBLE_AXES}), turned by a yaw about the ground's normal.
 */

import { latticeHash, lowbias32, octaveFade } from "../decoration/decorationLattice";

/**
 * The diameter, m, from which every rock is authoritative and from which no pebble is drawn:
 * `hyperion_surface::scatter::AUTHORITATIVE_ROCK_DIAMETER`, 0.2 m ÷ 0.5, 0.4 m (the wasm module's
 * `authoritativeRockDiameterM()`, which `pebbleLaw.wasm.test.ts` holds it to).
 */
export const PEBBLE_LARGEST_DIAMETER_M = 0.4;

/** q(k)'s constant term, per metre (Golombek and Rapp 1997, eq. 2; `law.rs`' `Q_CONSTANT_PER_M`). */
export const Q_CONSTANT_PER_M = 1.79;

/** q(k)'s coefficient of 1 ÷ k, per metre (Golombek and Rapp 1997, eq. 2; `law.rs`'). */
export const Q_INVERSE_PER_M = 0.152;

/** The largest rock abundance the law is used at (`law.rs`' `MAX_ROCK_ABUNDANCE`, Design note 4). */
export const MAX_ROCK_ABUNDANCE = 0.4;

/**
 * The pebbles' octaves of diameter, m, each [low, high), from the split down to 1.25 cm (lean,
 * 2026-11-18): five, each half the last, the smallest at the top of the Wentworth scale's fine
 * pebbles (4–64 mm; Wentworth 1922, "A scale of grade and class terms for clastic sediments",
 * Journal of Geology 30, 377–392). Below it the count, which grows as 1 ÷ D, is left to R11.T2's
 * detail sets.
 */
export const PEBBLE_OCTAVES: ReadonlyArray<readonly [lowM: number, highM: number]> = [
  [0.2, 0.4],
  [0.1, 0.2],
  [0.05, 0.1],
  [0.025, 0.05],
  [0.0125, 0.025],
];

/** The proposals a lattice cell takes on average, about which each octave's lattice is set. */
export const PEBBLE_PROPOSALS_PER_CELL = 6;

/**
 * The most proposals one cell draws: its Poisson count's tail past it, at the most a cell takes on
 * average (twice {@link PEBBLE_PROPOSALS_PER_CELL} at a lattice's rounding), is below 10⁻¹².
 */
export const PEBBLE_MAX_PER_CELL = 48;

/** The words each proposal draws: its place along x and y, diameter, keep, yaw and tint. */
export const PEBBLE_WORDS_EACH = 6;

/**
 * The octaves' salts of {@link latticeHash}: 16 plus the octave, past the decoration's own
 * (`DECORATION_SALT`'s 1 to 3).
 */
export const PEBBLE_SALT = 16;

/**
 * A pebble's horizontal axial ratio b ÷ a: √2 ÷ 2, the fragment ratio a : b ≈ 2 : √2 about which
 * the rocks' is drawn (`hyperion_surface::scatter::ROCK_ASPECT`, 0.55 to 0.9; Design note 5).
 */
export const PEBBLE_ASPECT = Math.SQRT1_2;

/**
 * A pebble's exposed height over its diameter: 0.375, the middle of the rocks' 0.25 to 0.50
 * (`ROCK_HEIGHT_RATIO`), its centre at the ground and the half below it buried, as a rock's equator
 * stands at the ground (`scatter/shape.rs`).
 */
export const PEBBLE_HEIGHT_RATIO = 0.375;

/**
 * A pebble's semi-axes over its radius D ÷ 2: the ellipsoid whose outline at the ground covers
 * π D² ÷ 4, a b = D² ÷ 4 (`shape.rs`' at n = 2), at {@link PEBBLE_ASPECT}, so a = 1 ÷ √(b ÷ a) and
 * b = √(b ÷ a) radii, and its vertical semi-axis the exposed height, 2 h ÷ D radii.
 */
export const PEBBLE_AXES: readonly [number, number, number] = [
  1 / Math.sqrt(PEBBLE_ASPECT),
  Math.sqrt(PEBBLE_ASPECT),
  2 * PEBBLE_HEIGHT_RATIO,
];

/**
 * A pebble's tint, a factor on its ground's albedo drawn uniform on this range, of mean 1, so that
 * the scatter keeps the ground's mean colour (Design note 3's zero mean; lean, 2026-11-18).
 */
export const PEBBLE_TINT: readonly [number, number] = [0.75, 1.25];

/**
 * The slopes, degrees from the datum's normal, below which every pebble stays and above which none
 * does, the keep falling linearly in cos θ between: loose gravel stands up to its angle of repose,
 * about 34° to 40° for angular gravels (Al-Hashemi and Al-Amoudi 2018, "A review on the angle of
 * repose of granular materials", Powder Technology 330, 397–417; lean, 2026-11-18).
 */
export const PEBBLE_REPOSE_DEG: readonly [full: number, none: number] = [30, 40];

/**
 * The pixel footprint, px, at which a pebble's diameter is gone: it is wholly drawn from twice it,
 * faded by its radius across the octave between (lean, 2026-11-18; see the module's remarks).
 */
export const PEBBLE_FADE_PX = 2;

/** The most lattice levels below a patch's an octave's cells lie: 2¹⁰ cells along a patch's side. */
export const MAX_PEBBLE_CELL_LEVELS = 10;

/** C ÷ 1: (4 ÷ π)(1.79 k + 0.152) at k, per square metre (times metres, over D²). */
function lawCoefficient(k: number): number {
  return (4 / Math.PI) * (Q_CONSTANT_PER_M * k + Q_INVERSE_PER_M);
}

/** The proposals' coefficient C, per m (λ(D) = C ÷ D² per m² per m), at {@link MAX_ROCK_ABUNDANCE}. */
export const PEBBLE_PROPOSAL_COEFFICIENT = lawCoefficient(MAX_ROCK_ABUNDANCE);

/**
 * n(D), rocks per square metre per metre of diameter at the abundance `k`: (4 k q ÷ π) e^(−qD) ÷ D²,
 * 0 at k ≤ 0 (`law.rs`' `number_per_m2_per_m`).
 */
export function numberDensityPerM2PerM(k: number, diameterM: number): number {
  if (!(k > 0)) {
    return 0;
  }
  const q = Q_CONSTANT_PER_M + Q_INVERSE_PER_M / k;
  return (lawCoefficient(k) * Math.exp(-q * diameterM)) / (diameterM * diameterM);
}

/**
 * The proposals per square metre of octave `octave`: ∫ C ÷ D² dD over it, C (1 ÷ low − 1 ÷ high).
 *
 * @throws RangeError for an octave not of {@link PEBBLE_OCTAVES}.
 */
export function proposalsPerM2(octave: number): number {
  const range = PEBBLE_OCTAVES[octave];
  if (range === undefined) {
    throw new RangeError(`no pebble octave ${String(octave)}`);
  }
  return PEBBLE_PROPOSAL_COEFFICIENT * (1 / range[0] - 1 / range[1]);
}

/**
 * The lattice level of each octave's cells on a body (see the module's remarks): the finest
 * level's plus m, the least m ≤ {@link MAX_PEBBLE_CELL_LEVELS} at which a cell of a finest patch,
 * `finestPatchM` ÷ 2^m across, takes at most twice {@link PEBBLE_PROPOSALS_PER_CELL} on average.
 *
 * @param finestLevel - The body's finest patch level.
 * @param finestPatchM - A finest patch's edge, m (`finestPatchSizeM`).
 */
export function pebbleLatticeLevels(finestLevel: number, finestPatchM: number): number[] {
  return PEBBLE_OCTAVES.map((_, octave) => {
    const rate = proposalsPerM2(octave);
    let m = 0;
    while (
      m < MAX_PEBBLE_CELL_LEVELS &&
      rate * (finestPatchM / 2 ** m) ** 2 > 2 * PEBBLE_PROPOSALS_PER_CELL
    ) {
      m += 1;
    }
    return finestLevel + m;
  });
}

/**
 * Word `n` of a cell of hash `cell`: `lowbias32(cell ^ lowbias32(n))`, word 0 its count's and word
 * 1 + 6 j + m proposal j's m-th ({@link PEBBLE_WORDS_EACH}); `pebbles.wgsl`'s `pebble_word`.
 */
export function pebbleWord(cell: number, n: number): number {
  return lowbias32((cell ^ lowbias32(n >>> 0)) >>> 0);
}

/** A word's top 24 bits as a number in [0, 1), exact in `f32`; `pebble_unit`. */
export function pebbleUnit(word: number): number {
  return (word >>> 8) / 16_777_216;
}

/** The hash of cell (x, y) of octave `octave`'s lattice of `level` on `face` (see the remarks). */
export function pebbleCellHash(
  seed: number,
  octave: number,
  face: number,
  level: number,
  x: number,
  y: number,
): number {
  return latticeHash(seed, PEBBLE_SALT + octave, face, level, x, y);
}

/**
 * The Poisson count of mean `mean` at the uniform `u`, its inverse distribution by summed terms,
 * at most {@link PEBBLE_MAX_PER_CELL}; `pebble_count`, in `f32` as the kernel forms it.
 */
export function poissonCount(mean: number, u: number): number {
  const f = Math.fround;
  let term = f(Math.exp(-f(mean)));
  let total = term;
  let n = 0;
  while (u >= total && n < PEBBLE_MAX_PER_CELL) {
    n += 1;
    term = f(f(term * f(mean)) / n);
    total = f(total + term);
  }
  return n;
}

/** The largest `f32` below `x`, a positive normal `f32`. */
function f32Below(x: number): number {
  const view = new DataView(new ArrayBuffer(4));
  view.setFloat32(0, x, true);
  view.setUint32(0, view.getUint32(0, true) - 1, true);
  return view.getFloat32(0, true);
}

/**
 * The diameter, m, at the uniform `u` of octave `octave` of the proposals' law C ÷ D²: its inverse
 * distribution low high ÷ (high − u (high − low)), from low at u = 0, in `f32` as the kernel forms
 * it, and at most the largest `f32` below high, so that none reaches the octave's top, nor any the
 * split (`pebble_diameter`).
 *
 * @throws RangeError for an octave not of {@link PEBBLE_OCTAVES}.
 */
export function pebbleDiameter(octave: number, u: number): number {
  const range = PEBBLE_OCTAVES[octave];
  if (range === undefined) {
    throw new RangeError(`no pebble octave ${String(octave)}`);
  }
  const f = Math.fround;
  const low = f(range[0]);
  const high = f(range[1]);
  const d = f(f(low * high) / f(high - f(u * f(high - low))));
  return Math.min(d, f32Below(high));
}

/**
 * The share of a proposal of diameter `diameterM` kept at a ground of the laws' `shares` (law 0,
 * the body's uniform law, whose ground bears no pebbles, then each palette slot's) and the slots'
 * abundances `abundances`: Σ share n_k(D) ÷ λ(D), each slot's term (1.79 k + 0.152) e^(−q(k) D) ÷
 * (1.79 k_max + 0.152) and none at k ≤ 0, in [0, 1]; `pebble_acceptance`.
 */
export function pebbleAcceptance(
  shares: ArrayLike<number>,
  abundances: ArrayLike<number>,
  diameterM: number,
): number {
  let keep = 0;
  const top = Q_CONSTANT_PER_M * MAX_ROCK_ABUNDANCE + Q_INVERSE_PER_M;
  for (let s = 0; s < abundances.length; s += 1) {
    const k = Math.min(abundances[s] ?? 0, MAX_ROCK_ABUNDANCE);
    const share = shares[s + 1] ?? 0;
    if (!(k > 0) || !(share > 0)) {
      continue;
    }
    const q = Q_CONSTANT_PER_M + Q_INVERSE_PER_M / k;
    keep += (share * (Q_CONSTANT_PER_M * k + Q_INVERSE_PER_M) * Math.exp(-q * diameterM)) / top;
  }
  return Math.min(Math.max(keep, 0), 1);
}

/**
 * The slope's filter on a pebble at `cosSlope`, the cosine between the ground's normal and the
 * datum's: 1 up to {@link PEBBLE_REPOSE_DEG}'s first, 0 past its second, linear in the cosine
 * between; `pebble_slope_filter`.
 */
export function slopeFilter(cosSlope: number): number {
  const full = Math.cos((PEBBLE_REPOSE_DEG[0] * Math.PI) / 180);
  const none = Math.cos((PEBBLE_REPOSE_DEG[1] * Math.PI) / 180);
  return Math.min(Math.max((cosSlope - none) / (full - none), 0), 1);
}

/**
 * The share of a pebble of diameter `diameterM` drawn at `distanceM` by a view of `pxPerRad` px/rad
 * (see the module's remarks): `octaveFade` of its radius against {@link PEBBLE_FADE_PX} ÷ 2, 1 while
 * its diameter spans twice {@link PEBBLE_FADE_PX} or more and 0 at it; `pebble_fade`.
 */
export function pebbleFade(diameterM: number, distanceM: number, pxPerRad: number): number {
  return octaveFade(diameterM / PEBBLE_FADE_PX, distanceM, pxPerRad);
}

/** The scatter density a setting draws at, clamped to [0, 1] (`R11Ladder.scatterDensity`). */
export function scatterDensity(density: number): number {
  return Number.isFinite(density) ? Math.min(Math.max(density, 0), 1) : 0;
}

/** One proposal of a cell, drawn: where it falls, its diameter and its words. */
export interface PebbleProposal {
  /** Its place across the cell, [0, 1)². */
  readonly at: readonly [number, number];
  /** Its diameter, m, below its octave's top. */
  readonly diameterM: number;
  /** Its keep's uniform, kept where below the share kept ({@link pebbleAcceptance}). */
  readonly keep: number;
  /** Its yaw about the ground's normal, rad, [0, 2π). */
  readonly yawRad: number;
  /** Its tint, on {@link PEBBLE_TINT}. */
  readonly tint: number;
  /** Its tint's byte, 0 to 255, as the instance carries it. */
  readonly tintByte: number;
}

/**
 * The proposals of a cell of hash `cell` (`pebbleCellHash`) of octave `octave` holding `mean` on
 * average: the Poisson count of word 0, then each proposal's six words in order
 * ({@link PEBBLE_WORDS_EACH}); `pebbles.wgsl`'s loop over a cell.
 */
export function cellProposals(cell: number, octave: number, mean: number): PebbleProposal[] {
  const count = poissonCount(mean, pebbleUnit(pebbleWord(cell, 0)));
  const out: PebbleProposal[] = [];
  for (let j = 0; j < count; j += 1) {
    const word = (m: number): number => pebbleWord(cell, 1 + PEBBLE_WORDS_EACH * j + m);
    const tintByte = word(5) >>> 24;
    out.push({
      at: [pebbleUnit(word(0)), pebbleUnit(word(1))],
      diameterM: pebbleDiameter(octave, pebbleUnit(word(2))),
      keep: pebbleUnit(word(3)),
      yawRad: 2 * Math.PI * pebbleUnit(word(4)),
      tint: pebbleTint(tintByte),
      tintByte,
    });
  }
  return out;
}

/** The tint of a tint byte, 0 to 255, on {@link PEBBLE_TINT}: `pebbleDraw.wgsl`'s `pebble_tint`. */
export function pebbleTint(byte: number): number {
  return PEBBLE_TINT[0] + ((PEBBLE_TINT[1] - PEBBLE_TINT[0]) * byte) / 255;
}

/**
 * The pebbles a cell keeps on a ground of uniform `shares` and `abundances`, at `density`, slope
 * and fade aside: each proposal whose keep is below its acceptance times the density. The CPU
 * reference's core, over which R11.T6.b's counts are taken.
 */
export function cellPebbles(
  cell: number,
  octave: number,
  mean: number,
  shares: ArrayLike<number>,
  abundances: ArrayLike<number>,
  density: number,
): PebbleProposal[] {
  const d = scatterDensity(density);
  return cellProposals(cell, octave, mean).filter(
    (p) => p.keep < pebbleAcceptance(shares, abundances, p.diameterM) * d,
  );
}
