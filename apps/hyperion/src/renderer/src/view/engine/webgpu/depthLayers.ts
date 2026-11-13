/**
 * Depth-only targets of array layers (plan R10, R10.T9.b): the terrain's shadow cascades, one
 * `depth32float` 2D-array texture whose layers are each a pass's only attachment.
 *
 * @remarks
 * The texture is the engine's own, made through the one creation path under the target's
 * category ({@link depthLayersTexture}), so its bytes are counted. Each layer's attachment is a
 * single-layer `2d` view of it, made once; a later pass samples the whole as a
 * `texture_depth_2d_array` through the draw path's own depth-aspect view. Each pass is a
 * submission of its own, since the draw path shares one `Frame` buffer and one ring between
 * submissions (decision-r10-t9b-design D1).
 */

import { assertDepthLayerPasses, assertDepthLayersFit, depthLayersTexture } from "../depthLayers";
import type { TextureSpec } from "../memory";
import type { GpuCapabilities } from "../platform";
import type { DepthLayerPass, DepthLayersSpec, DepthLayerTarget, TextureHandle } from "../types";
import type { DepthLayerOutput } from "./drawing";

/** What a depth-layer target needs of the engine that made it. */
export interface DepthLayersHost {
  /** The device's limits, against which the layers are checked. */
  readonly capabilities: GpuCapabilities;
  createTexture(spec: TextureSpec): TextureHandle;
  gpuTextureOf(handle: TextureHandle): GPUTexture;
  destroyTexture(handle: TextureHandle): void;
  /**
   * Encodes one pass into `output` and submits it, its timestamps left for the next resolve,
   * before it returns.
   *
   * @param label - The submission's command encoder's label.
   */
  renderDepthLayer(label: string, pass: DepthLayerPass, output: DepthLayerOutput): void;
  /** Records that a draw, not a kernel, wrote a texture. */
  wroteByDraw(texture: TextureHandle): void;
  /** Forgets a target its caller disposed. */
  forgetDepthLayers(target: DepthLayerTarget): void;
}

/** The single-layer attachment view of layer `index` of `texture`. */
export function layerAttachmentView(texture: GPUTexture, index: number): GPUTextureView {
  return texture.createView({
    label: `${texture.label} layer ${index}`,
    dimension: "2d",
    baseArrayLayer: index,
    arrayLayerCount: 1,
  });
}

/** A depth-only target of array layers on the engine's device. */
export class WebGpuDepthLayers implements DepthLayerTarget {
  readonly name: string;
  readonly sizePx: number;
  readonly layers: number;
  readonly depth: TextureHandle;
  readonly #host: DepthLayersHost;
  readonly #views: ReadonlyArray<GPUTextureView>;
  #disposed = false;

  /**
   * Makes the layers' texture and each layer's attachment view.
   *
   * @throws Error naming the target where its size or layer count does not fit the device.
   */
  constructor(host: DepthLayersHost, spec: DepthLayersSpec) {
    assertDepthLayersFit(spec, host.capabilities);
    this.name = spec.name;
    this.sizePx = spec.sizePx;
    this.layers = spec.layers;
    this.#host = host;
    this.depth = host.createTexture(depthLayersTexture(spec));
    const texture = host.gpuTextureOf(this.depth);
    this.#views = Array.from({ length: spec.layers }, (_, index) =>
      layerAttachmentView(texture, index),
    );
  }

  /**
   * Draws `passes`, each into its layer, each in a submission of its own.
   *
   * @throws Error naming the target and the layer for a layer outside the target or named twice.
   * @throws {@link DepthSelfSample} when a draw samples the target's own depth.
   */
  render(owner: string, passes: ReadonlyArray<DepthLayerPass>): void {
    if (this.#disposed || passes.length === 0) {
      return;
    }
    assertDepthLayerPasses(this, passes);
    const size = { widthPx: this.sizePx, heightPx: this.sizePx };
    for (const pass of passes) {
      const depth = this.#views[pass.layer];
      if (depth === undefined) {
        throw new Error(`${this.name} has no layer ${pass.layer}`);
      }
      this.#host.renderDepthLayer(`${owner} ${pass.label}`, pass, { size, depth });
    }
    this.#host.wroteByDraw(this.depth);
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#host.destroyTexture(this.depth);
    this.#host.forgetDepthLayers(this);
  }
}
