/**
 * A sky star's light for display: its illuminance per channel from its V and its chroma, and its
 * pixels' luminance through R02's point-spread function (plan R06, Design notes 17 and 20,
 * T13.a).
 *
 * @remarks
 * V tracks photopic illuminance to within 0.08 mag from O5 to M6 (R02's `illuminanceLx`), so a
 * star's illuminance is 2.54 µlx × 10^(−0.4 V), split over the display's channels by its chroma
 * at unit luminance. The wire's chroma is the chromaticity r ÷ (r + g + b) and g ÷ (r + g + b)
 * (`hyperion_protocol::sky`); dividing (r, g, b) by its Rec. 709 luminance, 0.2126 r + 0.7152 g
 * + 0.0722 b (ITU-R BT.709-6, Table 1, item 3.2), gives the colour of unit luminance, so the
 * green-weighted sum of the three channels is the star's illuminance exactly.
 */

import { illuminanceLx, pixelLuminance } from "../photometry/magnitude";
import type { Rgb } from "../photometry/toneCurve";

/** Rec. 709's luminance weights of linear red, green and blue (ITU-R BT.709-6). */
export const REC709_LUMA: Rgb = [0.2126, 0.7152, 0.0722];

/**
 * The linear Rec. 709 colour of unit luminance of a wire chromaticity.
 *
 * @param chromaR - r ÷ (r + g + b), in [0, 1].
 * @param chromaG - g ÷ (r + g + b), in [0, 1]; b's is one less the two.
 * @returns White, (1, 1, 1), for a chromaticity with no luminance, which the wire never sends.
 */
export function unitLuminanceRgb(chromaR: number, chromaG: number): Rgb {
  const chromaB = Math.max(0, 1 - chromaR - chromaG);
  const luminance = REC709_LUMA[0] * chromaR + REC709_LUMA[1] * chromaG + REC709_LUMA[2] * chromaB;
  if (!(luminance > 0)) {
    return [1, 1, 1];
  }
  return [chromaR / luminance, chromaG / luminance, chromaB / luminance];
}

/**
 * A star's illuminance per channel, lx: its photopic illuminance spread over the channels by its
 * colour of unit luminance.
 *
 * @param vMag - The apparent V after extinction.
 */
export function starIlluminanceRgbLx(vMag: number, chromaR: number, chromaG: number): Rgb {
  const total = illuminanceLx(vMag);
  const [r, g, b] = unitLuminanceRgb(chromaR, chromaG);
  return [total * r, total * g, total * b];
}

/**
 * One pixel's luminance per channel from a star, cd/m²: its share of the star's illuminance over
 * the pixel's true solid angle, R02's `pixelLuminance` per channel.
 *
 * @param psfWeight - The pixel's share, from R02's `psfPixelWeights`.
 * @param pixelSolidAngleSr - The pixel's solid angle, sr.
 */
export function starPixelLuminanceRgb(
  illuminanceRgbLx: Rgb,
  psfWeight: number,
  pixelSolidAngleSr: number,
): Rgb {
  return [
    pixelLuminance(illuminanceRgbLx[0], psfWeight, pixelSolidAngleSr),
    pixelLuminance(illuminanceRgbLx[1], psfWeight, pixelSolidAngleSr),
    pixelLuminance(illuminanceRgbLx[2], psfWeight, pixelSolidAngleSr),
  ];
}
