/**
 * The GPU resources of one dispatch, resolved from `ComputeBindings` by the names the kernel
 * declares.
 *
 * @remarks
 * Each uniform is written into a buffer of the kernel's own, made once per name through the one
 * creation path (Design note 18), so that its bytes and uploads are counted; a storage texture is
 * bound at the level `ComputeBindings` names, a cube's as a `2d-array` view of its six faces. A
 * texture is viewed at the dimension its declaration takes (R08.T0), under the rule a material's
 * binding follows, so that a one-layer 2D texture binds as a one-layer array where the kernel
 * declares one.
 */

import type { TextureSpec } from "../memory";
import type { ComputeBindings } from "../types";
import type { BufferHandle } from "../types";
import type { BoundResource, KernelRecord } from "./compute";
import {
  type ResourceRegistry,
  UNIFORM_BUFFER_USAGE,
  viewDimensionBinds,
  viewDimensionOf,
} from "./resources";

/** The bytes a uniform buffer is made with: the value's, rounded up to 16, at least 16. */
export function uniformBufferBytes(valueBytes: number): number {
  return Math.max(16, Math.ceil(valueBytes / 16) * 16);
}

/**
 * The dimension a texture bound to `kernel`'s binding `name` is viewed at: the one the binding
 * declares, where the texture binds there.
 *
 * @remarks
 * A storage view is never a cube (W3C WebGPU, §8.1.1 "Bind Group Layout Creation"), so a cube
 * bound as storage is the six-layer `2d-array` of its faces. A name with no declared dimension (one
 * the kernel does not declare, or declares as a buffer, a sampler or a texture type the table does
 * not read) is viewed at the texture's own dimension, unchecked, as before R08.T0:
 * `kernelBindGroupEntries` refuses an undeclared name, and WebGPU a texture given for a buffer or
 * a sampler.
 * @throws Error naming the kernel, the binding and both dimensions where the texture does not bind
 * at the declared one: WebGPU would refuse the bind group (§8.2.1 "Bind Group Creation"), and with
 * it the whole submission.
 */
function kernelViewDimension(
  kernel: KernelRecord,
  name: string,
  spec: TextureSpec,
  access: "sampled" | "storage",
): GPUTextureViewDimension {
  const own = viewDimensionOf(spec);
  const viewable = access === "storage" && own === "cube" ? "2d-array" : own;
  const declared = kernel.bindings.get(name)?.viewDimension;
  if (declared === undefined) {
    return viewable;
  }
  if (!viewDimensionBinds(declared, viewable)) {
    throw new Error(
      `kernel ${kernel.pair.name} declares ${name} as ${declared}, but ${spec.name} is ${own}`,
    );
  }
  return declared;
}

/**
 * Resolves `bindings` for `kernel`.
 *
 * @param uniformBuffers - The kernel's uniform buffers by name, made here when first needed and
 * kept by the caller.
 * @throws Error naming the kernel and the binding when a texture does not bind at the view
 * dimension the kernel declares for it, before any uniform is written or any view made; Error
 * naming the uniform when a value is larger than the buffer its first value made.
 */
export function resolveKernelResources(
  resources: ResourceRegistry,
  kernel: KernelRecord,
  bindings: ComputeBindings,
  uniformBuffers: Map<string, BufferHandle>,
): ReadonlyMap<string, BoundResource> {
  const sampled = Object.entries(bindings.sampled).map(([name, handle]) => {
    const { texture, spec } = resources.textureOf(handle);
    return { name, texture, spec, dimension: kernelViewDimension(kernel, name, spec, "sampled") };
  });
  const storage = Object.entries(bindings.storage).map(([name, { texture: handle, level }]) => {
    const { texture, spec } = resources.textureOf(handle);
    return {
      name,
      texture,
      spec,
      level,
      dimension: kernelViewDimension(kernel, name, spec, "storage"),
    };
  });
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
  for (const { name, texture, spec, dimension } of sampled) {
    resolved.set(name, {
      kind: "texture-view",
      view: texture.createView({ label: spec.name, dimension }),
    });
  }
  for (const { name, texture, spec, level, dimension } of storage) {
    resolved.set(name, {
      kind: "texture-view",
      view: texture.createView({
        label: `${spec.name} level ${level}`,
        dimension,
        baseMipLevel: level,
        mipLevelCount: 1,
      }),
    });
  }
  return resolved;
}
