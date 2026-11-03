/**
 * The two-pass occlusion's depth pyramid and its test (plan R14, R14.T3.d; Design note 4, as Karis
 * 2021, SIGGRAPH Advances, "A Deep Dive into Nanite Virtualized Geometry"): the pyramid a view
 * builds from its scene's depth after its first raster, the test by which the cull's late pass
 * refuses a cluster against it, each with its TypeScript twin, and {@link DepthPyramid}.
 *
 * @remarks
 * - **The pyramid.** Its level 0 is half the depth's resolution, each level after it half the one
 *   before, rounded up, to 1 × 1; each texel the farthest depth (reversed-Z's least) of every texel
 *   of the level below it covers, the odd row or column included (`hzb.wgsl`). Its levels lie in
 *   one `r32` storage buffer, row by row, not a texture's mips (lean, 2026-11-03): the engine binds a
 *   sampled texture whole, and a texture bound whole cannot be read while one of its levels is
 *   written. 1080p takes 2.8 MB.
 * - **The test** ({@link hzbRefuses}): a cluster's sphere, widened as the cull's camera test is,
 *   bounded on the screen by the box about it, its x ÷ z and y ÷ z extremes, its pixels widened by
 *   one each side; its nearest depth, the near plane over its nearest distance along the view axis.
 *   It is refused only where that depth lies behind the pyramid's farthest over every pixel of its
 *   rectangle, read at the first level where the rectangle spans at most two texels each way, so
 *   that each of its fragments fails the depth test against what the first raster drew. A sphere
 *   across the near plane, the camera inside it, or one wholly off the depth, is kept. Exact for the
 *   current frame: the first raster's geometry is this frame's.
 */

import type { Vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M } from "../camera/projection";
import type { Quaternion } from "../camera/pose";
import { rotate } from "../camera/quaternion";
import { BUFFER_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type {
  BufferHandle,
  ComputeHandle,
  RenderEngine,
  TextureHandle,
  ViewSize,
} from "../engine/types";
import { CULL_ROUNDING_FRACTION, CULL_ROUNDING_M } from "./cull";
import hzbWgsl from "./hzb.wgsl?raw";

/** The pyramid's kernel: one dispatch a level, a minimum exact on every path (`bit-exact`). */
export const DEPTH_PYRAMID_KERNEL: KernelPair = {
  name: "depth pyramid",
  reference: hzbWgsl,
  subgroup: null,
  readback: "bit-exact",
};

/** The pass label the pyramid's dispatches are timed under. */
export const DEPTH_PYRAMID_LABEL = "depth pyramid";

/** The kernel's workgroup's side: 8 × 8 texels. */
export const PYRAMID_WORKGROUP = 8;

/**
 * The most levels a pyramid has, as the cull's `HzbParams` holds their places: from half of 2¹⁶ px,
 * past every device's `maxTextureDimension2D`.
 */
export const HZB_MAX_LEVELS = 16;

/** Bytes of the cull's `HzbParams` (`clusterCull.wgsl`). */
export const HZB_PARAMS_BYTES = 128;

/** Bytes of the pyramid kernel's `PyramidParams`. */
export const PYRAMID_PARAMS_BYTES = 32;

/** One level of a pyramid: its size, texels, and its first texel in the pyramid's buffer. */
export interface PyramidLevel {
  readonly widthPx: number;
  readonly heightPx: number;
  readonly offset: number;
}

/**
 * The levels of the pyramid of a depth of `size`: ⌈W ÷ 2⌉ × ⌈H ÷ 2⌉ first, each after it the one
 * before halved and rounded up, to 1 × 1, one after another in the buffer.
 *
 * @throws RangeError for a size that is not a whole number of pixels from 1, or one whose pyramid
 *   would have more than {@link HZB_MAX_LEVELS} levels.
 */
export function pyramidLevels(size: ViewSize): ReadonlyArray<PyramidLevel> {
  const { widthPx, heightPx } = size;
  if (!Number.isInteger(widthPx) || !Number.isInteger(heightPx) || widthPx < 1 || heightPx < 1) {
    throw new RangeError(`a depth of ${String(widthPx)} × ${String(heightPx)} px has no pyramid`);
  }
  const levels: PyramidLevel[] = [];
  let width = widthPx;
  let height = heightPx;
  let offset = 0;
  do {
    width = Math.ceil(width / 2);
    height = Math.ceil(height / 2);
    levels.push({ widthPx: width, heightPx: height, offset });
    offset += width * height;
  } while (width > 1 || height > 1);
  if (levels.length > HZB_MAX_LEVELS) {
    throw new RangeError(
      `a depth of ${String(widthPx)} × ${String(heightPx)} px has ${String(levels.length)} levels, past ${String(HZB_MAX_LEVELS)}`,
    );
  }
  return levels;
}

/** The texels of every level of `levels`, the pyramid buffer's length. */
export function pyramidTexels(levels: ReadonlyArray<PyramidLevel>): number {
  const top = levels.at(-1);
  return top === undefined ? 0 : top.offset + top.widthPx * top.heightPx;
}

/**
 * The pyramid of `depth`, `size.widthPx` × `size.heightPx` texels row by row (reversed-Z), as
 * `hzb.wgsl` builds it: every level, one after another, each texel the least of the up to four
 * texels below it covers.
 */
export function depthPyramidTwin(depth: Float32Array, size: ViewSize): Float32Array {
  const levels = pyramidLevels(size);
  const out = new Float32Array(pyramidTexels(levels));
  let source = depth;
  let sourceOffset = 0;
  let sourceWidth = size.widthPx;
  let sourceHeight = size.heightPx;
  for (const level of levels) {
    for (let y = 0; y < level.heightPx; y += 1) {
      for (let x = 0; x < level.widthPx; x += 1) {
        const x0 = 2 * x;
        const y0 = 2 * y;
        const x1 = Math.min(x0 + 1, sourceWidth - 1);
        const y1 = Math.min(y0 + 1, sourceHeight - 1);
        const at = (sx: number, sy: number): number =>
          source[sourceOffset + sy * sourceWidth + sx] ?? Number.NaN;
        out[level.offset + y * level.widthPx + x] = Math.min(
          at(x0, y0),
          at(x1, y0),
          at(x0, y1),
          at(x1, y1),
        );
      }
    }
    source = out;
    sourceOffset = level.offset;
    sourceWidth = level.widthPx;
    sourceHeight = level.heightPx;
  }
  return out;
}

/** A view's axes in the frame its cull's positions are in: right, up and forward. */
export interface HzbAxes {
  readonly right: Vec3;
  readonly up: Vec3;
  readonly forward: Vec3;
}

/** `orientation`'s axes, as R02's projection takes them (`projection.ts`' `projector`). */
export function hzbAxesOf(orientation: Quaternion): HzbAxes {
  return {
    right: rotate(orientation, { x: 1, y: 0, z: 0 }),
    up: rotate(orientation, { x: 0, y: 1, z: 0 }),
    forward: rotate(orientation, { x: 0, y: 0, z: -1 }),
  };
}

/**
 * The projection a pyramid's depth was drawn with: its x and y scales, 1 ÷ tan(fov_x ÷ 2) and
 * that times the aspect (`perspectiveReversedInfinite`'s), with the near plane its own.
 */
export interface PyramidProjection {
  readonly scaleX: number;
  readonly scaleY: number;
}

/** The projection of a horizontal field of `fovXRad` over `size` (`perspectiveReversedInfinite`'s). */
export function pyramidProjectionOf(fovXRad: number, size: ViewSize): PyramidProjection {
  const scaleX = 1 / Math.tan(fovXRad / 2);
  return { scaleX, scaleY: (scaleX * size.widthPx) / size.heightPx };
}

/** What the occlusion's test reads: a pyramid, the depth's size and projection, and the axes. */
export interface HzbTestInput {
  /** Every level's texels, as {@link depthPyramidTwin} gives them. */
  readonly texels: Float32Array;
  readonly levels: ReadonlyArray<PyramidLevel>;
  /** The depth's size, px. */
  readonly size: ViewSize;
  readonly projection: PyramidProjection;
  readonly axes: HzbAxes;
}

/** A sphere's rectangle on the depth, px inclusive, and its nearest depth (see the module's head). */
export interface HzbFootprint {
  readonly x0: number;
  readonly y0: number;
  readonly x1: number;
  readonly y1: number;
  readonly nearestDepth: number;
}

/** The pixel column or row of an NDC coordinate on a side of `side` px, held within two of it. */
function pixelOf(ndc: number, side: number): number {
  return Math.floor(Math.min(Math.max(((ndc + 1) / 2) * side, -2), side + 2));
}

/**
 * The rectangle and nearest depth of a sphere at `centreM`, camera-relative in the axes' frame, of
 * radius `radiusM`, widened by `slackScale` times the cull's rounding allowance; `null` where it is
 * kept untested: across the near plane, about the camera, or wholly off the depth.
 */
export function hzbFootprint(
  input: Pick<HzbTestInput, "size" | "projection" | "axes">,
  centreM: Vec3,
  radiusM: number,
  slackScale = 1,
): HzbFootprint | null {
  const { right, up, forward } = input.axes;
  const { widthPx: w, heightPx: h } = input.size;
  const length = Math.hypot(centreM.x, centreM.y, centreM.z);
  const r = radiusM + slackScale * (CULL_ROUNDING_FRACTION * length + CULL_ROUNDING_M);
  const x = centreM.x * right.x + centreM.y * right.y + centreM.z * right.z;
  const y = centreM.x * up.x + centreM.y * up.y + centreM.z * up.z;
  const z = centreM.x * forward.x + centreM.y * forward.y + centreM.z * forward.z;
  const nearZ = z - r;
  if (!(nearZ > NEAR_PLANE_M)) {
    return null;
  }
  const farZ = z + r;
  // The box's extremes of x ÷ z and y ÷ z: each least over the box at its far face where it is
  // positive, its near face where it is negative, and the most the other way.
  const u0 = (x - r) / (x - r >= 0 ? farZ : nearZ);
  const u1 = (x + r) / (x + r >= 0 ? nearZ : farZ);
  const v0 = (y - r) / (y - r >= 0 ? farZ : nearZ);
  const v1 = (y + r) / (y + r >= 0 ? nearZ : farZ);
  const { scaleX, scaleY } = input.projection;
  const x0 = pixelOf(scaleX * u0, w) - 1;
  const x1 = pixelOf(scaleX * u1, w) + 1;
  const y0 = pixelOf(-scaleY * v1, h) - 1;
  const y1 = pixelOf(-scaleY * v0, h) + 1;
  if (x1 < 0 || y1 < 0 || x0 > w - 1 || y0 > h - 1) {
    return null;
  }
  return {
    x0: Math.max(x0, 0),
    y0: Math.max(y0, 0),
    x1: Math.min(x1, w - 1),
    y1: Math.min(y1, h - 1),
    nearestDepth: NEAR_PLANE_M / nearZ,
  };
}

/**
 * The pyramid's farthest depth over the depth's pixels `footprint` spans, read at the first level
 * where they span at most two texels each way (`clusterCull.wgsl`'s `hzb_farthest`).
 */
export function hzbFarthest(
  input: Pick<HzbTestInput, "texels" | "levels">,
  footprint: Pick<HzbFootprint, "x0" | "y0" | "x1" | "y1">,
): number {
  const tx0 = footprint.x0 >> 1;
  const ty0 = footprint.y0 >> 1;
  const tx1 = footprint.x1 >> 1;
  const ty1 = footprint.y1 >> 1;
  let level = 0;
  while (
    level < input.levels.length - 1 &&
    ((tx1 >> level) - (tx0 >> level) > 1 || (ty1 >> level) - (ty0 >> level) > 1)
  ) {
    level += 1;
  }
  const at = input.levels[level];
  if (at === undefined) {
    return Number.NaN;
  }
  let farthest = Number.POSITIVE_INFINITY;
  for (let ty = ty0 >> level; ty <= ty1 >> level; ty += 1) {
    for (let tx = tx0 >> level; tx <= tx1 >> level; tx += 1) {
      farthest = Math.min(farthest, input.texels[at.offset + ty * at.widthPx + tx] ?? Number.NaN);
    }
  }
  return farthest;
}

/**
 * Whether the late pass refuses a sphere at `centreM`, camera-relative, of radius `radiusM` against
 * `input`'s pyramid (`clusterCull.wgsl`'s `hzb_refuses`; see the module's head): its nearest depth
 * behind the farthest over its rectangle, the sphere widened by `slackScale` times the cull's
 * rounding allowance, so that at 0 it is the exact test and at 1 the kernel's.
 */
export function hzbRefuses(
  input: HzbTestInput,
  centreM: Vec3,
  radiusM: number,
  slackScale = 1,
): boolean {
  const footprint = hzbFootprint(input, centreM, radiusM, slackScale);
  return footprint !== null && footprint.nearestDepth < hzbFarthest(input, footprint);
}

/** The pyramid a cull's late pass reads, as {@link packHzbParams} packs it. */
export interface HzbPassInput {
  readonly levels: ReadonlyArray<PyramidLevel>;
  readonly size: ViewSize;
  readonly projection: PyramidProjection;
  readonly axes: HzbAxes;
}

/**
 * Packs `input` into `out` ({@link HZB_PARAMS_BYTES} at least), as `clusterCull.wgsl`'s
 * `HzbParams`: the right, up and forward axes, each with its `w` the projection's x scale, its y
 * scale and the near plane; the depth's width and height, px, and the levels; and each level's
 * first texel. `null` packs no levels, which refuses nothing.
 *
 * @throws RangeError for more than {@link HZB_MAX_LEVELS} levels.
 */
export function packHzbParams(input: HzbPassInput | null, out: ArrayBuffer): void {
  const view = new DataView(out);
  new Uint8Array(out, 0, HZB_PARAMS_BYTES).fill(0);
  if (input === null) {
    return;
  }
  if (input.levels.length > HZB_MAX_LEVELS) {
    throw new RangeError(`a pyramid of ${String(input.levels.length)} levels`);
  }
  const { right, up, forward } = input.axes;
  const rows: ReadonlyArray<readonly [Vec3, number]> = [
    [right, input.projection.scaleX],
    [up, input.projection.scaleY],
    [forward, NEAR_PLANE_M],
  ];
  rows.forEach(([axis, w], k) => {
    view.setFloat32(16 * k, axis.x, true);
    view.setFloat32(16 * k + 4, axis.y, true);
    view.setFloat32(16 * k + 8, axis.z, true);
    view.setFloat32(16 * k + 12, w, true);
  });
  view.setUint32(48, input.size.widthPx, true);
  view.setUint32(52, input.size.heightPx, true);
  view.setUint32(56, input.levels.length, true);
  input.levels.forEach((level, n) => {
    view.setUint32(64 + 4 * n, level.offset, true);
  });
}

/**
 * Packs the pyramid kernel's `PyramidParams` for `levels[level]` of the pyramid of a depth of
 * `size`: its source (the depth at level 0, else the level before) and itself.
 */
export function packPyramidParams(
  levels: ReadonlyArray<PyramidLevel>,
  size: ViewSize,
  level: number,
): Uint32Array {
  const target = levels[level];
  const source = level === 0 ? null : levels[level - 1];
  if (target === undefined || source === undefined) {
    throw new RangeError(`no level ${String(level)} of ${String(levels.length)}`);
  }
  return Uint32Array.of(
    source === null ? size.widthPx : source.widthPx,
    source === null ? size.heightPx : source.heightPx,
    level,
    source === null ? 0 : source.offset,
    target.widthPx,
    target.heightPx,
    target.offset,
    0,
  );
}

/** The part of the engine a pyramid uses. */
export type DepthPyramidEngine = Pick<RenderEngine, "createBuffer" | "releaseBuffer" | "dispatch">;

/** A pyramid's latest build, as a cull's late pass reads it. */
export interface PyramidFrame {
  readonly buffer: BufferHandle;
  readonly levels: ReadonlyArray<PyramidLevel>;
  readonly size: ViewSize;
  readonly projection: PyramidProjection;
}

/**
 * A view-frame's two-pass occlusion, as its renderer runs it (plan R14, R14.T3.d): the view's
 * pyramid, which it builds from its first raster's depth before it asks for the late draws, and
 * whether the last frame's visibility no longer holds ({@link DepthPyramid.begin}'s: the view's
 * first frame, a resize or a change of internal scale, a cut).
 */
export interface FrameOcclusion {
  readonly pyramid: Pick<DepthPyramid, "builds" | "frame">;
  readonly fresh: boolean;
}

/**
 * One view's depth pyramid (plan R14's `DepthPyramid`; see the module's head), its buffer made at
 * the first size it is begun at and made again at each new size, under `geometry-clusters`.
 *
 * @remarks
 * A frame {@link DepthPyramid.begin}s it before its first raster, which says whether the last
 * frame's visibility still holds, and {@link DepthPyramid.build}s it from that raster's depth. The
 * visibility no longer holds at its first frame, at a size not its last build's (a resize, a change
 * of internal scale) and at a cut the frame names; {@link DepthPyramid.builds} lets each pass that
 * keeps a visibility of its own see a frame it did not draw (a hand-over, a view's switch).
 */
export class DepthPyramid {
  readonly #engine: DepthPyramidEngine;
  readonly #kernel: ComputeHandle;
  readonly #name: string;
  #buffer: BufferHandle | null = null;
  #levels: ReadonlyArray<PyramidLevel> = [];
  #size: ViewSize | null = null;
  #frame: PyramidFrame | null = null;
  #builds = 0;
  #disposed = false;

  constructor(engine: DepthPyramidEngine, kernel: ComputeHandle, name: string) {
    this.#engine = engine;
    this.#kernel = kernel;
    this.#name = name;
  }

  /** How many times it has been built: a pass that drew its last frame saw one fewer. */
  get builds(): number {
    return this.#builds;
  }

  /** Its latest build, or `null` before one, or since a {@link DepthPyramid.begin} at a new size. */
  get frame(): PyramidFrame | null {
    return this.#frame;
  }

  /**
   * Begins a frame at `size`, the depth's, making the buffer again at a new size.
   *
   * @param cut - Whether the frame is a camera's cut (`present: false`, a dropped histogram).
   * @returns Whether the last frame's visibility no longer holds: the first frame, a new size, or
   *   a cut.
   */
  begin(size: ViewSize, cut: boolean): boolean {
    if (this.#disposed) {
      throw new Error(`the depth pyramid ${this.#name} was disposed`);
    }
    const same =
      this.#size !== null &&
      this.#size.widthPx === size.widthPx &&
      this.#size.heightPx === size.heightPx;
    if (!same) {
      const levels = pyramidLevels(size);
      if (this.#buffer !== null) {
        this.#engine.releaseBuffer(this.#buffer);
      }
      this.#buffer = this.#engine.createBuffer({
        name: `${this.#name} depth pyramid`,
        bytes: 4 * pyramidTexels(levels),
        // COPY_SRC for the smoke harness's twin check, which reads it back.
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
        category: "geometry-clusters",
      });
      this.#levels = levels;
      this.#size = size;
      this.#frame = null;
    }
    return !same || cut || this.#builds === 0;
  }

  /**
   * Builds every level from `depth`, the size {@link DepthPyramid.begin} was given, drawn with
   * `projection`: a dispatch a level, each under {@link DEPTH_PYRAMID_LABEL}.
   *
   * @throws Error before a {@link DepthPyramid.begin}, or once disposed.
   */
  build(depth: TextureHandle, projection: PyramidProjection): void {
    const buffer = this.#buffer;
    const size = this.#size;
    if (this.#disposed || buffer === null || size === null) {
      throw new Error(`the depth pyramid ${this.#name} was built before it was begun`);
    }
    this.#levels.forEach((level, n) => {
      this.#engine.dispatch(
        this.#kernel,
        {
          uniforms: { params: packPyramidParams(this.#levels, size, n) },
          buffers: { pyramid: buffer },
          sampled: { depth },
          storage: {},
        },
        [
          Math.ceil(level.widthPx / PYRAMID_WORKGROUP),
          Math.ceil(level.heightPx / PYRAMID_WORKGROUP),
          1,
        ],
        DEPTH_PYRAMID_LABEL,
      );
    });
    this.#builds += 1;
    this.#frame = { buffer, levels: this.#levels, size, projection };
  }

  /** Releases the buffer; nothing more is built. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#frame = null;
    if (this.#buffer !== null) {
      this.#engine.releaseBuffer(this.#buffer);
      this.#buffer = null;
    }
  }
}
