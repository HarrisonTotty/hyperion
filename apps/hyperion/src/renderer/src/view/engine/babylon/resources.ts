/**
 * The engine's one path for GPU memory: every buffer and texture is made here, with its category,
 * and every creation, destruction and upload raises one {@link AllocationEvent} (R01 Design note
 * 18).
 *
 * @remarks
 * Buffers and textures are made on the engine's device directly, not through Babylon, so that each
 * has exactly the usage, format and mips its specification names, and so that their bytes are
 * known. A texture a material samples is wrapped for Babylon when it is first bound. R06's packed
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
import type { BufferHandle, TextureHandle } from "../types";
import { paddedBytesPerRow } from "./readback";

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

/** The specification of R06's packed star cube at a face size and mip count. */
export function packedCubeSpec(
  sizePx: number,
  mips: number,
  category: MemoryCategory,
): TextureSpec {
  return {
    name: "packed star cube",
    size: { width: sizePx, height: sizePx, depthOrArrayLayers: 6 },
    dimension: "cube",
    format: "rgb9e5ufloat",
    mips,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category,
  };
}

/** Every buffer and texture of one engine, by handle. */
export class ResourceRegistry {
  readonly #device: GPUDevice;
  readonly #emit: (event: AllocationEvent) => void;
  readonly #buffers = new Map<BufferHandle, BufferRecord>();
  readonly #textures = new Map<TextureHandle, TextureRecord>();

  /**
   * Makes a registry on `device`.
   *
   * @param emit - Called once for every creation, destruction and upload.
   */
  constructor(device: GPUDevice, emit: (event: AllocationEvent) => void) {
    this.#device = device;
    this.#emit = emit;
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

  /** The buffer behind `handle`, which must be this registry's. */
  bufferOf(handle: BufferHandle): BufferRecord {
    const record = this.#buffers.get(handle);
    if (record === undefined) {
      throw new Error(`buffer ${handle.name} was not made by this engine`);
    }
    return record;
  }

  /** The texture behind `handle`, which must be this registry's. */
  textureOf(handle: TextureHandle): TextureRecord {
    const record = this.#textures.get(handle);
    if (record === undefined) {
      throw new Error(`texture ${handle.name} was not made by this engine`);
    }
    return record;
  }

  /**
   * Writes `data` into a buffer at `offsetBytes`, through the queue.
   *
   * @remarks
   * A queue write takes effect before any command submitted after it, Babylon's pending frame
   * included, as WebGPU orders them.
   */
  writeBuffer(handle: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
    const { buffer } = this.bufferOf(handle);
    this.#device.queue.writeBuffer(
      buffer,
      offsetBytes,
      data.buffer,
      data.byteOffset,
      data.byteLength,
    );
    this.#emit({ kind: "uploaded", name: handle.name, bytes: data.byteLength });
  }

  /**
   * Writes `data` into a region of a texture, through the queue.
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
    this.#device.queue.writeTexture(
      { texture, origin, mipLevel },
      data,
      {
        offset: 0,
        bytesPerRow: extent.width * bytesPerTexel(spec.format),
        rowsPerImage: extent.height,
      },
      extent,
    );
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
   * (`paddedBytesPerRow(size, 4)`), as `copyBufferToTexture` requires.
   */
  encodePackedCubeLevelFromBuffer(
    encoder: Pick<GPUCommandEncoder, "copyBufferToTexture">,
    cube: TextureHandle,
    level: number,
    packed: BufferHandle,
  ): void {
    const faceTexels = this.#cubeLevelSize(cube, level);
    const { texture } = this.textureOf(cube);
    const { buffer, spec } = this.bufferOf(packed);
    const bytesPerRow = paddedBytesPerRow(faceTexels, 4);
    const needed = bytesPerRow * faceTexels * 5 + bytesPerRow * (faceTexels - 1) + faceTexels * 4;
    if (spec.bytes < needed) {
      throw new Error(`${packed.name} holds ${spec.bytes} bytes; level ${level} needs ${needed}`);
    }
    if ((spec.usage & BUFFER_USAGE.COPY_SRC) === 0) {
      throw new Error(`${packed.name} was not made with COPY_SRC`);
    }
    encoder.copyBufferToTexture(
      { buffer, bytesPerRow, rowsPerImage: faceTexels },
      { texture, mipLevel: level },
      [faceTexels, faceTexels, 6],
    );
  }

  /** Destroys one buffer, raising its `destroyed` event. */
  destroyBuffer(handle: BufferHandle): void {
    const { spec, buffer } = this.bufferOf(handle);
    buffer.destroy();
    this.#buffers.delete(handle);
    this.#emit({
      kind: "destroyed",
      name: handle.name,
      bytes: spec.bytes,
      category: spec.category,
    });
  }

  /** Destroys one texture, raising its `destroyed` event. */
  destroyTexture(handle: TextureHandle): void {
    const { spec, texture, bytes } = this.textureOf(handle);
    texture.destroy();
    this.#textures.delete(handle);
    this.#emit({ kind: "destroyed", name: handle.name, bytes, category: spec.category });
  }

  /** Destroys every buffer and texture, raising each one's `destroyed` event. */
  dispose(): void {
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
