/**
 * Offscreen render targets: drawn like a view, sampled as a texture by later passes (R01 Design
 * note 19).
 *
 * @remarks
 * A target's colour and depth are textures of the engine's own, made through the one creation
 * path with its category, so their bytes are counted, and its depth is a `depth32float` with
 * `COPY_SRC` and `TEXTURE_BINDING`: Babylon's generated depth has neither `COPY_SRC` nor a usage
 * the adapter chooses (`webgpuTextureManager.js:834-838` in 9.28.0 gives it `TEXTURE_BINDING` and
 * `RENDER_ATTACHMENT` only), so `readTexture` could not read it. Babylon draws into them through a
 * `RenderTargetTexture` whose colour attachment is the wrapped colour and whose depth is set to the
 * wrapped depth, rendering in WebGPU's orientation (`_disableEngineYFlip`), as a view does. Mips
 * past the first are generated after each render.
 */

import { Constants } from "@babylonjs/core/Engines/constants";
import { WebGPURenderTargetWrapper } from "@babylonjs/core/Engines/WebGPU/webgpuRenderTargetWrapper";
import type { Camera } from "@babylonjs/core/Cameras/camera.pure";
import type { WebGPUEngine } from "@babylonjs/core/Engines/webgpuEngine.pure";
import { RenderTargetTexture } from "@babylonjs/core/Materials/Textures/renderTargetTexture.pure";
import { Matrix } from "@babylonjs/core/Maths/math.vector.pure";
import type { Scene } from "@babylonjs/core/scene.pure";

import { TEXTURE_USAGE } from "../gpuFlags";
import type { TextureSpec } from "../memory";
import type { GpuCapabilities } from "../platform";
import type {
  FrameSubmission,
  RenderTarget,
  RenderTargetSpec,
  TextureHandle,
  ViewSize,
} from "../types";
import { disableEngineYFlip } from "./internals";
import type { ResourceRegistry } from "./resources";
import type { RawAttachments } from "./rawPass";
import { CLEAR_COLOUR, frozenCamera, releaseTarget } from "./view";

/** What a target needs of the engine that made it. */
export interface TargetHost {
  readonly babylonEngine: WebGPUEngine;
  readonly scene: Scene;
  readonly resources: ResourceRegistry;
  /** Renders `frame` into `target`, its indirect draws into `attachments` after. */
  renderFrame(
    frame: FrameSubmission,
    target: RenderTargetTexture,
    attachments: () => RawAttachments,
  ): void;
  /** Encodes the mips of a texture past its first, submitted after the frame. */
  generateMips(texture: TextureHandle): void;
  /** Destroys a texture and whatever wrapped it for Babylon. */
  destroyTexture(texture: TextureHandle): void;
  /** The device's features, against which a target's format is checked. */
  readonly capabilities: GpuCapabilities;
  /** Records that a draw, not a kernel, wrote a texture. */
  wroteByDraw(texture: TextureHandle): void;
  /** Forgets a target its caller disposed. */
  forgetTarget(target: RenderTarget): void;
}

/** The colour texture of a target at a size. */
export function colourSpec(spec: RenderTargetSpec, size: ViewSize): TextureSpec {
  return {
    name: `${spec.name}:colour`,
    size: [size.widthPx, size.heightPx],
    dimension: "2d",
    format: spec.format,
    mips: spec.mips,
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC,
    category: spec.category,
  };
}

/** The depth texture of a target at a size: `depth32float`, readable and sampleable. */
export function depthSpec(spec: RenderTargetSpec, size: ViewSize): TextureSpec {
  return {
    name: `${spec.name}:depth`,
    size: [size.widthPx, size.heightPx],
    dimension: "2d",
    format: "depth32float",
    mips: 1,
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC,
    category: spec.category,
  };
}

/**
 * Checks that a target of `spec` can be made at `size` on a device of `capabilities`.
 *
 * @throws Error naming the target when its format is not renderable there, or it asks for more
 * mips than the size has.
 */
export function assertTargetFits(
  spec: RenderTargetSpec,
  size: ViewSize,
  capabilities: GpuCapabilities,
): void {
  if (spec.format === "rg11b10ufloat" && !capabilities.rg11b10Renderable) {
    throw new Error(`target ${spec.name}: the device cannot render to rg11b10ufloat`);
  }
  const mipsAvailable = Math.floor(Math.log2(Math.max(size.widthPx, size.heightPx))) + 1;
  if (!Number.isInteger(spec.mips) || spec.mips < 1 || spec.mips > mipsAvailable) {
    throw new Error(
      `target ${spec.name} asks for ${spec.mips} mips; ${size.widthPx} × ${size.heightPx} has ${mipsAvailable}`,
    );
  }
}

/** Babylon's objects and the engine's textures for a target at one size. */
interface Attachments {
  readonly colour: TextureHandle;
  readonly depth: TextureHandle | null;
  readonly target: RenderTargetTexture;
}

/** An offscreen colour target with its own depth. */
export class BabylonRenderTarget implements RenderTarget {
  readonly name: string;
  readonly #spec: RenderTargetSpec;
  readonly #host: TargetHost;
  readonly #camera: Camera;
  readonly #projection = new Matrix();
  #attachments: Attachments;
  #disposed = false;

  constructor(host: TargetHost, spec: RenderTargetSpec) {
    this.name = spec.name;
    this.#spec = spec;
    this.#host = host;
    this.#camera = frozenCamera(spec.name, host.scene);
    this.#attachments = this.#attach(spec.size);
  }

  get colour(): TextureHandle {
    return this.#attachments.colour;
  }

  get depth(): TextureHandle | null {
    return this.#attachments.depth;
  }

  resize(size: ViewSize): void {
    if (this.#disposed) {
      throw new Error(`target ${this.name} is disposed`);
    }
    this.#release();
    this.#attachments = this.#attach(size);
  }

  render(frame: FrameSubmission): void {
    if (this.#disposed) {
      return;
    }
    const { target, colour, depth } = this.#attachments;
    Matrix.FromArrayToRef(frame.projection, 0, this.#projection);
    this.#camera.freezeProjectionMatrix(this.#projection);
    target.activeCamera = this.#camera;
    const { resources } = this.#host;
    this.#host.renderFrame(frame, target, () => ({
      colour: resources.textureOf(colour).texture.createView({ baseMipLevel: 0, mipLevelCount: 1 }),
      colourFormat: this.#spec.format,
      depth: depth === null ? null : resources.textureOf(depth).texture.createView(),
    }));
    this.#host.wroteByDraw(colour);
    if (depth !== null) {
      this.#host.wroteByDraw(depth);
    }
    if (this.#spec.mips > 1) {
      this.#host.generateMips(colour);
    }
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#release();
    this.#camera.dispose();
    this.#host.forgetTarget(this);
  }

  #attach(size: ViewSize): Attachments {
    assertTargetFits(this.#spec, size, this.#host.capabilities);
    const { resources, babylonEngine: engine, scene } = this.#host;
    const colour = resources.createTexture(colourSpec(this.#spec, size));
    const depth = this.#spec.depth ? resources.createTexture(depthSpec(this.#spec, size)) : null;
    const target = new RenderTargetTexture(
      `${this.name}:target`,
      { width: size.widthPx, height: size.heightPx },
      scene,
      {
        colorAttachment: engine.wrapWebGPUTexture(resources.textureOf(colour).texture),
        generateDepthBuffer: false,
        generateStencilBuffer: false,
        generateMipMaps: false,
      },
    );
    target.clearColor = CLEAR_COLOUR;
    target.ignoreCameraViewport = true;
    const wrapper = target.renderTarget;
    if (!(wrapper instanceof WebGPURenderTargetWrapper)) {
      throw new Error(`target ${this.name} has no WebGPU render target`);
    }
    if (depth !== null) {
      const wrappedDepth = engine.wrapWebGPUTexture(resources.textureOf(depth).texture);
      // Babylon builds the depth attachment's view from the texture's own format and type, which
      // a wrapped texture leaves at their colour defaults.
      wrappedDepth.format = Constants.TEXTUREFORMAT_DEPTH32_FLOAT;
      wrappedDepth.type = Constants.TEXTURETYPE_FLOAT;
      wrapper.setDepthStencilTexture(wrappedDepth, false);
    }
    disableEngineYFlip(wrapper);
    return { colour, depth, target };
  }

  #release(): void {
    const { target, colour, depth } = this.#attachments;
    releaseTarget(target);
    this.#host.destroyTexture(colour);
    if (depth !== null) {
      this.#host.destroyTexture(depth);
    }
  }
}
