/**
 * One view: a canvas with its own context and depth, drawn by the engine's one device (R01 Design
 * notes 13 and 23).
 *
 * @remarks
 * Each view configures its own `GPUCanvasContext` against the engine's device at the view's own
 * size, with `COPY_SRC` so that the harness can read it back, and draws each frame into the
 * context's current texture and a `depth32float` of its own, in the adapter's own passes. The
 * canvas is configured with its preferred 8-bit format and that format's `-srgb` twin among its
 * view formats, and the pass renders through the sRGB view, so that blending happens in linear
 * light and the store encodes (Design note 18). WebGPU's framebuffer has y down and its clip space
 * y up, so R02's projection lands the right way up with no flip. A resize remakes this view's
 * depth and nothing else.
 */

import { BUFFER_USAGE, MAP_MODE, TEXTURE_USAGE } from "../gpuFlags";
import type { TextureSpec } from "../memory";
import type { FrameSubmission, RenderView, TextureHandle, ViewSize } from "../types";
import type { FrameOutput } from "./drawing";
import { type IntermediateHost, Intermediates } from "./intermediates";
import { paddedBytesPerRow } from "./readback";

/** What a view needs of the engine that made it. */
export interface ViewHost extends IntermediateHost {
  readonly device: GPUDevice;
  /** Draws `frame` into `output`, submitted before this returns. */
  renderFrame(frame: FrameSubmission, output: FrameOutput): void;
  /** Forgets a view its caller disposed. */
  forgetView(view: RenderView): void;
}

/** Whether `context` is a WebGPU canvas context, by the members only it has. */
function isGpuCanvasContext(context: unknown): context is GPUCanvasContext {
  return (
    typeof context === "object" &&
    context !== null &&
    "configure" in context &&
    "getCurrentTexture" in context
  );
}

/** The sRGB view format of an 8-bit canvas format. */
export function srgbViewFormat(format: GPUTextureFormat): GPUTextureFormat {
  if (format === "bgra8unorm") {
    return "bgra8unorm-srgb";
  }
  if (format === "rgba8unorm") {
    return "rgba8unorm-srgb";
  }
  throw new Error(`a canvas of format ${format} has no sRGB view`);
}

/**
 * The format a view's pass writes its canvas through: the sRGB twin by default, so that the store
 * encodes, or the canvas's own format under `encoding` `"in-pass"` (R07.T15).
 */
export function canvasPassFormat(
  format: GPUTextureFormat,
  encoding: FrameSubmission["encoding"],
): GPUTextureFormat {
  return encoding === "in-pass" ? format : srgbViewFormat(format);
}

/** A view's depth at a size: `depth32float`, attached only. */
export function viewDepthSpec(name: string, size: ViewSize): TextureSpec {
  return {
    name: `${name}:depth`,
    size: [size.widthPx, size.heightPx],
    dimension: "2d",
    format: "depth32float",
    mips: 1,
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT,
    category: "render-targets",
  };
}

/** A canvas drawn by the engine's device through its own context. */
export class WebGpuView implements RenderView {
  readonly name: string;
  readonly #host: ViewHost;
  readonly #canvas: HTMLCanvasElement;
  readonly #context: GPUCanvasContext;
  readonly #format: GPUTextureFormat;
  readonly #intermediates: Intermediates;
  #depth: {
    readonly size: ViewSize;
    readonly handle: TextureHandle;
    readonly view: GPUTextureView;
  } | null = null;
  #size: ViewSize;
  #lastTexture: GPUTexture | null = null;
  #disposed = false;

  constructor(host: ViewHost, canvas: HTMLCanvasElement, name: string, format: GPUTextureFormat) {
    const context = canvas.getContext("webgpu");
    if (!isGpuCanvasContext(context)) {
      throw new Error(`canvas for view ${name} gives no WebGPU context`);
    }
    this.name = name;
    this.#host = host;
    this.#canvas = canvas;
    this.#context = context;
    this.#format = format;
    this.#intermediates = new Intermediates(host, name, "render-targets");
    this.#size = { widthPx: Math.max(1, canvas.width), heightPx: Math.max(1, canvas.height) };
    this.#configure();
  }

  /** The size the view last drew at, or will draw at next. */
  get size(): ViewSize {
    return this.#size;
  }

  resize(size: ViewSize): void {
    if (size.widthPx === this.#size.widthPx && size.heightPx === this.#size.heightPx) {
      return;
    }
    this.#size = size;
    this.#canvas.width = size.widthPx;
    this.#canvas.height = size.heightPx;
    this.#releaseDepth();
    this.#intermediates.release();
  }

  render(frame: FrameSubmission): void {
    if (this.#disposed) {
      return;
    }
    const texture = this.#context.getCurrentTexture();
    const size = { widthPx: texture.width, heightPx: texture.height };
    const colourFormat = canvasPassFormat(this.#format, frame.encoding);
    this.#host.renderFrame(frame, {
      size,
      colour: texture.createView({ format: colourFormat }),
      colourFormat,
      depth: this.#depthAt(size),
      intermediates: () => this.#intermediates.at(size),
    });
    this.#lastTexture = texture;
  }

  /**
   * The harness's copy of the texture the last `render` drew, by `copyTextureToBuffer`: RGBA bytes,
   * sRGB-encoded, rows from the top, whatever the canvas's own byte order.
   *
   * @remarks
   * A canvas texture expires once the task that got it yields, so this is called in the same task
   * as the `render` it reads, before any `await`.
   */
  readBack(): Promise<Uint8Array> {
    const texture = this.#lastTexture;
    if (texture === null) {
      return Promise.reject(new Error(`view ${this.name} has drawn nothing to read back`));
    }
    return readCanvasTexture(this.#host.device, texture, this.#format === "bgra8unorm");
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#releaseDepth();
    this.#intermediates.release();
    this.#context.unconfigure();
    this.#host.forgetView(this);
  }

  #configure(): void {
    this.#canvas.width = this.#size.widthPx;
    this.#canvas.height = this.#size.heightPx;
    this.#context.configure({
      device: this.#host.device,
      format: this.#format,
      viewFormats: [srgbViewFormat(this.#format)],
      usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.COPY_SRC,
      alphaMode: "opaque",
    });
  }

  #depthAt(size: ViewSize): GPUTextureView {
    const depth = this.#depth;
    if (
      depth !== null &&
      depth.size.widthPx === size.widthPx &&
      depth.size.heightPx === size.heightPx
    ) {
      return depth.view;
    }
    this.#releaseDepth();
    const handle = this.#host.createTexture(viewDepthSpec(this.name, size));
    const view = this.#host.gpuTextureOf(handle).createView();
    this.#depth = { size, handle, view };
    return view;
  }

  #releaseDepth(): void {
    if (this.#depth !== null) {
      this.#host.destroyTexture(this.#depth.handle);
      this.#depth = null;
    }
  }
}

/**
 * The texels of an 8-bit four-channel copy, rows unpadded and in RGBA order.
 *
 * @param bgra - Whether the copy's bytes are in BGRA order, a `bgra8unorm` canvas's.
 */
export function unpadRows(
  padded: Uint8Array,
  widthTexels: number,
  heightTexels: number,
  bytesPerRow: number,
  bgra: boolean,
): Uint8Array {
  const rowBytes = widthTexels * 4;
  const texels = new Uint8Array(rowBytes * heightTexels);
  for (let row = 0; row < heightTexels; row += 1) {
    texels.set(padded.subarray(row * bytesPerRow, row * bytesPerRow + rowBytes), row * rowBytes);
  }
  if (bgra) {
    for (let texel = 0; texel < texels.length; texel += 4) {
      const blue = texels[texel] ?? 0;
      texels[texel] = texels[texel + 2] ?? 0;
      texels[texel + 2] = blue;
    }
  }
  return texels;
}

/** Copies an 8-bit canvas texture to the CPU as RGBA, rows unpadded. */
async function readCanvasTexture(
  device: GPUDevice,
  texture: GPUTexture,
  bgra: boolean,
): Promise<Uint8Array> {
  const { width, height } = texture;
  const bytesPerRow = paddedBytesPerRow(width, 4);
  // A transient staging buffer, not GPU memory a view keeps: made here, destroyed below.
  const staging = device.createBuffer({
    label: "view readback",
    size: bytesPerRow * height,
    usage: BUFFER_USAGE.COPY_DST | BUFFER_USAGE.MAP_READ,
  });
  const encoder = device.createCommandEncoder({ label: "view readback" });
  encoder.copyTextureToBuffer({ texture }, { buffer: staging, bytesPerRow }, { width, height });
  device.queue.submit([encoder.finish()]);
  try {
    await staging.mapAsync(MAP_MODE.READ);
    return unpadRows(new Uint8Array(staging.getMappedRange()), width, height, bytesPerRow, bgra);
  } finally {
    staging.destroy();
  }
}
