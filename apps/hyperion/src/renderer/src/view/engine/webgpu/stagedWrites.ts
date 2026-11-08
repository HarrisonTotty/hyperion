/**
 * The engine's staged writes (lane perf-page-long-tasks): buffer and texture writes whose bytes go
 * to the GPU through one buffer mapped at its creation and copies from it, rather than through the
 * queue's own writes.
 *
 * @remarks
 * `GPUQueue.writeBuffer` and `writeTexture` pass their bytes through the GPU process's command
 * stream, and the page's thread waits once that stream is full until the GPU process has read it,
 * which it does only between its own tasks. Behind GPU frames of seconds (the inspection's high
 * setting on SwiftShader, 5 to 36 s a frame), a burst of a patch's uploads stopped the page's thread
 * for up to 48 s, 86% of a run, in `writeBuffer` alone; 60 MB of queue writes held a probe's page
 * 1.0 to 3.7 s behind 1 s frames, where the same bytes staged held it 11 to 59 ms. A buffer mapped
 * at its creation holds its bytes in memory shared with the GPU process, so its unmapping and its
 * copies are commands of a few bytes, and a staged write never waits.
 *
 * The bytes are copied as each write is made, straight into a mapped staging buffer, so that a
 * caller may reuse its array at once, as it may after a queue write. The copies are encoded in the order the writes were made and submitted
 * in one command buffer with the engine's next submission, which goes after them, or before the
 * engine's next destruction or queue write outside a scope, or as a part fills
 * ({@link StagedWrites.flush}): nothing on the GPU reads a resource but in a submission, so every
 * staged write takes effect before anything that could read it, as a queue write does, and the
 * buffers and textures hold the same bytes for every command that reads them. A texture's rows are
 * laid out 256-byte aligned for `copyBufferToTexture`; the bytes that reach the texture are the
 * rows given.
 *
 * The copies wait for the engine's next submission, rather than going at the scope's end, because
 * the WebGPU calls that make and submit them can wait too: behind a SwiftShader frame of 16 s, a
 * height worker's reply waited 10 and 14 s in the staging's own `queue.submit` (lane
 * perf-page-long-tasks, part 2). The engine's next submission is its frame's, drawn once the GPU
 * process has finished the frame before, so they are made when it is free.
 *
 * The staging buffers are made, mapped, as the writes are staged, each of a chunk's bytes or a
 * write's own, rather than as one buffer of everything staged when the copies go: the copies wait
 * for the frame, a frame on SwiftShader comes every 5 to 20 s, and one buffer made and filled with
 * the uploads of those seconds in the frame's callback took 50 to 153 ms of it, in about 20 long
 * tasks of a run at high (lane perf-long-tasks-check). A chunk's buffer is made in the task that
 * stages into it, in a few milliseconds, and its bytes are copied once, not gathered first and
 * copied again.
 *
 * A write the copies cannot make (unaligned, out of bounds, to a resource without `COPY_DST`, of a
 * depth or stencil format) is not staged: the registry submits what is staged and hands it to the
 * queue, which writes it, or refuses it alone, as before, rather than in a copy that would void the
 * submission.
 *
 * The staging buffers are transient, made as writes are staged and destroyed by the submission
 * that copies from them, and are not GPU memory a view keeps, so they raise no allocation event, as
 * a readback's staging buffer raises none.
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../gpuFlags";
import type { Extent } from "../memory";

/** The alignment `copyBufferToTexture` asks of a staged texture's rows, bytes. */
const ROW_ALIGNMENT = 256;

/** The alignment `copyBufferToBuffer` asks of offsets and sizes, bytes. */
const COPY_ALIGNMENT = 4;

/**
 * The most bytes staged before what is staged is submitted, and the writes after it go into the
 * next part, 64 MiB: a scope of larger uploads is submitted in parts, in order, each within the
 * device's least `maxBufferSize` (256 MiB).
 */
export const STAGING_PART_BYTES = 64 * 1024 * 1024;

/**
 * The bytes of one staging buffer, made mapped as a write first needs it, 4 MiB (or the part's,
 * where that is less); a write of more has a buffer of its own bytes.
 */
export const STAGING_CHUNK_BYTES = 4 * 1024 * 1024;

/** One staging buffer, mapped at its creation, and the bytes of it staged so far. */
interface StagingChunk {
  readonly buffer: GPUBuffer;
  readonly bytes: Uint8Array;
  used: number;
}

/** One staged write: where its bytes lie in a staging buffer, and where they go. */
type StagedCopy =
  | {
      readonly kind: "buffer";
      readonly from: GPUBuffer;
      readonly fromBytes: number;
      readonly buffer: GPUBuffer;
      readonly offsetBytes: number;
      readonly bytes: number;
    }
  | {
      readonly kind: "texture";
      readonly from: GPUBuffer;
      readonly fromBytes: number;
      readonly texture: GPUTexture;
      readonly origin: GPUOrigin3DDict;
      readonly mipLevel: number;
      readonly extent: Extent;
      readonly bytesPerRow: number;
    };

/** `bytes` rounded up to a multiple of `alignment`. */
function aligned(bytes: number, alignment: number): number {
  return Math.ceil(bytes / alignment) * alignment;
}

/** An origin as `{ x, y, z }`, copied as the write is made; `null` for a coordinate not whole. */
function originOf(origin: GPUOrigin3D): Required<GPUOrigin3DDict> | null {
  const [x = 0, y = 0, z = 0] =
    Symbol.iterator in origin ? [...origin] : [origin.x ?? 0, origin.y ?? 0, origin.z ?? 0];
  return [x, y, z].every((each) => Number.isInteger(each) && each >= 0) ? { x, y, z } : null;
}

/** Whether a region lies within a texture's level (a 3D texture's depth halves with its level). */
function fitsLevel(
  texture: GPUTexture,
  mipLevel: number,
  origin: Required<GPUOrigin3DDict>,
  extent: Extent,
): boolean {
  if (!Number.isInteger(mipLevel) || mipLevel < 0 || mipLevel >= texture.mipLevelCount) {
    return false;
  }
  const width = Math.max(1, texture.width >> mipLevel);
  const height = Math.max(1, texture.height >> mipLevel);
  const layers =
    texture.dimension === "3d"
      ? Math.max(1, texture.depthOrArrayLayers >> mipLevel)
      : texture.depthOrArrayLayers;
  return (
    origin.x + extent.width <= width &&
    origin.y + extent.height <= height &&
    origin.z + extent.depthOrArrayLayers <= layers
  );
}

/** A view's bytes, as bytes. */
function bytesOf(data: ArrayBufferView): Uint8Array {
  return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
}

/** The writes of the engine's open scopes, submitted as the outermost ends or the engine submits. */
export class StagedWrites {
  readonly #device: GPUDevice;
  readonly #partBytes: number;
  readonly #chunkBytes: number;
  #depth = 0;
  /** The outermost open scope's label. */
  #scopeLabel = "";
  /** The staging's label: the scope's whose write was staged first since the last submission. */
  #label = "";
  /** The staging buffers of the copies staged so far, the last the one being filled. */
  #chunks: StagingChunk[] = [];
  /** The bytes staged so far, against the part's. */
  #used = 0;
  #copies: StagedCopy[] = [];

  /**
   * @param partBytes - The most bytes staged before they are submitted, {@link STAGING_PART_BYTES}
   *   but in a test.
   * @param chunkBytes - One staging buffer's bytes, {@link STAGING_CHUNK_BYTES} but in a test; at
   *   most the part's.
   */
  constructor(
    device: GPUDevice,
    partBytes: number = STAGING_PART_BYTES,
    chunkBytes: number = STAGING_CHUNK_BYTES,
  ) {
    this.#device = device;
    this.#partBytes = partBytes;
    this.#chunkBytes = Math.min(chunkBytes, partBytes);
  }

  /** Whether a scope is open, so that the registry's writes are staged. */
  get open(): boolean {
    return this.#depth > 0;
  }

  /**
   * Runs `write` in a scope, its writes staged for the next {@link StagedWrites.flush}, if `write`
   * throws as well: those made before take effect, as queue writes would have.
   *
   * @param label - The staging's label, where a write of this scope is the first staged since the
   *   last submission; an inner scope's joins the outer's.
   */
  run<T>(label: string, write: () => T): T {
    if (this.#depth === 0) {
      this.#scopeLabel = label;
    }
    this.#depth += 1;
    try {
      return write();
    } finally {
      this.#depth -= 1;
    }
  }

  /**
   * Stages a buffer write of `data` at `offsetBytes`, where it is one the copy can make.
   *
   * @returns `false`, staging nothing, for a write whose offset or size is not a multiple of 4,
   *   that passes the buffer's end, or to a buffer without `COPY_DST` or mapped: the caller hands
   *   it to the queue, whose write refuses it, as before, alone, rather than in a copy that would
   *   void the submission's every write.
   */
  buffer(buffer: GPUBuffer, offsetBytes: number, data: ArrayBufferView): boolean {
    const bytes = data.byteLength;
    if (
      bytes === 0 ||
      !Number.isInteger(offsetBytes) ||
      offsetBytes < 0 ||
      offsetBytes % COPY_ALIGNMENT !== 0 ||
      bytes % COPY_ALIGNMENT !== 0 ||
      offsetBytes + bytes > buffer.size ||
      (buffer.usage & BUFFER_USAGE.COPY_DST) === 0 ||
      buffer.mapState !== "unmapped"
    ) {
      return false;
    }
    const { chunk, fromBytes } = this.#reserve(bytes, COPY_ALIGNMENT);
    chunk.bytes.set(bytesOf(data), fromBytes);
    this.#stage({ kind: "buffer", from: chunk.buffer, fromBytes, buffer, offsetBytes, bytes });
    return true;
  }

  /**
   * Stages a texture write of `data`, its rows of `rowBytes` tightly packed, `extent.height` rows
   * an image and `extent.depthOrArrayLayers` images, where it is one the copy can make.
   *
   * @returns `false`, staging nothing, for a write of no texels, of a depth or stencil format, of
   *   too few bytes, outside the level or to a texture without `COPY_DST`, single-sampled, or with
   *   the level: the caller hands it to the queue, as {@link StagedWrites.buffer}'s.
   */
  texture(
    texture: GPUTexture,
    origin: GPUOrigin3D,
    mipLevel: number,
    extent: Extent,
    rowBytes: number,
    data: ArrayBufferView,
  ): boolean {
    const rows = extent.height * extent.depthOrArrayLayers;
    const at = originOf(origin);
    if (
      rows === 0 ||
      rowBytes === 0 ||
      extent.width === 0 ||
      data.byteLength < rows * rowBytes ||
      at === null ||
      !fitsLevel(texture, mipLevel, at, extent) ||
      texture.format.startsWith("depth") ||
      texture.format.startsWith("stencil") ||
      texture.sampleCount !== 1 ||
      (texture.usage & TEXTURE_USAGE.COPY_DST) === 0
    ) {
      return false;
    }
    const bytesPerRow = aligned(rowBytes, ROW_ALIGNMENT);
    const { chunk, fromBytes } = this.#reserve(bytesPerRow * rows, ROW_ALIGNMENT);
    const source = bytesOf(data);
    if (bytesPerRow === rowBytes) {
      chunk.bytes.set(source.subarray(0, rows * rowBytes), fromBytes);
    } else {
      for (let row = 0; row < rows; row += 1) {
        chunk.bytes.set(
          source.subarray(row * rowBytes, (row + 1) * rowBytes),
          fromBytes + row * bytesPerRow,
        );
      }
    }
    this.#stage({
      kind: "texture",
      from: chunk.buffer,
      fromBytes,
      texture,
      origin: at,
      mipLevel,
      extent,
      bytesPerRow,
    });
    return true;
  }

  /**
   * Submits the writes staged so far, in the order made, in one command buffer: before anything
   * else the engine submits, destroys or writes through the queue. Nothing is submitted with none.
   */
  flush(): void {
    const copies = this.#copies;
    const chunks = this.#chunks;
    this.#copies = [];
    this.#chunks = [];
    this.#used = 0;
    try {
      for (const chunk of chunks) {
        chunk.buffer.unmap();
      }
      if (copies.length === 0) {
        return;
      }
      const encoder = this.#device.createCommandEncoder({ label: this.#label });
      for (const copy of copies) {
        switch (copy.kind) {
          case "buffer":
            encoder.copyBufferToBuffer(
              copy.from,
              copy.fromBytes,
              copy.buffer,
              copy.offsetBytes,
              copy.bytes,
            );
            break;
          case "texture":
            encoder.copyBufferToTexture(
              {
                buffer: copy.from,
                offset: copy.fromBytes,
                bytesPerRow: copy.bytesPerRow,
                rowsPerImage: copy.extent.height,
              },
              { texture: copy.texture, origin: copy.origin, mipLevel: copy.mipLevel },
              copy.extent,
            );
            break;
        }
      }
      this.#device.queue.submit([encoder.finish()]);
    } finally {
      // Freed once the copies that read them have run.
      for (const chunk of chunks) {
        chunk.buffer.destroy();
      }
    }
  }

  /** Stages one copy, the first since the last submission naming the staging. */
  #stage(copy: StagedCopy): void {
    if (this.#copies.length === 0) {
      this.#label = this.#scopeLabel;
    }
    this.#copies.push(copy);
  }

  /** Drops the writes staged so far unsubmitted: their buffers and textures are being destroyed. */
  discard(): void {
    for (const chunk of this.#chunks) {
      chunk.buffer.destroy();
    }
    this.#chunks = [];
    this.#copies = [];
    this.#used = 0;
  }

  /**
   * Reserves `bytes` of a staging buffer at an offset aligned to `alignment`: the last one's, where
   * they fit, else a new one's, made mapped, of a chunk's bytes or of `bytes` where they are more;
   * submitting what is staged first where they would pass the part's bytes.
   */
  #reserve(bytes: number, alignment: number): { chunk: StagingChunk; fromBytes: number } {
    if (this.#used > 0 && this.#used + bytes > this.#partBytes) {
      this.flush();
    }
    const last = this.#chunks.at(-1);
    if (last !== undefined) {
      const fromBytes = aligned(last.used, alignment);
      if (fromBytes + bytes <= last.bytes.byteLength) {
        last.used = fromBytes + bytes;
        this.#used += bytes;
        return { chunk: last, fromBytes };
      }
    }
    const size = Math.max(this.#chunkBytes, aligned(bytes, COPY_ALIGNMENT));
    const buffer = this.#device.createBuffer({
      label: `${this.#scopeLabel} staging`,
      size,
      usage: BUFFER_USAGE.MAP_WRITE | BUFFER_USAGE.COPY_SRC,
      mappedAtCreation: true,
    });
    const chunk: StagingChunk = {
      buffer,
      bytes: new Uint8Array(buffer.getMappedRange()),
      used: bytes,
    };
    this.#chunks.push(chunk);
    this.#used += bytes;
    return { chunk, fromBytes: 0 };
  }
}
