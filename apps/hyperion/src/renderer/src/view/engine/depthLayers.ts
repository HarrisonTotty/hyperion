/**
 * The engine-agnostic rules of depth-only rendering (plan R10, R10.T9.b): a
 * {@link DepthLayerTarget}'s texture, the checks of its creation and its passes, and the refusal
 * of `unclippedDepth` without `depth-clip-control`.
 *
 * @remarks
 * The WebGPU adapter and the test fakes share them, so that a fake makes the texture the engine
 * makes, with its bytes, and refuses what the engine refuses.
 */

import { TEXTURE_USAGE } from "./gpuFlags";
import type { TextureSpec } from "./memory";
import { type GpuCapabilities, MAX_TEXTURE_ARRAY_LAYERS } from "./platform";
import {
  type DepthLayerPass,
  type DepthLayersSpec,
  DepthSelfSample,
  type TextureHandle,
  type WgslMaterialSpec,
} from "./types";

/**
 * The one texture of a target of `spec`: its layers, `depth32float`, one mip, attached and sampled.
 *
 * @remarks
 * It takes the target's name, being the whole of it. No `COPY_SRC`: nothing reads the layers back
 * (the smoke harness reads them through a comparison sampler, as the receivers do).
 */
export function depthLayersTexture(spec: DepthLayersSpec): TextureSpec {
  return {
    name: spec.name,
    size: { width: spec.sizePx, height: spec.sizePx, depthOrArrayLayers: spec.layers },
    dimension: "2d",
    format: "depth32float",
    mips: 1,
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING,
    category: spec.category,
  };
}

/**
 * Checks that a target of `spec` can be made on a device of `capabilities`.
 *
 * @throws Error naming the target for a size that is not a whole number of texels from 1 to
 *   `maxTextureDimension2D`, or a layer count that is not a whole number from 1 to
 *   {@link MAX_TEXTURE_ARRAY_LAYERS}: the device would refuse the texture with a validation error,
 *   and every pass into it would draw nothing.
 */
export function assertDepthLayersFit(
  spec: DepthLayersSpec,
  capabilities: Pick<GpuCapabilities, "maxTextureDimension2D">,
): void {
  const most = capabilities.maxTextureDimension2D;
  if (!Number.isInteger(spec.sizePx) || spec.sizePx < 1 || spec.sizePx > most) {
    throw new Error(
      `depth layers ${spec.name} of ${spec.sizePx} px a side: the device takes 1 to ${most}`,
    );
  }
  if (!Number.isInteger(spec.layers) || spec.layers < 1 || spec.layers > MAX_TEXTURE_ARRAY_LAYERS) {
    throw new Error(
      `depth layers ${spec.name} of ${spec.layers} layers: the device takes 1 to ${MAX_TEXTURE_ARRAY_LAYERS}`,
    );
  }
}

/**
 * Checks a render's passes against their target, before anything is encoded.
 *
 * @throws Error naming the target and the layer for a layer outside 0 to `layers` − 1 or named by
 *   two passes: the second pass's clear would lose the first's draws.
 * @throws {@link DepthSelfSample} naming the target and the draw's material when a draw samples
 *   the target's own depth, which WebGPU forbids as an attachment and a binding in one pass and
 *   would answer by dropping the submission.
 */
export function assertDepthLayerPasses(
  target: { readonly name: string; readonly layers: number; readonly depth: TextureHandle },
  passes: ReadonlyArray<DepthLayerPass>,
): void {
  const drawn = new Set<number>();
  for (const pass of passes) {
    if (!Number.isInteger(pass.layer) || pass.layer < 0 || pass.layer >= target.layers) {
      throw new Error(
        `pass ${pass.label} draws into layer ${pass.layer} of ${target.name}, which has layers 0 to ${target.layers - 1}`,
      );
    }
    if (drawn.has(pass.layer)) {
      throw new Error(`pass ${pass.label} draws into layer ${pass.layer} of ${target.name} again`);
    }
    drawn.add(pass.layer);
    for (const draw of pass.draws) {
      if (Object.values(draw.textures).includes(target.depth)) {
        throw new DepthSelfSample(target.name, draw.material.name);
      }
    }
  }
}

/**
 * Checks that a material's fixed-function state is one the device can make.
 *
 * @throws Error naming the material and `depth-clip-control` for a material with
 *   {@link WgslMaterialSpec.unclippedDepth} on a device without that feature, whose pipelines the
 *   device would refuse with a validation error.
 */
export function assertMaterialSupported(
  spec: WgslMaterialSpec,
  capabilities: Pick<GpuCapabilities, "depthClipControl">,
): void {
  if (spec.unclippedDepth === true && !capabilities.depthClipControl) {
    throw new Error(
      `material ${spec.name} asks for unclippedDepth, which needs depth-clip-control, which the device lacks`,
    );
  }
}
