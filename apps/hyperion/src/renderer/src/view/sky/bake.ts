/**
 * The sky's bake: the faint majority of a view's stars summed into an `rgb9e5ufloat` cube, which a
 * full-screen draw samples each frame (plan R06, Design note 21, T13.g).
 *
 * @remarks
 * On the GPU, one face at a time: the baked stars are splatted as a `point-list` into an
 * `rgba32float` scratch face through R01's `createPointSplat` (each point at its texel's centre by
 * `cubeTexel.wgsl`, alike to `cube.ts` to the bit), and a first pass over the six faces finds the
 * brightest texel's luminance, flux over solid angle, by an atomic maximum. A second pass splats
 * each face again into level 0 of the face's chain, sums its mips as flux and solid angle (the
 * first step reading level 0's solid angles from a buffer), packs each level with
 * the power of two that takes the brightest texel to at most 2¹⁵ (`bakePack.wgsl`, alike to
 * `pack.ts` to the bit) into a staging buffer of one face of one level, and copies it into the
 * cube with `writePackedCubeLevelFromBuffer`, with no readback. The scale's source, the brightest
 * luminance's bits, stays in a small buffer the cube's draw reads.
 *
 * Without `float32-blendable` the splat cannot be made, and the bake runs on the CPU through the
 * same steps' TypeScript references (`splatCpu.ts`, `mips.ts`, `pack.ts`), one face at a time; the
 * sums are the same in another order.
 *
 * The cube counts under `MemoryCategory` `sky-cube`; every transient, the scratch face and its mip
 * chain, the solid angles, the points and the staging, under `sky-scratch`, released when the bake
 * ends (T13.h's `releaseBuffer` and `releaseTexture`).
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type {
  BufferHandle,
  ComputeHandle,
  PointSplatHandle,
  RenderEngine,
  TextureHandle,
} from "../engine/types";
import { CUBE_FACE_COUNT, texelSolidAnglesSr } from "./cube";
import {
  divideBySolidAngle,
  faceMipChain,
  mipSizes,
  mipStep,
  peakScaleExponent,
  scaleByPowerOfTwo,
} from "./mips";
import { packRgb9e5Texels } from "./pack";
import bakeCommonWgsl from "./shaders/bakeCommon.wgsl?raw";
import bakeClearWgsl from "./shaders/bakeClear.wgsl?raw";
import bakeMipWgsl from "./shaders/bakeMip.wgsl?raw";
import bakePackWgsl from "./shaders/bakePack.wgsl?raw";
import bakePeakWgsl from "./shaders/bakePeak.wgsl?raw";
import cubeTexelWgsl from "./shaders/cubeTexel.wgsl?raw";
import splatWgsl from "./shaders/splat.wgsl?raw";
import splatFragmentWgsl from "./shaders/splatFragment.wgsl?raw";
import { SPLAT_POINT_FLOATS, splatCpu } from "./splatCpu";

/** Rows of a buffer copied into a texture are padded to this many bytes (WebGPU §"copies"). */
const COPY_ROW_ALIGNMENT = 256;

/** A face's row of `rgb9e5` texels padded for `copyBufferToTexture`, in texels. */
export function paddedRowTexels(sizePx: number): number {
  return Math.ceil((sizePx * 4) / COPY_ROW_ALIGNMENT) * (COPY_ROW_ALIGNMENT / 4);
}

/** The bake's point splat: `splat.wgsl` after `cubeTexel.wgsl`, and its fragment stage. */
export const BAKE_SPLAT = {
  name: "sky:bakeSplat",
  vertexWgsl: cubeTexelWgsl + splatWgsl,
  fragmentWgsl: splatFragmentWgsl,
  format: "rgba32float",
  blend: "additive",
} as const;

/** A bake kernel of one WGSL source. */
function kernel(name: string, reference: string, readback: KernelPair["readback"]): KernelPair {
  return { name, reference, subgroup: null, readback };
}

/** Zeros a scratch face before its splat, which adds to what it holds. */
export const BAKE_CLEAR_KERNEL = kernel("sky bake clear", bakeClearWgsl, "bit-exact");
/** The brightest luminance of a splatted face, into the bake's peak. */
export const BAKE_PEAK_KERNEL = kernel("sky bake peak", bakePeakWgsl, "presentation-only");
/** One mip step of a baked face. */
export const BAKE_MIP_KERNEL = kernel("sky bake mip", bakeMipWgsl, "presentation-only");
/**
 * A baked face's level, packed to `rgb9e5` with the cube's scale. The packing is exact for its
 * `f32` luminances, the division of flux by solid angle before it is not (WGSL divides to 2.5 ulp);
 * declared bit-exact so that the harness's cube reads compare with a tolerance (RM1 M1).
 */
export const BAKE_PACK_KERNEL = kernel("sky bake pack", bakeCommonWgsl + bakePackWgsl, "bit-exact");

/** The bake's kernels, in the order the harness's catalogue compiles them. */
export const BAKE_KERNELS: ReadonlyArray<KernelPair> = [
  BAKE_CLEAR_KERNEL,
  BAKE_PEAK_KERNEL,
  BAKE_MIP_KERNEL,
  BAKE_PACK_KERNEL,
];

/** A baked cube: the cube, and the buffer whose bits give its scale. */
export interface BakedCube {
  /** `rgb9e5ufloat`, its faces' luminance × 2^k, every level. */
  readonly cube: TextureHandle;
  /**
   * One `u32`, the brightest level-0 luminance's `f32` bits, from which the cube's draw takes k
   * as `bakeCommon.wgsl`'s `scaleExponent` does.
   */
  readonly peak: BufferHandle;
  readonly faceSizePx: number;
  /** How the bake ran: on the GPU, or on the CPU without `float32-blendable`. */
  readonly path: "gpu" | "cpu";
}

/** What a bake is given. */
export interface BakeInput {
  /** The baked stars' directions, three floats a star, galactic axes. */
  readonly directions: Float32Array;
  /** Their illuminance per channel, three floats a star, lx. */
  readonly illuminanceLx: Float32Array;
  /** The cube's face side, texels: 2ⁿ or 3 × 2ⁿ (the setting's `faceSizePx`). */
  readonly faceSizePx: number;
  /** The cube's name in allocation events (T13.h's named cube). */
  readonly name: string;
}

/** The splat's points with the header `splat.wgsl` reads: (face, size, 0, 0), then the stars. */
function headedPoints(input: BakeInput): Float32Array {
  const count = input.directions.length / 3;
  const points = new Float32Array(4 + count * SPLAT_POINT_FLOATS);
  points[1] = input.faceSizePx;
  for (let star = 0; star < count; star += 1) {
    const at = 4 + star * SPLAT_POINT_FLOATS;
    points.set(input.directions.subarray(star * 3, star * 3 + 3), at);
    points.set(input.illuminanceLx.subarray(star * 3, star * 3 + 3), at + 4);
    points[at + 7] = 1;
  }
  return points;
}

/** The `f32` bits of a value, as the peak buffer holds them. */
function f32Bits(value: number): Uint32Array {
  return new Uint32Array(Float32Array.of(value).buffer);
}

/** The bake's compute kernels on one engine. */
interface BakeKernels {
  readonly clear: ComputeHandle;
  readonly peak: ComputeHandle;
  readonly mip: ComputeHandle;
  readonly pack: ComputeHandle;
}

/** The bake's pass in `PassTimes`. */
const BAKE_PASS = "sky bake";

/** Each face size's texel solid angles in `f32`, computed once in `f64` and narrowed. */
const SOLID_ANGLES_F32 = new Map<number, Float32Array>();

/**
 * A face's texel solid angles as the kernels read them, `f32`; computed in `f64` (the four-corner
 * formula cancels in `f32` at 3,072²) once per face size.
 */
function texelSolidAnglesF32(sizePx: number): Float32Array {
  let angles = SOLID_ANGLES_F32.get(sizePx);
  if (angles === undefined) {
    angles = Float32Array.from(texelSolidAnglesSr(sizePx));
    SOLID_ANGLES_F32.set(sizePx, angles);
  }
  return angles;
}

/** Workgroups of 8 × 8 over a square of `size`. */
function groups(size: number): readonly [number, number, number] {
  return [Math.ceil(size / 8), Math.ceil(size / 8), 1];
}

/** The size uniform the kernels read: (a, b, c, 0). */
function params(a: number, b = 0, c = 0): Uint32Array {
  return Uint32Array.of(a, b, c, 0);
}

/**
 * Bakes the stars into a cube (Design note 21).
 *
 * @throws Error for a face size with no mip chain to one texel, or directions and illuminances of
 *   different counts.
 */
export function bakeSkyCube(engine: RenderEngine, input: BakeInput): BakedCube {
  if (input.directions.length !== input.illuminanceLx.length || input.directions.length % 3 !== 0) {
    throw new Error("a bake's directions and illuminances are not the same whole stars");
  }
  mipSizes(input.faceSizePx);
  return engine.capabilities.float32Blendable
    ? bakeOnGpu(engine, input)
    : bakeSkyCubeOnCpu(engine, input);
}

function bakeOnGpu(engine: RenderEngine, input: BakeInput): BakedCube {
  const size = input.faceSizePx;
  const sizes = mipSizes(size);
  const kernels: BakeKernels = {
    clear: engine.createCompute(BAKE_CLEAR_KERNEL),
    peak: engine.createCompute(BAKE_PEAK_KERNEL),
    mip: engine.createCompute(BAKE_MIP_KERNEL),
    pack: engine.createCompute(BAKE_PACK_KERNEL),
  };
  const headed = headedPoints(input);
  const count = input.directions.length / 3;
  // Everything made is released on the way out; the cube and its peak only on a failure.
  const transients: Array<BufferHandle | TextureHandle> = [];
  const made = <T extends BufferHandle | TextureHandle>(handle: T): T => {
    transients.push(handle);
    return handle;
  };
  let splat: PointSplatHandle | null = null;
  let result: BakedCube | null = null;
  const kept: Array<BufferHandle | TextureHandle> = [];
  try {
    splat = engine.createPointSplat(BAKE_SPLAT);
    const points = made(
      engine.createBuffer({
        name: `${input.name} points`,
        bytes: Math.max(16, headed.byteLength),
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "sky-scratch",
      }),
    );
    engine.writeBuffer(points, 0, headed);
    const omega = made(
      engine.createBuffer({
        name: `${input.name} solid angles`,
        bytes: size * size * 4,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "sky-scratch",
      }),
    );
    engine.writeBuffer(omega, 0, texelSolidAnglesF32(size));
    // The face's levels: level 0 the splat's target (flux and a count), every other level flux and
    // solid angle, summed from its children.
    const chain = made(
      engine.createTexture({
        name: `${input.name} face levels`,
        size: [size, size],
        dimension: "2d",
        format: "rgba32float",
        mips: sizes.length,
        usage:
          TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.COPY_SRC,
        category: "sky-scratch",
      }),
    );
    const staging = made(
      engine.createBuffer({
        name: `${input.name} staging`,
        bytes: paddedRowTexels(size) * size * 4,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
        category: "sky-scratch",
      }),
    );
    const peak = engine.createBuffer({
      name: `${input.name} peak`,
      bytes: 4,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | BUFFER_USAGE.COPY_SRC,
      category: "sky-cube",
    });
    kept.push(peak);
    engine.writeBuffer(peak, 0, f32Bits(0));
    const cube = engine.createPackedCube(size, sizes.length, "sky-cube", input.name);
    kept.push(cube);
    const activeSplat = splat;
    const splatFace = (face: number): void => {
      engine.dispatch(
        kernels.clear,
        {
          uniforms: { params: params(size) },
          buffers: {},
          sampled: {},
          storage: { face: { texture: chain, level: 0 } },
        },
        groups(size),
        BAKE_PASS,
      );
      engine.writeBuffer(points, 0, Float32Array.of(face, size, 0, 0));
      if (count > 0) {
        activeSplat.draw(chain, points, count);
      }
    };
    for (let face = 0; face < CUBE_FACE_COUNT; face += 1) {
      splatFace(face);
      engine.dispatch(
        kernels.peak,
        {
          uniforms: { params: params(size) },
          buffers: { omega, peak },
          sampled: {},
          storage: { splat: { texture: chain, level: 0 } },
        },
        groups(size),
        BAKE_PASS,
      );
    }
    for (let face = 0; face < CUBE_FACE_COUNT; face += 1) {
      splatFace(face);
      sizes.forEach((levelSize, level) => {
        if (level > 0) {
          const childSize = sizes[level - 1] ?? 1;
          engine.dispatch(
            kernels.mip,
            {
              // The first step reads the splat's level 0, whose solid angles are the buffer's.
              uniforms: { params: params(levelSize, mipStep(childSize), level === 1 ? 1 : 0) },
              buffers: { omega },
              sampled: {},
              storage: {
                children: { texture: chain, level: level - 1 },
                parent: { texture: chain, level },
              },
            },
            groups(levelSize),
            BAKE_PASS,
          );
        }
        engine.dispatch(
          kernels.pack,
          {
            uniforms: {
              params: params(levelSize, paddedRowTexels(levelSize), level === 0 ? 1 : 0),
            },
            buffers: { peak, omega, packed: staging },
            sampled: {},
            storage: { level: { texture: chain, level } },
          },
          groups(levelSize),
          BAKE_PASS,
        );
        engine.writePackedCubeLevelFromBuffer(cube, level, staging, face);
      });
    }
    result = { cube, peak, faceSizePx: size, path: "gpu" };
    return result;
  } finally {
    splat?.dispose();
    for (const handle of transients) {
      release(engine, handle);
    }
    if (result === null) {
      for (const handle of kept) {
        release(engine, handle);
      }
    }
  }
}

/** Releases a buffer or a texture by its kind. */
function release(engine: RenderEngine, handle: BufferHandle | TextureHandle): void {
  if (handle.kind === "buffer") {
    engine.releaseBuffer(handle);
  } else {
    engine.releaseTexture(handle);
  }
}

/** The stars of one face, in the splat's layout without the header. */
function pointsOf(input: BakeInput): Float32Array {
  return headedPoints(input).subarray(4);
}

/**
 * The bake on the CPU, the fallback without `float32-blendable`, and the harness's reference for
 * the GPU bake.
 */
export function bakeSkyCubeOnCpu(engine: RenderEngine, input: BakeInput): BakedCube {
  const size = input.faceSizePx;
  const sizes = mipSizes(size);
  const omegas = texelSolidAnglesSr(size);
  // The six faces at once: the fallback serves the low setting's 1,024² faces (some 100 MB here)
  // on adapters without float32-blendable; the high setting's devices have it.
  const faces = splatCpu(pointsOf(input), size);
  for (const face of faces) {
    divideBySolidAngle(face, size, omegas);
  }
  const exponent = peakScaleExponent(faces);
  const cube = engine.createPackedCube(size, sizes.length, "sky-cube", input.name);
  const levels: Uint32Array[] = sizes.map(
    (levelSize) => new Uint32Array(levelSize * levelSize * 6),
  );
  faces.forEach((face, faceIndex) => {
    scaleByPowerOfTwo(face, exponent);
    faceMipChain(face, size).forEach((level, index) => {
      const levelSize = sizes[index] ?? 1;
      levels[index]?.set(packRgb9e5Texels(level), faceIndex * levelSize * levelSize);
    });
  });
  levels.forEach((packed, level) => {
    engine.writePackedCubeLevel(cube, level, packed);
  });
  const peak = engine.createBuffer({
    name: `${input.name} peak`,
    bytes: 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | BUFFER_USAGE.COPY_SRC,
    category: "sky-cube",
  });
  // A peak whose `scaleExponent` is the exponent applied: 2^(15 − k).
  engine.writeBuffer(peak, 0, f32Bits(2 ** (15 - exponent)));
  return { cube, peak, faceSizePx: size, path: "cpu" };
}

/** Releases a baked cube and its peak (T14's view release). */
export function releaseBakedCube(engine: RenderEngine, baked: BakedCube): void {
  engine.releaseTexture(baked.cube);
  engine.releaseBuffer(baked.peak);
}
