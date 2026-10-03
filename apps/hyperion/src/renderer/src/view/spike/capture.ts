/**
 * The descent spike's capture of the GPU calls (plan R05, T15.a, Design note 22): a
 * measurement-only shim on the device, its queue, its objects and its encoders, after
 * webgpu_recorder's interception design, that records a span of the descent for the native
 * replayer (`tools/gpu-replay`, T15.b and T15.c).
 *
 * @remarks
 * Every call is logged as one {@link CaptureCall}: the object it was made on (by ID: 0 the
 * device, 1 its queue), the method's name, its arguments made JSON (objects by ID, bytes as blobs)
 * and the ID of what it returned. A replayer resolves the IDs and makes the same call, so the log
 * needs no schema per call.
 *
 * Before the span the shim logs only what makes, names or destroys an object. At the span's start
 * it copies every live buffer and texture to staging buffers in one submission and from then on
 * logs every call; the read-back bytes are placed in the log as writes at the point of that
 * submission (the snapshot), so the capture holds the GPU's state at the span's start without the
 * frames before it, and the engine's own calls during the read-back are not lost. For the copies,
 * buffers and textures are created with `COPY_SRC` added to their usage while the shim is installed
 * (the log keeps the descriptor as asked; the replayer adds `COPY_DST`). Mappable buffers,
 * multisampled textures and formats without a texel size here are listed as skipped: the engine
 * uses them only transiently.
 *
 * The canvases' textures come from `GPUCanvasContext.getCurrentTexture`, not the device, so the
 * shim also wraps that method (`contexts`), logging each canvas as a surface with its size and
 * format, which a replayer maps to a window or an offscreen texture.
 *
 * The device keeps its identity: methods are replaced on each instance, as `pipelineShim.ts` does,
 * since the browser's WebGPU calls refuse a `Proxy`. The device it wraps comes through
 * `wrapGpu`'s seam (T14.a). Its forwarding `createBuffer` and `createTexture`, and the staging
 * buffers of the snapshot, are why `engineBoundary.test.ts` names this file as an exemption.
 */

import { extentOf } from "../engine/memory";

/** The capture's schema name, which the replayer checks. */
export const CAPTURE_SCHEMA = "hyperion.gpu-capture";
/** The capture's schema version. */
export const CAPTURE_VERSION = 1;

/** A JSON value. */
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

/** One logged call. */
export interface CaptureCall {
  /** The ID of the object the method was called on. */
  readonly target: number;
  readonly op: string;
  readonly args: ReadonlyArray<Json>;
  /** The ID given to what the call returned, if it returned an object. */
  readonly result?: number;
}

/** A canvas, as the capture saw its texture last. */
export interface CaptureSurface {
  readonly id: number;
  readonly width: number;
  readonly height: number;
  readonly format: string;
}

/** What the client knew of the run, for the replay's results file. */
export interface CaptureMeta {
  readonly setting?: "high" | "low";
  /** The spike's seed, a u64 in decimal. */
  readonly seed?: string;
  /** Which of Design note 21's GPU rows each pass label counts towards. */
  readonly passRows?: Readonly<Record<string, "terrain" | "atmosphere" | "other">>;
}

/** A capture's JSON part (`capture.json`); the bytes its blobs name are in `capture.bin`. */
export interface CaptureFile {
  readonly schema: typeof CAPTURE_SCHEMA;
  readonly version: typeof CAPTURE_VERSION;
  readonly adapter: Readonly<Record<string, string>>;
  readonly features: ReadonlyArray<string>;
  readonly limits: Readonly<Record<string, number>>;
  readonly surfaces: ReadonlyArray<CaptureSurface>;
  /** The resources the snapshot could not read, and why. */
  readonly skipped: ReadonlyArray<{ readonly id: number; readonly reason: string }>;
  /**
   * What the span could not log faithfully: a call on, or an argument naming, an object whose
   * creation the log lacks (one made before the span and not kept by it, such as an encoder open
   * when the span started). Empty for a capture a replay can trust.
   */
  readonly problems: ReadonlyArray<string>;
  /** The index in `calls` where the span starts, after the snapshot's writes. */
  readonly spanStart: number;
  /** The indices in `calls` where each captured frame starts. */
  readonly frames: ReadonlyArray<number>;
  /** Each blob's offset and length in `capture.bin`, bytes. */
  readonly blobs: ReadonlyArray<{ readonly offset: number; readonly length: number }>;
  readonly calls: ReadonlyArray<CaptureCall>;
  readonly meta: CaptureMeta;
}

/** A capture in memory: its JSON part and its bytes. */
export interface Capture {
  readonly file: CaptureFile;
  readonly data: Uint8Array;
}

/** The device's ID in every capture. */
export const DEVICE_ID = 0;
/** The queue's ID in every capture. */
export const QUEUE_ID = 1;

/** WebGPU's usage bits the shim reads or adds (`GPUBufferUsage`, `GPUTextureUsage`). */
const BUFFER_MAP_READ = 0x1;
const BUFFER_MAP_WRITE = 0x2;
const BUFFER_COPY_SRC = 0x4;
const BUFFER_COPY_DST = 0x8;
const TEXTURE_COPY_SRC = 0x1;

/** What kind of object a call made, which says which of its methods are logged. */
type ObjectKind =
  | "plain"
  | "buffer"
  | "texture"
  | "pipeline"
  | "destroyable"
  | "encoder"
  | "render-pass"
  | "compute-pass";

/** The device's creations other than buffers and textures, and the kind of what each makes. */
const DEVICE_CREATES: Readonly<Record<string, ObjectKind>> = {
  createSampler: "plain",
  createShaderModule: "plain",
  createBindGroupLayout: "plain",
  createPipelineLayout: "plain",
  createBindGroup: "plain",
  createRenderPipeline: "pipeline",
  createComputePipeline: "pipeline",
  createQuerySet: "destroyable",
  createCommandEncoder: "encoder",
};

/** The asynchronous creations, logged on resolution as their synchronous forms. */
const DEVICE_ASYNC_CREATES: Readonly<Record<string, string>> = {
  createRenderPipelineAsync: "createRenderPipeline",
  createComputePipelineAsync: "createComputePipeline",
};

const PASS_COMMON: Readonly<Record<string, ObjectKind | null>> = {
  setPipeline: null,
  setBindGroup: null,
  insertDebugMarker: null,
  pushDebugGroup: null,
  popDebugGroup: null,
  end: null,
};

/** The logged methods of each kind, and the kind of what each returns. */
const METHODS: Readonly<Record<ObjectKind, Readonly<Record<string, ObjectKind | null>>>> = {
  plain: {},
  buffer: { destroy: null },
  texture: { createView: "plain", destroy: null },
  pipeline: { getBindGroupLayout: "plain" },
  destroyable: { destroy: null },
  encoder: {
    beginRenderPass: "render-pass",
    beginComputePass: "compute-pass",
    clearBuffer: null,
    copyBufferToBuffer: null,
    copyBufferToTexture: null,
    copyTextureToBuffer: null,
    copyTextureToTexture: null,
    resolveQuerySet: null,
    insertDebugMarker: null,
    pushDebugGroup: null,
    popDebugGroup: null,
    finish: "plain",
  },
  "render-pass": {
    ...PASS_COMMON,
    setVertexBuffer: null,
    setIndexBuffer: null,
    draw: null,
    drawIndexed: null,
    drawIndirect: null,
    drawIndexedIndirect: null,
    setViewport: null,
    setScissorRect: null,
    setBlendConstant: null,
    setStencilReference: null,
  },
  "compute-pass": {
    ...PASS_COMMON,
    dispatchWorkgroups: null,
    dispatchWorkgroupsIndirect: null,
  },
};

/** The calls logged before the span: those that make, name or destroy an object. */
const SETUP_OPS: ReadonlySet<string> = new Set([
  "createBuffer",
  "createTexture",
  ...Object.keys(DEVICE_CREATES).filter((op) => op !== "createCommandEncoder"),
  "createView",
  "getBindGroupLayout",
  "destroy",
]);

/** Bytes a texel of each format the snapshot reads; other formats are skipped. */
const TEXEL_BYTES: Readonly<Partial<Record<GPUTextureFormat, number>>> = {
  r8unorm: 1,
  r8uint: 1,
  rg8unorm: 2,
  r16float: 2,
  r16uint: 2,
  rgba8unorm: 4,
  "rgba8unorm-srgb": 4,
  bgra8unorm: 4,
  "bgra8unorm-srgb": 4,
  rgba8uint: 4,
  rg16float: 4,
  r32float: 4,
  r32uint: 4,
  r32sint: 4,
  rgb10a2unorm: 4,
  rg11b10ufloat: 4,
  rgba16float: 8,
  rgba16uint: 8,
  rg32float: 8,
  rg32uint: 8,
  rgba32float: 16,
  rgba32uint: 16,
};

/** `copyTextureToBuffer` rows are aligned to this many bytes. */
const ROW_ALIGNMENT = 256;

/** The canvas context method the capture wraps, normally `GPUCanvasContext.prototype`'s. */
export interface CanvasContexts {
  getCurrentTexture(this: object): GPUTexture;
}

/** What the capture needs at its start. */
export interface CaptureOptions {
  /** Where `getCurrentTexture` is wrapped; `null` where there are no canvases (tests). */
  readonly contexts: CanvasContexts | null;
  readonly meta: CaptureMeta;
}

/** A live buffer or texture, for the snapshot. */
type Resource =
  | {
      readonly kind: "buffer";
      readonly object: GPUBuffer;
      readonly descriptor: GPUBufferDescriptor;
    }
  | {
      readonly kind: "texture";
      readonly object: GPUTexture;
      readonly descriptor: GPUTextureDescriptor;
    };

/** One staging buffer of the snapshot and the read-back that places its bytes in the log. */
interface SnapshotRead {
  readonly staging: GPUBuffer;
  readonly run: () => Promise<void>;
}

function isIterable(value: object): value is Iterable<unknown> {
  return Symbol.iterator in value;
}

/** Records one device's calls for the native replay. */
export class GpuCapture {
  readonly #ids = new WeakMap<object, number>();
  readonly #calls: CaptureCall[] = [];
  /** The snapshot's writes, placed in the log at {@link GpuCapture.#snapshotAt}. */
  readonly #snapshotCalls: CaptureCall[] = [];
  readonly #blobs: Uint8Array[] = [];
  readonly #frames: number[] = [];
  readonly #skipped: Array<{ readonly id: number; readonly reason: string }> = [];
  readonly #surfaces = new Map<number, CaptureSurface>();
  readonly #live = new Map<number, Resource>();
  readonly #contexts: CanvasContexts | null;
  readonly #meta: CaptureMeta;
  #device: GPUDevice | undefined;
  #nextId = 2;
  #state: "setup" | "span" | "ended" = "setup";
  /** Set while the capture itself calls the device; those calls are neither logged nor tracked. */
  #quiet = false;
  /** Set during the synchronous part of an asynchronous pipeline creation. */
  #inAsync = false;
  #snapshotAt = 0;
  #restoreContexts: (() => void) | undefined;
  /** The IDs whose creation the log holds: the device, the queue, the surfaces, what was logged. */
  readonly #known = new Set<number>([DEVICE_ID, QUEUE_ID]);
  readonly #problems: string[] = [];
  /** Whether the snapshot's read-backs are all in. */
  #snapshotDone = false;

  constructor(options: CaptureOptions) {
    this.#contexts = options.contexts;
    this.#meta = options.meta;
  }

  /** Installs the shim on a device, as `wrapGpu`'s device wrapper; one device a capture. */
  readonly wrapDevice = (device: GPUDevice): GPUDevice => {
    if (this.#device !== undefined) {
      throw new Error("a capture records one device");
    }
    this.#device = device;
    this.#ids.set(device, DEVICE_ID);
    this.#ids.set(device.queue, QUEUE_ID);
    this.#instrumentDevice(device);
    this.#instrumentQueue(device.queue);
    return device;
  };

  /** Whether the span is being recorded. */
  get recording(): boolean {
    return this.#state === "span";
  }

  /**
   * Starts the span: copies every live resource for the snapshot, logs every call from then on,
   * wraps the canvases' `getCurrentTexture`, and resolves once the snapshot's bytes are read back.
   *
   * @remarks
   * Call it between frames: an encoder or command buffer made before it is not in the log, and a
   * span call on one is listed in `problems` rather than logged.
   * @throws Error if no device is wrapped or the span has already started, or if a read-back fails.
   */
  async startSpan(): Promise<void> {
    const device = this.#device;
    if (device === undefined || this.#state !== "setup") {
      throw new Error("the capture's span starts once, on a wrapped device");
    }
    const reads = this.#snapshot(device);
    this.#snapshotAt = this.#calls.length;
    this.#state = "span";
    this.#wrapContexts();
    const results = await Promise.allSettled(reads.map((read) => read.run()));
    const failure = results.find((result) => result.status === "rejected");
    if (failure !== undefined) {
      for (const read of reads) {
        read.staging.destroy();
      }
      throw new Error("the capture's snapshot could not be read back", { cause: failure.reason });
    }
    this.#snapshotDone = true;
  }

  /** Marks the start of a frame within the span. */
  frame(): void {
    if (this.#state === "span") {
      this.#frames.push(this.#calls.length);
    }
  }

  /** Ends the span and restores the canvas contexts' method; later calls are not logged. */
  endSpan(): void {
    this.#state = "ended";
    this.dispose();
  }

  /**
   * Restores the canvas contexts' method, whatever the capture's state: call it when a run ends
   * early, as on a lost device or a failed span.
   */
  dispose(): void {
    this.#restoreContexts?.();
    this.#restoreContexts = undefined;
  }

  /**
   * The capture so far.
   *
   * @throws Error if the span has started and its snapshot is not all read back.
   */
  result(): Capture {
    if (this.#state !== "setup" && !this.#snapshotDone) {
      throw new Error("the capture's snapshot is not read back yet");
    }
    const device = this.#device;
    const offsets: Array<{ offset: number; length: number }> = [];
    let total = 0;
    for (const blob of this.#blobs) {
      offsets.push({ offset: total, length: blob.byteLength });
      total += blob.byteLength;
    }
    const data = new Uint8Array(total);
    this.#blobs.forEach((blob, i) => {
      data.set(blob, offsets[i]?.offset ?? 0);
    });
    const at = this.#snapshotAt;
    const shift = this.#snapshotCalls.length;
    return {
      file: {
        schema: CAPTURE_SCHEMA,
        version: CAPTURE_VERSION,
        adapter: device === undefined ? {} : adapterOf(device),
        features: device === undefined ? [] : [...device.features].map(String).toSorted(),
        limits: device === undefined ? {} : limitsOf(device.limits),
        surfaces: [...this.#surfaces.values()],
        skipped: [...this.#skipped],
        problems: [...this.#problems],
        spanStart: at + shift,
        frames: this.#frames.map((index) => index + shift),
        blobs: offsets,
        calls: [...this.#calls.slice(0, at), ...this.#snapshotCalls, ...this.#calls.slice(at)],
        meta: this.#meta,
      },
      data,
    };
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

  #logs(op: string): boolean {
    if (this.#quiet || this.#state === "ended") {
      return false;
    }
    return this.#state === "span" || SETUP_OPS.has(op);
  }

  #log(target: number, op: string, args: ReadonlyArray<unknown>, result?: object): void {
    if (!this.#logs(op)) {
      return;
    }
    if (!this.#known.has(target)) {
      // Before the span this is a call on an object of no interest (a canvas texture of an
      // earlier frame); in it, the capture would name an object it never made.
      if (this.#state === "span") {
        this.#problems.push(`${op} on object ${target}, whose creation the log lacks`);
      }
      return;
    }
    const call: CaptureCall = { target, op, args: args.map((arg) => this.#json(arg)) };
    if (result === undefined) {
      this.#calls.push(call);
      return;
    }
    const id = this.#id(result);
    this.#known.add(id);
    this.#calls.push({ ...call, result: id });
  }

  #blob(bytes: Uint8Array): number {
    this.#blobs.push(bytes.slice());
    return this.#blobs.length - 1;
  }

  /** A value as JSON: objects the capture knows by ID, bytes as blobs. */
  #json(value: unknown): Json {
    if (value === undefined) {
      return { $undefined: true };
    }
    if (
      value === null ||
      typeof value === "boolean" ||
      typeof value === "string" ||
      typeof value === "number"
    ) {
      return value;
    }
    if (typeof value === "bigint") {
      return { $bigint: value.toString() };
    }
    if (ArrayBuffer.isView(value)) {
      const bytes = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
      return { $blob: this.#blob(bytes), $type: value.constructor.name };
    }
    if (value instanceof ArrayBuffer) {
      return { $blob: this.#blob(new Uint8Array(value)), $type: "ArrayBuffer" };
    }
    if (typeof value !== "object") {
      return { $unknown: typeof value };
    }
    const id = this.#ids.get(value);
    if (id !== undefined) {
      if (!this.#known.has(id)) {
        this.#problems.push(`an argument names object ${id}, whose creation the log lacks`);
      }
      return { $ref: id };
    }
    if (isIterable(value)) {
      return Array.from(value, (item) => this.#json(item));
    }
    const out: Record<string, Json> = {};
    for (const [key, item] of Object.entries(value)) {
      if (item !== undefined) {
        out[key] = this.#json(item);
      }
    }
    if (Object.keys(out).length === 0 && Object.getPrototypeOf(value) !== Object.prototype) {
      // A WebGPU object the shim never saw made: it would replay as an empty descriptor.
      this.#problems.push(`an argument is a ${value.constructor.name} the log never made`);
      return { $unknown: value.constructor.name };
    }
    return out;
  }

  #instrumentDevice(device: GPUDevice): void {
    const createBuffer = device.createBuffer.bind(device);
    device.createBuffer = (descriptor) => {
      if (this.#quiet) {
        return createBuffer(descriptor);
      }
      const mappable = (descriptor.usage & (BUFFER_MAP_READ | BUFFER_MAP_WRITE)) !== 0;
      const buffer = createBuffer(
        mappable ? descriptor : { ...descriptor, usage: descriptor.usage | BUFFER_COPY_SRC },
      );
      this.#log(DEVICE_ID, "createBuffer", [descriptor], buffer);
      this.#live.set(this.#id(buffer), { kind: "buffer", object: buffer, descriptor });
      this.#instrument(buffer, "buffer");
      return buffer;
    };
    const createTexture = device.createTexture.bind(device);
    device.createTexture = (descriptor) => {
      if (this.#quiet) {
        return createTexture(descriptor);
      }
      const single = (descriptor.sampleCount ?? 1) <= 1;
      const texture = createTexture(
        single ? { ...descriptor, usage: descriptor.usage | TEXTURE_COPY_SRC } : descriptor,
      );
      this.#log(DEVICE_ID, "createTexture", [descriptor], texture);
      this.#live.set(this.#id(texture), { kind: "texture", object: texture, descriptor });
      this.#instrument(texture, "texture");
      return texture;
    };
    for (const [op, kind] of Object.entries(DEVICE_CREATES)) {
      this.#replace(device, op, (original) => (...args: unknown[]) => {
        const made: unknown = original(...args);
        if (!this.#quiet && !this.#inAsync && typeof made === "object" && made !== null) {
          this.#log(DEVICE_ID, op, args, made);
          this.#instrument(made, kind);
        }
        return made;
      });
    }
    for (const [op, logged] of Object.entries(DEVICE_ASYNC_CREATES)) {
      this.#replace(device, op, (original) => async (...args: unknown[]) => {
        // An implementation whose asynchronous form calls the synchronous one through the
        // instance (as the test fake does) would otherwise log one creation twice.
        this.#inAsync = true;
        let pending: unknown;
        try {
          pending = original(...args);
        } finally {
          this.#inAsync = false;
        }
        const made: unknown = await pending;
        if (typeof made === "object" && made !== null) {
          this.#log(DEVICE_ID, logged, args, made);
          this.#instrument(made, "pipeline");
        }
        return made;
      });
    }
  }

  #instrumentQueue(queue: GPUQueue): void {
    const writeBuffer = queue.writeBuffer.bind(queue);
    queue.writeBuffer = (buffer, offset, data, dataOffset, size) => {
      if (this.#logs("writeBuffer")) {
        // The bytes written, so that the log needs no element offsets.
        const view = ArrayBuffer.isView(data) ? data : new Uint8Array(data);
        const element = "BYTES_PER_ELEMENT" in view ? Number(view.BYTES_PER_ELEMENT) : 1;
        const start = view.byteOffset + (dataOffset ?? 0) * element;
        const length =
          size === undefined ? view.byteLength - (start - view.byteOffset) : size * element;
        // Logged with its data offset and size in bytes, so a replayer need not measure it.
        this.#log(QUEUE_ID, "writeBuffer", [
          buffer,
          offset,
          new Uint8Array(view.buffer, start, length),
          0,
          length,
        ]);
      }
      writeBuffer(buffer, offset, data, dataOffset, size);
    };
    for (const op of ["writeTexture", "submit"]) {
      this.#replace(queue, op, (original) => (...args: unknown[]) => {
        this.#log(QUEUE_ID, op, args);
        return original(...args);
      });
    }
  }

  /** Logs the listed methods of an object the device made, and instruments what they make. */
  #instrument(object: object, kind: ObjectKind): void {
    for (const [op, made] of Object.entries(METHODS[kind])) {
      this.#replace(object, op, (original) => (...args: unknown[]) => {
        const result: unknown = original(...args);
        const target = this.#id(object);
        if (op === "destroy") {
          this.#live.delete(target);
        }
        if (made !== null && typeof result === "object" && result !== null) {
          this.#log(target, op, args, result);
          this.#instrument(result, made);
        } else {
          this.#log(target, op, args);
        }
        return result;
      });
    }
  }

  /** Replaces `object[op]` on the instance with `wrap` of its bound original. */
  #replace(
    object: object,
    op: string,
    wrap: (original: (...args: unknown[]) => unknown) => (...args: unknown[]) => unknown,
  ): void {
    const method: unknown = Reflect.get(object, op);
    if (typeof method !== "function") {
      return;
    }
    const original = (...args: unknown[]): unknown => Reflect.apply(method, object, args);
    Reflect.set(object, op, wrap(original));
  }

  #wrapContexts(): void {
    const contexts = this.#contexts;
    if (contexts === null) {
      return;
    }
    // Kept unbound on purpose: the wrapper calls it with each context as `this`.
    // oxlint-disable-next-line typescript/unbound-method
    const original = contexts.getCurrentTexture;
    const seen = (context: object, texture: GPUTexture): void => {
      const surface = this.#id(context);
      this.#known.add(surface);
      this.#surfaces.set(surface, {
        id: surface,
        width: texture.width,
        height: texture.height,
        format: texture.format,
      });
      this.#log(surface, "getCurrentTexture", [], texture);
      this.#instrument(texture, "texture");
    };
    contexts.getCurrentTexture = function getCurrentTexture(this: object): GPUTexture {
      const texture = original.call(this);
      seen(this, texture);
      return texture;
    };
    this.#restoreContexts = () => {
      contexts.getCurrentTexture = original;
    };
  }

  /**
   * Copies every live buffer and texture to staging buffers in one submission.
   *
   * @returns One read-back a staging buffer, each placing its bytes among the snapshot's writes.
   */
  #snapshot(device: GPUDevice): SnapshotRead[] {
    this.#quiet = true;
    try {
      const encoder = device.createCommandEncoder({ label: "capture snapshot" });
      const reads: SnapshotRead[] = [];
      for (const [id, resource] of this.#live) {
        const read =
          resource.kind === "buffer"
            ? this.#snapshotBuffer(device, encoder, id, resource)
            : this.#snapshotTexture(device, encoder, id, resource);
        reads.push(...read);
      }
      device.queue.submit([encoder.finish()]);
      return reads;
    } finally {
      this.#quiet = false;
    }
  }

  #staging(device: GPUDevice, size: number): GPUBuffer {
    return device.createBuffer({
      label: "capture staging",
      size,
      usage: BUFFER_MAP_READ | BUFFER_COPY_DST,
    });
  }

  /** Maps a staging buffer, copies its bytes out and destroys it. */
  async #readStaging(staging: GPUBuffer, bytes: number): Promise<Uint8Array> {
    await staging.mapAsync(BUFFER_MAP_READ);
    const copy = new Uint8Array(staging.getMappedRange()).slice(0, bytes);
    staging.unmap();
    staging.destroy();
    return copy;
  }

  #snapshotBuffer(
    device: GPUDevice,
    encoder: GPUCommandEncoder,
    id: number,
    resource: Extract<Resource, { kind: "buffer" }>,
  ): SnapshotRead[] {
    if ((resource.descriptor.usage & (BUFFER_MAP_READ | BUFFER_MAP_WRITE)) !== 0) {
      this.#skipped.push({ id, reason: "a mappable buffer" });
      return [];
    }
    // A copy's size is a multiple of four bytes; a buffer of another size loses its tail.
    const size = Math.floor(resource.descriptor.size / 4) * 4;
    if (size === 0) {
      return [];
    }
    const staging = this.#staging(device, size);
    encoder.copyBufferToBuffer(resource.object, 0, staging, 0, size);
    const run = async (): Promise<void> => {
      const bytes = await this.#readStaging(staging, size);
      this.#snapshotCalls.push({
        target: QUEUE_ID,
        op: "writeBuffer",
        args: [
          { $ref: id },
          0,
          { $blob: this.#blob(bytes), $type: "Uint8Array" },
          0,
          bytes.byteLength,
        ],
      });
    };
    return [{ staging, run }];
  }

  #snapshotTexture(
    device: GPUDevice,
    encoder: GPUCommandEncoder,
    id: number,
    resource: Extract<Resource, { kind: "texture" }>,
  ): SnapshotRead[] {
    const { descriptor } = resource;
    const texel = TEXEL_BYTES[descriptor.format];
    if ((descriptor.sampleCount ?? 1) > 1) {
      this.#skipped.push({ id, reason: "multisampled" });
      return [];
    }
    if (descriptor.format.startsWith("depth") || descriptor.format.startsWith("stencil")) {
      this.#skipped.push({ id, reason: "a depth or stencil format, never a copy destination" });
      return [];
    }
    if (texel === undefined) {
      this.#skipped.push({ id, reason: `format ${descriptor.format} is not read back` });
      return [];
    }
    const extent = extentOf(descriptor.size);
    const levels = descriptor.mipLevelCount ?? 1;
    const reads: SnapshotRead[] = [];
    for (let level = 0; level < levels; level += 1) {
      const width = Math.max(1, extent.width >> level);
      const height = descriptor.dimension === "1d" ? 1 : Math.max(1, extent.height >> level);
      const depth =
        descriptor.dimension === "3d"
          ? Math.max(1, extent.depthOrArrayLayers >> level)
          : extent.depthOrArrayLayers;
      const bytesPerRow = Math.ceil((width * texel) / ROW_ALIGNMENT) * ROW_ALIGNMENT;
      const size = bytesPerRow * height * depth;
      const staging = this.#staging(device, size);
      const copy = { width, height, depthOrArrayLayers: depth };
      encoder.copyTextureToBuffer(
        { texture: resource.object, mipLevel: level },
        { buffer: staging, bytesPerRow, rowsPerImage: height },
        copy,
      );
      const run = async (): Promise<void> => {
        const bytes = await this.#readStaging(staging, size);
        this.#snapshotCalls.push({
          target: QUEUE_ID,
          op: "writeTexture",
          args: [
            { texture: { $ref: id }, mipLevel: level },
            { $blob: this.#blob(bytes), $type: "Uint8Array" },
            { offset: 0, bytesPerRow, rowsPerImage: height },
            copy,
          ],
        });
      };
      reads.push({ staging, run });
    }
    return reads;
  }
}

function adapterOf(device: GPUDevice): Record<string, string> {
  const info = device.adapterInfo;
  return {
    vendor: info.vendor,
    architecture: info.architecture,
    device: info.device,
    description: info.description,
  };
}

function limitsOf(limits: GPUSupportedLimits): Record<string, number> {
  const out: Record<string, number> = {};
  for (const key in limits) {
    const value: unknown = Reflect.get(limits, key);
    if (typeof value === "number") {
      out[key] = value;
    }
  }
  return out;
}

/** How a blob is read back, by the `$type` its writer gave. */
const TYPED_ARRAYS: ReadonlyMap<string, (bytes: ArrayBuffer) => ArrayBufferView> = new Map<
  string,
  (bytes: ArrayBuffer) => ArrayBufferView
>([
  ["Uint8Array", (bytes) => new Uint8Array(bytes)],
  ["Int8Array", (bytes) => new Int8Array(bytes)],
  ["Uint16Array", (bytes) => new Uint16Array(bytes)],
  ["Int16Array", (bytes) => new Int16Array(bytes)],
  ["Uint32Array", (bytes) => new Uint32Array(bytes)],
  ["Int32Array", (bytes) => new Int32Array(bytes)],
  ["Float32Array", (bytes) => new Float32Array(bytes)],
  ["Float64Array", (bytes) => new Float64Array(bytes)],
]);

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * The capture file's shape, once the reader has checked the parts it relies on.
 *
 * @remarks
 * A documented cast at a trust boundary: the file is this capture's own output, checked for its
 * schema, version, blobs and call list by {@link parseCapture}.
 */
function trustedCaptureFile(value: unknown): CaptureFile {
  // The capture's own output, checked as far as the reader relies on it (see above).
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return value as CaptureFile;
}

/**
 * A capture read back from its JSON part and its bytes.
 *
 * @throws Error if the JSON is not a capture of this schema and version, or names a blob outside
 * the bytes.
 */
export function parseCapture(json: unknown, data: Uint8Array): Capture {
  if (!isRecord(json) || json["schema"] !== CAPTURE_SCHEMA || json["version"] !== CAPTURE_VERSION) {
    throw new Error(`not a ${CAPTURE_SCHEMA} version ${CAPTURE_VERSION} capture`);
  }
  const blobs = json["blobs"];
  const calls = json["calls"];
  if (!Array.isArray(blobs) || !Array.isArray(calls)) {
    throw new Error("the capture has no blobs or calls");
  }
  for (const blob of blobs) {
    const offset = isRecord(blob) ? blob["offset"] : undefined;
    const length = isRecord(blob) ? blob["length"] : undefined;
    if (
      typeof offset !== "number" ||
      typeof length !== "number" ||
      offset + length > data.byteLength
    ) {
      throw new Error("a blob lies outside the capture's bytes");
    }
  }
  return { file: trustedCaptureFile(json), data };
}

/** A capture's value back as the call took it: IDs as objects, blobs as bytes. */
function fromJson(value: Json, capture: Capture, objects: ReadonlyMap<number, unknown>): unknown {
  if (value === null || typeof value !== "object") {
    return value;
  }
  if (Array.isArray(value)) {
    return value.map((item) => fromJson(item, capture, objects));
  }
  const ref = value["$ref"];
  if (typeof ref === "number") {
    if (!objects.has(ref)) {
      throw new Error(`the capture names object ${ref} before making it`);
    }
    return objects.get(ref);
  }
  if (value["$undefined"] === true) {
    return undefined;
  }
  const big = value["$bigint"];
  if (typeof big === "string") {
    return BigInt(big);
  }
  const blob = value["$blob"];
  if (typeof blob === "number") {
    const span = capture.file.blobs[blob];
    if (span === undefined) {
      throw new Error(`the capture names blob ${blob}, which it lacks`);
    }
    const bytes = capture.data.slice(span.offset, span.offset + span.length);
    const type = value["$type"];
    if (type === "ArrayBuffer") {
      return bytes.buffer;
    }
    const make = typeof type === "string" ? TYPED_ARRAYS.get(type) : undefined;
    return make === undefined ? bytes : make(bytes.buffer);
  }
  const out: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) {
    out[key] = fromJson(item, capture, objects);
  }
  return out;
}

/** A canvas a capture's surface is replayed on. */
export interface ReplaySurface {
  getCurrentTexture(): GPUTexture;
}

/**
 * Replays a capture's calls on `device`, in order: the reference replayer the round-trip test
 * uses (the native one is `tools/gpu-replay`).
 *
 * @param surfaces - The canvas standing for each of the capture's surfaces.
 * @throws Error if a call names an object it has not made, or a method its target lacks.
 */
export function replayCapture(
  capture: Capture,
  device: GPUDevice,
  surfaces: (surface: CaptureSurface) => ReplaySurface,
): void {
  const objects = new Map<number, unknown>([
    [DEVICE_ID, device],
    [QUEUE_ID, device.queue],
  ]);
  for (const surface of capture.file.surfaces) {
    objects.set(surface.id, surfaces(surface));
  }
  for (const call of capture.file.calls) {
    const target = objects.get(call.target);
    const method: unknown =
      typeof target === "object" && target !== null ? Reflect.get(target, call.op) : undefined;
    if (typeof method !== "function" || typeof target !== "object" || target === null) {
      throw new Error(`object ${call.target} has no method ${call.op}`);
    }
    const result: unknown = Reflect.apply(
      method,
      target,
      call.args.map((arg) => fromJson(arg, capture, objects)),
    );
    if (call.result !== undefined) {
      objects.set(call.result, result);
    }
  }
}
