/**
 * A buffer the engine made, handed to Babylon as a material's storage buffer.
 *
 * @remarks
 * `ShaderMaterial.setStorageBuffer` and the engine's `setStorageBuffer` take Babylon's own
 * `StorageBuffer`, which makes its buffer itself; the engine's buffers are made on the device with
 * a memory category (R01 Design note 18). So this subclass releases the small buffer its base made
 * and answers `getBuffer` with the engine's, wrapped as Babylon's `WebGPUDataBuffer`. Babylon binds
 * only what `getBuffer` returns (`webgpuEngine.pure.js:3314-3316` in 9.28.0).
 */

import { StorageBuffer } from "@babylonjs/core/Buffers/storageBuffer";
import type { DataBuffer } from "@babylonjs/core/Buffers/dataBuffer";
import type { WebGPUEngine } from "@babylonjs/core/Engines/webgpuEngine.pure";
import { WebGPUDataBuffer } from "@babylonjs/core/Meshes/WebGPU/webgpuDataBuffer";

/** The smallest buffer Babylon's base class may make, released at once. */
const PLACEHOLDER_BYTES = 4;

/** An engine buffer under Babylon's `StorageBuffer` type. */
export class ExternalStorageBuffer extends StorageBuffer {
  readonly #data: WebGPUDataBuffer;

  /** @param bytes - The buffer's size. */
  constructor(engine: WebGPUEngine, buffer: GPUBuffer, bytes: number) {
    super(engine, PLACEHOLDER_BYTES);
    super.dispose();
    this.#data = new WebGPUDataBuffer(buffer, bytes);
  }

  override getBuffer(): DataBuffer {
    return this.#data;
  }

  /** The engine owns the buffer, so Babylon's disposal leaves it alone. */
  override dispose(): void {
    // The registry destroys the buffer with the engine.
  }
}
