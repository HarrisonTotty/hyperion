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
 * A face larger than {@link BAKE_TILE_PX} is baked a tile at a time (the owner's priority,
 * 2026-11-01: the high setting's first bake made 286 MB of scratch beside its 302 MB cube, on
 * Apple silicon's shared memory): each tile is splatted into a scratch tile, its levels down to
 * {@link BakeTiling.upperLevel} are summed, packed and copied into their places on the cube, and
 * its last step writes its parents into the face's upper levels, which are then baked whole. A
 * texel's stars blend in the same order, and each sum, division and packing takes the same values,
 * so the cube is the untiled bake's to the bit (the harness compares the two). The solid angles
 * are the face's lower triangle (`cube.ts`'s `texelSolidAnglesLowerF32`), half the table.
 *
 * Without `float32-blendable` the splat cannot be made, and the bake runs on the CPU through the
 * same steps' TypeScript references (`splatCpu.ts`, `mips.ts`, `pack.ts`), one face at a time; the
 * sums are the same in another order.
 *
 * The cube counts under `MemoryCategory` `sky-cube`; every transient, the scratch face's levels (or
 * a tile's and the face's upper levels), the solid angles, the points and the staging, under
 * `sky-scratch`, released when the bake ends (T13.h's `releaseBuffer` and `releaseTexture`).
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
import { CUBE_FACE_COUNT, texelSolidAnglesLowerF32, texelSolidAnglesSr } from "./cube";
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
import bakeOmegaWgsl from "./shaders/bakeOmega.wgsl?raw";
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
export const BAKE_PEAK_KERNEL = kernel(
  "sky bake peak",
  bakePeakWgsl + bakeOmegaWgsl,
  "presentation-only",
);
/** One mip step of a baked face. */
export const BAKE_MIP_KERNEL = kernel(
  "sky bake mip",
  bakeMipWgsl + bakeOmegaWgsl,
  "presentation-only",
);
/**
 * A baked face's level, packed to `rgb9e5` with the cube's scale. The packing is exact for its
 * `f32` luminances, the division of flux by solid angle before it is not (WGSL divides to 2.5 ulp);
 * declared bit-exact so that the harness's cube reads compare with a tolerance (RM1 M1).
 */
export const BAKE_PACK_KERNEL = kernel(
  "sky bake pack",
  bakeCommonWgsl + bakePackWgsl + bakeOmegaWgsl,
  "bit-exact",
);

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
  /**
   * The largest tile the face's first levels are baked in, texels; {@link BAKE_TILE_PX} when
   * absent. The harness's check sets it, to compare a tiled bake with an untiled one.
   */
  readonly tilePx?: number;
}

/**
 * The largest tile side a face is baked in, texels: a 3,072² face in nine tiles of 1,024², whose
 * scratch is 16.8 MB at level 0 against the whole face's 151 MB; the low setting's 1,024² face
 * whole.
 */
export const BAKE_TILE_PX = 1_024;

/** How a face is baked: its first levels a tile at a time, and the rest whole. */
export interface BakeTiling {
  /** A tile's side at level 0, texels; the face's own where it is baked whole. */
  readonly tilePx: number;
  /** Tiles along each side of the face. */
  readonly tilesPerSide: number;
  /**
   * The first level baked whole, its side at most half a tile's; 0 when the face is one tile. The
   * levels below it are baked a tile at a time.
   */
  readonly upperLevel: number;
}

/**
 * A face's tiling: tiles of the largest power of two that divides the face and is no larger than
 * `maxTilePx`, down to the first level of at most half a tile's side.
 *
 * @throws Error for a tiling whose tiles would shrink below a texel before that level.
 */
export function bakeTiling(faceSizePx: number, maxTilePx: number = BAKE_TILE_PX): BakeTiling {
  if (faceSizePx <= maxTilePx) {
    return { tilePx: faceSizePx, tilesPerSide: 1, upperLevel: 0 };
  }
  let tilePx = 1;
  while (tilePx * 2 <= maxTilePx && faceSizePx % (tilePx * 2) === 0) {
    tilePx *= 2;
  }
  const upperLevel = mipSizes(faceSizePx).findIndex((side) => side <= tilePx / 2);
  if (upperLevel < 1 || tilePx >> upperLevel < 1) {
    throw new Error(`a ${faceSizePx}² face has no tiling in tiles of at most ${maxTilePx}²`);
  }
  return { tilePx, tilesPerSide: faceSizePx / tilePx, upperLevel };
}

/**
 * The splat's points with the header `splat.wgsl` reads: (face, size, tile side, tile), the face
 * and tile written before each draw, then the stars.
 */
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

/** Each face size's texel solid angles in `f32`, the face's lower triangle. */
const SOLID_ANGLES_F32 = new Map<number, Float32Array>();

/**
 * A face's texel solid angles as the kernels read them, `f32`, its lower triangle; computed in
 * `f64` (the four-corner formula cancels in `f32` at 3,072²) once per face size.
 */
function texelSolidAnglesF32(sizePx: number): Float32Array {
  let angles = SOLID_ANGLES_F32.get(sizePx);
  if (angles === undefined) {
    angles = texelSolidAnglesLowerF32(sizePx);
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

/** The mip and pack kernels' uniform: the size's (a, b, c, 0), then an origin's four. */
function placed(size: Uint32Array, origin: readonly [number, number, number, number]): Uint32Array {
  return Uint32Array.of(...size, ...origin);
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

/** A storage binding of one level of a bake texture. */
interface LevelOf {
  readonly texture: TextureHandle;
  readonly level: number;
}

function bakeOnGpu(engine: RenderEngine, input: BakeInput): BakedCube {
  const size = input.faceSizePx;
  const sizes = mipSizes(size);
  const { tilePx, tilesPerSide, upperLevel } = bakeTiling(size, input.tilePx);
  const upperSize = sizes[upperLevel] ?? 1;
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
    const solidAngles = texelSolidAnglesF32(size);
    const omega = made(
      engine.createBuffer({
        name: `${input.name} solid angles`,
        bytes: solidAngles.byteLength,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "sky-scratch",
      }),
    );
    engine.writeBuffer(omega, 0, solidAngles);
    const usage =
      TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.COPY_SRC;
    // The face's levels from the upper level down, each flux and solid angle summed from its
    // children; on a face of one tile, level 0 is the splat's target (flux and a count).
    const upper = made(
      engine.createTexture({
        name: `${input.name} face levels`,
        size: [upperSize, upperSize],
        dimension: "2d",
        format: "rgba32float",
        mips: sizes.length - upperLevel,
        usage,
        category: "sky-scratch",
      }),
    );
    // A tile's levels below the upper level, level 0 the splat's target.
    const tiles =
      upperLevel === 0
        ? upper
        : made(
            engine.createTexture({
              name: `${input.name} tile levels`,
              size: [tilePx, tilePx],
              dimension: "2d",
              format: "rgba32float",
              mips: upperLevel,
              usage,
              category: "sky-scratch",
            }),
          );
    const staging = made(
      engine.createBuffer({
        name: `${input.name} staging`,
        bytes:
          Math.max(paddedRowTexels(tilePx) * tilePx, paddedRowTexels(upperSize) * upperSize) * 4,
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
    const tileCount = tilesPerSide * tilesPerSide;
    // A tile's origin on the face at a level below the upper level, texels; tiles row after row.
    const originOf = (tile: number, level: number): readonly [number, number] => {
      const side = tilePx >> level;
      return [(tile % tilesPerSide) * side, Math.floor(tile / tilesPerSide) * side];
    };
    const splatTile = (face: number, tile: number): void => {
      engine.dispatch(
        kernels.clear,
        {
          uniforms: { params: params(tilePx) },
          buffers: {},
          sampled: {},
          storage: { face: { texture: tiles, level: 0 } },
        },
        groups(tilePx),
        BAKE_PASS,
      );
      engine.writeBuffer(points, 0, Float32Array.of(face, size, tilePx, tile));
      if (count > 0) {
        activeSplat.draw(tiles, points, count);
      }
    };
    // One mip step into the face's `level`, `side` parents placed at `at` in their texture; the
    // first step reads the splat's level 0, whose solid angles are the buffer's, at `childOrigin`.
    const mip = (
      children: LevelOf,
      parents: LevelOf,
      level: number,
      side: number,
      childOrigin: readonly [number, number],
      at: readonly [number, number],
    ): void => {
      const fromSplat = level === 1;
      const [childX, childY] = fromSplat ? childOrigin : [0, 0];
      engine.dispatch(
        kernels.mip,
        {
          uniforms: {
            params: placed(params(side, mipStep(sizes[level - 1] ?? 1), fromSplat ? 1 : 0), [
              childX,
              childY,
              at[0],
              at[1],
            ]),
          },
          buffers: { omega },
          sampled: {},
          storage: { children, parent: parents },
        },
        groups(side),
        BAKE_PASS,
      );
    };
    // Packs the face's `level`, or a tile's `side`² of it at `origin`, and copies it to the cube.
    const pack = (
      face: number,
      source: LevelOf,
      level: number,
      side: number,
      origin: readonly [number, number] | null,
    ): void => {
      const [x, y] = origin ?? [0, 0];
      // The solid angles are read at level 0 alone.
      const [omegaX, omegaY] = level === 0 ? [x, y] : [0, 0];
      engine.dispatch(
        kernels.pack,
        {
          uniforms: {
            params: placed(params(side, paddedRowTexels(side), level === 0 ? 1 : 0), [
              omegaX,
              omegaY,
              0,
              0,
            ]),
          },
          buffers: { peak, omega, packed: staging },
          sampled: {},
          storage: { level: source },
        },
        groups(side),
        BAKE_PASS,
      );
      if (origin === null) {
        engine.writePackedCubeLevelFromBuffer(cube, level, staging, face);
      } else {
        engine.writePackedCubeLevelFromBuffer(cube, level, staging, face, {
          x,
          y,
          width: side,
          height: side,
        });
      }
    };
    for (let face = 0; face < CUBE_FACE_COUNT; face += 1) {
      for (let tile = 0; tile < tileCount; tile += 1) {
        splatTile(face, tile);
        const [x, y] = originOf(tile, 0);
        engine.dispatch(
          kernels.peak,
          {
            uniforms: { params: params(tilePx, x, y) },
            buffers: { omega, peak },
            sampled: {},
            storage: { splat: { texture: tiles, level: 0 } },
          },
          groups(tilePx),
          BAKE_PASS,
        );
      }
    }
    for (let face = 0; face < CUBE_FACE_COUNT; face += 1) {
      for (let tile = 0; tile < tileCount; tile += 1) {
        splatTile(face, tile);
        for (let level = 0; level < upperLevel; level += 1) {
          const side = tilePx >> level;
          if (level > 0) {
            mip(
              { texture: tiles, level: level - 1 },
              { texture: tiles, level },
              level,
              side,
              originOf(tile, 0),
              [0, 0],
            );
          }
          pack(face, { texture: tiles, level }, level, side, originOf(tile, level));
        }
        if (upperLevel > 0) {
          // The tile's last step writes its parents where they lie on the face's upper level.
          mip(
            { texture: tiles, level: upperLevel - 1 },
            { texture: upper, level: 0 },
            upperLevel,
            tilePx >> upperLevel,
            originOf(tile, 0),
            originOf(tile, upperLevel),
          );
        }
      }
      for (let level = upperLevel; level < sizes.length; level += 1) {
        const side = sizes[level] ?? 1;
        if (level > upperLevel) {
          mip(
            { texture: upper, level: level - upperLevel - 1 },
            { texture: upper, level: level - upperLevel },
            level,
            side,
            [0, 0],
            [0, 0],
          );
        }
        pack(face, { texture: upper, level: level - upperLevel }, level, side, null);
      }
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
