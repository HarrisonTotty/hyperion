/**
 * The clouds' noise volume (plan R11, R11.T7.b; Design notes 10 and 21): one tileable 3D texture of
 * band-limited noise with a standard normal marginal, generated on the CPU, never authored, which
 * both settings' clouds sample at every octave of their structure.
 *
 * @remarks
 * - **What it holds.** Two gradient-noise lattices (Perlin 2002's improved noise, its quintic fade
 *   and its twelve edge gradients), each of {@link CLOUD_NOISE_CELLS} cells along a tile's edge and
 *   wrapped at the tile, the second offset half a cell on every axis so that the two lattices'
 *   zeros do not coincide, summed and then Gaussianised: every texel replaced by Φ⁻¹ of its rank among all the tile's
 *   ({@link CloudNoiseBuilder}), so that its values are a standard normal's quantiles. A threshold on a sum of independent octaves of it then covers the share the normal's tail
 *   says, which is how the clouds' coverage is met (`cloudDensity.ts`).
 * - **Its storage.** `r8unorm`, byte b standing for n = 8b ÷ 255 − 4 (Design note 10's single
 *   channel, as Frostbite's), 64³ texels, 256 KiB, filtered trilinearly with `repeat`.
 * - **One for every body.** A body's clouds differ by the rotation and offsets its own seed gives
 *   their lattice coordinates (`cloudDensity.ts`), so the texture is the same for every world and
 *   made once an engine; it is generated from {@link CLOUD_NOISE_SEED} by this module, so no image
 *   ships (Design note 21).
 * - **The twin.** {@link sampleCloudNoise} filters as the GPU does, texel centres at (i + ½) ÷ 64,
 *   so the CPU reference of the density reads what the shaders read, to the GPU's filtering
 *   precision.
 * - The texels are a function of the constants alone; the builder spreads their making over
 *   frames (83 ms at once in a pod's Vitest, 2026-11-02).
 */
import type { Vec3 } from "../../geometry/vec3";
import { log } from "../../lib/pinnedMath";

/** Texels along the noise tile's edge. */
export const CLOUD_NOISE_SIZE = 64;

/** Gradient-noise lattice cells along the tile's edge: a feature about 8 texels across. */
export const CLOUD_NOISE_CELLS = 8;

/** The magnitude the stored values span: n in [−4, 4]. */
export const CLOUD_NOISE_RANGE = 4;

/** The generator's seed, a constant, so that every client makes the same tile. */
export const CLOUD_NOISE_SEED = 0x7a11_c10d;

/** The texel count of the tile. */
const TEXELS = CLOUD_NOISE_SIZE * CLOUD_NOISE_SIZE * CLOUD_NOISE_SIZE;

/** An integer hash of a lattice point and a seed, as a `u32` (a murmur-style finaliser). */
export function latticeHash(i: number, j: number, k: number, seed: number): number {
  let h =
    (seed ^ Math.imul(i, 0x8da6_b343) ^ Math.imul(j, 0xd816_3841) ^ Math.imul(k, 0xcb1a_b31f)) >>>
    0;
  h = Math.imul(h ^ (h >>> 16), 0x7feb_352d) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x846c_a68b) >>> 0;
  return (h ^ (h >>> 16)) >>> 0;
}

/** Perlin 2002's twelve gradients, the cube's edge midpoints. */
const GRADIENTS: ReadonlyArray<readonly [number, number, number]> = [
  [1, 1, 0],
  [-1, 1, 0],
  [1, -1, 0],
  [-1, -1, 0],
  [1, 0, 1],
  [-1, 0, 1],
  [1, 0, -1],
  [-1, 0, -1],
  [0, 1, 1],
  [0, -1, 1],
  [0, 1, -1],
  [0, -1, -1],
];

/** Perlin 2002's fade, 6t⁵ − 15t⁴ + 10t³. */
function fade(t: number): number {
  return t * t * t * (t * (t * 6 - 15) + 10);
}

function wrap(i: number, n: number): number {
  return ((i % n) + n) % n;
}

/**
 * A lattice's gradients, three components a point, point (i, j, k) of `cells`³ at 3((k cells + j)
 * cells + i): each one of {@link GRADIENTS} by its hash.
 */
function latticeGradients(cells: number, seed: number): Float64Array {
  const gradients = new Float64Array(3 * cells * cells * cells);
  for (let k = 0; k < cells; k += 1) {
    for (let j = 0; j < cells; j += 1) {
      for (let i = 0; i < cells; i += 1) {
        const g = GRADIENTS[latticeHash(i, j, k, seed) % GRADIENTS.length] ?? [0, 0, 0];
        gradients.set(g, 3 * ((k * cells + j) * cells + i));
      }
    }
  }
  return gradients;
}

/**
 * Improved gradient noise at `(x, y, z)` in lattice cells over `gradients`' lattice, periodic with
 * `cells` on every axis.
 */
function gradientNoise(
  gradients: Float64Array,
  cells: number,
  x: number,
  y: number,
  z: number,
): number {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const zi = Math.floor(z);
  const fx = x - xi;
  const fy = y - yi;
  const fz = z - zi;
  let result = 0;
  for (let dk = 0; dk < 2; dk += 1) {
    const wz = dk === 0 ? 1 - fade(fz) : fade(fz);
    const k = wrap(zi + dk, cells);
    for (let dj = 0; dj < 2; dj += 1) {
      const wy = dj === 0 ? 1 - fade(fy) : fade(fy);
      const j = wrap(yi + dj, cells);
      for (let di = 0; di < 2; di += 1) {
        const wx = di === 0 ? 1 - fade(fx) : fade(fx);
        const at = 3 * ((k * cells + j) * cells + wrap(xi + di, cells));
        const dot =
          (gradients[at] ?? 0) * (fx - di) +
          (gradients[at + 1] ?? 0) * (fy - dj) +
          (gradients[at + 2] ?? 0) * (fz - dk);
        result += wx * wy * wz * dot;
      }
    }
  }
  return result;
}

/**
 * Φ⁻¹, the standard normal's quantile function, by Acklam's rational approximation (relative error
 * under 1.15 × 10⁻⁹; P. J. Acklam, "An algorithm for computing the inverse normal cumulative
 * distribution function", 2003, from memory), refined by nothing further: the byte it is stored in
 * resolves 0.031.
 *
 * @param p - In (0, 1).
 */
export function inverseNormal(p: number): number {
  const a = [
    -39.696_830_286_653_76, 220.946_098_424_520_5, -275.928_510_446_968_7, 138.357_751_867_269,
    -30.664_798_066_147_16, 2.506_628_277_459_239,
  ];
  const b = [
    -54.476_098_798_224_06, 161.585_836_858_040_9, -155.698_979_859_886_6, 66.801_311_887_719_72,
    -13.280_681_552_885_72,
  ];
  const c = [
    -0.007_784_894_002_430_293, -0.322_396_458_041_136_5, -2.400_758_277_161_838,
    -2.549_732_539_343_734, 4.374_664_141_464_968, 2.938_163_982_698_783,
  ];
  const d = [
    0.007_784_695_709_041_462, 0.322_467_129_070_039_8, 2.445_134_137_142_996,
    3.754_408_661_907_416,
  ];
  const [a0 = 0, a1 = 0, a2 = 0, a3 = 0, a4 = 0, a5 = 0] = a;
  const [b0 = 0, b1 = 0, b2 = 0, b3 = 0, b4 = 0] = b;
  const [c0 = 0, c1 = 0, c2 = 0, c3 = 0, c4 = 0, c5 = 0] = c;
  const [d0 = 0, d1 = 0, d2 = 0, d3 = 0] = d;
  const low = 0.024_25;
  if (p < low) {
    const q = Math.sqrt(-2 * log(p));
    return (
      (((((c0 * q + c1) * q + c2) * q + c3) * q + c4) * q + c5) /
      ((((d0 * q + d1) * q + d2) * q + d3) * q + 1)
    );
  }
  if (p > 1 - low) {
    const q = Math.sqrt(-2 * log(1 - p));
    return (
      -(((((c0 * q + c1) * q + c2) * q + c3) * q + c4) * q + c5) /
      ((((d0 * q + d1) * q + d2) * q + d3) * q + 1)
    );
  }
  const q = p - 0.5;
  const r = q * q;
  return (
    ((((((a0 * r + a1) * r + a2) * r + a3) * r + a4) * r + a5) * q) /
    (((((b0 * r + b1) * r + b2) * r + b3) * r + b4) * r + 1)
  );
}

/** The byte that stores `n`, clamped to ±{@link CLOUD_NOISE_RANGE}. */
function encode(n: number): number {
  const unit =
    (Math.min(Math.max(n, -CLOUD_NOISE_RANGE), CLOUD_NOISE_RANGE) + CLOUD_NOISE_RANGE) /
    (2 * CLOUD_NOISE_RANGE);
  return Math.round(unit * 255);
}

/** The value a stored byte stands for. */
export function decodeCloudNoise(byte: number): number {
  return (byte / 255) * 2 * CLOUD_NOISE_RANGE - CLOUD_NOISE_RANGE;
}

/** The fine histogram's bins of the rank transform. */
const RANK_BINS = 65_536;

/**
 * Makes the noise tile a z slice at a time (the module's remarks), so that its making can be spread
 * over frames within their budget: each {@link CloudNoiseBuilder.step} does one slice of the raw
 * noise, then one of the rank transform, until the tile is done.
 *
 * @remarks
 * The rank transform is by a fine histogram: each texel takes Φ⁻¹ of its rank, its bin's count
 * spread evenly across the bin by its place in it, which is a sort's rank to within a bin's count
 * (2¹⁸ texels in 2¹⁶ bins), far under the byte's 255 levels. The result is the same however the
 * steps are spread.
 */
export class CloudNoiseBuilder {
  readonly #raw = new Float64Array(TEXELS);
  readonly #first = latticeGradients(CLOUD_NOISE_CELLS, CLOUD_NOISE_SEED);
  readonly #second = latticeGradients(CLOUD_NOISE_CELLS, (CLOUD_NOISE_SEED ^ 0x5bd1_e995) >>> 0);
  #slice = 0;
  #least = Infinity;
  #most = -Infinity;
  #counts: Uint32Array | null = null;
  #below: Float64Array | null = null;
  #texels: Uint8Array | null = null;
  #done = false;

  /** The finished tile, or `null` while slices remain. */
  get texels(): Uint8Array | null {
    return this.#done ? this.#texels : null;
  }

  /**
   * Does one slice of the work; the finished tile once the last is done, `null` before.
   */
  step(): Uint8Array | null {
    if (this.#done) {
      return this.#texels;
    }
    if (this.#slice < CLOUD_NOISE_SIZE) {
      this.#rawSlice(this.#slice);
    } else {
      this.#mapSlice(this.#slice - CLOUD_NOISE_SIZE);
    }
    this.#slice += 1;
    if (this.#slice === 2 * CLOUD_NOISE_SIZE) {
      this.#done = true;
    }
    return this.texels;
  }

  #rawSlice(z: number): void {
    const scale = CLOUD_NOISE_CELLS / CLOUD_NOISE_SIZE;
    for (let y = 0; y < CLOUD_NOISE_SIZE; y += 1) {
      for (let x = 0; x < CLOUD_NOISE_SIZE; x += 1) {
        const lx = (x + 0.5) * scale;
        const ly = (y + 0.5) * scale;
        const lz = (z + 0.5) * scale;
        const value =
          gradientNoise(this.#first, CLOUD_NOISE_CELLS, lx, ly, lz) +
          gradientNoise(this.#second, CLOUD_NOISE_CELLS, lx + 0.5, ly + 0.5, lz + 0.5);
        this.#raw[(z * CLOUD_NOISE_SIZE + y) * CLOUD_NOISE_SIZE + x] = value;
        this.#least = Math.min(this.#least, value);
        this.#most = Math.max(this.#most, value);
      }
    }
  }

  #binOf(value: number): number {
    const width = (this.#most - this.#least) / RANK_BINS;
    return Math.min(Math.floor((value - this.#least) / width), RANK_BINS - 1);
  }

  #mapSlice(z: number): void {
    let { counts, below, texels } = {
      counts: this.#counts,
      below: this.#below,
      texels: this.#texels,
    };
    if (counts === null || below === null || texels === null) {
      counts = new Uint32Array(RANK_BINS);
      for (const value of this.#raw) {
        const b = this.#binOf(value);
        counts[b] = (counts[b] ?? 0) + 1;
      }
      below = new Float64Array(RANK_BINS);
      let total = 0;
      for (let b = 0; b < RANK_BINS; b += 1) {
        below[b] = total;
        total += counts[b] ?? 0;
      }
      texels = new Uint8Array(TEXELS);
      this.#counts = counts;
      this.#below = below;
      this.#texels = texels;
    }
    const width = (this.#most - this.#least) / RANK_BINS;
    const sliceTexels = CLOUD_NOISE_SIZE * CLOUD_NOISE_SIZE;
    for (let t = z * sliceTexels; t < (z + 1) * sliceTexels; t += 1) {
      const value = this.#raw[t] ?? 0;
      const b = this.#binOf(value);
      const within = Math.min(Math.max((value - this.#least) / width - b, 0), 1);
      const rank = (below[b] ?? 0) + within * (counts[b] ?? 0);
      texels[t] = encode(inverseNormal(Math.min(Math.max(rank, 0.5), TEXELS - 0.5) / TEXELS));
    }
  }
}

/**
 * The noise tile's texels, `r8unorm`, x fastest then y then z, as `writeTexture` takes a 3D texture
 * (the module's remarks): {@link CloudNoiseBuilder} run to its end at once.
 */
export function cloudNoiseTexels(): Uint8Array {
  const builder = new CloudNoiseBuilder();
  let texels = builder.step();
  while (texels === null) {
    texels = builder.step();
  }
  return texels;
}

/**
 * The noise at `u`, in tiles (one tile per unit, `repeat`), filtered trilinearly between texel
 * centres at (i + ½) ÷ 64 as a GPU's linear sampler filters them; the decoded value, whose standard
 * deviation over the tile is {@link CLOUD_NOISE_STD}.
 */
export function sampleCloudNoise(texels: Uint8Array, u: Vec3): number {
  const n = CLOUD_NOISE_SIZE;
  const tx = u.x * n - 0.5;
  const ty = u.y * n - 0.5;
  const tz = u.z * n - 0.5;
  const x0 = Math.floor(tx);
  const y0 = Math.floor(ty);
  const z0 = Math.floor(tz);
  const fx = tx - x0;
  const fy = ty - y0;
  const fz = tz - z0;
  const at = (dx: number, dy: number, dz: number): number =>
    decodeCloudNoise(texels[(wrap(z0 + dz, n) * n + wrap(y0 + dy, n)) * n + wrap(x0 + dx, n)] ?? 0);
  const x00 = at(0, 0, 0) + fx * (at(1, 0, 0) - at(0, 0, 0));
  const x10 = at(0, 1, 0) + fx * (at(1, 1, 0) - at(0, 1, 0));
  const x01 = at(0, 0, 1) + fx * (at(1, 0, 1) - at(0, 0, 1));
  const x11 = at(0, 1, 1) + fx * (at(1, 1, 1) - at(0, 1, 1));
  const y0v = x00 + fy * (x10 - x00);
  const y1v = x01 + fy * (x11 - x01);
  return y0v + fz * (y1v - y0v);
}

/**
 * The standard deviation of {@link sampleCloudNoise} over the tile, by which each octave's sample
 * is divided so that it has unit variance: below 1 only by the trilinear filter's smoothing between
 * the texels' standard normal values, measured over 2¹⁶ points of a fixed sequence and held to it
 * by `cloudNoise.test.ts`.
 */
export const CLOUD_NOISE_STD = 0.974;
