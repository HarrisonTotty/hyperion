/**
 * The smoke harness's measure of what the shadow cascades darken (plan R10, R10.T9.b): a frame
 * drawn with the cascades against the same draws at w = 0, texel by texel over its terrain, read
 * back four channels a texel.
 */

import { isTerrain } from "./leverCaptures";

/** How much brighter than at w = 0 a texel of the frame with cascades may be, relative. */
export const NEVER_BRIGHTER = 1e-3;

/** The frame with cascades against the same draws at w = 0 ({@link againstUnshadowed}). */
export interface AgainstUnshadowed {
  /** The texels both draw as terrain. */
  readonly terrain: number;
  /** Those darker by more than {@link NEVER_BRIGHTER}, relative, in a channel at the floor or above. */
  readonly darker: number;
  /** Those darker by more than half in such a channel. */
  readonly halved: number;
  /** The most any is brighter, relative, a channel under the floor taken at it. */
  readonly brighter: number;
}

/**
 * The frame with cascades, `shadowed`, against the same draws at w = 0, `open`, over the texels
 * `open` draws as terrain. The floor is a millionth of `open`'s brightest channel: a channel under
 * it at w = 0 counts as darker nowhere, so that a texel as dark in both counts in neither.
 */
export function againstUnshadowed(shadowed: Float32Array, open: Float32Array): AgainstUnshadowed {
  const texels = Math.floor(open.length / 4);
  let brightest = 0;
  for (let p = 0; p < texels; p += 1) {
    brightest = Math.max(brightest, open[4 * p] ?? 0, open[4 * p + 1] ?? 0, open[4 * p + 2] ?? 0);
  }
  const floor = Math.max(1e-6 * brightest, Number.MIN_VALUE);
  let terrain = 0;
  let darker = 0;
  let halved = 0;
  let brighter = 0;
  for (let p = 0; p < texels; p += 1) {
    if (!isTerrain(open, p)) {
      continue;
    }
    terrain += 1;
    let dark = false;
    let half = false;
    for (let c = 0; c < 3; c += 1) {
      const drawn = open[4 * p + c] ?? Number.NaN;
      const lit = Math.max(drawn, floor);
      const seen = shadowed[4 * p + c] ?? Number.NaN;
      brighter = Math.max(brighter, (seen - lit) / lit);
      if (drawn >= floor) {
        dark ||= seen < drawn * (1 - NEVER_BRIGHTER);
        half ||= seen < drawn / 2;
      }
    }
    darker += dark ? 1 : 0;
    halved += half ? 1 : 0;
  }
  return { terrain, darker, halved, brighter };
}
