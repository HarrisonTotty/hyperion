/**
 * The eye's limit in a direction, from the sky's band map (plan R06, Design notes 2, 4 and 5,
 * T12).
 *
 * @remarks
 * The server gives each band texel the naked-eye limit at the request's field factor F, Crumey's
 * (2014) eq. 34 with the texel's background and the listed stars' glare. Every term of that limit
 * scales ΔI by F, so a view with another field factor F′ reads the texel's limit moved by
 * −2.5 log₁₀(F′ ÷ F): a factor of 2 against 1.4 costs 0.387 mag. The texel is the band cube's
 * texel in the direction, nearest, as the band layer's sampler would read it at its centre.
 */

import type { SkyBand } from "@hyperion/protocol";

import { cubeTexelOf } from "./cube";

/** The default field factor F, 1.4 (Crumey 2014; the sim's `EyeObserver` default). */
export const DEFAULT_FIELD_FACTOR = 1.4;

/**
 * The magnitude a field factor F′ moves a limit computed at F by: −2.5 log₁₀(F′ ÷ F).
 *
 * @throws RangeError for a field factor that is not finite and positive.
 */
export function fieldFactorOffsetMag(viewFieldFactor: number, requestFieldFactor: number): number {
  for (const factor of [viewFieldFactor, requestFieldFactor]) {
    if (!(Number.isFinite(factor) && factor > 0)) {
      throw new RangeError(`a field factor must be finite and positive, not ${factor}`);
    }
  }
  return -2.5 * Math.log10(viewFieldFactor / requestFieldFactor);
}

/** What {@link eyeLimitAt} reads: the band and the field factor it was computed at. */
export interface EyeLimitSource {
  readonly band: SkyBand;
  /** The band's face side, texels (the response's `face_texels`). */
  readonly faceTexels: number;
  /** The request's field factor; `null` where the eye was not asked. */
  readonly requestFieldFactor: number | null;
}

/**
 * The eye's limiting V in a direction on the galactic axes, at the view's field factor; NaN where
 * the sky carries no eye limit.
 *
 * @throws RangeError for a field factor that is not finite and positive; Error for a zero or
 *   non-finite direction.
 */
export function eyeLimitAt(
  source: EyeLimitSource,
  direction: readonly [number, number, number],
  fieldFactor: number,
): number {
  if (source.requestFieldFactor === null) {
    return Number.NaN;
  }
  const { face, column, row } = cubeTexelOf(
    direction[0],
    direction[1],
    direction[2],
    source.faceTexels,
  );
  const limit =
    source.band.eyeLimitMag[(face * source.faceTexels + row) * source.faceTexels + column] ??
    Number.NaN;
  return limit + fieldFactorOffsetMag(fieldFactor, source.requestFieldFactor);
}
