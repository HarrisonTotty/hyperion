/**
 * Bloom as veiling glare: the threshold, the level weights fitted to a view's glare spread
 * function, and the CPU twin of the GPU mip chain (plan R07, T14.a, Design note 12).
 *
 * @remarks
 * Only the light the display cannot show, L_high = L − min(L, T) with T at AgX's top of range, is
 * bloomed; the player's eye supplies the glare of what the display reproduces. The chain is
 * Jimenez 2014's 13-tap downsample and 3 × 3 tent upsample ("Next Generation Post Processing in
 * Call of Duty: Advanced Warfare", SIGGRAPH 2014) without the Karis average, which does not
 * conserve energy. Each mip level is weighted once on the way up, U_m = w_m D_m + up(U_{m+1}),
 * with non-negative weights summing to 1, so the chain conserves energy. The GPU chain
 * (`bloomDown.wgsl`, `bloomUp.wgsl`, R07.T14.b) follows this file sample for sample.
 */
import type { ViewRole } from "../camera/state";
import type { TargetRounding } from "../engine/status";
import { AGX_MAX_EV, HALF_FLOAT_MAX } from "../photometry/toneCurve";
import { glareSpreadFunction, type EyeObserver } from "./glare";
import type { QualitySetting } from "../quality/qualitySetting";
import { nnls } from "./nnls";

/**
 * The bloom threshold in exposed units: 2^`AGX_MAX_EV` = 16.29, AgX's top of range, which the
 * metered average sits about 156 times below (Design notes 9 and 12).
 */
export const BLOOM_THRESHOLD_EXPOSED = 2 ** AGX_MAX_EV;

/**
 * The bloom threshold in the HDR target's pre-exposed units.
 *
 * @param exposureScale - This frame's exposure scale, 1 ÷ (cd/m²) (R02's `exposureScale`).
 * @param preExposureScale - The scale the target was pre-exposed with, 1 ÷ (cd/m²).
 */
export function bloomThreshold(exposureScale: number, preExposureScale: number): number {
  return (BLOOM_THRESHOLD_EXPOSED * preExposureScale) / exposureScale;
}

/** The part of a pre-exposed value above the threshold, which alone is bloomed: L − min(L, T). */
export function bloomExcess(value: number, threshold: number): number {
  return value - Math.min(value, threshold);
}

/** Which mip levels a bloom chain weights. */
export interface BloomLevels {
  /** The first weighted mip level: 0 at full resolution, 1 at quarter resolution. */
  readonly firstLevel: number;
  /** The number of weighted mip levels. */
  readonly levels: number;
}

/** The chain's mip levels per setting: full resolution and 7 levels high, quarter and 5 low. */
export const BLOOM_LEVELS: Readonly<Record<QualitySetting, BloomLevels>> = {
  high: { firstLevel: 0, levels: 7 },
  low: { firstLevel: 1, levels: 5 },
};

/**
 * The bloom chain for one view: which mip levels it weights and with what (Design note 12).
 *
 * @remarks
 * `weights[k]` multiplies mip level `firstLevel + k`, level 0 being the view's internal resolution;
 * they are non-negative and sum to 1. The low setting starts at level 1, quarter resolution, with
 * fewer levels (Design note 18).
 */
export interface BloomKernel extends BloomLevels {
  /** One weight per weighted level, non-negative least squares, Σ = 1. */
  readonly weights: Float32Array;
}

/** One channel of an image on the chain, row-major, `widthPx × heightPx` values. */
export interface BloomImage {
  readonly widthPx: number;
  readonly heightPx: number;
  readonly values: Float64Array;
}

/** A new black image of the given size. */
export function bloomImage(widthPx: number, heightPx: number): BloomImage {
  return { widthPx, heightPx, values: new Float64Array(widthPx * heightPx) };
}

/**
 * A value as a colour-attachment write of `rgba16float` stores it: rounded to the nearest half
 * float, or toward zero where the adapter's probe says so (R01's `targetRounding`), and clamped to
 * the format's largest value. `unknown` is modelled as toward zero, the probed RTX 3080's and
 * Gen9's behaviour.
 */
export function roundToHalf(value: number, rounding: TargetRounding): number {
  if (!(value > 0)) {
    return value === 0 || Number.isNaN(value) ? value : -roundToHalf(-value, rounding);
  }
  if (value >= HALF_FLOAT_MAX) {
    return HALF_FLOAT_MAX;
  }
  const exponent = Math.max(Math.floor(Math.log2(value)), -14);
  const ulp = 2 ** (exponent - 10);
  const units = value / ulp;
  let rounded: number;
  switch (rounding) {
    case "nearest": {
      const floor = Math.floor(units);
      const rest = units - floor;
      const even = floor % 2 === 0;
      rounded = rest > 0.5 || (rest === 0.5 && !even) ? floor + 1 : floor;
      break;
    }
    case "toward-zero":
    case "unknown":
      rounded = Math.floor(units);
      break;
  }
  return Math.min(rounded * ulp, HALF_FLOAT_MAX);
}

/** A bilinear sample at continuous texel coordinates (texel n's centre at n + 0.5), clamped. */
function bilinear(image: BloomImage, x: number, y: number): number {
  const u = x - 0.5;
  const v = y - 0.5;
  const i0 = Math.floor(u);
  const j0 = Math.floor(v);
  const fu = u - i0;
  const fv = v - j0;
  const w = image.widthPx;
  const h = image.heightPx;
  const ia = Math.min(Math.max(i0, 0), w - 1);
  const ib = Math.min(Math.max(i0 + 1, 0), w - 1);
  const ja = Math.min(Math.max(j0, 0), h - 1);
  const jb = Math.min(Math.max(j0 + 1, 0), h - 1);
  const values = image.values;
  const rowA = ja * w;
  const rowB = jb * w;
  return (
    (1 - fv) * ((1 - fu) * (values[rowA + ia] ?? 0) + fu * (values[rowA + ib] ?? 0)) +
    fv * ((1 - fu) * (values[rowB + ia] ?? 0) + fu * (values[rowB + ib] ?? 0))
  );
}

/** The next mip level's size: ⌈n ÷ 2⌉. */
export function mipSize(sizePx: number): number {
  return Math.max(1, Math.ceil(sizePx / 2));
}

const TENT: ReadonlyArray<readonly [number, number]> = [
  [-1, 0.25],
  [0, 0.5],
  [1, 0.25],
];
const DOWN_OUTER: ReadonlyArray<readonly [number, number]> = [
  [-2, 0.25],
  [0, 0.5],
  [2, 0.25],
];
const DOWN_INNER: ReadonlyArray<number> = [-1, 1];

/**
 * Jimenez's 13-tap downsample to the next mip level, unrounded.
 *
 * @remarks
 * Each output texel sits over four source texels; its 13 bilinear taps, at offsets of 0, ±1 and ±2
 * source texels, are half a tent of the nine at 0 and ±2 (weights 1/8, 1/16, 1/32) and half a box of
 * the four at ±1 (1/8 each). Every tap falls between four texels, so each source texel feeds the
 * output with total weight 1/4, and energy is conserved.
 */
export function bloomDown(source: BloomImage): BloomImage {
  const out = bloomImage(mipSize(source.widthPx), mipSize(source.heightPx));
  const sx = source.widthPx / out.widthPx;
  const sy = source.heightPx / out.heightPx;
  for (let j = 0; j < out.heightPx; j += 1) {
    const cy = (j + 0.5) * sy;
    for (let i = 0; i < out.widthPx; i += 1) {
      const cx = (i + 0.5) * sx;
      let sum = 0;
      for (const [dy, wy] of DOWN_OUTER) {
        for (const [dx, wx] of DOWN_OUTER) {
          sum += 0.5 * wx * wy * bilinear(source, cx + dx, cy + dy);
        }
      }
      for (const dy of DOWN_INNER) {
        for (const dx of DOWN_INNER) {
          sum += 0.125 * bilinear(source, cx + dx, cy + dy);
        }
      }
      out.values[j * out.widthPx + i] = sum;
    }
  }
  return out;
}

/**
 * The 3 × 3 tent upsample of a level to the size of the level above, unrounded: nine bilinear taps
 * at 0 and ±1 coarse texels, weights (1, 2, 1) ÷ 4 on each axis.
 */
export function bloomUpTent(coarse: BloomImage, widthPx: number, heightPx: number): BloomImage {
  const out = bloomImage(widthPx, heightPx);
  const sx = coarse.widthPx / widthPx;
  const sy = coarse.heightPx / heightPx;
  for (let j = 0; j < heightPx; j += 1) {
    const cy = (j + 0.5) * sy;
    for (let i = 0; i < widthPx; i += 1) {
      const cx = (i + 0.5) * sx;
      let sum = 0;
      for (const [dy, wy] of TENT) {
        for (const [dx, wx] of TENT) {
          sum += wx * wy * bilinear(coarse, cx + dx, cy + dy);
        }
      }
      out.values[j * widthPx + i] = sum;
    }
  }
  return out;
}

function mapImage(image: BloomImage, f: (value: number, index: number) => number): BloomImage {
  const out = bloomImage(image.widthPx, image.heightPx);
  for (let k = 0; k < image.values.length; k += 1) {
    out.values[k] = f(image.values[k] ?? 0, k);
  }
  return out;
}

/** The weight of mip level `level` in `kernel`, zero outside its levels. */
export function levelWeight(kernel: BloomKernel, level: number): number {
  const k = level - kernel.firstLevel;
  return k >= 0 && k < kernel.levels ? (kernel.weights[k] ?? 0) : 0;
}

/**
 * The CPU twin of the GPU chain on one channel: the bloomed light at the input's resolution, for
 * an input that already holds only the excess above the threshold ({@link bloomExcess}).
 *
 * @remarks
 * Every intermediate level is a colour-attachment write and is rounded by `rounding`
 * ({@link roundToHalf}); the last step, w₀ D₀ + up(U₁), runs inside the tone-mapping pass in `f32`
 * and is not rounded. With `rounding` `null` the chain is exact, for the impulse responses.
 */
export function bloomChain(
  excess: BloomImage,
  kernel: BloomKernel,
  rounding: TargetRounding | null,
): BloomImage {
  const round = (value: number): number =>
    rounding === null ? value : roundToHalf(value, rounding);
  const last = kernel.firstLevel + kernel.levels - 1;
  const down: BloomImage[] = [excess];
  for (let m = 1; m <= last; m += 1) {
    const previous = down[m - 1];
    if (previous === undefined) {
      throw new Error("bloomChain: missing mip level");
    }
    down.push(mapImage(bloomDown(previous), round));
  }
  let up = mapImage(down[last] ?? excess, (v) => round(levelWeight(kernel, last) * v));
  for (let m = last - 1; m >= 0; m -= 1) {
    const level = down[m];
    if (level === undefined) {
      throw new Error("bloomChain: missing mip level");
    }
    const upsampled = bloomUpTent(up, level.widthPx, level.heightPx);
    const weight = levelWeight(kernel, m);
    up = mapImage(upsampled, (v, k) => {
      const sum = weight * (level.values[k] ?? 0) + v;
      return m === 0 ? sum : round(sum);
    });
  }
  return up;
}

/** The energy of an image at mip level 0: the sum of its values, each over one base pixel. */
export function bloomEnergy(image: BloomImage): number {
  let sum = 0;
  for (const v of image.values) {
    sum += v;
  }
  return sum;
}

/**
 * Encircled energy of an image about the centre of pixel (`ci`, `cj`): the sum of the values of
 * the pixels whose centres lie within each radius, px.
 */
export function encircledEnergy(
  image: BloomImage,
  ci: number,
  cj: number,
  radiiPx: ReadonlyArray<number>,
): number[] {
  const sums = radiiPx.map(() => 0);
  for (let j = 0; j < image.heightPx; j += 1) {
    for (let i = 0; i < image.widthPx; i += 1) {
      const d = Math.hypot(i - ci, j - cj);
      const v = image.values[j * image.widthPx + i] ?? 0;
      radiiPx.forEach((r, k) => {
        if (d <= r) {
          sums[k] = (sums[k] ?? 0) + v;
        }
      });
    }
  }
  return sums;
}

/** The radii, px, at which the kernel's encircled energy is fitted to the spread function's. */
export const FIT_RADII_PX: ReadonlyArray<number> = [
  0, 1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128,
];

/** Half the side of the window, in texels of level m, over which level m's response is formed. */
const RESPONSE_HALF_TEXELS = 5;

const responseCache = new Map<number, ReadonlyArray<number>>();

/**
 * The encircled energy at {@link FIT_RADII_PX} of mip level `level`'s unit impulse response at
 * level 0: a unit impulse taken down `level` times and tent-upsampled back, exactly.
 */
export function levelResponseEE(level: number): ReadonlyArray<number> {
  const known = responseCache.get(level);
  if (known !== undefined) {
    return known;
  }
  const side = 2 * RESPONSE_HALF_TEXELS * 2 ** level;
  const centre = side / 2;
  let image = bloomImage(side, side);
  image.values[centre * side + centre] = 1;
  const sizes: Array<readonly [number, number]> = [];
  for (let m = 0; m < level; m += 1) {
    sizes.push([image.widthPx, image.heightPx]);
    image = bloomDown(image);
  }
  for (let m = level - 1; m >= 0; m -= 1) {
    const [w, h] = sizes[m] ?? [0, 0];
    image = bloomUpTent(image, w, h);
  }
  const ee = encircledEnergy(image, centre, centre, FIT_RADII_PX);
  responseCache.set(level, ee);
  return ee;
}

/** The side, sub-samples, of the grid integrating the spread function over pixels near the centre. */
const PIXEL_SUBSAMPLES = 32;

/**
 * ∫ PSF dA over the disc of radius ½ px about the source, px² × sr⁻¹, by Simpson's rule in ln r:
 * the CIE function's core is far narrower than a pixel, so it is integrated radially rather than
 * sub-sampled.
 */
function centreDiscEnergy(psf: (dPx: number) => number): number {
  const intervals = 2000;
  const low = Math.log(1e-7);
  const high = Math.log(0.5);
  const step = (high - low) / intervals;
  let sum = 0;
  for (let i = 0; i <= intervals; i += 1) {
    const r = Math.exp(low + i * step);
    const weight = i === 0 || i === intervals ? 1 : i % 2 === 1 ? 4 : 2;
    sum += weight * psf(r) * 2 * Math.PI * r * r;
  }
  return (sum * step) / 3;
}

/**
 * The encircled energy at {@link FIT_RADII_PX} of a view's spread function as its pixels hold it:
 * each pixel's share integrated over its square (sub-sampled within 3 px of the centre; the centre
 * pixel's inscribed disc taken in closed form by radial quadrature), small-angle.
 */
export function spreadPixelEE(role: ViewRole, radPerPx: number, eye: EyeObserver): number[] {
  const spread = glareSpreadFunction(role, eye);
  const psf = (dPx: number): number => spread(dPx * radPerPx);
  const area = radPerPx * radPerPx;
  const disc = centreDiscEnergy(psf) * area;
  const maxR = FIT_RADII_PX[FIT_RADII_PX.length - 1] ?? 0;
  const reach = Math.ceil(maxR);
  const sums = FIT_RADII_PX.map(() => 0);
  for (let j = -reach; j <= reach; j += 1) {
    for (let i = -reach; i <= reach; i += 1) {
      const d = Math.hypot(i, j);
      if (d > maxR) {
        continue;
      }
      let energy = 0;
      if (d < 3) {
        const n = PIXEL_SUBSAMPLES;
        for (let b = 0; b < n; b += 1) {
          for (let a = 0; a < n; a += 1) {
            const x = i - 0.5 + (a + 0.5) / n;
            const y = j - 0.5 + (b + 0.5) / n;
            const r = Math.hypot(x, y);
            if (i === 0 && j === 0 && r < 0.5) {
              continue;
            }
            energy += psf(r);
          }
        }
        energy *= area / (n * n);
        if (i === 0 && j === 0) {
          energy += disc;
        }
      } else {
        energy = psf(d) * area;
      }
      FIT_RADII_PX.forEach((r, k) => {
        if (d <= r) {
          sums[k] = (sums[k] ?? 0) + energy;
        }
      });
    }
  }
  return sums;
}

/** The weight of the constraint Σ w = 1 against the relative encircled-energy residuals. */
const SUM_CONSTRAINT_WEIGHT = 100;

/**
 * The bloom kernel for a view: its levels from `setting`, and level weights fitted by non-negative
 * least squares to the view's spread function at its angular pixel scale (Design note 12).
 *
 * @remarks
 * The fit matches the chain's encircled energy, Σ_m w_m EE_m(r), to the spread function's as its
 * pixels hold it ({@link spreadPixelEE}) at {@link FIT_RADII_PX}, each residual relative to the
 * target, with Σ w = 1 held by a heavily weighted row and then exactly by renormalisation. The
 * spread function's energy beyond the chain's reach is gathered into its widest levels, so energy
 * is conserved. The eye observer is an argument, as for `glareSpread`, so that this builds before
 * R06; the call sites pass R06's `DEFAULT_EYE_OBSERVER`.
 *
 * @param radPerPx - The view's angular size of one internal-resolution pixel, rad.
 */
export function bloomKernel(
  setting: QualitySetting,
  role: ViewRole,
  radPerPx: number,
  eye: EyeObserver,
): BloomKernel {
  const { firstLevel, levels } = BLOOM_LEVELS[setting];
  const responses: Array<ReadonlyArray<number>> = [];
  for (let k = 0; k < levels; k += 1) {
    responses.push(levelResponseEE(firstLevel + k));
  }
  const target = spreadPixelEE(role, radPerPx, eye);
  const rows: number[][] = [];
  const b: number[] = [];
  FIT_RADII_PX.forEach((_, r) => {
    const t = target[r] ?? 0;
    if (t <= 0) {
      return;
    }
    rows.push(responses.map((ee) => (ee[r] ?? 0) / t));
    b.push(1);
  });
  rows.push(responses.map(() => SUM_CONSTRAINT_WEIGHT));
  b.push(SUM_CONSTRAINT_WEIGHT);
  const fitted = nnls(rows, b);
  const total = fitted.reduce((sum, w) => sum + w, 0);
  const weights = new Float32Array(levels);
  fitted.forEach((w, k) => {
    weights[k] = total > 0 ? w / total : k === levels - 1 ? 1 : 0;
  });
  return { levels, firstLevel, weights };
}
