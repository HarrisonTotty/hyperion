/**
 * A fake WebGPU entry point, adapter and device for tests in the `logic` project.
 *
 * @remarks
 * Only what the platform and status code reads is faked: adapter info, features and limits, the
 * consumption of an adapter by its first `requestDevice`, and a device whose `lost` promise the
 * test resolves. Every other device method throws, naming itself, so that a test that strays into
 * rendering fails loudly rather than passing on a stub.
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
  // The four per-stage storage limits default to the per-shader-stage ones; lib.dom lacks them, but
  // `@babylonjs/core` declares them on the global `GPUSupportedLimits`.
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

  get queue(): GPUQueue {
    throw notFaked("queue");
  }

  destroy(): void {
    if (!this.#destroyed) {
      this.#destroyed = true;
      this.loseDevice("destroyed", "destroyed");
    }
  }

  createBindGroup(): GPUBindGroup {
    throw notFaked("createBindGroup");
  }
  createBindGroupLayout(): GPUBindGroupLayout {
    throw notFaked("createBindGroupLayout");
  }
  createBuffer(): GPUBuffer {
    throw notFaked("createBuffer");
  }
  createCommandEncoder(): GPUCommandEncoder {
    throw notFaked("createCommandEncoder");
  }
  createComputePipeline(): GPUComputePipeline {
    throw notFaked("createComputePipeline");
  }
  createComputePipelineAsync(): Promise<GPUComputePipeline> {
    return Promise.reject(notFaked("createComputePipelineAsync"));
  }
  createPipelineLayout(): GPUPipelineLayout {
    throw notFaked("createPipelineLayout");
  }
  createQuerySet(): GPUQuerySet {
    throw notFaked("createQuerySet");
  }
  createRenderBundleEncoder(): GPURenderBundleEncoder {
    throw notFaked("createRenderBundleEncoder");
  }
  createRenderPipeline(): GPURenderPipeline {
    throw notFaked("createRenderPipeline");
  }
  createRenderPipelineAsync(): Promise<GPURenderPipeline> {
    return Promise.reject(notFaked("createRenderPipelineAsync"));
  }
  createSampler(): GPUSampler {
    throw notFaked("createSampler");
  }
  createShaderModule(): GPUShaderModule {
    throw notFaked("createShaderModule");
  }
  createTexture(): GPUTexture {
    throw notFaked("createTexture");
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
