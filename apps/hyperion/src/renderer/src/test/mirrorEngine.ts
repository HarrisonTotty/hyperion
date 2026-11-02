/**
 * An engine of buffers alone that keeps their bytes as the GPU would (plan R14, R14.T5.b), so that
 * the cull's twin reads a page pool's records as the pool rewrote them, and tallies each memory
 * category's live bytes.
 */

import type { AllocationEvent, BufferSpec, MemoryCategory } from "../view/engine/memory";
import type { BufferHandle } from "../view/engine/types";
import type { GeometryPagePoolEngine } from "../view/clusters/pages";

/** The engine (see the module's remarks). */
export class MirrorEngine implements GeometryPagePoolEngine {
  readonly #bytes = new Map<BufferHandle, Uint8Array>();
  readonly #categories = new Map<BufferHandle, MemoryCategory>();
  readonly events: AllocationEvent[] = [];

  createBuffer(spec: BufferSpec): BufferHandle {
    const handle: BufferHandle = { kind: "buffer", name: spec.name, bytes: spec.bytes };
    this.#bytes.set(handle, new Uint8Array(spec.bytes));
    this.#categories.set(handle, spec.category);
    this.events.push({
      kind: "created",
      name: spec.name,
      bytes: spec.bytes,
      category: spec.category,
    });
    return handle;
  }

  writeBuffer(buffer: BufferHandle, offsetBytes: number, data: ArrayBufferView): void {
    const target = this.#bytes.get(buffer);
    if (target === undefined) {
      throw new Error(`${buffer.name} is not a live buffer`);
    }
    if (offsetBytes < 0 || offsetBytes + data.byteLength > target.byteLength) {
      throw new RangeError(
        `a write of ${String(data.byteLength)} B at ${String(offsetBytes)} B passes ${buffer.name}'s ${String(target.byteLength)} B`,
      );
    }
    target.set(new Uint8Array(data.buffer, data.byteOffset, data.byteLength), offsetBytes);
    this.events.push({ kind: "uploaded", name: buffer.name, bytes: data.byteLength });
  }

  releaseBuffer(buffer: BufferHandle): void {
    const category = this.#categories.get(buffer);
    if (this.#bytes.delete(buffer) && category !== undefined) {
      this.events.push({ kind: "destroyed", name: buffer.name, bytes: buffer.bytes, category });
    }
    this.#categories.delete(buffer);
  }

  /** The live bytes of `buffer`. */
  bytes(buffer: BufferHandle): Uint8Array {
    const held = this.#bytes.get(buffer);
    if (held === undefined) {
      throw new Error(`${buffer.name} is not a live buffer`);
    }
    return held;
  }

  /** The live buffers' bytes in `category`. */
  heldBytes(category: MemoryCategory): number {
    let sum = 0;
    for (const [handle, held] of this.#bytes) {
      sum += this.#categories.get(handle) === category ? held.byteLength : 0;
    }
    return sum;
  }
}
