/**
 * One view: a canvas with its own context, target, depth and camera, drawn by the engine's one
 * device (R01 Design note 13).
 *
 * @remarks
 * There is no public way to render a camera into an external canvas context. Each view configures
 * its own `GPUCanvasContext` against the engine's device at the view's own size, with `COPY_SRC`
 * so that the harness can read it back, wraps the context's current texture with
 * `wrapWebGPUTexture`, and draws through a `RenderTargetTexture` whose colour attachment is that
 * texture and whose depth is its own `depth32float`. The target renders in WebGPU's orientation
 * (`_disableEngineYFlip`), so R02's projection is used as given. Each frame the wrapper is pointed
 * at the context's new texture; a resize rebuilds this view's target and nothing else.
 *
 * The canvas is configured with its preferred 8-bit format and that format's `-srgb` twin among its
 * view formats, and Babylon renders through the sRGB view, so that blending happens in linear light
 * and the store encodes (Design note 18).
 */

import { Camera } from "@babylonjs/core/Cameras/camera.pure";
import type { WebGPUEngine } from "@babylonjs/core/Engines/webgpuEngine.pure";
import { WebGPURenderTargetWrapper } from "@babylonjs/core/Engines/WebGPU/webgpuRenderTargetWrapper";
import type { InternalTexture } from "@babylonjs/core/Materials/Textures/internalTexture";
import { RenderTargetTexture } from "@babylonjs/core/Materials/Textures/renderTargetTexture.pure";
import { Color4 } from "@babylonjs/core/Maths/math.color.pure";
import { Matrix, Vector3 } from "@babylonjs/core/Maths/math.vector.pure";
import type { AbstractMesh } from "@babylonjs/core/Meshes/abstractMesh.pure";
import type { PostProcess } from "@babylonjs/core/PostProcesses/postProcess.pure";
import type { Scene } from "@babylonjs/core/scene.pure";

import { BUFFER_USAGE, MAP_MODE, TEXTURE_USAGE } from "../gpuFlags";
import type { FrameSubmission, RenderView, ViewSize } from "../types";
import { disableEngineYFlip, setAttachmentFormat } from "./internals";

/** What a view needs of the engine that made it. */
export interface ViewHost {
  readonly babylonEngine: WebGPUEngine;
  readonly device: GPUDevice;
  readonly scene: Scene;
  /** The Babylon meshes that draw `frame`, bound to their draws. */
  meshesFor(frame: FrameSubmission): AbstractMesh[];
  /** The Babylon post-processes of `frame`, the chain the next render runs. */
  postProcessesFor(frame: FrameSubmission): ReadonlyArray<PostProcess>;
  /** Runs `render` inside one Babylon frame and submits it. */
  inFrame(render: () => void): void;
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

/** The camera a view renders with: at the origin, its projection frozen to the submission's. */
function frozenCamera(name: string, scene: Scene): Camera {
  const camera = new Camera(`${name}:camera`, Vector3.Zero(), scene, false);
  camera.freezeProjectionMatrix(Matrix.Identity());
  return camera;
}

/**
 * Makes `chain` the target's post-processes, in order, changing nothing when it already is.
 *
 * @remarks
 * A target runs its post-processes itself: it draws the scene into the first one's input, with its
 * own depth, and the last one writes the target's colour.
 */
export function setChain(target: RenderTargetTexture, chain: ReadonlyArray<PostProcess>): void {
  // Babylon's `postProcesses` is undefined until one is added, though typed as an array.
  const current = chains.get(target) ?? [];
  if (current.length === chain.length && current.every((pass, index) => pass === chain[index])) {
    return;
  }
  if (current.length > 0) {
    target.clearPostProcesses(false);
  }
  for (const pass of chain) {
    target.addPostProcess(pass);
  }
  chains.set(target, [...chain]);
}

/** The chain each target was last given. */
const chains = new WeakMap<RenderTargetTexture, ReadonlyArray<PostProcess>>();

/** The colour a view clears to: black, opaque. */
const CLEAR_COLOUR = new Color4(0, 0, 0, 1);

/** Babylon's objects for a view at one size. */
interface Attachments {
  readonly size: ViewSize;
  readonly wrapped: InternalTexture;
  readonly target: RenderTargetTexture;
}

/** A canvas drawn by the engine's device through its own context. */
export class BabylonView implements RenderView {
  readonly name: string;
  readonly #host: ViewHost;
  readonly #canvas: HTMLCanvasElement;
  readonly #context: GPUCanvasContext;
  readonly #format: GPUTextureFormat;
  readonly #camera: Camera;
  readonly #projection = new Matrix();
  #attachments: Attachments | null = null;
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
    this.#size = { widthPx: Math.max(1, canvas.width), heightPx: Math.max(1, canvas.height) };
    this.#camera = frozenCamera(name, host.scene);
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
    this.#releaseAttachments();
  }

  render(frame: FrameSubmission): void {
    if (this.#disposed) {
      return;
    }
    const texture = this.#context.getCurrentTexture();
    const { target } = this.#attachmentsFor(texture);
    Matrix.FromArrayToRef(frame.projection, 0, this.#projection);
    this.#camera.freezeProjectionMatrix(this.#projection);
    target.activeCamera = this.#camera;
    this.#host.inFrame(() => {
      target.renderList = this.#host.meshesFor(frame);
      setChain(target, this.#host.postProcessesFor(frame));
      target.render(false);
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
    this.#releaseAttachments();
    this.#camera.dispose();
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

  #attachmentsFor(texture: GPUTexture): Attachments {
    const engine = this.#host.babylonEngine;
    const current = this.#attachments;
    if (
      current !== null &&
      current.size.widthPx === texture.width &&
      current.size.heightPx === texture.height
    ) {
      engine.updateWrappedWebGPUTexture(current.wrapped, texture);
      setAttachmentFormat(current.wrapped, srgbViewFormat(this.#format));
      return current;
    }
    this.#releaseAttachments();
    const wrapped = engine.wrapWebGPUTexture(texture);
    setAttachmentFormat(wrapped, srgbViewFormat(this.#format));
    const size = { widthPx: texture.width, heightPx: texture.height };
    const target = new RenderTargetTexture(
      `${this.name}:target`,
      { width: size.widthPx, height: size.heightPx },
      this.#host.scene,
      {
        colorAttachment: wrapped,
        generateDepthBuffer: true,
        generateStencilBuffer: false,
        generateMipMaps: false,
      },
    );
    target.clearColor = CLEAR_COLOUR;
    target.ignoreCameraViewport = true;
    const wrapper = target.renderTarget;
    if (!(wrapper instanceof WebGPURenderTargetWrapper)) {
      throw new Error(`view ${this.name}'s target has no WebGPU render target`);
    }
    disableEngineYFlip(wrapper);
    const attachments: Attachments = { size, wrapped, target };
    this.#attachments = attachments;
    return attachments;
  }

  #releaseAttachments(): void {
    const attachments = this.#attachments;
    if (attachments === null) {
      return;
    }
    // A target disposes the post-processes it holds, and those are the engine's, shared by handle.
    if (chains.has(attachments.target)) {
      attachments.target.clearPostProcesses(false);
      chains.delete(attachments.target);
    }
    attachments.target.dispose();
    this.#attachments = null;
  }
}

/** Rows of a texel copy are 256-byte aligned in the buffer (WebGPU's `bytesPerRow` rule). */
export const COPY_ROW_ALIGNMENT = 256;

/** The padded row pitch of a copy of `widthTexels` texels of `bytesPerTexel` each. */
export function paddedBytesPerRow(widthTexels: number, bytesPerTexel: number): number {
  return Math.ceil((widthTexels * bytesPerTexel) / COPY_ROW_ALIGNMENT) * COPY_ROW_ALIGNMENT;
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
  // A staging buffer for this one read, which the harness alone makes.
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
