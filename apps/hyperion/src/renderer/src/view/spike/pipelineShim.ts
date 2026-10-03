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
  /** When, in the descent's script time, s. */
  readonly scriptTimeS: number;
  /** Whether it came after the warm-up ended: a pipeline compiled mid-run, a likely hitch. */
  readonly late: boolean;
}

/** Counts pipeline creations, and marks those after the warm-up. */
export class PipelineTally {
  readonly #scriptTimeS: () => number;
  readonly #creations: PipelineCreation[] = [];
  #warm = false;

  /** @param scriptTimeS - The descent's script time now, s. */
  constructor(scriptTimeS: () => number) {
    this.#scriptTimeS = scriptTimeS;
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
      scriptTimeS: this.#scriptTimeS(),
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
  // An implementation whose asynchronous form calls the synchronous one through the instance (as
  // the test fake does) would otherwise count one creation twice.
  let inAsync = false;
  const once = <T>(kind: PipelineCreation["kind"], label: string | undefined, make: () => T): T => {
    if (!inAsync) {
      tally.record(kind, false, label);
    }
    return make();
  };
  const outer = <T>(
    kind: PipelineCreation["kind"],
    label: string | undefined,
    make: () => T,
  ): T => {
    tally.record(kind, true, label);
    inAsync = true;
    try {
      return make();
    } finally {
      inAsync = false;
    }
  };
  device.createRenderPipeline = (descriptor) =>
    once("render", descriptor.label, () => render(descriptor));
  device.createRenderPipelineAsync = (descriptor) =>
    outer("render", descriptor.label, () => renderAsync(descriptor));
  device.createComputePipeline = (descriptor) =>
    once("compute", descriptor.label, () => compute(descriptor));
  device.createComputePipelineAsync = (descriptor) =>
    outer("compute", descriptor.label, () => computeAsync(descriptor));
  return device;
}

/** A shim that installs itself on a device, as the capture (T15.a) does. */
export interface DeviceShim {
  readonly wrapDevice: DeviceWrapper;
}

/**
 * The spike's device wrapper for {@link wrapGpu}: the pipeline tally always, and the capture
 * (`--capture`, T13.c) only when one is given, so that an ordinary spike run carries no capture.
 */
export function spikeDeviceWrapper(
  tally: PipelineTally,
  capture: DeviceShim | null,
): DeviceWrapper {
  return (device) => {
    const shimmed = shimPipelines(device, tally);
    return capture === null ? shimmed : capture.wrapDevice(shimmed);
  };
}
