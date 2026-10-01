/**
 * Materials and post-processes as the adapter's own render pipelines (R01 Design note 23): their
 * fixed-function state, their `@group(2)` layouts and their pipeline descriptors.
 *
 * @remarks
 * Every pipeline has an explicit layout of three groups ({@link BIND_GROUPS}): the pass's `Frame`,
 * the draw's `Draw` at a dynamic offset in the frame's ring, and the specification's own textures,
 * samplers and storage buffers. Depth is reversed: cleared to 0, compared greater-or-equal, so a
 * bias away from the camera is negative.
 */

import { COLOUR_WRITE_ALL } from "../gpuFlags";
import {
  POST_PROCESS_BINDINGS,
  type SamplerSpec,
  type StorageBufferSpec,
  type TextureBindingSpec,
  type WgslMaterialSpec,
  type WgslPostProcessSpec,
} from "../types";

/** `GPUShaderStage`'s flags, which TypeScript 7's `lib.dom` declares no namespace for. */
export const SHADER_STAGE = { VERTEX: 0x1, FRAGMENT: 0x2, COMPUTE: 0x4 } as const;

/** The entry points of the convention. */
export const ENTRY_POINTS = { vertex: "vertexMain", fragment: "fragmentMain" } as const;

/**
 * The blend state of each mode, colour and alpha factors: every mode keeps the destination's
 * alpha, R07's meter class (Design note 21).
 */
export const BLEND_STATES: Readonly<Record<WgslMaterialSpec["blend"], GPUBlendState | undefined>> =
  {
    none: undefined,
    additive: {
      color: { srcFactor: "src-alpha", dstFactor: "one", operation: "add" },
      alpha: { srcFactor: "zero", dstFactor: "one", operation: "add" },
    },
    premultiplied: {
      color: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
      alpha: { srcFactor: "zero", dstFactor: "one", operation: "add" },
    },
  };

/** The sampler a post-process's input colour is read through: linear, clamped. */
export const INPUT_SAMPLER: SamplerSpec = {
  name: "hdr-colour-sampler",
  filter: "linear",
  address: "clamp-to-edge",
  binding: POST_PROCESS_BINDINGS["hdr-colour-sampler"],
};

/** The descriptor of a sampler of `spec`: its filter between mips too. */
export function samplerDescriptor(spec: SamplerSpec): GPUSamplerDescriptor {
  return {
    label: spec.name,
    magFilter: spec.filter,
    minFilter: spec.filter,
    mipmapFilter: spec.filter,
    addressModeU: spec.address,
    addressModeV: spec.address,
    addressModeW: spec.address,
  };
}

/** What a specification binds in `@group(2)`. */
export interface ResourceBindings {
  readonly owner: string;
  readonly textures: ReadonlyArray<TextureBindingSpec>;
  readonly samplers: ReadonlyArray<SamplerSpec>;
  readonly storageBuffers: ReadonlyArray<StorageBufferSpec>;
}

/** A material's `@group(2)` bindings. */
export function materialBindings(spec: WgslMaterialSpec): ResourceBindings {
  return {
    owner: `material ${spec.name}`,
    textures: spec.textures ?? [],
    samplers: spec.samplers,
    storageBuffers: spec.storageBuffers ?? [],
  };
}

/** A post-process's `@group(2)` bindings: its input colour and sampler, then its own. */
export function postProcessBindings(spec: WgslPostProcessSpec): ResourceBindings {
  return {
    owner: `post-process ${spec.name}`,
    textures: [
      { name: "hdr-colour", binding: POST_PROCESS_BINDINGS["hdr-colour"] },
      ...(spec.textures ?? []),
    ],
    samplers: [INPUT_SAMPLER, ...(spec.samplers ?? [])],
    storageBuffers: [],
  };
}

/**
 * The `@group(2)` layout entries of `bindings`, visible to both stages.
 *
 * @throws Error naming the owner when two resources share a binding.
 */
export function resourceLayoutEntries(
  bindings: ResourceBindings,
): ReadonlyArray<GPUBindGroupLayoutEntry> {
  const visibility = SHADER_STAGE.VERTEX | SHADER_STAGE.FRAGMENT;
  const entries: GPUBindGroupLayoutEntry[] = [
    ...bindings.textures.map((texture) => ({
      binding: texture.binding,
      visibility,
      texture: {
        sampleType: texture.sampleType ?? "float",
        viewDimension: texture.viewDimension ?? "2d",
      },
    })),
    ...bindings.samplers.map((sampler) => ({
      binding: sampler.binding,
      visibility,
      sampler: { type: sampler.filter === "linear" ? "filtering" : "non-filtering" } as const,
    })),
    ...bindings.storageBuffers.map((buffer) => ({
      binding: buffer.binding,
      visibility,
      buffer: { type: "read-only-storage" } as const,
    })),
  ];
  const seen = new Set<number>();
  for (const { binding } of entries) {
    if (seen.has(binding)) {
      throw new Error(`${bindings.owner} declares @group(2) @binding(${binding}) twice`);
    }
    seen.add(binding);
  }
  return entries.toSorted((a, b) => a.binding - b.binding);
}

/** The depth bias of a pipeline for a bias away from the camera, negated for reversed depth. */
export function depthBiasOf(
  bias: WgslMaterialSpec["depthBiasAway"],
  topology: GPUPrimitiveTopology,
): { readonly depthBias: number; readonly depthBiasSlopeScale: number } {
  // WebGPU takes a bias on triangles only.
  if (bias === undefined || topology !== "triangle-list") {
    return { depthBias: 0, depthBiasSlopeScale: 0 };
  }
  return { depthBias: -bias.constant, depthBiasSlopeScale: -bias.slopeScale };
}

/** The modules of a material, one when both stages are the same source. */
export interface MaterialModules {
  readonly vertex: GPUShaderModule;
  readonly fragment: GPUShaderModule;
}

/** The render pipeline of a material drawing a mesh of `buffers` into these attachments. */
export function materialPipelineDescriptor(
  spec: WgslMaterialSpec,
  modules: MaterialModules,
  layout: GPUPipelineLayout,
  buffers: ReadonlyArray<GPUVertexBufferLayout>,
  topology: GPUPrimitiveTopology,
  colourFormat: GPUTextureFormat,
  hasDepth: boolean,
): GPURenderPipelineDescriptor {
  const blend = BLEND_STATES[spec.blend];
  return {
    label: spec.name,
    layout,
    vertex: { module: modules.vertex, entryPoint: ENTRY_POINTS.vertex, buffers: [...buffers] },
    fragment: {
      module: modules.fragment,
      entryPoint: ENTRY_POINTS.fragment,
      targets: [
        {
          format: colourFormat,
          ...(blend === undefined ? {} : { blend }),
          writeMask: spec.colourWrites ? COLOUR_WRITE_ALL : 0,
        },
      ],
    },
    primitive: { topology, cullMode: spec.cullMode, frontFace: "ccw" },
    ...(hasDepth
      ? {
          depthStencil: {
            format: "depth32float",
            depthWriteEnabled: spec.depthWrite,
            depthCompare: "greater-equal",
            ...depthBiasOf(spec.depthBiasAway, topology),
          },
        }
      : {}),
  };
}

/**
 * The full-screen vertex stage of every post-process: one triangle covering the output, with
 * `uv` (0, 0) at the top left.
 */
export const FULL_SCREEN_WGSL = `
struct FullScreen {
  @builtin(position) position : vec4f,
  @location(0) uv : vec2f,
}

@vertex fn vertexMain(@builtin(vertex_index) index : u32) -> FullScreen {
  let corner = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
  var out : FullScreen;
  out.position = vec4f(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
  out.uv = corner;
  return out;
}
`;

/** The render pipeline of a post-process writing an output of `colourFormat`. */
export function postProcessPipelineDescriptor(
  spec: WgslPostProcessSpec,
  vertex: GPUShaderModule,
  fragment: GPUShaderModule,
  layout: GPUPipelineLayout,
  colourFormat: GPUTextureFormat,
): GPURenderPipelineDescriptor {
  return {
    label: spec.name,
    layout,
    vertex: { module: vertex, entryPoint: ENTRY_POINTS.vertex },
    fragment: {
      module: fragment,
      entryPoint: ENTRY_POINTS.fragment,
      targets: [{ format: colourFormat, writeMask: COLOUR_WRITE_ALL }],
    },
    primitive: { topology: "triangle-list", cullMode: "none" },
  };
}
