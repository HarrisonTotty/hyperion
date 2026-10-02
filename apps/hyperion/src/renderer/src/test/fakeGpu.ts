/**
 * A fake WebGPU entry point, adapter and device for tests in the `logic` project.
 *
 * @remarks
 * Faked: adapter info, features and limits, the consumption of an adapter by its first
 * `requestDevice`, a device whose `lost` promise the test resolves, and the engine's own objects:
 * buffers, textures, samplers, shader modules (whose compile errors the test chooses), layouts,
 * pipelines and bind groups, each recording its descriptor, and command encoders whose passes
 * record every command, so that a test reads what a frame encoded. Nothing is executed, so a mapped
 * buffer reads zeros. What the engine never calls (render bundles, external textures, error scopes)
 * throws,
 * naming itself, so that a test that strays there fails loudly rather than passing on a stub.
 */

/**
 * The UHD 620's adapter info: vendor, architecture, empty device and description and no fallback flag
 * as the probe of 2026-09-29 read them (R01 Design note 8); the subgroup sizes are illustrative.
 */
export const INTEL_UHD_620_INFO: FakeAdapterInfo = {
  vendor: "intel",
  architecture: "gen-9",
  device: "",
  description: "",
  isFallbackAdapter: false,
  subgroupMinSize: 8,
  subgroupMaxSize: 32,
};

/**
 * SwiftShader's adapter info: vendor, architecture and fallback flag as the probe read them (R01
 * Design note 17); the description and subgroup sizes are illustrative.
 */
export const SWIFTSHADER_INFO: FakeAdapterInfo = {
  vendor: "google",
  architecture: "swiftshader",
  device: "",
  description: "SwiftShader device (Subzero)",
  isFallbackAdapter: true,
  subgroupMinSize: 4,
  subgroupMaxSize: 4,
};

/** The fields of `GPUAdapterInfo`, as plain data. */
export type FakeAdapterInfo = Omit<GPUAdapterInfo, never>;

/** WebGPU's default limits. */
const DEFAULT_LIMITS = {
  maxBindGroups: 4,
  maxBindGroupsPlusVertexBuffers: 24,
  maxBindingsPerBindGroup: 1000,
  maxBufferSize: 268_435_456,
  maxColorAttachmentBytesPerSample: 32,
  maxColorAttachments: 8,
  maxComputeInvocationsPerWorkgroup: 256,
  maxComputeWorkgroupSizeX: 256,
  maxComputeWorkgroupSizeY: 256,
  maxComputeWorkgroupSizeZ: 64,
  maxComputeWorkgroupStorageSize: 16_384,
  maxComputeWorkgroupsPerDimension: 65_535,
  maxDynamicStorageBuffersPerPipelineLayout: 4,
  maxDynamicUniformBuffersPerPipelineLayout: 8,
  maxInterStageShaderVariables: 16,
  maxSampledTexturesPerShaderStage: 16,
  maxSamplersPerShaderStage: 16,
  maxStorageBufferBindingSize: 134_217_728,
  // The four per-stage storage limits default to the per-shader-stage ones; lib.dom lacks them, and
  // `webgpu.d.ts` declares them on the global `GPUSupportedLimits`.
  maxStorageBuffersInFragmentStage: 8,
  maxStorageBuffersInVertexStage: 8,
  maxStorageBuffersPerShaderStage: 8,
  maxStorageTexturesInFragmentStage: 4,
  maxStorageTexturesInVertexStage: 4,
  maxStorageTexturesPerShaderStage: 4,
  maxTextureArrayLayers: 256,
  maxTextureDimension1D: 8192,
  maxTextureDimension2D: 8192,
  maxTextureDimension3D: 2048,
  maxUniformBufferBindingSize: 65_536,
  maxUniformBuffersPerShaderStage: 12,
  maxVertexAttributes: 16,
  maxVertexBufferArrayStride: 2048,
  maxVertexBuffers: 8,
  minStorageBufferOffsetAlignment: 256,
  minUniformBufferOffsetAlignment: 256,
} as const satisfies GPUSupportedLimits;

function notFaked(member: string): Error {
  return new Error(`FakeDevice does not fake ${member}`);
}

/** A buffer that records its destruction, and maps to zeros, since nothing ran on it. */
export class FakeBuffer implements GPUBuffer {
  readonly size: number;
  readonly usage: GPUFlagsConstant;
  readonly mapState: GPUBufferMapState = "unmapped";
  label: string;
  destroyed = false;

  constructor(descriptor: GPUBufferDescriptor) {
    this.size = descriptor.size;
    this.usage = descriptor.usage;
    this.label = descriptor.label ?? "";
  }

  mapAsync(): Promise<undefined> {
    return this.destroyed
      ? Promise.reject(new Error(`${this.label} is destroyed`))
      : Promise.resolve(undefined);
  }
  getMappedRange(): ArrayBuffer {
    return new ArrayBuffer(this.size);
  }
  unmap(): undefined {
    return undefined;
  }
  destroy(): undefined {
    this.destroyed = true;
    return undefined;
  }
}

/** A view that knows its texture and its descriptor, so a test can tell what a pass bound. */
export class FakeTextureView implements GPUTextureView {
  label: string;
  readonly texture: FakeTexture;
  readonly descriptor: GPUTextureViewDescriptor | undefined;

  constructor(texture: FakeTexture, descriptor: GPUTextureViewDescriptor | undefined) {
    this.label = descriptor?.label ?? "";
    this.texture = texture;
    this.descriptor = descriptor;
  }
}

/** A texture that records its descriptor and its destruction. */
export class FakeTexture implements GPUTexture {
  readonly descriptor: GPUTextureDescriptor;
  label: string;
  destroyed = false;

  constructor(descriptor: GPUTextureDescriptor) {
    this.descriptor = descriptor;
    this.label = descriptor.label ?? "";
  }

  #extent(): readonly [number, number, number] {
    const { size } = this.descriptor;
    if (Symbol.iterator in size) {
      const [width = 1, height = 1, layers = 1] = [...size];
      return [width, height, layers];
    }
    return [size.width, size.height ?? 1, size.depthOrArrayLayers ?? 1];
  }

  get width(): number {
    return this.#extent()[0];
  }
  get height(): number {
    return this.#extent()[1];
  }
  get depthOrArrayLayers(): number {
    return this.#extent()[2];
  }
  get mipLevelCount(): number {
    return this.descriptor.mipLevelCount ?? 1;
  }
  get sampleCount(): number {
    return this.descriptor.sampleCount ?? 1;
  }
  get dimension(): GPUTextureDimension {
    return this.descriptor.dimension ?? "2d";
  }
  get format(): GPUTextureFormat {
    return this.descriptor.format;
  }
  get usage(): GPUFlagsConstant {
    return this.descriptor.usage;
  }
  get textureBindingViewDimension(): GPUTextureViewDimension | undefined {
    return this.descriptor.textureBindingViewDimension;
  }
  /** The descriptor of every view made, first first. */
  readonly views: Array<GPUTextureViewDescriptor | undefined> = [];

  createView(descriptor?: GPUTextureViewDescriptor): GPUTextureView {
    this.views.push(descriptor);
    return new FakeTextureView(this, descriptor);
  }
  destroy(): undefined {
    this.destroyed = true;
    return undefined;
  }
}

/** One write a fake queue received. */
export type FakeQueueWrite =
  | {
      readonly kind: "buffer";
      readonly label: string;
      readonly offset: number;
      readonly bytes: number;
    }
  | {
      readonly kind: "texture";
      readonly label: string;
      readonly mipLevel: number;
      readonly bytesPerRow: number | undefined;
      readonly bytes: number;
    };

/** A queue that records its writes and submissions. */
export class FakeQueue implements GPUQueue {
  label = "";
  readonly writes: FakeQueueWrite[] = [];
  readonly submitted: GPUCommandBuffer[] = [];

  submit(commandBuffers: Iterable<GPUCommandBuffer>): undefined {
    this.submitted.push(...commandBuffers);
    return undefined;
  }
  onSubmittedWorkDone(): Promise<undefined> {
    return Promise.resolve(undefined);
  }
  writeBuffer(
    buffer: GPUBuffer,
    bufferOffset: number,
    _data: unknown,
    _dataOffset?: number,
    size?: number,
  ): undefined {
    this.writes.push({
      kind: "buffer",
      label: buffer.label,
      offset: bufferOffset,
      bytes: size ?? 0,
    });
    return undefined;
  }
  writeTexture(
    destination: GPUTexelCopyTextureInfo,
    data: AllowSharedBufferSource,
    dataLayout: GPUTexelCopyBufferLayout,
  ): undefined {
    this.writes.push({
      kind: "texture",
      label: destination.texture.label,
      mipLevel: destination.mipLevel ?? 0,
      bytesPerRow: dataLayout.bytesPerRow,
      bytes: data.byteLength,
    });
    return undefined;
  }
  copyExternalImageToTexture(): undefined {
    throw notFaked("GPUQueue.copyExternalImageToTexture");
  }
  copyElementImageToTexture(): undefined {
    throw notFaked("GPUQueue.copyElementImageToTexture");
  }
}

/** An object the fake device made, with the descriptor it was made from. */
export class FakeObject<Descriptor> {
  readonly descriptor: Descriptor;
  label: string;

  constructor(descriptor: Descriptor, label: string | undefined) {
    this.descriptor = descriptor;
    this.label = label ?? "";
  }
}

/** A pipeline whose bind-group layouts are made on demand, as an `auto` layout's are. */
export class FakePipeline<Descriptor> extends FakeObject<Descriptor> {
  readonly #groups = new Map<number, GPUBindGroupLayout>();

  getBindGroupLayout(index: number): GPUBindGroupLayout {
    let layout = this.#groups.get(index);
    if (layout === undefined) {
      layout = { label: `${this.label} group ${index}` };
      this.#groups.set(index, layout);
    }
    return layout;
  }
}

/** A shader module whose compile errors are the ones its device was told to give. */
export class FakeShaderModule extends FakeObject<GPUShaderModuleDescriptor> {
  readonly #errors: ReadonlyArray<string>;

  constructor(descriptor: GPUShaderModuleDescriptor, errors: ReadonlyArray<string>) {
    super(descriptor, descriptor.label);
    this.#errors = errors;
  }

  getCompilationInfo(): Promise<GPUCompilationInfo> {
    return Promise.resolve({
      messages: this.#errors.map((message, index) => ({
        type: "error" as const,
        lineNum: index + 1,
        linePos: 1,
        offset: 0,
        length: 0,
        message,
      })),
    });
  }
}

/** A query set that records its destruction. */
export class FakeQuerySet extends FakeObject<GPUQuerySetDescriptor> implements GPUQuerySet {
  destroyed = false;

  get count(): number {
    return this.descriptor.count;
  }
  get type(): GPUQueryType {
    return this.descriptor.type;
  }
  destroy(): undefined {
    this.destroyed = true;
    return undefined;
  }
}

/** One command an encoder or one of its passes recorded, with its arguments. */
export interface FakeCommand {
  readonly op: string;
  readonly args: ReadonlyArray<unknown>;
}

/** What a render and a compute pass share: their commands go into their encoder's log. */
class FakePassBase {
  label: string;
  protected readonly log: FakeCommand[];

  constructor(log: FakeCommand[], label: string) {
    this.log = log;
    this.label = label;
  }

  protected record(op: string, ...args: ReadonlyArray<unknown>): undefined {
    this.log.push({ op, args });
    return undefined;
  }

  setBindGroup(
    index: number,
    bindGroup: GPUBindGroup | null,
    dynamicOffsets?: Iterable<number>,
  ): undefined {
    return this.record("setBindGroup", index, bindGroup, [...(dynamicOffsets ?? [])]);
  }
  insertDebugMarker(marker: string): undefined {
    return this.record("insertDebugMarker", marker);
  }
  popDebugGroup(): undefined {
    return this.record("popDebugGroup");
  }
  pushDebugGroup(group: string): undefined {
    return this.record("pushDebugGroup", group);
  }
  end(): undefined {
    return this.record("end");
  }
}

/** A render pass that records its commands in its encoder's log. */
export class FakeRenderPassEncoder extends FakePassBase implements GPURenderPassEncoder {
  setPipeline(pipeline: GPURenderPipeline): undefined {
    return this.record("setPipeline", pipeline);
  }
  setVertexBuffer(slot: number, buffer: GPUBuffer | null): undefined {
    return this.record("setVertexBuffer", slot, buffer);
  }
  setIndexBuffer(buffer: GPUBuffer, format: GPUIndexFormat): undefined {
    return this.record("setIndexBuffer", buffer, format);
  }
  draw(vertexCount: number, instanceCount = 1): undefined {
    return this.record("draw", vertexCount, instanceCount);
  }
  drawIndexed(indexCount: number, instanceCount = 1): undefined {
    return this.record("drawIndexed", indexCount, instanceCount);
  }
  drawIndirect(buffer: GPUBuffer, offset: number): undefined {
    return this.record("drawIndirect", buffer, offset);
  }
  drawIndexedIndirect(buffer: GPUBuffer, offset: number): undefined {
    return this.record("drawIndexedIndirect", buffer, offset);
  }
  setViewport(
    x: number,
    y: number,
    width: number,
    height: number,
    minDepth: number,
    maxDepth: number,
  ): undefined {
    return this.record("setViewport", x, y, width, height, minDepth, maxDepth);
  }
  setScissorRect(x: number, y: number, width: number, height: number): undefined {
    return this.record("setScissorRect", x, y, width, height);
  }
  setBlendConstant(colour: GPUColor | Iterable<number>): undefined {
    return this.record("setBlendConstant", colour);
  }
  setStencilReference(reference: number): undefined {
    return this.record("setStencilReference", reference);
  }
  beginOcclusionQuery(index: number): undefined {
    return this.record("beginOcclusionQuery", index);
  }
  endOcclusionQuery(): undefined {
    return this.record("endOcclusionQuery");
  }
  executeBundles(): undefined {
    throw notFaked("GPURenderPassEncoder.executeBundles");
  }
}

/** A compute pass that records its commands in its encoder's log. */
export class FakeComputePassEncoder extends FakePassBase implements GPUComputePassEncoder {
  setPipeline(pipeline: GPUComputePipeline): undefined {
    return this.record("setPipeline", pipeline);
  }
  dispatchWorkgroups(x: number, y = 1, z = 1): undefined {
    return this.record("dispatchWorkgroups", x, y, z);
  }
  dispatchWorkgroupsIndirect(buffer: GPUBuffer, offset: number): undefined {
    return this.record("dispatchWorkgroupsIndirect", buffer, offset);
  }
}

/** A finished encoder's commands, as the queue receives them. */
export class FakeCommandBuffer implements GPUCommandBuffer {
  label: string;
  readonly commands: ReadonlyArray<FakeCommand>;

  constructor(label: string, commands: ReadonlyArray<FakeCommand>) {
    this.label = label;
    this.commands = commands;
  }
}

/** An encoder that records its commands and its passes', in order, in one log. */
export class FakeCommandEncoder implements GPUCommandEncoder {
  label: string;
  readonly commands: FakeCommand[] = [];

  constructor(descriptor?: GPUCommandEncoderDescriptor) {
    this.label = descriptor?.label ?? "";
  }

  #record(op: string, ...args: ReadonlyArray<unknown>): undefined {
    this.commands.push({ op, args });
    return undefined;
  }

  beginRenderPass(descriptor: GPURenderPassDescriptor): GPURenderPassEncoder {
    this.#record("beginRenderPass", descriptor);
    return new FakeRenderPassEncoder(this.commands, descriptor.label ?? "");
  }
  beginComputePass(descriptor?: GPUComputePassDescriptor): GPUComputePassEncoder {
    this.#record("beginComputePass", descriptor);
    return new FakeComputePassEncoder(this.commands, descriptor?.label ?? "");
  }
  clearBuffer(buffer: GPUBuffer, offset?: number, size?: number): undefined {
    return this.#record("clearBuffer", buffer, offset, size);
  }
  copyBufferToBuffer(...args: ReadonlyArray<unknown>): undefined {
    return this.#record("copyBufferToBuffer", ...args);
  }
  copyBufferToTexture(
    source: GPUTexelCopyBufferInfo,
    destination: GPUTexelCopyTextureInfo,
    size: GPUExtent3D | Iterable<number>,
  ): undefined {
    return this.#record("copyBufferToTexture", source, destination, size);
  }
  copyTextureToBuffer(
    source: GPUTexelCopyTextureInfo,
    destination: GPUTexelCopyBufferInfo,
    size: GPUExtent3D | Iterable<number>,
  ): undefined {
    return this.#record("copyTextureToBuffer", source, destination, size);
  }
  copyTextureToTexture(
    source: GPUTexelCopyTextureInfo,
    destination: GPUTexelCopyTextureInfo,
    size: GPUExtent3D | Iterable<number>,
  ): undefined {
    return this.#record("copyTextureToTexture", source, destination, size);
  }
  resolveQuerySet(
    querySet: GPUQuerySet,
    first: number,
    count: number,
    destination: GPUBuffer,
    offset: number,
  ): undefined {
    return this.#record("resolveQuerySet", querySet, first, count, destination, offset);
  }
  insertDebugMarker(marker: string): undefined {
    return this.#record("insertDebugMarker", marker);
  }
  popDebugGroup(): undefined {
    return this.#record("popDebugGroup");
  }
  pushDebugGroup(group: string): undefined {
    return this.#record("pushDebugGroup", group);
  }
  finish(): GPUCommandBuffer {
    return new FakeCommandBuffer(this.label, [...this.commands]);
  }
}

/** A device whose loss the test controls. */
export class FakeDevice extends EventTarget implements GPUDevice {
  readonly adapterInfo: GPUAdapterInfo;
  readonly features: GPUSupportedFeatures;
  readonly limits: GPUSupportedLimits;
  readonly lost: Promise<GPUDeviceLostInfo>;
  label = "";
  onuncapturederror: ((this: GPUDevice, ev: GPUUncapturedErrorEvent) => unknown) | null = null;
  /** The features the device was asked for. */
  readonly requiredFeatures: ReadonlyArray<GPUFeatureName>;
  #resolveLost: (info: GPUDeviceLostInfo) => void = () => undefined;
  #destroyed = false;

  constructor(
    adapterInfo: GPUAdapterInfo,
    requiredFeatures: ReadonlyArray<GPUFeatureName>,
    limits: GPUSupportedLimits,
  ) {
    super();
    this.adapterInfo = adapterInfo;
    this.requiredFeatures = requiredFeatures;
    this.features = new Set<string>(requiredFeatures);
    this.limits = limits;
    this.lost = new Promise((resolve) => {
      this.#resolveLost = resolve;
    });
  }

  /** Resolves `lost`, as a GPU-process crash does with reason `unknown`. */
  loseDevice(reason: GPUDeviceLostReason = "unknown", message = "fake loss"): void {
    this.#resolveLost({ reason, message });
  }

  /** The device's queue, recording what it is given. */
  readonly queue = new FakeQueue();
  /** Every buffer and texture made, first first. */
  readonly buffers: FakeBuffer[] = [];
  readonly textures: FakeTexture[] = [];
  /** Every other object made, by kind, first first. */
  readonly bindGroups: Array<FakeObject<GPUBindGroupDescriptor>> = [];
  readonly bindGroupLayouts: Array<FakeObject<GPUBindGroupLayoutDescriptor>> = [];
  readonly pipelineLayouts: Array<FakeObject<GPUPipelineLayoutDescriptor>> = [];
  readonly renderPipelines: Array<FakePipeline<GPURenderPipelineDescriptor>> = [];
  readonly computePipelines: Array<FakePipeline<GPUComputePipelineDescriptor>> = [];
  readonly samplers: Array<FakeObject<GPUSamplerDescriptor>> = [];
  readonly shaderModules: FakeShaderModule[] = [];
  readonly querySets: FakeQuerySet[] = [];
  readonly encoders: FakeCommandEncoder[] = [];
  /** The compile errors a shader module of `code` gives: none, unless the test says otherwise. */
  shaderErrors: (code: string) => ReadonlyArray<string> = () => [];

  destroy(): void {
    if (!this.#destroyed) {
      this.#destroyed = true;
      this.loseDevice("destroyed", "destroyed");
    }
  }

  createBindGroup(descriptor: GPUBindGroupDescriptor): GPUBindGroup {
    const group = new FakeObject(descriptor, descriptor.label);
    this.bindGroups.push(group);
    return group;
  }
  createBindGroupLayout(descriptor: GPUBindGroupLayoutDescriptor): GPUBindGroupLayout {
    const layout = new FakeObject(descriptor, descriptor.label);
    this.bindGroupLayouts.push(layout);
    return layout;
  }
  createBuffer(descriptor: GPUBufferDescriptor): GPUBuffer {
    const buffer = new FakeBuffer(descriptor);
    this.buffers.push(buffer);
    return buffer;
  }
  createCommandEncoder(descriptor?: GPUCommandEncoderDescriptor): GPUCommandEncoder {
    const encoder = new FakeCommandEncoder(descriptor);
    this.encoders.push(encoder);
    return encoder;
  }
  createComputePipeline(descriptor: GPUComputePipelineDescriptor): GPUComputePipeline {
    const pipeline = new FakePipeline(descriptor, descriptor.label);
    this.computePipelines.push(pipeline);
    return pipeline;
  }
  createComputePipelineAsync(
    descriptor: GPUComputePipelineDescriptor,
  ): Promise<GPUComputePipeline> {
    return Promise.resolve(this.createComputePipeline(descriptor));
  }
  createPipelineLayout(descriptor: GPUPipelineLayoutDescriptor): GPUPipelineLayout {
    const layout = new FakeObject(descriptor, descriptor.label);
    this.pipelineLayouts.push(layout);
    return layout;
  }
  createQuerySet(descriptor: GPUQuerySetDescriptor): GPUQuerySet {
    const querySet = new FakeQuerySet(descriptor, descriptor.label);
    this.querySets.push(querySet);
    return querySet;
  }
  createRenderBundleEncoder(): GPURenderBundleEncoder {
    throw notFaked("createRenderBundleEncoder");
  }
  createRenderPipeline(descriptor: GPURenderPipelineDescriptor): GPURenderPipeline {
    const pipeline = new FakePipeline(descriptor, descriptor.label);
    this.renderPipelines.push(pipeline);
    return pipeline;
  }
  createRenderPipelineAsync(descriptor: GPURenderPipelineDescriptor): Promise<GPURenderPipeline> {
    return Promise.resolve(this.createRenderPipeline(descriptor));
  }
  createSampler(descriptor: GPUSamplerDescriptor = {}): GPUSampler {
    const sampler = new FakeObject(descriptor, descriptor.label);
    this.samplers.push(sampler);
    return sampler;
  }
  createShaderModule(descriptor: GPUShaderModuleDescriptor): GPUShaderModule {
    const module = new FakeShaderModule(descriptor, this.shaderErrors(descriptor.code));
    this.shaderModules.push(module);
    return module;
  }
  createTexture(descriptor: GPUTextureDescriptor): GPUTexture {
    const texture = new FakeTexture(descriptor);
    this.textures.push(texture);
    return texture;
  }
  importExternalTexture(): GPUExternalTexture {
    throw notFaked("importExternalTexture");
  }
  popErrorScope(): Promise<GPUError | null> {
    return Promise.reject(notFaked("popErrorScope"));
  }
  pushErrorScope(): void {
    throw notFaked("pushErrorScope");
  }
}

/** What a fake adapter reports. */
export interface FakeAdapterOptions {
  readonly info: FakeAdapterInfo;
  readonly features: ReadonlyArray<GPUFeatureName>;
  /** Pixels on a side; WebGPU's default, 8192, when absent. */
  readonly maxTextureDimension2D?: number;
}

/**
 * An adapter consumed by its first `requestDevice`, as the WebGPU specification's is: a second
 * call gives a device whose `lost` has already resolved.
 */
export class FakeAdapter implements GPUAdapter {
  readonly info: GPUAdapterInfo;
  readonly features: GPUSupportedFeatures;
  readonly limits: GPUSupportedLimits;
  /** Every device this adapter has handed out, first first. */
  readonly devices: FakeDevice[] = [];
  #consumed = false;

  constructor(options: FakeAdapterOptions) {
    this.info = options.info;
    this.features = new Set<string>(options.features);
    this.limits = {
      ...DEFAULT_LIMITS,
      maxTextureDimension2D: options.maxTextureDimension2D ?? DEFAULT_LIMITS.maxTextureDimension2D,
    };
  }

  requestDevice(descriptor?: GPUDeviceDescriptor): Promise<GPUDevice> {
    const required = [...(descriptor?.requiredFeatures ?? [])];
    for (const feature of required) {
      if (!this.features.has(feature)) {
        return Promise.reject(new TypeError(`the adapter lacks ${feature}`));
      }
    }
    const device = new FakeDevice(this.info, required, this.limits);
    if (this.#consumed) {
      device.loseDevice("unknown", "the adapter was already consumed");
    }
    this.#consumed = true;
    this.devices.push(device);
    return Promise.resolve(device);
  }
}

/** `navigator.gpu`, handing out the adapters the test queues, then null. */
export class FakeGpu implements GPU {
  readonly wgslLanguageFeatures: WGSLLanguageFeatures = new Set<string>();
  /** The options of every `requestAdapter` call, first first. */
  readonly requests: Array<GPURequestAdapterOptions | undefined> = [];
  readonly #adapters: Array<FakeAdapter | null>;

  constructor(adapters: ReadonlyArray<FakeAdapter | null>) {
    this.#adapters = [...adapters];
  }

  getPreferredCanvasFormat(): GPUTextureFormat {
    return "bgra8unorm";
  }

  requestAdapter(options?: GPURequestAdapterOptions): Promise<GPUAdapter | null> {
    this.requests.push(options);
    return Promise.resolve(this.#adapters.shift() ?? null);
  }
}
