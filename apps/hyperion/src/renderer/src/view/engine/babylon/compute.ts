/**
 * Compute kernels, compiled and dispatched on the engine's device.
 *
 * @remarks
 * A kernel is plain WGSL with its own `@group` and `@binding` attributes, not Babylon's dialect:
 * the adapter encodes its dispatches itself, so that each carries its pass's timestamps (R01 Design
 * note 19), binds a storage texture at the mip level `ComputeBindings` names, and takes GPU-written
 * workgroup counts through `dispatchWorkgroupsIndirect`. Babylon's `ComputeShader` offers none of
 * the first two. Bindings are found by the names the source declares.
 */

import { assertNoF16Subgroups, type KernelPair, selectKernel } from "../kernels";
import type { GpuCapabilities } from "../platform";

/** Where a resource a kernel declares is bound, and how. */
export interface KernelBinding {
  readonly group: number;
  readonly binding: number;
  /** The declaration's address space or type: `uniform`, `storage`, a texture or a sampler. */
  readonly kind: "uniform" | "storage" | "texture" | "storage-texture" | "sampler";
}

/** WGSL without its comments, so that a commented-out declaration is not read. */
function withoutComments(wgsl: string): string {
  return wgsl.replaceAll(/\/\*[\s\S]*?\*\//gu, " ").replaceAll(/\/\/[^\n]*/gu, " ");
}

const DECLARATION =
  /@group\(\s*(\d+)\s*\)\s*@binding\(\s*(\d+)\s*\)\s*var\s*(<[^>]*>)?\s*(\w+)\s*:\s*([\w]+)/gu;

function kindOf(addressSpace: string | undefined, type: string): KernelBinding["kind"] {
  if (addressSpace !== undefined) {
    return addressSpace.includes("uniform") ? "uniform" : "storage";
  }
  if (type.startsWith("texture_storage")) {
    return "storage-texture";
  }
  return type.startsWith("sampler") ? "sampler" : "texture";
}

/**
 * The bindings a kernel's source declares, by name.
 *
 * @remarks
 * Reads `@group(g) @binding(b) var<space> name : type` declarations, the attributes in that order,
 * which is how every kernel in the catalogue writes them.
 */
export function kernelBindings(wgsl: string): ReadonlyMap<string, KernelBinding> {
  const bindings = new Map<string, KernelBinding>();
  for (const match of withoutComments(wgsl).matchAll(DECLARATION)) {
    const [, group, binding, space, name, type] = match;
    if (group === undefined || binding === undefined || name === undefined || type === undefined) {
      continue;
    }
    bindings.set(name, {
      group: Number(group),
      binding: Number(binding),
      kind: kindOf(space, type),
    });
  }
  return bindings;
}

/** A kernel's chosen variant, compiled. */
export interface KernelRecord {
  readonly pair: KernelPair;
  readonly path: "reference" | "subgroup";
  readonly pipeline: GPUComputePipeline;
  readonly bindings: ReadonlyMap<string, KernelBinding>;
}

/**
 * The variant the device runs, checked and with its bindings read.
 *
 * @throws Error naming the kernel when its chosen source enables both `f16` and `subgroups`.
 */
export function prepareKernel(
  pair: KernelPair,
  capabilities: GpuCapabilities,
): { readonly path: "reference" | "subgroup"; readonly module: GPUShaderModuleDescriptor } {
  const { path, wgsl } = selectKernel(pair, capabilities);
  assertNoF16Subgroups(pair.name, wgsl);
  return { path, module: { label: `${pair.name}:${path}`, code: wgsl } };
}

/** Compiles `pair`'s variant on `device`, blocking the queue's timeline on the compile. */
export function createKernel(
  device: GPUDevice,
  pair: KernelPair,
  capabilities: GpuCapabilities,
): KernelRecord {
  const { path, module } = prepareKernel(pair, capabilities);
  const pipeline = device.createComputePipeline({
    label: module.label ?? pair.name,
    layout: "auto",
    compute: { module: device.createShaderModule(module), entryPoint: "main" },
  });
  return { pair, path, pipeline, bindings: kernelBindings(module.code) };
}
