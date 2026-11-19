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
 * A bake is made in steps ({@link startSkyBake}; lane perf-long-tasks-3): its stars laid out a
 * range at a time, then its uploads and scratch made in a frame, then a tile's splat and levels or
 * a face's last levels a step, so that a view's bake runs in short tasks between its frames
 * (`cache.ts`) rather than as one long frame. Its commands are the same and come in the same order
 * on resources of its own, so the cube is the one {@link bakeSkyCube}, which runs every step at
 * once, makes, to the bit.
 *
 * The cube counts under `MemoryCategory` `sky-cube`; every transient, the scratch face's levels (or
 * a tile's and the face's upper levels), the solid angles, the points and the staging, under
 * `sky-scratch`, released when the bake ends (T13.h's `releaseBuffer` and `releaseTexture`). A
 * view's bakes read the solid angles the device's cube cache holds from bake to bake instead,
 * under `sky-scratch` too ({@link HeldSolidAngles}; lane perf-long-tasks-4).
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

/** What a bake is given: its stars as arrays, the harness's and the tests' form. */
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
 * The stars a bake splats, laid out a range at a time into the splat's points (lane
 * perf-long-tasks-3): a view's are read from its sky as the bake goes, with no arrays of their own.
 */
export interface BakeStars {
  /** How many stars are baked. */
  readonly count: number;
  /**
   * Lays stars [from, to) out in `points`, each through {@link writeBakePoint} at its index.
   *
   * @param points - The splat's points, {@link BAKE_POINTS_HEADER} floats of header first.
   */
  writeInto(points: Float32Array, from: number, to: number): void;
}

/** What a bake is given, its stars laid out as it goes ({@link startSkyBake}). */
export interface SkyBakeSource extends Omit<BakeInput, "directions" | "illuminanceLx"> {
  readonly stars: BakeStars;
}

/** The floats before the first star of the splat's points: (face, size, tile side, tile). */
export const BAKE_POINTS_HEADER = 4;

/**
 * Writes one star into the splat's points as `splat.wgsl` reads it: its direction, 0, its
 * illuminance per channel, lx, and 1, each stored as `f32`.
 *
 * @param star - Its index among the baked stars.
 */
export function writeBakePoint(
  points: Float32Array,
  star: number,
  x: number,
  y: number,
  z: number,
  redLx: number,
  greenLx: number,
  blueLx: number,
): void {
  const at = BAKE_POINTS_HEADER + star * SPLAT_POINT_FLOATS;
  points[at] = x;
  points[at + 1] = y;
  points[at + 2] = z;
  points[at + 3] = 0;
  points[at + 4] = redLx;
  points[at + 5] = greenLx;
  points[at + 6] = blueLx;
  points[at + 7] = 1;
}

/**
 * A bake's source over stars given as arrays.
 *
 * @throws Error for directions and illuminances of different counts.
 */
export function bakeSourceOf(input: BakeInput): SkyBakeSource {
  const { directions, illuminanceLx, ...rest } = input;
  if (directions.length !== illuminanceLx.length || directions.length % 3 !== 0) {
    throw new Error("a bake's directions and illuminances are not the same whole stars");
  }
  return {
    ...rest,
    stars: {
      count: directions.length / 3,
      writeInto: (points, from, to) => {
        for (let star = from; star < to; star += 1) {
          const at = star * 3;
          writeBakePoint(
            points,
            star,
            directions[at] ?? 0,
            directions[at + 1] ?? 0,
            directions[at + 2] ?? 0,
            illuminanceLx[at] ?? 0,
            illuminanceLx[at + 1] ?? 0,
            illuminanceLx[at + 2] ?? 0,
          );
        }
      },
    },
  };
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

/** The stars a bake's step lays out (lane perf-long-tasks-3): 16,384, about 5 ms on a pod. */
export const BAKE_STARS_A_STEP = 16_384;

/**
 * The page's one array of splat points, kept from bake to bake (lane perf-long-tasks-3): a
 * re-bake's 300,000 stars took a new 9.6 MB array each time, beside the garbage it set off.
 */
let pointsScratch: Float32Array | null = null;
/** Whether a bake holds {@link pointsScratch}, from its stars' layout to their upload. */
let pointsScratchTaken = false;

/**
 * The splat's points for `count` stars, {@link BAKE_POINTS_HEADER} floats of header first: the
 * page's kept array where it is free and large enough, else a new one, kept where none is held.
 * Every float of the header and of each star is written before it is read; what lies past them is
 * never read.
 */
function takePoints(count: number): Float32Array {
  const floats = BAKE_POINTS_HEADER + count * SPLAT_POINT_FLOATS;
  if (!pointsScratchTaken && pointsScratch !== null && pointsScratch.length >= floats) {
    pointsScratchTaken = true;
    return pointsScratch.subarray(0, floats);
  }
  const points = new Float32Array(floats);
  if (!pointsScratchTaken) {
    pointsScratch = points;
    pointsScratchTaken = true;
  }
  return points;
}

/** Gives the page's kept array back once a bake's points are uploaded. */
function givePoints(points: Float32Array): void {
  if (pointsScratch !== null && points.buffer === pointsScratch.buffer) {
    pointsScratchTaken = false;
  }
}

/**
 * Lays a bake's stars out in the splat's points, a step of {@link BAKE_STARS_A_STEP} at a time,
 * after the header `splat.wgsl` reads: (face, size, tile side, tile), the face and tile written
 * before each draw.
 */
function* layOutPoints(
  source: SkyBakeSource,
  points: Float32Array,
): Generator<BakeStepPlace, void, void> {
  points.fill(0, 0, BAKE_POINTS_HEADER);
  points[1] = source.faceSizePx;
  const { count } = source.stars;
  for (let from = 0; from < count; from += BAKE_STARS_A_STEP) {
    source.stars.writeInto(points, from, Math.min(count, from + BAKE_STARS_A_STEP));
    yield "task";
  }
}

/**
 * Where a bake's next step runs: in a task of its own, or in a frame, as its uploads do.
 *
 * @remarks
 * A frame's step is the bake's uploads, 9.6 MB of points and 18.9 MB of solid angles at high, made
 * where the bake at once made them (lane perf-long-tasks-3): in parts, in tasks between the frames,
 * each waited on room in the page's command buffer to the GPU process, which a slow one
 * (SwiftShader's, drawing a frame of some 10 s at high on a pod) frees only as it reads, and one
 * part's write held the page 15 to 43 s there; in a frame they took 13 to 23 ms. Over held solid
 * angles ({@link HeldSolidAngles}) a bake writes its points alone there, and the held buffer's
 * first upload has a frame's step of its own before it (lane perf-long-tasks-4).
 */
export type BakeStepPlace = "task" | "frame";

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
    // A worker still forming them is stopped: its table would be this one, dropped.
    WARMING.get(sizePx)?.terminate();
    WARMING.delete(sizePx);
    angles = texelSolidAnglesLowerF32(sizePx);
    SOLID_ANGLES_F32.set(sizePx, angles);
  }
  return angles;
}

/**
 * A worker that forms one face size's solid angles: `solidAngles.worker.ts`, or a test's. It posts
 * back the `Float32Array` {@link texelSolidAnglesLowerF32} makes, its buffer transferred.
 */
export interface SolidAnglesWorker {
  addEventListener(type: "message", listener: (event: MessageEvent<unknown>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  postMessage(sizePx: number, transfer: Transferable[]): void;
  terminate(): void;
}

/** Starts the solid angles' module worker, or gives `null` where the page has no `Worker`. */
function startSolidAnglesWorker(): SolidAnglesWorker | null {
  return typeof Worker === "undefined"
    ? null
    : new Worker(new URL("./solidAngles.worker.ts", import.meta.url), {
        type: "module",
        name: "sky solid angles",
      });
}

/**
 * The workers forming a face size's solid angles, by size: each ends as it answers, fails, or a
 * bake forms the size's table first. They belong to the page, as the tables they fill do.
 */
const WARMING = new Map<number, SolidAnglesWorker>();

/**
 * Has a worker form a face size's solid angles ahead of its first bake (lane perf-long-tasks-2):
 * at high, the 3,072² face's triangle, 4.7 million arctangents, held the page's thread some 250 to
 * 400 ms on the pod within the first sky's frame. A view calls it from its first frame on; it does
 * nothing once the size is formed or forming.
 *
 * @remarks
 * The worker runs the same function on the same engine's `Math` (V8's `atan2` and the IEEE `sqrt`),
 * so its table is the one the page would form, to the bit. A bake that comes first forms its own,
 * as before, and stops the worker; a reply of the wrong length is dropped, and a worker that fails
 * to load or never answers leaves the bake to form it and stop it.
 *
 * @param start - Starts the worker; the module worker's by default.
 */
export function warmTexelSolidAngles(
  sizePx: number,
  start: () => SolidAnglesWorker | null = startSolidAnglesWorker,
): void {
  if (SOLID_ANGLES_F32.has(sizePx) || WARMING.has(sizePx)) {
    return;
  }
  const worker = start();
  if (worker === null) {
    return;
  }
  WARMING.set(sizePx, worker);
  const settle = (angles: Float32Array | null): void => {
    if (WARMING.get(sizePx) === worker) {
      WARMING.delete(sizePx);
    }
    worker.terminate();
    if (angles !== null && !SOLID_ANGLES_F32.has(sizePx)) {
      SOLID_ANGLES_F32.set(sizePx, angles);
    }
  };
  worker.addEventListener("message", (event) => {
    const angles = event.data;
    settle(
      angles instanceof Float32Array && angles.length === (sizePx * (sizePx + 1)) / 2
        ? angles
        : null,
    );
  });
  worker.addEventListener("error", () => {
    settle(null);
  });
  worker.addEventListener("messageerror", () => {
    settle(null);
  });
  worker.postMessage(sizePx, []);
}

/** Whether a face size's solid angles are formed, for the tests. */
export function hasTexelSolidAngles(sizePx: number): boolean {
  return SOLID_ANGLES_F32.has(sizePx);
}

/** One face size's held solid angles: the buffer, and how many bakes read it. */
interface HeldAngles {
  readonly buffer: BufferHandle;
  users: number;
}

/**
 * The solid angles' buffers a device's bakes share, by face size (lane perf-long-tasks-4): a
 * re-bake at high wrote its 18.9 MB of them anew in its frame's step, beside its 9.6 MB of points,
 * and that step held its frame 34 to 180 ms on a pod. The device's cube cache holds one
 * (`cache.ts`), and each bake it starts reads the buffer of its face size, made and written by the
 * first; the table is the one each bake wrote, so every cube is the one it was, to the bit.
 *
 * @remarks
 * A bake takes its size's buffer in a frame's step and gives it back as it ends, however it ends.
 * The last face size taken keeps its buffer once its bakes end, under `sky-scratch`, until a bake
 * of another size takes one and no bake reads it; so a view's re-bakes write none, and a change of
 * setting holds the two only while a bake of the old size runs. A lost device took them all: the
 * cache {@link HeldSolidAngles.forget | forgets} them, and the next bake writes its size's anew.
 */
export class HeldSolidAngles {
  readonly #engine: RenderEngine;
  readonly #bySize = new Map<number, HeldAngles>();
  /** The face size taken last, whose buffer is kept once no bake reads it. */
  #latest: number | null = null;

  constructor(engine: RenderEngine) {
    this.#engine = engine;
  }

  /** Whether a face size's buffer is held, so that a bake of it writes none. */
  holds(sizePx: number): boolean {
    return this.#bySize.has(sizePx);
  }

  /**
   * A face size's buffer, made and written where it is not held, for one bake to read until it
   * gives it back ({@link HeldSolidAngles.give}); a buffer of another size that no bake reads is
   * released.
   *
   * @throws Whatever making or writing the buffer throws, having released what it made.
   */
  take(sizePx: number): BufferHandle {
    let held = this.#bySize.get(sizePx);
    if (held === undefined) {
      const angles = texelSolidAnglesF32(sizePx);
      const buffer = this.#engine.createBuffer({
        name: `sky bake ${String(sizePx)}² solid angles`,
        bytes: angles.byteLength,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "sky-scratch",
      });
      try {
        this.#engine.writeBuffer(buffer, 0, angles);
      } catch (error: unknown) {
        this.#engine.releaseBuffer(buffer);
        throw error;
      }
      held = { buffer, users: 0 };
      this.#bySize.set(sizePx, held);
    }
    held.users += 1;
    this.#latest = sizePx;
    this.#releaseUnread();
    return held.buffer;
  }

  /** Gives a taken buffer back; nothing for one a restore has forgotten. */
  give(buffer: BufferHandle): void {
    for (const held of this.#bySize.values()) {
      if (held.buffer === buffer) {
        held.users -= 1;
        this.#releaseUnread();
        return;
      }
    }
  }

  /** Forgets every buffer, which the lost device took with it (`RenderEngine.onRestored`). */
  forget(): void {
    this.#bySize.clear();
    this.#latest = null;
  }

  /** Releases every buffer no bake reads, for an owner done with them (the harness's check). */
  release(): void {
    this.#latest = null;
    this.#releaseUnread();
  }

  /** Releases each buffer of a size but the latest that no bake reads. */
  #releaseUnread(): void {
    for (const [sizePx, held] of this.#bySize) {
      if (sizePx !== this.#latest && held.users <= 0) {
        this.#bySize.delete(sizePx);
        this.#engine.releaseBuffer(held.buffer);
      }
    }
  }
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
 * A bake under way, made a step at a time ({@link startSkyBake}).
 *
 * @remarks
 * A step is the stars' layout of {@link BAKE_STARS_A_STEP}; then, over held solid angles that hold
 * none of its size, a frame's, their upload; then a frame's, the kernels', the uploads' and the
 * scratch's making, the solid angles formed there where the worker has not answered; then a tile's splat and peak, a tile's splat and levels, or a face's last levels. The
 * CPU bake, the fallback, lays its stars out and packs its levels in tasks, then makes and writes
 * its cube in a frame's step.
 */
export interface SkyBakeJob {
  /**
   * Runs the bake's next steps, at least one, until `now()` reaches `untilMs`; none, or no more,
   * once its next step is a frame's where `inFrame` is false ({@link BakeStepPlace}).
   *
   * @param inFrame - Whether the caller is a frame's, which may run a frame's step.
   * @returns The cube once its last step is run, and `null` until then; what it made is then the
   *   caller's, to release with {@link releaseBakedCube}.
   * @throws Whatever a step throws (a lost device, a refused allocation), having released what
   *   the bake made; the job is then over.
   */
  step(untilMs: number, now: () => number, inFrame: boolean): BakedCube | null;
  /** Whether its next step is a frame's, which a task's call does not run. */
  readonly awaitsFrame: boolean;
  /** Stops an unfinished bake and releases what it made; nothing once it has finished or failed. */
  cancel(): void;
}

/**
 * Starts a bake that runs as its steps are asked for (Design note 21; lane perf-long-tasks-3).
 *
 * @param solidAngles - The device's held solid angles, which a GPU bake reads in place of a buffer
 *   of its own (lane perf-long-tasks-4); where they hold none of its size, the bake writes them in
 *   a frame's step of their own, before its other uploads. Absent, the bake makes and releases its
 *   own, as before.
 * @throws Error for a face size with no mip chain to one texel.
 */
export function startSkyBake(
  engine: RenderEngine,
  source: SkyBakeSource,
  solidAngles?: HeldSolidAngles,
): SkyBakeJob {
  mipSizes(source.faceSizePx);
  const steps = engine.capabilities.float32Blendable
    ? bakeOnGpu(engine, source, solidAngles)
    : bakeOnCpu(engine, source);
  let over = false;
  // Where the next step runs, as the last step said.
  let place: BakeStepPlace = "task";
  return {
    get awaitsFrame(): boolean {
      return !over && place === "frame";
    },
    step: (untilMs, now, inFrame) => {
      if (over) {
        throw new Error(`the bake of ${source.name} is over`);
      }
      // A step that throws ends the job, its `finally` having released what it made.
      let stepped = false;
      try {
        for (;;) {
          if (place === "frame" && !inFrame) {
            stepped = true;
            return null;
          }
          const next = steps.next();
          if (next.done === true) {
            over = true;
            if (next.value === null) {
              throw new Error(`the bake of ${source.name} ended without its cube`);
            }
            return next.value;
          }
          place = next.value;
          if (now() >= untilMs) {
            stepped = true;
            return null;
          }
        }
      } finally {
        if (!stepped) {
          over = true;
        }
      }
    },
    cancel: () => {
      if (!over) {
        over = true;
        // The generator's `finally` releases what the bake made.
        steps.return(null);
      }
    },
  };
}

/** The clock {@link bakeSkyCube} never reads: it runs every step at once. */
const NO_CLOCK = (): number => 0;

/**
 * Bakes the stars into a cube (Design note 21), every step at once.
 *
 * @throws Error for a face size with no mip chain to one texel, or directions and illuminances of
 *   different counts.
 */
export function bakeSkyCube(engine: RenderEngine, input: BakeInput): BakedCube {
  const baked = startSkyBake(engine, bakeSourceOf(input)).step(
    Number.POSITIVE_INFINITY,
    NO_CLOCK,
    true,
  );
  if (baked === null) {
    throw new Error(`the bake of ${input.name} stopped before its end`);
  }
  return baked;
}

/** A storage binding of one level of a bake texture. */
interface LevelOf {
  readonly texture: TextureHandle;
  readonly level: number;
}

/** The GPU bake, a step at a time; its `finally` releases its transients, and all on a failure. */
function* bakeOnGpu(
  engine: RenderEngine,
  source: SkyBakeSource,
  solidAngles: HeldSolidAngles | undefined,
): Generator<BakeStepPlace, BakedCube | null, void> {
  const size = source.faceSizePx;
  const sizes = mipSizes(size);
  const { tilePx, tilesPerSide, upperLevel } = bakeTiling(size, source.tilePx);
  const upperSize = sizes[upperLevel] ?? 1;
  const count = source.stars.count;
  // Everything made is released on the way out; the cube and its peak only on a failure.
  const transients: Array<BufferHandle | TextureHandle> = [];
  const made = <T extends BufferHandle | TextureHandle>(handle: T): T => {
    transients.push(handle);
    return handle;
  };
  let splat: PointSplatHandle | null = null;
  let result: BakedCube | null = null;
  const kept: Array<BufferHandle | TextureHandle> = [];
  let headed: Float32Array | null = takePoints(count);
  // The device's held solid angles this bake reads, taken until its end (perf-long-tasks-4).
  let taken: BufferHandle | null = null;
  try {
    yield* layOutPoints(source, headed);
    if (solidAngles !== undefined && !solidAngles.holds(size)) {
      // The held solid angles' first upload, in a frame's step of its own (`BakeStepPlace`).
      yield "frame";
      taken = solidAngles.take(size);
    }
    // The uploads and the scratch, in a frame (`BakeStepPlace`).
    yield "frame";
    const kernels: BakeKernels = {
      clear: engine.createCompute(BAKE_CLEAR_KERNEL),
      peak: engine.createCompute(BAKE_PEAK_KERNEL),
      mip: engine.createCompute(BAKE_MIP_KERNEL),
      pack: engine.createCompute(BAKE_PACK_KERNEL),
    };
    splat = engine.createPointSplat(BAKE_SPLAT);
    const points = made(
      engine.createBuffer({
        name: `${source.name} points`,
        bytes: Math.max(16, headed.byteLength),
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
        category: "sky-scratch",
      }),
    );
    engine.writeBuffer(points, 0, headed);
    // Written into the queue's own copy: the page's array is free for the next bake.
    givePoints(headed);
    headed = null;
    taken ??= solidAngles?.take(size) ?? null;
    const omega = taken ?? ownSolidAngles(engine, size, source.name, made);
    const usage =
      TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.COPY_SRC;
    // The face's levels from the upper level down, each flux and solid angle summed from its
    // children; on a face of one tile, level 0 is the splat's target (flux and a count).
    const upper = made(
      engine.createTexture({
        name: `${source.name} face levels`,
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
              name: `${source.name} tile levels`,
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
        name: `${source.name} staging`,
        bytes:
          Math.max(paddedRowTexels(tilePx) * tilePx, paddedRowTexels(upperSize) * upperSize) * 4,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
        category: "sky-scratch",
      }),
    );
    const peak = engine.createBuffer({
      name: `${source.name} peak`,
      bytes: 4,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | BUFFER_USAGE.COPY_SRC,
      category: "sky-cube",
    });
    kept.push(peak);
    engine.writeBuffer(peak, 0, f32Bits(0));
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
    yield "task";
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
        yield "task";
      }
    }
    // The cube is made once the peak is found: a re-bake's stand-in, the cube it replaces, is then
    // held beside it the shortest while (`cache.ts`).
    const cube = engine.createPackedCube(size, sizes.length, "sky-cube", source.name);
    kept.push(cube);
    // Packs the face's `level`, or a tile's `side`² of it at `origin`, and copies it to the cube.
    const pack = (
      face: number,
      from: LevelOf,
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
          storage: { level: from },
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
        yield "task";
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
      yield "task";
    }
    result = { cube, peak, faceSizePx: size, path: "gpu" };
    return result;
  } finally {
    if (headed !== null) {
      givePoints(headed);
    }
    if (taken !== null) {
      solidAngles?.give(taken);
    }
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

/** A bake's own solid angles' buffer, made and written, among its transients (`made`). */
function ownSolidAngles(
  engine: RenderEngine,
  sizePx: number,
  name: string,
  made: (handle: BufferHandle) => BufferHandle,
): BufferHandle {
  const angles = texelSolidAnglesF32(sizePx);
  const omega = made(
    engine.createBuffer({
      name: `${name} solid angles`,
      bytes: angles.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "sky-scratch",
    }),
  );
  engine.writeBuffer(omega, 0, angles);
  return omega;
}

/** Releases a buffer or a texture by its kind. */
function release(engine: RenderEngine, handle: BufferHandle | TextureHandle): void {
  if (handle.kind === "buffer") {
    engine.releaseBuffer(handle);
  } else {
    engine.releaseTexture(handle);
  }
}

/**
 * The CPU bake, as steps: its stars laid out, its levels splatted, summed and packed in a task,
 * and its cube made and written in a frame.
 */
function* bakeOnCpu(
  engine: RenderEngine,
  source: SkyBakeSource,
): Generator<BakeStepPlace, BakedCube | null, void> {
  const points = takePoints(source.stars.count);
  let packed: PackedCpuLevels;
  try {
    yield* layOutPoints(source, points);
    packed = packedLevelsOnCpu(points.subarray(BAKE_POINTS_HEADER), source.faceSizePx);
  } finally {
    givePoints(points);
  }
  // Its levels are written whole, as the GPU bake's uploads are: in a frame.
  yield "frame";
  return writtenCubeOnCpu(engine, packed, source);
}

/**
 * The bake on the CPU, the fallback without `float32-blendable`, and the harness's reference for
 * the GPU bake.
 */
export function bakeSkyCubeOnCpu(engine: RenderEngine, input: BakeInput): BakedCube {
  const source = bakeSourceOf(input);
  const points = new Float32Array(BAKE_POINTS_HEADER + source.stars.count * SPLAT_POINT_FLOATS);
  source.stars.writeInto(points, 0, source.stars.count);
  return writtenCubeOnCpu(
    engine,
    packedLevelsOnCpu(points.subarray(BAKE_POINTS_HEADER), source.faceSizePx),
    source,
  );
}

/** A CPU bake's packed levels, each the six faces' in turn, and the power of two they took. */
interface PackedCpuLevels {
  readonly levels: ReadonlyArray<Uint32Array>;
  readonly exponent: number;
}

/** The CPU bake's levels of stars in the splat's layout, without its header. */
function packedLevelsOnCpu(points: Float32Array, size: number): PackedCpuLevels {
  const sizes = mipSizes(size);
  const omegas = texelSolidAnglesSr(size);
  // The six faces at once: the fallback serves the low setting's 1,024² faces (some 100 MB here)
  // on adapters without float32-blendable; the high setting's devices have it.
  const faces = splatCpu(points, size);
  for (const face of faces) {
    divideBySolidAngle(face, size, omegas);
  }
  const exponent = peakScaleExponent(faces);
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
  return { levels, exponent };
}

/** The CPU bake's cube, made and written from its packed levels, and its peak. */
function writtenCubeOnCpu(
  engine: RenderEngine,
  packed: PackedCpuLevels,
  source: Pick<SkyBakeSource, "faceSizePx" | "name">,
): BakedCube {
  const size = source.faceSizePx;
  const cube = engine.createPackedCube(size, packed.levels.length, "sky-cube", source.name);
  let peak: BufferHandle | null = null;
  let made = false;
  try {
    packed.levels.forEach((level, index) => {
      engine.writePackedCubeLevel(cube, index, level);
    });
    peak = engine.createBuffer({
      name: `${source.name} peak`,
      bytes: 4,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | BUFFER_USAGE.COPY_SRC,
      category: "sky-cube",
    });
    // A peak whose `scaleExponent` is the exponent applied: 2^(15 − k).
    engine.writeBuffer(peak, 0, f32Bits(2 ** (15 - packed.exponent)));
    made = true;
    return { cube, peak, faceSizePx: size, path: "cpu" };
  } finally {
    // What a bake that fails partway made is released with it, as the GPU bake's is.
    if (!made) {
      engine.releaseTexture(cube);
      if (peak !== null) {
        engine.releaseBuffer(peak);
      }
    }
  }
}

/** Releases a baked cube and its peak (T14's view release). */
export function releaseBakedCube(engine: RenderEngine, baked: BakedCube): void {
  engine.releaseTexture(baked.cube);
  engine.releaseBuffer(baked.peak);
}
