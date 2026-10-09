/**
 * From a splatted face to the cube's levels on the CPU: the division by solid angle, the
 * power-of-two scale and the mip chain weighted by solid angle (plan R06, Design note 21 and
 * T13.b), the reference the bake's compute pass is tested against.
 *
 * @remarks
 * A splat leaves each texel the summed illuminance of the stars in it, lx. Dividing by the texel's
 * solid angle gives a luminance, cd/m², which a sampler can filter. The brightest texel of the six
 * faces is then scaled by a power of two to just under 2¹⁵, inside `rgb9e5ufloat`'s 65,408, and the
 * scale is kept with the cube; a power of two is exact, so unscaling loses nothing. Each mip texel
 * is its children's luminance weighted by their solid angles, so a level's Σ L Ω, its flux, is the
 * level above's.
 */

import { CUBE_FACE_COUNT, texelSolidAnglesSr } from "./cube";

/** The exponent the brightest texel is scaled to at most: 2¹⁵. */
export const CUBE_PEAK_EXPONENT = 15;

/** Refuses a face that is not `sizePx`² `rgba` texels. */
function assertFace(face: Float32Array, sizePx: number): void {
  if (face.length !== sizePx * sizePx * 4) {
    throw new Error(`${face.length} floats are not a ${sizePx}² face of rgba texels`);
  }
}

/**
 * The texels one mip step merges along each side: 2, or 3 for a 3-texel level (3,072's last step,
 * 3 → 1).
 *
 * @throws Error for a level side that is neither even, 3 nor 1, which no face size of 2ⁿ or 3 × 2ⁿ
 *   reaches.
 */
export function mipStep(sizePx: number): number {
  if (sizePx % 2 === 0) {
    return 2;
  }
  if (sizePx === 3) {
    return 3;
  }
  throw new Error(`a level of side ${sizePx} has no mip step; faces are 2ⁿ or 3 × 2ⁿ`);
}

/**
 * The side of every level from `sizePx` down to 1, as the engine sizes a cube's levels
 * (`max(1, size >> level)`).
 */
export function mipSizes(sizePx: number): number[] {
  const sizes = [sizePx];
  let size = sizePx;
  while (size > 1) {
    size /= mipStep(size);
    sizes.push(size);
  }
  return sizes;
}

/**
 * Divides each texel's red, green and blue by its solid angle, in place: illuminance in, lx, to
 * luminance, cd/m². Alpha is left as it is.
 *
 * @param face - One face's `rgba` texels, rows top to bottom.
 * @throws Error when the face or the solid angles are not `sizePx`² texels.
 */
export function divideBySolidAngle(
  face: Float32Array,
  sizePx: number,
  solidAnglesSr: Float64Array,
): void {
  assertFace(face, sizePx);
  if (solidAnglesSr.length !== sizePx * sizePx) {
    throw new Error(`${solidAnglesSr.length} solid angles do not cover a ${sizePx}² face`);
  }
  for (let texel = 0; texel < sizePx * sizePx; texel += 1) {
    const omega = solidAnglesSr[texel] ?? Number.NaN;
    for (let channel = 0; channel < 3; channel += 1) {
      const at = texel * 4 + channel;
      face[at] = (face[at] ?? 0) / omega;
    }
  }
}

/**
 * The power of two that takes the brightest channel of the six faces to at most 2¹⁵ and above
 * 2¹⁴, as an exponent; 0 for a black cube.
 *
 * @remarks
 * The texture holds L × 2^k; the sampler's reader multiplies by 2^−k.
 */
export function peakScaleExponent(faces: ReadonlyArray<Float32Array>): number {
  let peak = 0;
  for (const face of faces) {
    for (let texel = 0; texel < face.length; texel += 4) {
      for (let channel = 0; channel < 3; channel += 1) {
        const value = face[texel + channel] ?? 0;
        // A non-finite texel is a bug upstream, which the bake's finiteness check reports; it
        // must not set the scale of the rest.
        if (Number.isFinite(value) && value > peak) {
          peak = value;
        }
      }
    }
  }
  if (!(peak > 0) || !Number.isFinite(peak)) {
    return 0;
  }
  // `Math.log2` may round across an integer near a power of two; correct against exact powers.
  let exponent = CUBE_PEAK_EXPONENT - Math.ceil(Math.log2(peak));
  if (peak * 2 ** exponent > 2 ** CUBE_PEAK_EXPONENT) {
    exponent -= 1;
  } else if (peak * 2 ** (exponent + 1) <= 2 ** CUBE_PEAK_EXPONENT) {
    exponent += 1;
  }
  return exponent;
}

/** Multiplies every channel's red, green and blue by 2^`exponent`, in place; exact. */
export function scaleByPowerOfTwo(face: Float32Array, exponent: number): void {
  const factor = 2 ** exponent;
  for (let texel = 0; texel < face.length; texel += 4) {
    for (let channel = 0; channel < 3; channel += 1) {
      face[texel + channel] = (face[texel + channel] ?? 0) * factor;
    }
  }
}

/**
 * One face's mip chain from its level 0, each texel its children's luminance weighted by their
 * solid angles.
 *
 * @param level0 - The face's `rgba` luminances, rows top to bottom.
 * @returns Every level, level 0 (the argument itself) first, down to 1 × 1. Alpha is averaged the
 *   same way.
 * @throws Error when `level0` is not `sizePx`² `rgba` texels, or a level has no mip step.
 */
export function faceMipChain(level0: Float32Array, sizePx: number): Float32Array[] {
  assertFace(level0, sizePx);
  const levels = [level0];
  let size = sizePx;
  let texels = level0;
  let omegas = texelSolidAnglesSr(sizePx);
  while (size > 1) {
    const step = mipStep(size);
    const parentSize = size / step;
    const parent = new Float32Array(parentSize * parentSize * 4);
    const parentOmegas = new Float64Array(parentSize * parentSize);
    for (let row = 0; row < parentSize; row += 1) {
      for (let column = 0; column < parentSize; column += 1) {
        const sums = [0, 0, 0, 0];
        let omegaSum = 0;
        for (let dy = 0; dy < step; dy += 1) {
          for (let dx = 0; dx < step; dx += 1) {
            const child = (row * step + dy) * size + column * step + dx;
            const omega = omegas[child] ?? 0;
            omegaSum += omega;
            for (let channel = 0; channel < 4; channel += 1) {
              sums[channel] = (sums[channel] ?? 0) + (texels[child * 4 + channel] ?? 0) * omega;
            }
          }
        }
        const at = row * parentSize + column;
        parentOmegas[at] = omegaSum;
        for (let channel = 0; channel < 4; channel += 1) {
          parent[at * 4 + channel] = (sums[channel] ?? 0) / omegaSum;
        }
      }
    }
    levels.push(parent);
    texels = parent;
    omegas = parentOmegas;
    size = parentSize;
  }
  return levels;
}

/**
 * A cube's levels for `writePackedCubeLevel`: each level's six faces, face after face, joined.
 *
 * @param faceChains - Six faces' chains from {@link faceMipChain}, in WebGPU's layer order.
 * @throws Error when there are not six faces or their chains differ in length.
 */
export function cubeLevels(faceChains: ReadonlyArray<ReadonlyArray<Float32Array>>): Float32Array[] {
  if (faceChains.length !== CUBE_FACE_COUNT) {
    throw new Error(`a cube has ${CUBE_FACE_COUNT} faces, not ${faceChains.length}`);
  }
  const levelCount = faceChains[0]?.length ?? 0;
  if (faceChains.some((chain) => chain.length !== levelCount)) {
    throw new Error("a cube's faces have mip chains of different lengths");
  }
  return Array.from({ length: levelCount }, (_, level) => {
    const faces = faceChains.map((chain) => {
      const face = chain[level];
      if (face === undefined) {
        throw new Error(`a face's chain stops before level ${level}`);
      }
      return face;
    });
    const joined = new Float32Array(faces.reduce((total, face) => total + face.length, 0));
    let offset = 0;
    for (const face of faces) {
      joined.set(face, offset);
      offset += face.length;
    }
    return joined;
  });
}
