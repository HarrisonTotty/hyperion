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
 *
 * Each render pass's attachments are counted too (R14.T2.e, the owner's Mac): by pass label, each
 * attachment's aspect, view and operations, and the bytes a tile-based GPU moves between memory
 * and its tiles where no tile is kept from one pass to the next ({@link attachmentBytes}). A view's
 * format and size are read from the texture it was made of: the device's, and the canvases' as
 * their contexts hand them out, without counting the call: from the first device wrapped, which is
 * the engine's load and not a render, to {@link GpuCallCounter.release}.
 */

import type {
  InspectionAttachmentLoad,
  InspectionAttachmentSlot,
  InspectionAttachmentStore,
  InspectionGpuAttachment,
  InspectionGpuCall,
} from "../../../../../preload/api";
import { loadRenderEngine } from "../../../view/engine/loadEngine";
import { texelBytesOf } from "../../../view/engine/memory";
import { requestAdapterOutcome } from "../../../view/engine/platform";
import { navigatorGpu } from "../../../view/engine/status";
import type { CanvasContexts } from "../../../view/spike/capture";
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

/** What a counted window took: its calls, and its render passes' attachments. */
export interface GpuCounts {
  readonly calls: ReadonlyArray<InspectionGpuCall>;
  readonly attachments: ReadonlyArray<InspectionGpuAttachment>;
}

/** A texture view as an attachment: its format, its mip level's size, px, and its samples. */
export interface AttachedView {
  readonly format: GPUTextureFormat;
  readonly widthPx: number;
  readonly heightPx: number;
  readonly sampleCount: number;
}

/** A number read from `object`'s member `name`, or `fallback` where it is none. */
function numberOf(object: unknown, name: string, fallback: number): number {
  const value: unknown =
    typeof object === "object" && object !== null ? Reflect.get(object, name) : undefined;
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/**
 * The view `descriptor` makes of `texture` as an attachment: the descriptor's format or the
 * texture's, the base mip level's size and the texture's samples; `null` where the texture states
 * no format.
 */
export function attachedView(texture: object, descriptor: unknown): AttachedView | null {
  const asked: unknown =
    typeof descriptor === "object" && descriptor !== null
      ? Reflect.get(descriptor, "format")
      : undefined;
  const own: unknown = Reflect.get(texture, "format");
  const format = typeof asked === "string" ? asked : own;
  if (typeof format !== "string") {
    return null;
  }
  const level = numberOf(descriptor, "baseMipLevel", 0);
  return {
    // The texture's own format or the one its view was asked in, both the browser's.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    format: format as GPUTextureFormat,
    widthPx: Math.max(1, Math.floor(numberOf(texture, "width", 1) / 2 ** level)),
    heightPx: Math.max(1, Math.floor(numberOf(texture, "height", 1) / 2 ** level)),
    sampleCount: numberOf(texture, "sampleCount", 1),
  };
}

/** Whether a format has a stencil aspect. */
function hasStencil(format: GPUTextureFormat): boolean {
  return format === "stencil8" || format.endsWith("-stencil8");
}

/** Whether a format has a depth aspect. */
function hasDepth(format: GPUTextureFormat): boolean {
  return format.startsWith("depth");
}

/**
 * The bytes one aspect of `view` holds, its texels' own size (`engine/memory.ts`' table, from the
 * WebGPU specification's §26.1): a stencil aspect 1 byte a texel and a depth aspect the rest of a
 * depth-stencil format's; `null` for a format the table does not size (a compressed one, which no
 * pass attaches).
 */
export function attachmentBytes(
  view: AttachedView,
  aspect: "colour" | "depth" | "stencil",
): number | null {
  let texel = texelBytesOf(view.format);
  if (texel === null) {
    return null;
  }
  if (aspect === "stencil") {
    texel = 1;
  } else if (aspect === "depth" && hasStencil(view.format)) {
    texel -= 1;
  }
  return view.widthPx * view.heightPx * view.sampleCount * texel;
}

/** The sum of two byte counts, or `null` where either is not known. */
function sumOrNull(a: number | null, b: number | null): number | null {
  return a === null || b === null ? null : a + b;
}

/** One attachment as a render pass took it, before its passes are counted. */
type AttachmentTake = Omit<InspectionGpuAttachment, "label" | "passes">;

/**
 * One attachment's take of `view` (`null`: not seen made) under `loadOp` and `storeOp`: its bytes
 * loaded where it loads or is read-only, and stored where it stores or resolves.
 */
function takeOf(
  attachment: InspectionAttachmentSlot,
  view: AttachedView | null,
  aspect: "colour" | "depth" | "stencil",
  loadOp: InspectionAttachmentLoad | null,
  storeOp: InspectionAttachmentStore | null,
): AttachmentTake {
  const bytes = view === null ? null : attachmentBytes(view, aspect);
  const moved = (yes: boolean): number | null => (yes ? bytes : 0);
  return {
    attachment,
    view,
    loadOp,
    storeOp,
    bytesLoaded: moved(loadOp === "load" || loadOp === "read-only"),
    bytesStored: moved(storeOp === "store" || storeOp === "resolve"),
  };
}

/** The page's canvas contexts' prototype, or `null` where the page has none (a test's). */
function pageCanvasContexts(): CanvasContexts | null {
  return typeof GPUCanvasContext === "undefined" ? null : GPUCanvasContext.prototype;
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

/** `GPUBufferUsage.MAP_WRITE`. */
const MAP_WRITE = 0x2;

/**
 * The bytes a buffer made mapped for writing at its creation uploads, its size: the engine's staged
 * writes' staging buffer (lane perf-page-long-tasks), whose bytes no queue write carries; 0 for
 * any other buffer.
 */
export function stagedUploadBytes(descriptor: unknown): number {
  if (typeof descriptor !== "object" || descriptor === null) {
    return 0;
  }
  const mapped: unknown = Reflect.get(descriptor, "mappedAtCreation");
  const usage: unknown = Reflect.get(descriptor, "usage");
  const size: unknown = Reflect.get(descriptor, "size");
  return mapped === true &&
    typeof usage === "number" &&
    (usage & MAP_WRITE) !== 0 &&
    typeof size === "number"
    ? size
    : 0;
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
  /** Each attachment kind's passes and bytes by its label and take, in the order first counted. */
  #attachments = new Map<string, InspectionGpuAttachment>();
  /** The views of the textures seen made, as attachments. */
  readonly #views = new WeakMap<object, AttachedView>();
  /** The canvases' textures whose views are read. */
  readonly #canvasTextures = new WeakSet<object>();
  readonly #contexts: CanvasContexts | null;
  /** What puts the canvases' contexts back, once they are wrapped. */
  #restoreCanvases: (() => void) | null = null;

  /**
   * Counts every call on `device` from now on, its queue's, its encoders' and their passes', and
   * those of the textures, buffers and pipelines it makes, and returns it: `wrapGpu`'s device
   * wrapper.
   */
  readonly wrapDevice = (device: GPUDevice): GPUDevice => {
    this.#restoreCanvases ??= this.#watchCanvases(this.#contexts);
    this.#instrument(device, "device", null);
    this.#instrument(device.queue, "queue", null);
    return device;
  };

  /**
   * @param contexts - Where the canvases' textures are handed out, `GPUCanvasContext.prototype`
   *   unless given: wrapped as the first device is, so that a pass that draws into a canvas counts
   *   its bytes; `null` reads no canvas.
   */
  constructor(contexts: CanvasContexts | null = pageCanvasContexts()) {
    this.#contexts = contexts;
  }

  /** Puts the canvases' contexts back as they were, if the counter wrapped them. */
  release(): void {
    this.#restoreCanvases?.();
    this.#restoreCanvases = null;
  }

  /**
   * Reads the views of the canvases' textures from now on, as `contexts` hands the textures out;
   * their views are not counted as calls.
   *
   * @returns What puts `contexts` back as it was.
   */
  #watchCanvases(contexts: CanvasContexts | null): () => void {
    if (contexts === null) {
      return () => undefined;
    }
    // Kept unbound on purpose: the wrapper calls it with each context as `this`.
    // oxlint-disable-next-line typescript/unbound-method
    const original = contexts.getCurrentTexture;
    const seen = (texture: GPUTexture): void => {
      this.#watchCanvasTexture(texture);
    };
    contexts.getCurrentTexture = function getCurrentTexture(this: object): GPUTexture {
      const texture = original.call(this);
      seen(texture);
      return texture;
    };
    return () => {
      contexts.getCurrentTexture = original;
    };
  }

  /** Starts a window: everything counted before is forgotten. */
  begin(): void {
    this.#tallies = new Map();
    this.#held = new Map();
    this.#attachments = new Map();
  }

  /**
   * The calls since {@link GpuCallCounter.begin}, each pass label's by name, in the order first
   * counted, those held for a pass not yet begun under {@link AFTER_LAST_PASS}; and the render
   * passes' attachments, each label's kinds in the order first counted. Counting goes on from none.
   */
  take(): GpuCounts {
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
    const attachments = [...this.#attachments.values()];
    this.begin();
    return { calls, attachments };
  }

  /** Reads the views made of a canvas's texture as attachments, once a texture. */
  #watchCanvasTexture(texture: object): void {
    if (this.#canvasTextures.has(texture)) {
      return;
    }
    this.#canvasTextures.add(texture);
    const createView: unknown = Reflect.get(texture, "createView");
    if (typeof createView !== "function") {
      return;
    }
    Reflect.set(texture, "createView", (...args: unknown[]): unknown => {
      const view: unknown = Reflect.apply(createView, texture, args);
      this.#seeView(texture, args[0], view);
      return view;
    });
  }

  /** Keeps what `view`, made of `texture` by `descriptor`, is as an attachment. */
  #seeView(texture: object, descriptor: unknown, view: unknown): void {
    const attached = attachedView(texture, descriptor);
    if (typeof view === "object" && view !== null && attached !== null) {
      this.#views.set(view, attached);
    }
  }

  /**
   * An attachment's view as the counter saw it made, or, for a texture attached whole, the texture's
   * own; `null` for a view not seen made.
   */
  #viewOf(view: GPUTexture | GPUTextureView): AttachedView | null {
    return this.#views.get(view) ?? ("createView" in view ? attachedView(view, undefined) : null);
  }

  /** Counts the attachments of a render pass labelled `label`, begun with `descriptor`. */
  #countAttachments(label: string, descriptor: GPURenderPassDescriptor): void {
    const takes: AttachmentTake[] = [];
    Array.from(descriptor.colorAttachments).forEach((colour, i) => {
      if (colour === null) {
        return;
      }
      takes.push(
        takeOf(`colour ${i}`, this.#viewOf(colour.view), "colour", colour.loadOp, colour.storeOp),
      );
      if (colour.resolveTarget !== undefined) {
        takes.push(
          takeOf(`resolve ${i}`, this.#viewOf(colour.resolveTarget), "colour", null, "resolve"),
        );
      }
    });
    const depthStencil = descriptor.depthStencilAttachment;
    if (depthStencil !== undefined) {
      const view = this.#viewOf(depthStencil.view);
      // A view not seen made is counted as a depth attachment, its bytes unknown.
      if (view === null || hasDepth(view.format)) {
        const readOnly = depthStencil.depthReadOnly === true;
        takes.push(
          takeOf(
            "depth",
            view,
            "depth",
            readOnly ? "read-only" : (depthStencil.depthLoadOp ?? null),
            readOnly ? "read-only" : (depthStencil.depthStoreOp ?? null),
          ),
        );
      }
      if (view !== null && hasStencil(view.format)) {
        const readOnly = depthStencil.stencilReadOnly === true;
        takes.push(
          takeOf(
            "stencil",
            view,
            "stencil",
            readOnly ? "read-only" : (depthStencil.stencilLoadOp ?? null),
            readOnly ? "read-only" : (depthStencil.stencilStoreOp ?? null),
          ),
        );
      }
    }
    for (const take of takes) {
      this.#tallyAttachment(label, take);
    }
  }

  /** Counts one pass's take of an attachment under `label`, beside the same takes before it. */
  #tallyAttachment(label: string, take: AttachmentTake): void {
    const key = [
      label,
      take.attachment,
      take.view?.format,
      take.view?.widthPx,
      take.view?.heightPx,
      take.view?.sampleCount,
      take.loadOp,
      take.storeOp,
    ]
      .map(String)
      .join("\u0000");
    const known = this.#attachments.get(key);
    this.#attachments.set(
      key,
      known === undefined
        ? { label, ...take, passes: 1 }
        : {
            ...known,
            passes: known.passes + 1,
            bytesLoaded: sumOrNull(known.bytesLoaded, take.bytesLoaded),
            bytesStored: sumOrNull(known.bytesStored, take.bytesStored),
          },
    );
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
        this.#made(object, kind, method, args, result, label);
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
      kind === "device"
        ? method === "createBuffer"
          ? stagedUploadBytes(args[0])
          : 0
        : kind !== "queue"
          ? 0
          : method === "writeBuffer"
            ? bufferWriteBytes(args[2], args[3], args[4])
            : method === "writeTexture"
              ? textureWriteBytes(args[1])
              : 0;
    this.#count(label, call, 1, bytes);
  }

  /**
   * Instruments what a call on `object` made: an encoder, a pass, a command buffer or a maker's
   * object; and keeps a texture's view as an attachment.
   */
  #made(
    object: object,
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
    if (kind === "encoder" && method === "beginRenderPass") {
      // The descriptor the browser took as the pass began, which it checked against its type.
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion
      const descriptor = args[0] as GPURenderPassDescriptor;
      this.#countAttachments(labelOf(descriptor) ?? label ?? UNLABELLED, descriptor);
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
    if (kind === "texture" && method === "createView") {
      this.#seeView(object, args[0], result);
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
