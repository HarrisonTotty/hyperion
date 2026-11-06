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
 * The bytes are copied as each write is made, so that a caller may reuse its array at once, as it
 * may after a queue write. The copies are encoded in the order the writes were made and submitted
 * in one command buffer when the outermost scope ends, or earlier when the engine submits anything
 * else ({@link StagedWrites.flush}): every staged write takes effect before anything submitted
 * after it, as a queue write does, and the buffers and textures hold the same bytes for every
 * command that reads them. A texture's rows are laid out 256-byte aligned for
 * `copyBufferToTexture`; the bytes that reach the texture are the rows given.
 *
 * The staging buffer is transient, made and destroyed by each submission, and is not GPU memory a
 * view keeps, so it raises no allocation event, as a readback's staging buffer raises none.
 */

import { BUFFER_USAGE } from "../gpuFlags";
import type { Extent } from "../memory";

/** The alignment `copyBufferToTexture` asks of a staged texture's rows, bytes. */
const ROW_ALIGNMENT = 256;

/** The alignment `copyBufferToBuffer` asks of offsets and sizes, bytes. */
const COPY_ALIGNMENT = 4;

/**
 * The most bytes one staging buffer holds before the writes after it go into the next, 64 MiB: a
 * scope of larger uploads is submitted in parts, in order, each within the device's least
 * `maxBufferSize` (256 MiB).
 */
export const STAGING_PART_BYTES = 64 * 1024 * 1024;

/** The scratch the bytes are gathered in, kept between submissions up to this size, 4 MiB. */
const KEPT_SCRATCH_BYTES = 4 * 1024 * 1024;

/** The scratch's least size, 64 KiB. */
const LEAST_SCRATCH_BYTES = 64 * 1024;

/** One staged write: where its bytes lie in the staging buffer, and where they go. */
type StagedCopy =
  | {
      readonly kind: "buffer";
      readonly fromBytes: number;
      readonly buffer: GPUBuffer;
      readonly offsetBytes: number;
      readonly bytes: number;
    }
  | {
      readonly kind: "texture";
      readonly fromBytes: number;
      readonly texture: GPUTexture;
      readonly origin: GPUOrigin3D;
      readonly mipLevel: number;
      readonly extent: Extent;
      readonly bytesPerRow: number;
    };

/** `bytes` rounded up to a multiple of `alignment`. */
function aligned(bytes: number, alignment: number): number {
  return Math.ceil(bytes / alignment) * alignment;
}

/** A view's bytes, as bytes. */
function bytesOf(data: ArrayBufferView): Uint8Array {
  return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
}

/** The writes of the engine's open scopes, submitted as the outermost ends or the engine submits. */
export class StagedWrites {
  readonly #device: GPUDevice;
  readonly #partBytes: number;
  #depth = 0;
  #label = "";
  #scratch = new Uint8Array(0);
  #used = 0;
  #copies: StagedCopy[] = [];

  /**
   * @param partBytes - The most bytes one staging buffer holds, {@link STAGING_PART_BYTES} but in
   *   a test.
   */
  constructor(device: GPUDevice, partBytes: number = STAGING_PART_BYTES) {
    this.#device = device;
    this.#partBytes = partBytes;
  }

  /** Whether a scope is open, so that the registry's writes are staged. */
  get open(): boolean {
    return this.#depth > 0;
  }

  /**
   * Runs `write` in a scope, and submits the writes staged in it as the outermost scope ends, if
   * `write` throws as well: those made before take effect, as queue writes would have.
   *
   * @param label - The submission's label; an inner scope's joins the outer's.
   */
  run<T>(label: string, write: () => T): T {
    if (this.#depth === 0) {
      this.#label = label;
    }
    this.#depth += 1;
    try {
      return write();
    } finally {
      this.#depth -= 1;
      if (this.#depth === 0) {
        this.flush();
      }
    }
  }

  /** Stages a buffer write of `data` at `offsetBytes`. */
  buffer(buffer: GPUBuffer, offsetBytes: number, data: ArrayBufferView): void {
    const bytes = data.byteLength;
    if (bytes === 0) {
      return;
    }
    const fromBytes = this.#reserve(aligned(bytes, COPY_ALIGNMENT), COPY_ALIGNMENT);
    this.#scratch.set(bytesOf(data), fromBytes);
    this.#copies.push({ kind: "buffer", fromBytes, buffer, offsetBytes, bytes });
  }

  /**
   * Stages a texture write of `data`, its rows of `rowBytes` tightly packed, `extent.height` rows
   * an image and `extent.depthOrArrayLayers` images.
   *
   * @throws RangeError for data shorter than its rows.
   */
  texture(
    texture: GPUTexture,
    origin: GPUOrigin3D,
    mipLevel: number,
    extent: Extent,
    rowBytes: number,
    data: ArrayBufferView,
  ): void {
    const rows = extent.height * extent.depthOrArrayLayers;
    if (rows === 0 || rowBytes === 0) {
      return;
    }
    if (data.byteLength < rows * rowBytes) {
      throw new RangeError(
        `${texture.label}: ${String(data.byteLength)} B written, where its ${String(rows)} rows hold ${String(rows * rowBytes)} B`,
      );
    }
    const bytesPerRow = aligned(rowBytes, ROW_ALIGNMENT);
    const fromBytes = this.#reserve(bytesPerRow * rows, ROW_ALIGNMENT);
    const source = bytesOf(data);
    if (bytesPerRow === rowBytes) {
      this.#scratch.set(source.subarray(0, rows * rowBytes), fromBytes);
    } else {
      for (let row = 0; row < rows; row += 1) {
        this.#scratch.set(
          source.subarray(row * rowBytes, (row + 1) * rowBytes),
          fromBytes + row * bytesPerRow,
        );
      }
    }
    this.#copies.push({
      kind: "texture",
      fromBytes,
      texture,
      origin,
      mipLevel,
      extent,
      bytesPerRow,
    });
  }

  /**
   * Submits the writes staged so far, in the order made, in one command buffer: before anything
   * else the engine submits, and as the outermost scope ends. Nothing is submitted with none.
   */
  flush(): void {
    const copies = this.#copies;
    if (copies.length === 0) {
      return;
    }
    const bytes = aligned(this.#used, COPY_ALIGNMENT);
    this.#copies = [];
    this.#used = 0;
    try {
      const staging = this.#device.createBuffer({
        label: `${this.#label} staging`,
        size: bytes,
        usage: BUFFER_USAGE.MAP_WRITE | BUFFER_USAGE.COPY_SRC,
        mappedAtCreation: true,
      });
      new Uint8Array(staging.getMappedRange()).set(this.#scratch.subarray(0, bytes));
      staging.unmap();
      const encoder = this.#device.createCommandEncoder({ label: this.#label });
      for (const copy of copies) {
        switch (copy.kind) {
          case "buffer":
            encoder.copyBufferToBuffer(
              staging,
              copy.fromBytes,
              copy.buffer,
              copy.offsetBytes,
              copy.bytes,
            );
            break;
          case "texture":
            encoder.copyBufferToTexture(
              {
                buffer: staging,
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
      // Freed once the copies that read it have run.
      staging.destroy();
    } finally {
      if (this.#scratch.byteLength > KEPT_SCRATCH_BYTES) {
        this.#scratch = new Uint8Array(0);
      }
    }
  }

  /** Drops the writes staged so far unsubmitted: their buffers and textures are being destroyed. */
  discard(): void {
    this.#copies = [];
    this.#used = 0;
  }

  /**
   * Reserves `bytes` of the scratch at an offset aligned to `alignment`, submitting what is staged
   * first where it would pass the part's bytes.
   */
  #reserve(bytes: number, alignment: number): number {
    if (this.#used > 0 && aligned(this.#used, alignment) + bytes > this.#partBytes) {
      this.flush();
    }
    const fromBytes = aligned(this.#used, alignment);
    const needed = fromBytes + bytes;
    if (needed > this.#scratch.byteLength) {
      const grown = new Uint8Array(
        Math.max(needed, 2 * this.#scratch.byteLength, LEAST_SCRATCH_BYTES),
      );
      grown.set(this.#scratch.subarray(0, this.#used));
      this.#scratch = grown;
    }
    this.#used = needed;
    return fromBytes;
  }
}
