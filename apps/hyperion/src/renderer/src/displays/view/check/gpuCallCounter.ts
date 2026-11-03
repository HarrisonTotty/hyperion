/**
 * The inspection profile's count of WebGPU calls (`--count-gpu-calls`, check-only; plan R14, the
 * owner's Mac): every call the engine makes on its device, its queue, its encoders and their
 * passes, and on the textures, buffers and pipelines the device made, tallied by call and by the
 * pass it was made for, over each profiled window.
 *
 * @remarks
 * On the owner's M5 the GPU process's main thread was busy 93–99% of the profile's windows, and a
 * trace cannot tell Dawn's encoding from Metal's waits; the encoding's cost scales with the calls a
 * frame makes, on every backend, so they are counted. The device keeps its identity: each method is
 * replaced on the instance, calling the original, as the descent spike's shim does
 * (`pipelineShim.ts`), since the browser's own calls brand-check their arguments and refuse a
 * proxy.
 *
 * A call inside a pass is counted under the pass's label (an unlabelled pass's encoder's); an
 * encoder's own under the encoder's; a submission under its first command buffer's encoder's; and
 * any other call (a write, a bind group or a view made) under the pass open, or else under the next
 * pass begun, the submission it precedes in this engine's order (upload, then encode), or, at the
 * window's end, under {@link AFTER_LAST_PASS}. A canvas's texture is the context's, not the
 * device's, so its view, one a view a frame, is not counted. The counter costs the main thread a
 * little, so a counted window's times are not a player's.
 */

import type { InspectionGpuCall } from "../../../../../preload/api";
import { loadRenderEngine } from "../../../view/engine/loadEngine";
import { requestAdapterOutcome } from "../../../view/engine/platform";
import { navigatorGpu } from "../../../view/engine/status";
import { wrapGpu } from "../../../view/spike/pipelineShim";
import type { ViewEngineSource } from "../useViewEngine";

/** The label of the calls held for a pass that had not begun as the window ended. */
export const AFTER_LAST_PASS = "(after the last pass)";

/** The label of an encoder made with none, and of an unlabelled pass of one. */
export const UNLABELLED = "(unlabelled)";

/**
 * What each maker's result is counted as, by the call that made it. Any other call's result is not
 * instrumented: a bind group, a layout or a sampler has no method to count.
 */
const MADE: Readonly<Record<string, string>> = {
  createBuffer: "buffer",
  createTexture: "texture",
  createRenderPipeline: "pipeline",
  createComputePipeline: "pipeline",
  createRenderPipelineAsync: "pipeline",
  createComputePipelineAsync: "pipeline",
  createShaderModule: "shader module",
  createQuerySet: "query set",
  createRenderBundleEncoder: "bundle encoder",
};

/** One call's count and the bytes it wrote. */
interface Tally {
  count: number;
  bytes: number;
}

/** The names of an object's methods, on its prototypes below `Object`'s and `EventTarget`'s. */
function methodsOf(object: object): ReadonlyArray<string> {
  const names = new Set<string>();
  let prototype: unknown = Object.getPrototypeOf(object);
  while (
    typeof prototype === "object" &&
    prototype !== null &&
    prototype !== Object.prototype &&
    prototype !== EventTarget.prototype
  ) {
    for (const name of Object.getOwnPropertyNames(prototype)) {
      const descriptor = Object.getOwnPropertyDescriptor(prototype, name);
      if (name !== "constructor" && typeof descriptor?.value === "function") {
        names.add(name);
      }
    }
    prototype = Object.getPrototypeOf(prototype);
  }
  return [...names];
}

/** A descriptor's label, or `null` where it has none. */
function labelOf(descriptor: unknown): string | null {
  const label: unknown =
    typeof descriptor === "object" && descriptor !== null
      ? Reflect.get(descriptor, "label")
      : undefined;
  return typeof label === "string" && label.length > 0 ? label : null;
}

/**
 * The bytes a `writeBuffer` writes: `size` elements of `data` where given, else the rest of `data`
 * from `dataOffset` elements, an element being a typed array's and a byte otherwise.
 */
export function bufferWriteBytes(data: unknown, dataOffset: unknown, size: unknown): number {
  const element =
    ArrayBuffer.isView(data) && "BYTES_PER_ELEMENT" in data ? Number(data.BYTES_PER_ELEMENT) : 1;
  if (typeof size === "number") {
    return size * element;
  }
  const length = ArrayBuffer.isView(data) || data instanceof ArrayBuffer ? data.byteLength : 0;
  return length - (typeof dataOffset === "number" ? dataOffset : 0) * element;
}

/** The bytes a `writeTexture` hands over: its data's, whatever its layout reads of them. */
function textureWriteBytes(data: unknown): number {
  return ArrayBuffer.isView(data) || data instanceof ArrayBuffer ? data.byteLength : 0;
}

/** Counts every WebGPU call made on the devices it wraps, between its `begin` and its `take`. */
export class GpuCallCounter {
  /** Each pass label's calls, by `<object>.<method>`. */
  #tallies = new Map<string, Map<string, Tally>>();
  /** The calls made outside a pass since the last pass began, for the next one. */
  #held = new Map<string, Tally>();
  /** The label of the pass open now, if any. */
  #pass: string | null = null;
  /** Each command buffer's encoder's label, for its submission. */
  readonly #buffers = new WeakMap<object, string>();
  /** The objects instrumented, each once. */
  readonly #instrumented = new WeakSet<object>();
  /** Set inside a counted call: a call the implementation makes through the instance is its own. */
  #inCall = false;

  /**
   * Counts every call on `device` from now on, its queue's, its encoders' and their passes', and
   * those of the textures, buffers and pipelines it makes, and returns it: `wrapGpu`'s device
   * wrapper.
   */
  readonly wrapDevice = (device: GPUDevice): GPUDevice => {
    this.#instrument(device, "device", null);
    this.#instrument(device.queue, "queue", null);
    return device;
  };

  /** Starts a window: everything counted before is forgotten. */
  begin(): void {
    this.#tallies = new Map();
    this.#held = new Map();
  }

  /**
   * The calls since {@link GpuCallCounter.begin}, each pass label's by name, in the order first
   * counted; those held for a pass not yet begun are under {@link AFTER_LAST_PASS}. Counting goes
   * on from none.
   */
  take(): ReadonlyArray<InspectionGpuCall> {
    const calls: InspectionGpuCall[] = [];
    const add = (label: string, tallies: ReadonlyMap<string, Tally>): void => {
      for (const [call, { count, bytes }] of tallies) {
        calls.push({ label, call, count, bytes });
      }
    };
    for (const [label, tallies] of this.#tallies) {
      add(label, tallies);
    }
    add(AFTER_LAST_PASS, this.#held);
    this.begin();
    return calls;
  }

  /** The tallies of `label`, made at its first call. */
  #talliesOf(label: string): Map<string, Tally> {
    let tallies = this.#tallies.get(label);
    if (tallies === undefined) {
      tallies = new Map();
      this.#tallies.set(label, tallies);
    }
    return tallies;
  }

  /** Counts `count` calls of `call` under `label`, or the open pass, or for the next pass. */
  #count(label: string | null, call: string, count: number, bytes: number): void {
    const under = label ?? this.#pass;
    const into = under === null ? this.#held : this.#talliesOf(under);
    const tally = into.get(call);
    if (tally === undefined) {
      into.set(call, { count, bytes });
    } else {
      tally.count += count;
      tally.bytes += bytes;
    }
  }

  /** A pass labelled `label` begins: the calls held for it are its. */
  #begin(label: string): void {
    this.#pass = label;
    for (const [call, { count, bytes }] of this.#held) {
      this.#count(label, call, count, bytes);
    }
    this.#held = new Map();
  }

  /**
   * Replaces each method of `object` on the instance with one that counts the call as
   * `<kind>.<method>`, under `label` (an encoder's or a pass's) or, for `null`, under the open pass
   * or the next, and instruments what it makes.
   */
  #instrument(object: object, kind: string, label: string | null): void {
    if (this.#instrumented.has(object)) {
      return;
    }
    this.#instrumented.add(object);
    for (const method of methodsOf(object)) {
      const original: unknown = Reflect.get(object, method);
      if (typeof original !== "function") {
        continue;
      }
      const call = `${kind}.${method}`;
      Reflect.set(object, method, (...args: unknown[]): unknown => {
        if (this.#inCall) {
          return Reflect.apply(original, object, args);
        }
        this.#inCall = true;
        let result: unknown;
        try {
          this.#counted(kind, method, call, args, label);
          result = Reflect.apply(original, object, args);
        } finally {
          this.#inCall = false;
        }
        this.#made(kind, method, args, result, label);
        return result;
      });
    }
  }

  /** Counts one call of `object.method`, made under `label` (see {@link GpuCallCounter.#instrument}). */
  #counted(
    kind: string,
    method: string,
    call: string,
    args: ReadonlyArray<unknown>,
    label: string | null,
  ): void {
    if (kind === "device" && method === "createCommandEncoder") {
      this.#count(labelOf(args[0]) ?? UNLABELLED, call, 1, 0);
      return;
    }
    if (kind === "encoder" && (method === "beginRenderPass" || method === "beginComputePass")) {
      const pass = labelOf(args[0]) ?? label ?? UNLABELLED;
      this.#begin(pass);
      this.#count(pass, call, 1, 0);
      return;
    }
    if ((kind === "render pass" || kind === "compute pass") && method === "end") {
      this.#count(label, call, 1, 0);
      this.#pass = null;
      return;
    }
    if (kind === "queue" && method === "submit") {
      const [buffers] = args;
      const first: unknown = Array.isArray(buffers) ? Reflect.get(buffers, 0) : undefined;
      const encoder =
        typeof first === "object" && first !== null ? this.#buffers.get(first) : undefined;
      this.#count(encoder ?? null, call, 1, 0);
      return;
    }
    const bytes =
      kind !== "queue"
        ? 0
        : method === "writeBuffer"
          ? bufferWriteBytes(args[2], args[3], args[4])
          : method === "writeTexture"
            ? textureWriteBytes(args[1])
            : 0;
    this.#count(label, call, 1, bytes);
  }

  /** Instruments what a call made: an encoder, a pass, a command buffer or a maker's object. */
  #made(
    kind: string,
    method: string,
    args: ReadonlyArray<unknown>,
    result: unknown,
    label: string | null,
  ): void {
    if (typeof result !== "object" || result === null) {
      return;
    }
    if (kind === "device" && method === "createCommandEncoder") {
      this.#instrument(result, "encoder", labelOf(args[0]) ?? UNLABELLED);
      return;
    }
    if (kind === "encoder" && (method === "beginRenderPass" || method === "beginComputePass")) {
      this.#instrument(
        result,
        method === "beginRenderPass" ? "render pass" : "compute pass",
        labelOf(args[0]) ?? label ?? UNLABELLED,
      );
      return;
    }
    if (kind === "encoder" && method === "finish") {
      this.#buffers.set(result, label ?? UNLABELLED);
      return;
    }
    const made = kind === "device" ? MADE[method] : undefined;
    if (made === undefined) {
      return;
    }
    if (result instanceof Promise) {
      void result.then(
        (resolved: unknown) => {
          if (typeof resolved === "object" && resolved !== null) {
            this.#instrument(resolved, made, null);
          }
          return undefined;
        },
        // The caller's own promise carries the rejection.
        () => undefined,
      );
      return;
    }
    this.#instrument(result, made, null);
  }
}

/**
 * R01's engine source with its devices' calls counted by `counter`: `navigator.gpu`'s adapter
 * through {@link wrapGpu}, whose `GPU` the engine also rebuilds from after a device loss.
 */
export function countedEngineSource(
  counter: GpuCallCounter,
  gpu: GPU | undefined = navigatorGpu(),
): ViewEngineSource {
  const wrapped = gpu === undefined ? undefined : wrapGpu(gpu, counter.wrapDevice);
  return {
    requestAdapter: () => requestAdapterOutcome(wrapped),
    load: (outcome, status) =>
      loadRenderEngine(outcome, status, wrapped === undefined ? {} : { gpu: wrapped }),
  };
}
