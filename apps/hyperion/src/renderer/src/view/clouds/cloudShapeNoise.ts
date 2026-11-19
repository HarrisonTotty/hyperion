/**
 * The cloud volume's shape noises (plan R11, R11.T7.c; Design notes 10 and 21): Schneider and Vos
 * 2015's three tileable textures (Nubis, "The real-time volumetric cloudscapes of Horizon Zero
 * Dawn", SIGGRAPH Advances in Real-Time Rendering): a 128³ base noise of the clouds' billows, a 32³
 * detail noise that erodes their edges, and a 128² curl noise that turns the detail's coordinates,
 * each one channel of `r8unorm`, as Frostbite packed them (Hillaire 2016, "Physically based sky,
 * atmosphere and cloud rendering in Frostbite", §5.2): 2 MiB, 32 KiB and 16 KiB.
 *
 * @remarks
 * - **The base noise** is Nubis's Perlin–Worley shape: a Perlin fbm (Perlin 2002's improved
 *   gradient noise at 4, 8 and 16 cells a tile, each octave half the last's amplitude) dilated by a
 *   Worley fbm (Worley 1996's inverted F1 at 4, 8 and 16 cells, weighed 0.625, 0.25 and 0.125, as
 *   Nubis weighs its three channels), PW = W + P (1 − W), then remapped by the next Worley fbm, at
 *   8, 16 and 32 cells, as Nubis's shader remaps its base by its fbm: (PW − (W₂ − 1)) ÷ (2 − W₂).
 * - **The detail noise** is a Worley fbm at 2, 4 and 8 cells a tile, the same weights.
 * - **The curl noise** stores a potential ψ, a two-dimensional Perlin fbm at 4 and 8 cells, whose
 *   curl (∂ψ ÷ ∂y, −∂ψ ÷ ∂x) the shader takes by central differences (Bridson et al. 2007's
 *   curl-noise, divergence-free by construction): one channel, where an `rg8` curl would be two.
 * - **Each is equalised**: every texel replaced by its rank among the tile's over the count, so that
 *   its values are uniform on [0, 1] (`cloudNoise.ts`' rank transform, a fine histogram), whatever
 *   the fbm's own distribution; the erosion's mean (`cloudShape.ts`'
 *   `cloudShapeNormalisation`) then reads a uniform marginal.
 * - **One set a world**, generated from the world's cloud seed (`cloudDensity.ts`' `cloudSeedOf`),
 *   never authored, so that no two worlds' clouds share their billows; the gradients' and the
 *   feature points' hashes are `cloudNoise.ts`' `latticeHash`.
 * - **Deterministic and cheap.** Only `+ − × ÷`, `Math.floor` and `Math.sqrt` reach the texels, so
 *   every platform makes the same bytes. {@link CloudShapeNoiseBuilder} makes them a z slice of one
 *   field's octave a step, so that their making spreads over frames.
 */
import { latticeHash } from "./cloudNoise";

/** Texels along the base noise's edge. */
export const BASE_NOISE_SIZE = 128;

/** Texels along the detail noise's edge. */
export const DETAIL_NOISE_SIZE = 32;

/** Texels along the curl noise's edge. */
export const CURL_NOISE_SIZE = 128;

/** The base noise's Perlin octaves' cells a tile, each half the last's amplitude. */
const BASE_PERLIN_CELLS = [4, 8, 16] as const;

/** The base noise's dilating Worley octaves' cells a tile. */
const BASE_WORLEY_CELLS = [4, 8, 16] as const;

/** The base noise's remapping Worley octaves' cells a tile. */
const BASE_REMAP_CELLS = [8, 16, 32] as const;

/** The detail noise's Worley octaves' cells a tile. */
const DETAIL_WORLEY_CELLS = [2, 4, 8] as const;

/** The curl potential's Perlin octaves' cells a tile, each half the last's amplitude. */
const CURL_PERLIN_CELLS = [4, 8] as const;

/** Nubis's weights of its three Worley octaves. */
const WORLEY_WEIGHTS = [0.625, 0.25, 0.125] as const;

/** Each noise's own stream, folded into the world's seed so that its lattices differ. */
const STREAMS = {
  basePerlin: 0x1b87_3593,
  baseWorley: 0x2c1b_3c6d,
  baseRemap: 0x297a_2d39,
  detail: 0x5ed3_c26f,
  curl: 0x6a09_e667,
} as const;

/** Perlin 2002's twelve gradients, the cube's edge midpoints (`cloudNoise.ts`'). */
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

/** A stream's seed under a world's. */
function streamSeed(seed: number, stream: number, octave: number): number {
  return latticeHash(stream, octave, 0, seed);
}

/** Perlin 2002's fade, 6t⁵ − 15t⁴ + 10t³. */
function fade(t: number): number {
  return t * t * t * (t * (t * 6 - 15) + 10);
}

function wrap(i: number, n: number): number {
  return ((i % n) + n) % n;
}

/** A periodic lattice's gradients, three a point, point (i, j, k) at 3((k n + j) n + i). */
function latticeGradients(cells: number, seed: number, depth: number): Float64Array {
  const out = new Float64Array(3 * cells * cells * depth);
  for (let k = 0; k < depth; k += 1) {
    for (let j = 0; j < cells; j += 1) {
      for (let i = 0; i < cells; i += 1) {
        const g = GRADIENTS[latticeHash(i, j, k, seed) % GRADIENTS.length] ?? [0, 0, 0];
        out.set(g, 3 * ((k * cells + j) * cells + i));
      }
    }
  }
  return out;
}

/**
 * Improved gradient noise at (x, y, z), in cells, over a lattice periodic with `cells` on x and y
 * and with `depth` on z (1 for a plane, whose z is 0).
 */
function gradientNoise(
  g: Float64Array,
  cells: number,
  depth: number,
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
  const u = fade(fx);
  const v = fade(fy);
  const w = fade(fz);
  const i0 = wrap(xi, cells);
  const i1 = wrap(xi + 1, cells);
  const j0 = wrap(yi, cells) * cells;
  const j1 = wrap(yi + 1, cells) * cells;
  const k0 = wrap(zi, depth) * cells * cells;
  const k1 = wrap(zi + 1, depth) * cells * cells;
  const corner = (at: number, dx: number, dy: number, dz: number): number =>
    (g[3 * at] ?? 0) * dx + (g[3 * at + 1] ?? 0) * dy + (g[3 * at + 2] ?? 0) * dz;
  const x00 =
    corner(k0 + j0 + i0, fx, fy, fz) +
    u * (corner(k0 + j0 + i1, fx - 1, fy, fz) - corner(k0 + j0 + i0, fx, fy, fz));
  const x10 =
    corner(k0 + j1 + i0, fx, fy - 1, fz) +
    u * (corner(k0 + j1 + i1, fx - 1, fy - 1, fz) - corner(k0 + j1 + i0, fx, fy - 1, fz));
  const x01 =
    corner(k1 + j0 + i0, fx, fy, fz - 1) +
    u * (corner(k1 + j0 + i1, fx - 1, fy, fz - 1) - corner(k1 + j0 + i0, fx, fy, fz - 1));
  const x11 =
    corner(k1 + j1 + i0, fx, fy - 1, fz - 1) +
    u * (corner(k1 + j1 + i1, fx - 1, fy - 1, fz - 1) - corner(k1 + j1 + i0, fx, fy - 1, fz - 1));
  const y0 = x00 + v * (x10 - x00);
  const y1 = x01 + v * (x11 - x01);
  return y0 + w * (y1 - y0);
}

/**
 * A periodic Worley lattice's feature points, one a cell, three coordinates a cell in cells, cell
 * (i, j, k) at 3((k n + j) n + i): the cell's corner plus three uniform hashes.
 */
function featurePoints(cells: number, seed: number): Float64Array {
  const out = new Float64Array(3 * cells * cells * cells);
  for (let k = 0; k < cells; k += 1) {
    for (let j = 0; j < cells; j += 1) {
      for (let i = 0; i < cells; i += 1) {
        const at = 3 * ((k * cells + j) * cells + i);
        out[at] = i + latticeHash(i, j, k, seed) / 4_294_967_296;
        out[at + 1] = j + latticeHash(i, j, k, (seed ^ 0x9e37_79b9) >>> 0) / 4_294_967_296;
        out[at + 2] = k + latticeHash(i, j, k, (seed ^ 0x7f4a_7c15) >>> 0) / 4_294_967_296;
      }
    }
  }
  return out;
}

/**
 * Adds `weight` × (1 − F1) of a periodic Worley lattice of `cells` a tile to slice `z` of a
 * `size`³ field, F1 the distance in cells to the nearest feature point, held to 1 (Worley 1996,
 * inverted, as Nubis takes it): each texel's 27 neighbouring cells, gathered once a cell.
 */
function addWorleySlice(
  out: Float32Array,
  size: number,
  z: number,
  points: Float64Array,
  cells: number,
  weight: number,
): void {
  const per = size / cells;
  const scale = cells / size;
  const lz = (z + 0.5) * scale;
  const ck = Math.floor(lz);
  const near = new Float64Array(81);
  const slice = z * size * size;
  for (let cj = 0; cj < cells; cj += 1) {
    for (let ci = 0; ci < cells; ci += 1) {
      let n = 0;
      for (let dk = -1; dk <= 1; dk += 1) {
        for (let dj = -1; dj <= 1; dj += 1) {
          for (let di = -1; di <= 1; di += 1) {
            const i = ci + di;
            const j = cj + dj;
            const k = ck + dk;
            const at = 3 * ((wrap(k, cells) * cells + wrap(j, cells)) * cells + wrap(i, cells));
            // The point where its cell stands beside this one, across the tile's wrap.
            near[n] = (points[at] ?? 0) + (i - wrap(i, cells));
            near[n + 1] = (points[at + 1] ?? 0) + (j - wrap(j, cells));
            near[n + 2] = (points[at + 2] ?? 0) + (k - wrap(k, cells));
            n += 3;
          }
        }
      }
      for (let y = cj * per; y < (cj + 1) * per; y += 1) {
        const ly = (y + 0.5) * scale;
        for (let x = ci * per; x < (ci + 1) * per; x += 1) {
          const lx = (x + 0.5) * scale;
          let best = Infinity;
          for (let p = 0; p < 81; p += 3) {
            const dx = (near[p] ?? 0) - lx;
            const dy = (near[p + 1] ?? 0) - ly;
            const dz = (near[p + 2] ?? 0) - lz;
            const d = dx * dx + dy * dy + dz * dz;
            if (d < best) {
              best = d;
            }
          }
          const t = slice + y * size + x;
          out[t] = (out[t] ?? 0) + weight * (1 - Math.min(Math.sqrt(best), 1));
        }
      }
    }
  }
}

/** Adds `weight` × a Perlin octave of `cells` a tile to slice `z` of a `size`² × `depth` field. */
function addPerlinSlice(
  out: Float32Array,
  size: number,
  z: number,
  gradients: Float64Array,
  cells: number,
  depth: number,
  weight: number,
): void {
  const scale = cells / size;
  const lz = depth === 1 ? 0 : (z + 0.5) * scale;
  const slice = z * size * size;
  for (let y = 0; y < size; y += 1) {
    const ly = (y + 0.5) * scale;
    for (let x = 0; x < size; x += 1) {
      const t = slice + y * size + x;
      out[t] =
        (out[t] ?? 0) + weight * gradientNoise(gradients, cells, depth, (x + 0.5) * scale, ly, lz);
    }
  }
}

/** The fine histogram's bins of the rank transform. */
const RANK_BINS = 65_536;

/**
 * A field's rank transform made a slice at a time: each value replaced by its rank among all of
 * them over their count, as a byte, uniform on [0, 1] (`cloudNoise.ts`' rank transform, a fine
 * histogram, each bin's count spread across it by a value's place in it). Its steps take the
 * range, then the histogram, then the bytes, each a slice of `sliceTexels`; the result is the
 * same however they are spread.
 */
class Equaliser {
  readonly #values: Float32Array;
  readonly #slice: number;
  readonly #counts = new Uint32Array(RANK_BINS);
  readonly #below = new Float64Array(RANK_BINS);
  readonly bytes: Uint8Array;
  #least = Infinity;
  #most = -Infinity;

  constructor(values: Float32Array, sliceTexels: number) {
    this.#values = values;
    this.#slice = sliceTexels;
    this.bytes = new Uint8Array(values.length);
  }

  /** The slices each of the three passes takes. */
  get slices(): number {
    return this.#values.length / this.#slice;
  }

  /** The range over slice `s`. */
  range(s: number): void {
    for (let t = s * this.#slice; t < (s + 1) * this.#slice; t += 1) {
      const v = this.#values[t] ?? 0;
      this.#least = Math.min(this.#least, v);
      this.#most = Math.max(this.#most, v);
    }
  }

  #width(): number {
    return (this.#most - this.#least) / RANK_BINS || 1;
  }

  #binOf(v: number, width: number): number {
    return Math.min(Math.max(Math.floor((v - this.#least) / width), 0), RANK_BINS - 1);
  }

  /** Slice `s`'s count into the histogram, and after the last the counts below each bin. */
  count(s: number): void {
    const width = this.#width();
    for (let t = s * this.#slice; t < (s + 1) * this.#slice; t += 1) {
      const b = this.#binOf(this.#values[t] ?? 0, width);
      this.#counts[b] = (this.#counts[b] ?? 0) + 1;
    }
    if (s === this.slices - 1) {
      let total = 0;
      for (let b = 0; b < RANK_BINS; b += 1) {
        this.#below[b] = total;
        total += this.#counts[b] ?? 0;
      }
    }
  }

  /** Slice `s`'s bytes. */
  map(s: number): void {
    const width = this.#width();
    const n = this.#values.length;
    for (let t = s * this.#slice; t < (s + 1) * this.#slice; t += 1) {
      const v = this.#values[t] ?? 0;
      const b = this.#binOf(v, width);
      const within = Math.min(Math.max((v - this.#least) / width - b, 0), 1);
      const rank = ((this.#below[b] ?? 0) + within * (this.#counts[b] ?? 0)) / n;
      this.bytes[t] = Math.min(Math.round(rank * 255), 255);
    }
  }
}

/** A world's three shape noises' texels, `r8unorm`, x fastest, then y, then z. */
export interface CloudShapeNoises {
  /** {@link BASE_NOISE_SIZE}³. */
  readonly base: Uint8Array;
  /** {@link DETAIL_NOISE_SIZE}³. */
  readonly detail: Uint8Array;
  /** {@link CURL_NOISE_SIZE}², the curl's potential. */
  readonly curl: Uint8Array;
}

/** One step of the making: a slice of one field's octave, or a field's equalisation. */
type Step = () => void;

/**
 * Makes a world's shape noises a step at a time (the module's remarks): each
 * {@link CloudShapeNoiseBuilder.step} adds one octave to one z slice of one field, or equalises a
 * finished field; the result is the same however the steps are spread.
 */
export class CloudShapeNoiseBuilder {
  readonly #steps: Step[] = [];
  #next = 0;
  #result: CloudShapeNoises | null = null;

  /** @param seed - The world's cloud seed, a `u32` (`cloudDensity.ts`' `cloudSeedOf`). */
  constructor(seed: number) {
    const b = BASE_NOISE_SIZE;
    const d = DETAIL_NOISE_SIZE;
    const c = CURL_NOISE_SIZE;
    const perlin = new Float32Array(b * b * b);
    const worley = new Float32Array(b * b * b);
    const remap = new Float32Array(b * b * b);
    const detail = new Float32Array(d * d * d);
    const curl = new Float32Array(c * c);
    for (const [o, cells] of BASE_PERLIN_CELLS.entries()) {
      const g = latticeGradients(cells, streamSeed(seed, STREAMS.basePerlin, o), cells);
      const weight = 0.5 ** o;
      for (let z = 0; z < b; z += 1) {
        this.#steps.push(() => {
          addPerlinSlice(perlin, b, z, g, cells, cells, weight);
        });
      }
    }
    const worleys = [
      [BASE_WORLEY_CELLS, STREAMS.baseWorley, worley],
      [BASE_REMAP_CELLS, STREAMS.baseRemap, remap],
    ] as const;
    for (const [octaves, stream, field] of worleys) {
      for (const [o, cells] of octaves.entries()) {
        const points = featurePoints(cells, streamSeed(seed, stream, o));
        for (let z = 0; z < b; z += 1) {
          this.#steps.push(() => {
            addWorleySlice(field, b, z, points, cells, WORLEY_WEIGHTS[o] ?? 0);
          });
        }
      }
    }
    const slice = b * b;
    for (let z = 0; z < b; z += 1) {
      this.#steps.push(() => {
        // Perlin's fbm into [0, 1] (its amplitude about 1.75 at most), dilated by the Worley fbm,
        // then remapped by the finer one (the module's remarks).
        for (let t = z * slice; t < (z + 1) * slice; t += 1) {
          const p = Math.min(Math.max(0.5 + (perlin[t] ?? 0) / 1.75, 0), 1);
          const w = worley[t] ?? 0;
          const pw = w + p * (1 - w);
          const w2 = remap[t] ?? 0;
          perlin[t] = (pw - (w2 - 1)) / (2 - w2);
        }
      });
    }
    const baseRanks = this.#equalise(perlin, slice);
    for (const [o, cells] of DETAIL_WORLEY_CELLS.entries()) {
      const points = featurePoints(cells, streamSeed(seed, STREAMS.detail, o));
      for (let z = 0; z < d; z += 1) {
        this.#steps.push(() => {
          addWorleySlice(detail, d, z, points, cells, WORLEY_WEIGHTS[o] ?? 0);
        });
      }
    }
    const detailRanks = this.#equalise(detail, d * d);
    for (const [o, cells] of CURL_PERLIN_CELLS.entries()) {
      const g = latticeGradients(cells, streamSeed(seed, STREAMS.curl, o), 1);
      this.#steps.push(() => {
        addPerlinSlice(curl, c, 0, g, cells, 1, 0.5 ** o);
      });
    }
    const curlRanks = this.#equalise(curl, c * 8);
    this.#steps.push(() => {
      this.#result = { base: baseRanks.bytes, detail: detailRanks.bytes, curl: curlRanks.bytes };
    });
  }

  /** Queues a field's rank transform, a slice a step through each of its three passes. */
  #equalise(values: Float32Array, sliceTexels: number): Equaliser {
    const equaliser = new Equaliser(values, sliceTexels);
    for (const pass of ["range", "count", "map"] as const) {
      for (let s = 0; s < equaliser.slices; s += 1) {
        this.#steps.push(() => {
          equaliser[pass](s);
        });
      }
    }
    return equaliser;
  }

  /** The steps the making takes in all. */
  get stepCount(): number {
    return this.#steps.length;
  }

  /** Does one step; the finished noises once the last is done, `null` before. */
  step(): CloudShapeNoises | null {
    const next = this.#steps[this.#next];
    if (next !== undefined) {
      next();
      this.#next += 1;
    }
    return this.#result;
  }
}

/** A world's shape noises made at once ({@link CloudShapeNoiseBuilder} run to its end). */
export function cloudShapeNoises(seed: number): CloudShapeNoises {
  const builder = new CloudShapeNoiseBuilder(seed);
  let made = builder.step();
  while (made === null) {
    made = builder.step();
  }
  return made;
}

/** A request to a shape-noise worker: a world's noises, by its cloud seed. */
export interface ShapeNoiseRequest {
  readonly kind: "make";
  readonly seed: number;
}

/** A shape-noise worker's answer: a world's noises, their buffers transferred. */
export interface ShapeNoiseReply {
  readonly kind: "made";
  readonly seed: number;
  readonly noises: CloudShapeNoises;
}

/** The part of a worker's scope {@link serveShapeNoises} uses, so that a test can stand in for it. */
export interface ShapeNoiseScope {
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<ShapeNoiseRequest>) => void,
  ): void;
  postMessage(reply: ShapeNoiseReply, transfer: Transferable[]): void;
}

/** Serves a worker's requests: each world's noises made at once and posted back, transferred. */
export function serveShapeNoises(scope: ShapeNoiseScope): void {
  scope.addEventListener("message", (event) => {
    const { seed } = event.data;
    const noises = cloudShapeNoises(seed);
    scope.postMessage({ kind: "made", seed, noises }, [
      noises.base.buffer,
      noises.detail.buffer,
      noises.curl.buffer,
    ]);
  });
}

/** The part of a `Worker` {@link CloudShapeNoiseSource} uses, so that tests can stand in for one. */
export interface ShapeNoiseWorkerLike {
  addEventListener(type: "message", listener: (event: MessageEvent<ShapeNoiseReply>) => void): void;
  postMessage(message: ShapeNoiseRequest, transfer: Transferable[]): void;
  terminate(): void;
}

/** The time a frame may spend on a world's noises where no worker makes them, ms. */
export const SHAPE_NOISE_STEP_BUDGET_MS = 2;

/**
 * A world's shape noises as frames ask for them: made by a worker off the page's thread where one
 * is given, else a step at a time within each frame's budget ({@link CloudShapeNoiseBuilder}); one
 * world's held at a time, the last asked for.
 */
export class CloudShapeNoiseSource {
  readonly #worker: ShapeNoiseWorkerLike | null;
  #seed: number | null = null;
  #made: CloudShapeNoises | null = null;
  #builder: CloudShapeNoiseBuilder | null = null;

  constructor(worker: ShapeNoiseWorkerLike | null) {
    this.#worker = worker;
    worker?.addEventListener("message", (event) => {
      if (event.data.seed === this.#seed) {
        this.#made = event.data.noises;
      }
    });
  }

  /**
   * World `seed`'s noises once made, `null` before: asking for a new world lets the last go and
   * starts its making, which a worker does at once and the builder within `budgetMs` of each call.
   */
  noisesOf(
    seed: number,
    nowMs: () => number,
    budgetMs = SHAPE_NOISE_STEP_BUDGET_MS,
  ): CloudShapeNoises | null {
    if (seed !== this.#seed) {
      this.#seed = seed;
      this.#made = null;
      this.#builder = null;
      if (this.#worker === null) {
        this.#builder = new CloudShapeNoiseBuilder(seed);
      } else {
        this.#worker.postMessage({ kind: "make", seed }, []);
      }
    }
    const builder = this.#builder;
    if (this.#made === null && builder !== null) {
      const start = nowMs();
      let made = builder.step();
      while (made === null && nowMs() - start < budgetMs) {
        made = builder.step();
      }
      if (made !== null) {
        this.#made = made;
        this.#builder = null;
      }
    }
    return this.#made;
  }

  /** World `seed`'s noises made at once, here, for a still's first sample. */
  noisesNow(seed: number): CloudShapeNoises {
    if (seed !== this.#seed || this.#made === null) {
      this.#seed = seed;
      this.#builder = null;
      this.#made = cloudShapeNoises(seed);
    }
    return this.#made;
  }

  /** Stops the worker. */
  dispose(): void {
    this.#worker?.terminate();
    this.#made = null;
    this.#builder = null;
    this.#seed = null;
  }
}
