/**
 * The adapter's one draw path (R01 Design note 23): a frame's draws, direct and indirect, in
 * submission order in one render pass over a view's or target's colour and depth, then its
 * post-processes as full-screen passes.
 *
 * @remarks
 * Every pipeline is the adapter's own, made from standard WGSL under explicit layouts of three
 * groups: the pass's `Frame`, written once a pass; the draw's `Draw`, a slot of the frame's
 * {@link UniformRing} at a dynamic offset; and the material's resources. With post-processes, the
 * draws go into an `rgba16float` intermediate of the output's size, and the passes ping-pong
 * between two such intermediates, the last writing the output.
 */

import { BUFFER_USAGE } from "../gpuFlags";
import type { TextureSpec } from "../memory";
import {
  type BufferHandle,
  type DrawItem,
  type FrameSubmission,
  type PostProcessItem,
  type TextureHandle,
  type ViewSize,
  type WgslMaterialSpec,
  type WgslPostProcessSpec,
} from "../types";
import {
  type MaterialModules,
  materialPipelineDescriptor,
  postProcessPipelineDescriptor,
  type ResourceBindings,
  resourceLayoutEntries,
  SHADER_STAGE,
} from "./materials";
import type { MeshRecord } from "./meshes";
import { viewDimensionOf } from "./resources";
import type { TextureBindingSpec } from "../types";
import {
  FRAME_BYTES,
  packFrame,
  type RingHost,
  type RingSlot,
  type StructLayout,
  UniformRing,
} from "./uniforms";

/** The colour a pass clears to: black, opaque. */
export const CLEAR_COLOUR: GPUColor = { r: 0, g: 0, b: 0, a: 1 };

/** The format of a post-process chain's intermediates. */
export const INTERMEDIATE_FORMAT: GPUTextureFormat = "rgba16float";

/** Whether a pipeline is ready, being made, or failed. */
type PipelineEntry = GPURenderPipeline | "pending" | "failed";

/** A material on the device: its modules, its layouts and its pipelines by mesh layout and target. */
export interface MaterialRecord {
  readonly spec: WgslMaterialSpec;
  readonly bindings: ResourceBindings;
  readonly modules: MaterialModules;
  readonly layout: GPUPipelineLayout;
  readonly resourceLayout: GPUBindGroupLayout;
  readonly drawLayout: StructLayout;
  /** Whether a missing pipeline is made asynchronously, its draws left out until it is ready. */
  readonly asyncPipelines: boolean;
  readonly pipelines: Map<string, PipelineEntry>;
  /** Set when its shaders failed to compile: its draws are left out. */
  broken: boolean;
}

/** A post-process on the device. */
export interface PostProcessRecord {
  readonly spec: WgslPostProcessSpec;
  readonly bindings: ResourceBindings;
  readonly vertex: GPUShaderModule;
  readonly fragment: GPUShaderModule;
  readonly layout: GPUPipelineLayout;
  readonly resourceLayout: GPUBindGroupLayout;
  readonly drawLayout: StructLayout;
  readonly pipelines: Map<string, PipelineEntry>;
  broken: boolean;
}

/** Where a frame is drawn. */
export interface FrameOutput {
  readonly size: ViewSize;
  readonly colour: GPUTextureView;
  readonly colourFormat: GPUTextureFormat;
  readonly depth: GPUTextureView | null;
  /** Two `rgba16float` textures of the output's size, asked for only when there are post-processes. */
  readonly intermediates: () => readonly [GPUTexture, GPUTexture];
}

/** What the draw path needs of the engine. */
export interface DrawingHost extends RingHost {
  readonly device: GPUDevice;
  materialOf(handle: DrawItem["material"]): MaterialRecord;
  meshOf(handle: DrawItem["mesh"]): MeshRecord;
  postProcessOf(handle: PostProcessItem["postProcess"]): PostProcessRecord;
  /** A texture's GPU object and specification. */
  textureOf(handle: TextureHandle): { readonly texture: GPUTexture; readonly spec: TextureSpec };
  sampler(spec: {
    readonly name: string;
    readonly filter: "nearest" | "linear";
    readonly address: "clamp-to-edge" | "repeat";
  }): GPUSampler;
}

/** A draw resolved to what its encoding needs. */
interface ResolvedDraw {
  readonly draw: DrawItem;
  readonly mesh: MeshRecord;
  readonly pipeline: GPURenderPipeline;
  readonly slot: RingSlot;
  readonly resources: GPUBindGroup;
  readonly indirect: { readonly buffer: GPUBuffer; readonly offsetBytes: number } | null;
}

/** A post-process resolved but for its input, which the chain decides. */
interface ResolvedPostProcess {
  readonly item: PostProcessItem;
  readonly record: PostProcessRecord;
  readonly pipeline: GPURenderPipeline;
  readonly slot: RingSlot;
}

/** A material's or post-process's pipeline for a key, made as `make` says, or `undefined` while it is being made. */
function pipelineFor(
  pipelines: Map<string, PipelineEntry>,
  key: string,
  owner: string,
  asynchronous: boolean,
  device: GPUDevice,
  descriptor: () => GPURenderPipelineDescriptor,
): GPURenderPipeline | undefined {
  const existing = pipelines.get(key);
  if (existing !== undefined) {
    return existing === "pending" || existing === "failed" ? undefined : existing;
  }
  if (!asynchronous) {
    const pipeline = device.createRenderPipeline(descriptor());
    pipelines.set(key, pipeline);
    return pipeline;
  }
  pipelines.set(key, "pending");
  void device
    .createRenderPipelineAsync(descriptor())
    .then((pipeline): void => {
      pipelines.set(key, pipeline);
      return undefined;
    })
    .catch((error: unknown) => {
      pipelines.set(key, "failed");
      console.error(`the pipeline of ${owner} could not be made:`, error);
    });
  return undefined;
}

/** A pipeline's key: the mesh's layout (or none), the colour format and whether there is depth. */
export function pipelineKey(
  layoutKey: string,
  colourFormat: GPUTextureFormat,
  hasDepth: boolean,
): string {
  return `${layoutKey}|${colourFormat}|${String(hasDepth)}`;
}

/** The draw path's device objects, shared by every view and target of one engine. */
export class Drawing {
  readonly #host: DrawingHost;
  readonly frameLayout: GPUBindGroupLayout;
  readonly drawLayout: GPUBindGroupLayout;
  readonly #frameBuffer: BufferHandle;
  readonly #frameBindGroup: GPUBindGroup;
  readonly #ring: UniformRing;
  readonly #fullScreen: GPUShaderModule;
  /** Each object's number, for cache keys. */
  readonly #ids = new WeakMap<object, number>();
  #nextId = 1;
  readonly #bindGroups = new Map<string, GPUBindGroup>();
  readonly #views = new WeakMap<GPUTexture, Map<string, GPUTextureView>>();

  constructor(host: DrawingHost, fullScreenWgsl: string) {
    this.#host = host;
    const { device } = host;
    const visibility = SHADER_STAGE.VERTEX | SHADER_STAGE.FRAGMENT;
    this.frameLayout = device.createBindGroupLayout({
      label: "frame",
      entries: [{ binding: 0, visibility, buffer: { type: "uniform" } }],
    });
    this.drawLayout = device.createBindGroupLayout({
      label: "draw",
      entries: [{ binding: 0, visibility, buffer: { type: "uniform", hasDynamicOffset: true } }],
    });
    this.#frameBuffer = host.createBuffer({
      name: "frame uniforms",
      bytes: FRAME_BYTES,
      usage: BUFFER_USAGE.UNIFORM | BUFFER_USAGE.COPY_DST,
      category: "other",
    });
    this.#frameBindGroup = device.createBindGroup({
      label: "frame",
      layout: this.frameLayout,
      entries: [{ binding: 0, resource: { buffer: host.gpuBufferOf(this.#frameBuffer) } }],
    });
    this.#ring = new UniformRing(host, this.drawLayout);
    this.#fullScreen = device.createShaderModule({ label: "full screen", code: fullScreenWgsl });
  }

  /** The full-screen vertex module every post-process shares. */
  get fullScreen(): GPUShaderModule {
    return this.#fullScreen;
  }

  /** The pipeline layout of a specification whose resources are laid out as `resourceLayout`. */
  pipelineLayout(label: string, resourceLayout: GPUBindGroupLayout): GPUPipelineLayout {
    return this.#host.device.createPipelineLayout({
      label,
      bindGroupLayouts: [this.frameLayout, this.drawLayout, resourceLayout],
    });
  }

  /** The `@group(2)` layout of `bindings`. */
  resourceLayout(bindings: ResourceBindings): GPUBindGroupLayout {
    return this.#host.device.createBindGroupLayout({
      label: bindings.owner,
      entries: [...resourceLayoutEntries(bindings)],
    });
  }

  /** The pipeline of `record` drawing `mesh` into an output, or `undefined` while it is made. */
  materialPipeline(
    record: MaterialRecord,
    mesh: MeshRecord,
    colourFormat: GPUTextureFormat,
    hasDepth: boolean,
    asynchronous = record.asyncPipelines,
  ): GPURenderPipeline | undefined {
    return pipelineFor(
      record.pipelines,
      pipelineKey(mesh.layoutKey, colourFormat, hasDepth),
      `material ${record.spec.name}`,
      asynchronous,
      this.#host.device,
      () =>
        materialPipelineDescriptor(
          record.spec,
          record.modules,
          record.layout,
          mesh.layouts,
          mesh.spec.topology,
          colourFormat,
          hasDepth,
        ),
    );
  }

  /** Makes, and awaits, the pipeline of `record` for `mesh` into an output. */
  async prepareMaterialPipeline(
    record: MaterialRecord,
    mesh: MeshRecord,
    colourFormat: GPUTextureFormat,
    hasDepth: boolean,
  ): Promise<void> {
    const key = pipelineKey(mesh.layoutKey, colourFormat, hasDepth);
    if (record.pipelines.has(key)) {
      return;
    }
    record.pipelines.set(key, "pending");
    try {
      record.pipelines.set(
        key,
        await this.#host.device.createRenderPipelineAsync(
          materialPipelineDescriptor(
            record.spec,
            record.modules,
            record.layout,
            mesh.layouts,
            mesh.spec.topology,
            colourFormat,
            hasDepth,
          ),
        ),
      );
    } catch (error: unknown) {
      record.pipelines.set(key, "failed");
      throw new Error(`the pipeline of material ${record.spec.name} could not be made`, {
        cause: error,
      });
    }
  }

  /**
   * Encodes `frame` into `output`: the draws' pass, then each post-process's.
   *
   * @remarks
   * The `Frame` buffer and the ring are shared by every view and target and rewritten from the
   * start for each frame, so the caller submits `encoder` before it encodes the next frame: the
   * queue then orders each frame's uploads before its own commands and after the previous ones.
   *
   * @param timestamps - The timestamp writes of a pass by its label, when the frame is timed.
   */
  encodeFrame(
    encoder: GPUCommandEncoder,
    frame: FrameSubmission,
    output: FrameOutput,
    timestamps: (label: string) => GPURenderPassTimestampWrites | undefined,
  ): void {
    const host = this.#host;
    this.#ring.begin();
    // A post-process whose shaders failed is left out of the chain; with none left, the draws go
    // straight to the output.
    const postProcesses = this.#resolvePostProcesses(frame, output.colourFormat);
    const intermediates = postProcesses.length > 0 ? output.intermediates() : null;
    const sceneFormat = intermediates === null ? output.colourFormat : INTERMEDIATE_FORMAT;
    const draws = this.#resolveDraws(frame, sceneFormat, output.depth !== null);
    host.writeBuffer(this.#frameBuffer, 0, packFrame(frame, output.size));
    this.#ring.upload();

    const sceneColour =
      intermediates === null ? output.colour : this.#viewOf(intermediates[0], "2d", false);
    const scene = encoder.beginRenderPass({
      label: frame.label,
      colorAttachments: [
        { view: sceneColour, loadOp: "clear", storeOp: "store", clearValue: CLEAR_COLOUR },
      ],
      ...(output.depth === null
        ? {}
        : {
            depthStencilAttachment: {
              view: output.depth,
              depthLoadOp: "clear",
              depthStoreOp: "store",
              depthClearValue: 0,
            },
          }),
      ...optionalTimestamps(timestamps(frame.label)),
    });
    scene.setBindGroup(0, this.#frameBindGroup);
    for (const resolved of draws) {
      encodeDraw(scene, resolved, this.#ring.bindGroup(resolved.slot));
    }
    scene.end();

    if (intermediates === null) {
      return;
    }
    const steps = chainSteps(postProcesses.length);
    postProcesses.forEach((resolved, index) => {
      const step = steps[index];
      if (step === undefined) {
        return;
      }
      const input = intermediates[step.input];
      const target =
        step.output === "output"
          ? output.colour
          : this.#viewOf(intermediates[step.output], "2d", false);
      const label = `${frame.label} ${resolved.record.spec.name}`;
      const pass = encoder.beginRenderPass({
        label,
        colorAttachments: [
          { view: target, loadOp: "clear", storeOp: "store", clearValue: CLEAR_COLOUR },
        ],
        ...optionalTimestamps(timestamps(label)),
      });
      pass.setPipeline(resolved.pipeline);
      pass.setBindGroup(0, this.#frameBindGroup);
      pass.setBindGroup(1, this.#ring.bindGroup(resolved.slot), [resolved.slot.offsetBytes]);
      pass.setBindGroup(2, this.#postProcessResources(resolved, input));
      pass.draw(3);
      pass.end();
    });
  }

  /** Destroys the draw path's buffers. */
  dispose(): void {
    this.#ring.dispose();
    this.#host.destroyBuffer(this.#frameBuffer);
    this.#bindGroups.clear();
  }

  /** Forgets every cached bind group, after a texture or buffer they may hold was destroyed. */
  forgetBindGroups(): void {
    this.#bindGroups.clear();
  }

  #resolveDraws(
    frame: FrameSubmission,
    colourFormat: GPUTextureFormat,
    hasDepth: boolean,
  ): ResolvedDraw[] {
    const resolved: ResolvedDraw[] = [];
    for (const draw of frame.draws) {
      if ((draw.instanceCount ?? 1) === 0 && draw.indirect === undefined) {
        continue;
      }
      const record = this.#host.materialOf(draw.material);
      const mesh = this.#host.meshOf(draw.mesh);
      if (record.broken) {
        continue;
      }
      const pipeline = this.materialPipeline(record, mesh, colourFormat, hasDepth);
      if (pipeline === undefined) {
        continue;
      }
      const slot = this.#ring.push(record.drawLayout, (name) =>
        name === "offsetFromCameraM" ? draw.offsetFromCameraM : draw.uniforms[name],
      );
      const { indirect } = draw;
      resolved.push({
        draw,
        mesh,
        pipeline,
        slot,
        resources: this.#drawResources(record, draw),
        indirect:
          indirect === undefined
            ? null
            : {
                buffer: this.#host.gpuBufferOf(indirect.buffer),
                offsetBytes: indirect.offsetBytes,
              },
      });
    }
    return resolved;
  }

  #resolvePostProcesses(
    frame: FrameSubmission,
    outputFormat: GPUTextureFormat,
  ): ResolvedPostProcess[] {
    const resolved: ResolvedPostProcess[] = [];
    const usable = frame.postProcesses
      .map((item) => ({ item, record: this.#host.postProcessOf(item.postProcess) }))
      .filter(({ record }) => !record.broken);
    usable.forEach(({ item, record }, index) => {
      const format = index === usable.length - 1 ? outputFormat : INTERMEDIATE_FORMAT;
      const pipeline = pipelineFor(
        record.pipelines,
        format,
        `post-process ${record.spec.name}`,
        false,
        this.#host.device,
        () =>
          postProcessPipelineDescriptor(
            record.spec,
            record.vertex,
            record.fragment,
            record.layout,
            format,
          ),
      );
      if (pipeline === undefined) {
        return;
      }
      const slot = this.#ring.push(record.drawLayout, (name) => item.uniforms[name]);
      resolved.push({ item, record, pipeline, slot });
    });
    return resolved;
  }

  #drawResources(record: MaterialRecord, draw: DrawItem): GPUBindGroup {
    const entries: Array<{
      readonly binding: number;
      readonly resource: GPUBindingResource;
      readonly key: object;
    }> = [];
    for (const texture of record.bindings.textures) {
      const handle = draw.textures[texture.name];
      if (handle === undefined) {
        throw new Error(`draw of ${record.spec.name} binds no texture ${texture.name}`);
      }
      const view = this.#boundView(`material ${record.spec.name}`, texture, handle);
      entries.push({ binding: texture.binding, resource: view, key: view });
    }
    for (const buffer of record.bindings.storageBuffers) {
      const handle = draw.storageBuffers?.[buffer.name];
      if (handle === undefined) {
        throw new Error(`draw of ${record.spec.name} binds no storage buffer ${buffer.name}`);
      }
      const gpuBuffer = this.#host.gpuBufferOf(handle);
      entries.push({ binding: buffer.binding, resource: { buffer: gpuBuffer }, key: gpuBuffer });
    }
    for (const sampler of record.bindings.samplers) {
      const gpuSampler = this.#host.sampler(sampler);
      entries.push({ binding: sampler.binding, resource: gpuSampler, key: gpuSampler });
    }
    return this.#bindGroup(record.resourceLayout, record.spec.name, entries);
  }

  #postProcessResources(resolved: ResolvedPostProcess, input: GPUTexture): GPUBindGroup {
    const { record, item } = resolved;
    const entries: Array<{
      readonly binding: number;
      readonly resource: GPUBindingResource;
      readonly key: object;
    }> = [];
    for (const texture of record.bindings.textures) {
      let view: GPUTextureView;
      if (texture.name === "hdr-colour") {
        view = this.#viewOf(input, "2d", false);
      } else {
        const handle = item.textures?.[texture.name];
        if (handle === undefined) {
          throw new Error(`post-process ${record.spec.name} is given no texture ${texture.name}`);
        }
        view = this.#boundView(`post-process ${record.spec.name}`, texture, handle);
      }
      entries.push({ binding: texture.binding, resource: view, key: view });
    }
    for (const sampler of record.bindings.samplers) {
      const gpuSampler = this.#host.sampler(sampler);
      entries.push({ binding: sampler.binding, resource: gpuSampler, key: gpuSampler });
    }
    return this.#bindGroup(record.resourceLayout, record.spec.name, entries);
  }

  #bindGroup(
    layout: GPUBindGroupLayout,
    label: string,
    entries: ReadonlyArray<{
      readonly binding: number;
      readonly resource: GPUBindingResource;
      readonly key: object;
    }>,
  ): GPUBindGroup {
    const key = [
      this.#id(layout),
      ...entries.map((entry) => `${entry.binding}:${this.#id(entry.key)}`),
    ].join(",");
    let bindGroup = this.#bindGroups.get(key);
    if (bindGroup === undefined) {
      if (this.#bindGroups.size > 4096) {
        this.#bindGroups.clear();
      }
      bindGroup = this.#host.device.createBindGroup({
        label,
        layout,
        entries: entries.map(({ binding, resource }) => ({ binding, resource })),
      });
      this.#bindGroups.set(key, bindGroup);
    }
    return bindGroup;
  }

  /**
   * The view a texture binding takes, checked against what the layout declared.
   *
   * @throws Error naming the owner and the binding when the texture's dimension is not the one
   * declared (`2d` by default), or a depth texture is not declared `depth`, or the reverse: the
   * bind group would fail validation and the draw silently draw nothing.
   */
  #boundView(owner: string, binding: TextureBindingSpec, handle: TextureHandle): GPUTextureView {
    const { texture, spec } = this.#host.textureOf(handle);
    const declared = binding.viewDimension ?? "2d";
    const own = viewDimensionOf(spec);
    if (own !== declared) {
      throw new Error(
        `${owner} declares ${binding.name} as ${declared}, but ${handle.name} is ${own}`,
      );
    }
    const depth = spec.format.startsWith("depth");
    if (depth !== (binding.sampleType === "depth")) {
      throw new Error(
        `${owner} declares ${binding.name} as ${binding.sampleType ?? "float"}, but ${handle.name} is ${spec.format}`,
      );
    }
    return this.#viewOf(texture, declared, depth);
  }

  /** A view of every mip of `texture` as `dimension`, its depth aspect alone for a depth format. */
  #viewOf(texture: GPUTexture, dimension: GPUTextureViewDimension, depth: boolean): GPUTextureView {
    const views = this.#views.get(texture) ?? new Map<string, GPUTextureView>();
    this.#views.set(texture, views);
    let view = views.get(dimension);
    if (view === undefined) {
      view = texture.createView({ dimension, ...(depth ? { aspect: "depth-only" } : {}) });
      views.set(dimension, view);
    }
    return view;
  }

  #id(object: object): number {
    let id = this.#ids.get(object);
    if (id === undefined) {
      id = this.#nextId;
      this.#nextId += 1;
      this.#ids.set(object, id);
    }
    return id;
  }
}

/** One post-process of a chain: the intermediate it reads, and where it writes. */
export interface ChainStep {
  readonly input: 0 | 1;
  readonly output: 0 | 1 | "output";
}

/**
 * The ping-pong of a chain of `count` post-processes: the draws go to intermediate 0, each pass
 * reads the one the pass before wrote, and the last writes the output.
 */
export function chainSteps(count: number): ReadonlyArray<ChainStep> {
  return Array.from({ length: count }, (_, index) => {
    const input = index % 2 === 0 ? 0 : 1;
    return { input, output: index === count - 1 ? "output" : input === 0 ? 1 : 0 };
  });
}

/** A pass descriptor's timestamp writes, when there are some. */
function optionalTimestamps(writes: GPURenderPassTimestampWrites | undefined): {
  readonly timestampWrites?: GPURenderPassTimestampWrites;
} {
  return writes === undefined ? {} : { timestampWrites: writes };
}

/** Encodes one resolved draw: direct, indexed or indirect, as its mesh and item say. */
function encodeDraw(
  pass: GPURenderPassEncoder,
  resolved: ResolvedDraw,
  drawGroup: GPUBindGroup,
): void {
  const { draw, mesh, pipeline, slot, resources } = resolved;
  pass.setPipeline(pipeline);
  pass.setBindGroup(1, drawGroup, [slot.offsetBytes]);
  pass.setBindGroup(2, resources);
  mesh.vertexBuffers.forEach((buffer, index) => {
    pass.setVertexBuffer(index, buffer);
  });
  if (mesh.index !== null) {
    pass.setIndexBuffer(mesh.index.buffer, "uint32");
  }
  if (resolved.indirect !== null) {
    // The GPU wrote the counts (Design note 19), in WebGPU's layout of the draw call.
    const { buffer, offsetBytes } = resolved.indirect;
    if (mesh.index === null) {
      pass.drawIndirect(buffer, offsetBytes);
    } else {
      pass.drawIndexedIndirect(buffer, offsetBytes);
    }
    return;
  }
  const instances = draw.instanceCount ?? 1;
  if (mesh.index === null) {
    pass.draw(mesh.vertexCount, instances);
  } else {
    pass.drawIndexed(mesh.index.count, instances);
  }
}
