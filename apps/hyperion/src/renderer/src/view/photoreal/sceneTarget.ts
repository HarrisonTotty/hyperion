/**
 * Each photorealistic view's HDR scene target (plan R07, T7; decisions-r06-r07, item 1).
 *
 * @remarks
 * An ordinary R01 offscreen target in R02's `HDR_COLOUR_FORMAT` (`rgba16float`) at the view's
 * internal resolution, with its own reversed-Z `depth32float`, one mip, category `render-targets`,
 * named `<view>:hdr`. One per photorealistic view: made when a view turns photorealistic, resized
 * with its internal scale, disposed when it turns back. No other task creates one; the post passes
 * (T12–T15) take its colour as a `TextureHandle`. R01 clears it to alpha 1, which is
 * `METER_CLASS.other`, so a pixel no opaque pass writes meters as "other".
 */
import type { RenderEngine, RenderTarget, RenderTargetSpec, ViewSize } from "../engine/types";
import { HDR_COLOUR_FORMAT } from "../photometry/toneCurve";

/** The scene target's spec for a view. */
export function sceneTargetSpec(viewName: string, size: ViewSize): RenderTargetSpec {
  return {
    name: `${viewName}:hdr`,
    size,
    format: HDR_COLOUR_FORMAT,
    mips: 1,
    depth: true,
    category: "render-targets",
  };
}

/** Makes a view's scene target; its owner disposes of it and re-creates it after a device loss. */
export function createSceneTarget(
  engine: RenderEngine,
  viewName: string,
  size: ViewSize,
): RenderTarget {
  return engine.createRenderTarget(sceneTargetSpec(viewName, size));
}
