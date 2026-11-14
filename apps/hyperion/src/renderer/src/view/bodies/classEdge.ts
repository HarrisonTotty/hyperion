/**
 * The edges between a surface's classes as R07's disc and R10's terrain draw them (lane
 * fix-jagged-biome-edges; the owner's report of 2026-11-10, "the boundary line between biomes is
 * drawn very jagged"): a boundary given a natural, deterministic sub-texel shape by a noise of the
 * world's seed, its width the drawn lattice's and never a blur wider than one of its spacings.
 *
 * @remarks
 * - **The cause it answers.** The classifier decides a class from its coarse cell's record (the
 *   water's flag, the climate, the crust, the basin: `hyperion_surface`'s `Site`), so a boundary
 *   between two classes so decided runs along the field's cells, and the class map's texels and the
 *   terrain's vertices hold it as a staircase of them. Drawn bilinearly, that staircase is a soft
 *   ramp one lattice spacing wide whose middle steps with the cells: aliased jaggies where a cell is
 *   a few pixels, a blurred staircase where it is many.
 * - **The shape.** Where the lattice's points about a point disagree, each class's share s is
 *   sharpened about the others' by a noise of its own: its logit (1 + g (q − 1)) ln s + g q B n,
 *   the classes renormalised by their softmax, so that a boundary between two classes lies where
 *   ln(s₁ ÷ s₂) = B (n₂ − n₁), a natural line that wanders inside the lattice's blend and never past
 *   it, since a lattice point holding one class alone keeps it. The gate g ({@link edgeGate}) is 1
 *   across a boundary between lattice points of different classes and 0 where they agree, so that
 *   a texel's own mix of classes (a partial ice cover, a cliffed slope's share) is drawn as before.
 * - **The noise** n ({@link edgeNoise}) is a fractal sum of 3D gradient noise (Perlin 2002, its 16
 *   gradients and quintic fade) over the body-fixed unit direction, one octave a halving of the
 *   wavelength from {@link EDGE_BASE_WAVELENGTH_RAD}, a level-6 texel, and each class's own by its
 *   key ({@link edgeKey}: the world's seed and the class's form and entry), so that the disc and
 *   the terrain draw one class's edge alike at the hand-over. An octave is drawn only where its
 *   wavelength spans more than {@link EDGE_FADE_FROM_PX} pixels, faded in to
 *   {@link EDGE_FADE_TO_PX}: so nothing shimmers in motion, the line is stable under zoom, and it
 *   gains finer detail as the camera closes in.
 * - **The width.** The sharpness q ({@link edgeSharpness}) makes the line about
 *   {@link EDGE_WIDTH_PX} pixels wide, from the lattice's spacing on screen and the noise's own
 *   slope, never below 1 (the bilinear blend itself) and so never wider than one lattice spacing.
 *
 * The shader twin is `shaders/classEdge.wgsl`, operation for operation in `f32`; this module is its
 * twin in `f64`. Nothing here is generated: the weights are the module's, and the shaders only mix
 * the classes' photometric parameters by them, as Design note 3 rules.
 */

/** A body-fixed direction, x, y and z. */
type Direction = readonly [number, number, number];

/** The wavelength of the noise's first octave, rad: a level-6 texel's side at a face's centre, (π ÷ 2) ÷ 64. */
export const EDGE_BASE_WAVELENGTH_RAD = Math.PI / 128;

/** The noise's octaves: from a level-6 texel to 2⁻¹⁰ of one, 150 m on an Earth-sized body. */
export const EDGE_OCTAVES = 11;

/**
 * Each octave's amplitude against the one before: 2^(−¾), a noise of Hurst exponent H = ¾, under
 * which a boundary's fractal dimension is 2 − H = 1.25, the west coast of Britain's (Mandelbrot
 * 1967, Science 156, 636, doi:10.1126/science.156.3775.636), as the surface crate's class edges'
 * warp takes it (`material::edges`' `WARP_GAIN`).
 */
export const EDGE_OCTAVE_GAIN = 0.5946035575013605;

/** The pixels below which an octave's wavelength is not drawn. */
export const EDGE_FADE_FROM_PX = 2;

/** The pixels from which an octave's wavelength is drawn whole. */
export const EDGE_FADE_TO_PX = 4;

/** The noise's weight B in a class's logit, per standard deviation of its sum. */
export const EDGE_NOISE_WEIGHT = 1.1;

/** The width of a drawn edge, px: its logit's 10%–90% rise. */
export const EDGE_WIDTH_PX = 1.25;

/** The largest sharpness q, at which a lattice spacing of 100 px or more draws its edge whole. */
export const EDGE_SHARPNESS_MAX = 64;

/**
 * The root mean square of one octave's gradient noise ({@link gradientNoise}) over space, measured
 * over 2¹⁸ points and pinned (the tests re-measure it over 2¹⁶ to 2%).
 */
export const EDGE_NOISE_RMS = 0.2702;

/**
 * The root mean square of one component of an octave's gradient over space, per lattice unit,
 * measured as {@link EDGE_NOISE_RMS} is.
 */
export const EDGE_NOISE_SLOPE_RMS = 0.694;

/** The logit's rise between 10% and 90% of a share, 2 ln 9. */
const RISE_LOGIT = 2 * Math.log(9);

/** The logit's slope, per lattice spacing, of a bilinear share at its midpoint, 1 ÷ (½ · ½). */
const MIDPOINT_SLOPE = 4;

/** Each octave's lattice offset, so that no two octaves' lattices meet at the origin. */
const OCTAVE_OFFSET: Direction = [0.618034, 0.414214, 0.732051];

/** A 32-bit word's avalanche (Wellons' `lowbias32`), `>>> 0` to stay unsigned. */
function mix32(x: number): number {
  let h = x >>> 0;
  h ^= h >>> 16;
  h = Math.imul(h, 0x7feb352d) >>> 0;
  h ^= h >>> 15;
  h = Math.imul(h, 0x846ca68b) >>> 0;
  h ^= h >>> 16;
  return h >>> 0;
}

/** The hash of lattice corner (i, j, k) under `key`, a 32-bit word. */
export function edgeHash(i: number, j: number, k: number, key: number): number {
  let h = mix32(key ^ (i >>> 0));
  h = mix32(h ^ (j >>> 0));
  return mix32(h ^ (k >>> 0));
}

/** Perlin's (2002) gradient of index `h`'s low four bits, dotted with (x, y, z). */
function gradientDot(h: number, x: number, y: number, z: number): number {
  const g = h & 15;
  const u = g < 8 ? x : y;
  const v = g < 4 ? y : g === 12 || g === 14 ? x : z;
  return ((g & 1) === 0 ? u : -u) + ((g & 2) === 0 ? v : -v);
}

/** The quintic fade 6t⁵ − 15t⁴ + 10t³. */
function fade(t: number): number {
  return t * t * t * (t * (t * 6 - 15) + 10);
}

/** Linear interpolation from `a` to `b` by `t`. */
function lerp(a: number, b: number, t: number): number {
  return a + t * (b - a);
}

/**
 * Improved gradient noise (Perlin 2002) at `x`, lattice units, of `key`: each corner's gradient the
 * top four bits of its hash ({@link edgeHash}), about [−1, 1], its mean 0.
 */
export function gradientNoise(x: number, y: number, z: number, key: number): number {
  const i = Math.floor(x);
  const j = Math.floor(y);
  const k = Math.floor(z);
  const fx = x - i;
  const fy = y - j;
  const fz = z - k;
  const corner = (di: number, dj: number, dk: number): number =>
    gradientDot(edgeHash(i + di, j + dj, k + dk, key) >>> 28, fx - di, fy - dj, fz - dk);
  const u = fade(fx);
  const v = fade(fy);
  const w = fade(fz);
  return lerp(
    lerp(lerp(corner(0, 0, 0), corner(1, 0, 0), u), lerp(corner(0, 1, 0), corner(1, 1, 0), u), v),
    lerp(lerp(corner(0, 0, 1), corner(1, 0, 1), u), lerp(corner(0, 1, 1), corner(1, 1, 1), u), v),
    w,
  );
}

/** Octave `o`'s amplitude, gain^o. */
function octaveAmplitude(o: number): number {
  return EDGE_OCTAVE_GAIN ** o;
}

/** The standard deviation of the octaves' whole sum, every octave drawn. */
const NOISE_SIGMA =
  EDGE_NOISE_RMS *
  Math.sqrt(
    Array.from({ length: EDGE_OCTAVES }, (_, o) => octaveAmplitude(o) ** 2).reduce(
      (a, b) => a + b,
      0,
    ),
  );

/** The octaves a footprint draws: each one's weight, 0 to 1, and the sum's slope, per pixel. */
export interface EdgePlan {
  /** Each octave's weight, {@link EDGE_OCTAVES} of them. */
  readonly weights: ReadonlyArray<number>;
  /** The RMS slope of the normalised sum per pixel, one component. */
  readonly slopePerPx: number;
}

/** Hermite's smoothstep of `x` from `a` to `b`. */
function smoothstep(a: number, b: number, x: number): number {
  const t = Math.min(Math.max((x - a) / (b - a), 0), 1);
  return t * t * (3 - 2 * t);
}

/**
 * The octaves drawn at a pixel whose footprint on the unit sphere is `footprintRad`: octave o, of
 * wavelength λ₀ 2⁻ᵒ, weighted by smoothstep from {@link EDGE_FADE_FROM_PX} to {@link EDGE_FADE_TO_PX}
 * of its wavelength in pixels.
 */
export function edgePlan(footprintRad: number): EdgePlan {
  const weights: number[] = [];
  let slope = 0;
  for (let o = 0; o < EDGE_OCTAVES; o += 1) {
    const wavelengthPx = EDGE_BASE_WAVELENGTH_RAD / 2 ** o / footprintRad;
    const weight = smoothstep(EDGE_FADE_FROM_PX, EDGE_FADE_TO_PX, wavelengthPx);
    weights.push(weight);
    slope += (weight * octaveAmplitude(o) * EDGE_NOISE_SLOPE_RMS) / wavelengthPx;
  }
  return { weights, slopePerPx: slope / NOISE_SIGMA };
}

/**
 * The edge noise of `key` at the body-fixed unit direction `d` under `plan`: the octaves' weighted
 * sum over the whole sum's standard deviation, so about unit variance with every octave drawn.
 */
export function edgeNoise(d: Direction, key: number, plan: EdgePlan): number {
  let sum = 0;
  for (let o = 0; o < EDGE_OCTAVES; o += 1) {
    const weight = plan.weights[o] ?? 0;
    if (!(weight > 0)) {
      continue;
    }
    const frequency = 2 ** o / EDGE_BASE_WAVELENGTH_RAD;
    const octaveKey = mix32(key ^ Math.imul(o + 1, 0x9e3779b9));
    sum +=
      weight *
      octaveAmplitude(o) *
      gradientNoise(
        d[0] * frequency + o * OCTAVE_OFFSET[0],
        d[1] * frequency + o * OCTAVE_OFFSET[1],
        d[2] * frequency + o * OCTAVE_OFFSET[2],
        octaveKey,
      );
  }
  return sum / NOISE_SIGMA;
}

/**
 * The sharpness q of an edge whose lattice spacing spans `latticePx` pixels under `plan`: the logit
 * rises {@link RISE_LOGIT} over {@link EDGE_WIDTH_PX}, from the bilinear share's own slope 4 ÷ Δ and
 * the two classes' noises' (√2 B their sum's slope), held to 1 to {@link EDGE_SHARPNESS_MAX}.
 */
export function edgeSharpness(latticePx: number, plan: EdgePlan): number {
  const slope = MIDPOINT_SLOPE / latticePx + Math.SQRT2 * EDGE_NOISE_WEIGHT * plan.slopePerPx;
  const q = RISE_LOGIT / (EDGE_WIDTH_PX * slope);
  return Math.min(Math.max(q, 1), EDGE_SHARPNESS_MAX);
}

/**
 * The boundary's gate g from the lattice points' mean purity `meanPurity` (Σ b P(v) ÷ Σ b over the
 * reconstruction's terms, P(v) = Σ v² of a term's classes' shares renormalised) and the purity of
 * their mean `purity`: the share of the classes' impurity that is the points' disagreement, 0 where
 * every point holds the same mix and 1 across a boundary between points of one class each.
 */
export function edgeGate(meanPurity: number, purity: number): number {
  const impurity = 1 - purity;
  if (!(impurity > 1e-6)) {
    return 0;
  }
  return Math.min(Math.max((meanPurity - purity) / impurity, 0), 1);
}

/** The purity Σ (v ÷ Σ v)² of the classes' shares `shares` past law 0, 0 where they hold none. */
export function classPurity(shares: ArrayLike<number>): number {
  let total = 0;
  let squares = 0;
  for (let m = 1; m < shares.length; m += 1) {
    const s = shares[m] ?? 0;
    total += s;
    squares += s * s;
  }
  return total > 0 ? squares / (total * total) : 0;
}

/** Where and how finely a reconstruction's shares are drawn. */
export interface EdgeSite {
  /** The point's body-fixed unit direction on R05's cube sphere. */
  readonly direction: Direction;
  /** A pixel's footprint there on the unit sphere, rad. */
  readonly footprintRad: number;
  /** The lattice's spacing there on the unit sphere, rad. */
  readonly latticeRad: number;
  /** The lattice points' mean purity ({@link edgeGate}). */
  readonly meanPurity: number;
  /**
   * The purity of the classes' mix at the point where the shares merge some classes into law 0, as
   * the terrain's do its slots of no named substance (`terrainMaterial.ts`'
   * `terrainEdgeShapedShares`); absent, the shares' own ({@link classPurity}).
   */
  readonly mixPurity?: number;
}

/**
 * The laws' shares `shares` (law 0, then a class a law) with their classes' edges shaped (the
 * module's remarks): each class of a key of its own (`keys[m − 1]`, 0 for none) and a
 * share above 0 sharpened about the others by its noise, their sum kept; law 0 and a class of no
 * key keep their shares. Unchanged where fewer than two keyed classes hold a share, or where the
 * gate is 0.
 */
export function edgeShapedShares(
  shares: ReadonlyArray<number>,
  keys: ReadonlyArray<number>,
  site: EdgeSite,
): number[] {
  const out = [...shares];
  let keyed = 0;
  let kept = 0;
  for (let m = 1; m < shares.length; m += 1) {
    if ((keys[m - 1] ?? 0) !== 0 && (shares[m] ?? 0) > 0) {
      keyed += 1;
      kept += shares[m] ?? 0;
    }
  }
  if (keyed < 2) {
    return out;
  }
  const gate = edgeGate(site.meanPurity, site.mixPurity ?? classPurity(shares));
  if (!(gate > 0)) {
    return out;
  }
  const plan = edgePlan(site.footprintRad);
  const q = edgeSharpness(site.latticeRad / site.footprintRad, plan);
  const logits = new Map<number, number>();
  let largest = -Infinity;
  for (let m = 1; m < shares.length; m += 1) {
    const key = keys[m - 1] ?? 0;
    const s = shares[m] ?? 0;
    if (key === 0 || !(s > 0)) {
      continue;
    }
    const noise = edgeNoise(site.direction, key, plan);
    const z = (1 + gate * (q - 1)) * Math.log(s / kept) + gate * q * EDGE_NOISE_WEIGHT * noise;
    logits.set(m, z);
    largest = Math.max(largest, z);
  }
  let sum = 0;
  for (const z of logits.values()) {
    sum += Math.exp(z - largest);
  }
  for (const [m, z] of logits) {
    out[m] = (kept * Math.exp(z - largest)) / sum;
  }
  return out;
}

/** The 24 bits a key keeps, so that an `f32` holds it exactly. */
const KEY_BITS = 0xffffff;

/**
 * The edge key of a class of form code `form` and palette entry `entry` (both bytes, as the module
 * names a material) on a world of edge seed `seed` ({@link edgeSeedOf}): a nonzero 24-bit word, so
 * that one class's noise is the same wherever its world draws it.
 */
export function edgeKey(seed: number, form: number, entry: number): number {
  const h = mix32(mix32(seed ^ 0x5bd1e995) ^ (((form & 0xff) << 8) | (entry & 0xff))) & KEY_BITS;
  return h === 0 ? 1 : h;
}

/**
 * A world's edge seed from its detail seed (`DetailSeedHex`, 16 hexadecimal digits): its two 32-bit
 * halves mixed, so that each world's edges differ.
 *
 * @throws Error for a seed that is not 16 hexadecimal digits.
 */
export function edgeSeedOf(detailSeed: string): number {
  if (!/^[0-9a-f]{16}$/iu.test(detailSeed)) {
    throw new Error(`a detail seed is 16 hexadecimal digits, not ${detailSeed}`);
  }
  const high = Number.parseInt(detailSeed.slice(0, 8), 16);
  const low = Number.parseInt(detailSeed.slice(8), 16);
  return mix32(mix32(high) ^ low);
}

/**
 * The datum's μ below which a pixel's footprint stops growing (`classEdge.wgsl`'s
 * `EDGE_GRAZING_MU`), so that the limb's foreshortened pixels take a finite one.
 */
export const EDGE_GRAZING_MU = 0.2;
