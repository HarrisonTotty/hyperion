/**
 * A scene target's read-back as a capture (plan R10, the terrain light's captures): its HDR texels
 * at one exposure through R02's tone curve, 8-bit sRGB, each pixel repeated `factor` times along
 * both axes so that a small target's pixels can be seen.
 */

import type { Viewport } from "../view/camera/projection";
import { toneCurve } from "../view/photometry/toneCurve";
import { base64Of, type CapturedImage, srgb8 } from "./atmosphere";

/**
 * `texels` (RGBA, row by row from the top, `viewport`'s size) as the capture `name`, each channel
 * times `exposure` before the tone curve, every pixel a `factor` × `factor` block.
 */
export function hdrCapture(
  name: string,
  texels: Float32Array,
  viewport: Viewport,
  exposure: number,
  factor: number,
): CapturedImage {
  const width = viewport.widthPx * factor;
  const height = viewport.heightPx * factor;
  const out = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const from = (Math.floor(y / factor) * viewport.widthPx + Math.floor(x / factor)) * 4;
      const [r, g, b] = toneCurve([
        (texels[from] ?? 0) * exposure,
        (texels[from + 1] ?? 0) * exposure,
        (texels[from + 2] ?? 0) * exposure,
      ]);
      const to = (y * width + x) * 4;
      out[to] = srgb8(r);
      out[to + 1] = srgb8(g);
      out[to + 2] = srgb8(b);
      out[to + 3] = 255;
    }
  }
  return { name, width, height, rgba: base64Of(out) };
}
