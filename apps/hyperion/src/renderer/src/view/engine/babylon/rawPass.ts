/**
 * Indirect draws, encoded by the adapter in a pass of its own after Babylon's pass for the same
 * attachments (R01 Design note 19).
 *
 * @remarks
 * Babylon's indirect draw path fills its indirect buffer from CPU counts, so a draw whose counts a
 * kernel wrote is drawn here instead: with `loadOp: "load"` on the view's or target's colour and
 * depth, through a pipeline built from the shaders Babylon compiled for the same material (the
 * effect's final `vertexSourceCode` and `fragmentSourceCode`), whose bindings this module reads
 * back from that code by name. Babylon's own uniforms there are its leftover block, `uniforms`,
 * laid out by WGSL's rules from the `LeftOver` struct the code declares, and its internals block,
 * `internals`, which this pass fills as an unflipped target does (`yFactor_` 1).
 */

import { COLOUR_WRITE_ALL } from "../gpuFlags";
import type { DrawItem, IndirectArgs } from "../types";
import { type BoundResource, kernelBindGroupEntries, kernelBindings } from "./compute";

/** One member of a WGSL struct, with its offset in bytes. */
export interface StructMember {
  readonly name: string;
  readonly type: string;
  readonly offsetBytes: number;
  readonly sizeBytes: number;
}

/** The size and alignment of each scalar, vector and matrix type a leftover block may hold. */
const LAYOUT: Readonly<Record<string, { readonly size: number; readonly align: number }>> = {
  f32: { size: 4, align: 4 },
  u32: { size: 4, align: 4 },
  vec2f: { size: 8, align: 8 },
  vec3f: { size: 12, align: 16 },
  vec4f: { size: 16, align: 16 },
  vec2u: { size: 8, align: 8 },
  vec3u: { size: 12, align: 16 },
  vec4u: { size: 16, align: 16 },
  mat3x3f: { size: 48, align: 16 },
  mat4x4f: { size: 64, align: 16 },
};

/** A WGSL type in its short form: `vec3<f32>` as `vec3f`. */
function shortType(type: string): string {
  return type
    .replaceAll(/\s+/gu, "")
    .replace(/^(vec[234]|mat[234]x[234])<f32>$/u, "$1f")
    .replace(/^(vec[234])<u32>$/u, "$1u");
}

/**
 * The members of the struct `name` declares in `wgsl`, laid out by WGSL's rules for a uniform
 * buffer, with the struct's total size.
 *
 * @throws Error naming the struct and member when a member's type has no layout here.
 */
export function structLayout(
  wgsl: string,
  name: string,
): { readonly members: ReadonlyArray<StructMember>; readonly sizeBytes: number } {
  const match = new RegExp(String.raw`struct\s+${name}\s*\{([^}]*)\}`, "u").exec(wgsl);
  if (match === null) {
    return { members: [], sizeBytes: 0 };
  }
  const members: StructMember[] = [];
  let offset = 0;
  let structAlign = 16;
  for (const declaration of (match[1] ?? "").split(",")) {
    const parts = /^\s*(\w+)\s*:\s*(.+?)\s*$/u.exec(declaration);
    if (parts === null) {
      continue;
    }
    const [, member = "", rawType = ""] = parts;
    const type = shortType(rawType);
    const layout = LAYOUT[type];
    if (layout === undefined) {
      throw new Error(`struct ${name}'s member ${member} has a type, ${rawType}, with no layout`);
    }
    offset = Math.ceil(offset / layout.align) * layout.align;
    members.push({ name: member, type, offsetBytes: offset, sizeBytes: layout.size });
    offset += layout.size;
    structAlign = Math.max(structAlign, layout.align);
  }
  return { members, sizeBytes: Math.max(16, Math.ceil(offset / structAlign) * structAlign) };
}

/**
 * The bytes of a uniform block of `layout`, from values by member name.
 *
 * @remarks
 * A `u32` or `vec*u` member is written from its value as unsigned integers, every other member as
 * `f32`s; a member with no value stays zero.
 */
export function packStruct(
  layout: { readonly members: ReadonlyArray<StructMember>; readonly sizeBytes: number },
  values: ReadonlyMap<string, Float32Array>,
): ArrayBuffer {
  const bytes = new ArrayBuffer(layout.sizeBytes);
  const floats = new Float32Array(bytes);
  const uints = new Uint32Array(bytes);
  for (const member of layout.members) {
    const value = values.get(member.name);
    if (value === undefined) {
      continue;
    }
    const count = Math.min(value.length, member.sizeBytes / 4);
    const start = member.offsetBytes / 4;
    const integral = member.type === "u32" || member.type.endsWith("u");
    for (let index = 0; index < count; index += 1) {
      const component = value[index] ?? 0;
      if (integral) {
        uints[start + index] = component;
      } else {
        floats[start + index] = component;
      }
    }
  }
  return bytes;
}

/** A vertex attribute's location, from the `VertexInputs` struct the compiled code declares. */
export function attributeLocations(vertexWgsl: string): ReadonlyMap<string, number> {
  const struct = /struct\s+VertexInputs\s*\{([^}]*)\}/u.exec(vertexWgsl)?.[1] ?? "";
  const locations = new Map<string, number>();
  for (const match of struct.matchAll(/@location\(\s*(\d+)\s*\)\s*(\w+)\s*:/gu)) {
    const [, location, name] = match;
    if (location !== undefined && name !== undefined) {
      locations.set(name, Number(location));
    }
  }
  return locations;
}

/**
 * The bindings the compiled code declares and uses, by name, across both stages: what an `auto`
 * layout holds ({@link usedBindings}).
 */
export function compiledBindings(
  vertexWgsl: string,
  fragmentWgsl: string,
): ReturnType<typeof kernelBindings> {
  const declared = new Map([...kernelBindings(vertexWgsl), ...kernelBindings(fragmentWgsl)]);
  const used = usedBindings(declared.keys(), vertexWgsl, fragmentWgsl);
  return new Map([...declared].filter(([name]) => used.has(name)));
}

/** The `GPUVertexFormat` of a float attribute of `components`. */
export function floatVertexFormat(components: number): GPUVertexFormat {
  switch (components) {
    case 1:
      return "float32";
    case 2:
      return "float32x2";
    case 3:
      return "float32x3";
    case 4:
      return "float32x4";
    default:
      throw new Error(`a float attribute has 1 to 4 components, not ${components}`);
  }
}

/**
 * The depth bias a raw pipeline takes for a bias away from the camera.
 *
 * @remarks
 * Depth is reversed, so away is smaller: the values are negated, as Babylon negates `zOffset` and
 * `zOffsetUnits` itself for its own pipelines. Only triangles take a bias.
 */
export function rawDepthBias(
  bias: { readonly constant: number; readonly slopeScale: number } | undefined,
  topology: GPUPrimitiveTopology,
): { readonly depthBias: number; readonly depthBiasSlopeScale: number } {
  if (bias === undefined || topology !== "triangle-list") {
    return { depthBias: 0, depthBiasSlopeScale: 0 };
  }
  return { depthBias: -bias.constant, depthBiasSlopeScale: -bias.slopeScale };
}

/**
 * A frame's draws split by who draws them: Babylon's pass draws those with CPU counts, the raw
 * pass those whose counts the GPU wrote (`indirect`).
 */
export function partitionDraws(draws: ReadonlyArray<DrawItem>): {
  readonly babylon: ReadonlyArray<DrawItem>;
  readonly raw: ReadonlyArray<DrawItem & { readonly indirect: IndirectArgs }>;
} {
  const babylon: DrawItem[] = [];
  const raw: Array<DrawItem & { readonly indirect: IndirectArgs }> = [];
  for (const draw of draws) {
    const { indirect } = draw;
    if (indirect === undefined) {
      babylon.push(draw);
    } else {
      raw.push({ ...draw, indirect });
    }
  }
  return { babylon, raw };
}

/** One vertex buffer a raw draw binds, at the location the compiled code gives its attribute. */
export interface RawVertexBuffer {
  readonly location: number;
  readonly buffer: GPUBuffer;
  readonly offsetBytes: number;
  readonly strideBytes: number;
  readonly format: GPUVertexFormat;
  readonly instanced: boolean;
}

/** The fixed-function state of a raw draw, from its material's specification. */
export interface RawState {
  readonly cullMode: GPUCullMode;
  readonly depthWrite: boolean;
  readonly colourWrites: boolean;
  readonly blend: GPUBlendState | undefined;
  readonly depthBias: { readonly constant: number; readonly slopeScale: number } | undefined;
}

/** Everything one indirect draw needs, resolved to GPU objects. */
export interface RawDraw {
  readonly name: string;
  readonly vertexWgsl: string;
  readonly fragmentWgsl: string;
  readonly topology: GPUPrimitiveTopology;
  readonly vertexBuffers: ReadonlyArray<RawVertexBuffer>;
  readonly index: { readonly buffer: GPUBuffer; readonly format: GPUIndexFormat } | null;
  readonly resources: ReadonlyMap<string, BoundResource>;
  readonly state: RawState;
  readonly indirect: { readonly buffer: GPUBuffer; readonly offsetBytes: number };
}

/** The attachments a raw pass draws into after Babylon's pass. */
export interface RawAttachments {
  readonly colour: GPUTextureView;
  readonly colourFormat: GPUTextureFormat;
  readonly depth: GPUTextureView | null;
}

/** The render pipeline of a raw draw into attachments of `colourFormat` and `depth32float`. */
export function rawPipelineDescriptor(
  draw: RawDraw,
  vertex: GPUShaderModule,
  fragment: GPUShaderModule,
  colourFormat: GPUTextureFormat,
  hasDepth: boolean,
): GPURenderPipelineDescriptor {
  return {
    label: `${draw.name} indirect`,
    layout: "auto",
    vertex: {
      module: vertex,
      entryPoint: "main",
      buffers: draw.vertexBuffers.map((vertexBuffer) => ({
        arrayStride: vertexBuffer.strideBytes,
        stepMode: vertexBuffer.instanced ? "instance" : "vertex",
        attributes: [
          { shaderLocation: vertexBuffer.location, offset: 0, format: vertexBuffer.format },
        ],
      })),
    },
    fragment: {
      module: fragment,
      entryPoint: "main",
      targets: [
        {
          format: colourFormat,
          ...(draw.state.blend === undefined ? {} : { blend: draw.state.blend }),
          writeMask: draw.state.colourWrites ? COLOUR_WRITE_ALL : 0,
        },
      ],
    },
    primitive: { topology: draw.topology, cullMode: draw.state.cullMode, frontFace: "ccw" },
    ...(hasDepth
      ? {
          depthStencil: {
            format: "depth32float",
            depthWriteEnabled: draw.state.depthWrite,
            depthCompare: "greater-equal",
            ...rawDepthBias(draw.state.depthBias, draw.topology),
          },
        }
      : {}),
  };
}

/**
 * Raw pipelines, made once per shader pair, layout and target, off the frame path.
 *
 * @remarks
 * A pipeline is made by `createRenderPipelineAsync` the first time a draw asks for it, and the
 * draw is left out until it resolves, as Babylon leaves out a draw whose shaders are compiling.
 */
export class RawPipelines {
  readonly #device: Pick<GPUDevice, "createShaderModule" | "createRenderPipelineAsync">;
  readonly #pipelines = new Map<string, GPURenderPipeline | "pending">();
  readonly #modules = new Map<string, GPUShaderModule>();

  constructor(device: Pick<GPUDevice, "createShaderModule" | "createRenderPipelineAsync">) {
    this.#device = device;
  }

  /** The pipeline that draws `draw` into these attachments, or `undefined` while it is made. */
  pipelineFor(
    draw: RawDraw,
    colourFormat: GPUTextureFormat,
    hasDepth: boolean,
  ): GPURenderPipeline | undefined {
    const vertex = this.#module(draw.vertexWgsl);
    const fragment = this.#module(draw.fragmentWgsl);
    const key = JSON.stringify([
      draw.vertexWgsl,
      draw.fragmentWgsl,
      draw.topology,
      draw.vertexBuffers.map(({ location, strideBytes, format, instanced }) => [
        location,
        strideBytes,
        format,
        instanced,
      ]),
      draw.state,
      colourFormat,
      hasDepth,
    ]);
    const existing = this.#pipelines.get(key);
    if (existing !== undefined) {
      return existing === "pending" ? undefined : existing;
    }
    this.#pipelines.set(key, "pending");
    void this.#device
      .createRenderPipelineAsync(
        rawPipelineDescriptor(draw, vertex, fragment, colourFormat, hasDepth),
      )
      .then((pipeline): void => {
        this.#pipelines.set(key, pipeline);
        return undefined;
      })
      .catch((error: unknown) => {
        console.error(`the indirect pipeline of ${draw.name} could not be made:`, error);
      });
    return undefined;
  }

  #module(code: string): GPUShaderModule {
    let module = this.#modules.get(code);
    if (module === undefined) {
      module = this.#device.createShaderModule({ code });
      this.#modules.set(code, module);
    }
    return module;
  }
}

/**
 * The names of the bindings a draw's compiled code uses, not merely declares.
 *
 * @remarks
 * A pipeline layout made `auto` holds only the bindings its entry points use, and Babylon declares
 * a `<name>Sampler` for every texture, used or not; a bind group may hold only what the layout
 * does. A name that occurs anywhere but in its declarations is taken as used.
 */
export function usedBindings(
  names: Iterable<string>,
  ...stages: ReadonlyArray<string>
): Set<string> {
  const used = new Set<string>();
  for (const name of names) {
    let uses = 0;
    for (const code of stages) {
      const occurrences = code.match(new RegExp(String.raw`\b${name}\b`, "gu"))?.length ?? 0;
      const declarations =
        code.match(new RegExp(String.raw`\bvar\s*(<[^>]*>)?\s*${name}\s*:`, "gu"))?.length ?? 0;
      uses += occurrences - declarations;
    }
    if (uses > 0) {
      used.add(name);
    }
  }
  return used;
}

/**
 * Encodes one raw pass of `draws` over `attachments`, loading what Babylon's pass stored.
 *
 * @param timestampWrites - The pass's timestamps, when the frame is timed.
 */
export function encodeRawPass(
  device: GPUDevice,
  encoder: GPUCommandEncoder,
  pipelines: RawPipelines,
  draws: ReadonlyArray<RawDraw>,
  attachments: RawAttachments,
  label: string,
  timestampWrites: GPURenderPassTimestampWrites | undefined,
): void {
  const pass = encoder.beginRenderPass({
    label,
    colorAttachments: [{ view: attachments.colour, loadOp: "load", storeOp: "store" }],
    ...(attachments.depth === null
      ? {}
      : {
          depthStencilAttachment: {
            view: attachments.depth,
            depthLoadOp: "load",
            depthStoreOp: "store",
          },
        }),
    ...(timestampWrites === undefined ? {} : { timestampWrites }),
  });
  for (const draw of draws) {
    const pipeline = pipelines.pipelineFor(
      draw,
      attachments.colourFormat,
      attachments.depth !== null,
    );
    if (pipeline === undefined) {
      continue;
    }
    pass.setPipeline(pipeline);
    for (const [group, entries] of kernelBindGroupEntries(
      { pair: { name: draw.name }, bindings: compiledBindings(draw.vertexWgsl, draw.fragmentWgsl) },
      draw.resources,
    )) {
      pass.setBindGroup(
        group,
        device.createBindGroup({
          label: `${draw.name} indirect ${group}`,
          layout: pipeline.getBindGroupLayout(group),
          entries: [...entries],
        }),
      );
    }
    draw.vertexBuffers.forEach((vertexBuffer, slot) => {
      pass.setVertexBuffer(slot, vertexBuffer.buffer, vertexBuffer.offsetBytes);
    });
    if (draw.index === null) {
      pass.drawIndirect(draw.indirect.buffer, draw.indirect.offsetBytes);
    } else {
      pass.setIndexBuffer(draw.index.buffer, draw.index.format);
      pass.drawIndexedIndirect(draw.indirect.buffer, draw.indirect.offsetBytes);
    }
  }
  pass.end();
}
