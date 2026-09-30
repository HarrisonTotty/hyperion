/**
 * A view's canvas size in device pixels.
 */

import type { ViewSize } from "./types";

/**
 * Canvas pixels from a CSS size: rounded, clamped to the device's 2D texture limit, never zero.
 *
 * @param cssSize - The canvas's CSS box, in CSS pixels.
 * @param devicePixelRatio - Device pixels per CSS pixel.
 * @param maxTextureDimension2D - The device's limit, in pixels on a side.
 */
export function viewPixelSize(
  cssSize: { readonly width: number; readonly height: number },
  devicePixelRatio: number,
  maxTextureDimension2D: number,
): ViewSize {
  const side = (css: number): number => {
    const pixels = Math.round(css * devicePixelRatio);
    return Number.isFinite(pixels) ? Math.min(Math.max(pixels, 1), maxTextureDimension2D) : 1;
  };
  return { widthPx: side(cssSize.width), heightPx: side(cssSize.height) };
}
