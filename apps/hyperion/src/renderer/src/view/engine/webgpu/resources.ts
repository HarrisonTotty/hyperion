/**
 * The engine's one path for GPU memory: every buffer, texture and query set is made here, with its
 * category, and every creation, destruction and upload raises one {@link AllocationEvent} (R01
 * Design note 18).
 *
 * @remarks
 * Buffers and textures are made on the engine's device here and nowhere else, so that each has
 * exactly the usage, format and mips its specification names, and so that their bytes are known.
 * R06's packed
 * star cube is an `rgb9e5ufloat` cube made the same way, its levels written by
 * `queue.writeTexture` from the CPU or `copyBufferToTexture` from a kernel's buffer (Design note
 * 21).
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import {
  type AllocationEvent,
  bytesPerTexel,
  extentOf,
  type BufferSpec,
  type MemoryCategory,
  textureBytes,
  type TextureSpec,
} from "../memory";
import type { BufferHandle, TexelRect, TextureHandle } from "../types";
import { paddedBytesPerRow } from "./readback";
import { StagedWrites } from "./stagedWrites";

/** A buffer the registry made. */
export interface BufferRecord {
  readonly spec: BufferSpec;
  readonly buffer: GPUBuffer;
}

/** A texture the registry made, with its bytes. */
export interface TextureRecord {
  readonly spec: TextureSpec;
  readonly texture: GPUTexture;
  readonly bytes: number;
}

/** The texture view dimension of each `TextureSpec` dimension. */
const VIEW_DIMENSIONS: Readonly<Record<TextureSpec["dimension"], GPUTextureViewDimension>> = {
  "2d": "2d",
  "3d": "3d",
  cube: "cube",
};

/** The `GPUTextureDimension` a `TextureSpec` dimension is made with: a cube is six 2D layers. */
function textureDimension(dimension: TextureSpec["dimension"]): GPUTextureDimension {
  return dimension === "3d" ? "3d" : "2d";
}

/** The view dimension a texture is sampled through: a layered 2D texture is a `2d-array`. */
export function viewDimensionOf(spec: TextureSpec): GPUTextureViewDimension {
  if (spec.dimension === "2d" && extentOf(spec.size).depthOrArrayLayers > 1) {
    return "2d-array";
  }
  return VIEW_DIMENSIONS[spec.dimension];
}

/**
 * Whether a texture whose own view dimension is `own` binds where a layout declares `declared`:
 * the one rule of the material and compute paths.
 *
 * @remarks
 * The same dimension always binds. A single-layer 2D texture (`own` `2d`) also binds as a
 * one-layer `2d-array`, a view WebGPU allows (W3C WebGPU, §6.2.1 "Texture View Creation":
 * `"2d-array"` asks only that the texture be `"2d"`, whatever its layer count), so that a
 * material declaring an array binds it whatever its layer count (R05.T11.a's normals atlas, whose
 * layers depend on the device's limits), and a kernel declaring `texture_2d_array` or
 * `texture_storage_2d_array` binds a one-layer table (R08.T0). Nothing else is reinterpreted.
 */
export function viewDimensionBinds(
  declared: GPUTextureViewDimension,
  own: GPUTextureViewDimension,
): boolean {
  return own === declared || (declared === "2d-array" && own === "2d");
}

/** The name a packed star cube takes when its caller names none. */
export const PACKED_CUBE_NAME = "packed star cube";

/** The specification of R06's packed star cube at a face size and mip count. */
export function packedCubeSpec(
  sizePx: number,
  mips: number,
  category: MemoryCategory,
  name: string = PACKED_CUBE_NAME,
): TextureSpec {
  return {
    name,
    size: { width: sizePx, height: sizePx, depthOrArrayLayers: 6 },
    dimension: "cube",
    format: "rgb9e5ufloat",
    mips,
    // COPY_SRC for the harness's round trips (T9.g); it costs no memory.
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST | TEXTURE_USAGE.COPY_SRC,
    category,
  };
}

/** Every buffer and texture of one engine, by handle. */
export class ResourceRegistry {
  readonly #device: GPUDevice;
  readonly #emit: (event: AllocationEvent) => void;
  readonly #buffers = new Map<BufferHandle, BufferRecord>();
  readonly #textures = new Map<TextureHandle, TextureRecord>();
  /** Every query set made, with its name. */
  readonly #querySets = new Map<GPUQuerySet, string>();
  /** Every handle destroyed before disposal, so that a later use names the release. */
  readonly #released = new WeakSet<BufferHandle | TextureHandle>();
  /** The writes of the open staged scopes ({@link ResourceRegistry.staged}). */
  readonly #staged: StagedWrites;

  /**
   * Makes a registry on `device`.
   *
   * @param emit - Called once for every creation, destruction and upload.
   * @param stagingPartBytes - The most bytes staged before they are submitted (`stagedWrites.ts`'
   *   `STAGING_PART_BYTES` but in a test).
   */
  constructor(
    device: GPUDevice,
    emit: (event: AllocationEvent) => void,
    stagingPartBytes?: number,
  ) {
    this.#device = device;
    this.#emit = emit;
    this.#staged = new StagedWrites(device, stagingPartBytes);
  }

  /**
   * Runs `write` with every buffer and texture write made through the registry staged
   * (`stagedWrites.ts`; lane perf-page-long-tasks), submitted before the engine's next submission
   * ({@link ResourceRegistry.flushStaged}), destruction or write outside a scope.
   */
  staged<T>(label: string, write: () => T): T {
    return this.#staged.run(label, write);
  }

  /** Submits the writes staged so far, before a submission of the engine's; nothing with none. */
  flushStaged(): void {
    this.#staged.flush();
  }

  /** Makes a buffer of `spec`. */
  createBuffer(spec: BufferSpec): BufferHandle {
    // The one place a buffer is made (Design note 18).
    const buffer = this.#device.createBuffer({
      label: spec.name,
      size: spec.bytes,
      usage: spec.usage,
    });
    const handle: BufferHandle = Object.freeze({
      kind: "buffer",
      name: spec.name,
      bytes: spec.bytes,
    });
    this.#buffers.set(handle, { spec, buffer });
    this.#emit({ kind: "created", name: spec.name, bytes: spec.bytes, category: spec.category });
    return handle;
  }

  /** Makes a texture of `spec`. */
  createTexture(spec: TextureSpec): TextureHandle {
    const { width, height, depthOrArrayLayers } = extentOf(spec.size);
    // The one place a texture is made (Design note 18).
    const texture = this.#device.createTexture({
      label: spec.name,
      size: {
        width,
        height,
        depthOrArrayLayers: spec.dimension === "cube" ? 6 : depthOrArrayLayers,
      },
      dimension: textureDimension(spec.dimension),
      format: spec.format,
      mipLevelCount: spec.mips,
      usage: spec.usage,
    });
    const bytes = textureBytes(spec);
    const handle: TextureHandle = Object.freeze({ kind: "texture", name: spec.name });
    this.#textures.set(handle, { spec, texture, bytes });
    this.#emit({ kind: "created", name: spec.name, bytes, category: spec.category });
    return handle;
  }

  /**
   * Makes a query set, counted as `other` at 8 bytes a query (a timestamp's size), and destroyed
   * at disposal.
   */
  createQuerySet(descriptor: GPUQuerySetDescriptor): GPUQuerySet {
    // The one place a query set is made (Design note 18).
    const querySet = this.#device.createQuerySet(descriptor);
    const name = descriptor.label ?? "query set";
    this.#querySets.set(querySet, name);
    this.#emit({ kind: "created", name, bytes: descriptor.count * 8, category: "other" });
    return querySet;
  }

  /** The buffer behind `handle`, which must be this registry's. */
  bufferOf(handle: BufferHandle): BufferRecord {
    const record = this.#buffers.get(handle);
    if (record === undefined) {
      if (this.#released.has(handle)) {
        throw new Error(`buffer ${handle.name} was released`);
      }
      throw new Error(`buffer ${handle.name} was not made by this engine`);
    }
    return record;
  }

  /** The texture behind `handle`, which must be this registry's. */
  textureOf(handle: TextureHandle): TextureRecord {
    const record = this.#textures.get(handle);
    if (record === undefined) {
      if (this.#released.has(handle)) {
        throw new Error(`texture ${handle.name} was released`);
      }
      throw new Error(`texture ${handle.name} was not made by this engine`);
    }
    return record;
  }

  /**
   * Writes `data` into a buffer at `offsetBytes`, through the queue, or staged inside a staged
   * scope ({@link ResourceRegistry.staged}).
   *
   * @remarks
   * A queue write takes effect before any command submitted after it, as WebGPU orders them; a
   * staged write is submitted before any of the engine's submissions after it, which orders it the
   * same.
   */
  writeBuffer(handle: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
    const { buffer } = this.bufferOf(handle);
    if (!(this.#staged.open && this.#staged.buffer(buffer, offsetBytes, data))) {
      // A write the copies cannot make goes to the queue after what is staged, as before.
      this.#staged.flush();
      this.#device.queue.writeBuffer(
        buffer,
        offsetBytes,
        data.buffer,
        data.byteOffset,
        data.byteLength,
      );
    }
    this.#emit({ kind: "uploaded", name: handle.name, bytes: data.byteLength });
  }

  /**
   * Writes `data` into a region of a texture, through the queue, or staged inside a staged scope.
   *
   * @remarks
   * Rows of `data` are tightly packed: `size.width` texels of the format each.
   */
  writeTexture(
    handle: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
    mipLevel = 0,
  ): void {
    const { texture, spec } = this.textureOf(handle);
    const extent = extentOf(size);
    const rowBytes = extent.width * bytesPerTexel(spec.format);
    if (!(
      this.#staged.open && this.#staged.texture(texture, origin, mipLevel, extent, rowBytes, data)
    )) {
      this.#staged.flush();
      this.#device.queue.writeTexture(
        { texture, origin, mipLevel },
        data,
        { offset: 0, bytesPerRow: rowBytes, rowsPerImage: extent.height },
        extent,
      );
    }
    this.#emit({ kind: "uploaded", name: handle.name, bytes: data.byteLength });
  }

  /**
   * Writes one level of a packed cube from the CPU: six faces, each `size × size` texels at that
   * level, face after face, rows tightly packed.
   *
   * @throws Error when `packed` does not hold exactly the level's texels.
   */
  writePackedCubeLevel(cube: TextureHandle, level: number, packed: Uint32Array): void {
    const faceTexels = this.#cubeLevelSize(cube, level);
    if (packed.length !== faceTexels * faceTexels * 6) {
      throw new Error(
        `level ${level} of ${cube.name} holds ${faceTexels * faceTexels * 6} texels, not ${packed.length}`,
      );
    }
    this.writeTexture(cube, [0, 0, 0], [faceTexels, faceTexels, 6], packed, level);
  }

  /**
   * Encodes the copy of one level of a packed cube from a kernel's buffer, with no readback.
   *
   * @remarks
   * The buffer holds six faces, face after face, each row padded to 256 bytes
   * (`paddedBytesPerRow(size, 4)`), as `copyBufferToTexture` requires; or, given `face`, that one
   * face alone (R06.T13.g); or, given a `region` of it too, those texels alone, each row padded
   * to 256 bytes (R06's tiled bake).
   *
   * @throws Error for a face outside 0–5, a region without a face or outside the level, a buffer
   *   too small or without `COPY_SRC`.
   */
  encodePackedCubeLevelFromBuffer(
    encoder: Pick<GPUCommandEncoder, "copyBufferToTexture">,
    cube: TextureHandle,
    level: number,
    packed: BufferHandle,
    face?: number,
    region?: TexelRect,
  ): void {
    const faceTexels = this.#cubeLevelSize(cube, level);
    const { texture } = this.textureOf(cube);
    const { buffer, spec } = this.bufferOf(packed);
    if (face !== undefined && !(Number.isInteger(face) && face >= 0 && face < 6)) {
      throw new Error(`${cube.name} has no face ${face}`);
    }
    if (region !== undefined && face === undefined) {
      throw new Error(`a region of ${cube.name} is written on one face`);
    }
    const { x, y, width, height } = region ?? {
      x: 0,
      y: 0,
      width: faceTexels,
      height: faceTexels,
    };
    if (
      !(Number.isInteger(x) && Number.isInteger(y) && x >= 0 && y >= 0) ||
      !(Number.isInteger(width) && Number.isInteger(height) && width >= 1 && height >= 1) ||
      x + width > faceTexels ||
      y + height > faceTexels
    ) {
      throw new Error(
        `the region is outside level ${level} of ${cube.name}, ${faceTexels} × ${faceTexels}`,
      );
    }
    const faces = face === undefined ? 6 : 1;
    const bytesPerRow = paddedBytesPerRow(width, 4);
    const needed = bytesPerRow * height * (faces - 1) + bytesPerRow * (height - 1) + width * 4;
    if (spec.bytes < needed) {
      throw new Error(`${packed.name} holds ${spec.bytes} bytes; level ${level} needs ${needed}`);
    }
    if ((spec.usage & BUFFER_USAGE.COPY_SRC) === 0) {
      throw new Error(`${packed.name} was not made with COPY_SRC`);
    }
    encoder.copyBufferToTexture(
      { buffer, bytesPerRow, rowsPerImage: height },
      { texture, mipLevel: level, origin: [x, y, face ?? 0] },
      [width, height, faces],
    );
  }

  /**
   * Destroys one buffer, raising its `destroyed` event, the writes staged so far submitted first,
   * as a queue write made before the destruction would have been.
   */
  destroyBuffer(handle: BufferHandle): void {
    const { spec, buffer } = this.bufferOf(handle);
    this.#staged.flush();
    buffer.destroy();
    this.#buffers.delete(handle);
    this.#released.add(handle);
    this.#emit({
      kind: "destroyed",
      name: handle.name,
      bytes: spec.bytes,
      category: spec.category,
    });
  }

  /** Destroys one texture, raising its `destroyed` event, the writes staged so far submitted first. */
  destroyTexture(handle: TextureHandle): void {
    const { spec, texture, bytes } = this.textureOf(handle);
    this.#staged.flush();
    texture.destroy();
    this.#textures.delete(handle);
    this.#released.add(handle);
    this.#emit({ kind: "destroyed", name: handle.name, bytes, category: spec.category });
  }

  /**
   * Destroys every buffer, texture and query set, raising each one's `destroyed` event; writes
   * staged and not yet submitted are dropped with them.
   */
  dispose(): void {
    this.#staged.discard();
    for (const [querySet, name] of this.#querySets) {
      querySet.destroy();
      this.#emit({ kind: "destroyed", name, bytes: querySet.count * 8, category: "other" });
    }
    this.#querySets.clear();
    for (const [handle, { spec, buffer }] of this.#buffers) {
      buffer.destroy();
      this.#emit({
        kind: "destroyed",
        name: handle.name,
        bytes: spec.bytes,
        category: spec.category,
      });
    }
    for (const [handle, { spec, texture, bytes }] of this.#textures) {
      texture.destroy();
      this.#emit({ kind: "destroyed", name: handle.name, bytes, category: spec.category });
    }
    this.#buffers.clear();
    this.#textures.clear();
  }

  #cubeLevelSize(cube: TextureHandle, level: number): number {
    const { spec } = this.textureOf(cube);
    if (spec.dimension !== "cube" || spec.format !== "rgb9e5ufloat") {
      throw new Error(`${cube.name} is not a packed cube`);
    }
    if (!Number.isInteger(level) || level < 0 || level >= spec.mips) {
      throw new Error(`${cube.name} has no level ${level}`);
    }
    return Math.max(1, extentOf(spec.size).width >> level);
  }
}

/** The usage a uniform buffer the engine makes for a kernel has. */
export const UNIFORM_BUFFER_USAGE = BUFFER_USAGE.UNIFORM | BUFFER_USAGE.COPY_DST;
