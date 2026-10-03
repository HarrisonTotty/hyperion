/**
 * The descent spike's measurement seam on the GPU device (plan R05, T14.a, Design note 18): a
 * wrapped `GPU` whose adapters hand out devices passed through a wrapper first, and the wrapper
 * that counts pipeline creations after warm-up.
 *
 * @remarks
 * No `GPUDevice` is reachable outside R01's adapter (`engineBoundary.test.ts`), so the spike's
 * `ViewEngineSource` asks for its adapter through {@link wrapGpu}'s `GPU` and hands the same `GPU`
 * to the engine as `LoadEngineOptions.gpu`, so that a rebuild after a device loss is wrapped too.
 * R01's adapter itself is not edited.
 *
 * The device keeps its identity: its methods are replaced on the instance, each calling the
 * prototype's, rather than the device being put behind a `Proxy`, because the browser's own
 * WebGPU calls (`GPUCanvasContext.configure({ device })`, a pass's `setPipeline`) brand-check
 * their arguments and refuse a proxy. The capture (T15.a) wraps the same way.
 */

/** A pipeline creation the shim saw. */
export interface PipelineCreation {
  readonly label: string;
  readonly kind: "render" | "compute";
  /** Whether it was the asynchronous form. */
  readonly async: boolean;
  /** When, on the tally's clock, ms. */
  readonly atMs: number;
  /** Whether it came after the warm-up ended: a pipeline compiled mid-run, a likely hitch. */
  readonly late: boolean;
}

/** Counts pipeline creations, and marks those after the warm-up. */
export class PipelineTally {
  readonly #nowMs: () => number;
  readonly #creations: PipelineCreation[] = [];
  #warm = false;

  constructor(nowMs: () => number) {
    this.#nowMs = nowMs;
  }

  /** Ends the warm-up: every creation from now on is late. */
  endWarmup(): void {
    this.#warm = true;
  }

  /** Records one creation. */
  record(kind: PipelineCreation["kind"], async: boolean, label: string | undefined): void {
    this.#creations.push({
      label: label ?? "",
      kind,
      async,
      atMs: this.#nowMs(),
      late: this.#warm,
    });
  }

  /** Every creation so far, first first. */
  get creations(): ReadonlyArray<PipelineCreation> {
    return [...this.#creations];
  }

  /** The creations after the warm-up. */
  late(): ReadonlyArray<PipelineCreation> {
    return this.#creations.filter(({ late }) => late);
  }
}

/** Turns each device a wrapped `GPU` hands out into the device the caller gets. */
export type DeviceWrapper = (device: GPUDevice) => GPUDevice;

/**
 * A `GPU` whose adapters' `requestDevice` passes each device through `wrapDevice`.
 *
 * @remarks
 * The returned object is a plain one forwarding to `gpu`; it is handed to our own code only
 * (`requestAdapterOutcome`, `LoadEngineOptions.gpu`), never to a browser API. The adapters are
 * the browser's own, their `requestDevice` replaced on the instance.
 */
export function wrapGpu(gpu: GPU, wrapDevice: DeviceWrapper): GPU {
  return {
    get wgslLanguageFeatures(): WGSLLanguageFeatures {
      return gpu.wgslLanguageFeatures;
    },
    getPreferredCanvasFormat: () => gpu.getPreferredCanvasFormat(),
    requestAdapter: async (options?: GPURequestAdapterOptions): Promise<GPUAdapter | null> => {
      const adapter = await gpu.requestAdapter(options);
      if (adapter === null) {
        return null;
      }
      const requestDevice = adapter.requestDevice.bind(adapter);
      adapter.requestDevice = async (descriptor?: GPUDeviceDescriptor): Promise<GPUDevice> =>
        wrapDevice(await requestDevice(descriptor));
      return adapter;
    },
  };
}

/**
 * `device`, its four pipeline-creating methods replaced on the instance so that each creation is
 * recorded in `tally` before the call passes through unchanged.
 */
export function shimPipelines(device: GPUDevice, tally: PipelineTally): GPUDevice {
  const render = device.createRenderPipeline.bind(device);
  const renderAsync = device.createRenderPipelineAsync.bind(device);
  const compute = device.createComputePipeline.bind(device);
  const computeAsync = device.createComputePipelineAsync.bind(device);
  device.createRenderPipeline = (descriptor) => {
    tally.record("render", false, descriptor.label);
    return render(descriptor);
  };
  device.createRenderPipelineAsync = (descriptor) => {
    tally.record("render", true, descriptor.label);
    return renderAsync(descriptor);
  };
  device.createComputePipeline = (descriptor) => {
    tally.record("compute", false, descriptor.label);
    return compute(descriptor);
  };
  device.createComputePipelineAsync = (descriptor) => {
    tally.record("compute", true, descriptor.label);
    return computeAsync(descriptor);
  };
  return device;
}
