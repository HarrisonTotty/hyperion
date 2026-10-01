/**
 * The GPU resources of one dispatch, resolved from `ComputeBindings` by the names the kernel
 * declares.
 *
 * @remarks
 * Each uniform is written into a buffer of the kernel's own, made once per name through the one
 * creation path (Design note 18), so that its bytes and uploads are counted; a storage texture is
 * bound at the level `ComputeBindings` names, a cube's as a `2d-array` view of its six faces.
 */

import type { ComputeBindings } from "../types";
import type { BufferHandle } from "../types";
import type { BoundResource, KernelRecord } from "./compute";
import { type ResourceRegistry, UNIFORM_BUFFER_USAGE, viewDimensionOf } from "./resources";

/** The bytes a uniform buffer is made with: the value's, rounded up to 16, at least 16. */
export function uniformBufferBytes(valueBytes: number): number {
  return Math.max(16, Math.ceil(valueBytes / 16) * 16);
}

/**
 * Resolves `bindings` for `kernel`.
 *
 * @param uniformBuffers - The kernel's uniform buffers by name, made here when first needed and
 * kept by the caller.
 * @throws Error naming the uniform when a value is larger than the buffer its first value made.
 */
export function resolveKernelResources(
  resources: ResourceRegistry,
  kernel: KernelRecord,
  bindings: ComputeBindings,
  uniformBuffers: Map<string, BufferHandle>,
): ReadonlyMap<string, BoundResource> {
  const resolved = new Map<string, BoundResource>();
  for (const [name, value] of Object.entries(bindings.uniforms)) {
    let handle = uniformBuffers.get(name);
    if (handle === undefined) {
      handle = resources.createBuffer({
        name: `${kernel.pair.name}:${name}`,
        bytes: uniformBufferBytes(value.byteLength),
        usage: UNIFORM_BUFFER_USAGE,
        category: "other",
      });
      uniformBuffers.set(name, handle);
    }
    if (value.byteLength > handle.bytes) {
      throw new Error(
        `uniform ${name} of kernel ${kernel.pair.name} grew from ${handle.bytes} to ${value.byteLength} bytes`,
      );
    }
    resources.writeBuffer(handle, 0, value);
    resolved.set(name, { kind: "buffer", buffer: resources.bufferOf(handle).buffer });
  }
  for (const [name, handle] of Object.entries(bindings.buffers)) {
    resolved.set(name, { kind: "buffer", buffer: resources.bufferOf(handle).buffer });
  }
  for (const [name, handle] of Object.entries(bindings.sampled)) {
    const { texture, spec } = resources.textureOf(handle);
    resolved.set(name, {
      kind: "texture-view",
      view: texture.createView({ label: spec.name, dimension: viewDimensionOf(spec) }),
    });
  }
  for (const [name, { texture: handle, level }] of Object.entries(bindings.storage)) {
    const { texture, spec } = resources.textureOf(handle);
    resolved.set(name, {
      kind: "texture-view",
      view: texture.createView({
        label: `${spec.name} level ${level}`,
        dimension: spec.dimension === "cube" ? "2d-array" : viewDimensionOf(spec),
        baseMipLevel: level,
        mipLevelCount: 1,
      }),
    });
  }
  return resolved;
}
