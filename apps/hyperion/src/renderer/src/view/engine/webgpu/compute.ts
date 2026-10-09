/**
 * Compute kernels, compiled and dispatched on the engine's device.
 *
 * @remarks
 * A kernel is plain WGSL with its own `@group` and `@binding` attributes and entry point `main`:
 * the adapter encodes its dispatches itself, so that each carries its pass's timestamps (R01 Design
 * note 19), binds a storage texture at the mip level `ComputeBindings` names, and takes GPU-written
 * workgroup counts through `dispatchWorkgroupsIndirect`. Bindings are found by the names the source
 * declares.
 */

import { assertNoF16Subgroups, type KernelPair, selectKernel } from "../kernels";
import type { GpuCapabilities } from "../platform";

/** Where a resource a kernel declares is bound, and how. */
export interface KernelBinding {
  readonly group: number;
  readonly binding: number;
  /** The declaration's address space or type: `uniform`, `storage`, a texture or a sampler. */
  readonly kind: "uniform" | "storage" | "texture" | "storage-texture" | "sampler";
  /** Whether the kernel may write it: a `read_write` storage buffer, or a storage texture. */
  readonly writable: boolean;
  /**
   * The view dimension a texture's declaration takes under the pipeline's `auto` layout, which a
   * dispatch views the texture at (R08.T0).
   *
   * @remarks
   * `undefined` for a buffer, a sampler, a `texture_external` and a type the table of WebGPU's
   * types does not list (an `alias`, a `binding_array`): a texture given there is viewed at its own
   * dimension, unchecked, as before R08.T0.
   */
  readonly viewDimension: GPUTextureViewDimension | undefined;
}

/** WGSL without its comments, so that a commented-out declaration is not read. */
function withoutComments(wgsl: string): string {
  return wgsl.replaceAll(/\/\*[\s\S]*?\*\//gu, " ").replaceAll(/\/\/[^\n]*/gu, " ");
}

const DECLARATION =
  /@group\(\s*(\d+)\s*\)\s*@binding\(\s*(\d+)\s*\)\s*var\s*(<[^>]*>)?\s*(\w+)\s*:\s*(\w+)(<[^;]*>)?/gu;

/**
 * The view dimension of each WGSL texture type: the "Corresponding WGSL types" of
 * `GPUTextureViewDimension` (W3C WebGPU, Candidate Recommendation, §6.2.1 "Texture View
 * Creation", read 2026-10-09). `texture_external` binds an external texture, not a view, and has
 * none.
 */
const TYPE_VIEW_DIMENSIONS: ReadonlyMap<string, GPUTextureViewDimension> = new Map([
  ["texture_1d", "1d"],
  ["texture_storage_1d", "1d"],
  ["texture_2d", "2d"],
  ["texture_storage_2d", "2d"],
  ["texture_multisampled_2d", "2d"],
  ["texture_depth_2d", "2d"],
  ["texture_depth_multisampled_2d", "2d"],
  ["texture_2d_array", "2d-array"],
  ["texture_storage_2d_array", "2d-array"],
  ["texture_depth_2d_array", "2d-array"],
  ["texture_cube", "cube"],
  ["texture_depth_cube", "cube"],
  ["texture_cube_array", "cube-array"],
  ["texture_depth_cube_array", "cube-array"],
  ["texture_3d", "3d"],
  ["texture_storage_3d", "3d"],
]);

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
 * A kernel uses every binding it declares: its pipeline's layout is `auto`, which holds only the
 * bindings the entry point uses, and every declared binding goes into the bind group.
 * Reads `@group(g) @binding(b) var<space> name : type` declarations, the attributes in that order,
 * which is how every kernel in the catalogue writes them. A texture's view dimension is its type's,
 * which is the one its `auto` layout entry takes.
 */
export function kernelBindings(wgsl: string): ReadonlyMap<string, KernelBinding> {
  const bindings = new Map<string, KernelBinding>();
  for (const match of withoutComments(wgsl).matchAll(DECLARATION)) {
    const [, group, binding, space, name, type, parameters] = match;
    if (group === undefined || binding === undefined || name === undefined || type === undefined) {
      continue;
    }
    const kind = kindOf(space, type);
    bindings.set(name, {
      group: Number(group),
      binding: Number(binding),
      kind,
      writable:
        (kind === "storage-texture" && !/,\s*read\s*>/u.test(parameters ?? "")) ||
        (kind === "storage" && (space ?? "").includes("read_write")),
      viewDimension: space === undefined ? TYPE_VIEW_DIMENSIONS.get(type) : undefined,
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

/** Compiles `pair`'s variant on `device` off the queue's timeline. */
export async function createKernelAsync(
  device: GPUDevice,
  pair: KernelPair,
  capabilities: GpuCapabilities,
): Promise<KernelRecord> {
  const { path, module } = prepareKernel(pair, capabilities);
  const pipeline = await device.createComputePipelineAsync({
    label: module.label ?? pair.name,
    layout: "auto",
    compute: { module: device.createShaderModule(module), entryPoint: "main" },
  });
  return { pair, path, pipeline, bindings: kernelBindings(module.code) };
}

/** A resource bound to one of a kernel's declarations, resolved to the GPU object it is. */
export type BoundResource =
  | { readonly kind: "buffer"; readonly buffer: GPUBuffer }
  | { readonly kind: "texture-view"; readonly view: GPUTextureView }
  | { readonly kind: "sampler"; readonly sampler: GPUSampler };

/**
 * The bind-group entries of one dispatch, by group, from its resources by name.
 *
 * @throws Error naming the kernel and the binding when a declared binding is given nothing, or a
 * resource is given for a name the kernel does not declare.
 */
export function kernelBindGroupEntries(
  kernel: { readonly pair: { readonly name: string }; readonly bindings: KernelRecord["bindings"] },
  resources: ReadonlyMap<string, BoundResource>,
): ReadonlyMap<number, ReadonlyArray<GPUBindGroupEntry>> {
  for (const name of resources.keys()) {
    if (!kernel.bindings.has(name)) {
      throw new Error(`kernel ${kernel.pair.name} declares no binding ${name}`);
    }
  }
  const groups = new Map<number, GPUBindGroupEntry[]>();
  for (const [name, { group, binding }] of kernel.bindings) {
    const resource = resources.get(name);
    if (resource === undefined) {
      throw new Error(`kernel ${kernel.pair.name}'s binding ${name} was given nothing`);
    }
    const entries = groups.get(group) ?? [];
    entries.push({
      binding,
      resource: bindingResource(resource),
    });
    groups.set(group, entries);
  }
  return new Map([...groups.entries()].toSorted(([a], [b]) => a - b));
}

/** The bind-group resource of a bound resource. */
function bindingResource(resource: BoundResource): GPUBindingResource {
  let bound: GPUBindingResource;
  switch (resource.kind) {
    case "buffer":
      bound = { buffer: resource.buffer };
      break;
    case "texture-view":
      bound = resource.view;
      break;
    case "sampler":
      bound = resource.sampler;
      break;
  }
  return bound;
}

/** Whether `value` is a `GPUBuffer`, by the members a buffer has and a texture lacks. */
export function isGpuBuffer(value: unknown): value is GPUBuffer {
  return typeof value === "object" && value !== null && "mapAsync" in value && "size" in value;
}

/** Workgroup counts, given on the CPU or written by the GPU. */
export type Workgroups =
  | { readonly kind: "direct"; readonly counts: readonly [number, number, number] }
  | { readonly kind: "indirect"; readonly buffer: GPUBuffer; readonly offsetBytes: number };

/** Encodes one dispatch in its own compute pass. */
export function encodeDispatch(
  device: GPUDevice,
  encoder: GPUCommandEncoder,
  kernel: KernelRecord,
  resources: ReadonlyMap<string, BoundResource>,
  workgroups: Workgroups,
  descriptor: GPUComputePassDescriptor,
): void {
  const groups = kernelBindGroupEntries(kernel, resources);
  const pass = encoder.beginComputePass(descriptor);
  pass.setPipeline(kernel.pipeline);
  for (const [group, entries] of groups) {
    pass.setBindGroup(
      group,
      device.createBindGroup({
        label: `${kernel.pair.name}:${group}`,
        layout: kernel.pipeline.getBindGroupLayout(group),
        entries: [...entries],
      }),
    );
  }
  if (workgroups.kind === "direct") {
    const [x, y, z] = workgroups.counts;
    pass.dispatchWorkgroups(x, y, z);
  } else {
    pass.dispatchWorkgroupsIndirect(workgroups.buffer, workgroups.offsetBytes);
  }
  pass.end();
}
