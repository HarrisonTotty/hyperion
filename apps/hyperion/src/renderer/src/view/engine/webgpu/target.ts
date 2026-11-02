/**
 * Offscreen render targets: drawn like a view, sampled as a texture by later passes (R01 Design
 * notes 19 and 23).
 *
 * @remarks
 * A target's colour and depth are textures of the engine's own, made through the one creation
 * path with its category, so their bytes are counted, and its depth is a `depth32float` with
 * `COPY_SRC` and `TEXTURE_BINDING`, so that `readTexture` reads it and a later draw samples it
 * (`texture_depth_2d`). The adapter's own pass draws into them, in WebGPU's orientation, as a
 * view's does. Mips past the first are generated after each render.
 *
 * An ID target (`r32uint`, plan R14, R14.T2.b) has one mip and no post-process, and with
 * `depthOf` draws with another target's depth in place of one of its own, loaded and stored
 * whatever its frame's `colourLoad`, so that its draws are tested against what the other target
 * drew; a draw that samples that depth is refused as a sample of its own would be.
 */

import { TEXTURE_USAGE } from "../gpuFlags";
import type { TextureSpec } from "../memory";
import type { GpuCapabilities } from "../platform";
import {
  ColourSelfSample,
  DepthSelfSample,
  type DrawItem,
  type FrameSubmission,
  type RenderTarget,
  type RenderTargetSpec,
  type TextureHandle,
  type ViewSize,
} from "../types";
import { assertScissorWithin, type FrameOutput } from "./drawing";
import { type IntermediateHost, Intermediates } from "./intermediates";

/** What a target needs of the engine that made it. */
export interface TargetHost extends IntermediateHost {
  /** Draws `frame` into `output`, submitted before this returns. */
  renderFrame(frame: FrameSubmission, output: FrameOutput): void;
  /** Encodes the mips of a texture past its first, submitted after the frame. */
  generateMips(texture: TextureHandle): void;
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
    // COPY_DST for a frame that takes another target's colour first (`FrameSubmission.colourFrom`).
    usage:
      TEXTURE_USAGE.RENDER_ATTACHMENT |
      TEXTURE_USAGE.TEXTURE_BINDING |
      TEXTURE_USAGE.COPY_SRC |
      TEXTURE_USAGE.COPY_DST,
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
 * @throws Error naming the target when its format is not renderable there, it asks for more mips
 * than the size has or an `r32uint` target for more than one, or it asks for its own depth and
 * {@link RenderTargetSpec.depthOf}'s both (plan R14, R14.T2.b).
 */
export function assertTargetFits(
  spec: RenderTargetSpec,
  size: ViewSize,
  capabilities: GpuCapabilities,
): void {
  if (spec.format === "rg11b10ufloat" && !capabilities.rg11b10Renderable) {
    throw new Error(`target ${spec.name}: the device cannot render to rg11b10ufloat`);
  }
  if (spec.format === "r32uint" && spec.mips !== 1) {
    throw new Error(`target ${spec.name}: an r32uint target has one mip, not ${String(spec.mips)}`);
  }
  if (spec.depthOf !== undefined && spec.depth) {
    throw new Error(
      `target ${spec.name} draws with ${spec.depthOf.name}'s depth and asks for its own`,
    );
  }
  const mipsAvailable = Math.floor(Math.log2(Math.max(size.widthPx, size.heightPx))) + 1;
  if (!Number.isInteger(spec.mips) || spec.mips < 1 || spec.mips > mipsAvailable) {
    throw new Error(
      `target ${spec.name} asks for ${spec.mips} mips; ${size.widthPx} × ${size.heightPx} has ${mipsAvailable}`,
    );
  }
}

/**
 * Refuses a draw that samples the depth of the target it renders into: WebGPU forbids a texture as
 * an attachment and a binding in one pass (Design note 21).
 *
 * @throws {@link DepthSelfSample} naming the target and the draw's material.
 */
export function assertNoDepthSelfSample(
  targetName: string,
  depth: TextureHandle | null,
  draws: ReadonlyArray<DrawItem>,
): void {
  if (depth === null) {
    return;
  }
  for (const draw of draws) {
    if (Object.values(draw.textures).includes(depth)) {
      throw new DepthSelfSample(targetName, draw.material.name);
    }
  }
}

/**
 * Refuses a draw or a post-process that samples the colour of the target it renders into, for the
 * same reason, in the one pass that has the target's colour as its attachment: the draws' pass
 * when the frame has no post-processes, else the chain's last pass. A draw with post-processes
 * renders into an intermediate, and an earlier post-process into the other one, so either may
 * sample the target's colour.
 *
 * @remarks
 * A post-process whose shaders failed is left out of the chain at encoding, which can make the
 * draws' pass write the target after all; such a frame is already reported by `shader-refused`.
 *
 * @throws {@link ColourSelfSample} naming the target and the material or post-process.
 */
export function assertNoColourSelfSample(
  targetName: string,
  colour: TextureHandle,
  frame: Pick<FrameSubmission, "draws" | "postProcesses">,
): void {
  const last = frame.postProcesses.at(-1);
  if (last === undefined) {
    for (const draw of frame.draws) {
      if (Object.values(draw.textures).includes(colour)) {
        throw new ColourSelfSample(targetName, `material ${draw.material.name}`);
      }
    }
  } else if (Object.values(last.textures ?? {}).includes(colour)) {
    throw new ColourSelfSample(targetName, `post-process ${last.postProcess.name}`);
  }
}

/** A target's textures at one size. */
interface Attachments {
  readonly size: ViewSize;
  readonly colour: TextureHandle;
  readonly depth: TextureHandle | null;
}

/** An offscreen colour target with its own depth. */
export class WebGpuRenderTarget implements RenderTarget {
  readonly name: string;
  readonly #spec: RenderTargetSpec;
  readonly #host: TargetHost;
  readonly #intermediates: Intermediates;
  #attachments: Attachments;
  #disposed = false;

  constructor(host: TargetHost, spec: RenderTargetSpec) {
    this.name = spec.name;
    this.#spec = spec;
    this.#host = host;
    this.#intermediates = new Intermediates(host, spec.name, spec.category);
    this.#attachments = this.#attach(spec.size);
  }

  get colour(): TextureHandle {
    return this.#attachments.colour;
  }

  get depth(): TextureHandle | null {
    return this.#spec.depthOf?.depth ?? this.#attachments.depth;
  }

  resize(size: ViewSize): void {
    if (this.#disposed) {
      throw new Error(`target ${this.name} is disposed`);
    }
    this.#release();
    this.#attachments = this.#attach(size);
  }

  /**
   * Renders `frame` into the target.
   *
   * @throws {@link DepthSelfSample} when a draw samples the depth this target draws with, its own
   *   or `depthOf`'s.
   * @throws Error for an ID target's frame with post-processes, or a `depthOf` target with no
   *   depth or of another size.
   * @throws {@link ColourSelfSample} when a draw or a post-process samples its own colour, or the
   *   frame copies it into itself.
   * @throws RangeError for a scissor outside the target.
   */
  render(frame: FrameSubmission): void {
    if (this.#disposed) {
      return;
    }
    assertNoDepthSelfSample(this.name, this.depth, frame.draws);
    assertNoColourSelfSample(this.name, this.colour, frame);
    const { size, colour } = this.#attachments;
    const depth = this.#depthInUse(size);
    if (this.#spec.format === "r32uint" && frame.postProcesses.length > 0) {
      throw new Error(`target ${this.name}: an r32uint target takes no post-process`);
    }
    if (frame.colourFrom === colour) {
      throw new ColourSelfSample(this.name, "the frame's colour copy");
    }
    if (frame.scissor !== undefined) {
      assertScissorWithin(frame.scissor, size);
    }
    const host = this.#host;
    const texture = host.gpuTextureOf(colour);
    host.renderFrame(frame, {
      size,
      colour: texture.createView({ baseMipLevel: 0, mipLevelCount: 1 }),
      colourTexture: texture,
      colourFormat: this.#spec.format,
      depth: depth === null ? null : host.gpuTextureOf(depth).createView(),
      ...(this.#spec.depthOf === undefined ? {} : { depthLoad: "load" as const }),
      intermediates: () => this.#intermediates.at(size),
    });
    host.wroteByDraw(colour);
    if (depth !== null) {
      host.wroteByDraw(depth);
    }
    if (this.#spec.mips > 1) {
      host.generateMips(colour);
    }
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#release();
    this.#host.forgetTarget(this);
  }

  /**
   * The depth this target draws with at `size`: its own, or `depthOf`'s, which must be there and
   * of the same size.
   *
   * @throws Error for a `depthOf` target with no depth or of another size.
   */
  #depthInUse(size: ViewSize): TextureHandle | null {
    const other = this.#spec.depthOf;
    if (other === undefined) {
      return this.#attachments.depth;
    }
    const shared = other.depth;
    if (shared === null) {
      throw new Error(`target ${this.name} draws with ${other.name}'s depth, which has none`);
    }
    const texture = this.#host.gpuTextureOf(shared);
    if (texture.width !== size.widthPx || texture.height !== size.heightPx) {
      throw new Error(
        `target ${this.name} of ${String(size.widthPx)} × ${String(size.heightPx)} draws with ${other.name}'s depth of ${String(texture.width)} × ${String(texture.height)}`,
      );
    }
    return shared;
  }

  #attach(size: ViewSize): Attachments {
    assertTargetFits(this.#spec, size, this.#host.capabilities);
    const colour = this.#host.createTexture(colourSpec(this.#spec, size));
    const depth = this.#spec.depth ? this.#host.createTexture(depthSpec(this.#spec, size)) : null;
    return { size, colour, depth };
  }

  #release(): void {
    const { colour, depth } = this.#attachments;
    this.#host.destroyTexture(colour);
    if (depth !== null) {
      this.#host.destroyTexture(depth);
    }
    this.#intermediates.release();
  }
}
