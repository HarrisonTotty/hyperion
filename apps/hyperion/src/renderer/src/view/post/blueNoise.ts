/**
 * A static blue-noise tile for the tone-mapping pass's dither (plan R07, T15, Design note 13), made
 * once by the void-and-cluster method and never animated, so that the guide's flash limit holds.
 *
 * @remarks
 * Ulichney 1993, "The void-and-cluster method for dither array generation" (Proc. SPIE 1913): a
 * toroidal Gaussian energy (σ = 1.5 px) picks, from a seeded initial pattern, the tightest cluster
 * to rank downward and the largest void to rank upward. Deterministic: the initial pattern comes
 * from a fixed integer hash, and ties go to the lowest index.
 */

/** The tile's side, px. */
export const BLUE_NOISE_SIDE = 64;

/** The energy's Gaussian width, px (Ulichney's 1.5). */
const SIGMA_PX = 1.5;

/** A fixed 32-bit integer hash (Wang's), for the initial pattern. */
function hash(value: number): number {
  let x = value >>> 0;
  x = (x ^ 61 ^ (x >>> 16)) >>> 0;
  x = Math.imul(x, 9) >>> 0;
  x = (x ^ (x >>> 4)) >>> 0;
  x = Math.imul(x, 0x27d4eb2d) >>> 0;
  return (x ^ (x >>> 15)) >>> 0;
}

/**
 * The tile's thresholds, row-major: each texel's rank over the texel count, at the rank's centre,
 * (rank + 0.5) ÷ side², so uniform over (0, 1).
 */
export function blueNoiseTile(side: number = BLUE_NOISE_SIDE): Float32Array {
  const n = side * side;
  const kernel = new Float64Array(n);
  for (let y = 0; y < side; y += 1) {
    for (let x = 0; x < side; x += 1) {
      const dx = Math.min(x, side - x);
      const dy = Math.min(y, side - y);
      kernel[y * side + x] = Math.exp(-(dx * dx + dy * dy) / (2 * SIGMA_PX * SIGMA_PX));
    }
  }
  const energy = new Float64Array(n);
  const ones = new Uint8Array(n);
  const splat = (index: number, sign: number): void => {
    const ix = index % side;
    const iy = Math.floor(index / side);
    for (let y = 0; y < side; y += 1) {
      const ky = (y - iy + side) % side;
      for (let x = 0; x < side; x += 1) {
        const kx = (x - ix + side) % side;
        energy[y * side + x] = (energy[y * side + x] ?? 0) + sign * (kernel[ky * side + kx] ?? 0);
      }
    }
  };
  const extreme = (wantOnes: boolean, largest: boolean): number => {
    let best = -1;
    let bestEnergy = largest ? -Infinity : Infinity;
    for (let i = 0; i < n; i += 1) {
      if ((ones[i] === 1) !== wantOnes) {
        continue;
      }
      const e = energy[i] ?? 0;
      if (largest ? e > bestEnergy : e < bestEnergy) {
        best = i;
        bestEnergy = e;
      }
    }
    return best;
  };
  // The initial pattern: a tenth of the texels, by the hash.
  const initial = Math.floor(n / 10);
  let placed = 0;
  for (let k = 0; placed < initial; k += 1) {
    const index = hash(k) % n;
    if (ones[index] === 0) {
      ones[index] = 1;
      splat(index, 1);
      placed += 1;
    }
  }
  // Tighten it: move the tightest cluster to the largest void until that is the same texel.
  for (let guard = 0; guard < n; guard += 1) {
    const cluster = extreme(true, true);
    ones[cluster] = 0;
    splat(cluster, -1);
    const voidIndex = extreme(false, false);
    ones[voidIndex] = 1;
    splat(voidIndex, 1);
    if (voidIndex === cluster) {
      break;
    }
  }
  const rank = new Int32Array(n).fill(-1);
  const prototype = Uint8Array.from(ones);
  const prototypeEnergy = Float64Array.from(energy);
  // Phase 1: rank the initial ones downward, removing the tightest cluster each time.
  for (let r = initial - 1; r >= 0; r -= 1) {
    const cluster = extreme(true, true);
    ones[cluster] = 0;
    splat(cluster, -1);
    rank[cluster] = r;
  }
  // Phase 2: from the initial pattern, rank upward, filling the largest void each time.
  ones.set(prototype);
  energy.set(prototypeEnergy);
  for (let r = initial; r < n; r += 1) {
    const voidIndex = extreme(false, false);
    ones[voidIndex] = 1;
    splat(voidIndex, 1);
    rank[voidIndex] = r;
  }
  return Float32Array.from(rank, (r) => (r + 0.5) / n);
}
